// SPDX-License-Identifier: GPL-3.0-only
//! Vínculos locales por clave y publicación; nunca por código, nombre o IP.
use super::{
    manifiesto::hexadecimal, revisiones::ContextoPublicacion, Error, Limites, Manifiesto, Origen,
    Perfil,
};
use crate::instancias::{Almacen, Instancia};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{fs, io::Read, path::Path};

const VINCULO: &str = ".icaro-compartir-vinculo.json";
pub(crate) const PUBLICACION: &str = ".icaro-compartir-publicacion.json";
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Vinculo {
    pub origen: Origen,
    pub perfil: Perfil,
}
fn leer_json<T: DeserializeOwned>(ruta: &Path) -> Result<Option<T>, Error> {
    match fs::symlink_metadata(ruta) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(Error::Disco),
        Ok(_) => (),
    }
    let meta = crate::almacenamiento::regular(ruta).map_err(|_| Error::Disco)?;
    if !meta.is_file() || meta.len() > 4096 {
        return Err(Error::Datos("vínculo local"));
    }
    let mut bytes = Vec::new();
    fs::File::open(ruta)
        .map_err(|_| Error::Disco)?
        .take(4097)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Disco)?;
    if bytes.len() > 4096 {
        return Err(Error::Limite("vínculo local"));
    }
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| Error::Datos("vínculo local"))
}
fn guardar(ruta: &Path, valor: &impl Serialize) -> Result<(), Error> {
    crate::almacenamiento::escribir_json(
        ruta,
        &serde_json::to_value(valor).map_err(|_| Error::Disco)?,
    )
    .map_err(|_| Error::Disco)
}
fn carpeta(ruta: &Path) -> Result<(), Error> {
    if !crate::almacenamiento::regular(ruta)
        .map_err(|_| Error::Disco)?
        .is_dir()
    {
        return Err(Error::Disco);
    }
    Ok(())
}
pub fn leer(juego: &Path) -> Result<Option<Vinculo>, Error> {
    carpeta(juego)?;
    let vinculo: Option<Vinculo> = leer_json(&juego.join(VINCULO))?;
    if let Some(v) = &vinculo {
        v.origen.validar()?;
        v.perfil.validar()?;
    }
    Ok(vinculo)
}

/// Perfil actual de la instancia, sin escribir nada.
pub(crate) fn perfil_de(store: &Almacen, instancia: &Instancia) -> Perfil {
    Perfil {
        minecraft: instancia.minecraft.clone(),
        loader: instancia.loader.clone(),
        loader_version: crate::versiones_instancia::leer_loader(store, instancia),
    }
}
/// Origen ya persistido de una publicación propia; nunca lo crea.
pub(crate) fn origen_guardado(juego: &Path) -> Result<Option<Origen>, Error> {
    carpeta(juego)?;
    let origen: Option<Origen> = leer_json(&juego.join(PUBLICACION))?;
    if let Some(o) = &origen {
        o.validar()?;
    }
    Ok(origen)
}
pub(crate) fn contexto(
    raiz: &Path,
    juego: std::path::PathBuf,
    origen: Origen,
    perfil: Perfil,
) -> ContextoPublicacion {
    ContextoPublicacion {
        juego,
        almacen: raiz
            .join("compartir/publicaciones")
            .join(&origen.publicacion),
        origen,
        perfil,
        limites: Limites::default(),
    }
}

/// Ejecutar fuera del hilo de iced. Reabrir conserva el origen de esta instancia.
pub fn publicacion(
    raiz: &Path,
    store: &Almacen,
    instancia: &Instancia,
    anfitrion: &str,
) -> Result<ContextoPublicacion, Error> {
    if !hexadecimal(anfitrion, 64) {
        return Err(Error::Datos("identidad del anfitrión"));
    }
    let perfil = perfil_de(store, instancia);
    perfil.validar()?;
    let juego = store.juego(&instancia.id).map_err(|_| Error::Disco)?;
    carpeta(&juego)?;
    let _bloqueo = crate::mods::guardia(&juego).map_err(|_| Error::Datos("instancia ocupada"))?;
    let path = juego.join(PUBLICACION);
    let origen = match origen_guardado(&juego)? {
        Some(origen) => {
            if origen.anfitrion != anfitrion {
                return Err(Error::Datos("la publicación pertenece a otra identidad"));
            }
            origen
        }
        None => {
            let mut nonce = [0u8; 16];
            getrandom::fill(&mut nonce).map_err(|_| Error::Credencial)?;
            let origen = Origen {
                anfitrion: anfitrion.into(),
                publicacion: nonce.iter().map(|b| format!("{b:02x}")).collect(),
            };
            guardar(&path, &origen)?;
            origen
        }
    };
    Ok(contexto(raiz, juego, origen, perfil))
}

/// Siempre crea una instancia nueva. El vínculo no acredita una revisión aplicada:
/// esa base solo la escribe la transacción tras verificar todos sus archivos.
pub fn crear_destino(
    store: &Almacen,
    nombre: &str,
    manifiesto: &Manifiesto,
    limites: &Limites,
) -> Result<Instancia, Error> {
    manifiesto.validar(limites)?;
    let p = &manifiesto.perfil;
    let instancia = store
        .crear_con_loader(nombre, &p.minecraft, &p.loader, 0)
        .map_err(|_| Error::Datos("no se pudo crear una instancia nueva con ese nombre"))?;
    preparar_destino(store, &instancia, manifiesto, limites)?;
    Ok(instancia)
}

/// Completa metadatos del destino nuevo retenido por el llamante tras un fallo.
/// Nunca adopta un juego con contenido personal y sin vínculo.
pub fn preparar_destino(
    store: &Almacen,
    instancia: &Instancia,
    manifiesto: &Manifiesto,
    limites: &Limites,
) -> Result<(), Error> {
    manifiesto.validar(limites)?;
    let p = &manifiesto.perfil;
    if instancia.minecraft != p.minecraft || instancia.loader != p.loader {
        return Err(Error::Datos("perfil distinto del destino"));
    }
    let juego = store.juego(&instancia.id).map_err(|_| Error::Disco)?;
    match leer(&juego)? {
        Some(v) if v.origen != manifiesto.origen || v.perfil != *p => {
            return Err(Error::Datos("destino de otra publicación"))
        }
        Some(_) => (),
        None => {
            let registrado = super::intentos::registrado_para(store, instancia, manifiesto)?;
            for entrada in fs::read_dir(&juego).map_err(|_| Error::Disco)? {
                let entrada = entrada.map_err(|_| Error::Disco)?;
                let nombre = entrada.file_name();
                if !registrado
                    || !super::intentos::archivo_de_preparacion(&nombre)
                    || !crate::almacenamiento::regular(&entrada.path())
                        .map_err(|_| Error::Disco)?
                        .is_file()
                {
                    return Err(Error::Datos(
                        "el destino sin vínculo contiene archivos personales",
                    ));
                }
            }
        }
    }
    guardar(
        &juego.join(VINCULO),
        &Vinculo {
            origen: manifiesto.origen.clone(),
            perfil: p.clone(),
        },
    )?;
    if let Some(version) = &p.loader_version {
        crate::versiones_instancia::guardar_loader(store, instancia, version)
            .map_err(|_| Error::Disco)?;
    }
    Ok(())
}

/// Impide lanzar un destino compartido cuyo contenido aún no se aplicó.
/// No sustituye la consulta de nuevas revisiones antes de Jugar.
pub fn comprobar_lista(store: &Almacen, instancia: &Instancia) -> Result<(), Error> {
    let juego = store.juego(&instancia.id).map_err(|_| Error::Disco)?;
    if let Some(vinculo) = leer(&juego)? {
        let base = super::aplicacion::cargar_base(&super::aplicacion::ContextoDestino {
            juego,
            limites: Limites::default(),
        })?;
        if base
            .manifiesto
            .as_ref()
            .is_none_or(|m| m.origen != vinculo.origen || m.perfil != vinculo.perfil)
        {
            return Err(Error::Datos("instancia compartida incompleta; termina la instalación desde la invitación antes de jugar"));
        }
    }
    Ok(())
}
