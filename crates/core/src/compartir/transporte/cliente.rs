// SPDX-License-Identifier: GPL-3.0-only
use super::{endpoint, Modo};
use crate::compartir::{
    identidad::Identidad,
    protocolo::{self, Respuesta, Solicitud, ALPN, CONTROL_BYTES},
    Artefacto, Error, Limites, Manifiesto, Origen,
};
use iroh::{endpoint::Connection, Endpoint, EndpointAddr};
use std::path::{Path, PathBuf};
use tokio_util::sync::CancellationToken;
#[path = "descarga.rs"]
mod descarga;
pub use descarga::RevisionDescarga;

pub struct Cliente {
    endpoint: Endpoint,
    endpoint_propio: bool,
    conn: Connection,
    origen: Origen,
    limites: Limites,
}
impl Drop for Cliente {
    fn drop(&mut self) {
        self.conn.close(0u32.into(), b"fin");
    }
}
impl Cliente {
    pub async fn conectar(
        identidad: &Identidad,
        direccion: EndpointAddr,
        origen: &Origen,
        modo: Modo,
        limites: Limites,
    ) -> Result<Self, Error> {
        let endpoint = endpoint(identidad, modo).await?;
        Self::desde_endpoint(endpoint, direccion, origen, limites, true).await
    }
    pub(super) async fn desde_endpoint(
        endpoint: Endpoint,
        direccion: EndpointAddr,
        origen: &Origen,
        limites: Limites,
        endpoint_propio: bool,
    ) -> Result<Self, Error> {
        origen.validar()?;
        if direccion.id.to_string() != origen.anfitrion {
            return Err(Error::Datos("identidad remota"));
        }
        let conn = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            endpoint.connect(direccion, ALPN),
        )
        .await
        .map_err(|_| Error::Datos("tiempo de conexión"))?
        .map_err(|_| Error::Datos("conexión"))?;
        if conn.remote_id().to_string() != origen.anfitrion {
            return Err(Error::Datos("identidad remota"));
        }
        Ok(Self {
            endpoint,
            endpoint_propio,
            conn,
            origen: origen.clone(),
            limites,
        })
    }
    pub async fn consultar(&self) -> Result<Manifiesto, Error> {
        let (mut send, mut recv) = self
            .conn
            .open_bi()
            .await
            .map_err(|_| Error::Datos("conexión"))?;
        protocolo::escribir(
            &mut send,
            &Solicitud::Revision {
                origen: self.origen.clone(),
            },
            CONTROL_BYTES,
        )
        .await?;
        send.finish().map_err(|_| Error::Datos("conexión"))?;
        let max = self
            .limites
            .manifiesto_bytes
            .saturating_mul(2)
            .saturating_add(CONTROL_BYTES);
        let respuesta: Respuesta = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            protocolo::leer(&mut recv, max),
        )
        .await
        .map_err(|_| Error::Datos("tiempo de consulta"))??;
        let Respuesta::Revision { firmada } = respuesta else {
            return Err(Error::Datos("consulta rechazada"));
        };
        let manifiesto = firmada.verificar(&self.origen.anfitrion, &self.limites)?;
        if manifiesto.origen != self.origen {
            return Err(Error::Datos("publicación distinta"));
        }
        Ok(manifiesto)
    }
    /// Descarga a un parcial externo, con reanudación e integridad completa.
    pub async fn descargar(
        &self,
        manifiesto: &Manifiesto,
        artefacto: &Artefacto,
        staging: &Path,
        cancel: &CancellationToken,
    ) -> Result<PathBuf, Error> {
        self.descargar_archivo(manifiesto, artefacto, staging, cancel)
            .await
    }
    pub async fn cerrar(&self) {
        self.conn.close(0u32.into(), b"fin");
        if self.endpoint_propio {
            self.endpoint.close().await;
        }
    }
}
