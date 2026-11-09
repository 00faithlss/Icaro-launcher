// SPDX-License-Identifier: GPL-3.0-only
use super::{modelos, tareas, Estado, Etapa, Operacion};
use icaro_ui::compartir as v;

pub fn analizada(s: &mut Estado, id: u64, resultado: Result<tareas::Trabajo, String>) {
    if !s.vigente(id) || s.etapa != Etapa::Analizando {
        return;
    }
    match resultado {
        Ok(t) if s.seleccionada.as_ref() == Some(&t.instancia.id) => {
            s.revision = Some(modelos::revision(&t));
            s.trabajo = Some(t);
            s.etapa = Etapa::Revision;
            s.error = None;
        }
        Ok(_) => (),
        Err(e) => {
            s.etapa = Etapa::Selector;
            s.error = Some(e);
        }
    }
}
/// Devuelve `true` si la publicación quedó activa y hay que generar la invitación.
pub fn publicada(s: &mut Estado, id: u64, resultado: Result<tareas::Publicada, String>) -> bool {
    if !s.vigente(id) || s.etapa != Etapa::Publicando {
        return false;
    }
    match resultado {
        Err(e) => {
            s.etapa = Etapa::Revision;
            if let Some(r) = &mut s.revision {
                r.estado = v::EstadoRevisionPublicacion::Error(e);
            }
        }
        Ok(publicada) => {
            let Some(t) = s.trabajo.as_mut() else {
                return false;
            };
            t.anterior = Some(publicada.revision.clone());
            s.publicacion = Some(tareas::PublicacionLocal::desde(t));
            let activa = publicada.anfitrion.is_ok();
            let error = publicada.anfitrion.as_ref().err().cloned();
            if let Ok(h) = publicada.anfitrion {
                s.anfitrion = Some(h);
            }
            s.panel = Some(v::PanelPublicacion {
                instancia: modelos::instancia(&t.instancia, false),
                revision: publicada.revision.manifiesto.revision,
                fecha_revision: "Ahora".into(),
                hash_revision: String::new(),
                disponibilidad: error.map_or_else(
                    || "Disponible mientras Ícaro esté abierto".into(),
                    |e| format!("Revisión guardada; no se pudo iniciar: {e}"),
                ),
                transferencias: "Sin transferencias activas".into(),
                huella: t.identidad.huella(),
                contenido_privado: t
                    .inventario
                    .archivos
                    .len()
                    .saturating_sub(publicada.revision.manifiesto.archivos.len()),
                estado: if activa {
                    v::EstadoPublicacion::Compartiendo
                } else {
                    v::EstadoPublicacion::Detenida
                },
                invitacion: None,
                accesos: v::Accesos {
                    instancia: t.instancia.nombre.clone(),
                    ..Default::default()
                },
            });
            s.etapa = Etapa::Panel;
            if activa && !s.cerrar_al_terminar {
                return true;
            }
        }
    }
    if s.cerrar_al_terminar {
        s.cerrar_al_terminar = false;
        s.cerrar();
    }
    false
}
pub fn invitacion(
    s: &mut Estado,
    id: u64,
    resultado: Result<icaro_core::compartir::invitaciones::InvitacionCreada, String>,
) {
    if !s.vigente(id) {
        return;
    }
    s.red_ocupada = false;
    if let Some(p) = &mut s.panel {
        match resultado {
            Ok(i) => {
                p.invitacion = Some(v::Invitacion {
                    codigo: icaro_core::compartir::invitaciones::Codigo::parse(&i.code)
                        .map_or_else(|_| String::new(), |c| c.mostrar()),
                    estado: v::EstadoInvitacion::Vigente {
                        restante: format!(
                            "{} horas",
                            super::acciones::caducidad(s.caducidad).horas()
                        ),
                    },
                    caducidad: s.caducidad,
                    cupo: i.capacity,
                    aceptados: i.accepted,
                    copiado: false,
                    ajustes_editables: true,
                });
            }
            Err(e) => {
                p.disponibilidad =
                    format!("Publicación disponible. No se pudo generar la invitación: {e}");
                p.invitacion = Some(v::Invitacion {
                    estado: v::EstadoInvitacion::Revocada,
                    ..Default::default()
                });
            }
        }
    }
    if s.cerrar_al_terminar {
        s.cerrar_al_terminar = false;
        s.cerrar();
    }
}
pub fn operada(s: &mut Estado, id: u64, operacion: Operacion, resultado: Result<(), String>) {
    if !s.vigente(id) {
        return;
    }
    s.red_ocupada = false;
    if let Some(p) = &mut s.panel {
        if matches!(operacion, Operacion::Detener) {
            s.anfitrion = None;
            p.estado = v::EstadoPublicacion::Detenida;
            if let Some(i) = &mut p.invitacion {
                i.estado = v::EstadoInvitacion::Revocada;
            }
        }
        match resultado {
            Err(e) => {
                p.disponibilidad = e;
                if let Operacion::Acceso(key) = &operacion {
                    if let Some(a) = p.accesos.personas.iter_mut().find(|a| &a.id == key) {
                        a.estado = "Revocado localmente; pendiente en el servicio".into();
                        a.conectado = false;
                    }
                }
            }
            Ok(()) => match operacion {
                Operacion::Codigo => {
                    if let Some(i) = &mut p.invitacion {
                        i.estado = v::EstadoInvitacion::Revocada;
                    }
                }
                Operacion::Acceso(key) => {
                    p.accesos.personas.retain(|a| a.id != key);
                    p.accesos.confirmar = None;
                }
                Operacion::Detener => {
                    s.anfitrion = None;
                    p.estado = v::EstadoPublicacion::Detenida;
                    p.invitacion = None;
                    p.accesos.personas.clear();
                }
            },
        }
    }
    s.etapa = Etapa::Panel;
    if s.cerrar_al_terminar {
        s.cerrar_al_terminar = false;
        s.cerrar();
    }
}
