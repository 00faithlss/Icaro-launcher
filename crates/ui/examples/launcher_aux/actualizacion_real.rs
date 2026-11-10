// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! ActualizacionLauncher del System Design, con releases y notas reales.
use crate::{pantallas, App, Mensaje};
use icaro_core::actualizacion::{self, Canal, Disponible, Preparada, Resultado};
use icaro_ui::{
    aviso_actualizacion as aviso,
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
    /// Versión (con compilación) cuya tarjeta se minimizó; se guarda para no
    /// volver a mostrarla como tarjeta hasta la siguiente versión.
    pub minimizada: Option<String>,
    /// Cuándo se volvió a abrir la tarjeta desde la pastilla, para animarla.
    mostrada: Option<std::time::Instant>,
    /// Bytes descargados y totales, y cuándo empezó la descarga.
    hecho: u64,
    total: u64,
    inicio: Option<std::time::Instant>,
    cancelar: Option<tokio_util::sync::CancellationToken>,
    /// El último intento de descarga falló (no una comprobación).
    pub fallo_descarga: bool,
    /// Solo escenas de diseño: simulan una descarga lista o un juego abierto.
    diseno_lista: bool,
    diseno_juego: bool,
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
    /// AvisoActualizacion: dejar la tarjeta como pastilla o volver a abrirla.
    Minimizar,
    Mostrar,
    Cancelar,
    Cancelada,
    /// Descargar desde la página de releases.
    Web,
    /// Ver todo lo que cambia en la versión disponible.
    Novedades,
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
pub fn juego_abierto(app: &App) -> bool {
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
            if let Some(p) = app.actualizacion.preparada.as_ref() {
                // Nunca se reinicia con el juego abierto: queda para el cierre.
                if ahora && juego_abierto(app) {
                    app.actualizacion.al_cerrar = true;
                    return Task::none();
                }
                if ahora {
                    let preparada=p.clone();
                    app.actualizacion.aplicando=true;
                    app.actualizacion.al_cerrar=false;
                    app.actualizacion.resultado=None;
                    app.pantalla=Some(pantallas::Pantalla::Actualizacion);
                    let copia = (app.aj.copias_antes && !app.servicios.demo).then(|| crate::estado::directorio(false));
                    return Task::perform(async move {
                        tokio::task::spawn_blocking(move || {
                            if let Some(raiz) = copia { crate::canal_real::copia_previa(&raiz); }
                            actualizacion::aplicar(&preparada,true)
                        })
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
            app.actualizacion.hecho = 0;
            app.actualizacion.total = d.bytes;
            app.actualizacion.inicio = Some(std::time::Instant::now());
            app.actualizacion.fallo_descarga = false;
            let cancelar = tokio_util::sync::CancellationToken::new();
            app.actualizacion.cancelar = Some(cancelar.clone());
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
                            // Cancelar suelta la descarga; la próxima empieza de cero.
                            _ = cancelar.cancelled() => {
                                let _ = salida.send(m(Accion::Cancelada)).await;
                                return;
                            }
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
            app.actualizacion.hecho = n;
            app.actualizacion.total = total;
            app.actualizacion.avance = if total == 0 {
                0.
            } else {
                (n as f32 / total as f32).clamp(0., 1.)
            };
        }
        Accion::Descargada(ahora, r) => {
            app.actualizacion.descargando = false;
            app.actualizacion.cancelar = None;
            match r {
                Err(e) => {
                    app.actualizacion.fallo_descarga = true;
                    app.actualizacion.error = Some(e);
                }
                Ok(p) => {
                    app.actualizacion.preparada = Some(p);
                    if ahora && !juego_abierto(app) {
                        return actualizar(app, Accion::Ahora);
                    }
                    // Con Minecraft abierto, la tarjeta pasa a «espera a Minecraft».
                    app.actualizacion.al_cerrar = true;
                    if !ahora && app.pantalla == Some(pantallas::Pantalla::Actualizacion) {
                        app.pantalla = app.actualizacion.anterior;
                    }
                }
            }
        }
        Accion::Cancelar => {
            if let Some(c) = app.actualizacion.cancelar.take() {
                c.cancel();
            }
        }
        Accion::Cancelada => {
            app.actualizacion.descargando = false;
            app.actualizacion.cancelar = None;
            app.actualizacion.avance = 0.;
            app.actualizacion.hecho = 0;
        }
        Accion::Minimizar => app.actualizacion.minimizada = clave(&app.actualizacion),
        Accion::Mostrar => {
            app.actualizacion.minimizada = None;
            app.ahora = std::time::Instant::now();
            app.actualizacion.mostrada = Some(app.ahora);
        }
        Accion::Web => {
            return crate::servicios::actualizar(
                app,
                crate::servicios::Accion::Enlace(format!("{}/releases", env!("CARGO_PKG_REPOSITORY"))),
            );
        }
        Accion::Novedades => {
            // Leer las novedades cierra la tarjeta: queda la pastilla.
            app.actualizacion.minimizada = clave(&app.actualizacion);
            let disponible = app.actualizacion.disponible.clone();
            crate::novedades::abrir_con(app, false, disponible.as_ref());
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
        if app.aj.copias_antes && !app.servicios.demo { crate::canal_real::copia_previa(&crate::estado::directorio(false)); }
        if let Err(e) = actualizacion::aplicar(p, false) {
            app.actualizacion.error = Some(e.to_string());
            app.pantalla = Some(pantallas::Pantalla::Actualizacion);
            return false;
        }
    }
    app.actualizacion.al_cerrar = false;
    true
}

/// Identidad de la versión ofrecida, para recordar que se minimizó.
fn clave(e: &Estado) -> Option<String> {
    e.disponible.as_ref().map(|d| format!("{}+{}", d.version, d.compilacion))
}

/// Fase de AvisoActualizacion; `None` si no hay nada que avisar o se está
/// aplicando (entonces se ve la pantalla completa).
fn fase(app: &App) -> Option<aviso::Fase> {
    let e = &app.actualizacion;
    if e.aplicando || e.disponible.is_none() {
        return None;
    }
    Some(if e.descargando {
        let velocidad = e
            .inicio
            .map(|i| i.elapsed().as_secs_f64())
            .filter(|s| *s > 0.5)
            .map_or(0, |s| (e.hecho as f64 / s) as u64);
        aviso::Fase::Descargando { hecho: e.hecho, total: e.total, velocidad }
    } else if e.preparada.is_some() || e.diseno_lista {
        if juego_abierto(app) || e.diseno_juego {
            let instancia = app
                .servicios
                .juego
                .procesos
                .keys()
                .next()
                .and_then(|id| app.servicios.instancias.registros.iter().find(|r| &r.id == id))
                .map(|r| r.nombre.clone());
            aviso::Fase::EsperaJuego { instancia }
        } else {
            aviso::Fase::Lista
        }
    } else if let (true, Some(error)) = (e.fallo_descarga, &e.error) {
        aviso::Fase::Error(error.clone())
    } else {
        aviso::Fase::Disponible
    })
}

/// Una línea en lenguaje de jugador por tipo de cambio.
fn resumen(notas: &[actualizacion::Nota]) -> aviso::Resumen {
    let de = |tipo: &'static str| notas.iter().filter(move |n| n.tipo == tipo);
    let corta = |t: &str| -> String {
        let t = t.trim_end_matches('.');
        if t.chars().count() <= 72 {
            t.to_owned()
        } else {
            format!("{}…", t.chars().take(70).collect::<String>().trim_end())
        }
    };
    // Las nuevas se nombran por su título (o lo que va antes de «: »).
    let titulos: Vec<String> = de("nuevo")
        .map(|n| {
            n.titulo.clone().unwrap_or_else(|| {
                n.texto
                    .split_once(": ")
                    .filter(|(t, _)| t.chars().count() <= 48)
                    .map_or_else(|| corta(&n.texto), |(t, _)| t.to_owned())
            })
        })
        .take(3)
        .collect();
    let lista = match titulos.as_slice() {
        [] => String::new(),
        [a] => a.clone(),
        [inicio @ .., ultimo] => format!("{} y {}", inicio.join(", "), ultimo.to_lowercase()),
    };
    let primera = |tipo: &'static str| -> String {
        let mut it = de(tipo);
        match (it.next(), it.next()) {
            (Some(a), Some(_)) => format!("{} y más", corta(&a.texto)),
            (Some(a), None) => corta(&a.texto),
            _ => String::new(),
        }
    };
    aviso::Resumen {
        nuevas: de("nuevo").count(),
        mejoras: de("mejora").count(),
        correcciones: de("correccion").count(),
        linea_nuevas: lista,
        linea_mejoras: primera("mejora"),
        linea_correcciones: primera("correccion"),
    }
}

/// La tarjeta flotante, alineada abajo a la derecha. No aparece durante el
/// arranque del juego ni sobre un diálogo; minimizada queda solo la pastilla.
pub fn tarjeta(app: &App) -> Option<Element<'_, Mensaje>> {
    let e = &app.actualizacion;
    let d = e.disponible.as_ref()?;
    let fase = fase(app)?;
    let importante = d.importante.is_some() && fase == aviso::Fase::Disponible;
    let minimizada = e.minimizada.is_some() && e.minimizada == clave(e);
    if (minimizada && !importante)
        || app.pantalla == Some(pantallas::Pantalla::Actualizacion)
        || app.umbral.is_some()
        || crate::hay_dialogo(app)
    {
        return None;
    }
    let datos = aviso::Datos {
        version: d.version.clone(),
        actual: version_visible(),
        bytes: d.bytes,
        fase,
        importante: d.importante.clone(),
        resumen: resumen(&d.notas),
    };
    let tarjeta = aviso::tarjeta(
        app.paleta(),
        &datos,
        aviso::Mensajes {
            minimizar: m(Accion::Minimizar),
            ahora: m(Accion::Ahora),
            al_cerrar: m(Accion::AlCerrar),
            cancelar: m(Accion::Cancelar),
            reiniciar: m(Accion::Ahora),
            reintentar: m(Accion::Ahora),
            web: m(Accion::Web),
            novedades: m(Accion::Novedades),
        },
    );
    // Al abrirla desde la pastilla se descubre desde abajo y se asienta.
    Some(icaro_ui::efectos::descubrir_desde_abajo(tarjeta, progreso_mostrada(app)))
}

/// Avance (0 a 1) de la aparición de la tarjeta tras pulsar la pastilla.
fn progreso_mostrada(app: &App) -> f32 {
    if app.reducir {
        return 1.0;
    }
    app.actualizacion.mostrada.map_or(1.0, |i| {
        (app.ahora.saturating_duration_since(i).as_millis() as f32 / icaro_ui::tema::duracion::LOSA as f32).clamp(0.0, 1.0)
    })
}

/// La tarjeta se está descubriendo y pide cuadros.
pub fn animando(app: &App) -> bool {
    app.actualizacion.mostrada.is_some() && progreso_mostrada(app) < 1.0
}

/// Pastilla del pie de la barra lateral: con la tarjeta minimizada o con una
/// actualización lista para aplicar.
pub fn pastilla(app: &App) -> Option<Element<'_, Mensaje>> {
    let e = &app.actualizacion;
    let d = e.disponible.as_ref()?;
    let fase = fase(app)?;
    let minimizada = e.minimizada.is_some() && e.minimizada == clave(e);
    let lista = matches!(fase, aviso::Fase::Lista | aviso::Fase::EsperaJuego { .. });
    if !minimizada && !lista {
        return None;
    }
    Some(aviso::pastilla(
        app.paleta(),
        &d.version,
        lista,
        app.barra_colapsada(),
        m(Accion::Mostrar),
    ))
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
    let acciones: Element<'_, Mensaje> = if e.resultado.as_ref().is_some_and(|resultado| resultado.correcto) {
        row![componentes::boton(p, "Continuar", Variante::Primario, Some(m(Accion::MasTarde)))]
            .spacing(espacio::S2).into()
    } else if e.disponible.is_some() {
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

/// AvisoActualizacion sobre Instancias: `aviso-actualizacion-<estado>` con
/// disponible, descargando, lista, juego, importante, error o minimizado.
pub fn preparar_diseno_aviso(app: &mut App, escena: &str) {
    preparar_diseno(app);
    app.pantalla = None;
    let e = &mut app.actualizacion;
    if let Some(d) = e.disponible.as_mut() {
        d.version = "0.4.1".into();
        d.notas[0].titulo = Some("Sincronizar mods con un servidor".into());
        d.notas[1].titulo = Some("Estudio de skin".into());
    }
    match escena.trim_start_matches("aviso-actualizacion-") {
        "descargando" => {
            e.descargando = true;
            e.total = 14_889_779;
            e.hecho = 9_231_663;
            e.inicio = Some(std::time::Instant::now() - std::time::Duration::from_secs(2));
        }
        "lista" => e.diseno_lista = true,
        "juego" => {
            e.diseno_lista = true;
            e.diseno_juego = true;
        }
        "importante" => {
            if let Some(d) = e.disponible.as_mut() {
                d.importante = Some(
                    "Corrige un fallo que podía dañar mundos al cerrar Minecraft de golpe. Conviene actualizar hoy."
                        .into(),
                );
            }
        }
        "error" => {
            e.fallo_descarga = true;
            e.error = Some("GitHub no responde.".into());
        }
        "minimizado" => e.minimizada = clave(e),
        _ => {}
    }
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
    fn minimizar_deja_solo_la_pastilla_hasta_otra_version() {
        let mut app = app();
        preparar_diseno_aviso(&mut app, "aviso-actualizacion-disponible");
        assert!(tarjeta(&app).is_some() && pastilla(&app).is_none());
        let _ = actualizar(&mut app, Accion::Minimizar);
        assert!(tarjeta(&app).is_none() && pastilla(&app).is_some());
        // Otra versión vuelve a mostrar la tarjeta.
        app.actualizacion.disponible.as_mut().unwrap().version = "0.4.2".into();
        assert!(tarjeta(&app).is_some());
        let _ = actualizar(&mut app, Accion::Minimizar);
        let _ = actualizar(&mut app, Accion::Mostrar);
        assert!(tarjeta(&app).is_some());
    }

    #[test]
    fn una_importante_no_se_esconde_y_un_fallo_de_comprobacion_no_es_de_descarga() {
        let mut app = app();
        preparar_diseno_aviso(&mut app, "aviso-actualizacion-importante");
        let _ = actualizar(&mut app, Accion::Minimizar);
        assert!(tarjeta(&app).is_some());
        let mut app = super::tests::app();
        preparar_diseno_aviso(&mut app, "aviso-actualizacion-disponible");
        app.actualizacion.error = Some("Sin red".into());
        assert_eq!(fase(&app), Some(aviso::Fase::Disponible));
        app.actualizacion.fallo_descarga = true;
        assert!(matches!(fase(&app), Some(aviso::Fase::Error(_))));
    }

    #[test]
    fn cancelar_la_descarga_vuelve_a_disponible() {
        let mut app = app();
        preparar_diseno_aviso(&mut app, "aviso-actualizacion-descargando");
        let token = tokio_util::sync::CancellationToken::new();
        app.actualizacion.cancelar = Some(token.clone());
        let _ = actualizar(&mut app, Accion::Cancelar);
        assert!(token.is_cancelled());
        let _ = actualizar(&mut app, Accion::Cancelada);
        assert!(!app.actualizacion.descargando);
        assert_eq!(fase(&app), Some(aviso::Fase::Disponible));
    }

    #[test]
    fn el_resumen_nombra_las_nuevas_y_resume_lo_demas() {
        let nota = |tipo: &str, texto: &str, titulo: Option<&str>| actualizacion::Nota {
            tipo: tipo.into(),
            texto: texto.into(),
            titulo: titulo.map(Into::into),
        };
        let r = resumen(&[
            nota("nuevo", "Transiciones suaves al pulsar Jugar.", Some("Transiciones suaves")),
            nota("nuevo", "Saludo de inicio: una frase arriba de Instancias.", None),
            nota("mejora", "La revisión agrupa por tipo.", None),
            nota("mejora", "Códigos más flexibles.", None),
            nota("correccion", "Descargas pausadas.", None),
        ]);
        assert_eq!((r.nuevas, r.mejoras, r.correcciones), (2, 2, 1));
        assert_eq!(r.linea_nuevas, "Transiciones suaves y saludo de inicio");
        assert_eq!(r.linea_mejoras, "La revisión agrupa por tipo y más");
        assert_eq!(r.linea_correcciones, "Descargas pausadas");
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
