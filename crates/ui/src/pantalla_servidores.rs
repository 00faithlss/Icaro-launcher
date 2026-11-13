// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! PantallaServidores: las piezas de la sección Servidores del System Design.
//! Pestañas con línea que se desliza, filas que se despliegan con alto suave,
//! notas con regla de color, confirmaciones en el mismo lugar, señal de
//! cuatro barras, pasos, cupo e icono de píxeles. Son vistas puras: la
//! aplicación arma los datos y recibe los mensajes.

use std::time::Instant;

use iced::advanced::layout::{Layout, Limits, Node};
use iced::advanced::renderer::{self, Quad};
use iced::advanced::widget::{Operation, Tree};
use iced::advanced::{mouse, overlay, Clipboard, Renderer as _, Shell, Widget};
use iced::widget::{button, column, container, row, text, Space};
use iced::{
    event, Alignment, Background, Border, Color, Element, Event, Length, Padding, Point, Rectangle, Renderer,
    Size, Theme, Vector,
};

use crate::estilo;
use crate::fuentes;
use crate::iconos::{icono, Icono, Tam};
use crate::movimiento::Curva;
use crate::tema::{borde, espacio, texto, Paleta};

/// Las tres pestañas de la sección.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Pestana {
    #[default]
    Servidores,
    ConAmigos,
    Red,
}

impl Pestana {
    pub const TODAS: [Pestana; 3] = [Pestana::Servidores, Pestana::ConAmigos, Pestana::Red];

    pub fn indice(self) -> usize {
        match self {
            Pestana::Servidores => 0,
            Pestana::ConAmigos => 1,
            Pestana::Red => 2,
        }
    }

    /// Flecha derecha o izquierda: la siguiente o la anterior, en círculo.
    pub fn vecina(self, paso: i32) -> Pestana {
        let i = (self.indice() as i32 + paso).rem_euclid(3) as usize;
        Pestana::TODAS[i]
    }
}

/// Avance (0 a 1) de una animación que empezó en `desde` y dura `ms`.
pub fn avance(desde: Option<Instant>, ahora: Instant, ms: u64) -> f32 {
    desde.map_or(1.0, |d| {
        (ahora.saturating_duration_since(d).as_millis() as f32 / ms as f32).clamp(0.0, 1.0)
    })
}

fn t<'a, M: 'a>(s: impl Into<String>, f: iced::Font, tam: (f32, f32), c: Color) -> Element<'a, M> {
    text(s.into())
        .font(f)
        .size(tam.0)
        .line_height(iced::widget::text::LineHeight::Absolute(tam.1.into()))
        .color(c)
        .into()
}

/// Cuerpo pequeño (13/19).
pub fn cuerpo_sm<'a, M: 'a>(s: impl Into<String>, c: Color) -> Element<'a, M> {
    t(s, fuentes::CUERPO, texto::BODY_SM, c)
}

/// Mono pequeño (11/16).
pub fn mono_sm<'a, M: 'a>(s: impl Into<String>, c: Color) -> Element<'a, M> {
    t(s, fuentes::MONO, texto::MONO_SM, c)
}

/// Rótulo en mayúsculas (`ic-label`).
pub fn rotulo<'a, M: 'a>(p: Paleta, s: &str) -> Element<'a, M> {
    t(s.to_uppercase(), fuentes::ETIQUETA, texto::LABEL, p.text_muted)
}

/// `ic-heading` a 15 px.
pub fn subtitulo<'a, M: 'a>(s: impl Into<String>, c: Color) -> Element<'a, M> {
    t(s, fuentes::CUERPO_NEGRITA, (15.0, 20.0), c)
}

/// `ic-title`.
pub fn titulo<'a, M: 'a>(s: impl Into<String>, c: Color) -> Element<'a, M> {
    t(s, fuentes::TITULO, texto::TITLE, c)
}

/// Contenedor de un color con borde opcional.
pub fn caja(fondo: Option<Color>, borde_color: Color, ancho: f32) -> impl Fn(&Theme) -> container::Style {
    move |_| container::Style {
        background: fondo.map(Background::Color),
        border: Border {
            color: borde_color,
            width: ancho,
            radius: 0.0.into(),
        },
        ..Default::default()
    }
}

/// Regla horizontal.
pub fn regla<'a, M: 'a>(alto: f32, color: Color) -> Element<'a, M> {
    container(Space::new(Length::Fill, alto))
        .style(estilo::bloque(color))
        .into()
}

/// Icono de píxeles de 8 × 8 (`ic-pixel`); `bits` sale de
/// `icaro_core::icono_instancia::patron` con el nombre.
pub fn pixel<'a, M: 'a>(p: Paleta, bits: [u8; 64], lado: f32) -> Element<'a, M> {
    let colores = [p.surface_sunken, p.border, p.border_strong, p.text_muted, p.text];
    let celda = lado / 8.0;
    let mut filas = column![];
    for y in 0..8 {
        let mut fila = row![];
        for x in 0..8 {
            fila = fila.push(
                container(Space::new(celda, celda))
                    .style(estilo::bloque(colores[bits[y * 8 + x] as usize % colores.len()])),
            );
        }
        filas = filas.push(fila);
    }
    container(filas).width(lado).height(lado).into()
}

/// Señal de cuatro barras (4 × 5/8/11/14 px). `None`: sin respuesta.
/// 4: menos de 60 ms; 3: hasta 120; 2: hasta 250; 1: más.
pub fn senal<'a, M: 'a>(p: Paleta, ms: Option<u32>) -> Element<'a, M> {
    let n = match ms {
        None => 0,
        Some(ms) if ms < 60 => 4,
        Some(ms) if ms < 120 => 3,
        Some(ms) if ms < 250 => 2,
        Some(_) => 1,
    };
    let mut fila = row![].spacing(2).align_y(Alignment::End);
    for i in 0..4 {
        let color = if i < n { p.success } else { p.border_strong };
        fila = fila.push(container(Space::new(4, 5.0 + 3.0 * i as f32)).style(estilo::bloque(color)));
    }
    container(fila).height(14).align_y(Alignment::End).into()
}

/// Tono de una nota.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tono {
    Neutro,
    Error,
    Alerta,
    Info,
    Exito,
}

fn colores_tono(p: Paleta, tono: Tono) -> (Color, Color) {
    match tono {
        Tono::Neutro => (p.surface_sunken, p.text),
        Tono::Error => (p.error_soft, p.error),
        Tono::Alerta => (p.warning_soft, p.warning),
        Tono::Info => (p.info_soft, p.info),
        Tono::Exito => (p.success_soft, p.success),
    }
}

/// `ic-sv__nota`: fondo suave con regla de 3 px a la izquierda, icono,
/// texto y una acción opcional a la derecha.
pub fn nota<'a, M: 'a>(
    p: Paleta,
    tono: Tono,
    glifo: Icono,
    contenido: Element<'a, M>,
    accion: Option<Element<'a, M>>,
) -> Element<'a, M> {
    let (fondo, regla_color) = colores_tono(p, tono);
    let mut fila = row![icono(glifo, Tam::Sm, p.text), container(contenido).width(Length::Fill)]
        .spacing(10)
        .align_y(Alignment::Center);
    if let Some(a) = accion {
        fila = fila.push(a);
    }
    con_regla(regla_color, fondo, container(fila).padding(Padding::from([10.0, 14.0])).width(Length::Fill).into())
}

/// Confirmación en el mismo lugar (`ic-sv__confirmar`): borde de 2 px,
/// fondo `accent-soft`, la pregunta en negrita y Cancelar / acción.
pub fn confirmar<'a, M: Clone + 'a>(
    p: Paleta,
    pregunta: &str,
    explicacion: &str,
    rotulo_si: &str,
    si: M,
    no: M,
) -> Element<'a, M> {
    let texto_c = row![
        t(pregunta.to_owned(), fuentes::CUERPO_NEGRITA, texto::BODY_SM, p.text),
        t(format!(" {explicacion}"), fuentes::CUERPO, texto::BODY_SM, p.text),
    ]
    .wrap();
    container(
        row![
            container(texto_c).width(Length::Fill),
            row![
                crate::componentes::boton_compacto(p, "Cancelar", crate::componentes::Variante::Secundario, Some(no)),
                crate::componentes::boton_compacto(p, rotulo_si, crate::componentes::Variante::Peligro, Some(si)),
            ]
            .spacing(espacio::S2),
        ]
        .spacing(espacio::S4)
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([12.0, 14.0]))
    .width(Length::Fill)
    .style(caja(Some(p.accent_soft), p.text, borde::MEDIO))
    .into()
}

/// Insignia de color de 8 × 8 (`ic-sv__marca`).
pub fn marca<'a, M: 'a>(color: Color) -> Element<'a, M> {
    container(Space::new(8, 8)).style(estilo::bloque(color)).into()
}

/// Barra de cupo: un bloque por plaza (hasta 25), llenos en tinta.
pub fn cupo<'a, M: 'a>(p: Paleta, ocupados: usize, limite: usize) -> Element<'a, M> {
    let n = limite.clamp(1, 25);
    let llenos = if limite == 0 { 0 } else { (ocupados * n).div_ceil(limite.max(1)) };
    let mut fila = row![].spacing(4);
    for i in 0..n {
        let color = if i < llenos.min(n) { p.text } else { p.border };
        fila = fila.push(
            container(Space::new(Length::Fill, 8))
                .width(Length::FillPortion(1))
                .style(estilo::bloque(color)),
        );
    }
    fila.width(Length::Fill).into()
}

/// Pasos de Compartir: tres columnas con número (check al hacerse) y una
/// barra de 2 px que avanza hasta el paso actual.
pub fn pasos<'a, M: 'a>(p: Paleta, actual: usize, barra: f32) -> Element<'a, M> {
    let nombres = ["Instancia", "Qué compartes", "Código"];
    let mut fila = row![].spacing(12);
    for (i, nombre) in nombres.iter().enumerate() {
        let n = i + 1;
        let hecho = n < actual;
        let es_actual = n == actual;
        let num: Element<'a, M> = if hecho {
            icono(Icono::Check, Tam::Sm, p.bg).into()
        } else {
            t(n.to_string(), fuentes::MONO, (11.0, 11.0), if es_actual { p.text } else { p.text_muted })
        };
        let cuadro = container(num).center_x(24).center_y(24).style(if hecho {
            caja(Some(p.text), p.text, 1.0)
        } else if es_actual {
            caja(None, p.text, 2.0)
        } else {
            caja(None, p.border_strong, 1.0)
        });
        fila = fila.push(
            row![
                cuadro,
                t(*nombre, fuentes::CUERPO_NEGRITA, (13.0, 18.0), if hecho || es_actual { p.text } else { p.text_muted })
            ]
            .spacing(10)
            .align_y(Alignment::Center)
            .width(Length::FillPortion(1)),
        );
    }
    // La barra cubre del primer paso al actual: (p*2+1)/3 del ancho.
    let fraccion = ((barra * 2.0 + 1.0) / 3.0).clamp(0.0, 1.0);
    let lleno = (fraccion * 1000.0) as u16;
    column![
        fila,
        Space::with_height(14),
        container(
            row![
                container(Space::new(Length::Fill, 2))
                    .width(Length::FillPortion(lleno.max(1)))
                    .style(estilo::bloque(p.text)),
                container(Space::new(Length::Fill, 2))
                    .width(Length::FillPortion((1000 - lleno).max(1)))
                    .style(estilo::bloque(p.border)),
            ]
            .width(Length::Fill)
        ),
    ]
    .into()
}

/// Lo que va a la derecha de una pestaña.
pub enum ExtraPestana {
    Numero(usize),
    /// Red privada: punto y «Conectada» o «Desconectada».
    Estado(bool),
}

/// Las tres pestañas (52 px de alto, relleno de 18 px) con regla de 1 px y
/// la línea de 2 px que se desliza de la pestaña `desde` a la `activa`.
pub fn pestanas<'a, M: Clone + 'a>(
    p: Paleta,
    activa: Pestana,
    desde: Pestana,
    progreso: f32,
    servidores: usize,
    amigos: usize,
    red_conectada: bool,
    al_elegir: impl Fn(Pestana) -> M,
) -> Element<'a, M> {
    let datos = [
        (Pestana::Servidores, Icono::Servidores, "Servidores", ExtraPestana::Numero(servidores)),
        (Pestana::ConAmigos, Icono::Usuarios, "Con amigos", ExtraPestana::Numero(amigos)),
        (Pestana::Red, Icono::Monitor, "Red privada", ExtraPestana::Estado(red_conectada)),
    ];
    let mut fila = row![].spacing(4);
    for (pestana, glifo, nombre, extra) in datos {
        let sel = pestana == activa;
        let construir = |color: Color| -> Element<'a, M> {
            let extra: Element<'a, M> = match &extra {
                ExtraPestana::Numero(n) => container(t(n.to_string(), fuentes::MONO, (11.0, 18.0), color))
                    .padding(Padding::from([0.0, 6.0]))
                    .center_x(Length::Shrink)
                    .width(Length::Shrink)
                    .style(caja(None, color, 1.0))
                    .into(),
                ExtraPestana::Estado(on) => row![
                    container(Space::new(8, 8)).style(estilo::bloque(if *on { p.success } else { p.text_disabled })),
                    t(if *on { "Conectada" } else { "Desconectada" }, fuentes::MONO, (11.0, 11.0), p.text_muted),
                ]
                .spacing(6)
                .align_y(Alignment::Center)
                .into(),
            };
            container(
                row![icono(glifo, Tam::Base, color), t(nombre, fuentes::CUERPO_NEGRITA, (14.0, 20.0), color), extra]
                    .spacing(10)
                    .align_y(Alignment::Center),
            )
            .center_y(52)
            .padding(Padding::from([0.0, 18.0]))
            .into()
        };
        let normal = construir(if sel { p.text } else { p.text_muted });
        let sobre = construir(p.text);
        fila = fila.push(
            button(alterno(normal, sobre))
                .padding(0)
                .on_press(al_elegir(pestana))
                .style(estilo::sin_estilo(p.text)),
        );
    }
    Element::new(LineaPestanas {
        fila: fila.into(),
        activa: activa.indice(),
        desde: desde.indice(),
        progreso: Curva::Salida.aplicar(progreso.clamp(0.0, 1.0)),
        regla: p.border,
        linea: p.text,
    })
}

/// Dibuja `normal` o, con el puntero encima, `sobre`; nunca los dos.
/// `iced::widget::hover` dibuja `sobre` en una capa encima de `normal` sin
/// ocultarla, y los textos quedan dos veces, uno sobre otro. Aquí todo va en
/// la misma capa, así que las reglas que se pintan después siguen encima.
/// Ambas variantes deben tener el mismo tamaño: la de referencia es `normal`.
pub fn alterno<'a, M: 'a>(normal: Element<'a, M>, sobre: Element<'a, M>) -> Element<'a, M> {
    Element::new(Alterno { normal, sobre })
}

struct Alterno<'a, M> {
    normal: Element<'a, M>,
    sobre: Element<'a, M>,
}

impl<M> Widget<M, Theme, Renderer> for Alterno<'_, M> {
    fn size(&self) -> Size<Length> {
        self.normal.as_widget().size()
    }
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.normal), Tree::new(&self.sobre)]
    }
    fn diff(&self, arbol: &mut Tree) {
        arbol.diff_children(&[&self.normal, &self.sobre]);
    }
    fn layout(&self, arbol: &mut Tree, renderer: &Renderer, limites: &Limits) -> Node {
        let base = self.normal.as_widget().layout(&mut arbol.children[0], renderer, limites);
        // Mismos límites que `normal`: acotar el ancho al medido hace que el
        // texto se parta por redondeo («Conectad / a»).
        let encima = self.sobre.as_widget().layout(&mut arbol.children[1], renderer, limites);
        Node::with_children(base.size(), vec![base, encima])
    }
    fn draw(
        &self,
        arbol: &Tree,
        renderer: &mut Renderer,
        tema: &Theme,
        estilo: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        visible: &Rectangle,
    ) {
        let mut hijos = layout.children();
        let (Some(base), Some(encima)) = (hijos.next(), hijos.next()) else { return };
        if cursor.is_over(layout.bounds()) {
            self.sobre.as_widget().draw(&arbol.children[1], renderer, tema, estilo, encima, cursor, visible);
        } else {
            self.normal.as_widget().draw(&arbol.children[0], renderer, tema, estilo, base, cursor, visible);
        }
    }
    fn mouse_interaction(&self, arbol: &Tree, layout: Layout<'_>, cursor: mouse::Cursor, visible: &Rectangle, renderer: &Renderer) -> mouse::Interaction {
        layout.children().next().map_or(mouse::Interaction::None, |base| {
            self.normal.as_widget().mouse_interaction(&arbol.children[0], base, cursor, visible, renderer)
        })
    }
}

struct LineaPestanas<'a, M> {
    fila: Element<'a, M>,
    activa: usize,
    desde: usize,
    progreso: f32,
    regla: Color,
    linea: Color,
}

impl<M> Widget<M, Theme, Renderer> for LineaPestanas<'_, M> {
    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Shrink)
    }
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.fila)]
    }
    fn diff(&self, arbol: &mut Tree) {
        arbol.diff_children(std::slice::from_ref(&self.fila));
    }
    fn layout(&self, arbol: &mut Tree, renderer: &Renderer, limites: &Limits) -> Node {
        let hijo = self.fila.as_widget().layout(&mut arbol.children[0], renderer, limites);
        Node::with_children(Size::new(limites.max().width, hijo.size().height), vec![hijo])
    }
    fn draw(
        &self,
        arbol: &Tree,
        renderer: &mut Renderer,
        tema: &Theme,
        estilo: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        visible: &Rectangle,
    ) {
        let caja = layout.bounds();
        let Some(hijo) = layout.children().next() else { return };
        self.fila.as_widget().draw(&arbol.children[0], renderer, tema, estilo, hijo, cursor, visible);
        let quad = |r: &mut Renderer, b: Rectangle, c: Color| {
            r.fill_quad(Quad { bounds: b, ..Quad::default() }, c);
        };
        // Regla de 1 px bajo todas las pestañas.
        quad(renderer, Rectangle::new(Point::new(caja.x, caja.y + caja.height - 1.0), Size::new(caja.width, 1.0)), self.regla);
        let botones: Vec<Rectangle> = hijo.children().map(|c| c.bounds()).collect();
        let (Some(a), Some(b)) = (botones.get(self.desde), botones.get(self.activa)) else { return };
        let k = self.progreso;
        let x = a.x + (b.x - a.x) * k;
        let w = a.width + (b.width - a.width) * k;
        quad(renderer, Rectangle::new(Point::new(x, caja.y + caja.height - 2.0), Size::new(w, 2.0)), self.linea);
    }
    fn operate(&self, arbol: &mut Tree, layout: Layout<'_>, renderer: &Renderer, op: &mut dyn Operation) {
        if let Some(hijo) = layout.children().next() {
            self.fila.as_widget().operate(&mut arbol.children[0], hijo, renderer, op);
        }
    }
    fn on_event(
        &mut self,
        arbol: &mut Tree,
        evento: Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        portapapeles: &mut dyn Clipboard,
        shell: &mut Shell<'_, M>,
        visible: &Rectangle,
    ) -> event::Status {
        let Some(hijo) = layout.children().next() else { return event::Status::Ignored };
        self.fila.as_widget_mut().on_event(&mut arbol.children[0], evento, hijo, cursor, renderer, portapapeles, shell, visible)
    }
    fn mouse_interaction(&self, arbol: &Tree, layout: Layout<'_>, cursor: mouse::Cursor, visible: &Rectangle, renderer: &Renderer) -> mouse::Interaction {
        layout.children().next().map_or(mouse::Interaction::None, |hijo| {
            self.fila.as_widget().mouse_interaction(&arbol.children[0], hijo, cursor, visible, renderer)
        })
    }
}

/// Despliegue con alto suave (`grid-template-rows: 0fr → 1fr`): ocupa
/// `k` (0 a 1) del alto de su contenido y recorta lo demás.
pub fn revelar<'a, M: 'a>(contenido: impl Into<Element<'a, M>>, k: f32) -> Element<'a, M> {
    Element::new(Revelar {
        contenido: contenido.into(),
        k: Curva::Salida.aplicar(k.clamp(0.0, 1.0)),
        dx: 0.0,
        velo: None,
        halo: None,
    })
}

/// Salida de una fila: se desliza `12 px` a la izquierda y se funde con el
/// fondo (`k` de 0 a 1).
pub fn salir<'a, M: 'a>(fondo: Color, contenido: impl Into<Element<'a, M>>, k: f32) -> Element<'a, M> {
    let k = k.clamp(0.0, 1.0);
    Element::new(Revelar {
        contenido: contenido.into(),
        k: 1.0,
        dx: -12.0 * k,
        velo: (k > 0.0).then_some(Color { a: k, ..fondo }),
        halo: None,
    })
}

/// Halo de 3 px alrededor de una fila recién agregada; se apaga con `k`.
pub fn halo<'a, M: 'a>(color: Color, contenido: impl Into<Element<'a, M>>, k: f32) -> Element<'a, M> {
    let k = k.clamp(0.0, 1.0);
    Element::new(Revelar {
        contenido: contenido.into(),
        k: 1.0,
        dx: 0.0,
        velo: None,
        halo: (k < 1.0).then_some(Color { a: color.a * (1.0 - k), ..color }),
    })
}

struct Revelar<'a, M> {
    contenido: Element<'a, M>,
    k: f32,
    dx: f32,
    velo: Option<Color>,
    halo: Option<Color>,
}

impl<M> Widget<M, Theme, Renderer> for Revelar<'_, M> {
    fn size(&self) -> Size<Length> {
        let s = self.contenido.as_widget().size();
        Size::new(s.width, Length::Shrink)
    }
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.contenido)]
    }
    fn diff(&self, arbol: &mut Tree) {
        arbol.diff_children(std::slice::from_ref(&self.contenido));
    }
    fn layout(&self, arbol: &mut Tree, renderer: &Renderer, limites: &Limits) -> Node {
        let hijo = self.contenido.as_widget().layout(&mut arbol.children[0], renderer, &limites.loose());
        let tam = hijo.size();
        Node::with_children(Size::new(tam.width, (tam.height * self.k).round()), vec![hijo])
    }
    fn draw(
        &self,
        arbol: &Tree,
        renderer: &mut Renderer,
        tema: &Theme,
        estilo: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        visible: &Rectangle,
    ) {
        let caja = layout.bounds();
        let Some(hijo) = layout.children().next() else { return };
        if self.k <= 0.0 {
            return;
        }
        if let Some(c) = self.halo {
            let b = hijo.bounds().expand(3.0);
            renderer.fill_quad(Quad { bounds: b, ..Quad::default() }, c);
        }
        let completo = self.k >= 1.0 && self.dx == 0.0 && self.velo.is_none();
        if completo {
            self.contenido.as_widget().draw(&arbol.children[0], renderer, tema, estilo, hijo, cursor, visible);
            return;
        }
        let recorte = Rectangle::new(caja.position(), Size::new(caja.width + 16.0, caja.height));
        renderer.with_layer(recorte, |r| {
            r.with_translation(Vector::new(self.dx, 0.0), |r| {
                self.contenido.as_widget().draw(&arbol.children[0], r, tema, estilo, hijo, mouse::Cursor::Unavailable, visible);
            });
        });
        if let Some(velo) = self.velo {
            renderer.with_layer(recorte, |r| {
                r.fill_quad(Quad { bounds: recorte, ..Quad::default() }, velo);
            });
        }
    }
    fn operate(&self, arbol: &mut Tree, layout: Layout<'_>, renderer: &Renderer, op: &mut dyn Operation) {
        if let Some(hijo) = layout.children().next() {
            self.contenido.as_widget().operate(&mut arbol.children[0], hijo, renderer, op);
        }
    }
    fn on_event(
        &mut self,
        arbol: &mut Tree,
        evento: Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        portapapeles: &mut dyn Clipboard,
        shell: &mut Shell<'_, M>,
        visible: &Rectangle,
    ) -> event::Status {
        let Some(hijo) = layout.children().next() else { return event::Status::Ignored };
        // Mientras se despliega o sale, no recibe clics.
        let cursor = if self.k >= 1.0 && self.velo.is_none() { cursor } else { mouse::Cursor::Unavailable };
        self.contenido.as_widget_mut().on_event(&mut arbol.children[0], evento, hijo, cursor, renderer, portapapeles, shell, visible)
    }
    fn mouse_interaction(&self, arbol: &Tree, layout: Layout<'_>, cursor: mouse::Cursor, visible: &Rectangle, renderer: &Renderer) -> mouse::Interaction {
        if self.k < 1.0 {
            return mouse::Interaction::None;
        }
        layout.children().next().map_or(mouse::Interaction::None, |hijo| {
            self.contenido.as_widget().mouse_interaction(&arbol.children[0], hijo, cursor, visible, renderer)
        })
    }
    fn overlay<'b>(
        &'b mut self,
        arbol: &'b mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        traslado: Vector,
    ) -> Option<overlay::Element<'b, M, Theme, Renderer>> {
        if self.k < 1.0 {
            return None;
        }
        let hijo = layout.children().next()?;
        self.contenido.as_widget_mut().overlay(&mut arbol.children[0], hijo, renderer, traslado)
    }
}

/// Botón de sección grande de Con amigos (`ic-sv__accion`): icono, título y
/// línea; la activa lleva borde de 2 px y fondo `accent-soft`.
pub fn accion_grande<'a, M: Clone + 'a>(
    p: Paleta,
    glifo: Icono,
    titulo_s: &str,
    linea: &str,
    activa: bool,
    al_pulsar: M,
) -> Element<'a, M> {
    let contenido = row![
        icono(glifo, Tam::Base, p.text),
        column![
            t(titulo_s.to_owned(), fuentes::CUERPO_NEGRITA, (14.0, 20.0), p.text),
            cuerpo_sm(linea.to_owned(), p.text_muted),
        ]
        .spacing(2),
    ]
    .spacing(14)
    .align_y(Alignment::Center);
    button(container(contenido).padding(if activa { Padding::from([13.0, 15.0]) } else { Padding::from([14.0, 16.0]) }))
        .width(Length::Fill)
        .padding(0)
        .on_press(al_pulsar)
        .style(move |_, estado| {
            let sobre = matches!(estado, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: Some(Background::Color(if activa { p.accent_soft } else { p.surface })),
                text_color: p.text,
                border: Border {
                    color: if activa || sobre { p.text } else { p.border_strong },
                    width: if activa { borde::MEDIO } else { borde::FINO },
                    radius: 0.0.into(),
                },
                ..Default::default()
            }
        })
        .into()
}

/// Ítem de la lista Compartes / Recibes (`ic-sv__item`): icono, nombre,
/// línea y cuadro de color; el actual con regla izquierda de 3 px.
pub fn item_lista<'a, M: Clone + 'a>(
    p: Paleta,
    nombre: &str,
    bits: [u8; 64],
    linea: &str,
    color_marca: Option<Color>,
    actual: bool,
    al_pulsar: M,
) -> Element<'a, M> {
    let construir = |sobre: bool| -> Element<'a, M> {
        let mut fila = row![
            pixel(p, bits, 32.0),
            column![
                t(nombre.to_owned(), fuentes::CUERPO_NEGRITA, (14.0, 18.0), p.text),
                text(linea.to_owned())
                    .font(fuentes::CUERPO)
                    .size(texto::BODY_SM.0)
                    .color(p.text_muted)
                    .wrapping(iced::widget::text::Wrapping::None),
            ]
            .spacing(2)
            .width(Length::Fill),
        ]
        .spacing(12)
        .align_y(Alignment::Center);
        if let Some(c) = color_marca {
            fila = fila.push(marca(c));
        }
        let fondo = if actual { Some(p.surface) } else if sobre { Some(p.surface_hover) } else { None };
        let interior = container(fila).padding(Padding::from([10.0, 12.0])).width(Length::Fill).clip(true);
        if actual {
            con_regla(p.text, p.surface, interior.into())
        } else {
            container(interior)
                .style(move |_: &Theme| container::Style { background: fondo.map(Background::Color), ..Default::default() })
                .into()
        }
    };
    let normal = construir(false);
    let sobre = construir(true);
    let contenido: Element<'a, M> = if actual {
        container(normal)
            .style(move |_: &Theme| container::Style { background: Some(Background::Color(p.surface)), ..Default::default() })
            .into()
    } else {
        iced::widget::hover(normal, sobre)
    };
    button(contenido)
        .width(Length::Fill)
        .padding(0)
        .on_press(al_pulsar)
        .style(estilo::sin_estilo(p.text))
        .into()
}

/// Cabecera de un panel de detalle (`ic-sv__dcab`): icono, título, línea,
/// algo a la derecha y regla de 2 px abajo.
pub fn cabecera_detalle<'a, M: 'a>(
    p: Paleta,
    icono_e: Element<'a, M>,
    titulo_s: &str,
    linea: &str,
    extra: Option<Element<'a, M>>,
) -> Element<'a, M> {
    let mut fila = row![
        icono_e,
        column![titulo(titulo_s.to_owned(), p.text), cuerpo_sm(linea.to_owned(), p.text_muted)]
            .spacing(4)
            .width(Length::Fill),
    ]
    .spacing(16)
    .align_y(Alignment::Center);
    if let Some(e) = extra {
        fila = fila.push(e);
    }
    column![fila, Space::with_height(20), regla(borde::MEDIO, p.text)].into()
}

/// Marca de 48 px en tinta con un icono (`ic-sv__dmarca`).
pub fn marca_grande<'a, M: 'a>(p: Paleta, glifo: Icono) -> Element<'a, M> {
    container(icono(glifo, Tam::Base, p.bg))
        .center_x(48)
        .center_y(48)
        .style(caja(Some(p.text), p.text, 0.0))
        .into()
}

/// Pie de un panel (`ic-sv__pie`): regla de 1 px y una fila de acciones.
pub fn pie<'a, M: 'a>(p: Paleta, contenido: Element<'a, M>) -> Element<'a, M> {
    column![regla(borde::FINO, p.border), Space::with_height(18), contenido].into()
}

/// Opción con radio (`ic-sv__op`): marcada lleva borde de 2 px y
/// `accent-soft`; deshabilitada, fondo hundido y su motivo a la derecha.
pub fn opcion<'a, M: Clone + 'a>(
    p: Paleta,
    marcada: bool,
    contenido: Element<'a, M>,
    motivo: Option<Element<'a, M>>,
    al_pulsar: Option<M>,
) -> Element<'a, M> {
    let no = al_pulsar.is_none();
    let radio = container(if marcada {
        container(Space::new(8, 8)).style(estilo::bloque(p.text))
    } else {
        container(Space::new(8, 8))
    })
    .center_x(16)
    .center_y(16)
    .style(caja(None, if no { p.border_strong } else { p.text }, borde::MEDIO));
    let mut fila = row![radio, container(contenido).width(Length::Fill)].spacing(14).align_y(Alignment::Center);
    if let Some(m) = motivo {
        fila = fila.push(m);
    }
    button(container(fila).padding(Padding::from([12.0, 14.0])))
        .width(Length::Fill)
        .padding(0)
        .on_press_maybe(al_pulsar)
        .style(move |_, estado| {
            let sobre = matches!(estado, button::Status::Hovered | button::Status::Pressed);
            button::Style {
                background: Some(Background::Color(if no {
                    p.surface_sunken
                } else if marcada {
                    p.accent_soft
                } else {
                    p.surface
                })),
                text_color: p.text,
                border: Border {
                    color: if marcada {
                        p.text
                    } else if sobre && !no {
                        p.text_muted
                    } else {
                        p.border_strong
                    },
                    width: if marcada { borde::MEDIO } else { borde::FINO },
                    radius: 0.0.into(),
                },
                ..Default::default()
            }
        })
        .into()
}

/// Fila de una persona (`ic-sv__per`): icono, nombre, estado y acción.
pub fn persona<'a, M: 'a>(
    p: Paleta,
    nombre: &str,
    bits: [u8; 64],
    detalle: Option<&str>,
    en_linea: bool,
    estado_s: &str,
    accion: Option<Element<'a, M>>,
) -> Element<'a, M> {
    let mut fila = row![
        pixel(p, bits, 28.0),
        container(cuerpo_sm(nombre.to_owned(), p.text)).width(Length::Fill),
    ]
    .spacing(14)
    .align_y(Alignment::Center);
    if let Some(d) = detalle {
        fila = fila.push(container(mono_sm(d.to_owned(), p.text_muted)).width(140));
    }
    fila = fila.push(
        row![
            container(Space::new(8, 8)).style(estilo::bloque(if en_linea { p.success } else { p.text_disabled })),
            mono_sm(estado_s.to_owned(), p.text_muted),
        ]
        .spacing(6)
        .align_y(Alignment::Center),
    );
    if let Some(a) = accion {
        fila = fila.push(a);
    }
    container(fila).center_y(36).padding(Padding::from([8.0, 0.0])).into()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn las_flechas_recorren_las_pestanas_en_circulo() {
        assert_eq!(Pestana::Servidores.vecina(-1), Pestana::Red);
        assert_eq!(Pestana::Red.vecina(1), Pestana::Servidores);
        assert_eq!(Pestana::Servidores.vecina(1), Pestana::ConAmigos);
    }

    #[test]
    fn el_avance_va_de_cero_a_uno() {
        let t = Instant::now();
        assert_eq!(avance(None, t, 300), 1.0);
        assert_eq!(avance(Some(t), t, 300), 0.0);
        assert_eq!(avance(Some(t), t + std::time::Duration::from_millis(600), 300), 1.0);
    }
}

/// Regla izquierda de 3 px sobre un fondo (`box-shadow: inset 3px 0 0`):
/// el color asoma como relleno a la izquierda del contenido.
pub fn con_regla<'a, M: 'a>(regla_color: Color, fondo: Color, contenido: Element<'a, M>) -> Element<'a, M> {
    container(container(contenido).width(Length::Fill).style(caja(Some(fondo), fondo, 0.0)))
        .padding(Padding { top: 0.0, right: 0.0, bottom: 0.0, left: 3.0 })
        .width(Length::Fill)
        .style(caja(Some(regla_color), regla_color, 0.0))
        .into()
}
