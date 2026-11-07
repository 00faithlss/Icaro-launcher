// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Botones con icono y rótulo, y tooltips para los de solo icono.
//!
//! El icono toma el color del texto del botón, así que se invierte junto con
//! él al pasar el puntero (el secundario cambia su relleno a tinta).

use iced::advanced::layout::{self, Layout, Node};
use iced::advanced::renderer;
use iced::advanced::svg::{Renderer as _, Svg};
use iced::advanced::widget::Tree;
use iced::advanced::Widget;
use iced::widget::{button, container, row, text, tooltip};
use iced::{mouse, Alignment, Element, Length, Padding, Rectangle, Size};

use crate::componentes::Variante;
use crate::estilo;
use crate::fuentes;
use crate::iconos::{Icono, Tam};
use crate::tema::{espacio, medida, texto, Paleta};

/// Icono que se pinta con el color de texto del contenedor que lo recibe.
struct IconoHeredado {
    icono: Icono,
    tam: f32,
}

impl<M> Widget<M, iced::Theme, iced::Renderer> for IconoHeredado {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fixed(self.tam), Length::Fixed(self.tam))
    }

    fn layout(&self, _: &mut Tree, _: &iced::Renderer, limites: &layout::Limits) -> Node {
        layout::atomic(limites, Length::Fixed(self.tam), Length::Fixed(self.tam))
    }

    fn draw(
        &self,
        _: &Tree,
        renderer: &mut iced::Renderer,
        _: &iced::Theme,
        estilo: &renderer::Style,
        layout: Layout<'_>,
        _: mouse::Cursor,
        _: &Rectangle,
    ) {
        let asa = iced::widget::svg::Handle::from_memory(self.icono.bytes());
        renderer.draw_svg(Svg::new(asa).color(estilo.text_color), layout.bounds());
    }
}

/// Icono que hereda el color del texto del botón en que está.
pub fn icono_heredado<'a, M: 'a>(icono: Icono, tam: Tam) -> Element<'a, M> {
    Element::new(IconoHeredado {
        icono,
        tam: tam.px(),
    })
}

/// Tamaño de un botón con rótulo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TamBoton {
    /// 28 px, para tablas y barras.
    Sm,
    /// 44 px, para diálogos y estados vacíos.
    Lg,
}

/// Botón con icono opcional y rótulo en mayúsculas. Sin mensaje queda
/// deshabilitado.
pub fn boton<'a, M: Clone + 'a>(
    p: Paleta,
    glifo: Option<Icono>,
    rotulo: &str,
    variante: Variante,
    tam: TamBoton,
    al_pulsar: Option<M>,
) -> Element<'a, M> {
    match variante {
        Variante::Primario => {
            boton_estilo(glifo, rotulo, tam, estilo::boton_primario(p), al_pulsar)
        }
        Variante::Secundario => {
            boton_estilo(glifo, rotulo, tam, estilo::boton_secundario(p), al_pulsar)
        }
        Variante::Fantasma => {
            boton_estilo(glifo, rotulo, tam, estilo::boton_fantasma(p), al_pulsar)
        }
        Variante::Peligro => boton_estilo(glifo, rotulo, tam, estilo::boton_peligro(p), al_pulsar),
    }
}

/// Igual que [`boton`], con la función de estilo a elección.
pub fn boton_estilo<'a, M: Clone + 'a>(
    glifo: Option<Icono>,
    rotulo: &str,
    tam: TamBoton,
    estilo: impl Fn(&iced::Theme, button::Status) -> button::Style + 'a,
    al_pulsar: Option<M>,
) -> Element<'a, M> {
    let (alto, lateral, letra) = match tam {
        TamBoton::Sm => (medida::CONTROL_SM, espacio::S3, texto::LABEL.0),
        TamBoton::Lg => (medida::CONTROL_LG, espacio::S6, 12.0),
    };
    let mut contenido = row![].spacing(espacio::S1).align_y(Alignment::Center);
    let relleno = Padding::from([0.0, lateral]);
    if let Some(g) = glifo {
        contenido = contenido.push(icono_heredado(g, Tam::Base));
    }
    contenido = contenido.push(
        text(rotulo.to_uppercase())
            .font(fuentes::ETIQUETA)
            .size(letra),
    );
    let b = button(
        container(contenido)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Center),
    )
    .height(alto)
    .padding(relleno)
    .on_press_maybe(al_pulsar)
    .style(estilo);
    b.into()
}

/// Botón fantasma de solo icono con su tooltip, de `control-sm`.
pub fn boton_icono<'a, M: Clone + 'a>(
    p: Paleta,
    glifo: Icono,
    etiqueta: &str,
    al_pulsar: M,
) -> Element<'a, M> {
    let b = button(container(icono_heredado(glifo, Tam::Base)).center(Length::Fill))
        .width(medida::CONTROL_SM)
        .height(medida::CONTROL_SM)
        .padding(0)
        .on_press(al_pulsar)
        .style(estilo::boton_fantasma(p));
    con_tooltip(p, b.into(), etiqueta, tooltip::Position::Bottom)
}

/// Rótulo flotante junto a un control, en `surface-raised` con regla de 2 px.
pub fn con_tooltip<'a, M: 'a>(
    p: Paleta,
    contenido: Element<'a, M>,
    etiqueta: &str,
    posicion: tooltip::Position,
) -> Element<'a, M> {
    tooltip(
        contenido,
        container(
            text(etiqueta.to_owned())
                .font(fuentes::CUERPO)
                .size(texto::BODY_SM.0)
                .color(p.text),
        )
        .padding(Padding::from([espacio::S1, espacio::S2]))
        .style(estilo::flotante(p)),
        posicion,
    )
    .gap(espacio::S1)
    .into()
}

/// Botón de solo icono de `control-sm` con estilo a elección; el icono toma
/// el color del texto del estilo. Lleva tooltip.
pub fn boton_icono_estilo<'a, M: Clone + 'a>(
    p: Paleta,
    glifo: Icono,
    etiqueta: &str,
    estilo: impl Fn(&iced::Theme, button::Status) -> button::Style + 'a,
    al_pulsar: M,
) -> Element<'a, M> {
    let b = button(container(icono_heredado(glifo, Tam::Base)).center(Length::Fill))
        .width(medida::CONTROL_SM)
        .height(medida::CONTROL_SM)
        .padding(0)
        .on_press(al_pulsar)
        .style(estilo);
    con_tooltip(p, b.into(), etiqueta, tooltip::Position::Top)
}
