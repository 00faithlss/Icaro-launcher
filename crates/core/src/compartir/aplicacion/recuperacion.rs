// SPDX-License-Identifier: GPL-3.0-only
//! Recuperación por estados observables del diario; nunca pisa cambios externos.
use super::{
    diario::{Diario, Operacion},
    estado, rutas, Error, PREFIJO,
};
use std::{fs, path::Path};
use tokio_util::sync::CancellationToken;

pub(super) fn recuperar(juego: &Path) -> Result<(), Error> {
    rutas::directorio(juego)?;
    for entry in fs::read_dir(juego).map_err(|_| Error::Disco)? {
        let entry = entry.map_err(|_| Error::Disco)?;
        let nombre = entry.file_name().to_string_lossy().into_owned();
        if !nombre.starts_with(PREFIJO) {
            continue;
        }
        let tx = rutas::transaccion(juego, &nombre)?;
        if fs::symlink_metadata(tx.join("diario.json")).is_err() {
            // Cierre antes de guardar el diario: solo había staging. Se conserva para inspección.
            let id = nombre
                .strip_prefix(PREFIJO)
                .ok_or(Error::Datos("transacción"))?;
            let incompleta = juego.join(format!(".icaro-compartir-incompleta-{id}"));
            if fs::symlink_metadata(&incompleta).is_ok() {
                return Err(Error::Recuperacion);
            }
            fs::rename(tx, incompleta).map_err(|_| Error::Disco)?;
            continue;
        }
        let diario = Diario::cargar(&tx)?;
        // Un corte durante limpieza puede haber eliminado ya uno de los grupos.
        rutas::crear_directorio(&tx.join("nuevos"))?;
        rutas::crear_directorio(&tx.join("anteriores"))?;
        if diario.confirmada {
            conservar_respaldo(juego, &nombre)?;
        } else {
            deshacer(juego, &nombre, &diario)?;
        }
    }
    Ok(())
}
pub(super) fn conservar_respaldo(juego: &Path, nombre: &str) -> Result<(), Error> {
    let tx = rutas::transaccion(juego, nombre)?;
    let id = nombre
        .strip_prefix(PREFIJO)
        .ok_or(Error::Datos("transacción"))?;
    let respaldo = juego.join(format!(".icaro-compartir-respaldo-{id}"));
    if fs::symlink_metadata(&respaldo).is_ok() {
        return Err(Error::Recuperacion);
    }
    fs::rename(tx, respaldo).map_err(|_| Error::Disco)
}
pub(super) fn deshacer(juego: &Path, nombre: &str, diario: &Diario) -> Result<(), Error> {
    let tx = rutas::transaccion(juego, nombre)?;
    if !diario.estructura.vacia() {
        super::estructura::deshacer(juego, &tx, diario)?;
        return descartar(&tx, diario);
    }
    let mut fallo = false;
    for (i, op) in diario.operaciones.iter().enumerate().rev() {
        if deshacer_operacion(juego, &tx, i, op).is_err() {
            fallo = true;
        }
    }
    if fallo {
        return Err(Error::Recuperacion);
    }
    descartar(&tx, diario)
}
fn deshacer_operacion(juego: &Path, tx: &Path, i: usize, op: &Operacion) -> Result<(), Error> {
    let cancel = CancellationToken::new();
    let destino = rutas::resolver(juego, &op.destino, true)?;
    let anterior = rutas::slot(tx, "anteriores", i)?;
    let nuevo = rutas::slot(tx, "nuevos", i)?;
    let guardado = estado::huella(&anterior, &cancel)?;
    let actual = estado::huella(&destino, &cancel)?;
    let pendiente = fs::symlink_metadata(&nuevo).is_ok();
    if let Some(esperada) = &op.antes {
        if let Some(guardado) = guardado {
            if &guardado != esperada {
                return Err(Error::Recuperacion);
            }
            if actual.is_some() {
                if actual != op.despues || pendiente {
                    return Err(Error::Recuperacion);
                }
                fs::rename(&destino, &nuevo).map_err(|_| Error::Disco)?;
            }
            fs::rename(&anterior, &destino).map_err(|_| Error::Disco)?;
        } else if actual.as_ref() != Some(esperada)
            && !(actual.is_some() && (pendiente || op.despues.is_none()))
        {
            return Err(Error::Recuperacion);
        }
    } else if guardado.is_some() {
        return Err(Error::Recuperacion);
    } else if actual.is_some() && !pendiente {
        if actual != op.despues {
            return Err(Error::Recuperacion);
        }
        fs::rename(&destino, &nuevo).map_err(|_| Error::Disco)?;
    }
    Ok(())
}
fn descartar(tx: &Path, diario: &Diario) -> Result<(), Error> {
    // Se eliminan únicamente slots numéricos conocidos; nunca un árbol arbitrario.
    for i in 0..diario.operaciones.len() {
        for grupo in ["nuevos", "anteriores"] {
            let path = rutas::slot(tx, grupo, i)?;
            match fs::symlink_metadata(&path) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(_) => return Err(Error::Disco),
                Ok(_) => (),
            }
            if !crate::almacenamiento::regular(&path)
                .map_err(|_| Error::Recuperacion)?
                .is_file()
            {
                return Err(Error::Recuperacion);
            }
            fs::remove_file(path).map_err(|_| Error::Disco)?;
        }
    }
    fs::remove_dir(tx.join("nuevos")).map_err(|_| Error::Disco)?;
    fs::remove_dir(tx.join("anteriores")).map_err(|_| Error::Disco)?;
    fs::remove_file(tx.join("diario.json")).map_err(|_| Error::Disco)?;
    fs::remove_dir(tx).map_err(|_| Error::Disco)
}
