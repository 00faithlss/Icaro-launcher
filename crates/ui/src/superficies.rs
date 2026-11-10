// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Superficies que flotan: diálogo con velo, menú contextual, toast y banner.

use iced::widget::{
    button, center, column, container, opaque, row, stack, text, Space,
};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Point, Size, Theme};

use crate::componentes::Estado;
use crate::estilo;
use crate::fuentes;
use crate::iconos::{icono, Icono, Tam};
use crate::laminas::Lamina;
use crate::tema::{borde, espacio, medida, texto, Paleta};

fn regla<'a, M: 'a>(alto: f32, color: Color) -> Element<'a, M> {
    container(Space::new(Length::Fill, Length::Fixed(alto)))
        .style(estilo::bloque(color))
        .into()
}

/// La base conserva su árbol y su desplazamiento al abrir o cerrar la capa.
pub fn capa_flotante<'a, M: 'a>(
    fondo: Element<'a, M>,
    capa: Option<Element<'a, M>>,
) -> Element<'a, M> {
    let mut capas = stack![fondo];
    if let Some(capa) = capa { capas = capas.push(capa); }
    capas.into()
}

/// Diálogo modal: marco de 2 px, cabeza con título y cierre, cuerpo y pie de
/// acciones a la derecha separado por una regla. Con `detalle`, lleva un
/// grabado en columna a la izquierda. El alto es fijo porque el grabado
/// ocupa toda la columna.
#[allow(clippy::too_many_arguments)]
pub fn dialogo<'a, M: Clone + 'a>(
    p: Paleta,
    titulo: &str,
    cuerpo: Element<'a, M>,
    acciones: Vec<Element<'a, M>>,
    detalle: Option<Lamina>,
    ancho: f32,
    alto: f32,
    cerrar: M,
) -> Element<'a, M> {
    let cabeza = row![
        text(titulo.to_owned())
            .font(fuentes::TITULO)
            .size(texto::HEADING.0 + 2.0)
            .color(p.text),
        Space::with_width(Length::Fill),
        button(crate::iconos::icono_texto(Icono::Cerrar, Tam::Base))
            .padding(espacio::S2)
            .on_press(cerrar)
            .style(estilo::boton_fantasma(p)),
    ]
    .align_y(Alignment::Center);
    let mut pie = row![Space::with_width(Length::Fill)].spacing(espacio::S2);
    for a in acciones {
        pie = pie.push(a);
    }
    let columna = column![
        container(cabeza).padding(Padding::from([espacio::S4, espacio::S6])),
        container(cuerpo)
            .padding(Padding::from([espacio::S2, espacio::S6]))
            .height(Length::Fill),
        regla(borde::FINO, p.border),
        container(pie).padding(Padding::from([espacio::S4, espacio::S6])),
    ]
    .width(Length::Fill);

    let contenido: Element<'a, M> = match detalle {
        Some(l) => row![
            crate::laminas::grabado(p, l, 170, Length::Fill),
            container(Space::new(Length::Fixed(borde::MEDIO), Length::Fill))
                .style(estilo::bloque(p.text)),
            columna,
        ]
        .into(),
        None => columna.into(),
    };
    // El relleno del grosor del marco evita que el grabado tape el contorno.
    container(container(contenido).padding(borde::MEDIO))
        .width(ancho)
        .height(alto)
        .style(estilo::flotante(p))
        .into()
}

/// Pone un diálogo sobre el contenido con el velo `scrim`. Un clic fuera del
/// diálogo emite `al_cerrar`.
pub fn con_velo<'a, M: Clone + 'a>(
    p: Paleta,
    fondo: Element<'a, M>,
    dialogo: Element<'a, M>,
    al_cerrar: M,
) -> Element<'a, M> {
    let velo = iced::widget::mouse_area(
        center(opaque(dialogo)).style(estilo::bloque(p.scrim)),
    )
    .on_press(al_cerrar);
    stack![fondo, opaque(velo)].into()
}

/// Elemento de un menú contextual.
pub enum ElementoMenu<M> {
    Opcion {
        icono: Icono,
        texto: std::borrow::Cow<'static, str>,
        atajo: Option<&'static str>,
        /// Acción destructiva: va al final y en tono de error.
        peligro: bool,
        mensaje: M,
    },
    /// Separa grupos: regla de 1 px con 6 px de aire.
    Separador,
    /// Rótulo de un grupo («Organizar», «Jugar como»).
    Rotulo(std::borrow::Cow<'static, str>),
    /// Cabecera: sobre qué actúa el menú (instancia o cuenta).
    Cabecera {
        titulo: std::borrow::Cow<'static, str>,
        detalle: std::borrow::Cow<'static, str>,
    },
    /// Opción principal (Jugar): en negrita y con el icono en tinta.
    Principal {
        icono: Icono,
        texto: std::borrow::Cow<'static, str>,
        atajo: Option<&'static str>,
        mensaje: M,
    },
    /// Opción de una lista exclusiva: check `success` si está marcada.
    Marcada {
        texto: std::borrow::Cow<'static, str>,
        detalle: Option<std::borrow::Cow<'static, str>>,
        marcada: bool,
        /// Sin mensaje queda deshabilitada.
        mensaje: Option<M>,
    },
    /// Abre un submenú al lado; `abierto` lo deja resaltado.
    Submenu {
        icono: Icono,
        texto: std::borrow::Cow<'static, str>,
        abierto: bool,
        mensaje: M,
    },
}

pub const ANCHO_MENU: f32 = 300.0;
const ALTO_OPCION: f32 = 36.0;
/// Relleno interior del menú (6 px) más su borde.
const MARCO_MENU: f32 = 6.0 + borde::MEDIO;

fn alto_elemento<M>(e: &ElementoMenu<M>) -> f32 {
    match e {
        ElementoMenu::Opcion { .. } | ElementoMenu::Principal { .. } | ElementoMenu::Submenu { .. } => ALTO_OPCION,
        ElementoMenu::Marcada { detalle: Some(_), .. } => 52.0,
        ElementoMenu::Marcada { .. } => ALTO_OPCION,
        ElementoMenu::Separador => 13.0,
        ElementoMenu::Rotulo(_) => 24.0,
        ElementoMenu::Cabecera { .. } => 56.0,
    }
}

pub fn tamano_menu<M>(elementos: &[ElementoMenu<M>]) -> Size {
    Size::new(
        ANCHO_MENU,
        2.0 * MARCO_MENU + elementos.iter().map(alto_elemento).sum::<f32>(),
    )
}

/// Se abre junto al punto pulsado, hacia arriba cuando falta espacio abajo.
pub fn posicion_menu(ancla: Point, ventana: Size, menu: Size, progreso: f32) -> Point {
    let margen = espacio::S2;
    let arriba = ancla.y + menu.height > ventana.height - margen;
    let y = if arriba { ancla.y - menu.height } else { ancla.y };
    let x = if ancla.x + menu.width > ventana.width - margen { ancla.x - menu.width } else { ancla.x };
    let desplazamiento = 12.0 * (1.0 - crate::movimiento::Curva::Salida.aplicar(progreso));
    Point::new(
        x.clamp(margen, (ventana.width - menu.width - margen).max(margen)),
        (y + if arriba { desplazamiento } else { -desplazamiento })
            .clamp(margen, (ventana.height - menu.height - margen).max(margen)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_menu_inferior_abre_hacia_arriba_sin_salirse_de_la_ventana() {
        let ventana = Size::new(1280.0, 800.0);
        let menu = Size::new(260.0, 407.0);
        let ancla = Point::new(1210.0, 720.0);
        let inicio = posicion_menu(ancla, ventana, menu, 0.0);
        let medio = posicion_menu(ancla, ventana, menu, 0.5);
        let final_ = posicion_menu(ancla, ventana, menu, 1.0);
        assert!(inicio.y > medio.y && medio.y > final_.y);
        assert_eq!(final_.y + menu.height, ancla.y);
        assert_eq!(final_.x + menu.width, ancla.x);
        assert!(inicio.y + menu.height < ventana.height);
    }

    #[test]
    fn el_menu_superior_abre_hacia_abajo_y_respeta_el_borde() {
        let ventana = Size::new(1280.0, 800.0);
        let menu = Size::new(260.0, 200.0);
        let ancla = Point::new(250.0, 100.0);
        assert!(posicion_menu(ancla, ventana, menu, 0.0).y < posicion_menu(ancla, ventana, menu, 1.0).y);
        assert_eq!(posicion_menu(ancla, ventana, menu, 1.0), ancla);
        let al_borde = posicion_menu(Point::ORIGIN, ventana, menu, 0.0);
        assert!(al_borde.x >= espacio::S2 && al_borde.y >= espacio::S2);
    }
}

/// Lo que va a la derecha de una fila del menú.
enum Derecha {
    Atajo(&'static str),
    Check,
    Flecha,
}

/// Fila de MenuAnclado: 36 px, icono gris que pasa a tinta y fondo
/// `surface-hover` al pasar el puntero (sin invertir a negro). Las de peligro
/// van en `error` con hover `error-soft`.
#[allow(clippy::too_many_arguments)]
fn fila_menu<'a, M: Clone + 'a>(
    p: Paleta,
    glifo: Option<Icono>,
    etiqueta: std::borrow::Cow<'static, str>,
    detalle: Option<std::borrow::Cow<'static, str>>,
    derecha: Option<Derecha>,
    peligro: bool,
    principal: bool,
    resaltada: Option<Color>,
    alto: f32,
    mensaje: Option<M>,
) -> Element<'a, M> {
    let tinta = if mensaje.is_none() {
        p.text_disabled
    } else if peligro {
        p.error
    } else {
        p.text
    };
    let construir = |sobre: bool, derecha: Option<Element<'a, M>>| -> Element<'a, M> {
        let color_icono = if peligro || principal || sobre { tinta } else { p.text_muted };
        let mut fila = row![].spacing(espacio::S3).align_y(Alignment::Center);
        if let Some(g) = glifo {
            fila = fila.push(icono(g, Tam::Base, color_icono));
        }
        let nombre = text(etiqueta.clone())
            .font(if principal || detalle.is_some() { fuentes::CUERPO_NEGRITA } else { fuentes::CUERPO })
            .size(13)
            .line_height(iced::widget::text::LineHeight::Absolute(18.0.into()))
            .color(tinta);
        fila = fila.push(match &detalle {
            Some(d) => Element::from(
                column![
                    nombre,
                    text(d.clone())
                        .font(fuentes::MONO)
                        .size(texto::MONO_SM.0)
                        .color(p.text_muted)
                ]
                .width(Length::Fill),
            ),
            None => container(nombre).width(Length::Fill).into(),
        });
        if let Some(d) = derecha {
            fila = fila.push(d);
        }
        let fondo = if sobre {
            Some(if peligro { p.error_soft } else { p.surface_hover })
        } else {
            resaltada
        };
        container(fila)
            .center_y(alto)
            .width(Length::Fill)
            .padding(Padding::from([0.0, 10.0]))
            .style(move |_: &Theme| container::Style {
                background: fondo.map(Background::Color),
                ..Default::default()
            })
            .into()
    };
    // Lo de la derecha (atajo, check, flecha) se dibuja igual en ambos estados.
    let lado = |d: &Derecha| -> Element<'a, M> {
        match d {
            Derecha::Atajo(a) => atajo_menu(p, a),
            Derecha::Check => icono(Icono::Check, Tam::Base, p.success).into(),
            Derecha::Flecha => icono(Icono::ChevronDerecha, Tam::Base, p.text_muted).into(),
        }
    };
    let normal = construir(false, derecha.as_ref().map(lado));
    let sobre = construir(true, derecha.as_ref().map(lado));
    let contenido: Element<'a, M> = if mensaje.is_some() {
        iced::widget::hover(normal, sobre)
    } else {
        normal
    };
    button(contenido)
        .width(Length::Fill)
        .padding(0)
        .on_press_maybe(mensaje)
        .style(move |_: &Theme, _| button::Style {
            text_color: tinta,
            ..Default::default()
        })
        .into()
}

fn atajo_menu<'a, M: 'a>(p: Paleta, a: &'static str) -> Element<'a, M> {
    text(a)
        .font(fuentes::MONO)
        .size(texto::MONO_SM.0)
        .color(p.text_muted)
        .into()
}

/// MenuAnclado: superficie elevada con borde de 2 px y sombra dura baja
/// (8 px de tinta al 18 %). Grupos separados por una regla con rótulo.
pub fn menu_contextual<'a, M: Clone + 'a>(p: Paleta, elementos: Vec<ElementoMenu<M>>) -> Element<'a, M> {
    menu_contextual_ancho(p, elementos, ANCHO_MENU)
}

pub fn menu_contextual_ancho<'a, M: Clone + 'a>(
    p: Paleta,
    elementos: Vec<ElementoMenu<M>>,
    ancho: f32,
) -> Element<'a, M> {
    let mut col = column![];
    for e in elementos {
        let alto = alto_elemento(&e);
        col = col.push(match e {
            ElementoMenu::Separador => container(regla(borde::FINO, p.border))
                .padding(Padding::from([6.0, 0.0]))
                .into(),
            ElementoMenu::Rotulo(t) => container(
                text(t.to_uppercase())
                    .font(fuentes::ETIQUETA)
                    .size(10)
                    .color(p.text_muted),
            )
            .height(alto)
            .padding(Padding {
                top: 4.0,
                right: 10.0,
                bottom: 6.0,
                left: 10.0,
            })
            .into(),
            ElementoMenu::Cabecera { titulo, detalle } => container(
                column![
                    text(titulo)
                        .font(fuentes::TITULO)
                        .size(texto::HEADING.0)
                        .color(p.text),
                    text(detalle)
                        .font(fuentes::MONO)
                        .size(texto::MONO_SM.0)
                        .color(p.text_muted),
                ]
                .spacing(2),
            )
            .height(alto)
            .center_y(alto)
            .padding(Padding::from([0.0, 10.0]))
            .into(),
            ElementoMenu::Opcion { icono: g, texto: t, atajo, peligro, mensaje } => fila_menu(
                p,
                Some(g),
                t,
                None,
                atajo.map(Derecha::Atajo),
                peligro,
                false,
                None,
                alto,
                Some(mensaje),
            ),
            ElementoMenu::Principal { icono: g, texto: t, atajo, mensaje } => fila_menu(
                p,
                Some(g),
                t,
                None,
                atajo.map(Derecha::Atajo),
                false,
                true,
                None,
                alto,
                Some(mensaje),
            ),
            ElementoMenu::Marcada { texto: t, detalle, marcada, mensaje } => fila_menu(
                p,
                None,
                t,
                detalle,
                marcada.then_some(Derecha::Check),
                false,
                false,
                marcada.then_some(p.accent_soft),
                alto,
                mensaje,
            ),
            ElementoMenu::Submenu { icono: g, texto: t, abierto, mensaje } => fila_menu(
                p,
                Some(g),
                t,
                None,
                Some(Derecha::Flecha),
                false,
                false,
                abierto.then_some(p.surface_hover),
                alto,
                Some(mensaje),
            ),
        });
    }
    container(col)
        .padding(6.0)
        .width(ancho)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(p.surface_raised)),
            border: Border {
                color: p.text,
                width: borde::MEDIO,
                radius: 0.0.into(),
            },
            // Tinta al 18 % ya mezclada con el fondo: una sombra opaca no deja
            // ver a través las reglas y textos que quedan debajo.
            shadow: iced::Shadow {
                color: Color {
                    r: p.bg.r + (p.text.r - p.bg.r) * 0.18,
                    g: p.bg.g + (p.text.g - p.bg.g) * 0.18,
                    b: p.bg.b + (p.text.b - p.bg.b) * 0.18,
                    a: 1.0,
                },
                offset: iced::Vector::new(8.0, 8.0),
                blur_radius: 0.0,
            },
            ..Default::default()
        })
        .into()
}

/// Toast: una línea invertida en tinta plena, con acción opcional. El de
/// error usa el fondo `error`.
pub fn toast<'a, M: Clone + 'a>(
    p: Paleta,
    glifo: Icono,
    mensaje: &str,
    accion: Option<(&str, M)>,
    cerrar: Option<M>,
    error: bool,
) -> Element<'a, M> {
    toast_con_opacidad(p, glifo, mensaje, accion, cerrar, error, 1.0)
}

/// La opacidad incluye fondo, texto, iconos y acciones del aviso.
#[allow(clippy::too_many_arguments)]
pub fn toast_con_opacidad<'a, M: Clone + 'a>(
    p: Paleta,
    glifo: Icono,
    mensaje: &str,
    accion: Option<(&str, M)>,
    cerrar: Option<M>,
    error: bool,
    opacidad: f32,
) -> Element<'a, M> {
    let (fondo, tinta) = if error { (p.error, p.on_error) } else { (p.text, p.bg) };
    let atenuar = |c: Color| Color { a: c.a * opacidad.clamp(0.0, 1.0), ..c };
    let (fondo, tinta) = (atenuar(fondo), atenuar(tinta));
    let mut fila = row![
        icono(glifo, Tam::Base, tinta),
        text(mensaje.to_owned())
            .size(texto::BODY_SM.0)
            .color(tinta)
            .width(Length::Fill),
    ]
    .spacing(espacio::S3)
    .align_y(Alignment::Center);
    let contorno = move |_: &Theme, _| button::Style {
        text_color: tinta,
        border: Border {
            color: tinta,
            width: borde::MEDIO,
            radius: 0.0.into(),
        },
        ..Default::default()
    };
    if let Some((etiqueta, m)) = accion {
        fila = fila.push(
            button(
                container(
                    text(etiqueta.to_uppercase())
                            .font(fuentes::ETIQUETA)
                            .size(texto::LABEL.0),
                )
                .center_y(Length::Fill),
            )
            .height(medida::CONTROL_SM)
            .padding(Padding::from([0.0, espacio::S3]))
            .on_press(m)
            .style(contorno),
        );
    }
    if let Some(m) = cerrar {
        fila = fila.push(
            button(icono(Icono::Cerrar, Tam::Base, tinta))
                .padding(espacio::S1)
                .on_press(m)
                .style(estilo::sin_estilo(tinta)),
        );
    }
    container(fila)
        .width(420)
        .padding(Padding { top: 12., right: 12., bottom: 12., left: 16. })
        .style(estilo::bloque(fondo))
        .into()
}

/// Banner persistente a todo el ancho: regla inferior de 2 px del tono sobre
/// su fondo suave, con una acción a la derecha.
pub fn banner<'a, M: Clone + 'a>(
    p: Paleta,
    estado: Estado,
    glifo: Icono,
    titulo: &str,
    mensaje: &str,
    accion: Option<(&str, M)>,
) -> Element<'a, M> {
    let (fondo, tono) = match estado {
        Estado::Exito => (p.success_soft, p.success),
        Estado::Aviso => (p.warning_soft, p.warning),
        Estado::Error => (p.error_soft, p.error),
        Estado::Info => (p.info_soft, p.info),
    };
    let mut fila = row![
        icono(glifo, Tam::Base, tono),
        text(format!("{titulo} "))
            .font(fuentes::CUERPO_NEGRITA)
            .size(texto::BODY.0)
            .color(p.text),
        text(mensaje.to_owned())
            .size(texto::BODY.0)
            .color(p.text)
            .width(Length::Fill),
    ]
    .spacing(espacio::S2)
    .align_y(Alignment::Center);
    if let Some((etiqueta, m)) = accion {
        fila = fila.push(
            button(
                container(
                    text(etiqueta.to_uppercase())
                            .font(fuentes::ETIQUETA)
                            .size(texto::LABEL.0),
                )
                .center_y(Length::Fill),
            )
            .height(medida::CONTROL_SM)
            .padding(Padding::from([0.0, espacio::S3]))
            .on_press(m)
            .style(estilo::boton_secundario(p)),
        );
    }
    column![
        container(fila)
            .width(Length::Fill)
            .padding(Padding::from([espacio::S3, espacio::S12]))
            .style(estilo::bloque(fondo)),
        regla(borde::MEDIO, tono),
    ]
    .into()
}

/// Como `con_velo`, con el diálogo cayendo desde arriba (Losa): `desfase` son
/// los píxeles por encima de su sitio; negativo es el impacto final.
pub fn con_velo_losa<'a, M: Clone + 'a>(
    p: Paleta,
    fondo: Element<'a, M>,
    dialogo: Element<'a, M>,
    al_cerrar: M,
    desfase: f32,
) -> Element<'a, M> {
    let relleno = if desfase >= 0.0 {
        Padding { bottom: desfase * 2.0, ..Padding::ZERO }
    } else {
        Padding { top: -desfase * 2.0, ..Padding::ZERO }
    };
    con_velo(p, fondo, container(dialogo).padding(relleno).into(), al_cerrar)
}
