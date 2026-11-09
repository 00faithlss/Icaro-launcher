// SPDX-License-Identifier: GPL-3.0-only
use super::{AlmacenClave, Error, Identidad};
use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::Path};
use zeroize::Zeroizing;

struct Sistema {
    servicio: String,
}
impl AlmacenClave for Sistema {
    fn leer(&self) -> Result<Option<Zeroizing<Vec<u8>>>, Error> {
        #[cfg(any(windows, target_os = "linux"))]
        {
            let entry =
                keyring::Entry::new(&self.servicio, "device").map_err(|_| Error::Credencial)?;
            match entry.get_secret() {
                Ok(bytes) => Ok(Some(Zeroizing::new(bytes))),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(_) => Err(Error::Credencial),
            }
        }
        #[cfg(not(any(windows, target_os = "linux")))]
        {
            Err(Error::Credencial)
        }
    }
    fn guardar(&self, valor: &[u8]) -> Result<(), Error> {
        #[cfg(any(windows, target_os = "linux"))]
        {
            keyring::Entry::new(&self.servicio, "device")
                .map_err(|_| Error::Credencial)?
                .set_secret(valor)
                .map_err(|_| Error::Credencial)
        }
        #[cfg(not(any(windows, target_os = "linux")))]
        {
            let _ = valor;
            Err(Error::Credencial)
        }
    }
}
fn sistema(raiz: &Path) -> Result<Sistema, Error> {
    let canonica = raiz.canonicalize().map_err(|_| Error::Credencial)?;
    let scope = format!(
        "{:x}",
        Sha256::digest(canonica.as_os_str().as_encoded_bytes())
    );
    Ok(Sistema {
        servicio: format!("org.icaro.launcher.compartir.{scope}"),
    })
}
fn conocida(registro: &Path) -> Result<Option<String>, Error> {
    match fs::symlink_metadata(registro) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err(Error::Credencial),
        Ok(_) => {
            let meta = crate::almacenamiento::regular(registro).map_err(|_| Error::Credencial)?;
            if !meta.is_file() || meta.len() > 256 {
                return Err(Error::Credencial);
            }
            let mut bytes = Vec::new();
            fs::File::open(registro)
                .map_err(|_| Error::Credencial)?
                .take(257)
                .read_to_end(&mut bytes)
                .map_err(|_| Error::Credencial)?;
            Ok(Some(
                serde_json::from_slice::<String>(&bytes).map_err(|_| Error::Credencial)?,
            ))
        }
    }
}
/// Solo lectura: sin bloqueo exclusivo, sin generar clave ni escribir el registro público.
pub(super) fn existente(raiz: &Path) -> Result<Option<Identidad>, Error> {
    if !crate::almacenamiento::regular(raiz)
        .map_err(|_| Error::Credencial)?
        .is_dir()
    {
        return Err(Error::Credencial);
    }
    let conocida = conocida(&raiz.join(".icaro-compartir-identidad-publica.json"))?;
    match Identidad::existente(&sistema(raiz)?)? {
        None if conocida.is_some() => Err(Error::Credencial),
        None => Ok(None),
        Some(i) if conocida.as_ref().is_some_and(|c| c != &i.clave_publica()) => {
            Err(Error::Credencial)
        }
        Some(i) => Ok(Some(i)),
    }
}
pub(super) fn cargar(raiz: &Path) -> Result<Identidad, Error> {
    if !crate::almacenamiento::regular(raiz)
        .map_err(|_| Error::Credencial)?
        .is_dir()
    {
        return Err(Error::Credencial);
    }
    let path = raiz.join(".icaro-compartir-identidad.lock");
    if path.exists() {
        crate::almacenamiento::regular(&path).map_err(|_| Error::Credencial)?;
    }
    let bloqueo = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|_| Error::Credencial)?;
    bloqueo.try_lock().map_err(|_| Error::Credencial)?;
    let almacen = sistema(raiz)?;
    let registro = raiz.join(".icaro-compartir-identidad-publica.json");
    let conocida = conocida(&registro)?;
    if conocida.is_some() && almacen.leer()?.is_none() {
        return Err(Error::Credencial);
    }
    let identidad = Identidad::desde_almacen(&almacen)?;
    if conocida
        .as_ref()
        .is_some_and(|c| c != &identidad.clave_publica())
    {
        return Err(Error::Credencial);
    }
    if conocida.is_none() {
        crate::almacenamiento::escribir_json(
            &registro,
            &serde_json::Value::String(identidad.clave_publica()),
        )
        .map_err(|_| Error::Credencial)?;
    }
    Ok(identidad)
}
