// SPDX-License-Identifier: GPL-3.0-only
//! Transporte autenticado iroh. No abre ningún servidor HTTP.
mod anfitrion;
mod cliente;
use super::{identidad::Identidad, protocolo::ALPN, Error};
pub use anfitrion::NodoAnfitrion;
pub use cliente::Cliente;
use iroh::{endpoint::presets, Endpoint, RelayMode};

#[derive(Clone, Copy, Debug)]
pub enum Modo {
    Internet,
    Local,
}
async fn endpoint(identidad: &Identidad, modo: Modo) -> Result<Endpoint, Error> {
    let builder = match modo {
        Modo::Internet => Endpoint::builder(presets::N0),
        Modo::Local => Endpoint::builder(presets::Minimal)
            .relay_mode(RelayMode::Disabled)
            .bind_addr("127.0.0.1:0")
            .map_err(|_| Error::Datos("dirección local"))?,
    };
    builder
        .secret_key(identidad.clave_endpoint())
        .alpns(vec![ALPN.to_vec()])
        .bind()
        .await
        .map_err(|_| Error::Datos("transporte"))
}
