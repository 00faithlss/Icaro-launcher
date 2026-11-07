// SPDX-License-Identifier: GPL-3.0-only
//! Autorizaciones por identidad de equipo, sin credenciales Microsoft.
use super::{manifiesto::hexadecimal, Error};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, future::Future, path::PathBuf, pin::Pin, sync::Mutex};

/// La implementación consulta el servicio autoritativo usando la identidad del anfitrión.
/// Nunca acepta una declaración o token enviado únicamente por el invitado.
pub trait Autoridad: Send + Sync {
    fn autorizado(
        &self,
        invitado: String,
    ) -> Pin<Box<dyn Future<Output = Result<bool, Error>> + Send + '_>>;
}

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Registro {
    #[serde(default)]
    detenida: bool,
    permitidos: BTreeSet<String>,
    revocados: BTreeSet<String>,
}
pub struct Accesos {
    path: PathBuf,
    registro: Mutex<Registro>,
}
impl Accesos {
    pub fn abrir(path: PathBuf) -> Result<Self, Error> {
        let registro = match std::fs::read(&path) {
            Ok(bytes) => {
                if bytes.len() > 1024 * 1024 {
                    return Err(Error::Limite("accesos"));
                }
                serde_json::from_slice::<Registro>(&bytes).map_err(|_| Error::Datos("accesos"))?
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Registro::default(),
            Err(_) => return Err(Error::Disco),
        };
        if registro
            .permitidos
            .iter()
            .chain(&registro.revocados)
            .any(|k| !hexadecimal(k, 64))
        {
            return Err(Error::Datos("identidad de acceso"));
        }
        Ok(Self {
            path,
            registro: Mutex::new(registro),
        })
    }
    fn cambiar(&self, clave: &str, autorizar: bool) -> Result<(), Error> {
        if !hexadecimal(clave, 64) {
            return Err(Error::Datos("identidad de acceso"));
        }
        let mut actual = self.registro.lock().map_err(|_| Error::Disco)?;
        let mut nuevo = actual.clone();
        if autorizar {
            if nuevo.detenida {
                return Err(Error::Datos("publicación detenida"));
            }
            // Una autorización remota repetida no deshace una revocación local.
            if nuevo.revocados.contains(clave) {
                return Err(Error::Datos("acceso revocado"));
            }
            nuevo.permitidos.insert(clave.to_owned());
        } else {
            nuevo.permitidos.remove(clave);
            nuevo.revocados.insert(clave.to_owned());
        }
        crate::almacenamiento::escribir_json(
            &self.path,
            &serde_json::to_value(&nuevo).map_err(|_| Error::Disco)?,
        )
        .map_err(|_| Error::Disco)?;
        *actual = nuevo;
        Ok(())
    }
    pub fn autorizar(&self, clave: &str) -> Result<(), Error> {
        self.cambiar(clave, true)
    }
    pub fn revocar(&self, clave: &str) -> Result<(), Error> {
        self.cambiar(clave, false)
    }
    pub fn permitir(&self, clave: &str) -> bool {
        self.registro.lock().is_ok_and(|r| {
            !r.detenida && r.permitidos.contains(clave) && !r.revocados.contains(clave)
        })
    }
    /// Persistir antes de cerrar la red: un fallo remoto no restaura los permisos
    /// al reiniciar el programa. Solo reiniciar explícitamente inicia otra etapa.
    pub fn detener(&self) -> Result<(), Error> {
        let mut actual = self.registro.lock().map_err(|_| Error::Disco)?;
        let mut nuevo = actual.clone();
        nuevo.detenida = true;
        crate::almacenamiento::escribir_json(
            &self.path,
            &serde_json::to_value(&nuevo).map_err(|_| Error::Disco)?,
        )
        .map_err(|_| Error::Disco)?;
        *actual = nuevo;
        Ok(())
    }
    /// Acción explícita del anfitrión al volver a compartir esta publicación.
    pub fn reiniciar(&self) -> Result<(), Error> {
        let mut actual = self.registro.lock().map_err(|_| Error::Disco)?;
        let nuevo = Registro::default();
        crate::almacenamiento::escribir_json(
            &self.path,
            &serde_json::to_value(&nuevo).map_err(|_| Error::Disco)?,
        )
        .map_err(|_| Error::Disco)?;
        *actual = nuevo;
        Ok(())
    }
}
