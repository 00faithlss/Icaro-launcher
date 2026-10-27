// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! ArchivosNoVerificados: la lámina y filas del sistema de diseño original.
//! La vista no clasifica archivos: el importador aporta sólo evidencia real.

use crate::{
    componentes::{self, Estado, Variante},
    estilo, fuentes,
    iconos::{icono, Icono, Tam},
    laminas::{self, Lamina},
    tema::Paleta,
};
use iced::widget::{column, container, row, scrollable, text, Space};
use iced::{Alignment, Element, Length};

#[derive(Debug, Clone)]
pub struct Archivo {
    pub nombre: String,
    pub bytes: u64,
    pub sha512: String,
    /// Si se desconoce, se omite; no se atribuye a un sitio ni a una copia manual.
    pub origen: Option<String>,
    /// Sólo una evidencia del proveedor permite mostrar la fila de error.
    pub evidencia_bloqueo: Option<String>,
    pub alternativa: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accion {
    Buscar(usize),
    Confiar(usize),
    Quitar(usize),
}

fn tam(bytes: u64) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024. * 1024.)).replace('.', ",")
    } else {
        format!("{} KB", bytes.div_ceil(1024))
    }
}
fn corto(hash: &str) -> String {
    if hash.len() == 128 && hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        format!("{}…{}", &hash[..4], &hash[124..])
    } else {
        "sin hash".into()
    }
}

pub fn vista<'a, M: Clone + 'a>(
    p: Paleta,
    archivos: &[Archivo],
    ocupado: bool,
    al: impl Fn(Accion) -> M + 'a,
) -> Element<'a, M> {
    let mut lista = column![container(Space::new(Length::Fill, 2)).style(estilo::bloque(p.text))];
    for (i, a) in archivos.iter().enumerate() {
        let accion = |x| (!ocupado).then(|| al(x));
        let (detalle, tinta) = if let Some(evidencia) = &a.evidencia_bloqueo {
            (
                text(evidencia.clone()).size(12).color(p.error).into(),
                p.error,
            )
        } else {
            let origen = a
                .origen
                .as_ref()
                .map(|s| format!(" · {s}"))
                .unwrap_or_default();
            (
                text(format!(
                    "{} · sha512 {}{origen}",
                    tam(a.bytes),
                    corto(&a.sha512)
                ))
                .font(fuentes::MONO)
                .size(11)
                .color(p.text_muted)
                .into(),
                p.text,
            )
        };
        let detalle: Element<'a, M> = detalle;
        let datos = column![
            text(a.nombre.clone())
                .font(fuentes::ETIQUETA)
                .size(13)
                .color(tinta),
            detalle
        ]
        .spacing(2)
        .width(Length::Fill);
        let acciones = if a.evidencia_bloqueo.is_some() {
            row![
                componentes::insignia(p, Estado::Error, "Desactivado"),
                componentes::boton_con_icono(
                    p,
                    Icono::Papelera,
                    "Quitar",
                    Variante::Peligro,
                    accion(Accion::Quitar(i)),
                    true
                )
            ]
            .spacing(12)
            .align_y(Alignment::Center)
        } else {
            row![
                componentes::boton_con_icono(
                    p,
                    Icono::Buscar,
                    if a.alternativa {
                        "Buscar alternativa"
                    } else {
                        "Buscar en Modrinth"
                    },
                    Variante::Secundario,
                    accion(Accion::Buscar(i)),
                    true
                ),
                componentes::boton_compacto(
                    p,
                    "Confiar",
                    Variante::Fantasma,
                    accion(Accion::Confiar(i))
                ),
                iced::widget::button(container(icono(Icono::Papelera, Tam::Sm, p.text_muted)).center(Length::Fill))
                    .width(28)
                    .height(28)
                    .padding(0)
                    .on_press_maybe(accion(Accion::Quitar(i)))
                    .style(estilo::boton_fantasma(p))
            ]
            .spacing(4)
            .align_y(Alignment::Center)
        };
        let bloqueado = a.evidencia_bloqueo.is_some();
        lista = lista.push(
            container(row![datos, acciones].spacing(12).align_y(Alignment::Center))
                .padding([12., 16.])
                .width(Length::Fill)
                .style(move |_| container::Style {
                    background: bloqueado.then_some(p.error_soft.into()),
                    ..Default::default()
                }),
        );
    }
    let lamina = container(laminas::grabado_entero(p, Lamina::TorreReja, 200., 420.))
        .width(200).height(420);
    let contenido = row![lamina,
        column![column![text(format!("{} ARCHIVOS SIN\nVERIFICAR", archivos.len()))
            .font(fuentes::DISPLAY).size(32)
            .line_height(iced::widget::text::LineHeight::Absolute(iced::Pixels(34.))).color(p.text),
            text("No pudimos verificar el origen de estos mods. Pueden ser legítimos (copiados a mano), pero conviene reemplazarlos por la versión de Modrinth.")
                .size(14).color(p.text_muted)].spacing(8), lista].spacing(20).width(Length::Fill)
    ].spacing(40).width(920.);
    scrollable(container(contenido).padding(40))
        .direction(scrollable::Direction::Both {
            horizontal: scrollable::Scrollbar::default(),
            vertical: scrollable::Scrollbar::default(),
        })
        .into()
}
