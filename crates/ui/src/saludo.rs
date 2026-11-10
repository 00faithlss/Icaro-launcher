// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! SaludoInicio: una frase arriba de Instancias, justo bajo la banda, con una
//! segunda línea útil (última instancia y cuándo se jugó). La frase la elige
//! la aplicación; aquí solo se dibuja. El nombre de la cuenta va más grueso.

use iced::widget::{column, container, rich_text, span, text};
use iced::{Element, Length, Padding};

use crate::fuentes;
use crate::tema::{espacio, texto, Paleta};

/// Tamaño y alto de línea de la frase (30/36).
pub const FRASE: (f32, f32) = (30.0, 36.0);
/// Ancho máximo de la frase: unas 34 letras por línea.
const ANCHO_FRASE: f32 = 620.0;

/// Un trozo de la frase: texto corrido o el nombre de la cuenta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Trozo {
    Texto(String),
    Nombre(String),
}

/// Lo que muestra el saludo, ya calculado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatosSaludo {
    pub frase: Vec<Trozo>,
    /// Segunda línea: «Supervivencia te espera. Jugaste hace 2 h.».
    pub sub: Option<String>,
}

/// El saludo: la frase y su segunda línea, sin acciones. La frase cambia al
/// abrir Ícaro, no mientras se usa.
pub fn saludo_inicio<'a, M: Clone + 'static>(p: Paleta, datos: &DatosSaludo) -> Element<'a, M> {
    let mut spans = Vec::with_capacity(datos.frase.len());
    for trozo in &datos.frase {
        spans.push(match trozo {
            Trozo::Texto(t) => span(t.clone()),
            // El nombre va más grueso, sin subrayado.
            Trozo::Nombre(n) => span(n.clone()).font(fuentes::TITULO),
        });
    }
    let frase = rich_text(spans)
        .font(fuentes::ETIQUETA)
        .size(FRASE.0)
        .line_height(iced::widget::text::LineHeight::Absolute(FRASE.1.into()))
        .color(p.text);
    let mut texto_col = column![container(frase).max_width(ANCHO_FRASE)].spacing(espacio::S2);
    if let Some(sub) = &datos.sub {
        texto_col = texto_col.push(
            text(sub.clone())
                .font(fuentes::CUERPO)
                .size(texto::BODY_SM.0)
                .line_height(iced::widget::text::LineHeight::Absolute(
                    texto::BODY_SM.1.into(),
                ))
                .color(p.text_muted),
        );
    }
    container(texto_col)
        .width(Length::Fill)
        .padding(Padding {
            top: espacio::S1,
            ..Padding::ZERO
        })
        .into()
}
