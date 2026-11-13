// SPDX-License-Identifier: GPL-3.0-only
use super::{modelos, Etapa};
use crate::App;
use icaro_ui::compartir as v;
use iced::Element;

pub fn transferencias(s: &super::Estado) -> Vec<v::Transferencia> {
    s.anfitrion.as_ref().map_or_else(Vec::new, |h| {
        h.transferencias_activas()
            .into_iter()
            .map(|t| v::Transferencia {
                nombre: format!(
                    "Equipo {} · {}",
                    t.invitado.chars().take(8).collect::<String>(),
                    if t.archivo.is_empty() {
                        "Archivo"
                    } else {
                        &t.archivo
                    }
                ),
                fase: if t.total == 0 {
                    "Preparando".into()
                } else {
                    format!("{} / {} bytes", t.enviados, t.total)
                },
                porcentaje: if t.total == 0 {
                    0.
                } else {
                    (t.enviados as f64 / t.total as f64 * 100.).clamp(0., 100.) as f32
                },
            })
            .collect()
    })
}

/// La fase del flujo de compartir sin migas ni página: Con amigos la
/// muestra dentro de su panel de detalle.
pub fn fase_pura(app: &App) -> Option<Element<'_, v::Accion>> {
    let s = &app.servicios.compartir;
    let p = app.paleta();
    let vista = match s.etapa {
        Etapa::Cerrada => return None,
        Etapa::Selector => v::selector_instancia(p, &super::selector::modelo(app)),
        Etapa::Analizando => {
            // Misma fase que la revisión: se ve su estado «Analizando» o su error.
            let id = s.seleccionada.as_deref()?;
            let i = app.servicios.instancias.registros.iter().find(|i| i.id == id)?;
            let base = s
                .biblioteca
                .entradas
                .iter()
                .find(|e| e.instancia == id)
                .and_then(|e| e.revision)
                .unwrap_or(0);
            v::revision_publicacion(
                p,
                &v::RevisionPublicacion {
                    instancia: modelos::instancia(i, false),
                    base,
                    nueva: base + 1,
                    cambios: vec![],
                    estado: match &s.error {
                        Some(e) => v::EstadoRevisionPublicacion::Error(e.clone()),
                        None => v::EstadoRevisionPublicacion::Analizando { hechos: 0, total: 0 },
                    },
                    confirmar_cancelacion: false,
                    guardando: false,
                    vista: Default::default(),
                },
            )
        }
        Etapa::Revision | Etapa::Publicando | Etapa::Guardando => {
            v::revision_publicacion(p, s.revision.as_ref()?)
        }
        Etapa::Propios => {
            let t = s.trabajo.as_ref()?;
            v::confirmar_propios(
                p,
                &v::ConfirmarPropios {
                    instancia: t.instancia.nombre.clone(),
                    revision: s.revision.as_ref()?.nueva,
                    archivos: t
                        .borrador
                        .autorizacion_pendiente()
                        .iter()
                        .map(|a| modelos::archivo(a, t))
                        .collect(),
                    autorizado: s.permiso_propios,
                },
            )
        }
        Etapa::Listo => v::compartido_listo(p, &super::listo::modelo(s)?),
        // Recuperando o reconectando (Reanudar, Volver a compartir): esqueleto.
        Etapa::Panel if s.red_ocupada && s.anfitrion.is_none() => {
            v::panel_publicacion_cargando(p, s.panel.as_ref()?, app.reducir)
        }
        Etapa::Panel => v::panel_publicacion(p, s.panel.as_ref()?),
        Etapa::Cierre => v::cierre_transferencias(
            p,
            &v::CierreTransferencias {
                cerrar_icaro: false,
                instancia: s.panel.as_ref()?.instancia.nombre.clone(),
                transferencias: transferencias(s),
                accesos: s.panel.as_ref()?.accesos.personas.len(),
                opcion: s.opcion_cierre,
            },
        ),
        Etapa::Entrada => v::entrada_invitacion(
            p,
            &v::EntradaInvitacion {
                codigo: s.codigo_entrada.clone(),
                estado: s.invitado.entrada.clone(),
            },
        ),
        Etapa::Identidad => super::invitado_vistas::identidad(p, s)?,
        Etapa::Destino => super::invitado_vistas::destino(p, s)?,
        Etapa::RevisionInvitado => super::invitado_vistas::revision(p, s)?,
        Etapa::InstalandoInvitado | Etapa::ResultadoInvitado => {
            super::invitado_vistas::progreso(p, s)?
        }
        Etapa::ConsultandoVinculo
        | Etapa::RevisionVinculo
        | Etapa::AplicandoVinculo
        | Etapa::ErrorVinculo => super::sync_vistas::vista(p, s)?,
    };
    Some(vista)
}
