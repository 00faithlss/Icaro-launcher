// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Galería de capturas de pantalla: filtro por instancia, cuadrícula de
//! miniaturas 16:9 con nombre de archivo e instancia, selección múltiple y
//! acciones al pasar el puntero. El visor ampliado está en [`crate::visor`].

use iced::widget::image::Handle;
use iced::widget::{Space, button, column, container, mouse_area, row, stack, text};
use iced::{Alignment, Background, Border, Element, Length};
use std::{path::PathBuf, time::SystemTime};

use crate::botones::{TamBoton, boton, boton_icono_estilo, icono_heredado};
use crate::componentes::{Variante, selector};
use crate::estilo;
use crate::fuentes;
use crate::grabados::Imagen;
use crate::iconos::{Icono, Tam};
use crate::laminas::Lamina;
use crate::tema::{Paleta, borde, espacio, texto};

/// Captura de pantalla. En el launcher la imagen es la captura real del
/// usuario; mientras tanto se usa una lámina de ejemplo.
#[derive(Debug, Clone)]
pub struct Captura {
    pub archivo: String,
    pub instancia: String,
    /// Fecha y hora en formato DD-MM-AAAA HH:MM.
    pub fecha: String,
    pub bytes: u64,
    pub imagen: Lamina,
    /// Original local; la lámina solo se usa en la maqueta del diseño.
    pub original: Option<OriginalCaptura>,
}

#[derive(Debug, Clone)]
pub struct OriginalCaptura {
    pub ruta: PathBuf,
    pub instancia_id: String,
    /// Original a resolución completa; solo lo decodifica el visor.
    pub imagen: Handle,
    /// Miniatura reducida para la cuadrícula, generada fuera del hilo de la
    /// interfaz. Mientras falta se muestra un hueco vacío.
    pub miniatura: Option<Handle>,
    pub modificada: Option<SystemTime>,
}

/// Separación entre miniaturas.
pub const HUECO: f32 = espacio::S5;
/// Ancho mínimo de una miniatura: de ahí salen las columnas que caben.
const ANCHO_MIN: f32 = 200.0;
const COLUMNAS_MAX: usize = 6;

/// Columnas que caben en `ancho` px.
pub fn columnas_para(ancho: f32) -> usize {
    (((ancho + HUECO) / (ANCHO_MIN + HUECO)).floor() as usize).clamp(2, COLUMNAS_MAX)
}

/// Peso en formato chileno: coma decimal, `MB` o `GB`.
pub fn formato_peso(bytes: u64) -> String {
    const MB: f64 = 1024.0 * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    let (valor, unidad) = if b >= GB {
        (b / GB, "GB")
    } else {
        (b / MB, "MB")
    };
    let cifra = if valor >= 100.0 {
        format!("{valor:.0}")
    } else {
        format!("{valor:.1}")
    };
    format!("{} {unidad}", cifra.replace('.', ","))
}

/// Datos que necesita la galería.
pub struct VistaCapturas<'a> {
    pub p: Paleta,
    /// Ancho disponible para la cuadrícula, en px.
    pub ancho: f32,
    /// Opciones del filtro; la primera es "Todas las instancias".
    pub instancias: Vec<String>,
    pub filtro: usize,
    pub capturas: &'a [Captura],
    /// Índices (en `capturas`) de las que pasan el filtro.
    pub visibles: &'a [usize],
    pub seleccion: &'a [usize],
    pub encima: Option<usize>,
}

/// Mensajes de la galería.
pub struct MensajesCapturas<M> {
    pub filtrar: fn(String) -> M,
    /// Clic en una miniatura.
    pub abrir: fn(usize) -> M,
    /// Casilla de selección.
    pub alternar: fn(usize) -> M,
    /// El puntero entra o sale de una miniatura.
    pub pasar: fn(Option<usize>) -> M,
    pub copiar: fn(usize) -> M,
    pub carpeta: fn(usize) -> M,
    pub eliminar: fn(usize) -> M,
    pub abrir_carpeta: M,
    pub copiar_seleccion: M,
    pub eliminar_seleccion: M,
    pub limpiar_seleccion: M,
}

/// Cabecera y cuadrícula de capturas.
pub fn galeria<'a, M: Clone + 'a>(
    v: &VistaCapturas<'_>,
    m: &MensajesCapturas<M>,
) -> Element<'a, M> {
    let p = v.p;
    let elegidas = v.seleccion.len();
    let peso: u64 = v.visibles.iter().map(|&i| v.capturas[i].bytes).sum();

    let mut cabecera = row![].spacing(espacio::S3).align_y(Alignment::Center);
    if !v.instancias.is_empty() {
        cabecera = cabecera.push(
            container(selector(
                p,
                v.instancias.clone(),
                v.instancias.get(v.filtro).cloned(),
                m.filtrar,
            ))
            .width(220),
        );
    }
    if elegidas == 0 {
        cabecera = cabecera
            .push(
                text(format!(
                    "{} {} · {}",
                    v.visibles.len(),
                    if v.visibles.len() == 1 {
                        "captura"
                    } else {
                        "capturas"
                    },
                    formato_peso(peso)
                ))
                .size(texto::BODY_SM.0)
                .color(p.text_muted),
            )
            .push(Space::with_width(Length::Fill))
            .push(boton(
                p,
                Some(Icono::Carpeta),
                "Abrir carpeta",
                Variante::Secundario,
                TamBoton::Sm,
                Some(m.abrir_carpeta.clone()),
            ));
    } else {
        cabecera = cabecera
            .push(
                text(format!(
                    "{elegidas} {}",
                    if elegidas == 1 {
                        "seleccionada"
                    } else {
                        "seleccionadas"
                    }
                ))
                .size(texto::BODY_SM.0)
                .color(p.text),
            )
            .push(Space::with_width(Length::Fill))
            .push(boton(
                p,
                Some(Icono::Copiar),
                "Copiar",
                Variante::Secundario,
                TamBoton::Sm,
                Some(m.copiar_seleccion.clone()),
            ))
            .push(boton(
                p,
                Some(Icono::Papelera),
                "Eliminar",
                Variante::Peligro,
                TamBoton::Sm,
                Some(m.eliminar_seleccion.clone()),
            ))
            .push(boton(
                p,
                None,
                "Quitar selección",
                Variante::Fantasma,
                TamBoton::Sm,
                Some(m.limpiar_seleccion.clone()),
            ));
    }

    let columnas = columnas_para(v.ancho);
    let ancho_col = ((v.ancho - HUECO * (columnas as f32 - 1.0)) / columnas as f32).floor();
    let mut filas = column![].spacing(HUECO);
    for grupo in v.visibles.chunks(columnas) {
        let mut fila = row![].spacing(HUECO).align_y(Alignment::Start);
        for &i in grupo {
            fila = fila.push(miniatura(
                p,
                i,
                &v.capturas[i],
                ancho_col,
                v.seleccion.contains(&i),
                v.encima == Some(i),
                m,
            ));
        }
        filas = filas.push(fila);
    }
    column![cabecera, filas].spacing(HUECO).into()
}

/// Una miniatura: marco, imagen 16:9, casilla, franja de acciones y leyenda.
fn miniatura<'a, M: Clone + 'a>(
    p: Paleta,
    i: usize,
    c: &Captura,
    ancho: f32,
    elegida: bool,
    encima: bool,
    m: &MensajesCapturas<M>,
) -> Element<'a, M> {
    let grosor = if elegida { borde::MEDIO } else { borde::FINO };
    let tinta = if elegida || encima { p.text } else { p.border };
    let alto = (ancho * 9.0 / 16.0).round();
    // La cuadrícula nunca usa el original: decodificar y subir decenas de
    // PNG a resolución completa en el primer cuadro congela la interfaz.
    let imagen: Element<'a, M> = if let Some(original) = &c.original {
        match &original.miniatura {
            Some(miniatura) => iced::widget::image(miniatura.clone())
                .content_fit(iced::ContentFit::Cover)
                .width(Length::Fill)
                .height(alto - 2.0 * grosor)
                .into(),
            None => container(Space::new(Length::Fill, Length::Fill))
                .width(Length::Fill)
                .height(alto - 2.0 * grosor)
                .style(move |_: &iced::Theme| container::Style {
                    background: Some(Background::Color(p.surface_sunken)),
                    ..Default::default()
                })
                .into(),
        }
    } else {
        Imagen::nueva(p, c.imagen)
            .barrido(p)
            .tamano(Length::Fill, alto - 2.0 * grosor)
            .into()
    };

    let mut capas = stack![imagen];
    if elegida || encima {
        capas = capas.push(
            container(
                button(casilla(p, elegida))
                    .padding(0)
                    .on_press((m.alternar)(i))
                    .style(estilo::sin_estilo(p.text)),
            )
            .padding(espacio::S2),
        );
    }
    if encima {
        let invertido = move |_: &iced::Theme, estado: button::Status| {
            let activo = matches!(estado, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: activo.then_some(Background::Color(p.bg)),
                text_color: if activo { p.text } else { p.bg },
                border: Border {
                    width: 0.0,
                    ..Default::default()
                },
                ..Default::default()
            }
        };
        let franja = container(
            row![
                boton_icono_estilo(p, Icono::Copiar, "Copiar", invertido, (m.copiar)(i)),
                boton_icono_estilo(
                    p,
                    Icono::Carpeta,
                    "Abrir carpeta",
                    invertido,
                    (m.carpeta)(i)
                ),
                Space::with_width(Length::Fill),
                boton_icono_estilo(p, Icono::Papelera, "Eliminar", invertido, (m.eliminar)(i)),
            ]
            .spacing(espacio::S1)
            .align_y(Alignment::Center),
        )
        .padding(espacio::S1)
        .width(Length::Fill)
        .style(estilo::bloque(p.text));
        capas = capas.push(column![Space::with_height(Length::Fill), franja].height(Length::Fill));
    }

    let marco = container(capas)
        .padding(grosor)
        .width(ancho)
        .height(alto)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(Background::Color(p.surface_raised)),
            border: Border {
                width: grosor,
                color: tinta,
                radius: 0.0.into(),
            },
            ..Default::default()
        });
    let nombre = c.archivo.trim_end_matches(".png").to_owned();
    column![
        mouse_area(marco)
            .on_enter((m.pasar)(Some(i)))
            .on_exit((m.pasar)(None))
            .on_press((m.abrir)(i)),
        column![
            text(nombre)
                .font(fuentes::MONO)
                .size(texto::MONO_SM.0)
                .color(p.text),
            text(c.instancia.clone())
                .size(texto::BODY_SM.0)
                .color(p.text_muted),
        ]
    ]
    .spacing(espacio::S2)
    .width(ancho)
    .into()
}

/// Casilla de selección de 20 px: bloque de acento marcado o pozo vacío.
fn casilla<'a, M: 'a>(p: Paleta, marcada: bool) -> Element<'a, M> {
    let contenido: Element<M> = if marcada {
        icono_heredado(Icono::Check, Tam::Sm)
    } else {
        Space::new(0, 0).into()
    };
    container(contenido)
        .center(20)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(Background::Color(if marcada {
                p.accent
            } else {
                p.surface_sunken
            })),
            text_color: Some(p.on_accent),
            border: Border {
                width: borde::MEDIO,
                color: if marcada { p.accent } else { p.border_strong },
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn las_columnas_siguen_el_ancho() {
        // Ventana de 1.280 px con la barra lateral abierta y márgenes de 48.
        assert_eq!(columnas_para(952.0), 4);
        // Mínimo de ventana con la barra abierta.
        assert_eq!(columnas_para(612.0), 2);
        assert_eq!(columnas_para(300.0), 2);
        assert_eq!(columnas_para(3000.0), COLUMNAS_MAX);
    }

    #[test]
    fn el_peso_va_en_formato_chileno() {
        assert_eq!(formato_peso(5_800_000_000), "5,4 GB");
        assert_eq!(formato_peso(2_200_000), "2,1 MB");
        assert_eq!(formato_peso(140 * 1024 * 1024), "140 MB");
    }
}
