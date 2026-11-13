// SPDX-License-Identifier: GPL-3.0-only
//! Comprobación previa a Jugar y sincronización explícita de un vínculo existente.
use super::{servicio::Servicio, sync_tareas as tareas, Accion, Estado as Compartir, Etapa};
use crate::{App, Mensaje};
use icaro_core::instancias::Instancia;
use icaro_ui::compartir as v;
use iced::Task;
use std::{collections::BTreeMap, sync::Arc};

#[derive(Default)]
pub struct Estado {
    pub instancia: Option<Instancia>,
    pub revision: Option<Arc<tareas::Revision>>,
    pub jugar: bool,
    pub servidor: Option<String>,
    pub error: Option<String>,
    pub detalle: bool,
    pub elecciones: BTreeMap<String, v::Resolucion>,
    pub completada: bool,
}
pub fn antes_de_jugar(
    app: &mut App,
    index: usize,
    jugar: bool,
    servidor: Option<String>,
) -> Option<Task<Mensaje>> {
    if app.servicios.demo || !app.servicios.compartir.habilitado || !jugar {
        return None;
    }
    let i = app.servicios.instancias.registros.get(index)?;
    if app.servicios.juego.procesos.contains_key(&i.id) {
        // Conserva el botón Detener de una partida que ya está ejecutándose.
        return None;
    }
    if app.servicios.juego.ocupada(&i.id) {
        // No convertir una instalación sin revisión en un lanzamiento automático.
        app.avisar("Espera a que termine la instalación y vuelve a pulsar Jugar para comprobar la publicación.".into());
        return Some(Task::none());
    }
    Some(abrir(app, index, true, servidor))
}
pub fn abrir(app: &mut App, index: usize, jugar: bool, servidor: Option<String>) -> Task<Mensaje> {
    if app.servicios.demo || !app.servicios.compartir.habilitado {
        return Task::none();
    }
    // Ver una publicación no impide jugar: la vista se cierra (la publicación
    // sigue activa). Otro flujo a medias se avisa en vez de no hacer nada.
    if app.servicios.compartir.etapa == Etapa::Panel && !app.servicios.compartir.red_ocupada {
        app.servicios.compartir.cerrar();
    }
    if app.servicios.compartir.modal() {
        app.avisar("Termina o cierra lo que tienes abierto en Con amigos antes de jugar.".into());
        return Task::none();
    }
    let Some(i) = app.servicios.instancias.registros.get(index).cloned() else {
        return Task::none();
    };
    if app.servicios.juego.ocupada(&i.id)
        || app.servicios.mods.busy.as_ref() == Some(&i.id)
        || app.servicios.instancias.operando
    {
        app.avisar("Termina la actividad de la instancia antes de revisar su publicación.".into());
        return Task::none();
    }
    app.menu = None;
    let s = &mut app.servicios.compartir;
    s.cancel.cancel();
    s.cancel = tokio_util::sync::CancellationToken::new();
    s.solicitud = s.solicitud.wrapping_add(1);
    s.sincronizar = Estado {
        instancia: Some(i.clone()),
        jugar,
        servidor,
        ..Default::default()
    };
    s.etapa = Etapa::ConsultandoVinculo;
    s.red_ocupada = true;
    s.cerrar_al_terminar = false;
    s.invitado.avance = Default::default();
    let (id, cancel, h) = (s.solicitud, s.cancel.clone(), s.anfitrion.clone());
    Task::perform(
        tareas::consultar(
            crate::estado::directorio(false),
            app.servicios.instancias.almacen.clone(),
            i,
            Servicio::desde_entorno(),
            h,
            cancel,
        ),
        move |r| Mensaje::Compartir(Accion::VinculoConsultado(id, r.map(|r| r.map(Arc::new)))),
    )
}
pub fn error(s: &mut Compartir, id: u64, e: String) {
    if !s.vigente(id) {
        return;
    }
    s.sincronizar.error = Some(e);
    s.etapa = Etapa::ErrorVinculo;
    s.red_ocupada = false;
}
pub fn recibida(
    app: &mut App,
    id: u64,
    r: Result<Option<Arc<tareas::Revision>>, String>,
) -> Task<Mensaje> {
    let s = &mut app.servicios.compartir;
    if !s.vigente(id) || s.etapa != Etapa::ConsultandoVinculo {
        return Task::none();
    }
    s.red_ocupada = false;
    if s.cerrar_al_terminar {
        s.cerrar_al_terminar = false;
        s.cerrar();
        return Task::none();
    }
    match r {
        Err(e) => error(s, id, e),
        Ok(None) => {
            if s.sincronizar.jugar {
                return continuar_juego(app, None);
            }
            s.cerrar();
            app.avisar("Esta instancia no está vinculada a una publicación compartida.".into());
        }
        Ok(Some(revision)) => {
            let jugar = s.sincronizar.jugar && revision.sin_cambios();
            let actividad = revision.actividad.clone();
            s.sincronizar.revision = Some(revision);
            s.etapa = Etapa::RevisionVinculo;
            if jugar {
                return continuar_juego(app, Some(actividad));
            }
        }
    }
    Task::none()
}
fn continuar_juego(
    app: &mut App,
    actividad: Option<Arc<icaro_core::actividad_instancia::Guardia>>,
) -> Task<Mensaje> {
    let s = &mut app.servicios.compartir;
    let id = s.sincronizar.instancia.as_ref().map(|i| i.id.clone());
    let servidor = s.sincronizar.servidor.clone();
    s.cerrar();
    let Some(index) = id.and_then(|id| {
        app.servicios
            .instancias
            .registros
            .iter()
            .position(|i| i.id == id)
    }) else {
        return Task::none();
    };
    crate::juego_real::iniciar_revisado(app, index, true, servidor, actividad)
}
pub fn accion(app: &mut App, a: v::Accion) -> Option<Task<Mensaje>> {
    let s = &mut app.servicios.compartir;
    if !matches!(
        s.etapa,
        Etapa::ConsultandoVinculo
            | Etapa::RevisionVinculo
            | Etapa::AplicandoVinculo
            | Etapa::ErrorVinculo
    ) {
        return None;
    }
    let task = match a {
        v::Accion::Reintentar | v::Accion::RevisarPublicacion => {
            let i = s.sincronizar.instancia.as_ref()?.id.clone();
            let jugar = s.sincronizar.jugar;
            let servidor = s.sincronizar.servidor.clone();
            s.cerrar();
            let index = app
                .servicios
                .instancias
                .registros
                .iter()
                .position(|v| v.id == i)?;
            abrir(app, index, jugar, servidor)
        }
        v::Accion::ResolverConflicto(r, e) if s.etapa == Etapa::RevisionVinculo => {
            s.sincronizar.elecciones.insert(r, e);
            Task::none()
        }
        v::Accion::Sincronizar if s.etapa == Etapa::RevisionVinculo => aplicar(app),
        v::Accion::Jugar
            if s.etapa == Etapa::RevisionVinculo
                && (s.sincronizar.completada
                    || s.sincronizar
                        .revision
                        .as_ref()
                        .is_some_and(|r| r.sin_cambios())) =>
        {
            let id = s.sincronizar.instancia.as_ref()?.id.clone();
            let servidor = s.sincronizar.servidor.clone();
            s.cerrar();
            let index = app
                .servicios
                .instancias
                .registros
                .iter()
                .position(|i| i.id == id)?;
            abrir(app, index, true, servidor)
        }
        v::Accion::VerDetalle => {
            s.sincronizar.detalle = true;
            Task::none()
        }
        v::Accion::Atras | v::Accion::Cerrar | v::Accion::Cancelar => {
            s.cerrar();
            Task::none()
        }
        _ => Task::none(),
    };
    Some(task)
}
fn aplicar(app: &mut App) -> Task<Mensaje> {
    let s = &mut app.servicios.compartir;
    let Some(original) = s.sincronizar.revision.as_ref() else {
        return Task::none();
    };
    let mut revision = (**original).clone();
    for (ruta, e) in &s.sincronizar.elecciones {
        if ruta == "@resourcepacks" {
            if let Some(orden) = &mut revision.plan.orden_packs {
                let r = match e {
                    v::Resolucion::Anfitrion => {
                        icaro_core::compartir::plan::Resolucion::DelAnfitrion
                    }
                    _ => icaro_core::compartir::plan::Resolucion::ConservarMia,
                };
                if let Err(e) = orden.resolver(r) {
                    s.sincronizar.error = Some(e.to_string());
                    return Task::none();
                }
            }
            continue;
        }
        let Ok(ruta) = icaro_core::compartir::RutaCompartida::parse(ruta) else {
            continue;
        };
        let e = match e {
            v::Resolucion::Anfitrion => icaro_core::compartir::plan::Resolucion::DelAnfitrion,
            _ => icaro_core::compartir::plan::Resolucion::ConservarMia,
        };
        if let Err(e) = revision.plan.resolver(&ruta, e) {
            s.sincronizar.error = Some(e.to_string());
            return Task::none();
        }
    }
    if !revision.plan.puede_aplicar() {
        return Task::none();
    }
    s.etapa = Etapa::AplicandoVinculo;
    s.red_ocupada = true;
    s.sincronizar.error = None;
    s.invitado.avance = Default::default();
    let (id, cancel, h) = (s.solicitud, s.cancel.clone(), s.anfitrion.clone());
    let raiz = crate::estado::directorio(false);
    let catalogo = app.servicios.catalog.as_ref().ok().cloned();
    super::avance::tarea_con_resultado(
        id,
        move |emitir| tareas::aplicar(raiz, Arc::new(revision), catalogo, h, cancel, emitir),
        Accion::VinculoAplicado,
    )
}
pub fn aplicada(app: &mut App, id: u64, r: Result<(), String>) -> Task<Mensaje> {
    let s = &mut app.servicios.compartir;
    if !s.vigente(id) || s.etapa != Etapa::AplicandoVinculo {
        return Task::none();
    }
    s.red_ocupada = false;
    match r {
        Ok(()) => {
            s.sincronizar.completada = true;
            s.etapa = Etapa::RevisionVinculo;
        }
        Err(e) => error(s, id, e),
    }
    if s.cerrar_al_terminar {
        s.cerrar_al_terminar = false;
        s.cerrar();
    }
    Task::none()
}
