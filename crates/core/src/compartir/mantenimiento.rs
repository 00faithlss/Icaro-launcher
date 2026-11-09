// SPDX-License-Identifier: GPL-3.0-only
//! Limpieza conservadora de staging propio; nunca elimina recursivamente.
use super::Error;
use std::{
    fs,
    path::Path,
    time::{Duration, SystemTime},
};
fn hex(s: &str, n: usize) -> bool {
    s.len() == n
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn viejo(m: &fs::Metadata, ahora: SystemTime, edad: Duration) -> bool {
    m.modified()
        .ok()
        .and_then(|t| ahora.duration_since(t).ok())
        .is_some_and(|d| d >= edad)
}
fn bloqueo(p: &Path) -> Result<fs::File, Error> {
    if fs::symlink_metadata(p).is_ok()
        && !crate::almacenamiento::regular(p)
            .map_err(|_| Error::Disco)?
            .is_file()
    {
        return Err(Error::Disco);
    }
    let f = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(p)
        .map_err(super::diagnostico::disco)?;
    f.try_lock()
        .map_err(|_| Error::Datos("descargas ocupadas"))?;
    Ok(f)
}
/// Conservar esta guardia durante descarga y preparación; nunca borrar el lock.
pub fn reservar(p: &Path) -> Result<fs::File, Error> {
    fs::create_dir_all(p).map_err(super::diagnostico::disco)?;
    for a in p.ancestors() {
        if !crate::almacenamiento::regular(a)
            .map_err(|_| Error::Disco)?
            .is_dir()
        {
            return Err(Error::Disco);
        }
    }
    bloqueo(&p.join(".icaro-descargas.lock"))
}
/// El llamante conserva reservar(); los parciales recientes quedan para reanudar.
pub fn limpiar(p: &Path, ahora: SystemTime, edad: Duration) -> Result<usize, Error> {
    if !crate::almacenamiento::regular(p)
        .map_err(|_| Error::Disco)?
        .is_dir()
    {
        return Err(Error::Disco);
    }
    let mut n = 0;
    for e in fs::read_dir(p).map_err(super::diagnostico::disco)? {
        let e = e.map_err(super::diagnostico::disco)?;
        let nombre = e.file_name();
        let Some(nombre) = nombre.to_str() else {
            continue;
        };
        let ruta = e.path();
        let Ok(m) = crate::almacenamiento::regular(&ruta) else {
            continue;
        };
        if !viejo(&m, ahora, edad) {
            continue;
        }
        if let Some(hash) = nombre.strip_suffix(".part").filter(|h| hex(h, 128)) {
            if !m.is_file() {
                continue;
            }
            let Ok(_lock) = bloqueo(&p.join(format!("{hash}.lock"))) else {
                continue;
            };
            fs::remove_file(&ruta).map_err(super::diagnostico::disco)?;
            n += 1;
        } else if nombre.strip_prefix("catalogo-").is_some_and(|h| hex(h, 32)) && m.is_dir() {
            let entradas = fs::read_dir(&ruta)
                .map_err(super::diagnostico::disco)?
                .collect::<Result<Vec<_>, _>>()
                .map_err(super::diagnostico::disco)?;
            if entradas.iter().any(|e| {
                e.file_name() != "catalogo.part"
                    || !crate::almacenamiento::regular(&e.path())
                        .is_ok_and(|m| m.is_file() && viejo(&m, ahora, edad))
            }) {
                continue;
            }
            for e in entradas {
                fs::remove_file(e.path()).map_err(super::diagnostico::disco)?;
                n += 1;
            }
            fs::remove_dir(ruta).map_err(super::diagnostico::disco)?;
        }
    }
    Ok(n)
}
