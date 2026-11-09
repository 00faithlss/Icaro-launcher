// SPDX-License-Identifier: GPL-3.0-only
use super::{comprobar_cancelacion, diario::Huella, Error};
use sha2::{Digest, Sha512};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
};
use tokio_util::sync::CancellationToken;

pub(super) fn de_bytes(bytes: &[u8]) -> Huella {
    Huella {
        bytes: bytes.len() as u64,
        sha512: format!("{:x}", Sha512::digest(bytes)),
    }
}
pub(super) fn abrir(path: &Path) -> Result<fs::File, Error> {
    let mut opciones = fs::OpenOptions::new();
    opciones.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        opciones.share_mode(1).custom_flags(0x00200000);
    }
    opciones.open(path).map_err(|e| crate::compartir::diagnostico::disco(e))
}
pub(super) fn leer_acotado(path: &Path, max: u64) -> Result<Option<Vec<u8>>, Error> {
    let meta = match fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(crate::compartir::diagnostico::disco(e)),
        Ok(_) => {
            crate::almacenamiento::regular(path).map_err(|_| Error::Datos("archivo no regular"))?
        }
    };
    if !meta.is_file() || meta.len() > max {
        return Err(Error::Limite("archivo de metadatos"));
    }
    let mut bytes = Vec::new();
    abrir(path)?
        .take(max.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| crate::compartir::diagnostico::disco(e))?;
    if bytes.len() as u64 > max {
        return Err(Error::Limite("archivo de metadatos"));
    }
    Ok(Some(bytes))
}
pub(super) fn huella(path: &Path, cancel: &CancellationToken) -> Result<Option<Huella>, Error> {
    match fs::symlink_metadata(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(crate::compartir::diagnostico::disco(e)),
        Ok(_) => (),
    }
    let meta =
        crate::almacenamiento::regular(path).map_err(|_| Error::Datos("archivo no regular"))?;
    if !meta.is_file() {
        return Err(Error::Datos("destino no es archivo"));
    }
    let mut entrada = abrir(path)?;
    let mut hash = Sha512::new();
    let mut bytes = 0u64;
    let mut buf = [0u8; 64 * 1024];
    loop {
        comprobar_cancelacion(cancel)?;
        let n = entrada.read(&mut buf).map_err(|e| crate::compartir::diagnostico::disco(e))?;
        if n == 0 {
            break;
        }
        bytes = bytes.checked_add(n as u64).ok_or(Error::CambioExterno)?;
        if bytes > meta.len() {
            return Err(Error::CambioExterno);
        }
        hash.update(&buf[..n]);
    }
    if bytes != meta.len() {
        return Err(Error::CambioExterno);
    }
    Ok(Some(Huella {
        bytes,
        sha512: format!("{:x}", hash.finalize()),
    }))
}
pub(super) fn copiar(
    mut entrada: Box<dyn Read + Send>,
    path: &Path,
    esperada: &Huella,
    cancel: &CancellationToken,
) -> Result<(), Error> {
    let mut salida = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|e| crate::compartir::diagnostico::disco(e))?;
    let mut hash = Sha512::new();
    let mut bytes = 0u64;
    let mut buf = [0u8; 64 * 1024];
    loop {
        comprobar_cancelacion(cancel)?;
        let n = entrada.read(&mut buf).map_err(|e| crate::compartir::diagnostico::disco(e))?;
        if n == 0 {
            break;
        }
        bytes = bytes.checked_add(n as u64).ok_or(Error::CambioExterno)?;
        if bytes > esperada.bytes {
            return Err(Error::CambioExterno);
        }
        hash.update(&buf[..n]);
        salida.write_all(&buf[..n]).map_err(|e| crate::compartir::diagnostico::disco(e))?;
    }
    if bytes != esperada.bytes || format!("{:x}", hash.finalize()) != esperada.sha512 {
        return Err(Error::CambioExterno);
    }
    salida.sync_all().map_err(|e| crate::compartir::diagnostico::disco(e))
}
