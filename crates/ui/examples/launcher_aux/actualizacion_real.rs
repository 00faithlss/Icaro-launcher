// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! ActualizacionLauncher del System Design, con releases y notas reales.
use crate::{pantallas, App, Mensaje};
use icaro_core::actualizacion::{self, Canal, Disponible, Preparada, Resultado};
use icaro_ui::{
    componentes::{self, Estado as Senal, Variante},
    fuentes,
    iconos::Icono,
    laminas::{self, Lamina},
    tema::{espacio, texto},
};
use iced::futures::SinkExt;
use iced::{
    widget::{column, container, row, text, Space},
    Alignment, Element, Length, Task,
};

#[derive(Default)]
pub struct Estado {
    pub disponible: Option<Disponible>,
    pub comprobando: bool,
    pub comprobada: bool,
    pub error: Option<String>,
    pub descargando: bool,
    pub aplicando: bool,
    pub avance: f32,
    pub preparada: Option<Preparada>,
    pub al_cerrar: bool,
    pub resultado: Option<Resultado>,
    anterior: Option<pantallas::Pantalla>,
}
#[derive(Debug, Clone)]
pub enum Accion {
    Comprobar,
    Comprobada(Result<Option<Disponible>, String>),
    Abrir,
    Ahora,
    AlCerrar,
    MasTarde,
    Progreso(u64, u64),
    Descargada(bool, Result<Preparada, String>),
    AplicacionLanzada(Result<(), String>),
}
pub fn m(a: Accion) -> Mensaje {
    Mensaje::Actualizacion(a)
}
/// Canal con el que se compiló este programa; es el predeterminado en Ajustes.
pub fn canal_compilado() -> Canal {
    if env!("ICARO_CANAL") == "beta" { Canal::Beta } else { Canal::Estable }
}
/// Versión para mostrar: la de Cargo, marcada si es una beta.
pub fn version_visible() -> String {
    match canal_compilado() {
        Canal::Beta => format!("{} beta", env!("CARGO_PKG_VERSION")),
        Canal::Estable => env!("CARGO_PKG_VERSION").into(),
    }
}
pub fn comprobar(canal: Canal) -> Task<Mensaje> {
    Task::perform(
        async move {
            actualizacion::comprobar_canal(env!("CARGO_PKG_VERSION"), env!("ICARO_COMPILACION").parse().unwrap_or(0), canal)
                .await
                .map_err(|e| e.to_string())
        },
        |r| m(Accion::Comprobada(r)),
    )
}
fn juego_abierto(app: &App) -> bool {
    !app.servicios.juego.procesos.is_empty()
        || app.servicios.juego.trabajos.values().any(|t| {
            matches!(
                t.estado,
                icaro_ui::descargas::EstadoDescarga::Activa(_)
                    | icaro_ui::descargas::EstadoDescarga::EnCola(_)
            )
        })
}
pub fn actualizar(app: &mut App, accion: Accion) -> Task<Mensaje> {
    match accion {
        Accion::Abrir => {
            if app.pantalla != Some(pantallas::Pantalla::Actualizacion) {
                app.actualizacion.anterior = app.pantalla;
            }
            app.pantalla = Some(pantallas::Pantalla::Actualizacion);
            if !app.actualizacion.comprobada && !app.actualizacion.comprobando {
                app.actualizacion.comprobando = true;
                return comprobar(app.aj.canal);
            }
        }
        Accion::Comprobar => {
            if !app.actualizacion.comprobando && !app.actualizacion.descargando && !app.actualizacion.aplicando {
                app.actualizacion.comprobando = true;
                app.actualizacion.error = None;
                return comprobar(app.aj.canal);
            }
        }
        Accion::Comprobada(r) => {
            app.actualizacion.comprobando = false;
            app.actualizacion.comprobada = true;
            match r {
                Ok(d) => {
                    if app
                        .actualizacion
                        .preparada
                        .as_ref()
                        .is_some_and(|p| d.as_ref().is_none_or(|n| n.version != p.version || n.compilacion != p.compilacion))
                    {
                        app.actualizacion.preparada = None;
                        app.actualizacion.al_cerrar = false;
                    }
                    app.actualizacion.disponible = d;
                    app.actualizacion.error = None;
                }
                Err(e) => app.actualizacion.error = Some(e),
            }
        }
        Accion::Ahora | Accion::AlCerrar => {
            let ahora = matches!(accion, Accion::Ahora);
            if app.actualizacion.descargando || app.actualizacion.aplicando {
                return Task::none();
            }
            if ahora && juego_abierto(app) {
                app.actualizacion.error =
                    Some("Cierra Minecraft y espera a que terminen sus tareas antes de actualizar Icaro Launcher.".into());
                return Task::none();
            }
            if let Some(p) = app.actualizacion.preparada.as_ref() {
                if ahora {
                    let preparada=p.clone();
                    app.actualizacion.aplicando=true;
                    app.actualizacion.al_cerrar=false;
                    app.actualizacion.resultado=None;
                    app.pantalla=Some(pantallas::Pantalla::Actualizacion);
                    return Task::perform(async move {
                        tokio::task::spawn_blocking(move ||actualizacion::aplicar(&preparada,true))
                            .await.map_err(|_|"No se pudo preparar la actualización.".to_owned())?
                            .map_err(|e|e.to_string())
                    },|r|m(Accion::AplicacionLanzada(r)));
                } else {
                    app.actualizacion.al_cerrar = true;
                    app.pantalla = app.actualizacion.anterior;
                }
                return Task::none();
            }
            let Some(d) = app.actualizacion.disponible.clone() else {
                return Task::none();
            };
            app.actualizacion.descargando = true;
            app.actualizacion.resultado = None;
            app.actualizacion.error = None;
            app.actualizacion.avance = 0.;
            return Task::run(
                iced::stream::channel(32, move |mut salida| async move {
                    let (tx, mut rx) = iced::futures::channel::mpsc::unbounded();
                    let descargar = actualizacion::descargar(d, move |n, total| {
                        let _ = tx.unbounded_send(m(Accion::Progreso(n, total)));
                    });
                    tokio::pin!(descargar);
                    use iced::futures::StreamExt;
                    let resultado = loop {
                        tokio::select! {
                            r = &mut descargar => break r.map_err(|e| e.to_string()),
                            Some(a) = rx.next() => { let _ = salida.send(a).await; }
                        }
                    };
                    let _ = salida.send(m(Accion::Descargada(ahora, resultado))).await;
                }),
                |a| a,
            );
        }
        Accion::MasTarde => {
            if app.actualizacion.descargando || app.actualizacion.aplicando {
                return Task::none();
            }
            app.actualizacion.al_cerrar = false;
            app.pantalla = if app.actualizacion.resultado.is_some() && !app.servicios.demo
                && app.servicios.session.is_none() {
                Some(pantallas::Pantalla::Primera)
            } else { app.actualizacion.anterior };
        }
        Accion::Progreso(n, total) => {
            app.actualizacion.avance = if total == 0 {
                0.
            } else {
                (n as f32 / total as f32).clamp(0., 1.)
            };
        }
        Accion::Descargada(ahora, r) => {
            app.actualizacion.descargando = false;
            match r {
                Err(e) => app.actualizacion.error = Some(e),
                Ok(p) => {
                    app.actualizacion.preparada = Some(p);
                    if ahora && !juego_abierto(app) {
                        return actualizar(app, Accion::Ahora);
                    }
                    app.actualizacion.al_cerrar = true;
                    if ahora {
                        app.actualizacion.error = Some(
                            "Minecraft está abierto. La actualización se aplicará al cerrar Icaro Launcher."
                                .into(),
                        );
                    } else {
                        app.pantalla = app.actualizacion.anterior;
                    }
                }
            }
        }
        Accion::AplicacionLanzada(r) if app.actualizacion.aplicando => match r {
            Ok(()) => return crate::juego_real::cerrar(app),
            Err(e) => {
                app.actualizacion.aplicando = false;
                app.actualizacion.preparada = None;
                app.actualizacion.al_cerrar = false;
                app.actualizacion.error = Some(e);
            }
        }
        Accion::AplicacionLanzada(_)=>{},
    }
    Task::none()
}
/// Invocar antes del cierre normal. No fuerza el cierre de un juego.
pub fn al_cerrar(app: &mut App) -> bool {
    if app.actualizacion.aplicando { return false; }
    if !app.actualizacion.al_cerrar {
        return true;
    }
    if juego_abierto(app) {
        app.actualizacion.error =
            Some("Cierra Minecraft y espera a que terminen sus tareas antes de aplicar la actualización pendiente.".into());
        app.pantalla = Some(pantallas::Pantalla::Actualizacion);
        return false;
    }
    if let Some(p) = app.actualizacion.preparada.as_ref() {
        if let Err(e) = actualizacion::aplicar(p, false) {
            app.actualizacion.error = Some(e.to_string());
            app.pantalla = Some(pantallas::Pantalla::Actualizacion);
            return false;
        }
    }
    app.actualizacion.al_cerrar = false;
    true
}

/// Banner info discreto; nunca abre la actualización por su cuenta.
pub fn banner(app: &App) -> Option<Element<'_, Mensaje>> {
    let d = app.actualizacion.disponible.as_ref()?;
    let p = app.paleta();
    if app.pantalla == Some(pantallas::Pantalla::Actualizacion) {
        return None;
    }
    Some(
        container(
            row![
                text(format!("Icaro Launcher {} disponible", d.version))
                    .size(texto::BODY_SM.0)
                    .color(p.text),
                Space::with_width(Length::Fill),
                componentes::boton(
                    p,
                    "Ver actualización",
                    Variante::Fantasma,
                    Some(m(Accion::Abrir))
                )
            ]
            .align_y(Alignment::Center),
        )
        .padding([espacio::S2, espacio::S4])
        .style(move |_: &iced::Theme| iced::widget::container::Style {
            background: Some(p.info_soft.into()),
            ..Default::default()
        })
        .into(),
    )
}
pub fn vista(app: &App) -> Element<'_, Mensaje> {
    iced::widget::responsive(move |tam| vista_ancho(app, tam.width)).into()
}
fn vista_ancho(app: &App, ancho: f32) -> Element<'_, Mensaje> {
    let p = app.paleta();
    let e = &app.actualizacion;
    let titulo = e
        .disponible
        .as_ref()
        .map(|d| format!("ICARO LAUNCHER {}{}", d.version, if d.beta { " BETA" } else { "" }))
        .unwrap_or_else(|| "ICARO LAUNCHER".into());
    let detalle = e
        .disponible
        .as_ref()
        .map(|d| {
            format!(
                "Tienes {} · descarga de {:.1} MB",
                version_visible(),
                d.bytes as f64 / 1048576.
            )
        })
        .unwrap_or_else(|| format!("Versión {}", version_visible()));
    let mut notas = column![].spacing(espacio::S2);
    if let Some(d) = &e.disponible {
        for n in &d.notas {
            let insignia = match n.tipo.as_str() {
                "nuevo" => componentes::insignia(p, Senal::Exito, "Nuevo"),
                "mejora" => componentes::insignia(p, Senal::Info, "Mejora"),
                _ => componentes::insignia_contorno(p, "Corrección"),
            };
            notas = notas.push(
                row![
                    container(insignia).width(110),
                    text(n.texto.clone()).size(texto::BODY.0).color(p.text)
                ]
                .spacing(espacio::S4)
                .align_y(Alignment::Center),
            );
        }
    } else if e.resultado.is_none() {
        notas = notas.push(
            text(if e.comprobando {
                "Comprobando actualizaciones…"
            } else if e.comprobada && e.error.is_none() {
                "Tienes la versión más reciente publicada."
            } else {
                "Comprueba si hay una nueva versión de Icaro Launcher."
            })
            .size(texto::BODY.0)
            .color(p.text_muted),
        );
    }
    if let Some(resultado)=&e.resultado {
        notas=notas.push(text(if resultado.correcto {
            "Actualización instalada correctamente. Puedes seguir usando Ícaro."
        } else {
            "No se pudo completar la actualización. Vuelve a intentarlo desde esta pantalla."
        }).size(texto::BODY.0).color(if resultado.correcto {p.success}else{p.error}));
        if let Some(detalle) = &resultado.detalle {
            notas = notas.push(text(detalle.clone()).size(texto::BODY_SM.0).color(p.error));
        }
    }
    if let Some(error) = &e.error {
        notas = notas.push(text(error.clone()).size(texto::BODY_SM.0).color(p.error));
    }
    if e.descargando {
        notas = notas.push(
            text(format!(
                "Descargando actualización · {:.0} %",
                e.avance * 100.
            ))
            .font(fuentes::MONO)
            .size(texto::MONO_SM.0)
            .color(p.text_muted),
        );
        notas=notas.push(icaro_ui::progreso::continuo(p,e.avance,app.reducir));
    }
    if e.aplicando {
        notas=notas.push(text("Verificando y preparando la actualización… Ícaro se reiniciará y volverá a esta pantalla.")
            .size(texto::BODY_SM.0).color(p.text_muted));
    }
    if e.al_cerrar {
        notas = notas.push(
            text("Actualización lista para aplicar al cerrar Icaro Launcher.")
                .size(texto::BODY_SM.0)
                .color(p.success),
        );
    }
    if juego_abierto(app) && e.disponible.is_some() {
        notas = notas.push(
            text("Cierra Minecraft antes de actualizar ahora.")
                .size(texto::BODY_SM.0)
                .color(p.text_muted),
        );
    }
    let acciones: Element<'_, Mensaje> = if e.disponible.is_some() {
        row![
            componentes::boton_con_icono(
                p,
                Icono::Actualizar,
                "Actualizar ahora",
                Variante::Primario,
                (!e.descargando && !e.aplicando && !juego_abierto(app)).then(|| m(Accion::Ahora)),
                false
            ),
            componentes::boton(
                p,
                "Al cerrar Icaro Launcher",
                Variante::Secundario,
                (!e.descargando && !e.aplicando).then(|| m(Accion::AlCerrar))
            ),
            componentes::boton(
                p,
                "Más tarde",
                Variante::Fantasma,
                (!e.descargando && !e.aplicando).then(|| m(Accion::MasTarde))
            )
        ]
        .spacing(espacio::S2)
        .align_y(Alignment::Center)
        .into()
    } else {
        row![
            componentes::boton(
                p,
                "Comprobar actualizaciones",
                Variante::Primario,
                (!e.comprobando).then(|| m(Accion::Comprobar))
            ),
            componentes::boton(p, "Volver", Variante::Fantasma, Some(m(Accion::MasTarde)))
        ]
        .spacing(espacio::S2)
        .into()
    };
    let cuerpo = column![
        column![
            text(titulo)
                .font(fuentes::DISPLAY)
                .size(texto::DISPLAY_XL.0)
                .color(p.text),
            text(detalle)
                .font(fuentes::MONO)
                .size(texto::MONO_SM.0)
                .color(p.text_muted)
        ]
        .spacing(espacio::S2),
        notas,
        Space::with_height(Length::Fill),
        acciones
    ]
    .spacing(espacio::S6)
    .padding(40)
    .width(Length::Fill)
    .height(Length::Fill);
    if ancho < 1000. {
        return cuerpo.into();
    }
    row![
        container(laminas::grabado_posicion(
            p,
            Lamina::Saturno,
            Length::Fill,
            Length::Fill,
            (0.5, 0.3)
        ))
        .width(300)
        .height(Length::Fill)
        .style(move |_: &iced::Theme| iced::widget::container::Style {
            border: iced::Border {
                color: p.text,
                width: 2.,
                radius: 0.into()
            },
            ..Default::default()
        }),
        cuerpo
    ]
    .height(Length::Fill)
    .width(Length::Fill)
    .into()
}

/// Escena aislada para comparar Tinta/Piedra con el diseño; nunca se usa como oferta real.
pub fn preparar_diseno(app: &mut App) {
    let disponible = serde_json::from_value(serde_json::json!({
        "version": "1.3.0",
        "bytes": 14889779,
        "url": "https://github.com/00faithlss/Icaro-launcher/releases/download/v1.3.0/icaro-windows-x86_64.exe",
        "sha256": "00".repeat(32),
        "notas": [
            {"tipo":"nuevo", "texto":"Sincroniza mods con un servidor antes de unirte."},
            {"tipo":"nuevo", "texto":"Estudio de skin con vista de frente y espalda."},
            {"tipo":"mejora", "texto":"La consola maneja 100.000 líneas sin trabarse."},
            {"tipo":"correccion", "texto":"Las descargas pausadas ya no se reinician al cerrar."}
        ]
    })).expect("Escena de ActualizacionLauncher válida");
    app.actualizacion = Estado {
        disponible: Some(disponible),
        comprobada: true,
        ..Default::default()
    };
    app.pantalla = Some(pantallas::Pantalla::Actualizacion);
}

/// Variante visual de la programación al cierre; no contiene ningún archivo ejecutable.
pub fn preparar_diseno_al_cerrar(app: &mut App) {
    preparar_diseno(app);
    app.actualizacion.al_cerrar = true;
}

/// Estados de la misma pantalla original, sin descarga ni reemplazo reales.
pub fn preparar_diseno_estado(app: &mut App, escena: &str) {
    preparar_diseno(app);
    match escena.trim_end_matches("-angosto") {
        "actualizacion-descargando" => {app.actualizacion.descargando=true;app.actualizacion.avance=0.62;},
        "actualizacion-preparando" => app.actualizacion.aplicando=true,
        "actualizacion-completada" | "actualizacion-error" => {
            let correcto=escena.starts_with("actualizacion-completada");
            app.actualizacion.resultado=Some(serde_json::from_value(serde_json::json!({
                "esquema":1,"version":env!("CARGO_PKG_VERSION"),"compilacion":0,"correcto":correcto
            })).unwrap());
            if correcto {app.actualizacion.disponible=None;}
        },
        _=>{},
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn app() -> App {
        let (mut app,_) = crate::iniciar_con_guardado(crate::Seccion::Instancias,false,None,false,crate::servicios::Estado::nuevo(true),None);
        preparar_diseno(&mut app);
        app
    }
    #[test]
    fn comprobar_tras_un_fallo_no_inicia_otra_descarga_ni_instalacion() {
        let mut app = app();
        let disponible = app.actualizacion.disponible.clone();
        app.actualizacion.al_cerrar = false;
        app.actualizacion.resultado = Some(
            serde_json::from_value(serde_json::json!({
                "esquema": 1, "version": "0.2.1", "compilacion": 42, "correcto": false,
                "detalle": "El reemplazo falló"
            }))
            .unwrap(),
        );
        let _ = actualizar(&mut app, Accion::Comprobada(Ok(disponible)));
        assert!(!app.actualizacion.descargando && !app.actualizacion.aplicando);
        assert!(!app.actualizacion.al_cerrar);
        assert!(app.actualizacion.resultado.is_some());
    }

    #[test]
    fn el_ayudante_lanzado_mantiene_el_bloqueo_hasta_el_cierre() {
        let mut app = app();
        app.actualizacion.aplicando = true;
        let _ = actualizar(&mut app, Accion::AplicacionLanzada(Ok(())));
        assert!(app.actualizacion.aplicando);
        let _ = actualizar(&mut app, Accion::Ahora);
        assert!(!app.actualizacion.descargando);
        assert!(!al_cerrar(&mut app));
    }
    #[test]
    fn la_preparacion_bloquea_otro_intento_el_cierre_y_el_juego() {
        let mut app=app();app.actualizacion.aplicando=true;
        assert!(!al_cerrar(&mut app));
        for accion in [Accion::Ahora,Accion::AlCerrar,Accion::MasTarde,Accion::Comprobar] {
            let _=actualizar(&mut app,accion);
        }
        assert!(app.actualizacion.aplicando && !app.actualizacion.descargando && !app.actualizacion.comprobando);
        assert_eq!(app.pantalla,Some(pantallas::Pantalla::Actualizacion));
        let _=crate::juego_real::iniciar(&mut app,0,true);
        assert!(app.servicios.juego.trabajos.is_empty());
        assert_eq!(app.pantalla,Some(pantallas::Pantalla::Actualizacion));
        let _=actualizar(&mut app,Accion::AplicacionLanzada(Err("Prueba: archivo bloqueado".into())));
        assert!(!app.actualizacion.aplicando && !app.actualizacion.al_cerrar);
        assert!(app.actualizacion.error.as_deref().unwrap().contains("bloqueado"));
        assert_eq!(app.pantalla,Some(pantallas::Pantalla::Actualizacion));
        // Una respuesta duplicada no puede cerrar el launcher después del fallo.
        let _=actualizar(&mut app,Accion::AplicacionLanzada(Ok(())));
        assert_eq!(app.pantalla,Some(pantallas::Pantalla::Actualizacion));
    }
}
