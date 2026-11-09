// SPDX-License-Identifier: GPL-3.0-only
//! Cliente firmado para el servicio de encuentro; no publica ni descarga mods.
mod autoridad;
mod cliente;
mod codigo;
mod encuentro;
mod modelos;
use super::{identidad::Identidad, Error};
pub use autoridad::AutoridadServicio;
pub use cliente::ClienteInvitaciones;
pub use codigo::Codigo;
pub use encuentro::{DatosEncuentro, EncuentroFirmado};
pub use modelos::{Caducidad, InvitacionCreada, InvitacionResuelta, Reencuentro};
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Serialize)]
struct SobreFirmado {
    public_key: String,
    nonce: String,
    issued_at: u64,
    payload: String,
    signature: String,
}
impl SobreFirmado {
    fn crear(identidad: &Identidad, ruta: &str, payload: String) -> Result<Self, Error> {
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes).map_err(|_| Error::Credencial)?;
        let nonce: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let issued_at = ahora()?;
        let public_key = identidad.clave_publica();
        let mensaje = format!(
            "icaro-invitaciones/1\nPOST\n{ruta}\n{public_key}\n{nonce}\n{issued_at}\n{payload}"
        );
        let signature = identidad.firmar(mensaje.as_bytes());
        Ok(Self {
            public_key,
            nonce,
            issued_at,
            payload,
            signature,
        })
    }
}
fn ahora() -> Result<u64, Error> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|t| t.as_secs())
        .map_err(|_| Error::Datos("reloj del equipo"))
}
