// SPDX-License-Identifier: GPL-3.0-only
//! SelectorInstanciaCompartir: datos de cada instancia y su elección antes
//! de analizarla. Contar mods, medir y leer la última partida se hace fuera de iced.
use super::{Accion, Etapa};
use crate::{App, Mensaje};
use icaro_core::{
    compartir::biblioteca::EstadoGuardada,
    instancias::{Almacen, Instancia},
};
use icaro_ui::compartir as v;
use iced::Task;
use std::{collections::HashMap, time::SystemTime};

/// Lo que se mide de cada instancia.
#[derive(Debug, Clone, Default)]
pub struct DatosOpcion {
    pub mods: usize,
    pub bytes: Option<u64>,
    /// Última vez que Minecraft escribió `logs/latest.log`.
    pub ultima: Option<SystemTime>,
}

#[derive(Debug, Default)]
pub struct Selector {
    pub datos: HashMap<String, DatosOpcion>,
    pub busqueda: String,
    pub generacion: u64,
}

fn m(a: Accion) -> Mensaje {
    Mensaje::Compartir(a)
}

/// Cuenta mods activos, mide la carpeta y lee la última partida.
pub async fn medir(almacen: Almacen, instancias: Vec<Instancia>) -> Vec<(String, DatosOpcion)> {
    icaro_core::diagnostico_contexto::bloqueante(move || {
        instancias
            .into_iter()
            .map(|i| {
                let juego = almacen.juego(&i.id).ok();
                let mods = juego
                    .as_ref()
                    .and_then(|j| std::fs::read_dir(j.join("mods")).ok())
                    .map_or(0, |l| {
                        l.filter_map(Result::ok)
                            .filter(|e| {
                                e.file_name()
                                    .to_string_lossy()
                                    .to_ascii_lowercase()
                                    .ends_with(".jar")
                                    && e.file_type().is_ok_and(|t| t.is_file())
                            })
                            .count()
                    });
                let ultima = juego
                    .as_ref()
                    .and_then(|j| std::fs::metadata(j.join("logs").join("latest.log")).ok())
                    .and_then(|m| m.modified().ok());
                let bytes = almacen.tamano(&i.id).ok();
                (
                    i.id,
                    DatosOpcion {
                        mods,
                        bytes,
                        ultima,
                    },
                )
            })
            .collect()
    })
    .await
    .unwrap_or_default()
}

pub fn cargar(app: &mut App) -> Task<Mensaje> {
    let s = &mut app.servicios.compartir;
    s.selector.generacion = s.selector.generacion.wrapping_add(1);
    s.selector.busqueda.clear();
    if app.servicios.demo {
        return Task::none();
    }
    let generacion = s.selector.generacion;
    let almacen = app.servicios.instancias.almacen.clone();
    let instancias = app.servicios.instancias.registros.clone();
    Task::perform(medir(almacen, instancias), move |r| {
        m(Accion::DatosSelector(generacion, r))
    })
}

pub fn recibidos(app: &mut App, generacion: u64, datos: Vec<(String, DatosOpcion)>) {
    let s = &mut app.servicios.compartir.selector;
    if generacion == s.generacion {
        s.datos = datos.into_iter().collect();
    }
}

fn tamano(bytes: u64) -> String {
    let gb = 1024. * 1024. * 1024.;
    if bytes as f64 >= gb {
        format!("{:.1} GB", bytes as f64 / gb).replace('.', ",")
    } else {
        super::tamano(bytes)
    }
}

/// Modelo del selector; la elección por defecto es la libre jugada más reciente.
pub fn modelo(app: &App) -> v::SelectorInstancia {
    let s = &app.servicios.compartir;
    let ahora = SystemTime::now();
    let opciones: Vec<v::OpcionInstancia> = app
        .servicios
        .instancias
        .registros
        .iter()
        .map(|i| {
            let datos = s.selector.datos.get(&i.id);
            let compartida = s.biblioteca.entradas.iter().find(|e| {
                e.instancia == i.id
                    && e.revision.is_some()
                    && matches!(
                        e.estado,
                        EstadoGuardada::Lista
                            | EstadoGuardada::Detenida
                            | EstadoGuardada::PerfilCambiado
                    )
            });
            let en_uso = app.servicios.juego.ocupada(&i.id);
            let vanilla =
                i.loader.eq_ignore_ascii_case("vanilla") || datos.is_some_and(|d| d.mods == 0);
            let estado = if en_uso {
                v::EstadoOpcion::EnUso
            } else if let Some(e) = compartida {
                v::EstadoOpcion::Compartida {
                    revision: e.revision.unwrap_or_default(),
                    personas: e.autorizados,
                }
            } else if vanilla {
                v::EstadoOpcion::Vanilla
            } else {
                v::EstadoOpcion::Libre
            };
            v::OpcionInstancia {
                id: i.id.clone(),
                nombre: i.nombre.clone(),
                version: format!("{} · {}", i.minecraft, i.loader),
                mods: datos.map_or_else(String::new, |d| match d.mods {
                    0 => "Sin mods".into(),
                    1 => "1 mod".into(),
                    n => format!("{n} mods"),
                }),
                tamano: datos.and_then(|d| d.bytes).map(tamano).unwrap_or_default(),
                jugado: if en_uso {
                    "Jugando ahora".into()
                } else {
                    datos.map_or_else(String::new, |d| crate::hora_local::jugaste(d.ultima, ahora))
                },
                estado,
            }
        })
        .collect();
    let elegida = s
        .seleccionada
        .clone()
        .filter(|id| opciones.iter().any(|o| &o.id == id && o.elegible()))
        .or_else(|| {
            opciones
                .iter()
                .filter(|o| o.elegible())
                .max_by_key(|o| s.selector.datos.get(&o.id).and_then(|d| d.ultima))
                .map(|o| o.id.clone())
        });
    v::SelectorInstancia {
        opciones,
        elegida,
        busqueda: s.selector.busqueda.clone(),
    }
}

/// Acciones del selector; `None` deja pasar las demás.
pub fn accion(app: &mut App, a: &v::Accion) -> Option<Task<Mensaje>> {
    if app.servicios.compartir.etapa != Etapa::Selector {
        return None;
    }
    match a {
        v::Accion::SeleccionarInstancia(id) => {
            app.servicios.compartir.seleccionada = Some(id.clone());
            Some(Task::none())
        }
        v::Accion::BuscarInstancia(q) => {
            app.servicios.compartir.selector.busqueda = q.chars().take(80).collect();
            Some(Task::none())
        }
        v::Accion::Continuar => {
            let id = modelo(app).elegida?;
            Some(super::actualizar(
                app,
                Accion::Vista(v::Accion::ElegirInstancia(id)),
            ))
        }
        v::Accion::CrearInstancia => {
            app.servicios.compartir.cerrar();
            Some(Task::batch([
                Task::done(Mensaje::Ir(icaro_ui::shell::Seccion::Instancias)),
                Task::done(Mensaje::Pant(crate::pantallas::Accion::NuevaInstancia)),
            ]))
        }
        _ => None,
    }
}
