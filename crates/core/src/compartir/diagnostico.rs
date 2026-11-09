// SPDX-License-Identifier: GPL-3.0-only
//! Causas seguras del protocolo, capturadas antes de convertirse a texto de UI.
use super::Error;
use crate::{diagnostico::Causa, diagnostico_contexto as registro};

pub fn causa(e: &Error) -> Causa {
    match e {
        Error::Datos("integridad del catálogo") => Causa::Integridad,
        Error::Datos(detalle) | Error::Limite(detalle) => Causa::Validacion(detalle),
        Error::Disco => Causa::Disco {
            tipo: "sin_detalle",
            codigo_os: None,
        },
        Error::Cancelada => Causa::Validacion("cancelada"),
        Error::CambioExterno => Causa::Integridad,
        Error::Recuperacion => Causa::Conflicto,
        Error::Credencial => Causa::Credencial,
        Error::Conexion => Causa::Red {
            timeout: false,
            conexion: true,
            estado: None,
        },
        Error::Servicio { estado, codigo, .. } => Causa::servicio(*estado, codigo),
    }
}

/// Sin Display/Debug en el registro. El texto de UI mantiene su comportamiento.
#[track_caller]
pub fn texto(e: Error) -> String {
    // El contexto conserva la causa precisa si después llega otra genérica.
    registro::fallo(causa(&e));
    e.to_string()
}

#[track_caller]
pub(crate) fn disco(e: std::io::Error) -> Error {
    registro::fallo(Causa::de_io(&e));
    Error::Disco
}

#[track_caller]
pub(crate) fn red(e: reqwest::Error) -> Error {
    registro::fallo(Causa::de_red(&e));
    Error::Conexion
}
