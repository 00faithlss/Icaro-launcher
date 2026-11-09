// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

use super::{Accion, ArchivoCompartido, InstanciaCompartida, Marca};
use crate::{
    botones::{self, TamBoton},
    componentes::Variante,
    estilo, fuentes,
    iconos::{icono, Icono, Tam},
    tema::Paleta,
};
use iced::{
    widget::{button, column, container, row, scrollable, text, Column, Row, Space},
    Alignment, Border, Color, Element, Length, Padding,
};

pub(super) fn cuerpo<'a>(p: Paleta, s: impl Into<String>) -> Element<'a, Accion> {
    text(s.into())
        .font(fuentes::CUERPO)
        .size(13)
        .color(p.text)
        .into()
}
pub(super) fn tenue<'a>(p: Paleta, s: impl Into<String>) -> Element<'a, Accion> {
    text(s.into())
        .font(fuentes::CUERPO)
        .size(13)
        .color(p.text_muted)
        .into()
}
pub(super) fn mono<'a>(p: Paleta, s: impl Into<String>) -> Element<'a, Accion> {
    text(s.into())
        .font(fuentes::MONO)
        .size(11)
        .color(p.text_muted)
        .into()
}
pub(super) fn heading<'a>(p: Paleta, s: impl Into<String>) -> Element<'a, Accion> {
    text(s.into())
        .font(fuentes::CUERPO_NEGRITA)
        .size(14)
        .color(p.text)
        .into()
}
pub(super) fn titulo<'a>(p: Paleta, s: impl Into<String>) -> Element<'a, Accion> {
    text(s.into())
        .font(fuentes::TITULO)
        .size(24)
        .color(p.text)
        .into()
}
pub(super) fn boton<'a>(p: Paleta, s: &str, v: Variante, a: Option<Accion>) -> Element<'a, Accion> {
    botones::boton(p, None, s, v, TamBoton::Lg, a)
}
pub(super) fn pequeno<'a>(
    p: Paleta,
    s: &str,
    v: Variante,
    a: Option<Accion>,
) -> Element<'a, Accion> {
    botones::boton(p, None, s, v, TamBoton::Sm, a)
}
pub(super) fn iconoboton<'a>(
    p: Paleta,
    i: Icono,
    s: &str,
    a: Option<Accion>,
) -> Element<'a, Accion> {
    let b = button(container(botones::icono_heredado(i, Tam::Base)).center(Length::Fill))
        .width(28)
        .height(28)
        .padding(0)
        .on_press_maybe(a)
        .style(estilo::boton_secundario(p));
    botones::con_tooltip(p, b.into(), s, iced::widget::tooltip::Position::Top)
}
pub(super) fn regla<'a>(p: Paleta, fuerte: bool) -> Element<'a, Accion> {
    container(Space::new(Length::Fill, if fuerte { 2 } else { 1 }))
        .style(estilo::bloque(if fuerte { p.text } else { p.border }))
        .into()
}
pub(super) fn marco(p: Paleta) -> impl Fn(&iced::Theme) -> container::Style {
    move |_| container::Style {
        background: Some(p.surface.into()),
        text_color: Some(p.text),
        border: Border {
            color: p.border_strong,
            width: 1.,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}
pub(super) fn cabecera<'a>(p: Paleta, nombre: &str, subtitulo: &str) -> Element<'a, Accion> {
    container(
        row![
            column![titulo(p, nombre), tenue(p, subtitulo)]
                .spacing(6)
                .width(Length::Fill),
            botones::boton_icono_estilo(
                p,
                Icono::Cerrar,
                "Cerrar",
                estilo::boton_fantasma(p),
                Accion::Cerrar
            )
        ]
        .spacing(16)
        .align_y(Alignment::Start),
    )
    .padding(Padding {
        top: 28.,
        right: 32.,
        bottom: 0.,
        left: 32.,
    })
    .into()
}
pub(super) fn pie<'a>(p: Paleta, acciones: Row<'a, Accion>) -> Element<'a, Accion> {
    column![
        regla(p, false),
        container(acciones.spacing(8).align_y(Alignment::Center)).padding([20., 32.])
    ]
    .into()
}
pub(super) fn dialogo<'a>(
    p: Paleta,
    ancho: f32,
    nombre: &str,
    subtitulo: &str,
    contenido: Column<'a, Accion>,
    acciones: Row<'a, Accion>,
) -> Element<'a, Accion> {
    container(interior_dialogo(p, nombre, subtitulo, contenido, acciones))
        .width(Length::Fill)
        .max_width(ancho)
        .max_height(800)
        .style(marco_dialogo(p))
        .into()
}
pub(super) const BORDE_DIALOGO: f32 = 2.;

pub(super) fn marco_dialogo(p: Paleta) -> impl Fn(&iced::Theme) -> container::Style {
    move |_| container::Style {
        background: Some(p.surface_raised.into()),
        text_color: Some(p.text),
        border: Border {
            color: p.text,
            width: BORDE_DIALOGO,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}
pub(super) fn interior_dialogo<'a>(
    p: Paleta,
    nombre: &str,
    subtitulo: &str,
    contenido: Column<'a, Accion>,
    acciones: Row<'a, Accion>,
) -> Element<'a, Accion> {
    column![
        cabecera(p, nombre, subtitulo),
        scrollable(container(contenido.spacing(24)).padding(Padding {
            top: 24.,
            right: 32.,
            bottom: 28.,
            left: 32.
        }))
        .height(Length::Fill),
        pie(p, acciones)
    ]
    .width(Length::Fill)
    .into()
}
pub(super) fn grupo<'a>(p: Paleta, nombre: &str, detalle: &str) -> Column<'a, Accion> {
    column![
        row![
            heading(p, nombre),
            Space::new(Length::Fill, 0),
            mono(p, detalle)
        ]
        .spacing(16),
        regla(p, true)
    ]
    .spacing(10)
}
pub(super) fn dato<'a>(p: Paleta, nombre: &str, valor: &str) -> Element<'a, Accion> {
    column![
        container(row![container(tenue(p, nombre)).width(112), cuerpo(p, valor)].spacing(16))
            .padding([12., 4.]),
        regla(p, false)
    ]
    .into()
}
pub(super) fn identidad_instancia<'a>(p: Paleta, i: &InstanciaCompartida) -> Element<'a, Accion> {
    row![
        crate::servidores::avatar_servidor(p, &i.nombre),
        column![
            heading(p, &i.nombre),
            mono(p, format!("{} · {}", i.minecraft, i.loader))
        ]
        .spacing(2)
    ]
    .spacing(14)
    .align_y(Alignment::Center)
    .into()
}
pub(super) fn ficha_archivo<'a>(p: Paleta, a: &ArchivoCompartido) -> Column<'a, Accion> {
    column![heading(p, &a.nombre), mono(p, &a.detalle)].spacing(4)
}
pub(super) fn marca<'a>(p: Paleta, m: Marca) -> Element<'a, Accion> {
    let (fondo, tinta, borde) = match m {
        Marca::Alta => (p.success_soft, p.success, p.success),
        Marca::Actualizacion => (p.info_soft, p.info, p.info),
        Marca::Retirada => (p.error_soft, p.error, p.error),
        Marca::Conflicto => (p.warning_soft, p.warning, p.warning),
        Marca::Conservado => (Color::TRANSPARENT, p.text_muted, p.border_strong),
    };
    container(
        text(m.texto())
            .font(fuentes::MONO_MEDIO)
            .size(14)
            .line_height(iced::widget::text::LineHeight::Relative(1.)),
    )
    .width(20)
    .height(20)
    .center(20)
    .style(move |_| container::Style {
        background: Some(fondo.into()),
        text_color: Some(tinta),
        border: Border {
            color: borde,
            width: 1.,
            radius: 0.0.into(),
        },
        ..Default::default()
    })
    .into()
}
pub(super) fn segmento<'a>(
    p: Paleta,
    opciones: Vec<(&str, bool, Option<Accion>)>,
) -> Element<'a, Accion> {
    let mut r = row![];
    for (s, elegido, a) in opciones {
        r = r.push(
            button(
                container(text(s.to_uppercase()).font(fuentes::ETIQUETA).size(10))
                    .height(Length::Fill)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center),
            )
            .height(28)
            .padding([0., 10.])
            .on_press_maybe(a)
            .style(estilo::segmento(p, elegido)),
        );
    }
    container(r)
        .style(move |_| container::Style {
            border: Border {
                color: p.text,
                width: 2.,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into()
}
pub(super) fn avance<'a>(p: Paleta, mensaje: &str, porcentaje: f32) -> Element<'a, Accion> {
    column![
        row![
            cuerpo(p, mensaje),
            Space::new(Length::Fill, 0),
            mono(p, format!("{:.0} %", porcentaje))
        ]
        .spacing(16),
        crate::progreso::continuo(p, porcentaje / 100., false)
    ]
    .spacing(10)
    .into()
}
pub(super) fn insignia_origen<'a>(p: Paleta, origen: &str, propio: bool) -> Element<'a, Accion> {
    if propio {
        crate::componentes::insignia(p, crate::componentes::Estado::Aviso, "Archivo propio")
    } else {
        crate::componentes::insignia_contorno(p, origen)
    }
}
pub(super) fn icono_estado<'a>(p: Paleta, i: Icono) -> Element<'a, Accion> {
    icono(i, Tam::Base, p.text).into()
}

/// Rayado original a 135°, una línea de un píxel cada seis píxeles.
pub(super) fn rayado<'a>(color: Color) -> Element<'a, Accion> {
    iced::widget::canvas(Rayado { color })
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
struct Rayado {
    color: Color,
}
impl iced::widget::canvas::Program<Accion> for Rayado {
    type State = ();
    fn draw(
        &self,
        _: &(),
        renderer: &iced::Renderer,
        _: &iced::Theme,
        limites: iced::Rectangle,
        _: iced::mouse::Cursor,
    ) -> Vec<iced::widget::canvas::Geometry> {
        use iced::widget::canvas::{Frame, Path, Stroke};
        let mut f = Frame::new(renderer, limites.size());
        let mut x = -limites.height;
        while x < limites.width {
            f.stroke(
                &Path::line(
                    iced::Point::new(x, limites.height),
                    iced::Point::new(x + limites.height, 0.),
                ),
                Stroke::default().with_color(self.color).with_width(1.),
            );
            x += 6. * std::f32::consts::SQRT_2;
        }
        vec![f.into_geometry()]
    }
}

pub(super) fn peligro_fantasma<'a>(p: Paleta, s: &str, a: Option<Accion>) -> Element<'a, Accion> {
    botones::boton_estilo(
        None,
        s,
        TamBoton::Lg,
        move |t, estado| {
            let mut estilo = estilo::boton_fantasma(p)(t, estado);
            estilo.text_color = if matches!(estado, button::Status::Disabled) {
                p.text_disabled
            } else {
                p.error
            };
            estilo
        },
        a,
    )
}
