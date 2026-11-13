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
    ActivarExperimental,
    /// Una publicación en pausa se quitó de la biblioteca.
    Quitada(Result<(), String>),
    Biblioteca(u64, Result<Vec<c::biblioteca::PublicacionGuardada>, String>),
    Recuperada(u64, Result<super::biblioteca_tareas::Recuperada, String>),
    Reconectada(u64, Result<super::biblioteca_tareas::Reconectada, String>),
    Contadores(u64, u64, super::contadores::Contadores),
    RevisarVinculo(usize),
    VinculoConsultado(
        u64,
        Result<Option<std::sync::Arc<super::sync_tareas::Revision>>, String>,
    ),
    VinculoAplicado(u64, Result<(), String>),
    Avance(u64, super::avance::Avance),
    Resuelta(
        u64,
        Result<super::invitado_tareas::Resuelta, super::invitado_tareas::ErrorEntrada>,
    ),
    Visitada(u64, Result<super::invitado_tareas::Visita, String>),
    Confiada(
        u64,
        Result<Option<icaro_core::instancias::Instancia>, String>,
    ),
    Instalada(u64, super::invitado_instalacion::Resultado),
    Vista(v::Accion),
    Recibidas(u64, Vec<super::compartidas::Recibida>),
    DatosSelector(u64, Vec<(String, super::selector::DatosOpcion)>),
    AnfitrionRecibido(u64, String, bool),
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
    let Some((id, futuro)) = iniciar_invitacion(s) else {
        return Task::none();
    };
    Task::perform(futuro, move |r| m(Accion::Invitacion(id, r)))
}
/// Marca la invitación en curso y devuelve el trabajo de red sin envolverlo en un Task.
pub fn iniciar_invitacion(
    s: &mut Estado,
) -> Option<(
    u64,
    impl std::future::Future<Output = Result<InvitacionCreada, String>> + Send + 'static,
)> {
    if s.red_ocupada {
        return None;
    }
    let h = s.anfitrion.clone()?;
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
    Some((id, async move {
        h.invitar(cad, cupo).await.map_err(|e| e.to_string())
    }))
}
/// Anula el código vigente; CompartidoListo lo usa para cambiarlo.
pub(crate) fn revocar_codigo(s: &mut Estado) -> Task<Mensaje> {
    operar(s, Operacion::Codigo)
}
fn operar(s: &mut Estado, op: Operacion) -> Task<Mensaje> {
    if s.red_ocupada {
        return Task::none();
    }
    let Some(h) = s.anfitrion.clone() else {
        return Task::none();
    };
    s.biblioteca.cancel_contadores.cancel();
    s.biblioteca.contadores = s.biblioteca.contadores.wrapping_add(1);
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
    if app.servicios.demo
        || (!app.servicios.compartir.habilitado && !matches!(a, Accion::ActivarExperimental))
    {
        return Task::none();
    }
    match a {
        Accion::ActivarExperimental => {
            app.servicios.compartir.activado_por_usuario = true;
            app.servicios.compartir.habilitado = true;
            return super::biblioteca::cargar(app);
        }
        Accion::Quitada(r) => {
            match r {
                Ok(()) => app.avisar("Se quitó de tus instancias compartidas.".into()),
                Err(e) => app.avisar(format!("No se pudo quitar: {e}")),
            }
            return super::biblioteca::cargar(app);
        }
        Accion::Biblioteca(generacion, r) => {
            super::biblioteca::recibida(&mut app.servicios.compartir, generacion, r)
        }
        Accion::Recuperada(id, r) => {
            if super::biblioteca::recuperada(&mut app.servicios.compartir, id, r) {
                return super::biblioteca::reconectar(&mut app.servicios.compartir);
            }
        }
        Accion::Reconectada(id, r) => {
            super::biblioteca::reconectada(&mut app.servicios.compartir, id, r);
            return super::contadores::pedir(&mut app.servicios.compartir)
                .unwrap_or_else(Task::none);
        }
        Accion::Contadores(id, generacion, r) => {
            let s = &mut app.servicios.compartir;
            if !s.vigente(id) || generacion != s.biblioteca.contadores {
                return Task::none();
            }
            super::contadores::recibidos(s, id, generacion, r);
            return super::contadores::pedir(s).unwrap_or_else(Task::none);
        }
        Accion::RevisarVinculo(index) => return super::sincronizar::abrir(app, index, false, None),
        Accion::VinculoConsultado(id, r) => return super::sincronizar::recibida(app, id, r),
        Accion::VinculoAplicado(id, r) => return super::sincronizar::aplicada(app, id, r),
        Accion::Avance(id, avance) => {
            super::avance::recibido(&mut app.servicios.compartir, id, avance)
        }
        Accion::Instalada(id, r) => super::invitado::instalada(app, id, r),
        Accion::Resuelta(id, r) => {
            super::invitado::resuelta(&mut app.servicios.compartir, id, r);
            // Con amigos valida solo: un código bueno sigue a consultar al anfitrión.
            let s = &app.servicios.compartir;
            if s.invitado.auto && s.etapa == Etapa::Entrada && s.invitado.resuelta.is_some() {
                return actualizar(app, Accion::Vista(v::Accion::Continuar));
            }
        }
        Accion::Visitada(id, r) => super::invitado::visitada(&mut app.servicios.compartir, id, r),
        Accion::Confiada(id, r) => {
            super::invitado::confiada(&mut app.servicios.compartir, id, r);
            // «Unirme y descargar» sigue directo a revisar la instalación.
            let s = &app.servicios.compartir;
            if s.invitado.auto && s.etapa == Etapa::Destino && s.invitado.error.is_none() {
                return actualizar(app, Accion::Vista(v::Accion::RevisarInstalacion));
            }
        }
        Accion::Recibidas(generacion, lista) => {
            return super::compartidas::recibidas(&mut app.servicios.compartir, generacion, lista)
        }
        Accion::DatosSelector(generacion, datos) => {
            super::selector::recibidos(app, generacion, datos)
        }
        Accion::AnfitrionRecibido(generacion, id, ok) => {
            super::compartidas::anfitrion(&mut app.servicios.compartir, generacion, &id, ok)
        }
        Accion::Analizada(id, r) => resultados::analizada(&mut app.servicios.compartir, id, r),
        Accion::Publicada(id, r) => {
            if resultados::publicada(&mut app.servicios.compartir, id, r) {
                let invitacion = invitar(&mut app.servicios.compartir);
                return Task::batch([invitacion, super::biblioteca::cargar(app)]);
            }
            return super::biblioteca::cargar(app);
        }
        Accion::Invitacion(id, r) => {
            resultados::invitacion(&mut app.servicios.compartir, id, r);
            return super::contadores::pedir(&mut app.servicios.compartir)
                .unwrap_or_else(Task::none);
        }
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
            let contadores =
                super::contadores::pedir(&mut app.servicios.compartir).unwrap_or_else(Task::none);
            return Task::batch([contadores, super::biblioteca::cargar(app)]);
        }
        Accion::Operada(id, op, r) => {
            // Cambiar código: tras anular el actual se genera otro.
            let regenerar = matches!(op, Operacion::Codigo)
                && r.is_ok()
                && std::mem::take(&mut app.servicios.compartir.listo.regenerar);
            resultados::operada(&mut app.servicios.compartir, id, op, r);
            let nuevo = if regenerar {
                app.servicios.compartir.listo.codigo_nuevo = true;
                invitar(&mut app.servicios.compartir)
            } else {
                Task::none()
            };
            let contadores =
                super::contadores::pedir(&mut app.servicios.compartir).unwrap_or_else(Task::none);
            return Task::batch([nuevo, contadores, super::biblioteca::cargar(app)]);
        }
        Accion::Guardada(id, confirmar, r) => {
            let s = &mut app.servicios.compartir;
            if !s.vigente(id) || s.etapa != Etapa::Guardando {
                return Task::none();
            }
            s.etapa = Etapa::Revision;
            if let Some(v) = &mut s.revision {
                v.guardando = false;
                match r {
                    Ok(()) => v.confirmar_cancelacion = confirmar,
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
    if let Some(task) = super::compartidas::accion(app, &a) {
        return task;
    }
    if let Some(task) = super::selector::accion(app, &a) {
        return task;
    }
    if let Some(task) = super::listo::accion(&mut app.servicios.compartir, &a) {
        return task;
    }
    if let Some(task) = super::biblioteca::accion(app, &a) {
        return task;
    }
    if let Some(task) = revision_local(&mut app.servicios.compartir, &a) {
        return task;
    }
    if !app.servicios.compartir.red_ocupada {
        if let Some(task) = super::sincronizar::accion(app, a.clone()) {
            return task;
        }
    }
    if let v::Accion::ElegirInstancia(id) = a {
        return analizar(app, id);
    }
    if matches!(a, v::Accion::RevisarPublicacion | v::Accion::Reintentar)
        && !matches!(
            app.servicios.compartir.etapa,
            Etapa::Entrada
                | Etapa::Identidad
                | Etapa::Destino
                | Etapa::RevisionInvitado
                | Etapa::ResultadoInvitado
                | Etapa::InstalandoInvitado
        )
    {
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
    if let Some(task) = super::invitado::accion(app, a.clone()) {
        return task;
    }
    let s = &mut app.servicios.compartir;
    match a {
        v::Accion::AbrirCompartir => {
            if !s.modal() {
                s.solicitud = s.solicitud.wrapping_add(1);
                s.etapa = Etapa::Selector;
                s.error = None;
                s.seleccionada = None;
            }
            let biblioteca = super::biblioteca::cargar(app);
            return Task::batch([biblioteca, super::selector::cargar(app)]);
        }
        v::Accion::AgregarServidor => {
            return crate::servidores_reales::actualizar(
                app,
                crate::servidores_reales::Accion::Nuevo,
            )
        }
        v::Accion::Cerrar | v::Accion::Cancelar => {
            if s.etapa == Etapa::Propios {
                s.etapa = Etapa::Revision;
            } else if s.etapa == Etapa::Revision
                && s.revision.as_ref().is_some_and(|r| {
                    r.vista.sucio && !r.confirmar_cancelacion && puede_elegir(r)
                })
            {
                // Con la selección cambiada se confirma en el mismo pie.
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
            return elegir(s, vec![(ruta, e)], None);
        }
        v::Accion::MarcarPublicacion(id, marcado) if s.etapa == Etapa::Revision => {
            let Some(r) = s.revision.as_ref().filter(|r| puede_elegir(r)) else {
                return Task::none();
            };
            let Some(c) = r.cambios.iter().find(|c| c.archivo.id == id) else {
                return Task::none();
            };
            let mut elecciones = vec![(id.clone(), c.eleccion_para(marcado))];
            // Las configuraciones siguen a su mod: publicar o dejar privado el mod hace lo mismo con sus ajustes.
            if r.vista.siguen_mod && c.tipo == v::TipoContenido::Mod {
                elecciones.extend(
                    r.cambios
                        .iter()
                        .filter(|x| x.de_mod.as_ref().is_some_and(|(m, _)| *m == id))
                        .map(|x| (x.archivo.id.clone(), x.eleccion_para(marcado))),
                );
            }
            return elegir(s, elecciones, None);
        }
        v::Accion::MarcarTipo(tipo, marcado) if s.etapa == Etapa::Revision => {
            let Some(r) = s.revision.as_ref().filter(|r| puede_elegir(r)) else {
                return Task::none();
            };
            let sin = matches!(r.estado, v::EstadoRevisionPublicacion::SinCambios);
            let elecciones = r
                .cambios
                .iter()
                .filter(|c| {
                    c.tipo == tipo
                        && !c.integrado
                        && (!sin || c.grupo == v::GrupoPublicacion::Privado)
                })
                .map(|c| (c.archivo.id.clone(), c.eleccion_para(marcado)))
                .collect();
            return elegir(s, elecciones, None);
        }
        v::Accion::AtajoPublicacion(atajo) if s.etapa == Etapa::Revision => {
            let Some(r) = s.revision.as_ref().filter(|r| puede_elegir(r)) else {
                return Task::none();
            };
            let (elecciones, fuera) = v::elecciones_de_atajo(r, atajo);
            return elegir(s, elecciones, Some((atajo, fuera)));
        }
        v::Accion::RevisarPropios if s.etapa == Etapa::Revision => {
            s.permiso_propios = false;
            s.etapa = Etapa::Propios;
        }
        v::Accion::PermisoPropios(p) if s.etapa == Etapa::Propios => s.permiso_propios = p,
        v::Accion::AutorizarPropios if s.etapa == Etapa::Propios && s.permiso_propios => {
            if let Some(t) = &mut s.trabajo {
                t.borrador.autorizar_propios();
            }
            reconstruir(s);
            s.etapa = Etapa::Revision;
        }
        v::Accion::ExcluirPropios if s.etapa == Etapa::Propios => {
            if let Some(t) = &mut s.trabajo {
                for f in t.borrador.autorizacion_pendiente() {
                    let _ = t.elegir(f.ruta.as_str(), Eleccion::Privado);
                }
            }
            reconstruir(s);
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
                let accesos = Task::perform(
                    async move { h.accesos().await.map_err(|e| e.to_string()) },
                    move |r| m(Accion::Accesos(id, r)),
                );
                return Task::batch([
                    accesos,
                    super::contadores::pedir(s).unwrap_or_else(Task::none),
                ]);
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
fn puede_elegir(r: &v::RevisionPublicacion) -> bool {
    !r.confirmar_cancelacion
        && !r.guardando
        && matches!(
            r.estado,
            v::EstadoRevisionPublicacion::Disponible | v::EstadoRevisionPublicacion::SinCambios
        )
}

/// Recalcula la revisión desde el borrador sin perder filtros, búsqueda ni secciones abiertas.
fn reconstruir(s: &mut Estado) {
    let Some(t) = &s.trabajo else {
        return;
    };
    let mut nueva = modelos::revision(t);
    if let Some(anterior) = &s.revision {
        nueva.vista = anterior.vista.clone();
    }
    s.revision = Some(nueva);
}

/// Estado de la interfaz que no toca el borrador: se atiende también mientras se guarda.
fn revision_local(s: &mut Estado, a: &v::Accion) -> Option<Task<Mensaje>> {
    if !matches!(s.etapa, Etapa::Revision | Etapa::Guardando) {
        return None;
    }
    let vista = &mut s.revision.as_mut()?.vista;
    match a {
        v::Accion::FiltroPublicacion(f) => vista.filtro = *f,
        v::Accion::BuscarPublicacion(q) => vista.busqueda = q.chars().take(120).collect(),
        v::Accion::AlternarTipo(t) => {
            if !vista.abiertos.remove(t) {
                vista.abiertos.insert(*t);
            }
        }
        v::Accion::IrATipo(t) => {
            vista.abiertos.insert(*t);
            return Some(v::llevar_a_tipo(*t, Mensaje::Nada));
        }
        v::Accion::VerTodosPublicacion(t, g) => {
            vista.ver_todos.insert((*t, *g));
        }
        v::Accion::SiguenSuMod(b) => vista.siguen_mod = *b,
        _ => return None,
    }
    Some(Task::none())
}

/// Aplica varias elecciones a la vez sobre una copia: o entran todas o ninguna.
fn elegir(
    s: &mut Estado,
    elecciones: Vec<(String, v::Eleccion)>,
    atajo: Option<(v::AtajoPublicacion, Vec<String>)>,
) -> Task<Mensaje> {
    let Some(t) = &s.trabajo else {
        return Task::none();
    };
    let mut copia = t.clone();
    let resultado = elecciones.iter().try_for_each(|(id, e)| {
        if id == "@orden-resourcepacks" {
            copia.elegir(id, modelos::desde_vista(*e))
        } else {
            Ok(())
        }
    });
    let resultado = resultado.and_then(|()| {
        copia.elegir_varios(
            elecciones
                .iter()
                .filter(|(id, _)| id != "@orden-resourcepacks")
                .map(|(id, e)| (id.as_str(), modelos::desde_vista(*e))),
        )
    });
    match resultado {
        Ok(()) => s.trabajo = Some(copia),
        Err(e) => {
            s.error = Some(e);
            return Task::none();
        }
    }
    reconstruir(s);
    if let Some(r) = &mut s.revision {
        r.vista.sucio = true;
        r.vista.atajo = atajo.as_ref().map(|(a, _)| *a);
        if let Some((a, fuera)) = &atajo {
            r.vista.aplicado = v::mensaje_atajo(r, *a, fuera);
        }
    }
    guardar(s, false)
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
        r.guardando = true;
    }
    Task::perform(tareas::guardar_seleccion(t), move |r| {
        m(Accion::Guardada(id, confirmar, r))
    })
}
