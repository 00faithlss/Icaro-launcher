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
                transferencias: (0..s.anfitrion.as_ref().map_or(0, |h| h.transferencias()))
                    .map(|n| v::Transferencia {
                        nombre: format!("Transferencia {}", n + 1),
                        fase: "En curso".into(),
                        porcentaje: 0.,
                    })
                    .collect(),
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

pub fn accesos(app: &App) -> Element<'_, Mensaje> {
    let s = &app.servicios.compartir;
    let resumen = s.panel.as_ref().map(|p| v::ResumenPublicacion {
        instancia: p.instancia.nombre.clone(),
        revision: p.revision,
        sesiones: p.accesos.personas.len(),
        transferencias: s.anfitrion.as_ref().map_or(0, |h| h.transferencias()),
    });
    v::accesos_servidores(app.paleta(), resumen.as_ref())
        .map(|a| Mensaje::Compartir(Accion::Vista(a)))
}
