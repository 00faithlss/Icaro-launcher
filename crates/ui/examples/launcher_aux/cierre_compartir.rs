// SPDX-License-Identifier: GPL-3.0-only
//! Cierre ordenado; IO y bandeja fuera de iced.
use crate::{bandeja, compartir_real, App, Mensaje};
use icaro_ui::compartir as v;
use iced::{window, Element, Task};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
#[derive(Default)]
pub struct Estado {
    pub solicitado: bool,
    pub esperando: bool,
    pub aprobado: bool,
    pub host_cerrado: bool,
    pub oculta: bool,
    pub transferencias: usize,
    pub bandeja: Option<Arc<bandeja::Bandeja>>,
    /// Cerrar con Minecraft abierto: Ícaro espera en la bandeja a que termine.
    pub por_juego: bool,
    /// «Salir» desde la bandeja o la partida terminó: cerrar sin volver a esperar.
    pub forzar: bool,
    iniciando: bool,
    cerrando_host: bool,
    ahora: CancellationToken,
}
impl Estado {
    pub fn puede_terminar(&self, ocupado: bool) -> bool {
        self.esperando && self.host_cerrado && !ocupado
    }
}
#[derive(Clone)]
pub enum Accion {
    Tic,
    Bandeja(Result<Arc<bandeja::Bandeja>, String>),
    Terminada(Result<(), String>),
}
impl std::fmt::Debug for Accion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("CierreCompartir")
            .field(&std::mem::discriminant(self))
            .finish()
    }
}
fn mensaje(a: Accion) -> Mensaje {
    Mensaje::CierreCompartir(a)
}
fn ocupado(app: &App) -> bool {
    let s = &app.servicios.compartir;
    s.red_ocupada
        || matches!(
            s.etapa,
            compartir_real::Etapa::Publicando | compartir_real::Etapa::Guardando
        )
}
pub fn solicitar(app: &mut App) -> Option<Task<Mensaje>> {
    if app.servicios.demo || !app.servicios.compartir.habilitado || app.cierre.aprobado {
        return None;
    }
    if app.servicios.compartir.anfitrion.is_none() && !ocupado(app) {
        return None;
    }
    app.cierre.transferencias = app
        .servicios
        .compartir
        .anfitrion
        .as_ref()
        .map_or(0, |h| h.transferencias());
    if app.cierre.transferencias == 0 {
        app.cierre.esperando = true;
        app.servicios.compartir.cancel.cancel();
        return Some(avanzar(app));
    }
    app.cierre.solicitado = true;
    Some(Task::none())
}
/// Cerrar con una partida abierta no la corta: Ícaro se oculta en la bandeja
/// y termina de cerrarse cuando Minecraft se cierra. `None` si no hay partida.
pub fn ocultar_por_juego(app: &mut App) -> Option<Task<Mensaje>> {
    if app.servicios.demo || app.cierre.forzar || app.servicios.juego.procesos.is_empty() {
        return None;
    }
    app.cierre.por_juego = true;
    if app.cierre.bandeja.is_some() {
        return Some(ocultar(app));
    }
    if app.cierre.iniciando {
        return Some(Task::none());
    }
    app.cierre.iniciando = true;
    Some(Task::perform(
        async {
            tokio::task::spawn_blocking(bandeja::Bandeja::iniciar)
                .await
                .map_err(|_| "No se pudo iniciar la bandeja.".to_string())?
                .map(Arc::new)
        },
        |r| mensaje(Accion::Bandeja(r)),
    ))
}

/// En la bandeja por una partida: al cerrarse Minecraft, termina el cierre.
fn seguir_por_juego(app: &mut App) -> Option<Task<Mensaje>> {
    if app.cierre.por_juego && app.servicios.juego.procesos.is_empty() {
        app.cierre.por_juego = false;
        app.cierre.forzar = true;
        return Some(Task::done(Mensaje::Cerrar));
    }
    None
}

pub fn vista(app: &App) -> Option<Element<'_, Mensaje>> {
    if !app.cierre.solicitado {
        return None;
    }
    let s = &app.servicios.compartir;
    let modelo = v::CierreTransferencias {
        cerrar_icaro: true,
        instancia: s.panel.as_ref().map_or_else(
            || "Operación en curso".into(),
            |p| p.instancia.nombre.clone(),
        ),
        transferencias: if s.anfitrion.is_some() {
            compartir_real::transferencias(s)
        } else {
            (0..app.cierre.transferencias)
                .map(|n| v::Transferencia {
                    nombre: format!("Transferencia {}", n + 1),
                    fase: "En curso".into(),
                    porcentaje: 0.,
                })
                .collect()
        },
        accesos: s.panel.as_ref().map_or(0, |p| p.accesos.personas.len()),
        opcion: s.opcion_cierre,
    };
    Some(
        iced::widget::container(
            v::cierre_transferencias(app.paleta(), &modelo)
                .map(|a| Mensaje::Compartir(compartir_real::Accion::Vista(a))),
        )
        .width(640f32.min((app.ventana.width - 48.).max(240.)))
        .height((app.ventana.height - 96.).clamp(240., 800.))
        .into(),
    )
}
pub fn filtrar(app: &mut App, m: &Mensaje) -> Option<Task<Mensaje>> {
    if !app.cierre.solicitado && !app.cierre.esperando {
        return None;
    }
    match m {
        Mensaje::Ir(_) | Mensaje::Jugar(_) => Some(Task::none()),
        Mensaje::CerrarDialogo => Some(cancelar(app)),
        Mensaje::Compartir(compartir_real::Accion::Vista(a)) => Some(match a {
            v::Accion::Cancelar | v::Accion::Cerrar | v::Accion::Atras => cancelar(app),
            v::Accion::OpcionCierre(o) => {
                app.servicios.compartir.opcion_cierre = *o;
                Task::none()
            }
            v::Accion::ConfirmarCierre => confirmar(app),
            _ => Task::none(),
        }),
        _ => None,
    }
}
fn cancelar(app: &mut App) -> Task<Mensaje> {
    if !app.cierre.esperando {
        app.cierre.solicitado = false;
    }
    Task::none()
}
fn confirmar(app: &mut App) -> Task<Mensaje> {
    app.cierre.esperando = true;
    app.servicios.compartir.cancel.cancel();
    if app.servicios.compartir.opcion_cierre == v::OpcionCierre::Ahora {
        app.cierre.ahora.cancel();
    }
    let mut tareas = vec![avanzar(app)];
    if app.servicios.compartir.opcion_cierre == v::OpcionCierre::Esperar {
        if app.cierre.bandeja.is_some() {
            tareas.push(ocultar(app));
        } else if !app.cierre.iniciando && !app.cierre.aprobado {
            app.cierre.iniciando = true;
            tareas.push(Task::perform(
                async {
                    tokio::task::spawn_blocking(bandeja::Bandeja::iniciar)
                        .await
                        .map_err(|_| "No se pudo iniciar la bandeja.".to_string())?
                        .map(Arc::new)
                },
                |r| mensaje(Accion::Bandeja(r)),
            ));
        }
    }
    Task::batch(tareas)
}
fn ocultar(app: &mut App) -> Task<Mensaje> {
    if app.cierre.bandeja.is_none() {
        return window::get_oldest().and_then(|id| window::minimize(id, true));
    }
    app.cierre.oculta = true;
    window::get_oldest().and_then(|id| window::change_mode(id, window::Mode::Hidden))
}
fn mostrar(app: &mut App) -> Task<Mensaje> {
    app.cierre.oculta = false;
    window::get_oldest().and_then(|id| {
        Task::batch([
            window::change_mode(id, window::Mode::Windowed),
            window::minimize(id, false),
            window::gain_focus(id),
        ])
    })
}
fn avanzar(app: &mut App) -> Task<Mensaje> {
    if app.cierre.puede_terminar(ocupado(app)) {
        app.cierre.aprobado = true;
        app.cierre.solicitado = false;
        return Task::done(Mensaje::Cerrar);
    }
    if !app.cierre.esperando || ocupado(app) || app.cierre.cerrando_host {
        return Task::none();
    }
    let Some(h) = app.servicios.compartir.anfitrion.clone() else {
        app.cierre.host_cerrado = true;
        return Task::done(mensaje(Accion::Tic));
    };
    app.cierre.cerrando_host = true;
    let cancel = app.cierre.ahora.clone();
    Task::perform(
        async move {
            let resultado = tokio::select! { _ = cancel.cancelled() => Ok(()), r = h.drenar() => r.map_err(icaro_core::diagnostico_contexto::texto) };
            h.cerrar().await;
            resultado
        },
        |r| mensaje(Accion::Terminada(r)),
    )
}
pub fn actualizar(app: &mut App, a: Accion) -> Task<Mensaje> {
    match a {
        Accion::Tic => {
            match app.cierre.bandeja.as_ref().and_then(|b| b.siguiente()) {
                Some(bandeja::Evento::Mostrar) => {
                    // Volver a Ícaro cancela el cierre pendiente por la partida.
                    app.cierre.por_juego = false;
                    return mostrar(app);
                }
                Some(bandeja::Evento::NoDisponible) => {
                    app.cierre.bandeja = None;
                    app.cierre.por_juego = false;
                    app.avisar("La bandeja dejó de estar disponible; Ícaro sigue visible mientras termina.".into());
                    return mostrar(app);
                }
                Some(bandeja::Evento::Salir) if app.cierre.por_juego => {
                    // Salir pedido a mano: se cierra también Minecraft.
                    app.cierre.por_juego = false;
                    app.cierre.forzar = true;
                    return Task::done(Mensaje::Cerrar);
                }
                Some(bandeja::Evento::Salir) => {
                    app.cierre.ahora.cancel();
                    return confirmar(app);
                }
                None => (),
            }
            if let Some(tarea) = seguir_por_juego(app) {
                return tarea;
            }
        }
        Accion::Bandeja(r) => {
            app.cierre.iniciando = false;
            match r {
                Ok(b) => {
                    app.cierre.bandeja = Some(b);
                    if (app.cierre.esperando && !app.cierre.aprobado) || app.cierre.por_juego {
                        return ocultar(app);
                    }
                }
                Err(e) => {
                    // Sin bandeja se minimiza; la partida sigue igual. No se cierra
                    // solo después: la ventana sigue a mano en la barra de tareas.
                    app.cierre.por_juego = false;
                    app.avisar(e);
                    return window::get_oldest().and_then(|id| window::minimize(id, true));
                }
            }
        }
        Accion::Terminada(r) => {
            if let Err(e) = r {
                app.avisar(e);
            }
            app.cierre.host_cerrado = true;
            app.servicios.compartir.anfitrion = None;
        }
    }
    avanzar(app)
}
#[cfg(test)]
#[path = "cierre_compartir_pruebas.rs"]
mod pruebas;
