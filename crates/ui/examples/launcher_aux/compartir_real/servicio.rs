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
    /// Endpoint desplegado; el entorno permite desarrollo o deshabilitarlo con vacío.
    pub fn desde_entorno() -> Option<Self> {
        let url = std::env::var("ICARO_SERVICIO_INVITACIONES")
            .unwrap_or_else(|_| "https://icaro-invitaciones.maurigajardov.workers.dev".into());
        if url.trim().is_empty() {
            return None;
        }
        let modo = if std::env::var("ICARO_COMPARTIR_LOCAL").as_deref() == Ok("1") {
            Modo::Local
        } else {
            Modo::Internet
        };
        Some(Self { url, modo })
    }
}
