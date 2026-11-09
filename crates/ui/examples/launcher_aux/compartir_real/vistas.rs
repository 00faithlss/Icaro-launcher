// SPDX-License-Identifier: GPL-3.0-only
use super::{modelos, Accion, Etapa};
use crate::{App, Mensaje};
use icaro_ui::compartir as v;
use iced::Element;

pub fn vista(app: &App) -> Option<Element<'_, Mensaje>> {
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
    let ancho: f32 = match s.etapa {
        Etapa::Selector => 720.,
        Etapa::Propios => 720.,
        Etapa::Listo => 700.,
        Etapa::Cierre => 640.,
        Etapa::Entrada => 820.,
        Etapa::Identidad => 680.,
        Etapa::Destino => 720.,
        _ => 960.,
    };
    // Fase de página dentro de Servidores: migas y fase centradas, sin velo.
    let migas = [
        v::Miga::enlace("Servidores", v::Accion::VerServidores),
        v::Miga::enlace("Instancias compartidas", v::Accion::VerCompartidas),
        v::Miga::actual(titulo(s.etapa)),
    ];
    Some(v::pagina(p, &migas, ancho, vista).map(|a| Mensaje::Compartir(Accion::Vista(a))))
}

/// Nombre de la fase en las migas.
fn titulo(e: Etapa) -> &'static str {
    match e {
        Etapa::Cerrada | Etapa::Selector => "Compartir instancia",
        Etapa::Analizando | Etapa::Revision | Etapa::Guardando | Etapa::Publicando => {
            "Revisar publicación"
        }
        Etapa::Propios => "Archivos propios",
        Etapa::Listo => "Compartida",
        Etapa::Panel => "Publicación",
        Etapa::Cierre => "Dejar de compartir",
        Etapa::Entrada | Etapa::Identidad | Etapa::Destino => "Unirme con un código",
        Etapa::RevisionInvitado | Etapa::InstalandoInvitado | Etapa::ResultadoInvitado => {
            "Instalar instancia"
        }
        Etapa::ConsultandoVinculo
        | Etapa::RevisionVinculo
        | Etapa::AplicandoVinculo
        | Etapa::ErrorVinculo => "Sincronizar",
    }
}

/// Orden de la fase en su flujo: la transición avanza o vuelve según la clave.
/// 0 es la página de Servidores y 1 Instancias compartidas.
pub fn clave(s: &super::Estado) -> u64 {
    match s.etapa {
        Etapa::Cerrada if s.compartidas.is_some() => 1,
        Etapa::Cerrada => 0,
        Etapa::Selector => 10,
        Etapa::Analizando | Etapa::Revision | Etapa::Guardando | Etapa::Publicando => 11,
        Etapa::Propios => 12,
        Etapa::Listo => 13,
        Etapa::Panel => 14,
        Etapa::Cierre => 15,
        Etapa::Entrada => 20,
        Etapa::Identidad => 21,
        Etapa::Destino => 22,
        Etapa::RevisionInvitado => 23,
        Etapa::InstalandoInvitado | Etapa::ResultadoInvitado => 24,
        Etapa::ConsultandoVinculo => 30,
        Etapa::RevisionVinculo | Etapa::AplicandoVinculo | Etapa::ErrorVinculo => 31,
    }
}

pub fn accesos(app: &App, ancho: f32) -> Element<'_, Mensaje> {
    if !app.servicios.compartir.habilitado {
        return icaro_ui::componentes::boton(
            app.paleta(),
            "Activar Compartir experimental",
            icaro_ui::componentes::Variante::Secundario,
            Some(Mensaje::Compartir(Accion::ActivarExperimental)),
        );
    }
    let resumen = super::biblioteca::resumenes(
        &app.servicios.compartir,
        &app.servicios.instancias.registros,
    );
    v::accesos_servidores(app.paleta(), &resumen, ancho).map(|a| Mensaje::Compartir(Accion::Vista(a)))
}

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
