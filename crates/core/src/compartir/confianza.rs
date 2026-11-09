// SPDX-License-Identifier: GPL-3.0-only
//! Confianza persistente del invitado en la clave del anfitrión, por publicación.
//!
//! Solo guarda el ID de publicación, la clave pública completa y la fecha Unix de
//! confirmación en `<raíz>/compartir/confianza.json`. Nunca guarda secretos, no busca
//! cuentas ni usa APPDATA o el keyring: la raíz la indica siempre el llamante.
//! Ejecutar fuera del hilo de iced: lee y escribe en disco.
use super::{identidad::publica_validada, manifiesto::hexadecimal, Error, Origen};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs,
    io::{ErrorKind, Read},
    path::{Path, PathBuf},
    sync::Mutex,
};

const CARPETA: &str = "compartir";
const ARCHIVO: &str = "confianza.json";
const ESQUEMA: u32 = 1;
const MAX_BYTES: u64 = 256 * 1024;
const MAX_REGISTROS: usize = 1000;

/// Serializa las confirmaciones de este proceso: leer-modificar-escribir sin perder
/// registros de otra publicación confirmada a la vez.
static ESCRITURA: Mutex<()> = Mutex::new(());

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoConfianza {
    /// No hay registro para esta publicación: mostrar huella y sello y pedir confirmación.
    PrimeraVez,
    /// La publicación ya se confirmó con esta misma clave completa.
    Conocido { confirmado_en: u64 },
    /// La publicación se confirmó con otra clave: posible suplantación o reinstalación.
    Distinto { confirmado_en: u64 },
}
impl EstadoConfianza {
    pub fn fecha(&self) -> Option<String> {
        let segundos = match self {
            Self::PrimeraVez => return None,
            Self::Conocido { confirmado_en } | Self::Distinto { confirmado_en } => *confirmado_en,
        };
        chrono::DateTime::from_timestamp(segundos.try_into().ok()?, 0)
            .map(|fecha| fecha.format("%d-%m-%Y %H:%M UTC").to_string())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Archivo {
    esquema: u32,
    registros: Vec<Registro>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Registro {
    publicacion: String,
    anfitrion: String,
    confirmado_en: u64,
}

/// Consulta sin escribir. Un archivo ilegible, corrupto o fuera de límites es un
/// error, nunca `PrimeraVez`: así no se puede forzar una nueva confianza borrando datos.
pub fn consultar(raiz: &Path, origen: &Origen) -> Result<EstadoConfianza, Error> {
    validar_origen(origen)?;
    let registros = leer(&ruta(raiz)?)?;
    Ok(estado(&registros, origen))
}

/// Confirma la clave del anfitrión para la publicación de `origen`.
///
/// - Primera vez: registra la clave con la fecha actual.
/// - Conocido: no escribe ni cambia la fecha.
/// - Distinto: exige `aceptar_cambio`; sin él devuelve error sin modificar el archivo.
///
/// Los registros de otras publicaciones se conservan intactos.
pub fn confirmar(raiz: &Path, origen: &Origen, aceptar_cambio: bool) -> Result<(), Error> {
    validar_origen(origen)?;
    let _guardia = ESCRITURA.lock().unwrap_or_else(|e| e.into_inner());
    let ruta = ruta(raiz)?;
    let carpeta = ruta.parent().ok_or(Error::Disco)?;
    match fs::create_dir(carpeta) {
        Ok(()) => (),
        Err(e) if e.kind() == ErrorKind::AlreadyExists => {
            if !crate::almacenamiento::regular(carpeta)
                .map_err(|_| Error::Disco)?
                .is_dir()
            {
                return Err(Error::Disco);
            }
        }
        Err(_) => return Err(Error::Disco),
    }
    let bloqueo = carpeta.join("confianza.lock");
    if fs::symlink_metadata(&bloqueo).is_ok() {
        if !crate::almacenamiento::regular(&bloqueo)
            .map_err(|_| Error::Disco)?
            .is_file()
        {
            return Err(Error::Disco);
        }
    }
    let guardia = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(bloqueo)
        .map_err(|_| Error::Disco)?;
    guardia
        .try_lock()
        .map_err(|_| Error::Datos("otro proceso está confirmando la confianza"))?;
    let mut registros = leer(&ruta)?;
    match registros
        .iter()
        .position(|r| r.publicacion == origen.publicacion)
    {
        Some(i) if registros[i].anfitrion == origen.anfitrion => return Ok(()),
        Some(_) if !aceptar_cambio => {
            return Err(Error::Datos(
                "la identidad del anfitrión cambió y requiere una confirmación explícita",
            ))
        }
        Some(i) => {
            let registro = &mut registros[i];
            registro.anfitrion = origen.anfitrion.clone();
            registro.confirmado_en = ahora()?;
        }
        None => {
            if registros.len() >= MAX_REGISTROS {
                return Err(Error::Limite("registros de confianza"));
            }
            registros.push(Registro {
                publicacion: origen.publicacion.clone(),
                anfitrion: origen.anfitrion.clone(),
                confirmado_en: ahora()?,
            });
        }
    }
    guardar(&ruta, registros)
}

fn estado(registros: &[Registro], origen: &Origen) -> EstadoConfianza {
    match registros
        .iter()
        .find(|r| r.publicacion == origen.publicacion)
    {
        None => EstadoConfianza::PrimeraVez,
        Some(r) if r.anfitrion == origen.anfitrion => EstadoConfianza::Conocido {
            confirmado_en: r.confirmado_en,
        },
        Some(r) => EstadoConfianza::Distinto {
            confirmado_en: r.confirmado_en,
        },
    }
}

fn validar_origen(origen: &Origen) -> Result<(), Error> {
    origen.validar()?;
    publica_validada(&origen.anfitrion)?;
    Ok(())
}

fn ahora() -> Result<u64, Error> {
    Ok(std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| Error::Datos("reloj del sistema"))?
        .as_secs()
        .max(1))
}

/// La raíz debe existir y ser una carpeta real; no se crea implícitamente.
fn ruta(raiz: &Path) -> Result<PathBuf, Error> {
    if !crate::almacenamiento::regular(raiz)
        .map_err(|_| Error::Disco)?
        .is_dir()
    {
        return Err(Error::Disco);
    }
    let carpeta = raiz.join(CARPETA);
    match fs::symlink_metadata(&carpeta) {
        Err(e) if e.kind() == ErrorKind::NotFound => {}
        Err(_) => return Err(Error::Disco),
        Ok(_) => {
            if !crate::almacenamiento::regular(&carpeta)
                .map_err(|_| Error::Disco)?
                .is_dir()
            {
                return Err(Error::Disco);
            }
        }
    }
    Ok(carpeta.join(ARCHIVO))
}

fn leer(ruta: &Path) -> Result<Vec<Registro>, Error> {
    match fs::symlink_metadata(ruta) {
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => return Err(Error::Disco),
        Ok(_) => (),
    }
    let meta = crate::almacenamiento::regular(ruta).map_err(|_| Error::Disco)?;
    if !meta.is_file() {
        return Err(Error::Datos("archivo de confianza"));
    }
    if meta.len() > MAX_BYTES {
        return Err(Error::Limite("archivo de confianza"));
    }
    let mut bytes = Vec::new();
    fs::File::open(ruta)
        .map_err(|_| Error::Disco)?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Disco)?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err(Error::Limite("archivo de confianza"));
    }
    let archivo: Archivo =
        serde_json::from_slice(&bytes).map_err(|_| Error::Datos("archivo de confianza"))?;
    if archivo.esquema != ESQUEMA {
        return Err(Error::Datos("versión del archivo de confianza"));
    }
    if archivo.registros.len() > MAX_REGISTROS {
        return Err(Error::Limite("registros de confianza"));
    }
    let mut vistos = BTreeSet::new();
    for registro in &archivo.registros {
        if !hexadecimal(&registro.publicacion, 32) || registro.confirmado_en == 0 {
            return Err(Error::Datos("registro de confianza"));
        }
        publica_validada(&registro.anfitrion).map_err(|_| Error::Datos("registro de confianza"))?;
        if !vistos.insert(registro.publicacion.as_str()) {
            return Err(Error::Datos("registros de confianza duplicados"));
        }
    }
    drop(vistos);
    Ok(archivo.registros)
}

/// Escritura atómica con el almacenamiento existente (temporal + `rename`).
fn guardar(ruta: &Path, registros: Vec<Registro>) -> Result<(), Error> {
    let valor = serde_json::to_value(Archivo {
        esquema: ESQUEMA,
        registros,
    })
    .map_err(|_| Error::Disco)?;
    let tamano = serde_json::to_vec_pretty(&valor)
        .map_err(|_| Error::Disco)?
        .len() as u64;
    if tamano > MAX_BYTES {
        return Err(Error::Limite("archivo de confianza"));
    }
    crate::almacenamiento::escribir_json(ruta, &valor).map_err(|_| Error::Disco)
}
