// SPDX-License-Identifier: GPL-3.0-only
use super::{ahora, Error, Identidad};
use crate::compartir::manifiesto::hexadecimal;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EncuentroFirmado {
    pub payload: String,
    pub signature: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DatosEncuentro {
    pub host: String,
    pub publication: String,
    pub endpoint_hints: Vec<String>,
    pub issued_at: u64,
}
impl DatosEncuentro {
    fn validar(&self) -> Result<(), Error> {
        if !hexadecimal(&self.host, 64)
            || !hexadecimal(&self.publication, 32)
            || self.endpoint_hints.len() > 8
            || self
                .endpoint_hints
                .iter()
                .any(|h| h.len() > 512 || h.chars().any(char::is_control))
        {
            return Err(Error::Datos("datos de encuentro"));
        }
        Ok(())
    }
}
impl EncuentroFirmado {
    pub fn desde_direccion(
        identidad: &Identidad,
        publicacion: &str,
        direccion: &iroh::EndpointAddr,
    ) -> Result<Self, Error> {
        if direccion.id.to_string() != identidad.clave_publica() {
            return Err(Error::Datos("identidad del endpoint"));
        }
        let hint = serde_json::to_string(direccion)
            .map_err(|_| Error::Datos("dirección del anfitrión"))?;
        Self::crear(identidad, publicacion, vec![hint])
    }
    /// Las direcciones son pistas; la clave pública firmada sigue siendo la identidad.
    pub fn direccion(&self, host: &str, publicacion: &str) -> Result<iroh::EndpointAddr, Error> {
        let datos = self.verificar(host, publicacion)?;
        for hint in datos.endpoint_hints {
            if let Ok(direccion) = serde_json::from_str::<iroh::EndpointAddr>(&hint) {
                if direccion.id.to_string() != host {
                    return Err(Error::Datos("identidad del endpoint"));
                }
                return Ok(direccion);
            }
        }
        Err(Error::Datos("dirección del anfitrión no disponible"))
    }
    pub fn crear(
        identidad: &Identidad,
        publicacion: &str,
        hints: Vec<String>,
    ) -> Result<Self, Error> {
        let datos = DatosEncuentro {
            host: identidad.clave_publica(),
            publication: publicacion.into(),
            endpoint_hints: hints,
            issued_at: ahora()?,
        };
        datos.validar()?;
        let payload = serde_json::to_string(&datos).map_err(|_| Error::Datos("encuentro"))?;
        if payload.len() > 4096 {
            return Err(Error::Limite("datos de encuentro"));
        }
        let signature = identidad.firmar(format!("icaro-encuentro/1\n{payload}").as_bytes());
        Ok(Self { payload, signature })
    }
    pub fn verificar(&self, host: &str, publicacion: &str) -> Result<DatosEncuentro, Error> {
        if self.payload.len() > 4096 {
            return Err(Error::Limite("datos de encuentro"));
        }
        Identidad::verificar(
            host,
            format!("icaro-encuentro/1\n{}", self.payload).as_bytes(),
            &self.signature,
        )?;
        let datos: DatosEncuentro =
            serde_json::from_str(&self.payload).map_err(|_| Error::Datos("encuentro"))?;
        datos.validar()?;
        if datos.host != host || datos.publication != publicacion {
            return Err(Error::Datos("identidad o publicación del encuentro"));
        }
        // El encuentro persistido puede tener días: su fecha no es el TTL de la petición.
        Ok(datos)
    }
}
