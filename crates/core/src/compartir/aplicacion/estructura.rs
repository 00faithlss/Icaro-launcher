// SPDX-License-Identifier: GPL-3.0-only
//! Cambios de tipo: respaldar archivos, cambiar carpetas vacías, publicar archivos.
use super::{diario::Diario, estado, rutas, Error, EventoAplicacion};
use std::{fs, path::Path};
use tokio_util::sync::CancellationToken;

pub(super) fn ejecutar(
    juego: &Path,
    tx: &Path,
    diario: &Diario,
    progreso: &impl Fn(EventoAplicacion),
    total: usize,
) -> Result<(), Error> {
    let cancel = CancellationToken::new();
    let mut hechos = 0;
    for (i, op) in diario
        .operaciones
        .iter()
        .enumerate()
        .filter(|(_, op)| op.antes.is_some())
    {
        let destino = rutas::resolver(juego, &op.destino, false)?;
        if estado::huella(&destino, &cancel)? != op.antes {
            return Err(Error::CambioExterno);
        }
        fs::rename(destino, rutas::slot(tx, "anteriores", i)?).map_err(|_| Error::Disco)?;
        hechos += 1;
        progreso(EventoAplicacion::Movimiento { hechos, total });
    }
    for ruta in diario.estructura.retirados.iter().rev() {
        rutas::estructura::quitar_vacio(juego, ruta)?;
    }
    for ruta in &diario.estructura.creados {
        rutas::estructura::crear(juego, ruta)?;
    }
    for (i, op) in diario
        .operaciones
        .iter()
        .enumerate()
        .filter(|(_, op)| op.despues.is_some())
    {
        let nuevo = rutas::slot(tx, "nuevos", i)?;
        if estado::huella(&nuevo, &cancel)? != op.despues {
            return Err(Error::CambioExterno);
        }
        let destino = rutas::resolver(juego, &op.destino, true)?;
        if fs::symlink_metadata(&destino).is_ok() {
            return Err(Error::CambioExterno);
        }
        fs::rename(nuevo, destino).map_err(|_| Error::Disco)?;
        hechos += 1;
        progreso(EventoAplicacion::Movimiento { hechos, total });
    }
    Ok(())
}

pub(super) fn deshacer(juego: &Path, tx: &Path, diario: &Diario) -> Result<(), Error> {
    let cancel = CancellationToken::new();
    // La existencia del slot distingue una publicación de un destino aún original.
    for (i, op) in diario
        .operaciones
        .iter()
        .enumerate()
        .rev()
        .filter(|(_, op)| op.despues.is_some())
    {
        let nuevo = rutas::slot(tx, "nuevos", i)?;
        if fs::symlink_metadata(&nuevo).is_ok() {
            continue;
        }
        match rutas::estructura::tipo(juego, &op.destino)? {
            None => continue,
            Some(true) => return Err(Error::Recuperacion),
            Some(false) => (),
        }
        let destino = rutas::resolver(juego, &op.destino, false)?;
        if estado::huella(&destino, &cancel)? != op.despues {
            return Err(Error::Recuperacion);
        }
        fs::rename(destino, nuevo).map_err(|_| Error::Disco)?;
    }
    for ruta in diario.estructura.creados.iter().rev() {
        match rutas::estructura::tipo(juego, ruta)? {
            Some(true) => rutas::estructura::quitar_vacio(juego, ruta)?,
            // Aún existe el archivo original o ya se restauró en otro intento.
            Some(false) | None => (),
        }
    }
    for ruta in &diario.estructura.retirados {
        match rutas::estructura::tipo(juego, ruta)? {
            Some(true) => (),
            None => rutas::estructura::crear(juego, ruta)?,
            Some(false) => return Err(Error::Recuperacion),
        }
    }
    for (i, op) in diario
        .operaciones
        .iter()
        .enumerate()
        .filter(|(_, op)| op.antes.is_some())
    {
        let anterior = rutas::slot(tx, "anteriores", i)?;
        let guardado = estado::huella(&anterior, &cancel)?;
        let destino = rutas::resolver(juego, &op.destino, true)?;
        let actual = estado::huella(&destino, &cancel)?;
        if guardado.is_some() {
            if guardado != op.antes || actual.is_some() {
                return Err(Error::Recuperacion);
            }
            fs::rename(anterior, destino).map_err(|_| Error::Disco)?;
        } else if actual != op.antes {
            return Err(Error::Recuperacion);
        }
    }
    Ok(())
}
