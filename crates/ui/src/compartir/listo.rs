// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! CompartidoListo: lo que ve el anfitrión justo después de publicar. El
//! código para enviar, quién se unió y los ajustes del código.

use super::{comun::*, *};
use crate::{
    componentes::{Estado, Variante},
    estilo, fuentes,
    iconos::{icono, Icono, Tam},
    tema::Paleta,
};
use iced::{
    widget::{button, column, container, rich_text, row, span, stack, text, Column, Space},
    Alignment, Background, Border, Color, Element, Length, Padding,
};

/// Estado del código mostrado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoCodigo {
    Generando,
    Vigente,
    /// Recién cambiado: el anterior ya no sirve.
    Nuevo,
    Completo,
    Vencido,
    Anulado,
}

#[derive(Debug, Clone)]
pub struct PersonaUnida {
    pub nombre: String,
    /// «se unió hace 12 min»; vacío si no se conoce.
    pub detalle: String,
}

#[derive(Debug, Clone)]
pub struct CompartidoListo {
    pub instancia: String,
    /// «Revisión 1 · 21 mods, 9 configuraciones y 2 scripts».
    pub resumen: String,
    /// `7QKM-4XPT-R2C9`.
    pub codigo: String,
    pub estado: EstadoCodigo,
    pub cupo: u32,
    pub aceptados: u32,
    pub personas: Vec<PersonaUnida>,
    pub caducidad: Caducidad,
    /// «Hasta mañana, martes 10-11 a las 04:15».
    pub vence: String,
    /// «Venció hoy a las 04:15.»
    pub vencio: String,
    pub confirmar_cambio: bool,
    pub copiado: bool,
    pub copiado_mensaje: bool,
    /// Los ajustes esperan a que el servicio responda.
    pub ocupado: bool,
}

const MAX_CUPO: u32 = 50;
/// La barra muestra un bloque por plaza hasta 25.
const MAX_BLOQUES: u32 = 25;

fn t<'a, M: 'a>(
    s: impl Into<String>,
    f: iced::Font,
    tam: f32,
    alto: f32,
    c: Color,
) -> Element<'a, M> {
    text(s.into())
        .font(f)
        .size(tam)
        .line_height(iced::Pixels(alto))
        .color(c)
        .into()
}

fn caja(
    fondo: Option<Color>,
    borde: Color,
    ancho: f32,
) -> impl Fn(&iced::Theme) -> container::Style {
    move |_| container::Style {
        background: fondo.map(Background::Color),
        border: Border {
            color: borde,
            width: ancho,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

/// `ic-cmp-codigo`: celdas de 36 × 48 con borde de 2 px; guiones de 14 px.
pub fn celdas<'a, M: 'a>(p: Paleta, codigo: &str, anulado: bool) -> Element<'a, M> {
    let mut fila = row![].spacing(5).align_y(Alignment::Center);
    for c in codigo.chars() {
        if c == '-' {
            fila = fila.push(
                container(t("-", fuentes::MONO_MEDIO, 24., 24., p.text_muted))
                    .center_x(14)
                    .center_y(48),
            );
            continue;
        }
        let letra = container(t(
            c.to_string(),
            fuentes::MONO_MEDIO,
            24.,
            24.,
            if anulado { p.text_disabled } else { p.text },
        ))
        .center_x(36)
        .center_y(48);
        let celda: Element<'a, M> = if anulado {
            stack![
                container(Space::new(36, 48)).style(caja(None, p.border_strong, 2.)),
                container(rayado(p.border)).width(36).height(48).padding(2),
                letra,
            ]
            .into()
        } else {
            letra.style(caja(Some(p.surface_sunken), p.text, 2.)).into()
        };
        fila = fila.push(celda);
    }
    fila.into()
}

/// `ic-cl__aviso`: borde de 1 px, regla de color de 3 px a la izquierda.
fn aviso_linea<'a>(p: Paleta, i: Icono, color: Color, texto: String) -> Element<'a, Accion> {
    stack![
        container(
            row![
                icono(i, Tam::Sm, p.text),
                t(texto, fuentes::CUERPO, 13., 19., p.text)
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        )
        .padding([10., 14.])
        .width(Length::Fill)
        .style(caja(Some(p.surface), p.border_strong, 1.)),
        container(Space::new(3, Length::Fill)).style(estilo::bloque(color)),
    ]
    .height(Length::Shrink)
    .into()
}

/// `ic-seg--sm`: borde de 2 px, opciones de 28 px separadas por 1 px.
fn segmentado<'a>(p: Paleta, actual: Caducidad, ocupado: bool) -> Element<'a, Accion> {
    let mut fila = row![];
    for (i, (c, nombre)) in [
        (Caducidad::Horas24, "24 horas"),
        (Caducidad::Dias2, "2 días"),
        (Caducidad::Semana, "1 semana"),
    ]
    .into_iter()
    .enumerate()
    {
        if i > 0 {
            fila = fila.push(container(Space::new(1, 28)).style(estilo::bloque(p.border_strong)));
        }
        let activo = c == actual;
        fila = fila.push(
            button(
                container(
                    text(nombre.to_uppercase())
                        .font(fuentes::ETIQUETA)
                        .size(10)
                        .wrapping(iced::widget::text::Wrapping::None),
                )
                .center_y(Length::Fill),
            )
            .height(28)
            .padding([0., 10.])
            .on_press_maybe((!ocupado && !activo).then_some(Accion::Caducidad(c)))
            .style(move |_, s| button::Style {
                background: Some(Background::Color(if activo {
                    p.text
                } else if matches!(s, button::Status::Hovered) {
                    p.surface_hover
                } else {
                    Color::TRANSPARENT
                })),
                text_color: if activo { p.bg } else { p.text },
                ..Default::default()
            }),
        );
    }
    container(fila).style(caja(None, p.text, 2.)).into()
}

/// Paso − n +: botones de 28 px centrados y la cifra al medio.
fn paso<'a>(p: Paleta, n: u32, minimo: u32, ocupado: bool) -> Element<'a, Accion> {
    let boton = |i: Icono, a: Option<Accion>| -> Element<'a, Accion> {
        button(container(crate::botones::icono_heredado(i, Tam::Base)).center(Length::Fill))
            .width(28)
            .height(28)
            .padding(0)
            .on_press_maybe(a)
            .style(estilo::boton_secundario(p))
            .into()
    };
    row![
        boton(
            Icono::Menos,
            (!ocupado && n > minimo).then(|| Accion::Cupo(n - 1))
        ),
        container(t(n.to_string(), fuentes::MONO, 13., 20., p.text))
            .width(28)
            .center_x(28),
        boton(
            Icono::Mas,
            (!ocupado && n < MAX_CUPO).then(|| Accion::Cupo(n + 1))
        ),
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .into()
}

/// Fila de ajuste: título y ayuda a la izquierda, control a la derecha.
fn ajuste<'a>(
    p: Paleta,
    titulo: &str,
    ayuda: String,
    control: Element<'a, Accion>,
) -> Element<'a, Accion> {
    row![
        column![
            t(titulo.to_owned(), fuentes::CUERPO, 14., 21., p.text),
            t(ayuda, fuentes::CUERPO, 13., 19., p.text_muted)
        ]
        .spacing(4)
        .width(Length::Fill),
        control
    ]
    .spacing(24)
    .align_y(Alignment::Center)
    .padding([4., 0.])
    .into()
}

/// Bloques de cupo: uno por plaza hasta 25, llenos los ocupados.
fn cupo<'a>(p: Paleta, aceptados: u32, cupo: u32) -> Element<'a, Accion> {
    let total = cupo.min(MAX_BLOQUES).max(1);
    let mut fila = row![].spacing(4);
    for i in 0..total {
        let lleno = i < aceptados.min(total);
        fila = fila.push(
            container(Space::new(Length::Fill, 10))
                .width(Length::FillPortion(1))
                .style(if lleno {
                    caja(Some(p.text), p.text, 0.)
                } else {
                    caja(Some(p.surface_sunken), p.border, 1.)
                }),
        );
    }
    fila.into()
}

pub fn compartido_listo<'a>(p: Paleta, m: &CompartidoListo) -> Element<'a, Accion> {
    let cabecera = container(
        row![
            container(icono(Icono::Check, Tam::Lg, p.bg))
                .width(56)
                .height(56)
                .center(56)
                .style(estilo::bloque(p.text)),
            column![
                titulo(p, format!("Ya compartes {}", m.instancia)),
                t(m.resumen.clone(), fuentes::CUERPO, 13., 19., p.text_muted)
            ]
            .spacing(4)
            .width(Length::Fill),
        ]
        .spacing(18)
        .align_y(Alignment::Center),
    )
    .padding([24., 32.]);

    // Banda del código, a sangre sobre `surface-sunken`.
    let (insignia, etiqueta) = match m.estado {
        EstadoCodigo::Completo => (Estado::Info, "Cupo completo"),
        EstadoCodigo::Vencido => (Estado::Aviso, "Vencido"),
        EstadoCodigo::Anulado => (Estado::Error, "Anulado"),
        EstadoCodigo::Generando => (Estado::Info, "Generando"),
        EstadoCodigo::Vigente | EstadoCodigo::Nuevo => (Estado::Exito, "Vigente"),
    };
    let mut banda = column![row![
        t(
            "ENVÍA ESTE CÓDIGO A TUS AMIGOS",
            fuentes::ETIQUETA,
            11.,
            14.,
            p.text_muted
        ),
        Space::new(Length::Fill, 0),
        insignia_plana(p, insignia, etiqueta)
    ]
    .align_y(Alignment::Center)]
    .spacing(16);
    let sin_codigo = matches!(m.estado, EstadoCodigo::Vencido | EstadoCodigo::Anulado);
    if !m.codigo.is_empty() {
        banda = banda.push(celdas(p, &m.codigo, sin_codigo));
    }
    match m.estado {
        EstadoCodigo::Nuevo => {
            banda = banda.push(aviso_linea(
                p,
                Icono::Check,
                p.success,
                "Código nuevo listo. El anterior ya no sirve; quienes se unieron con él conservan su acceso.".into(),
            ))
        }
        EstadoCodigo::Vencido => {
            banda = banda.push(aviso_linea(
                p,
                Icono::Reloj,
                p.warning,
                format!("{} Genera uno nuevo para que se una alguien más.", m.vencio),
            ))
        }
        EstadoCodigo::Anulado => {
            banda = banda.push(aviso_linea(
                p,
                Icono::Candado,
                p.warning,
                "Anulaste el código: nadie más puede unirse. Quienes ya entraron conservan su acceso.".into(),
            ))
        }
        EstadoCodigo::Completo => {
            banda = banda.push(aviso_linea(
                p,
                Icono::Usuarios,
                p.info,
                format!(
                    "Ya se unieron las {} personas. Sube el límite o genera otro código.",
                    m.cupo
                ),
            ))
        }
        _ => {}
    }
    banda = banda.push(if sin_codigo {
        Element::from(btn(
            p,
            Some(Icono::Refrescar),
            "Generar código nuevo",
            None,
            Variante::Primario,
            (!m.ocupado).then_some(Accion::GenerarInvitacion),
        ))
    } else {
        row![
            btn(
                p,
                Some(if m.copiado {
                    Icono::Check
                } else {
                    Icono::Copiar
                }),
                if m.copiado {
                    "Copiado"
                } else {
                    "Copiar código"
                },
                None,
                Variante::Primario,
                (!m.codigo.is_empty()).then_some(Accion::CopiarCodigo),
            ),
            btn(
                p,
                Some(if m.copiado_mensaje {
                    Icono::Check
                } else {
                    Icono::Documento
                }),
                if m.copiado_mensaje {
                    "Copiado"
                } else {
                    "Copiar mensaje con instrucciones"
                },
                None,
                Variante::Secundario,
                (!m.codigo.is_empty()).then_some(Accion::CopiarMensaje),
            ),
        ]
        .spacing(8)
        .into()
    });
    let banda = column![
        regla(p, false),
        container(banda)
            .padding([24., 32.])
            .width(Length::Fill)
            .style(estilo::bloque(p.surface_sunken)),
        regla(p, false),
    ];

    // Personas.
    let mut personas = column![
        row![
            t("Personas", fuentes::CUERPO_NEGRITA, 16., 22., p.text),
            Space::new(Length::Fill, 0),
            t(
                format!("{} de {} se unieron", m.aceptados, m.cupo),
                fuentes::MONO,
                11.,
                16.,
                p.text
            )
        ]
        .align_y(Alignment::Center),
        cupo(p, m.aceptados, m.cupo),
    ]
    .spacing(16);
    if m.personas.is_empty() {
        personas = personas.push(
            container(
                column![
                    t("Nadie se ha unido todavía", fuentes::CUERPO, 13., 19., p.text),
                    t(
                        "Cuando alguien pegue el código, aparece aquí. Puedes salir de esta página: lo verás en la publicación.",
                        fuentes::CUERPO,
                        13.,
                        19.,
                        p.text_muted
                    )
                ]
                .spacing(4),
            )
            .padding([16., 18.])
            .width(Length::Fill)
            .style(caja(Some(p.surface_sunken), p.border, 1.)),
        );
    } else {
        let n = m.personas.len();
        let mut lista = Column::new();
        for (i, persona) in m.personas.iter().enumerate() {
            lista = lista.push(
                container(
                    row![
                        crate::servidores::avatar_servidor_de(p, &persona.nombre, 34.),
                        container(t(persona.nombre.clone(), fuentes::CUERPO, 13., 19., p.text))
                            .width(Length::Fill),
                        t(
                            persona.detalle.clone(),
                            fuentes::MONO,
                            11.,
                            16.,
                            p.text_muted
                        ),
                    ]
                    .spacing(14)
                    .align_y(Alignment::Center),
                )
                .padding([10., 0.]),
            );
            if i + 1 < n {
                lista = lista.push(regla(p, false));
            }
        }
        personas = personas.push(lista);
    }
    let personas = container(personas).padding([24., 32.]);

    // Ajustes del código.
    let minimo = m.aceptados.max(1);
    let cambiar: Element<'a, Accion> = if m.confirmar_cambio {
        container(
            column![
                rich_text![
                    span("¿Cambiar el código? ").font(fuentes::CUERPO_NEGRITA),
                    span(format!(
                        "{} deja de servir en el acto. Tendrás que enviar el nuevo a quien aún no se una.",
                        m.codigo
                    )),
                ]
                .font(fuentes::CUERPO)
                .size(13)
                .color(p.text),
                row![
                    pequeno(p, "Cancelar", Variante::Secundario, Some(Accion::CancelarCambioCodigo)),
                    pequeno(
                        p,
                        "Cambiar código",
                        Variante::Primario,
                        (!m.ocupado).then_some(Accion::CambiarCodigo)
                    ),
                ]
                .spacing(8)
            ]
            .spacing(12),
        )
        .padding([14., 16.])
        .width(Length::Fill)
        .style(caja(Some(p.accent_soft), p.text, 2.))
        .into()
    } else {
        ajuste(
            p,
            "Cambiar código",
            "Útil si lo compartiste donde no debías. Quienes ya se unieron no pierden su acceso."
                .into(),
            crate::botones::boton(
                p,
                Some(Icono::Refrescar),
                "Cambiar código",
                Variante::Secundario,
                crate::botones::TamBoton::Sm,
                (!m.ocupado && !sin_codigo).then_some(Accion::PedirCambioCodigo),
            ),
        )
    };
    let anular = crate::botones::boton_estilo(
        None,
        "Anular",
        crate::botones::TamBoton::Sm,
        move |tema, s| {
            let mut e = estilo::boton_fantasma(p)(tema, s);
            e.text_color = if matches!(s, button::Status::Disabled) {
                p.text_disabled
            } else {
                p.error
            };
            e
        },
        (!m.ocupado && !sin_codigo).then_some(Accion::RevocarCodigo),
    );
    let ajustes = container(
        column![
            t(
                "Ajustes del código",
                fuentes::CUERPO_NEGRITA,
                16.,
                22.,
                p.text
            ),
            ajuste(
                p,
                "Caduca en",
                m.vence.clone(),
                segmentado(p, m.caducidad, m.ocupado)
            ),
            ajuste(
                p,
                "Personas que pueden unirse",
                if m.cupo == 1 {
                    "Con 1, el código sirve para una sola persona.".into()
                } else {
                    "Reconectarse no gasta cupo.".into()
                },
                paso(p, m.cupo, minimo, m.ocupado),
            ),
            cambiar,
            ajuste(
                p,
                "Anular código",
                "Nadie más podrá unirse hasta que generes otro. No revoca a nadie.".into(),
                anular
            ),
        ]
        .spacing(16),
    )
    .padding([24., 32.]);

    let cuerpo = column![banda, personas, regla(p, false), ajustes];
    let pie = row![
        container(t(
            "Todo esto también está en Servidores › Ver publicación.",
            fuentes::CUERPO,
            13.,
            19.,
            p.text_muted
        ))
        .width(Length::Fill),
        btn(
            p,
            None,
            "Ir a la publicación",
            None,
            Variante::Secundario,
            Some(Accion::VerPublicacion)
        ),
        btn(
            p,
            None,
            "Listo",
            None,
            Variante::Primario,
            Some(Accion::Cerrar)
        ),
    ]
    .spacing(12)
    .align_y(Alignment::Center);
    container(column![
        cabecera,
        crate::scroll::desplazable(container(cuerpo).padding(Padding {
            bottom: 8.,
            ..Padding::ZERO
        })),
        column![regla(p, false), container(pie).padding([18., 32.])],
    ])
    .width(Length::Fill)
    .max_width(700)
    .height(Length::Fill)
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_pagina_se_construye_en_todos_sus_estados() {
        let mut m = CompartidoListo {
            instancia: "Amigos".into(),
            resumen: "Revisión 1 · 21 mods".into(),
            codigo: "7QKM-4XPT-R2C9".into(),
            estado: EstadoCodigo::Vigente,
            cupo: 10,
            aceptados: 3,
            personas: vec![PersonaUnida {
                nombre: "Equipo 9e2a 7b14".into(),
                detalle: String::new(),
            }],
            caducidad: Caducidad::Horas24,
            vence: "Hasta mañana".into(),
            vencio: "Venció hoy a las 04:15.".into(),
            confirmar_cambio: true,
            copiado: false,
            copiado_mensaje: false,
            ocupado: false,
        };
        for e in [
            EstadoCodigo::Generando,
            EstadoCodigo::Vigente,
            EstadoCodigo::Nuevo,
            EstadoCodigo::Completo,
            EstadoCodigo::Vencido,
            EstadoCodigo::Anulado,
        ] {
            m.estado = e;
            let _ = compartido_listo(crate::tema::TINTA, &m);
            let _ = compartido_listo(crate::tema::PIEDRA, &m);
        }
    }
}
