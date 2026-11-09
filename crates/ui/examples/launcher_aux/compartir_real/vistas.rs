// SPDX-License-Identifier: GPL-3.0-only
use super::{modelos, Accion, Etapa};
use crate::{App, Mensaje};
use icaro_ui::compartir as v;
use iced::{widget::container, Element};

pub fn vista(app: &App) -> Option<Element<'_, Mensaje>> {
    let s = &app.servicios.compartir;
    let p = app.paleta();
    let vista = match s.etapa {
        Etapa::Cerrada => return None,
        Etapa::Selector | Etapa::Analizando => {
            let modelo = v::CompartirInstancia {
                instancias: app
                    .servicios
                    .instancias
                    .registros
                    .iter()
                    .map(|i| modelos::instancia(i, app.servicios.juego.ocupada(&i.id)))
                    .collect(),
                seleccionada: s.seleccionada.clone(),
                estado: if s.etapa == Etapa::Analizando {
                    v::EstadoContenido::Analizando {
                        hechos: 0,
                        total: 0,
                    }
                } else {
                    v::EstadoContenido::Bloqueado
                },
                incluidos: vec![],
                total_incluidos: 0,
                excluidos: vec![],
                propios: vec![],
                carpetas: "mods · config · kubejs · resourcepacks".into(),
                tamano_carpetas: String::new(),
                problemas: s
                    .error
                    .iter()
                    .map(|e| v::Problema {
                        archivo: String::new(),
                        titulo: "No se pudo revisar la instancia".into(),
                        detalle: e.clone(),
                        accion: v::Accion::Reintentar,
                        rotulo: "Volver a revisar".into(),
                    })
                    .collect(),
            };
            v::compartir_instancia(p, &modelo)
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
        Etapa::Selector | Etapa::Analizando => 900.,
        Etapa::Propios => 720.,
        Etapa::Cierre => 640.,
        Etapa::Entrada => 820.,
        Etapa::Identidad => 680.,
        Etapa::Destino => 720.,
        _ => 960.,
    };
    Some(
        container(vista.map(|a| Mensaje::Compartir(Accion::Vista(a))))
            .width(ancho.min((app.ventana.width - 48.).max(240.)))
            .height((app.ventana.height - 96.).clamp(240., 800.))
            .align_x(iced::Alignment::Center)
            .align_y(iced::Alignment::Center)
            .into(),
    )
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
