// SPDX-License-Identifier: GPL-3.0-only
use super::{modelos, resultados, tareas, Estado, Etapa};
use crate::{App, Mensaje};
use icaro_core::compartir::{
    self as c,
    invitaciones::{Codigo, InvitacionCreada},
    publicacion::Eleccion,
};
use icaro_ui::compartir as v;
use iced::Task;

#[derive(Clone)]
pub enum Accion {
    Vista(v::Accion),
    Analizada(u64, Result<tareas::Trabajo, String>),
    Publicada(u64, Result<tareas::Publicada, String>),
    Guardada(u64, bool, Result<(), String>),
    Invitacion(u64, Result<InvitacionCreada, String>),
    Operada(u64, Operacion, Result<(), String>),
    Accesos(u64, Result<Vec<String>, String>),
    Ajustada(
        u64,
        v::Caducidad,
        Result<c::invitaciones::InvitacionResuelta, String>,
    ),
    Reanudada(
        u64,
        Result<
            (
                std::sync::Arc<c::anfitrion::PublicacionActiva>,
                InvitacionCreada,
            ),
            String,
        >,
    ),
}
// Ni códigos introducidos ni resultados del servicio se vuelcan a Debug.
impl std::fmt::Debug for Accion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Compartir")
            .field(&std::mem::discriminant(self))
            .finish()
    }
}
#[derive(Debug, Clone)]
pub enum Operacion {
    Codigo,
    Acceso(String),
    Detener,
}
fn m(a: Accion) -> Mensaje {
    Mensaje::Compartir(a)
}
pub fn caducidad(c: v::Caducidad) -> c::invitaciones::Caducidad {
    match c {
        v::Caducidad::Horas24 => c::invitaciones::Caducidad::Horas24,
        v::Caducidad::Dias2 => c::invitaciones::Caducidad::Dias2,
        v::Caducidad::Semana => c::invitaciones::Caducidad::Semana,
    }
}
fn analizar(app: &mut App, id: String) -> Task<Mensaje> {
    if app.servicios.demo || !app.servicios.compartir.habilitado {
        return Task::none();
    }
    if app.servicios.juego.ocupada(&id) {
        app.servicios.compartir.error = Some(
            "Cierra la partida o espera a que termine la instalación antes de revisar.".into(),
        );
        return Task::none();
    }
    let s = &mut app.servicios.compartir;
    if s.etapa == Etapa::Publicando || s.red_ocupada || s.etapa == Etapa::Guardando {
        return Task::none();
    }
    if s.anfitrion.is_some() && s.panel.as_ref().is_some_and(|p| p.instancia.id != id) {
        s.error =
            Some("Deja de compartir la instancia actual antes de abrir otra publicación.".into());
        return Task::none();
    }
    let Some(instancia) = app
        .servicios
        .instancias
        .registros
        .iter()
        .find(|i| i.id == id)
        .cloned()
    else {
        return Task::none();
    };
    let request = s.comenzar(id);
    let (cancel, almacen, catalogo) = (
        s.cancel.clone(),
        app.servicios.instancias.almacen.clone(),
        app.servicios.catalog.as_ref().ok().cloned(),
    );
    let raiz = crate::estado::directorio(false);
    Task::perform(
        tareas::analizar(raiz, almacen, instancia, catalogo, cancel),
        move |r| m(Accion::Analizada(request, r)),
    )
}
pub fn invitar(s: &mut Estado) -> Task<Mensaje> {
    if s.red_ocupada {
        return Task::none();
    }
    let Some(h) = s.anfitrion.clone() else {
        return Task::none();
    };
    s.red_ocupada = true;
    if let Some(p) = &mut s.panel {
        p.invitacion = Some(v::Invitacion {
            caducidad: s.caducidad,
            cupo: s.cupo,
            ajustes_editables: false,
            ..Default::default()
        });
    }
    let (id, cad, cupo) = (s.solicitud, caducidad(s.caducidad), s.cupo);
    Task::perform(
        async move { h.invitar(cad, cupo).await.map_err(|e| e.to_string()) },
        move |r| m(Accion::Invitacion(id, r)),
    )
}
fn operar(s: &mut Estado, op: Operacion) -> Task<Mensaje> {
    if s.red_ocupada {
        return Task::none();
    }
    let Some(h) = s.anfitrion.clone() else {
        return Task::none();
    };
    let codigo = s
        .panel
        .as_ref()
        .and_then(|p| p.invitacion.as_ref())
        .and_then(|i| Codigo::parse(&i.codigo).ok());
    let (id, esperar) = (s.solicitud, s.opcion_cierre == v::OpcionCierre::Esperar);
    s.red_ocupada = true;
    Task::perform(
        async move {
            let r = match &op {
                Operacion::Codigo => match codigo {
                    Some(c) => h.revocar_codigo(&c).await,
                    None => Err(c::Error::Datos("código no disponible")),
                },
                Operacion::Acceso(k) => h.revocar(k).await,
                Operacion::Detener => {
                    let drenaje = if esperar { h.drenar().await } else { Ok(()) };
                    let parada = h.detener().await;
                    parada.and(drenaje)
                }
            };
            Ok::<_, c::Error>((op, r))
        },
        move |r| match r {
            Ok((op, r)) => m(Accion::Operada(id, op, r.map_err(|e| e.to_string()))),
            Err(e) => m(Accion::Operada(id, Operacion::Detener, Err(e.to_string()))),
        },
    )
}

pub fn actualizar(app: &mut App, a: Accion) -> Task<Mensaje> {
    if !app.servicios.compartir.habilitado || app.servicios.demo {
        return Task::none();
    }
    match a {
        Accion::Analizada(id, r) => resultados::analizada(&mut app.servicios.compartir, id, r),
        Accion::Publicada(id, r) => {
            return resultados::publicada(&mut app.servicios.compartir, id, r)
        }
        Accion::Invitacion(id, r) => resultados::invitacion(&mut app.servicios.compartir, id, r),
        Accion::Ajustada(id, cad, r) => {
            super::invitaciones::ajustada(&mut app.servicios.compartir, id, cad, r)
        }
        Accion::Reanudada(id, r) => {
            let s = &mut app.servicios.compartir;
            if !s.vigente(id) {
                return Task::none();
            }
            match r {
                Ok((h, i)) => {
                    s.anfitrion = Some(h);
                    if let Some(p) = &mut s.panel {
                        p.estado = v::EstadoPublicacion::Compartiendo;
                        p.disponibilidad = "Disponible mientras Ícaro esté abierto".into();
                        p.accesos.personas.clear();
                    }
                    resultados::invitacion(s, id, Ok(i));
                }
                Err(e) => {
                    s.red_ocupada = false;
                    if let Some(p) = &mut s.panel {
                        p.disponibilidad = e;
                    }
                    if s.cerrar_al_terminar {
                        s.cerrar_al_terminar = false;
                        s.cerrar();
                    }
                }
            }
        }
        Accion::Operada(id, op, r) => resultados::operada(&mut app.servicios.compartir, id, op, r),
        Accion::Guardada(id, confirmar, r) => {
            let s = &mut app.servicios.compartir;
            if !s.vigente(id) || s.etapa != Etapa::Guardando {
                return Task::none();
            }
            s.etapa = Etapa::Revision;
            if let Some(v) = &mut s.revision {
                match r {
                    Ok(()) => {
                        v.estado = v::EstadoRevisionPublicacion::Disponible;
                        v.confirmar_cancelacion = confirmar;
                    }
                    Err(e) => {
                        v.estado = v::EstadoRevisionPublicacion::Error(e);
                        s.cerrar_al_terminar = false;
                    }
                }
            }
            if s.cerrar_al_terminar {
                s.cerrar_al_terminar = false;
                s.cerrar();
            }
        }
        Accion::Accesos(id, r) => {
            let s = &mut app.servicios.compartir;
            if !s.vigente(id) {
                return Task::none();
            }
            s.red_ocupada = false;
            if let Some(p) = &mut s.panel {
                match r {
                    Ok(keys) => {
                        p.accesos.personas = keys
                            .into_iter()
                            .enumerate()
                            .map(|(n, k)| v::Acceso {
                                nombre: format!("Equipo {}", n + 1),
                                huella: format!("Clave {}", &k[..12]),
                                id: k,
                                estado: "Autorizado".into(),
                                conectado: false,
                                aceptado: String::new(),
                            })
                            .collect()
                    }
                    Err(e) => p.disponibilidad = e,
                }
            }
            if s.cerrar_al_terminar {
                s.cerrar_al_terminar = false;
                s.cerrar();
            }
        }
        Accion::Vista(a) => return vista(app, a),
    }
    Task::none()
}

fn vista(app: &mut App, a: v::Accion) -> Task<Mensaje> {
    if let v::Accion::ElegirInstancia(id) = a {
        return analizar(app, id);
    }
    if matches!(a, v::Accion::RevisarPublicacion | v::Accion::Reintentar) {
        if let Some(id) = app.servicios.compartir.seleccionada.clone() {
            return analizar(app, id);
        }
    }
    let s = &mut app.servicios.compartir;
    if s.etapa == Etapa::Publicando || s.etapa == Etapa::Guardando || s.red_ocupada {
        if matches!(
            a,
            v::Accion::Cerrar | v::Accion::Cancelar | v::Accion::ConfirmarCancelarPublicacion
        ) {
            s.cerrar();
        }
        return Task::none();
    }
    match a {
        v::Accion::AbrirCompartir => {
            if !s.modal() {
                s.solicitud = s.solicitud.wrapping_add(1);
                s.etapa = Etapa::Selector;
                s.error = None;
            }
        }
        v::Accion::AgregarServidor => {
            return crate::servidores_reales::actualizar(
                app,
                crate::servidores_reales::Accion::Nuevo,
            )
        }
        v::Accion::AceptarInvitacion => {
            s.etapa = Etapa::Entrada;
        }
        v::Accion::Codigo(c) => s.codigo_entrada = c,
        v::Accion::Cerrar | v::Accion::Cancelar => {
            if s.etapa == Etapa::Propios {
                s.etapa = Etapa::Revision;
            } else if s.etapa == Etapa::Revision {
                return guardar(s, true);
            } else {
                s.cerrar();
            }
        }
        v::Accion::ConfirmarCancelarPublicacion => return guardar(s, true),
        v::Accion::CerrarSinPublicar => s.cerrar(),
        v::Accion::SeguirRevisando => {
            if let Some(r) = &mut s.revision {
                r.confirmar_cancelacion = false;
            }
        }
        v::Accion::EleccionPublicacion(ruta, e) if s.etapa == Etapa::Revision => {
            if let Some(t) = &mut s.trabajo {
                if let Err(e) = t.elegir(&ruta, modelos::desde_vista(e)) {
                    s.error = Some(e);
                }
                s.revision = Some(modelos::revision(t));
            }
            return guardar(s, false);
        }
        v::Accion::RevisarPropios if s.etapa == Etapa::Revision => {
            s.permiso_propios = false;
            s.etapa = Etapa::Propios;
        }
        v::Accion::PermisoPropios(p) if s.etapa == Etapa::Propios => s.permiso_propios = p,
        v::Accion::AutorizarPropios if s.etapa == Etapa::Propios && s.permiso_propios => {
            if let Some(t) = &mut s.trabajo {
                t.borrador.autorizar_propios();
                s.revision = Some(modelos::revision(t));
            }
            s.etapa = Etapa::Revision;
        }
        v::Accion::ExcluirPropios if s.etapa == Etapa::Propios => {
            if let Some(t) = &mut s.trabajo {
                for f in t.borrador.autorizacion_pendiente() {
                    let _ = t.elegir(f.ruta.as_str(), Eleccion::Privado);
                }
                s.revision = Some(modelos::revision(t));
            }
            s.etapa = Etapa::Revision;
            return guardar(s, false);
        }
        v::Accion::PublicarRevision | v::Accion::Compartir if s.etapa == Etapa::Revision => {
            let Some(t) = s.trabajo.clone() else {
                return Task::none();
            };
            if !t.borrador.autorizacion_pendiente().is_empty() {
                s.permiso_propios = false;
                s.etapa = Etapa::Propios;
                return Task::none();
            }
            if app.servicios.juego.ocupada(&t.instancia.id) {
                s.error = Some("La instancia está en uso.".into());
                return Task::none();
            }
            s.etapa = Etapa::Publicando;
            s.cancel = tokio_util::sync::CancellationToken::new();
            if let Some(r) = &mut s.revision {
                r.estado = v::EstadoRevisionPublicacion::Preparando { porcentaje: 0. };
            }
            let (id, h, cancel) = (s.solicitud, s.anfitrion.clone(), s.cancel.clone());
            return Task::perform(tareas::publicar(t, h, cancel), move |r| {
                m(Accion::Publicada(id, r))
            });
        }
        v::Accion::VerPublicacion => {
            s.solicitud = s.solicitud.wrapping_add(1);
            s.etapa = Etapa::Panel;
            if let Some(h) = s.anfitrion.clone() {
                let id = s.solicitud;
                s.red_ocupada = true;
                return Task::perform(
                    async move { h.accesos().await.map_err(|e| e.to_string()) },
                    move |r| m(Accion::Accesos(id, r)),
                );
            }
        }
        v::Accion::GenerarInvitacion => return invitar(s),
        v::Accion::Caducidad(c) => return super::invitaciones::ajustar(s, c, s.cupo),
        v::Accion::Cupo(c) if c > 0 => return super::invitaciones::ajustar(s, s.caducidad, c),
        v::Accion::VolverACompartir => return super::invitaciones::reanudar(s),
        v::Accion::CopiarCodigo => {
            if let Some(i) = s.panel.as_mut().and_then(|p| p.invitacion.as_mut()) {
                i.copiado = true;
                return iced::clipboard::write(i.codigo.clone());
            }
        }
        v::Accion::CopiarHuella => {
            if let Some(p) = &s.panel {
                return iced::clipboard::write(p.huella.clone());
            }
        }
        v::Accion::RevocarCodigo => return operar(s, Operacion::Codigo),
        v::Accion::ConfirmarRevocacion(k) => {
            if let Some(p) = &mut s.panel {
                p.accesos.confirmar = Some(k);
            }
        }
        v::Accion::CancelarRevocacion => {
            if let Some(p) = &mut s.panel {
                p.accesos.confirmar = None;
            }
        }
        v::Accion::RevocarAcceso(k) => return operar(s, Operacion::Acceso(k)),
        v::Accion::DejarDeCompartir => {
            if s.anfitrion.as_ref().is_some_and(|h| h.transferencias() > 0) {
                s.etapa = Etapa::Cierre;
            } else {
                return operar(s, Operacion::Detener);
            }
        }
        v::Accion::OpcionCierre(o) => s.opcion_cierre = o,
        v::Accion::ConfirmarCierre if s.etapa == Etapa::Cierre => {
            return operar(s, Operacion::Detener)
        }
        _ => (),
    }
    Task::none()
}
fn guardar(s: &mut Estado, confirmar: bool) -> Task<Mensaje> {
    let Some(t) = s.trabajo.clone() else {
        s.cerrar();
        return Task::none();
    };
    if s.etapa != Etapa::Revision {
        return Task::none();
    }
    s.etapa = Etapa::Guardando;
    let id = s.solicitud;
    if let Some(r) = &mut s.revision {
        r.estado = v::EstadoRevisionPublicacion::Preparando { porcentaje: 0. };
    }
    Task::perform(tareas::guardar_seleccion(t), move |r| {
        m(Accion::Guardada(id, confirmar, r))
    })
}
