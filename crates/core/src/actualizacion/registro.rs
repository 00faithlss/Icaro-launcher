// SPDX-License-Identifier: GPL-3.0-only
//! Metadatos seguros del actualizador; no se vuelcan URLs ni rutas del instalador.
use super::Error;
use crate::{diagnostico::Causa, diagnostico_contexto as registro};
pub(super) async fn medir<T>(
    accion: &'static str,
    futuro: impl std::future::Future<Output = Result<T, Error>>,
) -> Result<T, Error> {
    registro::observar(
        "actualizador",
        accion,
        None,
        &tokio_util::sync::CancellationToken::new(),
        async {
            registro::etapa(if accion == "comprobar" {
                "consulta_release"
            } else {
                "descarga_instalador"
            });
            let resultado = futuro.await;
            if let Err(e) = &resultado {
                registro::fallo(match e {
                    Error::Red => Causa::Red {
                        timeout: false,
                        conexion: true,
                        estado: None,
                    },
                    Error::Integridad => Causa::Integridad,
                    Error::Disco => Causa::Disco {
                        tipo: "sin_detalle",
                        codigo_os: None,
                    },
                    Error::Datos => Causa::Validacion("manifiesto_actualizador"),
                    Error::SinArtefacto => Causa::Validacion("sin_artefacto"),
                    Error::Plataforma => Causa::Validacion("plataforma_actualizador"),
                });
            }
            resultado
        },
    )
    .await
}

#[track_caller]
pub(super) fn red(e: reqwest::Error) -> Error {
    registro::fallo(Causa::de_red(&e));
    Error::Red
}
