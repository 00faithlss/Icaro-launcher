// SPDX-License-Identifier: GPL-3.0-only
pub(crate) mod estructura;
use super::{Error, BASE, PREFIJO};
use crate::compartir::{manifiesto::hexadecimal, RutaCompartida};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) fn validar_destino(ruta: &str) -> Result<(), Error> {
    if matches!(ruta, BASE | "options.txt") {
        Ok(())
    } else {
        RutaCompartida::parse(ruta).map(|_| ())
    }
}
pub(super) fn directorio(path: &Path) -> Result<(), Error> {
    let meta =
        crate::almacenamiento::regular(path).map_err(|_| Error::Datos("carpeta no regular"))?;
    if !meta.is_dir() {
        return Err(Error::Datos("destino no es carpeta"));
    }
    Ok(())
}
pub(super) fn crear_directorio(path: &Path) -> Result<(), Error> {
    match fs::create_dir(path) {
        Ok(()) => (),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => (),
        Err(_) => return Err(Error::Disco),
    }
    directorio(path)
}
/// Valida cada padre sin seguir enlaces, incluidos los de opciones y metadatos.
pub(super) fn resolver(juego: &Path, ruta: &str, crear: bool) -> Result<PathBuf, Error> {
    validar_destino(ruta)?;
    directorio(juego)?;
    let mut resultado = juego.to_path_buf();
    let partes: Vec<_> = ruta.split('/').collect();
    for (i, parte) in partes.iter().enumerate() {
        resultado.push(parte);
        if i + 1 < partes.len() {
            match fs::symlink_metadata(&resultado) {
                Ok(_) => directorio(&resultado)?,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    if crear {
                        crear_directorio(&resultado)?;
                    }
                }
                Err(_) => return Err(Error::Disco),
            }
        } else if let Ok(_) = fs::symlink_metadata(&resultado) {
            let meta = crate::almacenamiento::regular(&resultado)
                .map_err(|_| Error::Datos("destino no regular"))?;
            if !meta.is_file() {
                return Err(Error::Datos("destino no es archivo"));
            }
        }
    }
    Ok(resultado)
}
pub(super) fn transaccion(juego: &Path, nombre: &str) -> Result<PathBuf, Error> {
    let id = nombre
        .strip_prefix(PREFIJO)
        .ok_or(Error::Datos("nombre de transacción"))?;
    if !hexadecimal(id, 32) {
        return Err(Error::Datos("identificador de transacción"));
    }
    directorio(juego)?;
    let path = juego.join(nombre);
    directorio(&path)?;
    Ok(path)
}
pub(super) fn slot(tx: &Path, grupo: &str, indice: usize) -> Result<PathBuf, Error> {
    if !matches!(grupo, "nuevos" | "anteriores") {
        return Err(Error::Datos("grupo de transacción"));
    }
    directorio(tx)?;
    let parent = tx.join(grupo);
    directorio(&parent)?;
    Ok(parent.join(indice.to_string()))
}
