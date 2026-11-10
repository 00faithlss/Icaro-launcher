// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Estructura de la ventana: barra de título propia, barra lateral con las
//! siete secciones, contenido y barra de juego.

use iced::widget::{button, column, container, mouse_area, progress_bar, row, text, Space};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Theme};

use crate::fuentes;
use crate::botones::icono_heredado;
use crate::iconos::{icono, Icono, Tam};
use crate::tema::{borde, espacio, medida, texto, Paleta};

/// Secciones de la barra lateral.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Seccion {
    Instancias,
    Servidores,
    Descargas,
    Consola,
    Modpacks,
    Capturas,
    Ajustes,
}

impl Seccion {
    pub const TODAS: [Seccion; 7] = [
        Seccion::Instancias,
        Seccion::Servidores,
        Seccion::Descargas,
        Seccion::Consola,
        Seccion::Modpacks,
        Seccion::Capturas,
        Seccion::Ajustes,
    ];

    pub const fn nombre(self) -> &'static str {
        match self {
            Seccion::Instancias => "Instancias",
            Seccion::Servidores => "Servidores",
            Seccion::Descargas => "Descargas",
            Seccion::Consola => "Consola",
            Seccion::Modpacks => "Modpacks",
            Seccion::Capturas => "Capturas",
            Seccion::Ajustes => "Ajustes",
        }
    }

    pub const fn icono(self) -> Icono {
        match self {
            Seccion::Instancias => Icono::Instancias,
            Seccion::Servidores => Icono::Servidores,
            Seccion::Descargas => Icono::Descargas,
            Seccion::Consola => Icono::Consola,
            Seccion::Modpacks => Icono::Mods,
            Seccion::Capturas => Icono::Capturas,
            Seccion::Ajustes => Icono::Ajustes,
        }
    }

    /// Número del atajo `Ctrl` + número.
    pub const fn atajo(self) -> u8 {
        self as u8 + 1
    }
}

/// Cuenta que se muestra al pie de la barra lateral.
pub struct Cuenta<'a> {
    pub nombre: &'a str,
    pub proveedor: &'a str,
    pub estado: crate::cuenta::EstadoSesion,
    pub textura: Option<&'a [u8]>,
}

/// Resumen de las descargas activas; la barra lateral lo muestra solo si existe.
pub struct DescargasActivas {
    pub cantidad: usize,
    /// Velocidad ya formateada, por ejemplo `6,1 MB/s`.
    pub velocidad: String,
    /// Avance de 0 a 1.
    pub avance: f32,
}

/// Mensajes que emite la estructura.
pub struct MensajesShell<'a, M> {
    pub ir_a: Box<dyn Fn(Seccion) -> M + 'a>,
    pub arrastrar: M,
    pub buscar: M,
    pub alternar_barra: M,
    /// Abre el menú de cuentas desde el bloque de la barra lateral.
    pub cuenta: M,
    pub minimizar: M,
    pub maximizar: M,
    pub cerrar: M,
}

fn regla<'a, M: 'a>(color: Color, grosor: f32) -> Element<'a, M> {
    container(Space::new(Length::Fill, Length::Fixed(grosor)))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(color)),
            ..Default::default()
        })
        .into()
}

/// Tecla de atajo en `mono-sm` con regla fina.
pub fn kbd<'a, M: 'a>(p: Paleta, contenido: &str) -> Element<'a, M> {
    container(
        text(contenido.to_string())
            .font(fuentes::MONO)
            .size(texto::MONO_SM.0)
            .color(p.text_muted),
    )
    .padding(Padding::from([0.0, espacio::S1]))
    .style(move |_: &Theme| container::Style {
        border: Border {
            width: borde::FINO,
            color: p.border,
            radius: 0.0.into(),
        },
        ..Default::default()
    })
    .into()
}

fn estilo_control(p: Paleta, cierre: bool) -> impl Fn(&Theme, button::Status) -> button::Style {
    move |_, estado| {
        let (fondo, tinta) = match estado {
            button::Status::Hovered | button::Status::Pressed => {
                if cierre {
                    (Some(p.error), p.on_error)
                } else {
                    (Some(p.text), p.bg)
                }
            }
            _ => (None, p.text),
        };
        button::Style {
            background: fondo.map(Background::Color),
            text_color: tinta,
            border: Border::default(),
            ..Default::default()
        }
    }
}

fn boton_ventana<'a, M: Clone + 'a>(
    p: Paleta,
    contenido: Element<'a, M>,
    cierre: bool,
    mensaje: M,
) -> Element<'a, M> {
    button(
        container(contenido)
            .center_x(Length::Fill)
            .center_y(Length::Fill),
    )
    .width(medida::BARRA_TITULO + 8.0)
    .height(medida::BARRA_TITULO)
    .padding(0)
    .on_press(mensaje)
    .style(estilo_control(p, cierre))
    .into()
}

pub fn barra_titulo<'a, M: Clone + 'a>(p: Paleta, m: &MensajesShell<'a, M>) -> Element<'a, M> {
    let marca = text("ICARO LAUNCHER")
        .font(fuentes::DISPLAY)
        .size(13.0)
        .color(p.text);
    let buscar = button(
        container(
            row![
                icono(Icono::Buscar, Tam::Base, p.text_muted),
                text("Buscar instancias, mods, servidores")
                    .font(fuentes::CUERPO)
                    .size(texto::BODY_SM.0)
                    .color(p.text_muted),
                kbd(p, "Ctrl K"),
            ]
            .spacing(espacio::S2)
            .align_y(Alignment::Center),
        )
        .center_y(Length::Fill),
    )
    .height(medida::CONTROL_SM)
    .padding(Padding::from([0.0, espacio::S3]))
    .on_press(m.buscar.clone())
    .style(move |_, estado| {
        let activo = matches!(estado, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: Some(Background::Color(p.surface_sunken)),
            text_color: p.text,
            border: Border {
                width: borde::MEDIO,
                color: if activo { p.text } else { p.border_strong },
                radius: 0.0.into(),
            },
            ..Default::default()
        }
    });
    let controles = row![
        boton_ventana(
            p,
            icono_heredado(Icono::Menos, Tam::Base).into(),
            false,
            m.minimizar.clone()
        ),
        boton_ventana(p, icono_heredado(Icono::Maximizar, Tam::Base).into(), false, m.maximizar.clone()),
        boton_ventana(
            p,
            icono_heredado(Icono::Cerrar, Tam::Base).into(),
            true,
            m.cerrar.clone()
        ),
    ];
    let fila = row![
        container(marca).padding(Padding::from([0.0, espacio::S4])),
        Space::with_width(Length::Fill),
        buscar,
        Space::with_width(Length::Fill),
        controles,
    ]
    .height(medida::BARRA_TITULO)
    .align_y(Alignment::Center);
    column![
        mouse_area(
            container(fila)
                .width(Length::Fill)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(p.surface)),
                    ..Default::default()
                })
        )
        .on_press(m.arrastrar.clone()),
        regla(p.text, borde::MEDIO),
    ]
    .into()
}

fn item_nav<'a, M: Clone + 'a>(
    p: Paleta,
    seccion: Seccion,
    activa: bool,
    colapsada: bool,
    mensaje: M,
) -> Element<'a, M> {
    let tinta = if activa { p.on_accent } else { p.text };
    let mut fila = row![icono_heredado(seccion.icono(), Tam::Base)]
        .spacing(espacio::S3)
        .align_y(Alignment::Center);
    if !colapsada {
        fila = fila
            .push(
                text(seccion.nombre())
                    .font(fuentes::CUERPO_NEGRITA)
                    .size(texto::BODY.0),
            )
            .push(Space::with_width(Length::Fill))
            .push(
                text(seccion.atajo().to_string())
                    .font(fuentes::MONO)
                    .size(texto::MONO_SM.0),
            );
    }
    let ancho = if colapsada {
        Length::Fixed(medida::BARRA_LATERAL_COLAPSADA - 2.0 * espacio::S2)
    } else {
        Length::Fill
    };
    let contenido = container(fila).center_y(Length::Fill);
    let (contenido, padding) = if colapsada {
        (contenido.center_x(Length::Fill), Padding::ZERO)
    } else {
        (contenido, Padding::from([0.0, espacio::S3]))
    };
    button(contenido)
        .width(ancho)
        .height(medida::CONTROL_LG)
        .padding(padding)
        .on_press(mensaje)
        .style(move |_, estado| {
            let fondo = if activa {
                Some(p.accent)
            } else if matches!(estado, button::Status::Hovered | button::Status::Pressed) {
                Some(p.surface_hover)
            } else {
                None
            };
            button::Style {
                background: fondo.map(Background::Color),
                text_color: tinta,
                border: Border::default(),
                ..Default::default()
            }
        })
        .into()
}

fn indicador_descargas<'a, M: 'a>(p: Paleta, d: &DescargasActivas) -> Element<'a, M> {
    let titulo = if d.cantidad == 1 {
        "1 descarga".to_string()
    } else {
        format!("{} descargas", d.cantidad)
    };
    column![
        row![
            text(titulo.to_uppercase())
                .font(fuentes::ETIQUETA)
                .size(texto::LABEL.0),
            Space::with_width(Length::Fill),
            text(d.velocidad.clone())
                .font(fuentes::MONO)
                .size(texto::MONO_SM.0)
                .color(p.text_muted),
        ],
        progress_bar(0.0..=1.0, d.avance.clamp(0.0, 1.0))
            .height(8.0)
            .style(move |_: &Theme| progress_bar::Style {
                background: Background::Color(p.surface_sunken),
                bar: Background::Color(p.text),
                border: Border {
                    width: borde::FINO,
                    color: p.border_strong,
                    radius: 0.0.into(),
                },
            }),
    ]
    .spacing(espacio::S2)
    .padding(Padding::from([0.0, espacio::S2]))
    .into()
}

/// Bloque de cuenta: cabeza de skin, nombre y proveedor, sobre una regla de
/// 1 px. Al pulsarlo se abre el menú de cuentas.
fn bloque_cuenta<'a, M: Clone + 'a>(p: Paleta, c: &Cuenta<'a>, al_pulsar: M, menu: Option<crate::instancias::MenuTarjeta<'a, M>>) -> Element<'a, M> {
    let bloque = crate::cuenta::bloque_cuenta(p, c.estado, c.nombre, c.proveedor, c.textura, al_pulsar);
    // MenuCuenta: unido al bloque, crece hacia arriba alineado a su izquierda.
    // Ocupa la barra hasta sus bordes. Los 2 px de la derecha quedan libres
    // para la línea de la barra; al abrirse, la pestaña los cubre y su borde
    // derecho coincide con esa línea.
    let bloque: Element<'a, M> = container(bloque)
        .width(Length::Fill)
        .padding(Padding { top: 0.0, right: borde::MEDIO, bottom: 0.0, left: 0.0 })
        .into();
    match menu {
        Some(m) => crate::menu_anclado::menu_unido(p, bloque, m.contenido, true, m.cerrar, crate::menu_anclado::Alineacion::Izquierda, crate::menu_anclado::Direccion::Arriba, m.progreso),
        None => bloque,
    }
}

fn barra_lateral<'a, M: Clone + 'a>(
    p: Paleta,
    activa: Option<Seccion>,
    colapsada: bool,
    cuenta: &Cuenta<'a>,
    descargas: Option<&DescargasActivas>,
    aviso: Option<Element<'a, M>>,
    menu_cuenta: Option<crate::instancias::MenuTarjeta<'a, M>>,
    m: &MensajesShell<'a, M>,
) -> Element<'a, M> {
    let mut items = column![].spacing(espacio::S1);
    for s in Seccion::TODAS {
        items = items.push(item_nav(p, s, Some(s) == activa, colapsada, (m.ir_a)(s)));
    }
    let (icono, etiqueta, alineacion) = if colapsada {
        (
            Icono::ChevronDerecha,
            "Desplegar barra lateral",
            iced::alignment::Horizontal::Center,
        )
    } else {
        (
            Icono::ChevronIzquierda,
            "Plegar barra lateral",
            iced::alignment::Horizontal::Right,
        )
    };
    let control = crate::botones::boton_icono_estilo(
        p,
        icono,
        etiqueta,
        crate::estilo::boton_fantasma(p),
        m.alternar_barra.clone(),
    );
    let mut pie =
        column![container(control).width(Length::Fill).align_x(alineacion)].spacing(espacio::S3);
    // Pastilla de AvisoActualizacion: también con la barra plegada.
    if let Some(aviso) = aviso {
        pie = pie.push(aviso);
    }
    if !colapsada {
        if let Some(d) = descargas {
            pie = pie.push(indicador_descargas(p, d));
        }
        pie = pie.push(
            container(Space::new(Length::Fill, Length::Fixed(borde::FINO))).style(move |_: &Theme| container::Style {
                background: Some(Background::Color(p.border)),
                ..Default::default()
            }),
        );
    }
    let ancho = if colapsada {
        medida::BARRA_LATERAL_COLAPSADA
    } else {
        medida::BARRA_LATERAL
    };
    let cuerpo = column![items, Space::with_height(Length::Fill), pie];
    let mut contenido = column![row![
        container(cuerpo).padding(espacio::S2).width(ancho - borde::MEDIO).height(Length::Fill),
        Space::with_width(borde::MEDIO),
    ]
    .height(Length::Fill)];
    if !colapsada {
        contenido = contenido.push(bloque_cuenta(p, cuenta, m.cuenta.clone(), menu_cuenta));
    }
    // La línea de la barra va debajo del contenido, de arriba abajo.
    let linea = row![
        Space::with_width(Length::Fill),
        container(Space::new(Length::Fixed(borde::MEDIO), Length::Fill)).style(move |_: &Theme| container::Style {
            background: Some(Background::Color(p.text)),
            ..Default::default()
        }),
    ]
    .height(Length::Fill);
    container(iced::widget::stack![linea, contenido].width(ancho).height(Length::Fill))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(p.surface)),
            ..Default::default()
        })
        .into()
}

/// Ventana completa. `pie` es la barra de juego; si no hay, no se dibuja.
#[allow(clippy::too_many_arguments)]
pub fn app_shell<'a, M: Clone + 'a>(
    p: Paleta,
    activa: impl Into<Option<Seccion>>,
    colapsada: bool,
    cuenta: Cuenta<'a>,
    descargas: Option<DescargasActivas>,
    contenido: Element<'a, M>,
    pie: Option<Element<'a, M>>,
    m: MensajesShell<'a, M>,
) -> Element<'a, M> {
    app_shell_con_aviso(p, activa, colapsada, cuenta, descargas, None, None, contenido, pie, m)
}

/// Como [`app_shell`], con la pastilla de una actualización en el pie de la
/// barra lateral.
#[allow(clippy::too_many_arguments)]
pub fn app_shell_con_aviso<'a, M: Clone + 'a>(
    p: Paleta,
    activa: impl Into<Option<Seccion>>,
    colapsada: bool,
    cuenta: Cuenta<'a>,
    descargas: Option<DescargasActivas>,
    aviso: Option<Element<'a, M>>,
    menu_cuenta: Option<crate::instancias::MenuTarjeta<'a, M>>,
    contenido: Element<'a, M>,
    pie: Option<Element<'a, M>>,
    m: MensajesShell<'a, M>,
) -> Element<'a, M> {
    let lateral = barra_lateral(p, activa.into(), colapsada, &cuenta, descargas.as_ref(), aviso, menu_cuenta, &m);
    let mut derecha = column![container(contenido)
        .width(Length::Fill)
        .height(Length::Fill)];
    if let Some(pie) = pie {
        derecha = derecha.push(regla(p.text, borde::MEDIO)).push(pie);
    }
    let cuerpo = row![lateral, derecha.width(Length::Fill)];
    container(column![barra_titulo(p, &m), cuerpo])
        .width(Length::Fill)
        .height(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(p.bg)),
            text_color: Some(p.text),
            ..Default::default()
        })
        .into()
}
