// SPDX-License-Identifier: GPL-3.0-only
//! Autenticación sin cuenta, códigos ni respuestas del proveedor en los logs.
use super::{Error, Stage};
use crate::{diagnostico::Causa, diagnostico_contexto as registro, http};
use tokio_util::sync::CancellationToken;

pub(super) fn progreso(etapa: Stage, informar: &impl Fn(Stage)) {
    registro::etapa(match etapa {
        Stage::WaitingForBrowser => "espera_navegador",
        Stage::Microsoft => "microsoft",
        Stage::Xbox => "xbox",
        Stage::Minecraft => "minecraft",
        Stage::Ownership => "titularidad",
        Stage::Profile => "perfil",
    });
    informar(etapa);
}

pub(super) async fn medir<T>(
    accion: &'static str,
    cancel: CancellationToken,
    futuro: impl std::future::Future<Output = Result<T, Error>>,
) -> Result<T, Error> {
    registro::observar("cuenta", accion, None, &cancel, async {
        let resultado = futuro.await;
        if let Err(e) = &resultado {
            registro::fallo(causa(e));
        }
        resultado
    })
    .await
}

fn causa(error: &Error) -> Causa {
    match error {
        Error::Http(http::Error::Status { status, .. }) => Causa::Red {
            timeout: false,
            conexion: false,
            estado: Some(*status),
        },
        Error::Http(http::Error::Network(_)) => Causa::Red {
            timeout: false,
            conexion: true,
            estado: None,
        },
        Error::Http(http::Error::RateLimited(_)) => Causa::Red {
            timeout: false,
            conexion: false,
            estado: Some(429),
        },
        Error::Http(http::Error::Response(_)) => Causa::Validacion("respuesta_auth"),
        Error::Cancelled => Causa::Validacion("auth_cancelada"),
        Error::Expired => Causa::Validacion("auth_vencida"),
        Error::Setup => Causa::Validacion("preparacion_auth"),
        Error::Browser => Causa::Validacion("navegador"),
        Error::PublicClient => Causa::Validacion("cliente_publico"),
        Error::Denied => Causa::Credencial,
        Error::Callback => Causa::Validacion("retorno_auth"),
        Error::XboxProfile => Causa::Validacion("perfil_xbox"),
        Error::XboxRestricted => Causa::Validacion("restriccion_xbox"),
        Error::NoMinecraft => Causa::Validacion("sin_minecraft"),
    }
}
