// SPDX-License-Identifier: GPL-3.0-only
//! Transporte autenticado iroh. No abre ningún servidor HTTP.
mod anfitrion;
mod cliente;
pub mod servidor;
use super::{identidad::Identidad, protocolo::ALPN, Error};
pub use anfitrion::{NodoAnfitrion, TransferenciaActiva};
pub use cliente::{Cliente, RevisionDescarga};

/// Invitado sin publicación propia: conexión al servidor del anfitrión con un
/// endpoint nuevo de su identidad.
pub async fn conectar_servidor(
    identidad: &Identidad,
    direccion: EndpointAddr,
    origen: &super::Origen,
    modo: Modo,
) -> Result<servidor::Conexion, Error> {
    let endpoint = endpoint(identidad, modo).await?;
    servidor::Conexion::nueva(endpoint, true, direccion, origen).await
}
use iroh::{endpoint::presets, Endpoint, EndpointAddr, RelayMode};

/// Nombre de la cuenta activa, con el que este equipo se presenta a los
/// anfitriones al consultar sus publicaciones.
static JUGADOR: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

/// La interfaz lo actualiza al cambiar de cuenta; `None` no se presenta.
pub fn fijar_jugador(nombre: Option<&str>) {
    if let Ok(mut j) = JUGADOR.lock() {
        let nuevo = nombre.and_then(super::nombres::limpiar);
        if *j != nuevo {
            *j = nuevo;
        }
    }
}
fn jugador() -> Option<String> {
    JUGADOR.lock().ok().and_then(|j| j.clone())
}

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
        .alpns(vec![ALPN.to_vec(), servidor::ALPN_SERVIDOR.to_vec()])
        .bind()
        .await
        .map_err(|_| Error::Datos("transporte"))
}
