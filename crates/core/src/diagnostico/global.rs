// SPDX-License-Identifier: GPL-3.0-only
//! Registro global opcional, instalado de forma explícita por una sesión.
use super::Registro;
use std::sync::Mutex;

static GLOBAL: Mutex<Option<Registro>> = Mutex::new(None);

/// Instala `registro` si no hay otro global abierto. Devuelve `true` si queda instalado.
pub(crate) fn instalar(registro: &Registro) -> bool {
    let mut global = GLOBAL.lock().unwrap_or_else(|e| e.into_inner());
    match global.as_ref() {
        Some(actual) if actual.mismo(registro) => true,
        Some(actual) if !actual.cerrado() => false,
        _ => {
            *global = Some(registro.clone());
            true
        }
    }
}

/// Copia del registro global; la espera del mutex es solo de memoria.
pub(crate) fn actual() -> Option<Registro> {
    GLOBAL.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Retira el global si corresponde a `registro`.
pub(crate) fn retirar(registro: &Registro) {
    let mut global = GLOBAL.lock().unwrap_or_else(|e| e.into_inner());
    if global.as_ref().is_some_and(|actual| actual.mismo(registro)) {
        *global = None;
    }
}
