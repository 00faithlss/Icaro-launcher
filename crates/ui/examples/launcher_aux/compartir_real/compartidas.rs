// SPDX-License-Identifier: GPL-3.0-only
//! Servidores › Instancias compartidas: lo que compartes, desde la biblioteca
//! de publicaciones, y lo que recibes, desde el vínculo de cada instancia.
use super::{servicio::Servicio, Accion, Estado, Etapa};
use crate::{App, Mensaje};
use icaro_core::{
    compartir::{
        self as c, aplicacion::ContextoDestino, biblioteca::EstadoGuardada, identidad::Identidad,
        Origen,
    },
    instancias::{Almacen, Instancia},
};
use icaro_ui::compartir as v;
use iced::{Element, Task};
use std::{path::PathBuf, time::Duration};

/// Una instancia recibida y el origen con el que se comprueba su anfitrión.
#[derive(Debug, Clone)]
pub struct Recibida {
    pub fila: v::InstanciaQueRecibes,
    pub origen: Origen,
}

fn m(a: Accion) -> Mensaje {
    Mensaje::Compartir(a)
}

/// «Equipo 9e2a 7b14»: los invitados aún no comparten su nombre.
pub fn equipo(clave: &str) -> String {
    let corta: String = clave.chars().take(8).collect();
    let (a, b) = corta.split_at(corta.len().min(4));
    format!("Equipo {a} {b}").trim_end().to_owned()
}

fn version(i: &Instancia) -> String {
    format!("{} · {}", i.minecraft, i.loader)
}

/// Modelo de la página con lo que hay en memoria; nunca lee disco ni red.
pub fn modelo(app: &App) -> v::InstanciasCompartidas {
    let s = &app.servicios.compartir;
    let registros = &app.servicios.instancias.registros;
    let activa = s
        .anfitrion
        .as_ref()
        .and(s.publicacion.as_ref())
        .map(|p| p.instancia_id.clone());
    let invitacion = s.panel.as_ref().and_then(|p| p.invitacion.as_ref());
    let compartes = s
        .biblioteca
        .entradas
        .iter()
        .filter(|e| {
            matches!(
                e.estado,
                EstadoGuardada::Lista | EstadoGuardada::Detenida | EstadoGuardada::PerfilCambiado
            )
        })
        .filter_map(|e| {
            let i = registros.iter().find(|i| i.id == e.instancia)?;
            let es_activa = activa.as_deref() == Some(i.id.as_str());
            let estado = if e.estado == EstadoGuardada::PerfilCambiado {
                v::EstadoCompartida::CambiosSinPublicar
            } else if es_activa {
                v::EstadoCompartida::Disponible
            } else {
                v::EstadoCompartida::EnPausa
            };
            // El código solo se conoce en la sesión que lo creó; en pausa no sirve.
            let vigente = invitacion
                .filter(|_| es_activa)
                .and_then(|inv| match &inv.estado {
                    v::EstadoInvitacion::Vigente { restante } => {
                        Some((inv.codigo.clone(), format!("caduca en {restante}")))
                    }
                    _ => None,
                });
            Some(v::InstanciaQueCompartes {
                id: i.id.clone(),
                nombre: i.nombre.clone(),
                estado,
                revision: e.revision?,
                personas: e.autorizados,
                version: version(i),
                copiado: s.copiado_de.as_deref() == Some(i.id.as_str()),
                codigo: vigente.as_ref().map(|v| v.0.clone()),
                caduca: vigente.map(|v| v.1).unwrap_or_default(),
            })
        })
        .collect();
    v::InstanciasCompartidas {
        pestana: s.compartidas.unwrap_or_default(),
        compartes,
        recibes: s.recibidas.iter().map(|r| r.fila.clone()).collect(),
        menu: s.menu_compartida.clone(),
    }
}

/// Página dentro de Servidores, solo cuando no hay un flujo de compartir abierto.
pub fn pagina(app: &App) -> Option<Element<'_, Mensaje>> {
    let s = &app.servicios.compartir;
    if s.etapa != Etapa::Cerrada || s.compartidas.is_none() {
        return None;
    }
    Some(
        v::instancias_compartidas(app.paleta(), &modelo(app))
            .map(|a| Mensaje::Compartir(Accion::Vista(a))),
    )
}

/// Lee el vínculo y la revisión aplicada de cada instancia, fuera de iced.
pub async fn listar_recibidas(almacen: Almacen, instancias: Vec<Instancia>) -> Vec<Recibida> {
    icaro_core::diagnostico_contexto::bloqueante(move || {
        instancias
            .into_iter()
            .filter_map(|i| {
                let juego = almacen.juego(&i.id).ok()?;
                let vinculo = c::vinculos::leer(&juego).ok().flatten()?;
                let contexto = ContextoDestino {
                    juego,
                    limites: c::Limites::default(),
                };
                let revision = c::aplicacion::cargar_base(&contexto)
                    .ok()
                    .and_then(|b| b.manifiesto.map(|m| m.revision));
                Some(Recibida {
                    fila: v::InstanciaQueRecibes {
                        id: i.id.clone(),
                        nombre: i.nombre.clone(),
                        anfitrion: equipo(&vinculo.origen.anfitrion),
                        estado: v::AnfitrionRecibida::Comprobando,
                        revision,
                        cambios: None,
                        version: version(&i),
                    },
                    origen: vinculo.origen,
                })
            })
            .collect::<Vec<_>>()
    })
    .await
    .unwrap_or_default()
}

/// Pregunta al servicio si el anfitrión sigue publicando; sin red o sin
/// respuesta en 8 s se muestra sin conexión.
pub async fn comprobar_anfitrion(raiz: PathBuf, origen: Origen) -> bool {
    let Some(servicio) = Servicio::desde_entorno() else {
        return false;
    };
    let identidad =
        icaro_core::diagnostico_contexto::bloqueante(move || Identidad::cargar_o_crear(&raiz).ok())
            .await
            .ok()
            .flatten();
    let Some(identidad) = identidad else {
        return false;
    };
    let Ok(cliente) = c::invitaciones::ClienteInvitaciones::nuevo(&servicio.url, identidad) else {
        return false;
    };
    tokio::time::timeout(Duration::from_secs(8), cliente.reencontrar(&origen))
        .await
        .is_ok_and(|r| r.is_ok())
}

/// Abre la página y carga lo que recibes en segundo plano.
pub fn abrir(app: &mut App, pestana: Option<v::PestanaCompartidas>) -> Task<Mensaje> {
    let s = &mut app.servicios.compartir;
    s.compartidas = Some(pestana.or(s.compartidas).unwrap_or_default());
    s.menu_compartida = None;
    if app.servicios.demo || !s.habilitado {
        return Task::none();
    }
    s.recibidas_generacion = s.recibidas_generacion.wrapping_add(1);
    let generacion = s.recibidas_generacion;
    let almacen = app.servicios.instancias.almacen.clone();
    let instancias = app.servicios.instancias.registros.clone();
    Task::batch([
        super::biblioteca::cargar(app),
        Task::perform(listar_recibidas(almacen, instancias), move |r| {
            m(Accion::Recibidas(generacion, r))
        }),
    ])
}

/// Guarda lo recibido y comprueba cada anfitrión por separado.
pub fn recibidas(s: &mut Estado, generacion: u64, lista: Vec<Recibida>) -> Task<Mensaje> {
    if generacion != s.recibidas_generacion {
        return Task::none();
    }
    let raiz = crate::estado::directorio(false);
    let tareas: Vec<_> = lista
        .iter()
        .map(|r| {
            let (id, origen, raiz) = (r.fila.id.clone(), r.origen.clone(), raiz.clone());
            Task::perform(comprobar_anfitrion(raiz, origen), move |ok| {
                m(Accion::AnfitrionRecibido(generacion, id.clone(), ok))
            })
        })
        .collect();
    s.recibidas = lista;
    Task::batch(tareas)
}

pub fn anfitrion(s: &mut Estado, generacion: u64, id: &str, disponible: bool) {
    if generacion != s.recibidas_generacion {
        return;
    }
    if let Some(r) = s.recibidas.iter_mut().find(|r| r.fila.id == id) {
        r.fila.estado = if disponible {
            v::AnfitrionRecibida::Disponible
        } else {
            v::AnfitrionRecibida::SinConexion
        };
    }
}

fn indice(app: &App, id: &str) -> Option<usize> {
    app.servicios
        .instancias
        .registros
        .iter()
        .position(|i| i.id == id)
}

/// Acciones propias de la página; `None` deja pasar las demás.
pub fn accion(app: &mut App, a: &v::Accion) -> Option<Task<Mensaje>> {
    let s = &mut app.servicios.compartir;
    match a {
        v::Accion::VerCompartidas => {
            if s.modal() {
                // Sale del flujo como Cerrar; si pide confirmación, la página espera.
                let cerrar = super::actualizar(app, Accion::Vista(v::Accion::Cerrar));
                return Some(Task::batch([cerrar, abrir(app, None)]));
            }
            Some(abrir(app, None))
        }
        v::Accion::VerServidores => {
            s.compartidas = None;
            s.menu_compartida = None;
            if s.modal() {
                return Some(super::actualizar(app, Accion::Vista(v::Accion::Cerrar)));
            }
            Some(Task::none())
        }
        v::Accion::PestanaCompartidas(p) => {
            s.compartidas = Some(*p);
            s.menu_compartida = None;
            Some(Task::none())
        }
        v::Accion::MenuCompartida(menu) => {
            s.menu_compartida = menu.clone();
            Some(Task::none())
        }
        v::Accion::CopiarCodigoDe(id) => {
            let codigo = s
                .panel
                .as_ref()
                .filter(|p| &p.instancia.id == id)
                .and_then(|p| p.invitacion.as_ref())
                .map(|i| i.codigo.clone())?;
            s.copiado_de = Some(id.clone());
            Some(iced::clipboard::write(codigo))
        }
        v::Accion::ReanudarDe(id) | v::Accion::CambiarCodigoDe(id) => {
            s.menu_compartida = None;
            // Abre la publicación: se reconecta si estaba en pausa y desde ahí
            // se cambia el código.
            Some(super::biblioteca::abrir(app, id.clone()))
        }
        v::Accion::RevisarCambiosDe(id) => {
            s.menu_compartida = None;
            s.seleccionada = Some(id.clone());
            Some(super::actualizar(
                app,
                Accion::Vista(v::Accion::ElegirInstancia(id.clone())),
            ))
        }
        v::Accion::PausarDe(id) => {
            s.menu_compartida = None;
            if s.publicacion.as_ref().map(|p| &p.instancia_id) != Some(id) {
                return Some(Task::none());
            }
            // Pausar apaga el nodo local sin revocar a nadie; Reanudar reconecta.
            let h = s.anfitrion.take()?;
            s.biblioteca.marcar_reconexion();
            if let Some(p) = &mut s.panel {
                p.invitacion = None;
            }
            Some(Task::perform(async move { h.cerrar().await }, |_| {
                Mensaje::Nada
            }))
        }
        v::Accion::DejarDeCompartirDe(id) => {
            s.menu_compartida = None;
            if s.publicacion.as_ref().map(|p| &p.instancia_id) != Some(id) {
                return Some(Task::none());
            }
            let abrir = super::biblioteca::abrir(app, id.clone());
            let detener = super::actualizar(app, Accion::Vista(v::Accion::DejarDeCompartir));
            Some(Task::batch([abrir, detener]))
        }
        v::Accion::JugarRecibida(id) => {
            s.menu_compartida = None;
            let i = indice(app, id)?;
            Some(Task::done(Mensaje::Jugar(i)))
        }
        v::Accion::SincronizarRecibida(id) => {
            s.menu_compartida = None;
            let i = indice(app, id)?;
            Some(super::sincronizar::abrir(app, i, false, None))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn el_equipo_se_nombra_con_su_huella_corta() {
        assert_eq!(super::equipo(&"9e2a7b14".repeat(8)), "Equipo 9e2a 7b14");
        assert_eq!(super::equipo("ab"), "Equipo ab");
    }
}
