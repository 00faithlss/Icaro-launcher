// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! `ic-migas`: ruta de una página dentro de su sección. Los enlaces van
//! atenuados y se aclaran al pasar el puntero; el último paso es la página actual.

use crate::{
    fuentes,
    iconos::{icono, Icono, Tam},
    tema::Paleta,
};
use iced::{
    widget::{button, rich_text, row, span, text},
    Alignment, Element,
};

/// Pasos de las migas: texto y, salvo el actual, el mensaje del enlace.
pub fn migas<'a, M: Clone + 'static>(p: Paleta, pasos: &[(String, Option<M>)]) -> Element<'a, M> {
    let mut fila = row![].spacing(8).align_y(Alignment::Center);
    for (i, (texto, accion)) in pasos.iter().enumerate() {
        if i > 0 {
            fila = fila.push(icono(Icono::ChevronDerecha, Tam::Sm, p.text_muted));
        }
        fila = fila.push(match accion {
            Some(a) => button(text(texto.clone()).font(fuentes::CUERPO).size(13))
                .padding(0)
                .on_press(a.clone())
                .style(move |_, s| button::Style {
                    text_color: if matches!(s, button::Status::Hovered | button::Status::Pressed) {
                        p.text
                    } else {
                        p.text_muted
                    },
                    ..Default::default()
                })
                .into(),
            None => Element::from(
                rich_text![span(texto.clone())
                    .font(fuentes::CUERPO_NEGRITA)
                    .color(p.text)]
                .size(13),
            ),
        });
    }
    fila.into()
}
