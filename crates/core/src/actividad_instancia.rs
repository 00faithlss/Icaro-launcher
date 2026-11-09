// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Guardia de actividad de una instancia.
//!
//! Marca que una instancia está ocupada (preparando Minecraft, ejecutando
//! Java o revisando/aplicando una sincronización) con un bloqueo exclusivo
//! del sistema operativo fuera de la carpeta de la instancia, por ID. El bloqueo
//! pertenece al descriptor abierto: se libera al soltar la [`Guardia`] o
//! cuando el proceso termina o muere, sin depender de PID ni de archivos
//! de estado que haya que limpiar.
//!
//! Es independiente de la guardia transaccional de `mods`: quien necesite
//! mantener la actividad durante varias fases debe compartir la misma
//! guardia con `Arc<Guardia>` en lugar de adquirirla otra vez (una segunda
//! adquisición en el mismo proceso también devuelve [`Error::Ocupada`]).
//!
//! Las operaciones son bloqueantes pero no esperan al bloqueo: desde código
//! asíncrono, llamarlas dentro de `spawn_blocking`.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::io;
use std::path::{Path, PathBuf};

/// Nombre del archivo de bloqueo dentro de la carpeta de reserva de actividad.
pub const ARCHIVO: &str = ".icaro-actividad.lock";

/// Errores al adquirir la guardia de actividad.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Otra guardia (de este u otro proceso) mantiene la instancia ocupada.
    #[error("la instancia ya está en uso")]
    Ocupada,
    /// La carpeta del juego o el archivo de bloqueo no son una carpeta o un
    /// archivo regular (enlace simbólico, unión, directorio, dispositivo…).
    #[error("ruta no admitida para la guardia de actividad: {}", ruta.display())]
    NoRegular { ruta: PathBuf },
    /// Error de entrada/salida al inspeccionar, crear, abrir o bloquear.
    #[error("no se pudo usar {}: {fuente}", ruta.display())]
    Disco {
        ruta: PathBuf,
        #[source]
        fuente: io::Error,
    },
}

/// Bloqueo exclusivo de actividad de una instancia (RAII).
///
/// No implementa `Clone`: para compartirla entre fases o hilos, envolverla
/// en `Arc<Guardia>`; el bloqueo dura hasta que se suelta el último `Arc`.
#[derive(Debug)]
pub struct Guardia {
    ruta: PathBuf,
    // Mantener el descriptor abierto mantiene el bloqueo; cerrarlo lo libera.
    _archivo: File,
}

impl Guardia {
    /// Reserva estable aunque `Almacen::guardar` renombre la instancia.
    pub fn adquirir_instancia(store: &crate::instancias::Almacen, id: &str) -> Result<Self, Error> {
        let ruta = Self::ruta_instancia(store, id)?;
        Self::adquirir(ruta.parent().expect("directorio de actividad"))
    }

    /// Ruta estable por raíz del almacén e ID; nunca depende del nombre visible.
    pub fn ruta_instancia(store: &crate::instancias::Almacen, id: &str) -> Result<PathBuf, Error> {
        // También comprueba que el ID corresponde a una instancia existente.
        let juego = store
            .juego(id)
            .map_err(|e| disco(Path::new(id), io::Error::other(e)))?;
        let almacen = juego
            .parent()
            .and_then(Path::parent)
            .ok_or_else(|| no_regular(&juego))?
            .canonicalize()
            .map_err(|e| disco(&juego, e))?;
        let raiz = almacen.join(".icaro-actividades");
        carpeta(&raiz)?;
        // La raíz canónica del almacén separa IDs iguales de almacenes distintos.
        let destino = raiz.join(id);
        carpeta(&destino)?;
        Ok(destino.join(ARCHIVO))
    }

    /// Intenta adquirir el bloqueo de `juego/.icaro-actividad.lock` sin esperar.
    ///
    /// `juego` debe existir y ser una carpeta real (no un enlace ni una unión).
    /// El archivo de bloqueo se crea si falta; si existe debe ser un archivo
    /// regular, y nunca se trunca, escribe ni borra.
    pub fn adquirir(juego: &Path) -> Result<Guardia, Error> {
        let meta = fs::symlink_metadata(juego).map_err(|fuente| disco(juego, fuente))?;
        if !meta.is_dir() || es_enlace(&meta) {
            return Err(no_regular(juego));
        }
        let ruta = juego.join(ARCHIVO);
        let archivo = abrir(&ruta)?;
        match archivo.try_lock() {
            Ok(()) => Ok(Guardia {
                ruta,
                _archivo: archivo,
            }),
            Err(TryLockError::WouldBlock) => Err(Error::Ocupada),
            Err(TryLockError::Error(fuente)) => Err(disco(&ruta, fuente)),
        }
    }

    /// Ruta del archivo de bloqueo.
    pub fn ruta(&self) -> &Path {
        &self.ruta
    }
}

fn carpeta(ruta: &Path) -> Result<(), Error> {
    match fs::create_dir(ruta) {
        Ok(()) => (),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(disco(ruta, e)),
    }
    let meta = fs::symlink_metadata(ruta).map_err(|e| disco(ruta, e))?;
    if !meta.is_dir() || es_enlace(&meta) {
        return Err(no_regular(ruta));
    }
    Ok(())
}

fn abrir(ruta: &Path) -> Result<File, Error> {
    // Dos intentos cubren la carrera en la que otro proceso crea el archivo
    // entre la inspección y `create_new`.
    for _ in 0..2 {
        match fs::symlink_metadata(ruta) {
            Ok(meta) => {
                if !meta.is_file() || es_enlace(&meta) {
                    return Err(no_regular(ruta));
                }
                let archivo = opciones_existente()
                    .open(ruta)
                    .map_err(|fuente| disco(ruta, fuente))?;
                verificar_abierto(ruta, &archivo)?;
                return Ok(archivo);
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                let mut opciones = OpenOptions::new();
                opciones.read(true).write(true).create_new(true);
                match opciones.open(ruta) {
                    Ok(archivo) => {
                        verificar_abierto(ruta, &archivo)?;
                        return Ok(archivo);
                    }
                    Err(e) if e.kind() == io::ErrorKind::AlreadyExists => continue,
                    Err(fuente) => return Err(disco(ruta, fuente)),
                }
            }
            Err(fuente) => return Err(disco(ruta, fuente)),
        }
    }
    Err(disco(
        ruta,
        io::Error::other("el archivo de bloqueo cambió mientras se abría"),
    ))
}

/// Apertura sin truncar ni crear. En Windows no sigue puntos de reanálisis,
/// de modo que un enlace colocado en una carrera se detecta al verificar.
fn opciones_existente() -> OpenOptions {
    let mut opciones = OpenOptions::new();
    opciones.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        opciones.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    opciones
}

/// Comprueba que el descriptor abierto es un archivo regular y, en Unix, que
/// sigue siendo el mismo objeto que hay en la ruta (sin enlaces intermedios).
fn verificar_abierto(ruta: &Path, archivo: &File) -> Result<(), Error> {
    let abierto = archivo.metadata().map_err(|fuente| disco(ruta, fuente))?;
    if !abierto.is_file() || es_enlace(&abierto) {
        return Err(no_regular(ruta));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let actual = fs::symlink_metadata(ruta).map_err(|fuente| disco(ruta, fuente))?;
        if !actual.is_file() || actual.dev() != abierto.dev() || actual.ino() != abierto.ino() {
            return Err(no_regular(ruta));
        }
    }
    Ok(())
}

fn es_enlace(meta: &fs::Metadata) -> bool {
    if meta.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        if meta.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
            return true;
        }
    }
    false
}

fn disco(ruta: &Path, fuente: io::Error) -> Error {
    Error::Disco {
        ruta: ruta.to_path_buf(),
        fuente,
    }
}

fn no_regular(ruta: &Path) -> Error {
    Error::NoRegular {
        ruta: ruta.to_path_buf(),
    }
}
