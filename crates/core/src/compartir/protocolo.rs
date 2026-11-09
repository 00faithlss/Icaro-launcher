// SPDX-License-Identifier: GPL-3.0-only
//! Tramas de control acotadas; los archivos viajan fuera del JSON.
use super::{firmas::RevisionFirmada, Error, Origen};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
pub const ALPN: &[u8] = b"icaro-compartir/1";
pub const CONTROL_BYTES: usize = 64 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(tag = "operacion", deny_unknown_fields)]
pub enum Solicitud {
    Revision {
        origen: Origen,
    },
    Archivo {
        origen: Origen,
        revision: u64,
        sha512: String,
        offset: u64,
    },
}
#[derive(Serialize, Deserialize)]
#[serde(tag = "respuesta", deny_unknown_fields)]
pub enum Respuesta {
    Revision { firmada: RevisionFirmada },
    Archivo { restantes: u64 },
    Rechazada,
}
pub async fn escribir<T: Serialize>(
    send: &mut (impl AsyncWrite + Unpin),
    valor: &T,
    max: usize,
) -> Result<(), Error> {
    let bytes = serde_json::to_vec(valor).map_err(|_| Error::Datos("trama"))?;
    if bytes.len() > max {
        return Err(Error::Limite("trama"));
    }
    send.write_u32(bytes.len() as u32)
        .await
        .map_err(|_| Error::Datos("conexión"))?;
    send.write_all(&bytes)
        .await
        .map_err(|_| Error::Datos("conexión"))
}
pub async fn leer<T: DeserializeOwned>(
    recv: &mut (impl AsyncRead + Unpin),
    max: usize,
) -> Result<T, Error> {
    let len = recv
        .read_u32()
        .await
        .map_err(|_| Error::Datos("conexión"))? as usize;
    if len > max {
        return Err(Error::Limite("trama"));
    }
    let mut bytes = vec![0; len];
    recv.read_exact(&mut bytes)
        .await
        .map_err(|_| Error::Datos("conexión"))?;
    serde_json::from_slice(&bytes).map_err(|_| Error::Datos("trama"))
}
