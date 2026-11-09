// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! RevisionPublicacion por tipo de contenido (versión 32 del diseño): el
//! anfitrión marca qué publica en la próxima revisión. Nada se comparte solo.

use super::{comun::*, *};
use crate::{
    botones,
    componentes::{self, Estado, Variante},
    estilo, fuentes,
    iconos::{icono, Icono, Tam},
    tema::Paleta,
};
use iced::{
    widget::{
        button, column, container, rich_text, row, scrollable, span, stack, text, Column, Space,
    },
    Alignment, Background, Border, Color, Element, Font, Length, Padding,
};

/// Área desplazable de la revisión; las secciones se llevan a la vista dentro de ella.
pub const ID_DESPLAZABLE: &str = "revision-publicacion";

fn id_seccion(t: TipoContenido) -> String {
    format!("revision-publicacion-{t:?}")
}

/// Abre la sección del tipo en la vista: mide su posición en el contenido y desplaza.
pub fn llevar_a_tipo<M: Send + 'static>(tipo: TipoContenido) -> iced::Task<M> {
    use iced::advanced::widget::{
        operate,
        operation::{Operation, Outcome, Scrollable},
        Id,
    };
    struct Busca {
        objetivo: Id,
        desplazable: Id,
        inicio: Option<f32>,
        y: Option<f32>,
    }
    impl Operation<Option<f32>> for Busca {
        fn container(
            &mut self,
            id: Option<&Id>,
            bounds: iced::Rectangle,
            hijos: &mut dyn FnMut(&mut dyn Operation<Option<f32>>),
        ) {
            if id == Some(&self.objetivo) {
                self.y = Some(bounds.y);
            }
            hijos(self);
        }
        fn scrollable(
            &mut self,
            _: &mut dyn Scrollable,
            id: Option<&Id>,
            _: iced::Rectangle,
            contenido: iced::Rectangle,
            _: iced::Vector,
        ) {
            if id == Some(&self.desplazable) {
                self.inicio = Some(contenido.y);
            }
        }
        fn finish(&self) -> Outcome<Option<f32>> {
            Outcome::Some(self.y.zip(self.inicio).map(|(y, inicio)| y - inicio))
        }
    }
    operate(Busca {
        objetivo: Id::new(id_seccion(tipo)),
        desplazable: Id::new(ID_DESPLAZABLE),
        inicio: None,
        y: None,
    })
    .then(|y| match y {
        Some(y) => scrollable::scroll_to(
            scrollable::Id::new(ID_DESPLAZABLE),
            scrollable::AbsoluteOffset {
                x: 0.,
                y: (y - 28.).max(0.),
            },
        ),
        None => iced::Task::none(),
    })
}

// --- Cálculo ---------------------------------------------------------------

#[derive(Default, Clone, Copy)]
struct Cuenta {
    publican: usize,
    total: usize,
}

struct Resumen<'a> {
    por: [Cuenta; 5],
    privados: usize,
    bloqueantes: Vec<&'a CambioPublicacion>,
    autorizaciones: usize,
    efectivos: usize,
    total: usize,
    con_problemas: usize,
}

fn indice(t: TipoContenido) -> usize {
    TipoContenido::TODOS
        .iter()
        .position(|x| *x == t)
        .unwrap_or(0)
}

fn sin_cambios(m: &RevisionPublicacion) -> bool {
    matches!(m.estado, EstadoRevisionPublicacion::SinCambios)
}

/// Sin cambios solo se ve lo privado desde antes, por si se quiere incluir.
fn en_vista(m: &RevisionPublicacion, c: &CambioPublicacion) -> bool {
    !c.integrado && (!sin_cambios(m) || c.grupo == GrupoPublicacion::Privado)
}

impl<'a> Resumen<'a> {
    fn calcular(m: &'a RevisionPublicacion) -> Self {
        let mut r = Resumen {
            por: [Cuenta::default(); 5],
            privados: 0,
            bloqueantes: vec![],
            autorizaciones: 0,
            efectivos: 0,
            total: 0,
            con_problemas: 0,
        };
        for c in m.cambios.iter().filter(|c| en_vista(m, c)) {
            let x = &mut r.por[indice(c.tipo)];
            x.total += 1;
            r.total += 1;
            if c.problema.is_some() {
                r.con_problemas += 1;
            }
            if c.marcado() {
                x.publican += 1;
                r.efectivos += 1;
                if c.problema.is_some() {
                    r.bloqueantes.push(c);
                }
                if c.propio && !c.autorizado {
                    r.autorizaciones += 1;
                }
            } else if c.queda_privado() {
                r.privados += 1;
            }
        }
        r
    }
    fn cuenta(&self, t: TipoContenido) -> Cuenta {
        self.por[indice(t)]
    }
    /// «Se publican 21 mods, 9 configuraciones y 2 scripts. 30 quedan privados.»
    fn frase(&self) -> String {
        let partes: Vec<String> = TipoContenido::TODOS
            .iter()
            .filter_map(|t| {
                let n = self.cuenta(*t).publican;
                let (uno, varios) = t.unidades();
                (n > 0).then(|| format!("{n} {}", if n == 1 { uno } else { varios }))
            })
            .collect();
        let Some((ultima, resto)) = partes.split_last() else {
            return String::new();
        };
        let lista = if resto.is_empty() {
            ultima.clone()
        } else {
            format!("{} y {ultima}", resto.join(", "))
        };
        let privados = match self.privados {
            0 => String::new(),
            1 => " 1 queda privado.".into(),
            n => format!(" {n} quedan privados."),
        };
        format!("Se publican {lista}.{privados}")
    }
}

/// Frase del pie para la selección actual.
pub fn frase_publicacion(m: &RevisionPublicacion) -> String {
    Resumen::calcular(m).frase()
}

/// Lo que marca un atajo y los nombres que quedan fuera por tener un problema.
pub fn elecciones_de_atajo(
    m: &RevisionPublicacion,
    atajo: AtajoPublicacion,
) -> (Vec<(String, Eleccion)>, Vec<String>) {
    let mut fuera = vec![];
    let mut elecciones = vec![];
    for c in m.cambios.iter().filter(|c| !c.integrado) {
        // Una configuración queda fuera si su mod es solo de cliente o tiene un problema.
        let mod_fuera = c.de_mod.as_ref().and_then(|(id, _)| {
            m.cambios
                .iter()
                .find(|x| &x.archivo.id == id)
                .map(|x| x.solo_cliente || x.problema.is_some())
        });
        let Some(marcar) = c.con_atajo(atajo, mod_fuera) else {
            continue;
        };
        if atajo == AtajoPublicacion::Necesario
            && c.problema.is_some()
            && matches!(c.tipo, TipoContenido::Mod | TipoContenido::Script)
            && !c.solo_cliente
        {
            fuera.push(c.archivo.nombre.clone());
        }
        elecciones.push((c.archivo.id.clone(), c.eleccion_para(marcar)));
    }
    (elecciones, fuera)
}

/// Línea de éxito tras aplicar un atajo, con la selección ya aplicada.
pub fn mensaje_atajo(m: &RevisionPublicacion, atajo: AtajoPublicacion, fuera: &[String]) -> String {
    match atajo {
        AtajoPublicacion::Todo => "Marcaste todo. Revisa abajo si algo tiene un problema.".into(),
        AtajoPublicacion::NadaNuevo => {
            "Lo nuevo queda privado. Se publican solo actualizaciones y retiradas.".into()
        }
        AtajoPublicacion::Necesario => {
            let frase = frase_publicacion(m);
            let lista = frase
                .strip_prefix("Se publican ")
                .and_then(|f| f.split('.').next())
                .unwrap_or_default();
            let mut texto = if lista.is_empty() {
                "Listo: nada nuevo cumple este atajo.".to_string()
            } else {
                format!("Listo: {lista}.")
            };
            match fuera {
                [] => {}
                [uno] => texto.push_str(&format!(" {uno} quedó fuera porque tiene un problema.")),
                varios => texto.push_str(&format!(
                    " {} quedaron fuera porque tienen un problema.",
                    varios.join(", ")
                )),
            }
            texto.push_str(" Ajusta abajo lo que quieras.");
            texto
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tri {
    No,
    Algunos,
    Todos,
}
fn tri(c: Cuenta) -> Tri {
    if c.publican == 0 {
        Tri::No
    } else if c.publican == c.total {
        Tri::Todos
    } else {
        Tri::Algunos
    }
}

/// Lo que dice una fila con la casilla vacía.
fn estado_no(c: &CambioPublicacion, base: u64) -> String {
    match c.grupo {
        GrupoPublicacion::Nuevo | GrupoPublicacion::Privado => "Queda privado".into(),
        GrupoPublicacion::Actualizaciones => format!(
            "Pospuesto: sigue la versión {}",
            if c.version_publicada.is_empty() {
                "publicada"
            } else {
                &c.version_publicada
            }
        ),
        GrupoPublicacion::Retiradas => format!("Sigue publicado como en la {base}"),
        GrupoPublicacion::Cambios => "Pospuesto".into(),
    }
}

fn coincide(m: &RevisionPublicacion, c: &CambioPublicacion) -> bool {
    let q = m.vista.busqueda.trim().to_lowercase();
    if !q.is_empty() {
        let mut texto = format!("{} {}", c.archivo.nombre, c.archivo.detalle);
        if let Some((_, n)) = &c.de_mod {
            texto.push(' ');
            texto.push_str(n);
        }
        if !texto.to_lowercase().contains(&q) {
            return false;
        }
    }
    match m.vista.filtro {
        FiltroPublicacion::Todo => true,
        FiltroPublicacion::SePublica => c.marcado(),
        FiltroPublicacion::NoSePublica => !c.marcado(),
        FiltroPublicacion::ConProblemas => c.problema.is_some(),
    }
}

fn buscando(m: &RevisionPublicacion) -> bool {
    !m.vista.busqueda.trim().is_empty() || m.vista.filtro != FiltroPublicacion::Todo
}

// --- Piezas visuales -------------------------------------------------------

fn t<'a>(s: impl Into<String>, f: Font, tam: f32, alto: f32, color: Color) -> Element<'a, Accion> {
    text(s.into())
        .font(f)
        .size(tam)
        .line_height(iced::Pixels(alto))
        .color(color)
        .into()
}
fn body<'a>(p: Paleta, s: impl Into<String>) -> Element<'a, Accion> {
    t(s, fuentes::CUERPO, 14., 21., p.text)
}
fn body_sm<'a>(s: impl Into<String>, color: Color) -> Element<'a, Accion> {
    t(s, fuentes::CUERPO, 13., 19., color)
}
fn mono_sm<'a>(s: impl Into<String>, color: Color) -> Element<'a, Accion> {
    t(s, fuentes::MONO, 11., 16., color)
}
fn etiqueta<'a>(p: Paleta, s: &str) -> Element<'a, Accion> {
    t(s.to_uppercase(), fuentes::ETIQUETA, 11., 14., p.text_muted)
}
fn titulo_de<'a>(p: Paleta, s: impl Into<String>, tam: f32) -> Element<'a, Accion> {
    t(s, fuentes::CUERPO_NEGRITA, tam, tam + 6., p.text)
}
/// Texto con un tramo en negrita al principio.
fn con_negrita<'a>(p: Paleta, fuerte: String, resto: String, tam: f32) -> Element<'a, Accion> {
    rich_text![
        span(fuerte).font(fuentes::CUERPO_NEGRITA).color(p.text),
        span(resto).font(fuentes::CUERPO).color(p.text),
    ]
    .size(tam)
    .line_height(iced::Pixels(tam + 6.))
    .into()
}
fn tenue(c: Color) -> Color {
    Color { a: c.a * 0.55, ..c }
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
fn bloque<'a>(
    color: Color,
    ancho: impl Into<Length>,
    alto: impl Into<Length>,
) -> Element<'a, Accion> {
    container(Space::new(ancho, alto))
        .style(estilo::bloque(color))
        .into()
}

/// Insignia del diseño: relleno suave y regla del estado, sin glifo.
fn insignia<'a>(p: Paleta, e: Estado, palabra: &str) -> Element<'a, Accion> {
    let (fondo, tinta) = match e {
        Estado::Exito => (p.success_soft, p.success),
        Estado::Aviso => (p.warning_soft, p.warning),
        Estado::Error => (p.error_soft, p.error),
        Estado::Info => (p.info_soft, p.info),
    };
    container(
        text(palabra.to_uppercase())
            .font(fuentes::ETIQUETA)
            .size(10)
            .line_height(iced::widget::text::LineHeight::Relative(1.))
            .wrapping(iced::widget::text::Wrapping::None),
    )
    .height(20)
    .center_y(20)
    .padding([0., 6.])
    .style(estilo::insignia(fondo, tinta))
    .into()
}

/// Marca `ic-diff` de la fila; atenuada cuando no se publica.
fn marca_fila<'a>(p: Paleta, m: Marca, apagada: bool) -> Element<'a, Accion> {
    let (fondo, tinta, borde) = match m {
        Marca::Alta => (p.success_soft, p.success, p.success),
        Marca::Actualizacion => (p.info_soft, p.info, p.info),
        Marca::Retirada => (p.error_soft, p.error, p.error),
        Marca::Conflicto => (p.warning_soft, p.warning, p.warning),
        Marca::Conservado => (Color::TRANSPARENT, p.text_muted, p.border_strong),
    };
    let f = |c: Color| if apagada { tenue(c) } else { c };
    let (fondo, tinta, borde) = (f(fondo), f(tinta), f(borde));
    container(
        text(m.texto())
            .font(fuentes::MONO_MEDIO)
            .size(14)
            .line_height(iced::widget::text::LineHeight::Relative(1.)),
    )
    .width(20)
    .height(20)
    .center(20)
    .style(move |_| container::Style {
        background: Some(fondo.into()),
        text_color: Some(tinta),
        border: Border {
            color: borde,
            width: 1.,
            radius: 0.0.into(),
        },
        ..Default::default()
    })
    .into()
}

fn interior_casilla<'a>(p: Paleta, e: Tri) -> Element<'a, Accion> {
    match e {
        Tri::No => Space::new(0, 0).into(),
        Tri::Todos => bloque(p.on_accent, 10, 10),
        Tri::Algunos => bloque(p.on_accent, 10, 2),
    }
}
/// Casilla `ic-casilla` dibujada: pozo apagado, bloque de acento; indeterminada, raya.
fn casilla<'a>(p: Paleta, e: Tri) -> Element<'a, Accion> {
    let llena = e != Tri::No;
    container(interior_casilla(p, e))
        .width(20)
        .height(20)
        .center(20)
        .style(caja(
            Some(if llena { p.accent } else { p.surface_sunken }),
            if llena { p.accent } else { p.border_strong },
            2.,
        ))
        .into()
}
/// Casilla con su propia pulsación (tarjetas y cabeceras de sección).
fn casilla_boton<'a>(p: Paleta, e: Tri, accion: Accion) -> Element<'a, Accion> {
    let llena = e != Tri::No;
    button(container(interior_casilla(p, e)).center(Length::Fill))
        .width(20)
        .height(20)
        .padding(0)
        .on_press(accion)
        .style(move |_, s| {
            let sobre = matches!(s, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: Some(Background::Color(if llena {
                    p.accent
                } else {
                    p.surface_sunken
                })),
                border: Border {
                    color: if llena {
                        p.accent
                    } else if sobre {
                        p.text
                    } else {
                        p.border_strong
                    },
                    width: 2.,
                    radius: 0.0.into(),
                },
                ..Default::default()
            }
        })
        .into()
}

/// Superficie pulsable sin marco que se ilumina al pasar el puntero.
fn superficie(
    p: Paleta,
    fondo: Option<Color>,
) -> impl Fn(&iced::Theme, button::Status) -> button::Style {
    move |_, s| button::Style {
        background: if matches!(s, button::Status::Hovered | button::Status::Pressed) {
            Some(Background::Color(p.surface_hover))
        } else {
            fondo.map(Background::Color)
        },
        text_color: p.text,
        ..Default::default()
    }
}

/// `ic-btn` de 36 px; el icono invade 4 px el relleno de su lado.
fn btn<'a>(
    p: Paleta,
    izquierda: Option<Icono>,
    rotulo: &str,
    derecha: Option<Icono>,
    v: Variante,
    accion: Option<Accion>,
) -> Element<'a, Accion> {
    let mut contenido = row![].spacing(4).align_y(Alignment::Center);
    if let Some(i) = izquierda {
        contenido = contenido.push(botones::icono_heredado(i, Tam::Base));
    }
    contenido = contenido.push(
        text(rotulo.to_uppercase())
            .font(fuentes::ETIQUETA)
            .size(11)
            .wrapping(iced::widget::text::Wrapping::None),
    );
    if let Some(i) = derecha {
        contenido = contenido.push(botones::icono_heredado(i, Tam::Base));
    }
    let b = button(container(contenido).center_y(Length::Fill))
        .height(36)
        .padding(Padding {
            top: 0.,
            right: if derecha.is_some() { 12. } else { 16. },
            bottom: 0.,
            left: if izquierda.is_some() { 12. } else { 16. },
        })
        .on_press_maybe(accion);
    match v {
        Variante::Primario => b.style(estilo::boton_primario(p)),
        Variante::Secundario => b.style(estilo::boton_secundario(p)),
        Variante::Fantasma => b.style(estilo::boton_fantasma(p)),
        Variante::Peligro => b.style(estilo::boton_peligro(p)),
    }
    .into()
}

/// Aviso `ic-aviso` con su icono.
fn aviso<'a>(
    p: Paleta,
    e: Estado,
    i: Icono,
    contenido: Element<'a, Accion>,
) -> Element<'a, Accion> {
    let (fondo, tinta) = match e {
        Estado::Error => (p.error_soft, p.error),
        Estado::Info => (p.info_soft, p.info),
        Estado::Exito => (p.success_soft, p.success),
        Estado::Aviso => (p.warning_soft, p.warning),
    };
    container(
        row![
            icono(i, Tam::Base, tinta),
            container(contenido).width(Length::Fill)
        ]
        .spacing(16)
        .align_y(Alignment::Start),
    )
    .padding([16., 20.])
    .width(Length::Fill)
    .style(caja(Some(fondo), tinta, 1.))
    .into()
}

/// Línea con regla de acento a la izquierda (`box-shadow: inset 3px 0 0`).
fn con_regla<'a>(
    regla: Color,
    fondo: Color,
    contenido: Element<'a, Accion>,
    relleno: [f32; 2],
) -> Element<'a, Accion> {
    // La primera capa fija el alto; la regla lo ocupa entero sin cambiarlo.
    stack![
        container(contenido)
            .padding(Padding {
                top: relleno[0],
                right: relleno[1],
                bottom: relleno[0],
                left: relleno[1],
            })
            .width(Length::Fill)
            .style(caja(Some(fondo), Color::TRANSPARENT, 0.)),
        bloque(regla, 3, Length::Fill),
    ]
    .width(Length::Fill)
    .into()
}

// --- Bloques de la revisión --------------------------------------------------

fn cabecera_rp<'a>(p: Paleta, m: &RevisionPublicacion) -> Element<'a, Accion> {
    let revs = container(
        row![
            mono_sm(format!("Publicada: revisión {}", m.base), p.text_muted),
            icono(Icono::Adelante, Tam::Sm, p.text),
            mono_sm(format!("Nueva: revisión {}", m.nueva), p.text),
        ]
        .spacing(10)
        .align_y(Alignment::Center),
    )
    .padding([6., 12.])
    .style(caja(None, p.border_strong, 1.));
    let instancia = row![
        crate::servidores::avatar_servidor_de(p, &m.instancia.nombre, 34.),
        column![
            titulo_de(p, &m.instancia.nombre, 15.),
            mono_sm(
                format!("{} · {}", m.instancia.minecraft, m.instancia.loader),
                p.text_muted
            )
        ]
        .spacing(2),
        revs
    ]
    .spacing(14)
    .align_y(Alignment::Center)
    .wrap();
    column![
        container(
            row![
                column![titulo(p, "Revisar publicación"), instancia]
                    .spacing(12)
                    .width(Length::Fill),
                botones::boton_icono_estilo(
                    p,
                    Icono::Cerrar,
                    "Cerrar",
                    estilo::boton_fantasma(p),
                    Accion::Cerrar
                )
            ]
            .spacing(24)
            .align_y(Alignment::Start),
        )
        .padding(Padding {
            top: 28.,
            right: 32.,
            bottom: 20.,
            left: 32.,
        }),
        regla(p, false)
    ]
    .into()
}

/// Alto común de los atajos: reserva dos líneas de título y tres de detalle.
const ALTO_ATAJO: f32 = 140.;
/// Alto común de las tarjetas de tipo: icono, título, cifra y estado.
const ALTO_TIPO: f32 = 148.;

fn atajos<'a>(p: Paleta, m: &RevisionPublicacion) -> Element<'a, Accion> {
    let atajo = |a: AtajoPublicacion, i: Icono, nombre: &str, detalle: &str, porcion: u16| {
        let recomendado = a == AtajoPublicacion::Necesario;
        let pulsado = m.vista.atajo == Some(a);
        let mut cabeza = row![
            icono(i, Tam::Base, p.text),
            container(titulo_de(p, nombre, 15.)).width(Length::Fill)
        ]
        .spacing(10)
        .align_y(Alignment::Center);
        if recomendado {
            cabeza = cabeza.push(insignia(p, Estado::Exito, "Recomendado"));
        }
        let interior = container(
            column![cabeza, body_sm(detalle, p.text_muted)]
                .spacing(8)
                .width(Length::Fill),
        )
        .padding(Padding {
            top: 17.,
            right: 19.,
            bottom: 17.,
            left: 19.,
        })
        .width(Length::Fill)
        .height(Length::Fill);
        let contenido: Element<'a, Accion> = if pulsado {
            stack![interior, bloque(p.text, 3, Length::Fill)].into()
        } else {
            interior.into()
        };
        button(contenido)
            .padding(0)
            .width(Length::FillPortion(porcion))
            .height(ALTO_ATAJO)
            .on_press_maybe((!m.guardando).then_some(Accion::AtajoPublicacion(a)))
            .style(move |_, s| {
                let sobre = matches!(s, button::Status::Hovered | button::Status::Pressed);
                button::Style {
                    background: Some(Background::Color(if pulsado {
                        p.accent_soft
                    } else if sobre {
                        p.surface_hover
                    } else {
                        p.surface
                    })),
                    text_color: p.text,
                    border: Border {
                        color: if recomendado || sobre {
                            p.text
                        } else {
                            p.border_strong
                        },
                        width: if recomendado { 2. } else { 1. },
                        radius: 0.0.into(),
                    },
                    ..Default::default()
                }
            })
    };
    let mut c = column![
        etiqueta(p, "Empieza por aquí"),
        row![
            atajo(
                AtajoPublicacion::Necesario,
                Icono::Mando,
                "Lo necesario para jugar juntos",
                "Mods del servidor, sus configuraciones y los scripts. Deja fuera lo que solo cambia cómo se ve el juego en tu equipo.",
                14
            ),
            atajo(
                AtajoPublicacion::Todo,
                Icono::Check,
                "Todo",
                "Tal como está tu instancia, incluidos shaders y mods visuales.",
                10
            ),
            atajo(
                AtajoPublicacion::NadaNuevo,
                Icono::Candado,
                "Nada nuevo",
                "Solo actualizaciones y retiradas de lo que ya compartías.",
                10
            ),
        ]
        .spacing(12)
    ]
    .spacing(12);
    if !m.vista.aplicado.is_empty() {
        c = c.push(con_regla(
            p.success,
            p.success_soft,
            body_sm(m.vista.aplicado.clone(), p.text),
            [12., 16.],
        ));
    }
    c.into()
}

fn problemas<'a>(p: Paleta, r: &Resumen<'_>) -> Option<Element<'a, Accion>> {
    if r.bloqueantes.is_empty() {
        return None;
    }
    let n = r.bloqueantes.len();
    let mut lista = column![con_negrita(
        p,
        if n == 1 {
            "Hay que resolver 1 cosa antes de publicar.".into()
        } else {
            format!("Hay que resolver {n} cosas antes de publicar.")
        },
        String::new(),
        13.
    )]
    .spacing(4);
    for (i, c) in r.bloqueantes.iter().enumerate() {
        let Some(problema) = &c.problema else {
            continue;
        };
        let fila = row![
            marca_fila(p, Marca::Conflicto, false),
            column![
                con_negrita(
                    p,
                    format!("{}:", c.archivo.nombre),
                    format!(" {}", problema.titulo.to_lowercase()),
                    13.
                ),
                body_sm(problema.detalle.clone(), p.text_muted)
            ]
            .spacing(2)
            .width(Length::Fill),
            // `ic-btn--sm`: 28 px.
            pequeno(
                p,
                &problema.rotulo,
                Variante::Secundario,
                Some(problema.accion.clone())
            )
        ]
        .spacing(16)
        .align_y(Alignment::Center);
        lista = lista.push(container(fila).padding([12., 0.]));
        if i + 1 < n {
            let linea = Color { a: 0.3, ..p.error };
            lista = lista.push(bloque(linea, Length::Fill, 1));
        }
    }
    Some(aviso(p, Estado::Error, Icono::Alerta, lista.into()))
}

fn tipos<'a>(p: Paleta, r: &Resumen<'_>) -> Element<'a, Accion> {
    let mut fila = row![];
    let mut primera = true;
    for tipo in TipoContenido::TODOS {
        let x = r.cuenta(tipo);
        if x.total == 0 {
            continue;
        }
        if !primera {
            fila = fila.push(bloque(p.border, 1, Length::Fill));
        }
        primera = false;
        let estado = tri(x);
        let resumen = if x.publican == x.total {
            "se publican todos"
        } else if x.publican > 0 {
            "se publican"
        } else {
            "nada se publica"
        };
        let cuenta = rich_text![
            span(x.publican.to_string())
                .font(fuentes::MONO_MEDIO)
                .size(26)
                .color(p.text),
            span(format!(" de {}", x.total))
                .font(fuentes::MONO)
                .size(13)
                .color(p.text_muted),
        ]
        .line_height(iced::Pixels(28.));
        // El título va bajo el icono, a todo el ancho: la casilla solo comparte la línea del icono.
        let interior = column![
            icono(tipo.icono(), Tam::Base, p.text),
            titulo_de(p, tipo.titulo(), 14.),
            cuenta,
            body_sm(resumen, p.text_muted),
        ]
        .spacing(8)
        .width(Length::Fill);
        let algo = x.publican > 0;
        let interior: Element<'a, Accion> = container(interior)
            .padding(Padding {
                top: if algo { 13. } else { 16. },
                right: 16.,
                bottom: 16.,
                left: 16.,
            })
            .width(Length::Fill)
            .into();
        let interior: Element<'a, Accion> = if algo {
            column![bloque(p.text, Length::Fill, 3), interior].into()
        } else {
            interior
        };
        let ir = button(interior)
            .padding(0)
            .width(Length::Fill)
            .height(Length::Fill)
            .on_press(Accion::IrATipo(tipo))
            .style(superficie(p, None));
        let marca = container(casilla_boton(
            p,
            estado,
            Accion::MarcarTipo(tipo, estado != Tri::Todos),
        ))
        .width(Length::Fill)
        .align_x(Alignment::End)
        .padding(14);
        fila = fila.push(
            container(stack![ir, marca])
                .width(Length::FillPortion(1))
                .height(Length::Fill),
        );
    }
    container(fila.height(ALTO_TIPO))
        .width(Length::Fill)
        .style(caja(None, p.border, 1.))
        .into()
}

fn chip_filtro<'a>(
    p: Paleta,
    nombre: &str,
    n: usize,
    activo: bool,
    a: Accion,
) -> Element<'a, Accion> {
    button(
        container(
            row![
                text(nombre.to_uppercase())
                    .font(fuentes::ETIQUETA)
                    .size(11)
                    .wrapping(iced::widget::text::Wrapping::None),
                text(n.to_string()).font(fuentes::MONO).size(11),
            ]
            .spacing(6)
            .align_y(Alignment::Center),
        )
        .center_y(Length::Fill),
    )
    .height(28)
    .padding([0., 12.])
    .on_press(a)
    .style(move |_, s| {
        let sobre = matches!(s, button::Status::Hovered | button::Status::Pressed);
        button::Style {
            background: activo.then_some(Background::Color(p.text)),
            text_color: if activo { p.bg } else { p.text },
            border: Border {
                color: if activo || sobre {
                    p.text
                } else {
                    p.border_strong
                },
                width: 1.,
                radius: 0.0.into(),
            },
            ..Default::default()
        }
    })
    .into()
}

fn filtros<'a>(p: Paleta, m: &RevisionPublicacion, r: &Resumen<'_>) -> Element<'a, Accion> {
    let f = m.vista.filtro;
    let chips = row![
        chip_filtro(
            p,
            "Todo",
            r.total,
            f == FiltroPublicacion::Todo,
            Accion::FiltroPublicacion(FiltroPublicacion::Todo)
        ),
        chip_filtro(
            p,
            "Se publica",
            r.efectivos,
            f == FiltroPublicacion::SePublica,
            Accion::FiltroPublicacion(FiltroPublicacion::SePublica)
        ),
        chip_filtro(
            p,
            "No se publica",
            r.total - r.efectivos,
            f == FiltroPublicacion::NoSePublica,
            Accion::FiltroPublicacion(FiltroPublicacion::NoSePublica)
        ),
        chip_filtro(
            p,
            "Con problemas",
            r.con_problemas,
            f == FiltroPublicacion::ConProblemas,
            Accion::FiltroPublicacion(FiltroPublicacion::ConProblemas)
        ),
    ]
    .spacing(6);
    row![
        container(componentes::campo_con_icono(
            p,
            Icono::Buscar,
            "Buscar por nombre o archivo",
            &m.vista.busqueda,
            Accion::BuscarPublicacion
        ))
        .width(Length::Fill),
        chips
    ]
    .spacing(12)
    .align_y(Alignment::Center)
    .into()
}

fn fila<'a>(p: Paleta, m: &RevisionPublicacion, c: &CambioPublicacion) -> Element<'a, Accion> {
    let marcado = c.marcado();
    let mut titulo = row![t(
        c.archivo.nombre.clone(),
        fuentes::CUERPO_NEGRITA,
        14.,
        20.,
        if marcado { p.text } else { tenue(p.text) }
    )]
    .spacing(8)
    .align_y(Alignment::Center);
    if c.propio && c.tipo == TipoContenido::Mod {
        titulo = titulo.push(insignia(p, Estado::Aviso, "Archivo propio"));
    }
    if c.solo_cliente {
        titulo = titulo.push(insignia(p, Estado::Info, "Solo cliente"));
    }
    if let Some((id, _)) = &c.de_mod {
        let sigue = m.vista.siguen_mod
            && m.cambios
                .iter()
                .find(|x| &x.archivo.id == id)
                .is_some_and(|x| x.marcado() == marcado);
        if sigue {
            titulo = titulo.push(componentes::insignia_contorno(p, "Sigue a su mod"));
        }
    }
    let archivo = match &c.de_mod {
        Some((_, nombre)) => format!("de {nombre} · {}", c.archivo.detalle),
        None => c.archivo.detalle.clone(),
    };
    let mut info = column![titulo.wrap(), mono_sm(archivo, p.text_muted)].spacing(4);
    let detalle = if c.grupo == GrupoPublicacion::Retiradas {
        if marcado {
            c.estado.clone()
        } else {
            estado_no(c, m.base)
        }
    } else if !marcado {
        estado_no(c, m.base)
    } else {
        c.estado.clone()
    };
    if !detalle.is_empty() {
        info = info.push(body_sm(detalle, p.text_muted));
    }
    if let Some(problema) = &c.problema {
        info = info.push(if marcado {
            Element::from(
                row![
                    icono(Icono::Aviso, Tam::Sm, p.error),
                    t(
                        problema.titulo.clone(),
                        fuentes::CUERPO_NEGRITA,
                        13.,
                        18.,
                        p.error
                    )
                ]
                .spacing(6)
                .align_y(Alignment::Center),
            )
        } else {
            t(
                format!("{} · no bloquea mientras sea privado", problema.titulo),
                fuentes::CUERPO,
                13.,
                18.,
                p.text_muted,
            )
        });
    }
    let version = container(
        text(c.version.clone())
            .font(fuentes::MONO)
            .size(11)
            .line_height(iced::Pixels(16.))
            .color(if marcado { p.text } else { tenue(p.text) })
            .align_x(Alignment::End),
    )
    .max_width(220)
    .padding(Padding {
        top: 2.,
        ..Padding::ZERO
    });
    let tamano = container(
        text(c.archivo.tamano.clone())
            .font(fuentes::MONO)
            .size(11)
            .line_height(iced::Pixels(16.))
            .color(p.text_muted)
            .wrapping(iced::widget::text::Wrapping::None),
    )
    .width(72)
    .align_x(Alignment::End)
    .padding(Padding {
        top: 2.,
        ..Padding::ZERO
    });
    let contenido = row![
        container(casilla(p, if marcado { Tri::Todos } else { Tri::No })).padding(Padding {
            top: 1.,
            ..Padding::ZERO
        }),
        marca_fila(p, c.marca, !marcado),
        info.width(Length::Fill),
        version,
        tamano,
    ]
    .spacing(14)
    .align_y(Alignment::Start);
    column![
        button(contenido)
            .padding([14., 4.])
            .width(Length::Fill)
            .on_press_maybe(
                (!m.guardando).then(|| Accion::MarcarPublicacion(c.archivo.id.clone(), !marcado))
            )
            .style(superficie(p, None)),
        regla(p, false)
    ]
    .into()
}

fn seccion<'a>(
    p: Paleta,
    m: &RevisionPublicacion,
    r: &Resumen<'_>,
    tipo: TipoContenido,
) -> Option<Element<'a, Accion>> {
    let todos: Vec<_> = m
        .cambios
        .iter()
        .filter(|c| c.tipo == tipo && en_vista(m, c))
        .collect();
    if todos.is_empty() {
        return None;
    }
    let busca = buscando(m);
    let visibles: Vec<_> = todos.into_iter().filter(|c| coincide(m, c)).collect();
    if busca && visibles.is_empty() {
        return None;
    }
    let x = r.cuenta(tipo);
    let abierto = m.vista.abiertos.contains(&tipo) || busca;
    let estado = tri(x);
    let cab = row![
        container(casilla_boton(
            p,
            estado,
            Accion::MarcarTipo(tipo, estado != Tri::Todos)
        ))
        .padding(Padding {
            top: 0.,
            right: 6.,
            bottom: 0.,
            left: 18.
        }),
        button(
            row![
                row![
                    icono(tipo.icono(), Tam::Base, p.text),
                    column![
                        t(tipo.titulo(), fuentes::CUERPO_NEGRITA, 16., 22., p.text),
                        body_sm(tipo.descripcion(), p.text_muted)
                    ]
                    .spacing(2)
                ]
                .spacing(12)
                .align_y(Alignment::Center)
                .width(Length::Fill),
                mono_sm(
                    format!("{} de {} se publican", x.publican, x.total),
                    p.text_muted
                ),
                container(icono(
                    if abierto {
                        Icono::Flecha
                    } else {
                        Icono::ChevronAbajo
                    },
                    Tam::Base,
                    p.text
                ))
                .width(24)
            ]
            .spacing(16)
            .align_y(Alignment::Center)
        )
        .padding(Padding {
            top: 16.,
            right: 18.,
            bottom: 16.,
            left: 10.,
        })
        .width(Length::Fill)
        .on_press(Accion::AlternarTipo(tipo))
        .style(superficie(p, None))
    ]
    .align_y(Alignment::Center);
    let mut c = column![cab];
    if abierto {
        let mut cuerpo = column![].spacing(20).padding(Padding {
            top: 18.,
            right: 18.,
            bottom: 20.,
            left: 18.,
        });
        if tipo == TipoContenido::Config {
            let siguen = m.vista.siguen_mod;
            cuerpo = cuerpo.push(
                button(
                    row![
                        casilla(p, if siguen { Tri::Todos } else { Tri::No }),
                        body_sm(
                            "Las configuraciones siguen a su mod: si publicas un mod, se publican sus ajustes",
                            p.text
                        )
                    ]
                    .spacing(12)
                    .align_y(Alignment::Center),
                )
                .padding([14., 16.])
                .width(Length::Fill)
                .on_press(Accion::SiguenSuMod(!siguen))
                .style(superficie(p, Some(p.surface_sunken))),
            );
        }
        if tipo == TipoContenido::Resourcepack {
            cuerpo = cuerpo.push(body_sm(
                "Los packs que vienen dentro de un mod no aparecen aquí: viajan con el mod.",
                p.text_muted,
            ));
        }
        for g in GrupoPublicacion::TODOS {
            let lista: Vec<_> = visibles.iter().filter(|c| c.grupo == g).collect();
            if lista.is_empty() {
                continue;
            }
            let todos = busca || m.vista.ver_todos.contains(&(tipo, g));
            let limite = if todos { lista.len() } else { 6 };
            let mut sub = column![
                container(etiqueta(p, &format!("{} · {}", g.titulo(), lista.len()))).padding(
                    Padding {
                        bottom: 8.,
                        ..Padding::ZERO
                    }
                ),
                regla(p, true)
            ];
            for c in lista.iter().take(limite) {
                sub = sub.push(fila(p, m, c));
            }
            if lista.len() > limite {
                sub = sub.push(
                    container(
                        button(
                            rich_text![span(format!("Ver los {}", lista.len()))
                                .font(fuentes::CUERPO_NEGRITA)
                                .underline(true)
                                .color(p.text)]
                            .size(13),
                        )
                        .padding([6., 0.])
                        .on_press(Accion::VerTodosPublicacion(tipo, g))
                        .style(estilo::sin_estilo(p.text)),
                    )
                    .padding(Padding {
                        top: 10.,
                        ..Padding::ZERO
                    }),
                );
            }
            cuerpo = cuerpo.push(sub);
        }
        c = c.push(regla(p, false)).push(cuerpo);
    }
    Some(
        container(c)
            .id(container::Id::new(id_seccion(tipo)))
            .width(Length::Fill)
            .style(caja(
                None,
                if abierto { p.border_strong } else { p.border },
                1.,
            ))
            .into(),
    )
}

fn secciones<'a>(p: Paleta, m: &RevisionPublicacion, r: &Resumen<'_>) -> Element<'a, Accion> {
    let lista: Vec<_> = TipoContenido::TODOS
        .into_iter()
        .filter_map(|t| seccion(p, m, r, t))
        .collect();
    if lista.is_empty() {
        return container(body(p, "Nada coincide con la búsqueda."))
            .padding(24)
            .width(Length::Fill)
            .style(caja(None, p.border, 1.))
            .into();
    }
    Column::with_children(lista).spacing(16).into()
}

fn avance_rp<'a>(p: Paleta, mensaje: String, dato: String, avance: f32) -> Column<'a, Accion> {
    column![
        row![
            body(p, mensaje),
            Space::new(Length::Fill, 0),
            mono_sm(dato, p.text_muted)
        ]
        .spacing(16),
        crate::progreso::continuo(p, avance, false)
    ]
}

fn cuerpo_rp<'a>(p: Paleta, m: &RevisionPublicacion, r: &Resumen<'_>) -> Column<'a, Accion> {
    let mut c = column![].spacing(28);
    match &m.estado {
        EstadoRevisionPublicacion::Analizando { hechos, total } => {
            c = c.push(
                container(
                    avance_rp(
                        p,
                        format!("Buscando qué cambió desde la revisión {}", m.base),
                        format!("{} de {} archivos", miles(*hechos), miles(*total)),
                        if *total == 0 {
                            0.
                        } else {
                            *hechos as f32 / *total as f32
                        },
                    )
                    .push(body_sm(
                        format!("Tus amigos siguen recibiendo la revisión {}.", m.base),
                        p.text_muted,
                    ))
                    .spacing(10),
                )
                .padding(Padding {
                    top: 8.,
                    bottom: 16.,
                    ..Padding::ZERO
                }),
            );
        }
        EstadoRevisionPublicacion::SinCambios | EstadoRevisionPublicacion::Disponible => {
            if sin_cambios(m) {
                c = c.push(
                    container(
                        row![
                            marca_fila(p, Marca::Conservado, false),
                            column![
                                titulo_de(p, format!("Tu instancia está igual que la revisión {}", m.base), 16.),
                                body_sm(
                                    "No hay nada nuevo para publicar. Lo que mantienes privado sigue abajo por si quieres incluirlo.",
                                    p.text_muted
                                )
                            ]
                            .spacing(4)
                        ]
                        .spacing(16)
                        .align_y(Alignment::Center),
                    )
                    .padding(20)
                    .width(Length::Fill)
                    .style(caja(None, p.border, 1.)),
                );
            } else {
                c = c.push(
                    container(
                        rich_text![
                            span("Nada de lo que instales se comparte solo. Marca lo que quieres que tus amigos reciban en la "),
                            span(format!("revisión {}", m.nueva)).font(fuentes::CUERPO_NEGRITA),
                            span("; lo que no marques se queda en tu equipo."),
                        ]
                        .font(fuentes::CUERPO)
                        .size(14)
                        .line_height(iced::Pixels(21.))
                        .color(p.text),
                    )
                    .max_width(64. * 7.5),
                );
                c = c.push(atajos(p, m));
                if let Some(aviso) = problemas(p, r) {
                    c = c.push(aviso);
                }
            }
            if r.total > 0 {
                c = c
                    .push(tipos(p, r))
                    .push(filtros(p, m, r))
                    .push(secciones(p, m, r));
            }
        }
        EstadoRevisionPublicacion::Preparando { porcentaje } => {
            c = c.push(
                container(
                    avance_rp(
                        p,
                        format!("Preparando la revisión {}", m.nueva),
                        format!("{porcentaje:.0} %"),
                        porcentaje / 100.,
                    )
                    .push(aviso(
                        p,
                        Estado::Info,
                        Icono::InfoCaja,
                        con_negrita(
                            p,
                            format!("Tus amigos siguen recibiendo la revisión {}.", m.base),
                            format!(
                                " La {} aparece para ellos solo cuando termine de prepararse. Si cancelas, no se publica nada.",
                                m.nueva
                            ),
                            13.,
                        ),
                    ))
                    .spacing(14),
                )
                .padding(Padding {
                    top: 8.,
                    bottom: 16.,
                    ..Padding::ZERO
                }),
            );
        }
        EstadoRevisionPublicacion::Publicada { resumen } => {
            c = c.push(
                container(
                    row![
                        container(icono(Icono::Check, Tam::Lg, p.bg))
                            .width(56)
                            .height(56)
                            .center(56)
                            .style(estilo::bloque(p.text)),
                        column![
                            t(format!("Publicaste la revisión {}", m.nueva), fuentes::TITULO, 22., 26., p.text),
                            body_sm(resumen.clone(), p.text_muted),
                            body_sm(
                                "Tus amigos la verán al pulsar Jugar o Sincronizar, y revisarán los cambios antes de aplicarlos.",
                                p.text
                            ),
                        ]
                        .spacing(6)
                    ]
                    .spacing(20)
                    .align_y(Alignment::Center),
                )
                .padding(24)
                .width(Length::Fill)
                .style(caja(None, p.text, 2.)),
            );
        }
        EstadoRevisionPublicacion::Error(motivo) => {
            c = c.push(aviso(
                p,
                Estado::Error,
                Icono::Alerta,
                column![
                    con_negrita(
                        p,
                        format!("No se pudo preparar la revisión {}.", m.nueva),
                        format!(" {motivo}"),
                        13.
                    ),
                    body_sm(
                        format!(
                            "No se publicó nada: tus amigos siguen recibiendo la revisión {}. Tu selección se conserva.",
                            m.base
                        ),
                        p.text
                    )
                ]
                .spacing(8)
                .into(),
            ));
        }
    }
    c
}

/// Cifra con separador de miles: 1.204.
fn miles(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push('.');
        }
        out.push(ch);
    }
    out
}

fn pie_rp<'a>(p: Paleta, m: &RevisionPublicacion, r: &Resumen<'_>) -> Element<'a, Accion> {
    let contenido: Element<'a, Accion> = if m.confirmar_cancelacion {
        con_regla(
            p.error,
            p.error_soft,
            row![
                container(con_negrita(
                    p,
                    "¿Cerrar sin publicar?".into(),
                    format!(
                        " Tu selección se pierde. Tus amigos siguen con la revisión {}.",
                        m.base
                    ),
                    13.
                ))
                .width(Length::Fill),
                btn(
                    p,
                    None,
                    "Seguir revisando",
                    None,
                    Variante::Secundario,
                    Some(Accion::SeguirRevisando)
                ),
                btn(
                    p,
                    None,
                    "Cerrar sin publicar",
                    None,
                    Variante::Peligro,
                    Some(Accion::CerrarSinPublicar)
                ),
            ]
            .spacing(12)
            .align_y(Alignment::Center)
            .into(),
            [14., 16.],
        )
    } else {
        let publicar = format!("Publicar revisión {}", m.nueva);
        let siguen = format!("Tus amigos siguen recibiendo la revisión {}.", m.base);
        let (frase, error, cancelar, primario): (String, bool, Option<&str>, Element<'a, Accion>) =
            match &m.estado {
                EstadoRevisionPublicacion::Analizando { .. } => (
                    "Buscando cambios…".into(),
                    false,
                    Some("Cancelar"),
                    btn(p, None, &publicar, None, Variante::Primario, None),
                ),
                EstadoRevisionPublicacion::SinCambios => (
                    format!(
                        "Nada que publicar: tu instancia está igual que la revisión {}.",
                        m.base
                    ),
                    false,
                    Some("Cerrar"),
                    btn(p, None, &publicar, None, Variante::Primario, None),
                ),
                EstadoRevisionPublicacion::Disponible => {
                    let frase = r.frase();
                    if r.efectivos == 0 {
                        (
                            "Todavía no marcaste nada para publicar.".into(),
                            false,
                            Some("Cancelar"),
                            btn(p, None, &publicar, None, Variante::Primario, None),
                        )
                    } else if !r.bloqueantes.is_empty() {
                        let n = r.bloqueantes.len();
                        (
                            if n == 1 {
                                "Resuelve 1 problema arriba para publicar.".into()
                            } else {
                                format!("Resuelve {n} problemas arriba para publicar.")
                            },
                            true,
                            Some("Cancelar"),
                            btn(p, None, &publicar, None, Variante::Primario, None),
                        )
                    } else if r.autorizaciones > 0 {
                        let n = r.autorizaciones;
                        (
                            format!(
                                "{frase} Falta tu permiso para {}",
                                if n == 1 {
                                    "1 archivo propio.".into()
                                } else {
                                    format!("{n} archivos propios.")
                                }
                            ),
                            false,
                            Some("Cancelar"),
                            btn(
                                p,
                                None,
                                "Continuar",
                                Some(Icono::Adelante),
                                Variante::Primario,
                                (!m.guardando).then_some(Accion::RevisarPropios),
                            ),
                        )
                    } else {
                        (
                            frase,
                            false,
                            Some("Cancelar"),
                            btn(
                                p,
                                Some(Icono::Subir),
                                &publicar,
                                None,
                                Variante::Primario,
                                (!m.guardando).then_some(Accion::PublicarRevision),
                            ),
                        )
                    }
                }
                EstadoRevisionPublicacion::Preparando { .. } => (
                    siguen,
                    false,
                    Some("Cancelar"),
                    btn(p, None, "Publicando", None, Variante::Primario, None),
                ),
                EstadoRevisionPublicacion::Publicada { .. } => (
                    String::new(),
                    false,
                    None,
                    btn(
                        p,
                        None,
                        "Listo",
                        None,
                        Variante::Primario,
                        Some(Accion::Cerrar),
                    ),
                ),
                EstadoRevisionPublicacion::Error(_) => (
                    siguen,
                    false,
                    Some("Cerrar"),
                    btn(
                        p,
                        Some(Icono::Refrescar),
                        "Reintentar",
                        None,
                        Variante::Primario,
                        Some(Accion::Reintentar),
                    ),
                ),
            };
        let mut acciones = row![].spacing(8).align_y(Alignment::Center);
        if let Some(rotulo) = cancelar {
            acciones = acciones.push(btn(
                p,
                None,
                rotulo,
                None,
                Variante::Secundario,
                Some(Accion::Cancelar),
            ));
        }
        acciones = acciones.push(primario);
        row![
            container(t(
                frase,
                fuentes::CUERPO,
                14.,
                21.,
                if error { p.error } else { p.text }
            ))
            .width(Length::Fill),
            acciones
        ]
        .spacing(20)
        .align_y(Alignment::Center)
        .into()
    };
    container(column![
        regla(p, true),
        container(contenido).padding([18., 32.]).width(Length::Fill)
    ])
    .width(Length::Fill)
    .style(estilo::bloque(p.surface_raised))
    .into()
}

pub fn revision_publicacion<'a>(p: Paleta, m: &RevisionPublicacion) -> Element<'a, Accion> {
    let r = Resumen::calcular(m);
    let cuerpo = cuerpo_rp(p, m, &r);
    let contenido = column![
        cabecera_rp(p, m),
        scrollable(container(cuerpo).padding(Padding {
            top: 28.,
            right: 32.,
            bottom: 32.,
            left: 32.,
        }))
        .id(scrollable::Id::new(ID_DESPLAZABLE))
        .height(Length::Fill),
        pie_rp(p, m, &r),
    ]
    .width(Length::Fill);
    container(contenido)
        .padding(BORDE_DIALOGO)
        .width(Length::Fill)
        .max_width(960)
        .max_height(800)
        .style(marco_dialogo(p))
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cambio(
        id: &str,
        tipo: TipoContenido,
        grupo: GrupoPublicacion,
        marcado: bool,
    ) -> CambioPublicacion {
        let (si, no) = match grupo {
            GrupoPublicacion::Nuevo | GrupoPublicacion::Privado => {
                (Eleccion::Compartir, Eleccion::Privado)
            }
            GrupoPublicacion::Retiradas => (Eleccion::Retirar, Eleccion::Posponer),
            _ => (Eleccion::Incluir, Eleccion::Posponer),
        };
        CambioPublicacion {
            archivo: ArchivoCompartido {
                id: id.into(),
                nombre: id.into(),
                ..Default::default()
            },
            tipo,
            grupo,
            marca: Marca::Alta,
            eleccion: if marcado { si } else { no },
            si,
            no,
            version: String::new(),
            version_publicada: String::new(),
            de_mod: None,
            propio: false,
            autorizado: true,
            solo_cliente: false,
            integrado: false,
            problema: None,
            estado: String::new(),
        }
    }
    fn modelo(cambios: Vec<CambioPublicacion>) -> RevisionPublicacion {
        RevisionPublicacion {
            instancia: InstanciaCompartida::default(),
            base: 7,
            nueva: 8,
            cambios,
            estado: EstadoRevisionPublicacion::Disponible,
            confirmar_cancelacion: false,
            guardando: false,
            vista: VistaRevision::default(),
        }
    }
    fn problema() -> Problema {
        Problema {
            archivo: String::new(),
            titulo: "Dependencia ausente".into(),
            detalle: String::new(),
            accion: Accion::Reintentar,
            rotulo: "Dejar privado".into(),
        }
    }

    #[test]
    fn la_revision_se_construye_en_todos_los_estados() {
        let mut m = modelo(vec![
            cambio(
                "mods/a.jar",
                TipoContenido::Mod,
                GrupoPublicacion::Nuevo,
                true,
            ),
            cambio(
                "config/a.json",
                TipoContenido::Config,
                GrupoPublicacion::Nuevo,
                false,
            ),
        ]);
        for e in [
            EstadoRevisionPublicacion::Analizando {
                hechos: 612,
                total: 1204,
            },
            EstadoRevisionPublicacion::SinCambios,
            EstadoRevisionPublicacion::Disponible,
            EstadoRevisionPublicacion::Preparando { porcentaje: 46. },
            EstadoRevisionPublicacion::Publicada {
                resumen: String::new(),
            },
            EstadoRevisionPublicacion::Error("x".into()),
        ] {
            m.estado = e;
            let _ = revision_publicacion(crate::tema::TINTA, &m);
            let _ = revision_publicacion(crate::tema::PIEDRA, &m);
        }
        m.confirmar_cancelacion = true;
        let _ = revision_publicacion(crate::tema::TINTA, &m);
    }

    #[test]
    fn el_problema_privado_no_bloquea_y_solo_lo_marcado_pide_permiso() {
        let mut privado = cambio(
            "mods/a.jar",
            TipoContenido::Mod,
            GrupoPublicacion::Nuevo,
            false,
        );
        privado.problema = Some(problema());
        privado.propio = true;
        privado.autorizado = false;
        let mut marcado = privado.clone();
        marcado.archivo.id = "mods/b.jar".into();
        marcado.eleccion = marcado.si;
        let m = modelo(vec![privado.clone()]);
        let r = Resumen::calcular(&m);
        assert_eq!(
            (
                r.bloqueantes.len(),
                r.autorizaciones,
                r.efectivos,
                r.privados
            ),
            (0, 0, 0, 1)
        );
        let m = modelo(vec![privado, marcado]);
        let r = Resumen::calcular(&m);
        assert_eq!(
            (
                r.bloqueantes.len(),
                r.autorizaciones,
                r.efectivos,
                r.con_problemas
            ),
            (1, 1, 1, 2)
        );
    }

    #[test]
    fn la_frase_cuenta_por_tipo_y_los_privados() {
        let m = modelo(vec![
            cambio(
                "mods/a.jar",
                TipoContenido::Mod,
                GrupoPublicacion::Nuevo,
                true,
            ),
            cambio(
                "mods/b.jar",
                TipoContenido::Mod,
                GrupoPublicacion::Nuevo,
                true,
            ),
            cambio(
                "config/a.json",
                TipoContenido::Config,
                GrupoPublicacion::Nuevo,
                true,
            ),
            cambio(
                "kubejs/a.js",
                TipoContenido::Script,
                GrupoPublicacion::Nuevo,
                true,
            ),
            cambio(
                "mods/c.jar",
                TipoContenido::Mod,
                GrupoPublicacion::Nuevo,
                false,
            ),
            cambio(
                "mods/d.jar",
                TipoContenido::Mod,
                GrupoPublicacion::Actualizaciones,
                false,
            ),
        ]);
        assert_eq!(
            Resumen::calcular(&m).frase(),
            "Se publican 2 mods, 1 configuración y 1 script. 1 queda privado."
        );
        assert_eq!(
            tri(Resumen::calcular(&m).cuenta(TipoContenido::Mod)),
            Tri::Algunos
        );
    }

    #[test]
    fn sin_cambios_solo_muestra_lo_privado_desde_antes() {
        let mut m = modelo(vec![
            cambio(
                "mods/a.jar",
                TipoContenido::Mod,
                GrupoPublicacion::Privado,
                false,
            ),
            cambio(
                "mods/b.jar",
                TipoContenido::Mod,
                GrupoPublicacion::Nuevo,
                false,
            ),
        ]);
        m.estado = EstadoRevisionPublicacion::SinCambios;
        assert_eq!(Resumen::calcular(&m).total, 1);
    }

    #[test]
    fn buscar_y_filtrar_por_nombre_archivo_y_mod() {
        let mut c = cambio(
            "config/x.json",
            TipoContenido::Config,
            GrupoPublicacion::Nuevo,
            false,
        );
        c.de_mod = Some(("mods/e.jar".into(), "Entity Culling".into()));
        let mut m = modelo(vec![c.clone()]);
        m.vista.busqueda = "entity".into();
        assert!(coincide(&m, &c));
        m.vista.filtro = FiltroPublicacion::SePublica;
        assert!(!coincide(&m, &c));
        assert_eq!(miles(1204), "1.204");
        assert_eq!(miles(612), "612");
    }
}
