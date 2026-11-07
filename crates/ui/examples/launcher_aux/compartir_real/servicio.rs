// SPDX-License-Identifier: GPL-3.0-only
//! Servicio de invitaciones y modo de transporte, inyectables en pruebas.
//! Producción los lee de las mismas variables de entorno; no valida aquí la URL:
//! eso lo hace el cliente del núcleo antes de escribir nada.
use icaro_core::compartir::transporte::Modo;

#[derive(Debug, Clone)]
pub struct Servicio {
    pub url: String,
    pub modo: Modo,
}
impl Servicio {
    /// Sin `ICARO_SERVICIO_INVITACIONES` no hay servicio configurado.
    pub fn desde_entorno() -> Option<Self> {
        let url = std::env::var("ICARO_SERVICIO_INVITACIONES").ok()?;
        let modo = if std::env::var("ICARO_COMPARTIR_LOCAL").as_deref() == Ok("1") {
            Modo::Local
        } else {
            Modo::Internet
        };
        Some(Self { url, modo })
    }
}
