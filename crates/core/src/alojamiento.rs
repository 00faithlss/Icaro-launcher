// SPDX-License-Identifier: GPL-3.0-only
//! Alojar una instancia como servidor dedicado en este equipo:
//! Ícaro prepara la carpeta del servidor con los mismos mods de la
//! instancia, lo ejecuta con Java y lo publica solo por el túnel cifrado de
//! Compartir (`compartir::transporte::servidor`) a los invitados autorizados.
//!
//! La carpeta vive en `<datos>/alojamiento/<instancia>/` y se conserva entre
//! sesiones: mundo, `server.properties`, listas de jugadores y mods añadidos a
//! mano no se tocan. Solo se reemplazan los JAR que Ícaro copió desde la
//! instancia (registrados en `.icaro-mods.json`).
pub mod proceso;
pub mod propiedades;

use crate::instancias::{Almacen, Instancia};
use propiedades::Propiedades;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use tokio_util::sync::CancellationToken;

pub const EULA_URL: &str = "https://aka.ms/MinecraftEULA";
const REGISTRO_MODS: &str = ".icaro-mods.json";
const AJUSTES: &str = ".icaro-alojamiento.json";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Por ahora solo se pueden alojar instancias Vanilla o Fabric.")]
    Loader,
    #[error("Falta aceptar el EULA de Minecraft para abrir el servidor.")]
    Eula,
    #[error("No se pudo leer o guardar la carpeta del servidor.")]
    Disco,
    #[error("La preparación del servidor fue cancelada.")]
    Cancelada,
    #[error("{0}")]
    Minecraft(#[from] crate::minecraft::Error),
    #[error("No se pudo iniciar Java para el servidor.")]
    Java,
}

/// Ajustes de Ícaro para el servidor que no van en `server.properties`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Ajustes {
    pub memoria_mb: u32,
}
impl Default for Ajustes {
    fn default() -> Self {
        Self { memoria_mb: 2048 }
    }
}

#[derive(Debug, Clone)]
pub struct Preparado {
    pub carpeta: PathBuf,
    /// Nombre del JAR que se ejecuta dentro de la carpeta.
    pub jar: String,
    pub puerto: u16,
    pub mods: usize,
}

pub fn loader_alojable(loader: &str) -> bool {
    matches!(loader, "vanilla" | "fabric")
}

pub fn carpeta(store: &Almacen, instancia: &str) -> Result<PathBuf, Error> {
    if instancia.is_empty()
        || !instancia
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        return Err(Error::Disco);
    }
    Ok(store
        .datos()
        .map_err(|_| Error::Disco)?
        .join("alojamiento")
        .join(instancia))
}

pub fn eula_aceptado(carpeta: &Path) -> bool {
    fs::read_to_string(carpeta.join("eula.txt")).is_ok_and(|t| {
        t.lines()
            .any(|l| l.trim().eq_ignore_ascii_case("eula=true"))
    })
}

pub fn aceptar_eula(carpeta: &Path) -> Result<(), Error> {
    fs::create_dir_all(carpeta).map_err(|_| Error::Disco)?;
    escribir_atomico(
        &carpeta.join("eula.txt"),
        format!("#Aceptado en Ícaro ({EULA_URL})\neula=true\n").as_bytes(),
    )
    .map_err(|_| Error::Disco)
}

/// `server.properties` actual; si todavía no existe, los valores iniciales.
pub fn leer_propiedades(carpeta: &Path) -> Result<Propiedades, Error> {
    Ok(Propiedades::cargar(&carpeta.join("server.properties"))
        .map_err(|_| Error::Disco)?
        .unwrap_or_else(|| Propiedades::nuevas(puerto_libre())))
}

pub fn guardar_propiedades(carpeta: &Path, p: &Propiedades) -> Result<(), Error> {
    fs::create_dir_all(carpeta).map_err(|_| Error::Disco)?;
    p.guardar(&carpeta.join("server.properties"))
        .map_err(|_| Error::Disco)
}

pub fn leer_ajustes(carpeta: &Path) -> Ajustes {
    fs::read(carpeta.join(AJUSTES))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

pub fn guardar_ajustes(carpeta: &Path, a: &Ajustes) -> Result<(), Error> {
    fs::create_dir_all(carpeta).map_err(|_| Error::Disco)?;
    let bytes = serde_json::to_vec_pretty(a).map_err(|_| Error::Disco)?;
    escribir_atomico(&carpeta.join(AJUSTES), &bytes).map_err(|_| Error::Disco)
}

/// Primer puerto libre desde 25565 en loopback; el servidor solo escucha ahí.
pub fn puerto_libre() -> u16 {
    (25565..25600)
        .find(|p| std::net::TcpListener::bind(("127.0.0.1", *p)).is_ok())
        .or_else(|| {
            std::net::TcpListener::bind("127.0.0.1:0")
                .ok()?
                .local_addr()
                .ok()
                .map(|a| a.port())
        })
        .unwrap_or(25565)
}

/// Descarga lo que falte y deja la carpeta lista para `proceso::iniciar`.
pub async fn preparar(
    store: &Almacen,
    instancia: &Instancia,
    cancel: &CancellationToken,
    etapa: impl Fn(&str),
) -> Result<Preparado, Error> {
    if !loader_alojable(&instancia.loader) {
        return Err(Error::Loader);
    }
    let dir = carpeta(store, &instancia.id)?;
    if !eula_aceptado(&dir) {
        return Err(Error::Eula);
    }
    etapa("Descargando el servidor oficial");
    let vanilla =
        crate::minecraft::servidor::jar_vanilla(store, &instancia.minecraft, cancel).await?;
    let fabric = if instancia.loader == "fabric" {
        let loader =
            crate::versiones_instancia::leer_loader(store, instancia).ok_or(Error::Loader)?;
        etapa("Descargando el lanzador de Fabric");
        Some(
            crate::minecraft::servidor::jar_fabric(store, &instancia.minecraft, &loader, cancel)
                .await?,
        )
    } else {
        None
    };
    if cancel.is_cancelled() {
        return Err(Error::Cancelada);
    }
    etapa("Copiando los mods de la instancia");
    let juego = store.juego(&instancia.id).map_err(|_| Error::Disco)?;
    let cancel_b = cancel.clone();
    let con_mods = fabric.is_some();
    tokio::task::spawn_blocking(move || {
        fs::create_dir_all(&dir).map_err(|_| Error::Disco)?;
        copiar_si_cambia(&vanilla, &dir.join("server.jar"))?;
        let jar = match &fabric {
            Some(lanzador) => {
                copiar_si_cambia(lanzador, &dir.join("fabric-server-launch.jar"))?;
                escribir_atomico(
                    &dir.join("fabric-server-launcher.properties"),
                    b"serverJar=server.jar\n",
                )
                .map_err(|_| Error::Disco)?;
                "fabric-server-launch.jar".to_owned()
            }
            None => "server.jar".to_owned(),
        };
        let mods = if con_mods {
            sincronizar_mods(&juego.join("mods"), &dir.join("mods"), &cancel_b)?
        } else {
            0
        };
        let ruta = dir.join("server.properties");
        let mut propiedades = match Propiedades::cargar(&ruta).map_err(|_| Error::Disco)? {
            Some(p) => p,
            None => Propiedades::nuevas(puerto_libre()),
        };
        // Solo se entra por el túnel: el servidor no se expone a la red local.
        propiedades.fijar("server-ip", "127.0.0.1");
        let puerto = match propiedades.numero("server-port") {
            Some(p) if (1024..=65535).contains(&p) => p as u16,
            _ => {
                let p = puerto_libre();
                propiedades.fijar("server-port", &p.to_string());
                p
            }
        };
        propiedades.guardar(&ruta).map_err(|_| Error::Disco)?;
        Ok(Preparado {
            carpeta: dir,
            jar,
            puerto,
            mods,
        })
    })
    .await
    .map_err(|_| Error::Disco)?
}

fn copiar_si_cambia(origen: &Path, destino: &Path) -> Result<(), Error> {
    let igual = match (fs::metadata(origen), fs::symlink_metadata(destino)) {
        (Ok(a), Ok(b)) if b.is_file() && a.len() == b.len() => {
            sha256(origen).ok() == sha256(destino).ok()
        }
        _ => false,
    };
    if igual {
        return Ok(());
    }
    copiar_atomico(origen, destino)
}

fn sha256(ruta: &Path) -> std::io::Result<[u8; 32]> {
    use sha2::{Digest, Sha256};
    let mut f = fs::File::open(ruta)?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().into())
}

fn copiar_atomico(origen: &Path, destino: &Path) -> Result<(), Error> {
    let padre = destino.parent().ok_or(Error::Disco)?;
    fs::create_dir_all(padre).map_err(|_| Error::Disco)?;
    let tmp = padre.join(format!(".icaro-{}.tmp", aleatorio()));
    let r = fs::copy(origen, &tmp)
        .and_then(|_| fs::OpenOptions::new().write(true).open(&tmp)?.sync_all())
        .and_then(|_| fs::rename(&tmp, destino));
    if r.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    r.map_err(|_| Error::Disco)
}

pub(crate) fn escribir_atomico(ruta: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let padre = ruta
        .parent()
        .ok_or_else(|| std::io::Error::other("sin carpeta"))?;
    fs::create_dir_all(padre)?;
    let tmp = padre.join(format!(".icaro-{}.tmp", aleatorio()));
    let r = (|| {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, ruta)
    })();
    if r.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    r
}

fn aleatorio() -> String {
    let mut b = [0u8; 8];
    let _ = getrandom::fill(&mut b);
    b.iter().map(|x| format!("{x:02x}")).collect()
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Copiados {
    /// Nombre → (tamaño, segundos de modificación) del JAR de la instancia.
    archivos: BTreeMap<String, (u64, u64)>,
}

/// Lleva los JAR activos de la instancia al servidor, sin los exclusivos de
/// cliente. Retira solo los que Ícaro copió antes y ya no están; los mods
/// añadidos a mano en la carpeta del servidor se conservan.
fn sincronizar_mods(
    origen: &Path,
    destino: &Path,
    cancel: &CancellationToken,
) -> Result<usize, Error> {
    fs::create_dir_all(destino).map_err(|_| Error::Disco)?;
    let registro = destino.join(REGISTRO_MODS);
    let anteriores: Copiados = fs::read(&registro)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default();
    let mut nuevos = Copiados::default();
    if let Ok(lectura) = fs::read_dir(origen) {
        for entrada in lectura.flatten() {
            if cancel.is_cancelled() {
                return Err(Error::Cancelada);
            }
            let nombre = entrada.file_name().to_string_lossy().into_owned();
            if !nombre.to_ascii_lowercase().ends_with(".jar") || nombre.starts_with('.') {
                continue;
            }
            let Ok(meta) = fs::symlink_metadata(entrada.path()) else {
                continue;
            };
            if !meta.is_file() || solo_cliente(&entrada.path()) {
                continue;
            }
            let firma = (
                meta.len(),
                meta.modified()
                    .ok()
                    .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |d| d.as_secs()),
            );
            let destino_mod = destino.join(&nombre);
            let nuestro = anteriores.archivos.contains_key(&nombre);
            let existe = destino_mod.exists();
            if existe && !nuestro {
                // Un JAR que el anfitrión puso a mano con el mismo nombre se respeta.
                continue;
            }
            if !(existe && anteriores.archivos.get(&nombre) == Some(&firma)) {
                copiar_atomico(&entrada.path(), &destino_mod)?;
            }
            nuevos.archivos.insert(nombre, firma);
        }
    }
    for nombre in anteriores.archivos.keys() {
        if !nuevos.archivos.contains_key(nombre) {
            let _ = fs::remove_file(destino.join(nombre));
        }
    }
    let bytes = serde_json::to_vec_pretty(&nuevos).map_err(|_| Error::Disco)?;
    escribir_atomico(&registro, &bytes).map_err(|_| Error::Disco)?;
    Ok(nuevos.archivos.len())
}

/// `fabric.mod.json` con `"environment": "client"`: el servidor no lo carga.
fn solo_cliente(jar: &Path) -> bool {
    let Ok(f) = fs::File::open(jar) else {
        return false;
    };
    let Ok(mut zip) = zip::ZipArchive::new(f) else {
        return false;
    };
    let Ok(entrada) = zip.by_name("fabric.mod.json") else {
        return false;
    };
    if entrada.size() > 1024 * 1024 {
        return false;
    }
    let mut texto = String::new();
    if entrada
        .take(1024 * 1024)
        .read_to_string(&mut texto)
        .is_err()
    {
        return false;
    }
    serde_json::from_str::<serde_json::Value>(&texto)
        .ok()
        .and_then(|v| v.get("environment")?.as_str().map(str::to_owned))
        .is_some_and(|e| e == "client")
}

/// Entradas y salidas de jugadores en la consola del servidor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Jugador {
    Entra(String),
    Sale(String),
}

pub fn jugador(linea: &str) -> Option<Jugador> {
    let (_, mensaje) = linea.rsplit_once("]: ")?;
    let mensaje = mensaje.trim();
    let nombre_valido = |n: &str| {
        !n.is_empty() && n.len() <= 16 && n.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    };
    if let Some(n) = mensaje.strip_suffix(" joined the game") {
        return nombre_valido(n).then(|| Jugador::Entra(n.into()));
    }
    if let Some(n) = mensaje.strip_suffix(" left the game") {
        return nombre_valido(n).then(|| Jugador::Sale(n.into()));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporal(nombre: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("icaro-alojamiento-{nombre}-{}", aleatorio()));
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn jar(ruta: &Path, entorno: Option<&str>) {
        let f = fs::File::create(ruta).unwrap();
        let mut z = zip::ZipWriter::new(f);
        if let Some(e) = entorno {
            z.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
                .unwrap();
            z.write_all(format!("{{\"id\":\"x\",\"environment\":\"{e}\"}}").as_bytes())
                .unwrap();
        }
        z.finish().unwrap();
    }

    #[test]
    fn copia_mods_sin_los_de_cliente_y_respeta_los_puestos_a_mano() {
        let base = temporal("mods");
        let (origen, destino) = (base.join("instancia"), base.join("servidor"));
        fs::create_dir_all(&origen).unwrap();
        fs::create_dir_all(&destino).unwrap();
        jar(&origen.join("comun.jar"), Some("*"));
        jar(&origen.join("menu.jar"), Some("client"));
        jar(&origen.join("viejo.jar"), None);
        fs::write(origen.join("apagado.jar.disabled"), b"x").unwrap();
        fs::write(destino.join("propio-del-servidor.jar"), b"mio").unwrap();
        let c = CancellationToken::new();
        assert_eq!(sincronizar_mods(&origen, &destino, &c).unwrap(), 2);
        assert!(destino.join("comun.jar").exists());
        assert!(destino.join("viejo.jar").exists());
        assert!(!destino.join("menu.jar").exists());
        assert!(!destino.join("apagado.jar.disabled").exists());
        // Quitar un mod de la instancia lo retira del servidor; el propio queda.
        fs::remove_file(origen.join("viejo.jar")).unwrap();
        assert_eq!(sincronizar_mods(&origen, &destino, &c).unwrap(), 1);
        assert!(!destino.join("viejo.jar").exists());
        assert_eq!(
            fs::read(destino.join("propio-del-servidor.jar")).unwrap(),
            b"mio"
        );
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn eula_solo_cuenta_si_dice_true() {
        let d = temporal("eula");
        assert!(!eula_aceptado(&d));
        fs::write(d.join("eula.txt"), "eula=false\n").unwrap();
        assert!(!eula_aceptado(&d));
        aceptar_eula(&d).unwrap();
        assert!(eula_aceptado(&d));
        let _ = fs::remove_dir_all(d);
    }

    #[test]
    fn reconoce_entradas_y_salidas() {
        assert_eq!(
            jugador("[12:00:01] [Server thread/INFO]: Pato_Lucas joined the game"),
            Some(Jugador::Entra("Pato_Lucas".into()))
        );
        assert_eq!(
            jugador("[12:05:00] [Server thread/INFO]: Pato_Lucas left the game"),
            Some(Jugador::Sale("Pato_Lucas".into()))
        );
        assert_eq!(
            jugador("[12:05:00] [Server thread/INFO]: <Pato> joined the game"),
            None
        );
    }

    #[test]
    fn la_carpeta_no_acepta_rutas() {
        let store = Almacen::nuevo(std::env::temp_dir().join("icaro-alojamiento-almacen"));
        assert!(carpeta(&store, "../x").is_err());
        assert!(carpeta(&store, "").is_err());
    }
}
