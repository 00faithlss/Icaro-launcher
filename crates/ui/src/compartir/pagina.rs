// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Marco de página de compartir dentro de Servidores: migas arriba y la fase
//! debajo, centradas con el mismo ancho. Sustituye los diálogos con velo.

use super::Accion;
use crate::{
    estilo, fuentes,
    iconos::{icono, Icono, Tam},
    tema::Paleta,
};
use iced::{
    widget::{button, column, container, rich_text, row, span, text},
    Alignment, Element, Length, Padding,
};

/// Un paso de las migas; sin acción es la página actual.
#[derive(Debug, Clone)]
pub struct Miga {
    pub texto: String,
    pub accion: Option<Accion>,
}
impl Miga {
    pub fn enlace(texto: impl Into<String>, accion: Accion) -> Self {
        Self {
            texto: texto.into(),
            accion: Some(accion),
        }
    }
    pub fn actual(texto: impl Into<String>) -> Self {
        Self {
            texto: texto.into(),
            accion: None,
        }
    }
}

/// `ic-migas`: 13/19, enlaces atenuados que se subrayan al pasar el puntero.
pub fn migas<'a>(p: Paleta, pasos: &[Miga]) -> Element<'a, Accion> {
    let mut fila = row![].spacing(8).align_y(Alignment::Center);
    for (i, m) in pasos.iter().enumerate() {
        if i > 0 {
            fila = fila.push(icono(Icono::ChevronDerecha, Tam::Sm, p.text_muted));
        }
        fila = fila.push(match &m.accion {
            Some(a) => {
                let texto = m.texto.clone();
                button(text(texto.clone()).font(fuentes::CUERPO).size(13))
                    .padding(0)
                    .on_press(a.clone())
                    .style(move |_, s| {
                        let sobre = matches!(s, button::Status::Hovered | button::Status::Pressed);
                        iced::widget::button::Style {
                            text_color: if sobre { p.text } else { p.text_muted },
                            ..Default::default()
                        }
                    })
                    .into()
            }
            None => Element::from(
                rich_text![span(m.texto.clone())
                    .font(fuentes::CUERPO_NEGRITA)
                    .color(p.text)]
                .size(13),
            ),
        });
    }
    fila.into()
}

/// Página de una fase: migas y fase centradas con el ancho del componente.
/// Las migas llevan el mismo relleno lateral que la fase para quedar alineadas.
pub fn pagina<'a>(
    p: Paleta,
    pasos: &[Miga],
    ancho: f32,
    fase: Element<'a, Accion>,
) -> Element<'a, Accion> {
    let columna = column![
        container(migas(p, pasos)).padding([0., 32.]),
        container(fase).width(Length::Fill).height(Length::Fill),
    ]
    .spacing(8)
    .width(Length::Fill)
    .max_width(ancho)
    .height(Length::Fill);
    container(columna)
        .center_x(Length::Fill)
        .height(Length::Fill)
        .padding(Padding {
            top: 24.,
            right: 16.,
            bottom: 0.,
            left: 16.,
        })
        .style(estilo::bloque(p.bg))
        .into()
}
