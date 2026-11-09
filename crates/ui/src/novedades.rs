// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! NovedadesVersion: qué trae cada versión de Ícaro, separado en Nuevo,
//! Mejoras y Correcciones. A la izquierda, la columna de versiones; a la
//! derecha, cabecera, filtros con su número, el cuerpo desplazable y el pie.
//! Es una página dentro de Ajustes, no un diálogo.

use iced::widget::{button, column, container, row, text, Space};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Theme};

use crate::componentes::{self, Estado, Variante};
use crate::fuentes;
use crate::iconos::{icono, Icono, Tam};
use crate::tema::{borde, espacio, medida, texto, Paleta};

/// Ancho máximo del bloque, el del diseño.
pub const ANCHO: f32 = 980.0;
const ANCHO_VERSIONES: f32 = 200.0;
/// Relleno lateral de la parte principal.
const LADO: f32 = 36.0;

/// Tipo de cambio, para los filtros.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tipo {
    Nuevo,
    Mejora,
    Correccion,
}

/// Una función nueva: título, una frase y una acción opcional que lleva a ella.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Funcion {
    pub titulo: String,
    pub texto: String,
    /// Rótulo del botón («Abrir Servidores»).
    pub accion: Option<String>,
}

/// Las notas de una versión.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub version: String,
    /// «09-11-2026».
    pub fecha: String,
    /// «Hoy», «Hace 12 días».
    pub hace: String,
    pub nuevo: Vec<Funcion>,
    pub mejoras: Vec<String>,
    pub correcciones: Vec<String>,
}

impl Version {
    pub fn cantidad(&self, tipo: Option<Tipo>) -> usize {
        match tipo {
            None => self.nuevo.len() + self.mejoras.len() + self.correcciones.len(),
            Some(Tipo::Nuevo) => self.nuevo.len(),
            Some(Tipo::Mejora) => self.mejoras.len(),
            Some(Tipo::Correccion) => self.correcciones.len(),
        }
    }
}

/// Estado de la vista que decide la aplicación.
pub struct Datos<'a> {
    pub versiones: &'a [Version],
    pub elegida: usize,
    /// Índice de la versión instalada: solo ella muestra las acciones.
    pub instalada: Option<usize>,
    pub filtro: Option<Tipo>,
    /// Recién actualizado: insignia «Actualizado».
    pub recien: bool,
    /// Texto del pie, junto a Listo.
    pub pie: String,
}

pub struct Mensajes<'a, M> {
    pub elegir: Box<dyn Fn(usize) -> M + 'a>,
    pub filtrar: Box<dyn Fn(Option<Tipo>) -> M + 'a>,
    /// Acción de la función nueva `i` de la versión elegida.
    pub accion: Box<dyn Fn(usize) -> M + 'a>,
    pub github: M,
    pub cerrar: M,
}

fn caja(fondo: Option<Color>, ancho: f32, color: Color) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: fondo.map(Background::Color),
        border: Border {
            color,
            width: ancho,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

fn regla<'a, M: 'a>(alto: f32, color: Color) -> Element<'a, M> {
    crate::descargas::regla(alto, color)
}

fn mono<'a>(s: impl text::IntoFragment<'a>, tam: (f32, f32), color: Color) -> text::Text<'a> {
    text(s)
        .font(fuentes::MONO)
        .size(tam.0)
        .line_height(text::LineHeight::Absolute(tam.1.into()))
        .color(color)
}

fn cuerpo_sm<'a>(s: impl text::IntoFragment<'a>, color: Color) -> text::Text<'a> {
    text(s)
        .font(fuentes::CUERPO)
        .size(texto::BODY_SM.0)
        .line_height(text::LineHeight::Absolute(texto::BODY_SM.1.into()))
        .color(color)
}

/// Fila de la columna de versiones; la elegida lleva regla izquierda de 3 px.
fn fila_version<'a, M: Clone + 'a>(
    p: Paleta,
    v: &'a Version,
    elegida: bool,
    instalada: bool,
    al_pulsar: M,
) -> Element<'a, M> {
    let mut datos = column![
        mono(v.version.as_str(), texto::MONO, p.text),
        mono(v.fecha.as_str(), texto::MONO_SM, p.text_muted),
    ]
    .spacing(2);
    if instalada {
        datos = datos.push(
            container(
                text("INSTALADA")
                    .font(fuentes::ETIQUETA)
                    .size(10)
                    .line_height(text::LineHeight::Absolute(14.0.into()))
                    .color(p.success),
            )
            .padding(Padding {
                top: espacio::S1,
                ..Padding::ZERO
            }),
        );
    }
    let alto = if instalada { 76.0 } else { 58.0 };
    let marca = container(Space::new(borde::GRUESO, alto)).style(caja(
        elegida.then_some(p.text),
        0.0,
        Color::TRANSPARENT,
    ));
    button(
        row![
            marca,
            container(datos)
                .center_y(Length::Fill)
                .padding(Padding::from([0.0, espacio::S5 - borde::GRUESO])),
        ]
        .height(alto),
    )
    .width(Length::Fill)
    .padding(0)
    .on_press(al_pulsar)
    .style(move |_, estado| button::Style {
        background: if elegida {
            Some(Background::Color(p.surface))
        } else if matches!(estado, button::Status::Hovered | button::Status::Pressed) {
            Some(Background::Color(p.surface_hover))
        } else {
            None
        },
        text_color: p.text,
        ..Default::default()
    })
    .into()
}

/// Chip de filtro con su número; sin cambios de ese tipo queda desactivado.
fn filtro<'a, M: Clone + 'a>(
    p: Paleta,
    etiqueta: &str,
    cantidad: usize,
    activo: bool,
    al_pulsar: M,
) -> Element<'a, M> {
    button(
        container(
            row![
                text(etiqueta.to_uppercase())
                    .font(fuentes::ETIQUETA)
                    .size(texto::LABEL.0),
                text(cantidad.to_string())
                    .font(fuentes::MONO)
                    .size(texto::MONO_SM.0),
            ]
            .spacing(espacio::S2)
            .align_y(Alignment::Center),
        )
        .center_y(Length::Fill),
    )
    .height(medida::CONTROL_SM)
    .padding(Padding::from([0.0, espacio::S4]))
    .on_press_maybe((cantidad > 0).then_some(al_pulsar))
    .style(crate::estilo::chip(p, activo && cantidad > 0))
    .into()
}

fn cabeza_seccion<'a, M: 'a>(p: Paleta, tipo: Tipo, cantidad: usize) -> Element<'a, M> {
    let insignia = match tipo {
        Tipo::Nuevo => componentes::insignia(p, Estado::Exito, "Nuevo"),
        Tipo::Mejora => componentes::insignia(p, Estado::Info, "Mejoras"),
        Tipo::Correccion => componentes::insignia_contorno(p, "Correcciones"),
    };
    column![
        container(
            row![
                insignia,
                mono(cantidad.to_string(), texto::MONO_SM, p.text_muted)
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        )
        .padding(Padding {
            bottom: 10.0,
            ..Padding::ZERO
        }),
        regla(borde::FINO, p.border),
    ]
    .into()
}

fn tarjeta_nueva<'a, M: Clone + 'a>(
    p: Paleta,
    n: usize,
    f: &'a Funcion,
    accion: Option<M>,
) -> Element<'a, M> {
    let destacada = n == 0;
    let mut cuerpo = column![text(f.titulo.as_str())
        .font(fuentes::CUERPO_NEGRITA)
        .size(17)
        .line_height(text::LineHeight::Absolute(22.0.into()))
        .color(p.text),]
    .spacing(6);
    if !f.texto.is_empty() {
        cuerpo = cuerpo.push(cuerpo_sm(f.texto.as_str(), p.text_muted));
    }
    if let (Some(rotulo), Some(m)) = (&f.accion, accion) {
        cuerpo =
            cuerpo
                .push(Space::with_height(espacio::S1))
                .push(row![componentes::boton_con_icono(
                    p,
                    Icono::Adelante,
                    rotulo,
                    Variante::Secundario,
                    Some(m),
                    true,
                )]);
    }
    let numero = text(format!("{:02}", n + 1))
        .font(fuentes::DISPLAY)
        .size(22)
        .line_height(text::LineHeight::Absolute(26.0.into()))
        .color(if destacada { p.text } else { p.text_muted });
    // En iced el borde se dibuja sobre el relleno: el mismo relleno deja el
    // contenido de todas las tarjetas en la misma posición.
    let (grosor, color, fondo) = if destacada {
        (borde::MEDIO, p.text, p.accent_soft)
    } else {
        (borde::FINO, p.border_strong, p.surface)
    };
    let relleno = Padding::from([18.0, 20.0]);
    container(
        row![
            container(numero).width(44),
            container(cuerpo).width(Length::Fill)
        ]
        .spacing(espacio::S4),
    )
    .width(Length::Fill)
    .padding(relleno)
    .style(caja(Some(fondo), grosor, color))
    .into()
}

fn lista<'a, M: 'a>(p: Paleta, marca: &'static str, lineas: &'a [String]) -> Element<'a, M> {
    let mut col = column![];
    for (i, l) in lineas.iter().enumerate() {
        if i > 0 {
            col = col.push(regla(borde::FINO, p.border));
        }
        col = col.push(
            row![
                container(mono(marca, (14.0, 20.0), p.text_muted)).center_x(20),
                container(cuerpo_sm(l.as_str(), p.text)).width(Length::Fill),
            ]
            .spacing(espacio::S3)
            .padding(Padding::from([10.0, 0.0])),
        );
    }
    col.into()
}

/// El diálogo completo.
pub fn novedades<'a, M: Clone + 'a>(
    p: Paleta,
    d: Datos<'a>,
    ms: Mensajes<'a, M>,
) -> Element<'a, M> {
    let Some(v) = d.versiones.get(d.elegida) else {
        return Space::new(0, 0).into();
    };
    // Un filtro sin cambios en esta versión vuelve a Todo.
    let filtro_activo = d.filtro.filter(|t| v.cantidad(Some(*t)) > 0);

    // Columna de versiones.
    let mut filas = column![].spacing(2);
    for (i, ver) in d.versiones.iter().enumerate() {
        filas = filas.push(fila_version(
            p,
            ver,
            i == d.elegida,
            d.instalada == Some(i),
            (ms.elegir)(i),
        ));
    }
    let todas = button(
        row![
            cuerpo_sm("Todas en GitHub", p.text_muted),
            icono(Icono::Externo, Tam::Sm, p.text_muted),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([0.0, espacio::S5]))
    .on_press(ms.github.clone())
    .style(move |_, _| button::Style {
        text_color: p.text_muted,
        ..Default::default()
    });
    let versiones = container(
        column![
            container(
                text("VERSIONES")
                    .font(fuentes::ETIQUETA)
                    .size(texto::LABEL.0)
                    .color(p.text_muted)
            )
            .padding(Padding::from([0.0, espacio::S5])),
            container(crate::scroll::desplazable(filas)).height(Length::Fill),
            todas,
        ]
        .spacing(espacio::S3)
        .height(Length::Fill),
    )
    .width(ANCHO_VERSIONES)
    .height(Length::Fill)
    .padding(Padding {
        top: 28.0,
        bottom: espacio::S5,
        ..Padding::ZERO
    })
    .style(caja(Some(p.surface_sunken), 0.0, Color::TRANSPARENT));

    // Cabecera.
    let mut titulo = column![].spacing(espacio::S2);
    if d.recien {
        titulo = titulo.push(componentes::insignia(p, Estado::Exito, "Actualizado"));
    }
    let total = v.cantidad(None);
    titulo = titulo
        .push(
            text(format!("Ícaro {}", v.version))
                .font(fuentes::DISPLAY)
                .size(texto::DISPLAY.0)
                .line_height(text::LineHeight::Absolute(texto::DISPLAY.1.into()))
                .color(p.text),
        )
        .push(mono(
            [
                v.fecha.clone(),
                v.hace.clone(),
                format!("{total} {}", if total == 1 { "cambio" } else { "cambios" }),
            ]
            .into_iter()
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join(" · "),
            texto::MONO_SM,
            p.text_muted,
        ));
    let cabecera = container(container(titulo).width(Length::Fill)).padding(Padding {
        top: 32.0,
        right: LADO,
        bottom: espacio::S6,
        left: LADO,
    });

    // Filtros.
    let mut filtros = row![filtro(
        p,
        "Todo",
        total,
        filtro_activo.is_none(),
        (ms.filtrar)(None)
    )]
    .spacing(espacio::S2);
    for (tipo, nombre) in [
        (Tipo::Nuevo, "Nuevo"),
        (Tipo::Mejora, "Mejoras"),
        (Tipo::Correccion, "Correcciones"),
    ] {
        filtros = filtros.push(filtro(
            p,
            nombre,
            v.cantidad(Some(tipo)),
            filtro_activo == Some(tipo),
            (ms.filtrar)(Some(tipo)),
        ));
    }
    let filtros = container(filtros.wrap()).padding(Padding::from([espacio::S5, LADO]));

    // Cuerpo por secciones.
    let ver = |t: Tipo| v.cantidad(Some(t)) > 0 && filtro_activo.is_none_or(|f| f == t);
    let mut cuerpo = column![].spacing(espacio::S8);
    if v.nuevo.is_empty() && filtro_activo.is_none() {
        cuerpo = cuerpo.push(
            container(cuerpo_sm(
                if v.mejoras.is_empty() {
                    "Versión de mantenimiento: sin funciones nuevas, solo correcciones."
                } else {
                    "Versión de mantenimiento: sin funciones nuevas, solo mejoras y correcciones."
                },
                p.text_muted,
            ))
            .width(Length::Fill)
            .padding(Padding::from([14.0, espacio::S4]))
            .style(caja(Some(p.surface_sunken), 0.0, Color::TRANSPARENT)),
        );
    }
    if ver(Tipo::Nuevo) {
        let mut tarjetas = column![].spacing(10);
        for (i, f) in v.nuevo.iter().enumerate() {
            let accion = (d.instalada == Some(d.elegida)).then(|| (ms.accion)(i));
            tarjetas = tarjetas.push(tarjeta_nueva(p, i, f, accion));
        }
        cuerpo = cuerpo
            .push(column![cabeza_seccion(p, Tipo::Nuevo, v.nuevo.len()), tarjetas].spacing(14));
    }
    if ver(Tipo::Mejora) {
        cuerpo = cuerpo.push(
            column![
                cabeza_seccion(p, Tipo::Mejora, v.mejoras.len()),
                lista(p, "~", &v.mejoras)
            ]
            .spacing(14),
        );
    }
    if ver(Tipo::Correccion) {
        cuerpo = cuerpo.push(
            column![
                cabeza_seccion(p, Tipo::Correccion, v.correcciones.len()),
                lista(p, "·", &v.correcciones)
            ]
            .spacing(14),
        );
    }
    let clave = (d.elegida as u64) << 3
        | match filtro_activo {
            None => 0,
            Some(Tipo::Nuevo) => 1,
            Some(Tipo::Mejora) => 2,
            Some(Tipo::Correccion) => 3,
        };
    let cuerpo = container(crate::scroll::desplazable_con_clave(
        container(cuerpo).padding(Padding {
            top: espacio::S1,
            right: LADO,
            bottom: 32.0,
            left: LADO,
        }),
        clave,
    ))
    .height(Length::Fill);

    let pie = container(
        row![
            container(cuerpo_sm(d.pie, p.text_muted)).width(Length::Fill),
            componentes::boton(p, "Listo", Variante::Primario, Some(ms.cerrar)),
        ]
        .spacing(espacio::S4)
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([espacio::S4, LADO]));

    let principal = column![
        cabecera,
        regla(borde::MEDIO, p.text),
        filtros,
        cuerpo,
        regla(borde::FINO, p.border),
        pie,
    ]
    .width(Length::Fill)
    .height(Length::Fill);

    // Es una página, no un diálogo: el bloque llena el área de contenido con
    // un contorno fino; el relleno del grosor evita que el fondo lo tape.
    container(
        row![
            versiones,
            container(Space::new(borde::FINO, Length::Fill)).style(caja(
                Some(p.border),
                0.0,
                Color::TRANSPARENT
            )),
            principal,
        ]
        .height(Length::Fill),
    )
    .padding(borde::FINO)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(caja(Some(p.surface_raised), borde::FINO, p.border))
    .into()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_cantidad_cuenta_por_tipo_y_en_total() {
        let v = Version {
            version: "0.4.0".into(),
            fecha: "09-11-2026".into(),
            hace: "Hoy".into(),
            nuevo: vec![Funcion {
                titulo: "A".into(),
                texto: String::new(),
                accion: None,
            }],
            mejoras: vec!["m".into(), "n".into()],
            correcciones: vec![],
        };
        assert_eq!(v.cantidad(None), 3);
        assert_eq!(v.cantidad(Some(Tipo::Mejora)), 2);
        assert_eq!(v.cantidad(Some(Tipo::Correccion)), 0);
    }
}
