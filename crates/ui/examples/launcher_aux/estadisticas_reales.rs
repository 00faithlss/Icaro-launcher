// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Tiempo jugado y tamaño de las instancias, medidos fuera del hilo de la
//! interfaz y solo para la instancia abierta en el editor.

use crate::{App, Mensaje};
use iced::Task;
use std::collections::{HashMap, HashSet};

#[derive(Default)]
pub struct Estado {
    datos: HashMap<String, Datos>,
    midiendo: HashSet<String>,
    vencidas: HashSet<String>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Datos {
    pub segundos: Option<u64>,
    pub bytes: Option<u64>,
}

#[derive(Debug, Clone)]
pub enum Accion {
    Medida(String, Datos),
    /// Sesión de juego terminada con su duración en segundos.
    Jugado(String, u64),
    Sumado(String),
    Fallo(String),
}

/// Vuelve a medir la instancia la próxima vez que se muestre.
pub fn vencer(app: &mut App, id: &str) {
    app.estadisticas.vencidas.insert(id.to_owned());
}

/// Mide la instancia abierta si todavía no tiene datos o vencieron. Una
/// medición en curso no se repite; si vence mientras tanto, se mide otra vez.
pub fn pedir(app: &mut App) -> Task<Mensaje> {
    if app.servicios.demo || app.editor.is_none() {
        return Task::none();
    }
    let Some(id) = app
        .servicios
        .instancias
        .registros
        .get(app.editor_inst)
        .map(|r| r.id.clone())
    else {
        return Task::none();
    };
    let estado = &mut app.estadisticas;
    if estado.midiendo.contains(&id)
        || (estado.datos.contains_key(&id) && !estado.vencidas.contains(&id))
    {
        return Task::none();
    }
    estado.vencidas.remove(&id);
    estado.midiendo.insert(id.clone());
    let almacen = app.servicios.instancias.almacen.clone();
    Task::perform(
        async move {
            let medir = id.clone();
            let datos = tokio::task::spawn_blocking(move || Datos {
                segundos: almacen.tiempo_jugado(&medir).ok(),
                bytes: almacen.tamano(&medir).ok(),
            })
            .await
            .unwrap_or_default();
            (id, datos)
        },
        |(id, datos)| Mensaje::Estadisticas(Accion::Medida(id, datos)),
    )
}

pub fn actualizar(app: &mut App, accion: Accion) -> Task<Mensaje> {
    match accion {
        Accion::Medida(id, datos) => {
            app.estadisticas.midiendo.remove(&id);
            app.estadisticas.datos.insert(id, datos);
            Task::none()
        }
        Accion::Jugado(id, segundos) => {
            let almacen = app.servicios.instancias.almacen.clone();
            Task::perform(
                async move {
                    let sumar = id.clone();
                    let resultado = tokio::task::spawn_blocking(move || {
                        almacen
                            .sumar_tiempo_jugado(&sumar, segundos)
                            .map_err(|e| e.to_string())
                    })
                    .await
                    .map_err(|e| e.to_string())
                    .and_then(|r| r);
                    (id, resultado)
                },
                |(id, resultado)| match resultado {
                    Ok(_) => Mensaje::Estadisticas(Accion::Sumado(id)),
                    Err(e) => Mensaje::Estadisticas(Accion::Fallo(e)),
                },
            )
        }
        Accion::Sumado(id) => {
            // La partida también pudo cambiar mundos, capturas y registros.
            vencer(app, &id);
            Task::none()
        }
        Accion::Fallo(e) => {
            app.avisar(format!("No se pudo guardar el tiempo jugado: {e}"));
            Task::none()
        }
    }
}

pub fn datos(app: &App, id: &str) -> Datos {
    app.estadisticas.datos.get(id).copied().unwrap_or_default()
}

/// «35 min», «1,5 h» o «41 h»; «—» si no se pudo leer.
pub fn texto_jugado(segundos: Option<u64>) -> String {
    let Some(s) = segundos else {
        return "—".into();
    };
    if s < 3600 {
        return format!("{} min", s / 60);
    }
    let horas = s as f64 / 3600.0;
    if horas < 10.0 {
        format!("{horas:.1} h").replace('.', ",")
    } else {
        format!("{} h", s / 3600)
    }
}

pub fn texto_tamano(bytes: Option<u64>) -> String {
    bytes.map_or_else(|| "—".into(), icaro_ui::capturas::formato_peso)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_tiempo_jugado_se_muestra_en_minutos_u_horas() {
        assert_eq!(texto_jugado(None), "—");
        assert_eq!(texto_jugado(Some(0)), "0 min");
        assert_eq!(texto_jugado(Some(35 * 60 + 20)), "35 min");
        assert_eq!(texto_jugado(Some(90 * 60)), "1,5 h");
        assert_eq!(texto_jugado(Some(41 * 3600 + 1800)), "41 h");
    }
}
