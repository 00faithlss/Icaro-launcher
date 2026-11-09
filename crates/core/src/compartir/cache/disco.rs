// SPDX-License-Identifier: GPL-3.0-only
use super::super::{comprobar_cancelacion, diagnostico::disco, Error, Procedencia};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio_util::sync::CancellationToken;

pub(super) fn hash_valido(hash: &str) -> bool {
    hash.len() == 128
        && hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub(super) fn ahora() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u64::MAX as u128) as u64
}
pub(super) fn directorio(path: &Path) -> Result<(), Error> {
    for p in path.ancestors() {
        match fs::symlink_metadata(p) {
            Ok(_) => {
                if !crate::almacenamiento::regular(p)
                    .map_err(|_| Error::Disco)?
                    .is_dir()
                {
                    return Err(Error::Disco);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(disco(e)),
        }
    }
    fs::create_dir_all(path).map_err(disco)?;
    if !crate::almacenamiento::regular(path)
        .map_err(|_| Error::Disco)?
        .is_dir()
    {
        return Err(Error::Disco);
    }
    Ok(())
}
pub(super) fn archivo_regular_opcional(path: &Path) -> Result<Option<fs::Metadata>, Error> {
    match fs::symlink_metadata(path) {
        Ok(_) => {
            let meta = crate::almacenamiento::regular(path).map_err(|_| Error::Disco)?;
            if !meta.is_file() {
                return Err(Error::Disco);
            }
            Ok(Some(meta))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(disco(e)),
    }
}
pub(super) fn bloqueo(path: &Path, compartido: bool) -> Result<Option<fs::File>, Error> {
    archivo_regular_opcional(path)?;
    let f = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(disco)?;
    let resultado = if compartido {
        f.try_lock_shared()
    } else {
        f.try_lock()
    };
    match resultado {
        Ok(()) => Ok(Some(f)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(e)) => Err(disco(e)),
    }
}
pub(super) async fn esperar_bloqueo(
    path: std::path::PathBuf,
    compartido: bool,
    cancel: &CancellationToken,
) -> Result<fs::File, Error> {
    loop {
        comprobar_cancelacion(cancel)?;
        let p = path.clone();
        if let Some(f) = crate::diagnostico_contexto::bloqueante(move || bloqueo(&p, compartido))
            .await
            .map_err(|_| Error::Disco)??
        {
            comprobar_cancelacion(cancel)?;
            return Ok(f);
        }
        tokio::select! {
            _ = cancel.cancelled() => return Err(Error::Cancelada),
            _ = tokio::time::sleep(Duration::from_millis(25)) => {},
        }
    }
}
pub(super) fn verificar(
    path: &Path,
    hash: &str,
    bytes: u64,
    cancel: &CancellationToken,
) -> Result<bool, Error> {
    comprobar_cancelacion(cancel)?;
    let Some(meta) = archivo_regular_opcional(path)? else {
        return Ok(false);
    };
    if meta.len() != bytes {
        return Ok(false);
    }
    let mut opciones = fs::OpenOptions::new();
    opciones.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        opciones.share_mode(1).custom_flags(0x00200000);
    }
    let mut entrada = opciones.open(path).map_err(disco)?;
    let mut sha = Sha512::new();
    let mut leidos = 0u64;
    let mut buf = [0; 64 * 1024];
    loop {
        comprobar_cancelacion(cancel)?;
        let n = entrada.read(&mut buf).map_err(disco)?;
        if n == 0 {
            break;
        }
        leidos = leidos
            .checked_add(n as u64)
            .ok_or(Error::Limite("contenido de caché"))?;
        if leidos > bytes {
            return Ok(false);
        }
        sha.update(&buf[..n]);
    }
    Ok(leidos == bytes && format!("{:x}", sha.finalize()) == hash)
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Meta {
    pub hash: String,
    pub bytes: u64,
    pub usado: u64,
    pub procedencias: Vec<Procedencia>,
}
pub(super) fn validar_procedencia(p: &Procedencia) -> Result<(), Error> {
    if let Procedencia::Modrinth { proyecto, version } = p {
        if [proyecto, version]
            .iter()
            .any(|s| s.is_empty() || s.len() > 256 || s.chars().any(char::is_control))
        {
            return Err(Error::Datos("procedencia de caché"));
        }
    }
    Ok(())
}
pub(super) fn leer_meta(path: &Path, hash: &str, bytes: u64) -> Result<Meta, Error> {
    let nueva = || Meta {
        hash: hash.into(),
        bytes,
        usado: ahora(),
        procedencias: vec![],
    };
    let Some(meta) = archivo_regular_opcional(path)? else {
        return Ok(nueva());
    };
    if meta.len() > 128 * 1024 {
        return Ok(nueva());
    }
    let mut buf = Vec::new();
    fs::File::open(path)
        .map_err(disco)?
        .take(128 * 1024 + 1)
        .read_to_end(&mut buf)
        .map_err(disco)?;
    let Ok(meta) = serde_json::from_slice::<Meta>(&buf) else {
        return Ok(nueva());
    };
    if meta.hash != hash
        || meta.bytes != bytes
        || meta.procedencias.len() > 128
        || meta
            .procedencias
            .iter()
            .any(|p| validar_procedencia(p).is_err())
    {
        return Ok(nueva());
    }
    Ok(meta)
}
pub(super) fn guardar_meta(path: &Path, meta: &Meta) -> Result<(), Error> {
    archivo_regular_opcional(path)?;
    let mut nonce = [0u8; 16];
    getrandom::fill(&mut nonce).map_err(|_| Error::Disco)?;
    let id: String = nonce.iter().map(|b| format!("{b:02x}")).collect();
    let temporal = path.with_file_name(format!(".meta-{id}.tmp"));
    let resultado = (|| {
        let mut out = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporal)
            .map_err(disco)?;
        let bytes = serde_json::to_vec(meta).map_err(|_| Error::Datos("metadatos de caché"))?;
        if bytes.len() > 128 * 1024 {
            return Err(Error::Limite("metadatos de caché"));
        }
        out.write_all(&bytes)
            .and_then(|_| out.sync_all())
            .map_err(disco)?;
        drop(out);
        fs::rename(&temporal, path).map_err(disco)
    })();
    if resultado.is_err() {
        let _ = fs::remove_file(temporal);
    }
    resultado
}
