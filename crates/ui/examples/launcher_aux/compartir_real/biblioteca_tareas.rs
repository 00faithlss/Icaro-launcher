// SPDX-License-Identifier: GPL-3.0-only
//! Lectura de publicaciones existentes y reconexión sin revocar sus accesos.
use super::{servicio::Servicio, tareas::PublicacionLocal};
use icaro_core::{
    compartir::{
        self as c, biblioteca::PublicacionGuardada, identidad::Identidad, revisiones::Revision,
    },
    instancias::{Almacen, Instancia},
};
use icaro_ui::compartir as v;
use std::{path::PathBuf, sync::Arc};

#[derive(Clone)]
pub struct Recuperada {
    pub local: PublicacionLocal,
    pub revision: Revision,
    pub detenida: bool,
}
pub type Reconectada = (
    Arc<c::anfitrion::PublicacionActiva>,
    c::invitaciones::InvitacionCreada,
);

pub async fn listar_con(
    raiz: PathBuf,
    almacen: Almacen,
    identidad: Option<Identidad>,
) -> Result<Vec<PublicacionGuardada>, String> {
    let cancel = tokio_util::sync::CancellationToken::new();
    icaro_core::diagnostico_contexto::observar(
        "compartir",
        "listar_publicaciones",
        None,
        &cancel,
        async move {
            icaro_core::diagnostico_contexto::bloqueante(move || {
                let identidad = match identidad {
                    Some(i) => Some(i),
                    None => Identidad::cargar_existente(&raiz)
                        .map_err(icaro_core::diagnostico_contexto::texto)?,
                };
                match identidad {
                    Some(i) => c::biblioteca::listar(&raiz, &almacen, &i.clave_publica())
                        .map_err(icaro_core::diagnostico_contexto::texto),
                    None => Ok(Vec::new()),
                }
            })
            .await
            .map_err(|_| "No se pudo leer la biblioteca.".to_string())?
        },
    )
    .await
}
pub async fn recuperar_con(
    raiz: PathBuf,
    almacen: Almacen,
    instancia: Instancia,
    identidad: Option<Identidad>,
) -> Result<Recuperada, String> {
    let id = instancia.id.clone();
    let cancel = tokio_util::sync::CancellationToken::new();
    icaro_core::diagnostico_contexto::observar(
        "compartir",
        "recuperar_publicacion",
        Some(&id),
        &cancel,
        async move {
            icaro_core::diagnostico_contexto::bloqueante(move || {
                let identidad = match identidad {
                    Some(i) => i,
                    None => Identidad::cargar_existente(&raiz)
                        .map_err(icaro_core::diagnostico_contexto::texto)?
                        .ok_or_else(|| {
                            "La identidad de esta publicación no está disponible.".to_string()
                        })?,
                };
                let contexto =
                    c::biblioteca::abrir(&raiz, &almacen, &instancia, &identidad.clave_publica())
                        .map_err(icaro_core::diagnostico_contexto::texto)?;
                c::aplicacion::recuperar(&contexto.juego)
                    .map_err(icaro_core::diagnostico_contexto::texto)?;
                let revision = c::revisiones::vigente(&contexto)
                    .map_err(icaro_core::diagnostico_contexto::texto)?
                    .ok_or_else(|| "No hay una revisión publicada para recuperar.".to_string())?;
                let detenida = c::accesos::resumen(&contexto.almacen.join("accesos.json"))
                    .map_err(icaro_core::diagnostico_contexto::texto)?
                    .detenida;
                Ok(Recuperada {
                    local: PublicacionLocal {
                        instancia_id: instancia.id,
                        contexto,
                        identidad,
                    },
                    revision,
                    detenida,
                })
            })
            .await
            .map_err(|_| "No se pudo recuperar la publicación.".to_string())?
        },
    )
    .await
}
pub async fn reconectar_con(
    local: PublicacionLocal,
    cad: v::Caducidad,
    cupo: u32,
    servicio: Option<Servicio>,
) -> Result<Reconectada, String> {
    let id = local.instancia_id.clone();
    let cancel = tokio_util::sync::CancellationToken::new();
    icaro_core::diagnostico_contexto::observar(
        "compartir",
        "reconectar_publicacion",
        Some(&id),
        &cancel,
        async move {
            let Servicio { url, modo } = servicio
                .ok_or_else(|| "El servicio de invitaciones no está configurado.".to_string())?;
            let contexto = local.contexto.clone();
            let revision = icaro_core::diagnostico_contexto::bloqueante(move || {
                if c::accesos::resumen(&contexto.almacen.join("accesos.json"))
                    .map_err(icaro_core::diagnostico_contexto::texto)?
                    .detenida
                {
                    return Err("Usa Volver a compartir para una publicación detenida.".to_string());
                }
                c::revisiones::vigente(&contexto)
                    .map_err(icaro_core::diagnostico_contexto::texto)?
                    .ok_or_else(|| "No hay una revisión publicada para recuperar.".to_string())
            })
            .await
            .map_err(|_| "No se pudo leer la publicación.".to_string())??;
            let h = Arc::new(
                c::anfitrion::PublicacionActiva::iniciar(
                    local.identidad,
                    revision,
                    &local.contexto.almacen.join("accesos.json"),
                    &url,
                    modo,
                    local.contexto.limites,
                )
                .await
                .map_err(icaro_core::diagnostico_contexto::texto)?,
            );
            match h.invitar(super::acciones::caducidad(cad), cupo).await {
                Ok(i) => Ok((h, i)),
                Err(e) => {
                    h.cerrar().await;
                    Err(icaro_core::diagnostico_contexto::texto(e))
                }
            }
        },
    )
    .await
}
