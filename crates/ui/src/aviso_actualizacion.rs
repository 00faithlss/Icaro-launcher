// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! AvisoActualizacion: tarjeta flotante abajo a la derecha que avisa de una
//! versión nueva de Ícaro sin tapar la navegación, y la pastilla que queda en
//! el pie de la barra lateral al minimizarla. Borde de 2 px y sombra dura de
//! 6 px, como las losas.

use iced::widget::{button, column, container, row, text, Space};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Shadow, Theme, Vector};

use crate::botones;
use crate::componentes::{self, Estado, Variante};
use crate::fuentes;
use crate::iconos::{icono, Icono, Tam};
use crate::tema::{borde, espacio, texto, Paleta};

/// Ancho de la tarjeta.
pub const ANCHO: f32 = 420.0;

/// En qué punto está la actualización.
#[derive(Debug, Clone, PartialEq)]
pub enum Fase {
    Disponible,
    Descargando {
        hecho: u64,
        total: u64,
        /// Bytes por segundo.
        velocidad: u64,
    },
    /// Descargada y verificada: falta reiniciar.
    Lista,
    /// Lista, pero Minecraft está abierto: se aplica al cerrar.
    EsperaJuego {
        instancia: Option<String>,
    },
    Error(String),
}

/// Una fila del resumen: cantidad y una línea para quien juega.
#[derive(Debug, Clone, PartialEq)]
pub struct Resumen {
    pub nuevas: usize,
    pub mejoras: usize,
    pub correcciones: usize,
    pub linea_nuevas: String,
    pub linea_mejoras: String,
    pub linea_correcciones: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Datos {
    /// Versión nueva, «0.4.1».
    pub version: String,
    /// Versión instalada, «0.4.0».
    pub actual: String,
    pub bytes: u64,
    pub fase: Fase,
    /// Motivo de una actualización importante (daño de datos o seguridad).
    pub importante: Option<String>,
    pub resumen: Resumen,
}

pub struct Mensajes<M> {
    /// Más tarde: queda como pastilla. `None` en las importantes.
    pub minimizar: M,
    pub ahora: M,
    pub al_cerrar: M,
    pub cancelar: M,
    pub reiniciar: M,
    pub reintentar: M,
    pub web: M,
    pub novedades: M,
}

/// «18,6 MB».
pub fn megas(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / 1_048_576.0).replace('.', ",")
}

fn cuerpo_sm<'a>(s: impl text::IntoFragment<'a>, color: Color) -> text::Text<'a> {
    text(s)
        .font(fuentes::CUERPO)
        .size(texto::BODY_SM.0)
        .line_height(text::LineHeight::Absolute(texto::BODY_SM.1.into()))
        .color(color)
}

fn mono<'a>(s: impl text::IntoFragment<'a>, color: Color) -> text::Text<'a> {
    text(s)
        .font(fuentes::MONO)
        .size(texto::MONO_SM.0)
        .line_height(text::LineHeight::Absolute(texto::MONO_SM.1.into()))
        .color(color)
}

/// Cuadro de 28 px con el icono de la marca.
fn marca<'a, M: 'a>(fondo: Color, tinta: Color, glifo: Icono) -> Element<'a, M> {
    container(icono(glifo, Tam::Base, tinta))
        .center(28)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fondo)),
            ..Default::default()
        })
        .into()
}

/// Barra continua de 6 px.
fn barra<'a, M: 'a>(p: Paleta, avance: f32) -> Element<'a, M> {
    let avance = avance.clamp(0.0, 1.0);
    let lleno: Element<'a, M> = if avance > 0.0 {
        container(Space::new(Length::Fill, 6))
            .width(Length::FillPortion((avance * 1000.0) as u16 + 1))
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(p.text)),
                ..Default::default()
            })
            .into()
    } else {
        Space::new(0, 6).into()
    };
    container(
        row![
            lleno,
            Space::new(Length::FillPortion(((1.0 - avance) * 1000.0) as u16 + 1), 6)
        ]
        .width(Length::Fill),
    )
    .width(Length::Fill)
    .height(6)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(p.surface_sunken)),
        border: Border {
            color: p.border,
            width: borde::FINO,
            radius: 0.0.into(),
        },
        ..Default::default()
    })
    .into()
}

fn enlace<'a, M: Clone + 'a>(p: Paleta, rotulo: &str, al_pulsar: M) -> Element<'a, M> {
    button(
        row![
            text(rotulo.to_owned())
                .font(fuentes::CUERPO)
                .size(texto::BODY_SM.0)
                .color(p.text),
            icono(Icono::Adelante, Tam::Sm, p.text),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding(0)
    .on_press(al_pulsar)
    .style(move |_, estado| button::Style {
        text_color: p.text,
        background: matches!(estado, button::Status::Hovered | button::Status::Pressed)
            .then_some(Background::Color(p.surface_hover)),
        ..Default::default()
    })
    .into()
}

/// La tarjeta flotante con el estado de la actualización.
pub fn tarjeta<'a, M: Clone + 'a>(p: Paleta, d: &Datos, m: Mensajes<M>) -> Element<'a, M> {
    let error = matches!(d.fase, Fase::Error(_));
    let importante = d.importante.is_some() && matches!(d.fase, Fase::Disponible);
    let (fondo, tinta, glifo) = if error {
        (p.error, p.on_error, Icono::Alerta)
    } else if importante {
        (p.warning, p.bg, Icono::Aviso)
    } else {
        (p.text, p.bg, Icono::Descargas)
    };
    let mut cab = row![
        marca(fondo, tinta, glifo),
        container(
            text("ACTUALIZACIÓN DE ÍCARO")
                .font(fuentes::ETIQUETA)
                .size(texto::LABEL.0)
                .color(p.text_muted)
        )
        .width(Length::Fill),
    ]
    .spacing(espacio::S3)
    .align_y(Alignment::Center);
    // Una importante no se puede dejar para más tarde.
    if !importante {
        cab = cab.push(botones::boton_icono(
            p,
            Icono::Menos,
            "Más tarde",
            m.minimizar.clone(),
        ));
    }
    let cab = container(cab).padding(Padding {
        top: espacio::S3,
        right: espacio::S3,
        bottom: espacio::S3,
        left: espacio::S5,
    });

    let titulo = match &d.fase {
        Fase::Disponible => format!("Ícaro {} está disponible", d.version),
        Fase::Descargando { .. } => format!("Descargando Ícaro {}", d.version),
        Fase::Lista => format!("Ícaro {} está lista", d.version),
        Fase::EsperaJuego { .. } => format!("Ícaro {} espera a Minecraft", d.version),
        Fase::Error(_) => format!("No se pudo descargar la {}", d.version),
    };
    let mut encabezado = column![text(titulo)
        .font(fuentes::TITULO)
        .size(22)
        .line_height(text::LineHeight::Absolute(26.0.into()))
        .color(p.text)]
    .spacing(6);
    match d.fase {
        Fase::Disponible => {
            encabezado = encabezado.push(mono(
                format!("Tienes {} · descarga de {}", d.actual, megas(d.bytes)),
                p.text_muted,
            ))
        }
        Fase::Lista | Fase::EsperaJuego { .. } => {
            encabezado = encabezado.push(mono(
                format!("Descargada · {}", megas(d.bytes)),
                p.text_muted,
            ))
        }
        _ => {}
    }
    let mut cuerpo = column![encabezado].spacing(18);

    if let (true, Some(motivo)) = (importante, &d.importante) {
        cuerpo = cuerpo.push(
            container(
                column![
                    componentes::insignia(p, Estado::Aviso, "Importante"),
                    cuerpo_sm(motivo.clone(), p.text),
                ]
                .spacing(espacio::S2),
            )
            .width(Length::Fill)
            .padding(Padding {
                top: espacio::S3,
                right: 14.0,
                bottom: espacio::S3,
                left: 14.0 + borde::GRUESO,
            })
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(p.warning_soft)),
                border: Border {
                    color: p.warning,
                    width: 0.0,
                    radius: 0.0.into(),
                },
                ..Default::default()
            }),
        );
    }

    match &d.fase {
        Fase::Disponible => {
            let r = &d.resumen;
            let mut lista = column![].spacing(10);
            for (n, palabra, linea, estado) in [
                (
                    r.nuevas,
                    if r.nuevas == 1 { "nueva" } else { "nuevas" },
                    &r.linea_nuevas,
                    Some(Estado::Exito),
                ),
                (
                    r.mejoras,
                    if r.mejoras == 1 { "mejora" } else { "mejoras" },
                    &r.linea_mejoras,
                    Some(Estado::Info),
                ),
                (
                    r.correcciones,
                    if r.correcciones == 1 {
                        "corrección"
                    } else {
                        "correcciones"
                    },
                    &r.linea_correcciones,
                    None,
                ),
            ] {
                if n == 0 {
                    continue;
                }
                let rotulo = format!("{n} {palabra}");
                let insignia = match estado {
                    Some(e) => componentes::insignia(p, e, &rotulo),
                    None => componentes::insignia_contorno(p, &rotulo),
                };
                lista = lista.push(
                    row![
                        container(insignia).width(132),
                        container(cuerpo_sm(linea.clone(), p.text)).width(Length::Fill),
                    ]
                    .spacing(espacio::S3),
                );
            }
            cuerpo = cuerpo.push(lista);
        }
        Fase::Descargando {
            hecho,
            total,
            velocidad,
        } => {
            let restante = if *velocidad > 0 {
                let s = total.saturating_sub(*hecho) / velocidad;
                format!(" · {s} s")
            } else {
                String::new()
            };
            cuerpo = cuerpo.push(
                column![
                    row![
                        mono(format!("{} de {}", megas(*hecho), megas(*total)), p.text),
                        Space::with_width(Length::Fill),
                        mono(format!("{}/s{restante}", megas(*velocidad)), p.text_muted),
                    ],
                    barra(
                        p,
                        if *total == 0 {
                            0.0
                        } else {
                            *hecho as f32 / *total as f32
                        }
                    ),
                    cuerpo_sm("Puedes seguir usando Ícaro mientras tanto.", p.text_muted),
                ]
                .spacing(espacio::S2),
            );
        }
        Fase::Lista => {
            cuerpo = cuerpo.push(cuerpo_sm(
                "Reinicia Ícaro para aplicarla. Tarda unos segundos y vuelves a donde estabas.",
                p.text_muted,
            ))
        }
        Fase::EsperaJuego { instancia } => {
            let juego = match instancia {
                Some(i) => format!("Minecraft está abierto con {i}."),
                None => "Minecraft está abierto.".to_owned(),
            };
            cuerpo = cuerpo.push(cuerpo_sm(
                format!(
                    "{juego} La {} se aplica cuando cierres el juego y Ícaro; tu partida no se interrumpe.",
                    d.version
                ),
                p.text_muted,
            ))
        }
        Fase::Error(motivo) => {
            cuerpo = cuerpo.push(cuerpo_sm(
                format!("{motivo} Ícaro sigue funcionando con la {}.", d.actual),
                p.text_muted,
            ))
        }
    }
    if matches!(
        d.fase,
        Fase::Disponible | Fase::Lista | Fase::EsperaJuego { .. }
    ) {
        cuerpo = cuerpo.push(enlace(p, "Ver todo lo que cambia", m.novedades.clone()));
    }
    let cuerpo = container(cuerpo).width(Length::Fill).padding(Padding {
        top: espacio::S5,
        right: espacio::S5,
        bottom: espacio::S1,
        left: espacio::S5,
    });

    let sm = |rotulo: &str, glifo: Option<Icono>, v: Variante, msg: M| -> Element<'a, M> {
        match glifo {
            Some(g) => componentes::boton_con_icono(p, g, rotulo, v, Some(msg), true),
            None => componentes::boton_compacto(p, rotulo, v, Some(msg)),
        }
    };
    let acciones: Element<'a, M> = match &d.fase {
        Fase::Disponible => row![
            sm(
                "Actualizar ahora",
                Some(Icono::Descargas),
                Variante::Primario,
                m.ahora
            ),
            sm("Al cerrar Ícaro", None, Variante::Secundario, m.al_cerrar),
        ]
        .spacing(espacio::S2)
        .into(),
        Fase::Descargando { .. } => sm("Cancelar", None, Variante::Secundario, m.cancelar),
        Fase::Lista => row![
            sm(
                "Reiniciar ahora",
                Some(Icono::Refrescar),
                Variante::Primario,
                m.reiniciar
            ),
            sm("Al cerrar Ícaro", None, Variante::Secundario, m.al_cerrar),
        ]
        .spacing(espacio::S2)
        .into(),
        Fase::EsperaJuego { .. } => sm("Entendido", None, Variante::Primario, m.minimizar),
        Fase::Error(_) => row![
            sm(
                "Reintentar",
                Some(Icono::Refrescar),
                Variante::Primario,
                m.reintentar
            ),
            sm(
                "Descargar desde la web",
                Some(Icono::Externo),
                Variante::Secundario,
                m.web
            ),
        ]
        .spacing(espacio::S2)
        .into(),
    };
    let pie = container(acciones).padding(Padding {
        top: espacio::S4,
        right: espacio::S5,
        bottom: espacio::S5,
        left: espacio::S5,
    });

    container(column![
        cab,
        crate::descargas::regla(borde::FINO, p.border),
        cuerpo,
        pie
    ])
    .width(ANCHO)
    .padding(borde::MEDIO)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(p.surface)),
        border: Border {
            color: p.text,
            width: borde::MEDIO,
            radius: 0.0.into(),
        },
        // Sombra dura de 6 px, sin desenfoque.
        shadow: Shadow {
            color: p.text,
            offset: Vector::new(6.0, 6.0),
            blur_radius: 0.0,
        },
        ..Default::default()
    })
    .into()
}

/// Pastilla del pie de la barra lateral: la tarjeta minimizada o una
/// actualización lista. Plegada la barra, queda solo la marca.
pub fn pastilla<'a, M: Clone + 'a>(
    p: Paleta,
    version: &str,
    lista: bool,
    plegada: bool,
    al_pulsar: M,
) -> Element<'a, M> {
    // Marca con el punto `info` en la esquina.
    let marca = iced::widget::stack![
        container(marca(p.accent, p.on_accent, Icono::Descargas)).padding(Padding {
            top: 4.0,
            right: 4.0,
            bottom: 0.0,
            left: 0.0,
        }),
        container(
            container(Space::new(8, 8)).style(move |_: &Theme| container::Style {
                background: Some(Background::Color(p.info)),
                border: Border {
                    color: p.surface,
                    width: borde::MEDIO,
                    radius: 0.0.into(),
                },
                ..Default::default()
            })
        )
        .width(32)
        .align_x(Alignment::End),
    ];
    let contenido: Element<'a, M> = if plegada {
        marca.into()
    } else {
        row![
            marca,
            column![
                text(if lista {
                    format!("Ícaro {version} lista")
                } else {
                    format!("Ícaro {version} disponible")
                })
                .font(fuentes::CUERPO_NEGRITA)
                .size(texto::BODY_SM.0)
                .color(p.text),
                mono(
                    if lista {
                        "Reiniciar para aplicar"
                    } else {
                        "Ver qué cambia"
                    },
                    p.text_muted
                ),
            ]
            .width(Length::Fill),
        ]
        .spacing(espacio::S3)
        .align_y(Alignment::Center)
        .into()
    };
    let b = button(container(contenido).center_y(Length::Fill))
        .width(Length::Fill)
        .height(56)
        .padding(Padding::from([0.0, espacio::S3]))
        .on_press(al_pulsar)
        .style(move |_, estado| button::Style {
            background: Some(Background::Color(
                if matches!(estado, button::Status::Hovered | button::Status::Pressed) {
                    p.surface_hover
                } else {
                    p.surface
                },
            )),
            border: Border {
                color: p.text,
                width: borde::MEDIO,
                radius: 0.0.into(),
            },
            text_color: p.text,
            ..Default::default()
        });
    if plegada {
        botones::con_tooltip(
            p,
            b.into(),
            &format!("Ícaro {version} disponible"),
            iced::widget::tooltip::Position::Right,
        )
    } else {
        b.into()
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_megas_usan_coma_decimal() {
        assert_eq!(megas(19_503_513), "18,6 MB");
        assert_eq!(megas(0), "0,0 MB");
    }
}
