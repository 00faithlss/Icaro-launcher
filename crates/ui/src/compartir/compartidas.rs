// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! InstanciasCompartidas: página de Servidores con lo que compartes y lo que
//! recibes, en dos pestañas. Cada fila tiene una sola acción principal y un menú.

use super::{comun::*, *};
use crate::{
    componentes::{Estado, Variante},
    estilo, fuentes,
    iconos::{Icono, Tam},
    superficies::{menu_contextual, ElementoMenu},
    tema::Paleta,
};
use iced::{
    widget::{button, column, container, row, text, Column, Space},
    Alignment, Background, Border, Color, Element, Length, Padding,
};

/// Ancho de las columnas Código y Acción de cada fila (`190px` y `210px`).
const ANCHO_CODIGO: f32 = 190.;
const ANCHO_ACCION: f32 = 210.;

fn caja(fondo: Option<Color>, borde: Color) -> impl Fn(&iced::Theme) -> container::Style {
    move |_| container::Style {
        background: fondo.map(Background::Color),
        border: Border {
            color: borde,
            width: 1.,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

fn t<'a>(
    s: impl Into<String>,
    f: iced::Font,
    tam: f32,
    alto: f32,
    c: Color,
) -> Element<'a, Accion> {
    text(s.into())
        .font(f)
        .size(tam)
        .line_height(iced::Pixels(alto))
        .color(c)
        .into()
}

/// Nombre de la instancia como enlace a su publicación.
fn nombre_enlace<'a>(p: Paleta, nombre: &str, accion: Option<Accion>) -> Element<'a, Accion> {
    let texto = t(nombre.to_owned(), fuentes::CUERPO_NEGRITA, 16., 22., p.text);
    match accion {
        None => texto,
        Some(a) => button(texto)
            .padding(0)
            .on_press(a)
            .style(estilo::sin_estilo(p.text))
            .into(),
    }
}

/// Botón de solo icono, 28 px, centrado en ambos ejes.
fn icono_boton<'a>(p: Paleta, i: Icono, accion: Accion) -> Element<'a, Accion> {
    button(container(crate::botones::icono_heredado(i, Tam::Base)).center(Length::Fill))
        .width(28)
        .height(28)
        .padding(0)
        .on_press(accion)
        .style(estilo::boton_fantasma(p))
        .into()
}

fn menu_fila<'a>(
    p: Paleta,
    id: &str,
    abierto: bool,
    opciones: Vec<ElementoMenu<Accion>>,
) -> Element<'a, Accion> {
    let accion = if abierto {
        Accion::MenuCompartida(None)
    } else {
        Accion::MenuCompartida(Some(id.to_owned()))
    };
    crate::menu_anclado::menu(
        icono_boton(p, Icono::Acciones, accion),
        menu_contextual(p, opciones),
        abierto,
        Accion::MenuCompartida(None),
    )
}

fn opcion(i: Icono, texto: &'static str, peligro: bool, accion: Accion) -> ElementoMenu<Accion> {
    ElementoMenu::Opcion {
        icono: i,
        texto: texto.into(),
        atajo: None,
        peligro,
        mensaje: accion,
    }
}

/// Fila `ic-ic__fila`: icono, datos, columna de código y acción.
fn fila<'a>(
    p: Paleta,
    nombre: &str,
    datos: Column<'a, Accion>,
    codigo: Element<'a, Accion>,
    accion: Element<'a, Accion>,
) -> Element<'a, Accion> {
    container(
        row![
            crate::servidores::avatar_servidor(p, nombre),
            datos.spacing(4).width(Length::Fill),
            container(codigo).width(ANCHO_CODIGO),
            container(accion)
                .width(ANCHO_ACCION)
                .align_x(Alignment::End),
        ]
        .spacing(24)
        .align_y(Alignment::Center),
    )
    .padding([18., 20.])
    .width(Length::Fill)
    .style(caja(Some(p.surface), p.border_strong))
    .into()
}

fn columna_dato<'a>(
    p: Paleta,
    etiqueta: &str,
    filas: Vec<Element<'a, Accion>>,
) -> Element<'a, Accion> {
    let mut c = column![t(
        etiqueta.to_uppercase(),
        fuentes::ETIQUETA,
        11.,
        14.,
        p.text_muted
    )]
    .spacing(2);
    for f in filas {
        c = c.push(f);
    }
    c.into()
}

fn fila_compartes<'a>(
    p: Paleta,
    m: &InstanciasCompartidas,
    f: &InstanciaQueCompartes,
) -> Element<'a, Accion> {
    let (estado, insignia) = match f.estado {
        EstadoCompartida::Disponible => (Estado::Exito, "Disponible"),
        EstadoCompartida::CambiosSinPublicar => (Estado::Info, "Cambios sin publicar"),
        EstadoCompartida::EnPausa => (Estado::Aviso, "En pausa"),
    };
    let personas = match f.personas {
        1 => "1 persona".to_string(),
        n => format!("{n} personas"),
    };
    let datos = column![
        row![
            nombre_enlace(p, &f.nombre, Some(Accion::VerPublicacionDe(f.id.clone()))),
            insignia_plana(p, estado, insignia)
        ]
        .spacing(10)
        .align_y(Alignment::Center)
        .wrap(),
        t(
            format!("Revisión {} · {personas}", f.revision),
            fuentes::CUERPO,
            13.,
            19.,
            p.text_muted
        ),
        t(f.version.clone(), fuentes::MONO, 11., 16., p.text_muted),
    ];
    let codigo = match &f.codigo {
        Some(c) => columna_dato(
            p,
            "Código",
            vec![
                row![
                    t(c.clone(), fuentes::MONO, 13., 20., p.text),
                    icono_boton(
                        p,
                        if f.copiado {
                            Icono::Check
                        } else {
                            Icono::Copiar
                        },
                        Accion::CopiarCodigoDe(f.id.clone())
                    )
                ]
                .spacing(4)
                .align_y(Alignment::Center)
                .into(),
                t(f.caduca.clone(), fuentes::MONO, 11., 16., p.text_muted),
            ],
        ),
        None => columna_dato(
            p,
            "Código",
            vec![t("Código anulado", fuentes::CUERPO, 13., 19., p.text_muted)],
        ),
    };
    let principal = match f.estado {
        EstadoCompartida::Disponible => btn_sm(
            p,
            None,
            "Ver publicación",
            Some(Icono::Adelante),
            Variante::Secundario,
            Accion::VerPublicacionDe(f.id.clone()),
        ),
        EstadoCompartida::CambiosSinPublicar => btn_sm(
            p,
            None,
            "Revisar cambios",
            None,
            Variante::Primario,
            Accion::RevisarCambiosDe(f.id.clone()),
        ),
        EstadoCompartida::EnPausa => btn_sm(
            p,
            Some(Icono::Jugar),
            "Reanudar",
            None,
            Variante::Secundario,
            Accion::ReanudarDe(f.id.clone()),
        ),
    };
    let activa = f.estado != EstadoCompartida::EnPausa;
    let mut opciones = vec![opcion(
        Icono::Externo,
        "Ver publicación",
        false,
        Accion::VerPublicacionDe(f.id.clone()),
    )];
    // Pausar, cambiar código y dejar de compartir actúan sobre la publicación activa.
    if activa {
        opciones.push(opcion(
            Icono::Pausa,
            "Pausar",
            false,
            Accion::PausarDe(f.id.clone()),
        ));
        opciones.push(opcion(
            Icono::Refrescar,
            "Cambiar código",
            false,
            Accion::CambiarCodigoDe(f.id.clone()),
        ));
        opciones.push(ElementoMenu::Separador);
        opciones.push(opcion(
            Icono::Detener,
            "Dejar de compartir",
            true,
            Accion::DejarDeCompartirDe(f.id.clone()),
        ));
    }
    let accion = row![
        principal,
        menu_fila(p, &f.id, m.menu.as_deref() == Some(f.id.as_str()), opciones)
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    fila(p, &f.nombre, datos, codigo, accion.into())
}

fn fila_recibes<'a>(
    p: Paleta,
    m: &InstanciasCompartidas,
    f: &InstanciaQueRecibes,
) -> Element<'a, Accion> {
    let mut cabeza = row![
        nombre_enlace(p, &f.nombre, None),
        t(
            format!("de {}", f.anfitrion),
            fuentes::CUERPO,
            13.,
            19.,
            p.text_muted
        )
    ]
    .spacing(10)
    .align_y(Alignment::Center);
    let nuevos = f.cambios.is_some_and(|n| n > 0);
    if nuevos {
        cabeza = cabeza.push(insignia_plana(p, Estado::Info, "Cambios nuevos"));
    }
    let (punto, estado) = match f.estado {
        AnfitrionRecibida::Disponible => (p.success, "Anfitrión disponible"),
        AnfitrionRecibida::SinConexion => (p.text_disabled, "Anfitrión sin conexión"),
        AnfitrionRecibida::Comprobando => (p.text_disabled, "Comprobando al anfitrión…"),
    };
    let datos = column![
        cabeza.wrap(),
        row![
            container(Space::new(8, 8)).style(estilo::bloque(punto)),
            t(estado, fuentes::CUERPO, 13., 19., p.text_muted)
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        t(f.version.clone(), fuentes::MONO, 11., 16., p.text_muted),
    ];
    let revision = match (f.revision, f.cambios) {
        (Some(r), Some(0)) => format!("Revisión {r} · al día"),
        (Some(r), Some(1)) => format!("Revisión {r} · 1 cambio"),
        (Some(r), Some(n)) => format!("Revisión {r} · {n} cambios"),
        (Some(r), None) => format!("Revisión {r}"),
        (None, _) => "Sin instalar".into(),
    };
    let codigo = columna_dato(
        p,
        "Revisión",
        vec![t(revision, fuentes::CUERPO, 13., 19., p.text)],
    );
    // Sin conexión se puede jugar con lo que ya se tiene.
    let principal = if nuevos {
        btn_sm(
            p,
            Some(Icono::Refrescar),
            "Sincronizar",
            None,
            Variante::Primario,
            Accion::SincronizarRecibida(f.id.clone()),
        )
    } else {
        btn_sm(
            p,
            Some(Icono::Jugar),
            "Jugar",
            None,
            Variante::Primario,
            Accion::JugarRecibida(f.id.clone()),
        )
    };
    let opciones = vec![opcion(
        Icono::Refrescar,
        "Sincronizar",
        false,
        Accion::SincronizarRecibida(f.id.clone()),
    )];
    let accion = row![
        principal,
        menu_fila(p, &f.id, m.menu.as_deref() == Some(f.id.as_str()), opciones)
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    fila(p, &f.nombre, datos, codigo, accion.into())
}

/// `ic-btn--sm` (28 px) con icono opcional a cada lado.
fn btn_sm<'a>(
    p: Paleta,
    izquierda: Option<Icono>,
    rotulo: &str,
    derecha: Option<Icono>,
    v: Variante,
    accion: Accion,
) -> Element<'a, Accion> {
    let mut contenido = row![].spacing(4).align_y(Alignment::Center);
    if let Some(i) = izquierda {
        contenido = contenido.push(crate::botones::icono_heredado(i, Tam::Base));
    }
    contenido = contenido.push(
        text(rotulo.to_uppercase())
            .font(fuentes::ETIQUETA)
            .size(11)
            .wrapping(iced::widget::text::Wrapping::None),
    );
    if let Some(i) = derecha {
        contenido = contenido.push(crate::botones::icono_heredado(i, Tam::Base));
    }
    let b = button(container(contenido).center_y(Length::Fill))
        .height(28)
        .padding(Padding {
            top: 0.,
            right: if derecha.is_some() { 8. } else { 12. },
            bottom: 0.,
            left: if izquierda.is_some() { 8. } else { 12. },
        })
        .on_press(accion);
    match v {
        Variante::Primario => b.style(estilo::boton_primario(p)),
        Variante::Secundario => b.style(estilo::boton_secundario(p)),
        Variante::Fantasma => b.style(estilo::boton_fantasma(p)),
        Variante::Peligro => b.style(estilo::boton_peligro(p)),
    }
    .into()
}

fn pestana<'a>(
    p: Paleta,
    nombre: &str,
    n: usize,
    activa: bool,
    accion: Accion,
) -> Element<'a, Accion> {
    let color = if activa { p.text } else { p.text_muted };
    let cuenta = container(t(n.to_string(), fuentes::MONO, 11., 18., color))
        .padding([0., 6.])
        .style(move |_: &iced::Theme| container::Style {
            border: Border {
                color,
                width: 1.,
                radius: 0.0.into(),
            },
            ..Default::default()
        });
    let contenido = container(
        row![
            t(nombre.to_owned(), fuentes::CUERPO_NEGRITA, 14., 20., color),
            cuenta
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .center_y(Length::Fill)
    .padding([0., 16.]);
    // Subrayado de 2 px en la activa, sobre la línea de 1 px de las pestañas.
    let subrayado = container(Space::new(Length::Fill, 2)).style(estilo::bloque(if activa {
        p.text
    } else {
        Color::TRANSPARENT
    }));
    button(column![container(contenido).height(42), subrayado].width(Length::Shrink))
        .padding(0)
        .on_press(accion)
        .style(move |_, s| button::Style {
            text_color: if matches!(s, button::Status::Hovered) {
                p.text
            } else {
                color
            },
            ..Default::default()
        })
        .into()
}

fn vacio<'a>(
    p: Paleta,
    titulo: &str,
    detalle: &str,
    boton: Element<'a, Accion>,
) -> Element<'a, Accion> {
    container(
        column![
            t(titulo.to_owned(), fuentes::CUERPO_NEGRITA, 16., 22., p.text),
            t(detalle.to_owned(), fuentes::CUERPO, 13., 19., p.text_muted),
            container(boton).padding(Padding {
                top: 8.,
                ..Padding::ZERO
            }),
        ]
        .spacing(8),
    )
    .padding(40)
    .width(Length::Fill)
    .style(caja(Some(p.surface_sunken), p.border))
    .into()
}

/// Página completa: migas, cabecera, pestañas y lista, centradas a 1080 px.
pub fn instancias_compartidas<'a>(p: Paleta, m: &InstanciasCompartidas) -> Element<'a, Accion> {
    let migas = super::migas(
        p,
        &[
            super::Miga::enlace("Servidores", Accion::VerServidores),
            super::Miga::actual("Instancias compartidas"),
        ],
    );
    let cabecera = column![
        row![
            column![
                t("Instancias compartidas", fuentes::TITULO, 28., 32., p.text),
                t(
                    "Lo que compartes con tus amigos y lo que ellos comparten contigo.",
                    fuentes::CUERPO,
                    13.,
                    19.,
                    p.text_muted
                ),
            ]
            .spacing(6)
            .width(Length::Fill),
            btn(
                p,
                Some(Icono::Llave),
                "Unirme con un código",
                None,
                Variante::Secundario,
                Some(Accion::AceptarInvitacion)
            ),
            btn(
                p,
                Some(Icono::Subir),
                "Compartir instancia",
                None,
                Variante::Primario,
                Some(Accion::AbrirCompartir)
            ),
        ]
        .spacing(8)
        .align_y(Alignment::End),
        Space::with_height(20),
        regla(p, true),
    ];
    // La línea de 1 px va debajo y el subrayado de 2 px de la activa la cubre.
    let pestanas = iced::widget::stack![
        column![Space::new(Length::Fill, 43), regla(p, false)],
        row![
            pestana(
                p,
                "Las que compartes",
                m.compartes.len(),
                m.pestana == PestanaCompartidas::Compartes,
                Accion::PestanaCompartidas(PestanaCompartidas::Compartes)
            ),
            pestana(
                p,
                "Las que recibes",
                m.recibes.len(),
                m.pestana == PestanaCompartidas::Recibes,
                Accion::PestanaCompartidas(PestanaCompartidas::Recibes)
            ),
        ]
        .spacing(4),
    ];
    let lista: Element<'a, Accion> = match m.pestana {
        PestanaCompartidas::Compartes if m.compartes.is_empty() => vacio(
            p,
            "No compartes ninguna instancia",
            "Comparte una y tus amigos tendrán los mismos mods para jugar contigo. Tú decides qué se publica.",
            btn(
                p,
                Some(Icono::Subir),
                "Compartir instancia",
                None,
                Variante::Primario,
                Some(Accion::AbrirCompartir),
            ),
        ),
        PestanaCompartidas::Recibes if m.recibes.is_empty() => vacio(
            p,
            "No recibes instancias de nadie",
            "Si un amigo te envía un código, úsalo en Unirme con un código.",
            btn(
                p,
                Some(Icono::Llave),
                "Unirme con un código",
                None,
                Variante::Secundario,
                Some(Accion::AceptarInvitacion),
            ),
        ),
        PestanaCompartidas::Compartes => Column::with_children(
            m.compartes.iter().map(|f| fila_compartes(p, m, f)),
        )
        .spacing(8)
        .into(),
        PestanaCompartidas::Recibes => Column::with_children(
            m.recibes.iter().map(|f| fila_recibes(p, m, f)),
        )
        .spacing(8)
        .into(),
    };
    let pagina = column![migas, cabecera, pestanas, lista]
        .spacing(24)
        .width(Length::Fill)
        .max_width(1080)
        .padding(Padding {
            top: 8.,
            right: 0.,
            bottom: 48.,
            left: 0.,
        });
    container(crate::scroll::desplazable(
        container(pagina).center_x(Length::Fill).padding([32., 48.]),
    ))
    .width(Length::Fill)
    .height(Length::Fill)
    .style(estilo::bloque(p.bg))
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn modelo() -> InstanciasCompartidas {
        InstanciasCompartidas {
            pestana: PestanaCompartidas::Compartes,
            compartes: vec![InstanciaQueCompartes {
                id: "a".into(),
                nombre: "Amigos".into(),
                estado: EstadoCompartida::Disponible,
                revision: 4,
                personas: 3,
                version: "1.21.1 · Fabric 0.16.9".into(),
                codigo: Some("7QKM-4XPT-R2C9".into()),
                caduca: "caduca en 23 h".into(),
                copiado: false,
            }],
            recibes: vec![InstanciaQueRecibes {
                id: "b".into(),
                nombre: "Survival 26".into(),
                anfitrion: "Equipo 9e2a 7b14".into(),
                estado: AnfitrionRecibida::Disponible,
                revision: Some(9),
                cambios: Some(0),
                version: "1.21.1 · Fabric 0.16.9".into(),
            }],
            menu: Some("a".into()),
        }
    }

    #[test]
    fn la_pagina_se_construye_con_filas_y_vacios_en_ambas_pestanas() {
        let mut m = modelo();
        for pestana in [PestanaCompartidas::Compartes, PestanaCompartidas::Recibes] {
            m.pestana = pestana;
            let _ = instancias_compartidas(crate::tema::TINTA, &m);
            let _ = instancias_compartidas(
                crate::tema::PIEDRA,
                &InstanciasCompartidas {
                    pestana,
                    ..Default::default()
                },
            );
        }
    }
}
