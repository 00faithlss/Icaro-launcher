// SPDX-License-Identifier: GPL-3.0-only
//! Intentos de instalación compartida que deben sobrevivir a un cierre o reinicio.
//!
//! Cada destino nuevo guarda `minecraft/.icaro-compartir-instalacion.json` con su propio
//! ID, el origen y el perfil de la publicación y la última revisión revisada. No contiene
//! códigos, direcciones, claves privadas, cuentas ni datos personales.
//!
//! Orden previsto: `Almacen::crear_con_loader` → [`registrar`] → vínculo/perfil →
//! descarga y `aplicacion::aplicar` → [`completar`]. Si el proceso se corta entre crear la
//! instancia y [`registrar`], la instancia queda huérfana sin marcador: nunca se adopta
//! (ni por nombre ni por contenido) y el usuario debe conservarla o quitarla a mano.
//!
//! Son funciones síncronas de servicio: invocarlas dentro de `spawn_blocking`.
use super::{
    aplicacion::{cargar_base, ContextoDestino},
    manifiesto::hexadecimal,
    vinculos, Error, Limites, Manifiesto, Origen, Perfil,
};
use crate::instancias::{Almacen, Instancia};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

pub(crate) const MARCADOR: &str = ".icaro-compartir-instalacion.json";
/// Solo para un destino con marcador propio validado: no elimina estos archivos.
pub(crate) fn archivo_de_preparacion(nombre: &std::ffi::OsStr) -> bool {
    if nombre == MARCADOR || nombre == ".icaro-mods.lock" {
        return true;
    }
    nombre
        .to_str()
        .and_then(|n| n.strip_prefix(".icaro-state-"))
        .and_then(|n| n.strip_suffix(".tmp"))
        .is_some_and(|n| {
            !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) && n.parse::<u64>().is_ok()
        })
}
const ESQUEMA: u32 = 1;
const MAX_MARCADOR: u64 = 16 * 1024;
const MAX_INSTANCIAS: usize = 1000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Marcador {
    esquema: u32,
    instancia_id: String,
    origen: Origen,
    perfil: Perfil,
    /// Última revisión revisada para este destino; no acredita que se haya aplicado.
    revision: u64,
}
impl Marcador {
    fn validar(&self) -> Result<(), Error> {
        if self.esquema != ESQUEMA {
            return Err(Error::Datos("versión del registro de instalación"));
        }
        if !hexadecimal(&self.instancia_id, 32) || self.revision == 0 {
            return Err(Error::Datos("registro de instalación"));
        }
        self.origen.validar()?;
        self.perfil.validar()
    }
    fn misma_publicacion(&self, m: &Manifiesto) -> bool {
        self.origen == m.origen && self.perfil == m.perfil
    }
}

/// Registra (o actualiza a una revisión nueva) el intento de instalar `manifiesto` en
/// el destino recién creado. Ejecutar antes de guardar el vínculo e instalar el perfil.
/// Nunca pisa el marcador de otra instancia o publicación ni marca un juego personal.
pub fn registrar(
    store: &Almacen,
    instancia: &Instancia,
    manifiesto: &Manifiesto,
) -> Result<(), Error> {
    manifiesto.validar(&Limites::default())?;
    let juego = destino(store, instancia, manifiesto)?;
    let _guardia = crate::mods::guardia(&juego).map_err(|_| Error::Datos("instancia ocupada"))?;
    let vinculo = vinculos::leer(&juego)?;
    if vinculo
        .as_ref()
        .is_some_and(|v| v.origen != manifiesto.origen || v.perfil != manifiesto.perfil)
    {
        return Err(Error::Datos("destino de otra publicación"));
    }
    match leer(&juego)? {
        Some(actual) => {
            propio(&actual, instancia, manifiesto)?;
            if manifiesto.revision < actual.revision {
                return Err(Error::Datos("revisión anterior a la ya revisada"));
            }
            if manifiesto.revision == actual.revision {
                return Ok(());
            }
        }
        None if vinculo.is_none() => {
            for entrada in fs::read_dir(&juego).map_err(|_| Error::Disco)? {
                let entrada = entrada.map_err(|_| Error::Disco)?;
                if entrada.file_name() != ".icaro-mods.lock"
                    || !crate::almacenamiento::regular(&entrada.path())
                        .map_err(|_| Error::Disco)?
                        .is_file()
                {
                    return Err(Error::Datos(
                        "el destino sin registro contiene archivos personales",
                    ));
                }
            }
        }
        None => (),
    }
    let marcador = Marcador {
        esquema: ESQUEMA,
        instancia_id: instancia.id.clone(),
        origen: manifiesto.origen.clone(),
        perfil: manifiesto.perfil.clone(),
        revision: manifiesto.revision,
    };
    crate::almacenamiento::escribir_json(
        &juego.join(MARCADOR),
        &serde_json::to_value(&marcador).map_err(|_| Error::Disco)?,
    )
    .map_err(|_| Error::Disco)
}

/// Busca el único destino incompleto de esta publicación (mismo origen y perfil; la
/// revisión puede diferir para volver a revisarla). Solo considera instancias con un
/// marcador propio y válido; un marcador dañado, copiado o enlazado es un error, nunca
/// motivo para crear otra copia. Con varios candidatos pide al usuario que elija.
///
/// Usa `Almacen::cargar`, que puede normalizar `instance.json` como en el arranque.
pub fn buscar(store: &Almacen, manifiesto: &Manifiesto) -> Result<Option<Instancia>, Error> {
    manifiesto.validar(&Limites::default())?;
    let informe = store.cargar().map_err(|_| Error::Disco)?;
    if informe.instancias.len().saturating_add(informe.omitidas) > MAX_INSTANCIAS {
        return Err(Error::Limite("instancias a revisar"));
    }
    let mut hallada: Option<Instancia> = None;
    for instancia in informe.instancias {
        let juego = juego(store, &instancia)?;
        let Some(marcador) = leer(&juego)? else {
            continue;
        };
        if marcador.instancia_id != instancia.id {
            return Err(Error::Datos("registro de instalación de otra instancia"));
        }
        if !marcador.misma_publicacion(manifiesto) {
            continue;
        }
        if instancia.minecraft != manifiesto.perfil.minecraft
            || instancia.loader != manifiesto.perfil.loader
        {
            return Err(Error::Datos("perfil distinto del destino"));
        }
        if vinculos::leer(&juego)?
            .is_some_and(|v| v.origen != marcador.origen || v.perfil != marcador.perfil)
        {
            return Err(Error::Datos("destino de otra publicación"));
        }
        match base(&juego)? {
            // Cierre entre aplicar la base y completar: la instancia ya está completa.
            Some(b) if marcador.misma_publicacion(&b) => {
                if b.revision >= marcador.revision {
                    continue;
                }
            }
            Some(_) => return Err(Error::Datos("base de otra publicación")),
            None => (),
        }
        if hallada.is_some() {
            return Err(Error::Datos(
                "hay varias instalaciones incompletas de esta publicación; elige cuál continuar",
            ));
        }
        hallada = Some(instancia);
    }
    Ok(hallada)
}

/// Retira el marcador cuando la base aplicada coincide exactamente con `manifiesto` y el
/// vínculo con su origen y perfil. Solo borra el marcador propio; no toca otro archivo.
/// Sin marcador no hace nada.
pub fn completar(
    store: &Almacen,
    instancia: &Instancia,
    manifiesto: &Manifiesto,
) -> Result<(), Error> {
    manifiesto.validar(&Limites::default())?;
    let juego = destino(store, instancia, manifiesto)?;
    let Some(marcador) = leer(&juego)? else {
        return Ok(());
    };
    propio(&marcador, instancia, manifiesto)?;
    if marcador.revision > manifiesto.revision {
        return Err(Error::Datos("revisión anterior a la ya revisada"));
    }
    let _bloqueo = crate::mods::guardia(&juego).map_err(|_| Error::Datos("instancia ocupada"))?;
    if leer(&juego)?.as_ref() != Some(&marcador) {
        return Err(Error::CambioExterno);
    }
    if vinculos::leer(&juego)?
        .is_none_or(|v| v.origen != manifiesto.origen || v.perfil != manifiesto.perfil)
    {
        return Err(Error::Datos("vínculo ausente o de otra publicación"));
    }
    if base(&juego)?.as_ref() != Some(manifiesto) {
        return Err(Error::Datos("la instalación aún no terminó"));
    }
    let ruta = juego.join(MARCADOR);
    if !crate::almacenamiento::regular(&ruta)
        .map_err(|_| Error::Datos("registro de instalación"))?
        .is_file()
    {
        return Err(Error::Datos("registro de instalación"));
    }
    fs::remove_file(ruta).map_err(|_| Error::Disco)
}

fn propio(marcador: &Marcador, instancia: &Instancia, m: &Manifiesto) -> Result<(), Error> {
    if marcador.instancia_id != instancia.id || !marcador.misma_publicacion(m) {
        return Err(Error::Datos("registro de instalación de otra publicación"));
    }
    Ok(())
}
pub(crate) fn registrado_para(
    store: &Almacen,
    instancia: &Instancia,
    m: &Manifiesto,
) -> Result<bool, Error> {
    let juego = destino(store, instancia, m)?;
    match leer(&juego)? {
        Some(marcador) => {
            propio(&marcador, instancia, m)?;
            Ok(true)
        }
        None => Ok(false),
    }
}

/// Carpeta de juego de una instancia cuyos metadatos en disco coinciden con el perfil.
fn destino(store: &Almacen, instancia: &Instancia, m: &Manifiesto) -> Result<PathBuf, Error> {
    if !hexadecimal(&instancia.id, 32) {
        return Err(Error::Datos("identificador de instancia"));
    }
    let guardada = store.leer(&instancia.id).map_err(|_| Error::Disco)?;
    if guardada.minecraft != m.perfil.minecraft
        || guardada.loader != m.perfil.loader
        || instancia.minecraft != m.perfil.minecraft
        || instancia.loader != m.perfil.loader
    {
        return Err(Error::Datos("perfil distinto del destino"));
    }
    juego(store, instancia)
}

fn juego(store: &Almacen, instancia: &Instancia) -> Result<PathBuf, Error> {
    let juego = store.juego(&instancia.id).map_err(|_| Error::Disco)?;
    if !crate::almacenamiento::regular(&juego)
        .map_err(|_| Error::Disco)?
        .is_dir()
    {
        return Err(Error::Disco);
    }
    Ok(juego)
}

fn base(juego: &Path) -> Result<Option<Manifiesto>, Error> {
    cargar_base(&ContextoDestino {
        juego: juego.to_owned(),
        limites: Limites::default(),
    })
    .map(|b| b.manifiesto)
}

/// Ausente es `None`; cualquier otro estado no regular o inválido es un error.
fn leer(juego: &Path) -> Result<Option<Marcador>, Error> {
    let ruta = juego.join(MARCADOR);
    match fs::symlink_metadata(&ruta) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(Error::Disco),
        Ok(_) => (),
    }
    let meta = crate::almacenamiento::regular(&ruta)
        .map_err(|_| Error::Datos("registro de instalación"))?;
    if !meta.is_file() {
        return Err(Error::Datos("registro de instalación"));
    }
    if meta.len() > MAX_MARCADOR {
        return Err(Error::Limite("registro de instalación"));
    }
    let mut bytes = Vec::new();
    fs::File::open(&ruta)
        .map_err(|_| Error::Disco)?
        .take(MAX_MARCADOR + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Disco)?;
    if bytes.len() as u64 > MAX_MARCADOR {
        return Err(Error::Limite("registro de instalación"));
    }
    let marcador: Marcador =
        serde_json::from_slice(&bytes).map_err(|_| Error::Datos("registro de instalación"))?;
    marcador.validar()?;
    Ok(Some(marcador))
}
