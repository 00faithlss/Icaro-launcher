// SPDX-License-Identifier: GPL-3.0-only
//! Servicio del anfitrión: servidor dedicado o instancia cliente de referencia.
use icaro_core::{instancias::Almacen, modrinth::Catalog, mods::sincronizacion as sync};
use std::{net::SocketAddr, path::PathBuf, sync::Arc, time::{Duration, Instant}};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
#[tokio::main]
async fn main() {
    if let Err(error) = ejecutar().await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
async fn ejecutar() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let mut data = None;
    let mut id = None;
    let mut listen = None;
    let mut servidor = None;
    let mut minecraft = None;
    let mut loader = None;
    let mut loader_version = None;
    let mut excluir = std::collections::BTreeSet::new();
    let mut incluir = std::collections::BTreeSet::new();
    let mut incluir_servidor = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--datos" => data = args.next().map(PathBuf::from),
            "--instancia" => id = args.next(),
            "--escuchar" => listen = args.next(),
            "--servidor" => servidor = args.next().map(PathBuf::from),
            "--minecraft" => minecraft = args.next(),
            "--loader" => loader = args.next(),
            "--version-loader" => loader_version = args.next(),
            "--excluir-archivo" => {
                excluir.insert(
                    args.next()
                        .ok_or("Falta el nombre de JAR tras --excluir-archivo.")?,
                );
            }
            "--incluir-archivo" => {
                incluir.insert(
                    args.next()
                        .ok_or("Falta el nombre de JAR tras --incluir-archivo.")?,
                );
            }
            "--incluir-servidor" => incluir_servidor = true,
            "--help" | "-h" => {
                println!("icaro-sync-host --servidor <carpeta absoluta del servidor> --minecraft <versión> --loader <fabric|quilt|forge|neoforge> --version-loader <versión> --escuchar <IP ZeroTier:puerto Minecraft+1>\nLee mods directamente, sin instalar Ícaro ni crear una instancia. Publica versiones de Modrinth y sirve JAR propios con hash. Ctrl+C detiene el servicio.\nPara excluir un mod exclusivo de servidor: --excluir-archivo <nombre.jar> (repetible).\nPara publicar también mods exclusivos de servidor: --incluir-archivo <nombre.jar> (repetible) o --incluir-servidor (todos).\nAlternativa anterior: --datos <carpeta Icaro> --instancia <ID> --escuchar <IP:puerto>.");
                return Ok(());
            }
            _ => return Err("Argumento desconocido. Usa --help.".into()),
        }
    }
    let origen = if let Some(carpeta) = servidor {
        if data.is_some() || id.is_some() {
            return Err("Usa --servidor o --datos/--instancia, no ambos orígenes.".into());
        }
        let servidor = sync::dedicado::ServidorDedicado {
            carpeta,
            minecraft: minecraft.ok_or("Indica --minecraft con la versión del servidor.")?,
            loader: loader.ok_or("Indica --loader con el loader del servidor.")?,
            loader_version: loader_version
                .ok_or("Indica --version-loader con la versión exacta del loader.")?,
            excluir,
            incluir,
            incluir_servidor,
        };
        servidor.validar()?;
        Origen::Servidor(servidor)
    } else {
        if minecraft.is_some()
            || loader.is_some()
            || loader_version.is_some()
            || !excluir.is_empty()
            || !incluir.is_empty()
            || incluir_servidor
        {
            return Err("Las versiones explícitas requieren --servidor.".into());
        }
        let data = data.ok_or("Indica --servidor con la carpeta del servidor dedicado. Usa --help para los demás argumentos.")?;
        if !data.is_absolute() {
            return Err("La carpeta de datos debe ser absoluta.".into());
        }
        Origen::Instancia {
            store: Almacen::nuevo(data.join("instances")),
            id: id.ok_or("Indica --instancia con el ID guardado en instance.json.")?,
        }
    };
    let addr: SocketAddr = listen
        .ok_or("Indica --escuchar con la IP privada de ZeroTier y puerto Minecraft+1.")?
        .parse()
        .map_err(|_| "Dirección de escucha inválida.")?;
    // Validación común: rechaza comodines y direcciones públicas.
    if addr.port() < 2
        || sync::endpoint(&SocketAddr::new(addr.ip(), addr.port() - 1).to_string())? != addr
    {
        return Err("Dirección de escucha inválida.".into());
    }
    let catalog = Catalog::new().map_err(|e| e.to_string())?.with_metadata_cache();
    let published = Arc::new(RwLock::new(None));
    let cancel = CancellationToken::new();
    let listener = tokio::net::TcpListener::bind(addr).await.map_err(|_| {
        "No se pudo abrir el puerto del servicio. Comprueba la IP de ZeroTier y el cortafuegos."
    })?;
    println!("Servicio de mods en {addr}. Preparando el índice; solo compartirá los JAR propios publicados. No comparte mundos ni cuentas.");
    let mods = match &origen {
        Origen::Servidor(s) => Some(s.carpeta.join("mods")),
        _ => None,
    };
    let service = tokio::spawn(sync::servir_archivos(
        listener,
        published.clone(),
        cancel.clone(),
        mods,
    ));
    let mut last = None;
    let mut attempted = None;
    let mut retry = Reintentos::default();
    loop {
        let refresh = async {
            let signature = match origen.huella() {
                Ok(signature) => signature,
                Err(error) => {
                    *published.write().await = None;
                    return if retry.puede(Instant::now()) { Err(error) } else { Ok(()) };
                }
            };
            if last.as_ref() != Some(&signature) {
                // Un cambio invalida el índice anterior antes de consultar Modrinth.
                *published.write().await = None;
                if attempted.as_ref() != Some(&signature) {
                    retry = Reintentos::default();
                    attempted = Some(signature.clone());
                }
                if !retry.puede(Instant::now()) || !catalog.retry_after().is_zero() {
                    return Ok(());
                }
                let index = origen.publicar(&catalog).await?;
                let count = index.mods.len() + index.archivos.len();
                for aviso in &index.avisos {
                    eprintln!("Aviso: {aviso}");
                }
                *published.write().await = Some(index);
                last = Some(signature);
                retry = Reintentos::default();
                println!("Índice actualizado: {count} mods de cliente, incluyendo archivos propios del anfitrión.");
            }
            Ok::<_, String>(())
        };
        tokio::select! {
            _=tokio::signal::ctrl_c()=>break,
            result=tokio::time::timeout(Duration::from_secs(120),refresh)=>{
                if let Err(error)=result.unwrap_or_else(|_|Err("La preparación del índice agotó el tiempo de espera.".into())) {
                    *published.write().await=None;
                    last=None;
                    let wait = retry.fallo(Instant::now(), catalog.retry_after());
                    eprintln!("{error} Próximo intento en al menos {} segundos; se conservan los metadatos ya consultados.",wait.as_secs());
                }
            }
        }
        tokio::select! {_=tokio::signal::ctrl_c()=>break, _=tokio::time::sleep(Duration::from_secs(10))=>{}}
        if service.is_finished() {
            break;
        }
    }
    cancel.cancel();
    service.await.map_err(|_| "El servicio no pudo cerrarse.")?
}

enum Origen {
    Servidor(sync::dedicado::ServidorDedicado),
    Instancia { store: Almacen, id: String },
}
impl Origen {
    fn instancia(store: &Almacen, id: &str) -> Result<icaro_core::instancias::Instancia, String> {
        store
            .cargar()
            .map_err(|e| e.to_string())?
            .instancias
            .into_iter()
            .find(|i| i.id == id)
            .ok_or("La instancia elegida no existe.".into())
    }
    fn huella(&self) -> Result<(Vec<(String, String)>, String, String, Option<String>), String> {
        match self {
            Self::Servidor(s) => Ok((
                s.huella()?,
                s.minecraft.clone(),
                s.loader.clone(),
                Some(s.loader_version.clone()),
            )),
            Self::Instancia { store, id } => {
                let i = Self::instancia(store, id)?;
                Ok((
                    sync::huella(store, id)?,
                    i.minecraft.clone(),
                    i.loader.clone(),
                    icaro_core::versiones_instancia::leer_loader(store, &i),
                ))
            }
        }
    }
    async fn publicar(&self, catalog: &Catalog) -> Result<sync::Indice, String> {
        match self {
            Self::Servidor(s) => sync::dedicado::publicar(catalog, s).await,
            Self::Instancia { store, id } => {
                sync::publicar(catalog, store, &Self::instancia(store, id)?).await
            }
        }
    }
}

/// Los fallos no convierten el sondeo local de diez segundos en otro barrido de la API.
#[derive(Default)]
struct Reintentos {
    fallos: u32,
    siguiente: Option<Instant>,
}
impl Reintentos {
    fn puede(&self, now: Instant) -> bool { self.siguiente.is_none_or(|next| now >= next) }
    fn fallo(&mut self, now: Instant, proveedor: Duration) -> Duration {
        let seconds = 30u64.saturating_mul(1u64 << self.fallos.min(5)).min(600);
        let wait = Duration::from_secs(seconds).max(proveedor);
        self.fallos = self.fallos.saturating_add(1);
        self.siguiente = Some(now + wait);
        wait
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reintentos_crecen_respetan_proveedor_y_se_reinician_tras_recuperacion() {
        let mut retry = Reintentos::default();
        let mut now = Instant::now();
        for seconds in [30,60,120,240,480,600,600] {
            assert!(retry.puede(now));
            assert_eq!(retry.fallo(now, Duration::ZERO),Duration::from_secs(seconds));
            assert!(!retry.puede(now + Duration::from_secs(10)));
            now += Duration::from_secs(seconds);
        }
        assert_eq!(retry.fallo(now,Duration::from_secs(1800)),Duration::from_secs(1800));
        assert!(!retry.puede(now + Duration::from_secs(1799)));
        retry = Reintentos::default();
        assert_eq!(retry.fallo(now,Duration::ZERO),Duration::from_secs(30));
    }
}
