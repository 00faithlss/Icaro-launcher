// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! SelectorInstanciaCompartir: el paso antes de RevisionPublicacion. Una
//! tarjeta por instancia con selección única; las compartidas y las que están
//! en uso no se pueden elegir.

use super::{comun::*, *};
use crate::{
    componentes::{self, Estado, Variante},
    estilo, fuentes,
    iconos::{icono, Icono, Tam},
    tema::Paleta,
};
use iced::{
    widget::{button, canvas, column, container, rich_text, row, span, text, Column, Space},
    Alignment, Background, Border, Color, Element, Length, Padding,
};

/// Caso de cada tarjeta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EstadoOpcion {
    Libre,
    /// Sin mods: basta la misma versión de Minecraft.
    Vanilla,
    /// Ya la compartes: no se elige; se abre su publicación.
    Compartida {
        revision: u64,
        personas: usize,
    },
    /// Minecraft está abierto con ella.
    EnUso,
}

#[derive(Debug, Clone)]
pub struct OpcionInstancia {
    pub id: String,
    pub nombre: String,
    /// `1.21.1 · Fabric 0.16.9`.
    pub version: String,
    /// `38 mods` o `Sin mods`; vacío mientras se cuenta.
    pub mods: String,
    /// `1,2 GB`; vacío mientras se mide.
    pub tamano: String,
    /// «Jugaste hace 2 h».
    pub jugado: String,
    pub estado: EstadoOpcion,
}
impl OpcionInstancia {
    pub fn elegible(&self) -> bool {
        matches!(self.estado, EstadoOpcion::Libre | EstadoOpcion::Vanilla)
    }
}

#[derive(Debug, Clone, Default)]
pub struct SelectorInstancia {
    pub opciones: Vec<OpcionInstancia>,
    pub elegida: Option<String>,
    pub busqueda: String,
}

/// Buscar aparece solo con más de cinco instancias.
const CON_BUSQUEDA: usize = 5;

fn t<'a>(
    s: impl Into<String>,
    f: iced::Font,
    tam: f32,
    alto: f32,
    c: Color,
) -> Element<'a, Accion> {
    text(s.into())
        .font(f)
        .size(tam)
        .line_height(iced::Pixels(alto))
        .color(c)
        .into()
}

fn tenue(c: Color) -> Color {
    Color { a: c.a * 0.5, ..c }
}

/// `ic-radio`: rombo de 20 px con borde de 2 px; marcado, punto de 8 px.
struct Rombo {
    borde: Color,
    fondo: Color,
    punto: Option<Color>,
}
impl<M> canvas::Program<M> for Rombo {
    type State = ();
    fn draw(
        &self,
        _: &(),
        renderer: &iced::Renderer,
        _: &iced::Theme,
        limites: iced::Rectangle,
        _: iced::mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        use canvas::{Frame, Path, Stroke};
        let mut f = Frame::new(renderer, limites.size());
        let c = f.center();
        let rombo = |r: f32| {
            Path::new(|b| {
                b.move_to(iced::Point::new(c.x, c.y - r));
                b.line_to(iced::Point::new(c.x + r, c.y));
                b.line_to(iced::Point::new(c.x, c.y + r));
                b.line_to(iced::Point::new(c.x - r, c.y));
                b.close();
            })
        };
        // Un cuadrado de 20 px girado 45° mide 28 px de punta a punta.
        let exterior = rombo(13.);
        f.fill(&exterior, self.fondo);
        f.stroke(
            &exterior,
            Stroke::default().with_color(self.borde).with_width(2.),
        );
        if let Some(punto) = self.punto {
            f.fill(&rombo(5.6), punto);
        }
        vec![f.into_geometry()]
    }
}

fn radio<'a>(p: Paleta, marcado: bool, apagado: bool) -> Element<'a, Accion> {
    let f = |c: Color| if apagado { tenue(c) } else { c };
    canvas(Rombo {
        borde: f(if marcado { p.text } else { p.border_strong }),
        fondo: f(p.surface_sunken),
        punto: marcado.then_some(f(p.text)),
    })
    .width(28)
    .height(28)
    .into()
}

fn tarjeta<'a>(p: Paleta, o: &OpcionInstancia, elegida: bool) -> Element<'a, Accion> {
    let no = o.estado == EstadoOpcion::EnUso;
    let compartida = matches!(o.estado, EstadoOpcion::Compartida { .. });
    let tinta = |c: Color| if no { tenue(c) } else { c };
    let mut tecnica = vec![o.version.clone()];
    if !o.mods.is_empty() {
        tecnica.push(o.mods.clone());
    }
    if !compartida && !o.tamano.is_empty() {
        tecnica.push(o.tamano.clone());
    }
    let mut nombre = row![t(
        o.nombre.clone(),
        fuentes::CUERPO_NEGRITA,
        15.,
        21.,
        tinta(p.text)
    )]
    .spacing(10)
    .align_y(Alignment::Center);
    if compartida {
        nombre = nombre.push(insignia_plana(p, Estado::Exito, "Compartida"));
    }
    let mut info = column![
        nombre,
        t(
            tecnica.join(" · "),
            fuentes::MONO,
            11.,
            16.,
            tinta(p.text_muted)
        )
    ]
    .spacing(3)
    .width(Length::Fill);
    let nota = match &o.estado {
        EstadoOpcion::EnUso => Some((
            "Minecraft está abierto con esta instancia. Ciérralo para compartirla.".to_string(),
            p.warning,
        )),
        EstadoOpcion::Compartida { revision, personas } => Some((
            format!(
                "Ya la compartes · revisión {revision} · {}",
                if *personas == 1 {
                    "1 persona".to_string()
                } else {
                    format!("{personas} personas")
                }
            ),
            p.text_muted,
        )),
        EstadoOpcion::Vanilla => Some((
            "Sin mods: tus amigos solo necesitan la misma versión de Minecraft.".to_string(),
            p.text_muted,
        )),
        EstadoOpcion::Libre => None,
    };
    if let Some((nota, color)) = nota {
        info = info.push(t(nota, fuentes::CUERPO, 13., 19., color));
    }
    let izquierda: Element<'a, Accion> = if compartida {
        container(icono(Icono::Check, Tam::Sm, p.bg))
            .width(28)
            .height(28)
            .center(28)
            .into()
    } else {
        radio(p, elegida, no)
    };
    let marca_compartida: Element<'a, Accion> = if compartida {
        // `ic-si__marca`: bloque de 20 px con check, centrado en la columna del radio.
        container(
            container(icono(Icono::Check, Tam::Sm, p.bg))
                .width(20)
                .height(20)
                .center(20)
                .style(estilo::bloque(p.text)),
        )
        .width(28)
        .height(28)
        .center(28)
        .into()
    } else {
        izquierda
    };
    let derecha: Element<'a, Accion> = if compartida {
        pequeno(
            p,
            "Ver publicación",
            Variante::Secundario,
            Some(Accion::VerPublicacionDe(o.id.clone())),
        )
    } else {
        text(o.jugado.clone())
            .font(fuentes::CUERPO)
            .size(13)
            .line_height(iced::Pixels(19.))
            .color(p.text_muted)
            .wrapping(iced::widget::text::Wrapping::None)
            .into()
    };
    let contenido = row![
        marca_compartida,
        crate::servidores::avatar_servidor_de(p, &o.nombre, 34.),
        info,
        derecha
    ]
    .spacing(16)
    .align_y(Alignment::Center);
    let fondo = if no || compartida {
        p.surface_sunken
    } else if elegida {
        p.accent_soft
    } else {
        p.surface
    };
    let base = move |sobre: bool| container::Style {
        background: Some(Background::Color(fondo)),
        border: Border {
            color: if elegida {
                p.text
            } else if sobre && o_elegible(no, compartida) {
                p.text_muted
            } else {
                p.border_strong
            },
            width: if elegida { 2. } else { 1. },
            radius: 0.0.into(),
        },
        ..Default::default()
    };
    // El relleno no cambia al elegir: el borde de 2 px se dibuja hacia dentro.
    if no || compartida {
        container(contenido)
            .padding([14., 18.])
            .width(Length::Fill)
            .style(move |_| base(false))
            .into()
    } else {
        button(contenido)
            .padding([14., 18.])
            .width(Length::Fill)
            .on_press(Accion::SeleccionarInstancia(o.id.clone()))
            .style(move |_, s| {
                let c = base(matches!(
                    s,
                    button::Status::Hovered | button::Status::Pressed
                ));
                button::Style {
                    background: c.background,
                    border: c.border,
                    text_color: p.text,
                    ..Default::default()
                }
            })
            .into()
    }
}

fn o_elegible(no: bool, compartida: bool) -> bool {
    !no && !compartida
}

/// Página del selector con el marco común de las fases.
pub fn selector_instancia<'a>(p: Paleta, m: &SelectorInstancia) -> Element<'a, Accion> {
    let q = m.busqueda.trim().to_lowercase();
    let visibles: Vec<_> = m
        .opciones
        .iter()
        .filter(|o| {
            q.is_empty()
                || format!("{} {}", o.nombre, o.version)
                    .to_lowercase()
                    .contains(&q)
        })
        .collect();
    let elegida = m
        .elegida
        .as_ref()
        .and_then(|id| m.opciones.iter().find(|o| &o.id == id && o.elegible()));
    let mut c = column![].spacing(20);
    if m.opciones.is_empty() {
        c = c.push(
            container(
                column![
                    t(
                        "Todavía no tienes instancias",
                        fuentes::CUERPO_NEGRITA,
                        16.,
                        22.,
                        p.text
                    ),
                    t(
                        "Crea una con los mods que quieras y vuelve aquí para compartirla.",
                        fuentes::CUERPO,
                        13.,
                        19.,
                        p.text_muted
                    ),
                    container(crate::botones::boton(
                        p,
                        Some(Icono::Mas),
                        "Crear instancia",
                        Variante::Secundario,
                        crate::botones::TamBoton::Sm,
                        Some(Accion::CrearInstancia),
                    ))
                    .padding(Padding {
                        top: 8.,
                        ..Padding::ZERO
                    }),
                ]
                .spacing(8),
            )
            .padding(32)
            .width(Length::Fill)
            .style(move |_| container::Style {
                background: Some(Background::Color(p.surface_sunken)),
                border: Border {
                    color: p.border,
                    width: 1.,
                    radius: 0.0.into(),
                },
                ..Default::default()
            }),
        );
    } else {
        if m.opciones.len() > CON_BUSQUEDA {
            c = c.push(componentes::campo_con_icono(
                p,
                Icono::Buscar,
                "Buscar instancia",
                &m.busqueda,
                Accion::BuscarInstancia,
            ));
        }
        if visibles.is_empty() {
            c = c.push(
                container(t(
                    "Ninguna instancia se llama así.",
                    fuentes::CUERPO,
                    13.,
                    19.,
                    p.text_muted,
                ))
                .padding([16., 0.]),
            );
        } else {
            c = c.push(
                Column::with_children(
                    visibles
                        .iter()
                        .map(|o| tarjeta(p, o, elegida.is_some_and(|e| e.id == o.id))),
                )
                .spacing(8),
            );
        }
    }
    let frase: Element<'a, Accion> = match elegida {
        Some(o) => rich_text![
            span("Vas a compartir "),
            span(o.nombre.clone()).font(fuentes::CUERPO_NEGRITA),
            span(if o.mods.is_empty() {
                String::new()
            } else {
                format!(" · {}", o.mods)
            }),
        ]
        .font(fuentes::CUERPO)
        .size(13)
        .color(p.text)
        .into(),
        None if m.opciones.is_empty() => Space::new(0, 0).into(),
        None => t("Elige una instancia", fuentes::CUERPO, 13., 19., p.text),
    };
    let acciones = row![
        container(frase).width(Length::Fill),
        btn(
            p,
            None,
            "Cancelar",
            None,
            Variante::Secundario,
            Some(Accion::Cancelar)
        ),
        btn(
            p,
            None,
            "Continuar",
            Some(Icono::Adelante),
            Variante::Primario,
            elegida.map(|_| Accion::Continuar)
        ),
    ];
    dialogo(
        p,
        720.,
        "¿Qué instancia compartes?",
        "Tus amigos tendrán una copia y podrán entrar a tu mundo con los mismos mods.",
        c,
        acciones,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opcion(id: &str, estado: EstadoOpcion) -> OpcionInstancia {
        OpcionInstancia {
            id: id.into(),
            nombre: id.into(),
            version: "1.21.1 · Fabric 0.16.9".into(),
            mods: "38 mods".into(),
            tamano: "1,2 GB".into(),
            jugado: "Jugaste hace 2 h".into(),
            estado,
        }
    }

    #[test]
    fn solo_se_eligen_las_libres_y_vanilla() {
        assert!(opcion("a", EstadoOpcion::Libre).elegible());
        assert!(opcion("a", EstadoOpcion::Vanilla).elegible());
        assert!(!opcion("a", EstadoOpcion::EnUso).elegible());
        assert!(!opcion(
            "a",
            EstadoOpcion::Compartida {
                revision: 1,
                personas: 1
            }
        )
        .elegible());
    }

    #[test]
    fn el_selector_se_construye_en_sus_estados() {
        let m = SelectorInstancia {
            opciones: vec![
                opcion("a", EstadoOpcion::Libre),
                opcion(
                    "b",
                    EstadoOpcion::Compartida {
                        revision: 12,
                        personas: 3,
                    },
                ),
                opcion("c", EstadoOpcion::EnUso),
                opcion("d", EstadoOpcion::Vanilla),
            ],
            elegida: Some("a".into()),
            busqueda: String::new(),
        };
        let _ = selector_instancia(crate::tema::TINTA, &m);
        let _ = selector_instancia(crate::tema::PIEDRA, &SelectorInstancia::default());
        let _ = selector_instancia(
            crate::tema::TINTA,
            &SelectorInstancia {
                busqueda: "zzz".into(),
                ..m
            },
        );
    }
}
