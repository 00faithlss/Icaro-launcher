// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Dibujo de los movimientos especiales del sistema: Umbral, Picar, Prensa,
//! Colocar y Lupa. La lógica (tiempos, curvas, geometría) vive en
//! [`crate::movimiento`]; aquí solo se pinta lo que esa lógica devuelve.
//!
//! Las capas animadas no guardan reloj propio: reciben los milisegundos
//! transcurridos y se redibujan con los cuadros de la aplicación, igual que
//! el resto de las transiciones.

use std::sync::OnceLock;

use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer::{self, Quad};
use iced::advanced::widget::{tree, Tree};
use iced::advanced::{Clipboard, Shell, Widget};
use iced::widget::canvas::{self, Canvas, Frame, Geometry, Path};
use iced::widget::image::Handle;
use iced::widget::{button, column, container, stack, text, Space};
use iced::{
    alignment, keyboard, mouse, Alignment, Background, Border, Color, Element, Event, Length,
    Padding, Point, Rectangle, Shadow, Size,
};

use crate::fuentes;
use crate::instancias::{ALTO_TARJETA, ANCHO_TARJETA};
use crate::laminas::Lamina;
use crate::movimiento::{self, FaseUmbral, Lupa, Preferencia};
use crate::tema::{borde, espacio, Modo, Paleta};

fn con_alfa(c: Color, a: f32) -> Color {
    Color { a: c.a * a.clamp(0.0, 1.0), ..c }
}

// --- Umbral ---------------------------------------------------------------

/// Forma en que la luz del Umbral llena la ventana.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EstiloUmbral {
    /// La luz sale del arco de la lámina (la original).
    #[default]
    Arco,
    /// Un círculo que se abre desde el centro.
    Iris,
    /// Un barrido de izquierda a derecha.
    Barrido,
    /// Persianas verticales que se abren a la vez.
    Persianas,
}
impl EstiloUmbral {
    pub const TODOS: [Self; 4] = [Self::Arco, Self::Iris, Self::Barrido, Self::Persianas];
    /// Estilo para una semilla: cambia en cada pulsación de Jugar.
    pub fn de_semilla(semilla: u64) -> Self {
        Self::TODOS[(semilla % Self::TODOS.len() as u64) as usize]
    }
}

/// Cómo se ve el Umbral: su entrada y si es de noche (sin luz blanca).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AspectoUmbral {
    pub estilo: EstiloUmbral,
    pub nocturno: bool,
}
impl AspectoUmbral {
    /// De noche la luz es tinta oscura con texto claro, para no deslumbrar.
    fn paleta(self, p: Paleta) -> Paleta {
        if !self.nocturno {
            return p;
        }
        Paleta {
            fulgor: Color::from_rgb8(0x16, 0x16, 0x14),
            on_fulgor: Color::from_rgb8(0xec, 0xec, 0xe8),
            ..p
        }
    }
    /// Intensidad del halo blanco: de noche apenas un brillo.
    fn halo(self) -> f32 {
        if self.nocturno {
            0.12
        } else {
            1.0
        }
    }
}

/// Es de noche entre las 21:00 y las 7:00, hora local.
pub fn es_de_noche(hora: u32) -> bool {
    hora >= 21 || hora < 7
}

fn lamina_umbral() -> &'static Handle {
    static CELDA: OnceLock<Handle> = OnceLock::new();
    CELDA.get_or_init(|| {
        Handle::from_bytes(&include_bytes!("../assets/laminas/umbral.webp")[..])
    })
}

struct CapaUmbral {
    fase: FaseUmbral,
    paleta: Paleta,
    /// Fundido de entrada o retirada por fallo de toda la capa.
    retirada: f32,
    aspecto: AspectoUmbral,
}

/// Rellena con `color` la región de la luz para un avance `k` según el estilo.
/// Con `rayas`, solo líneas de 2 px cada 6 px dentro de esa región.
fn region_luz(
    frame: &mut Frame,
    estilo: EstiloUmbral,
    tam: Size,
    arco: Rectangle,
    k: f32,
    color: Color,
    rayas: bool,
) {
    let ventana = Rectangle::new(Point::ORIGIN, tam);
    let rect = |frame: &mut Frame, r: Rectangle| {
        if rayas {
            let mut y = r.y;
            while y < r.y + r.height {
                frame.fill_rectangle(Point::new(r.x, y), Size::new(r.width, 2.0), color);
                y += 6.0;
            }
        } else {
            frame.fill_rectangle(r.position(), r.size(), color);
        }
    };
    match estilo {
        EstiloUmbral::Arco => rect(frame, movimiento::mezclar(arco, ventana, k)),
        EstiloUmbral::Barrido => rect(frame, Rectangle::new(Point::ORIGIN, Size::new(tam.width * k, tam.height))),
        EstiloUmbral::Persianas => {
            const N: usize = 8;
            let ancho = tam.width / N as f32;
            for i in 0..N {
                rect(
                    frame,
                    Rectangle::new(Point::new(i as f32 * ancho, 0.0), Size::new(ancho * k, tam.height)),
                );
            }
        }
        EstiloUmbral::Iris => {
            let c = ventana.center();
            let radio = k * (tam.width.hypot(tam.height) / 2.0);
            if rayas {
                // Franjas del círculo: cada línea ocupa el ancho de su cuerda.
                let mut y = (c.y - radio).max(0.0);
                while y < (c.y + radio).min(tam.height) {
                    let dy = y - c.y;
                    let media = (radio * radio - dy * dy).max(0.0).sqrt();
                    frame.fill_rectangle(Point::new(c.x - media, y), Size::new(2.0 * media, 2.0), color);
                    y += 6.0;
                }
            } else {
                frame.fill(&Path::circle(c, radio), color);
            }
        }
    }
}

impl<M> canvas::Program<M> for CapaUmbral {
    type State = ();

    fn draw(
        &self,
        _estado: &(),
        renderer: &iced::Renderer,
        _tema: &iced::Theme,
        limites: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let p = self.aspecto.paleta(self.paleta);
        let f = self.fase;
        let tam = limites.size();
        let mut frame = Frame::new(renderer, tam);
        let estilo = self.aspecto.estilo;

        // La lámina va debajo, como widget de imagen: el lienzo pinta las
        // imágenes por encima de sus formas, y la luz debe quedar sobre ella.
        // Trama de líneas de la pantalla, 1 px de cada 3.
        let mut y = 2.0;
        while y < tam.height {
            frame.fill_rectangle(Point::new(0.0, y), Size::new(tam.width, 1.0), con_alfa(Color::BLACK, 0.18 * self.retirada));
            y += 3.0;
        }
        // Velo de tinta.
        if f.velo > 0.0 {
            frame.fill_rectangle(Point::ORIGIN, tam, con_alfa(Color::BLACK, f.velo * self.retirada));
        }

        let arco = movimiento::arco_en(tam);

        // Halo: círculos concéntricos que aproximan el degradado radial de
        // 360 px, en el arco o en el centro según el estilo. De noche, tenue.
        if f.halo > 0.0 && matches!(estilo, EstiloUmbral::Arco | EstiloUmbral::Iris) {
            let centro = if estilo == EstiloUmbral::Arco {
                arco.center()
            } else {
                Rectangle::new(Point::ORIGIN, tam).center()
            };
            let intensidad = self.aspecto.halo();
            let radio = 180.0 * f.escala_halo;
            const ANILLOS: usize = 14;
            let a = 1.0 - 0.3_f32.powf(1.0 / ANILLOS as f32);
            for i in 0..ANILLOS {
                let r = radio * (1.0 - i as f32 / ANILLOS as f32);
                frame.fill(&Path::circle(centro, r), con_alfa(Color::WHITE, a * f.halo * intensidad * self.retirada));
            }
        }

        // Líneas horizontales de 2 px, paso de 6 px, que llenan la ventana.
        if f.lineas > 0.0 {
            region_luz(&mut frame, estilo, tam, arco, f.lineas, con_alfa(p.fulgor, self.retirada), true);
        }

        // Luz sólida, un poco detrás de las líneas.
        let alfa = f.luz_alfa * self.retirada;
        if alfa > 0.0 {
            region_luz(&mut frame, estilo, tam, arco, f.luz, con_alfa(p.fulgor, alfa), false);
        }

        vec![frame.into_geometry()]
    }
}

/// Pantalla de salida del Umbral: ABRIENDO EL UMBRAL y el registro de arranque.
fn salida<'a, M: Clone + 'a>(
    p: Paleta,
    registro: usize,
    lineas: &[String],
    opacidad: f32,
    al_volver: M,
    al_cancelar: Option<M>,
) -> Element<'a, M> {
    let mut log = column![].spacing(2);
    for (i, l) in lineas.iter().enumerate() {
        let alfa = if i < registro { opacidad } else { 0.0 };
        log = log.push(
            text(l.clone())
                .font(fuentes::MONO)
                .size(12)
                .line_height(iced::widget::text::LineHeight::Absolute(18.0.into()))
                .color(con_alfa(p.on_fulgor, alfa)),
        );
    }
    let volver = button(
        text("Volver").font(fuentes::ETIQUETA).size(12).color(con_alfa(p.on_fulgor, opacidad)),
    )
    .padding(Padding::from([8.0, 14.0]))
    .on_press(al_volver)
    .style(move |_, _| button::Style {
        background: None,
        text_color: con_alfa(p.on_fulgor, opacidad),
        border: Border { color: con_alfa(p.on_fulgor, opacidad), width: borde::MEDIO, radius: 0.0.into() },
        ..Default::default()
    });
    // Cancelar junto a Volver, con el mismo contorno.
    let mut botones = iced::widget::row![].spacing(espacio::S2);
    if let Some(c) = al_cancelar {
        botones = botones.push(
            button(text("Cancelar").font(fuentes::ETIQUETA).size(12).color(con_alfa(p.on_fulgor, opacidad)))
                .padding(Padding::from([8.0, 14.0]))
                .on_press(c)
                .style(move |_, _| button::Style {
                    background: None,
                    text_color: con_alfa(p.on_fulgor, opacidad),
                    border: Border { color: con_alfa(p.on_fulgor, opacidad), width: borde::MEDIO, radius: 0.0.into() },
                    ..Default::default()
                }),
        );
    }
    let botones = botones.push(volver);
    container(
        column![
            text("ABRIENDO\nEL UMBRAL")
                .font(fuentes::DISPLAY)
                .size(72)
                .line_height(iced::widget::text::LineHeight::Absolute(66.0.into()))
                .color(con_alfa(p.on_fulgor, opacidad)),
            log,
            botones,
        ]
        .spacing(espacio::S6)
        .align_x(Alignment::Start),
    )
    .padding(espacio::S12)
    .width(Length::Fill)
    .height(Length::Fill)
    .align_y(alignment::Vertical::Bottom)
    .into()
}

/// Umbral: momento firma al pulsar Jugar. `ms` son los milisegundos desde que
/// se pulsó. Cubre toda la ventana y se pinta sobre cualquier otra capa.
pub fn umbral<'a, M: Clone + 'a>(
    p: Paleta,
    ms: f32,
    pref: Preferencia,
    aspecto: AspectoUmbral,
    instancia: &str,
    al_volver: M,
) -> Element<'a, M> {
    let lineas = [
        format!("[{}] Java 21.0.5 · 6.144 MB asignados", hora()),
        format!(
            "[{}] Verificando mods de {instancia} · 0 conflictos",
            hora()
        ),
        format!("[{}] Cargando el cargador de mods", hora()),
        format!("[{}] Minecraft arrancando", hora()),
    ];
    umbral_registro(p, ms, pref, aspecto, &lineas, al_volver)
}

/// Mismo Umbral, alimentado por el registro real en vez de datos de demostración.
pub fn umbral_registro<'a, M: Clone + 'a>(
    p: Paleta,
    ms: f32,
    pref: Preferencia,
    aspecto: AspectoUmbral,
    lineas: &[String],
    al_volver: M,
) -> Element<'a, M> {
    umbral_registro_transicion(p, ms, pref, aspecto, lineas, None, al_volver, None)
}

/// Entrada original, retirada por fallo de 200 ms y Rastrillo al volver.
/// El recorte conserva el tamaño de la lámina y su arco durante la retirada.
pub fn umbral_registro_transicion<'a, M: Clone + 'a>(
    p: Paleta,
    ms: f32,
    pref: Preferencia,
    aspecto: AspectoUmbral,
    lineas: &[String],
    retirada: Option<(f32, bool)>,
    al_volver: M,
    al_cancelar: Option<M>,
) -> Element<'a, M> {
    let fase = movimiento::umbral(ms, pref);
    let (opacidad, recorte) = estado_capa_umbral(ms, pref, retirada);
    let capa = Canvas::new(CapaUmbral {
        fase,
        paleta: p,
        retirada: opacidad,
        aspecto,
    })
    .width(Length::Fill)
    .height(Length::Fill);
    let fondo: Element<'a, M> = Element::new(FondoUmbral {
        opacidad: if pref.reducir { 0.0 } else { opacidad },
    });
    let mut pila = stack![fondo, capa];
    if fase.salida {
        pila = pila.push(salida(
            aspecto.paleta(p),
            if ms > 1700. {
                lineas.len()
            } else {
                fase.registro
            },
            lineas,
            opacidad,
            al_volver,
            al_cancelar,
        ));
    }
    // El Umbral bloquea la interfaz mientras dura: nada debajo recibe clics.
    Element::new(RecorteUmbral {
        contenido: iced::widget::mouse_area(pila.width(Length::Fill).height(Length::Fill)).into(),
        restante: recorte,
    })
}

fn estado_capa_umbral(ms: f32, pref: Preferencia, retirada: Option<(f32, bool)>) -> (f32, f32) {
    let entrada = if pref.reducir { 1.0 } else { (ms / 120.0).clamp(0.0, 1.0) };
    match retirada {
        Some((ms, _)) if pref.reducir => (entrada * (1.0 - (ms / 120.0).clamp(0.0, 1.0)), 1.0),
        Some((ms, true)) => (entrada * movimiento::umbral_retirada(ms), 1.0),
        Some((ms, false)) => (entrada * movimiento::umbral_retirada(ms), 1.0),
        None => (entrada, 1.0),
    }
}

#[cfg(test)]
mod pruebas_salida_umbral {
    use super::*;

    #[test]
    fn salida_funde_la_capa_completa_sin_cortina() {
        for fallo in [false, true] {
            let (opacidad, alto) = estado_capa_umbral(3500., Preferencia { reducir: false }, Some((100., fallo)));
            assert!(opacidad > 0. && opacidad < 1.);
            assert_eq!(alto, 1.);
            assert_eq!(estado_capa_umbral(3500., Preferencia { reducir: false }, Some((200., fallo))), (0., 1.));
        }
    }
}

struct RecorteUmbral<'a, M> {
    contenido: Element<'a, M>,
    restante: f32,
}

/// FichaMod conserva su ancho y entra desde el borde donde está anclada.
pub fn panel_desde_derecha<'a, M: 'a>(contenido: Element<'a, M>, desfase: f32) -> Element<'a, M> {
    Element::new(PanelDerecho { contenido, desfase: desfase.max(0.0) })
}

struct PanelDerecho<'a, M> {
    contenido: Element<'a, M>,
    desfase: f32,
}

impl<M> Widget<M, iced::Theme, iced::Renderer> for PanelDerecho<'_, M> {
    fn size(&self) -> Size<Length> { Size::new(Length::Fill, Length::Fill) }
    fn children(&self) -> Vec<Tree> { vec![Tree::new(&self.contenido)] }
    fn diff(&self, arbol: &mut Tree) { arbol.diff_children(std::slice::from_ref(&self.contenido)); }
    fn layout(&self, arbol: &mut Tree, renderer: &iced::Renderer, limites: &layout::Limits) -> layout::Node {
        let tam = limites.resolve(Length::Fill, Length::Fill, Size::ZERO);
        let mut ficha = self.contenido.as_widget().layout(&mut arbol.children[0], renderer,
            &layout::Limits::new(Size::ZERO, tam));
        ficha.move_to_mut(Point::new((tam.width - ficha.size().width).max(0.0) + self.desfase, 0.0));
        layout::Node::with_children(tam, vec![ficha])
    }
    fn operate(&self, arbol: &mut Tree, layout: Layout<'_>, renderer: &iced::Renderer, operacion: &mut dyn iced::advanced::widget::Operation) {
        self.contenido.as_widget().operate(&mut arbol.children[0], layout.children().next().unwrap(), renderer, operacion);
    }
    fn on_event(&mut self, arbol: &mut Tree, evento: Event, layout: Layout<'_>, cursor: mouse::Cursor,
        renderer: &iced::Renderer, portapapeles: &mut dyn Clipboard, shell: &mut Shell<'_, M>, visible: &Rectangle) -> iced::event::Status {
        self.contenido.as_widget_mut().on_event(&mut arbol.children[0], evento, layout.children().next().unwrap(), cursor, renderer, portapapeles, shell, visible)
    }
    fn mouse_interaction(&self, arbol: &Tree, layout: Layout<'_>, cursor: mouse::Cursor, visible: &Rectangle, renderer: &iced::Renderer) -> mouse::Interaction {
        self.contenido.as_widget().mouse_interaction(&arbol.children[0], layout.children().next().unwrap(), cursor, visible, renderer)
    }
    fn overlay<'b>(&'b mut self, arbol: &'b mut Tree, layout: Layout<'_>, renderer: &iced::Renderer,
        translation: iced::Vector) -> Option<iced::advanced::overlay::Element<'b, M, iced::Theme, iced::Renderer>> {
        self.contenido.as_widget_mut().overlay(&mut arbol.children[0], layout.children().next().unwrap(), renderer, translation)
    }
    fn draw(&self, arbol: &Tree, renderer: &mut iced::Renderer, tema: &iced::Theme,
        estilo: &renderer::Style, layout: Layout<'_>, cursor: mouse::Cursor, visible: &Rectangle) {
        use iced::advanced::Renderer as _;
        let Some(recorte) = layout.bounds().intersection(visible) else { return; };
        renderer.with_layer(recorte, |renderer| {
            self.contenido.as_widget().draw(&arbol.children[0], renderer, tema, estilo, layout.children().next().unwrap(), cursor, &recorte);
        });
    }
}

#[cfg(test)]
mod pruebas_panel_selector {
    use super::*;

    #[test]
    fn selector_del_panel_abre_menu_y_emite_la_instancia_elegida() {
        let renderer = iced::Renderer::Secondary(iced_tiny_skia::Renderer::new(iced::Font::DEFAULT, 16.into()));
        for desfase in [0.0, 40.0] {
            let selector = iced::widget::pick_list(vec!["Primera", "Segunda"], None::<&str>, |v| v)
                .width(300);
            let mut panel = panel_desde_derecha(selector.into(), desfase);
            let mut arbol = Tree::new(panel.as_widget());
            let tam = Size::new(1000.0, 700.0);
            let nodo = panel.as_widget().layout(&mut arbol, &renderer, &layout::Limits::new(Size::ZERO, tam));
            let layout = Layout::new(&nodo);
            let cursor = mouse::Cursor::Available(layout.children().next().unwrap().bounds().center());
            let mut mensajes = Vec::new();
            let mut clipboard = iced::advanced::clipboard::Null;
            panel.as_widget_mut().on_event(&mut arbol,
                Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)), layout, cursor,
                &renderer, &mut clipboard, &mut Shell::new(&mut mensajes), &Rectangle::new(Point::ORIGIN, tam));
            let mut menu = panel.as_widget_mut().overlay(&mut arbol, layout, &renderer, iced::Vector::ZERO)
                .expect("El panel debe transmitir el desplegable abierto");
            let nodo_menu = menu.layout(&renderer, tam);
            let limites = Layout::new(&nodo_menu).bounds();
            let cursor = mouse::Cursor::Available(Point::new(limites.x + 15.0, limites.y + 15.0));
            menu.on_event(Event::Mouse(mouse::Event::CursorMoved { position: cursor.position().unwrap() }),
                Layout::new(&nodo_menu), cursor, &renderer, &mut clipboard, &mut Shell::new(&mut mensajes));
            menu.on_event(Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
                Layout::new(&nodo_menu), cursor, &renderer, &mut clipboard, &mut Shell::new(&mut mensajes));
            assert_eq!(mensajes, vec!["Primera"]);
        }
    }
}

/// Toast entra desde abajo sin cambiar el espacio ni desmontar sus controles.
pub fn aviso_desde_abajo<'a,M:'a>(contenido:Element<'a,M>,desfase:f32)->Element<'a,M>{
    Element::new(EntradaAviso{contenido,desfase:desfase.max(0.)})
}

struct EntradaAviso<'a, M> {
    contenido: Element<'a, M>,
    desfase: f32,
}

impl<M> Widget<M, iced::Theme, iced::Renderer> for EntradaAviso<'_, M> {
    fn size(&self) -> Size<Length> { self.contenido.as_widget().size() }
    fn children(&self) -> Vec<Tree> { vec![Tree::new(&self.contenido)] }
    fn diff(&self, arbol: &mut Tree) { arbol.diff_children(std::slice::from_ref(&self.contenido)); }
    fn layout(&self, arbol: &mut Tree, renderer: &iced::Renderer, limites: &layout::Limits) -> layout::Node {
        let mut ficha=self.contenido.as_widget().layout(&mut arbol.children[0],renderer,limites);
        let tam=ficha.size();
        ficha.move_to_mut(Point::new(0.,self.desfase));
        layout::Node::with_children(tam,vec![ficha])
    }
    fn operate(&self, arbol: &mut Tree, layout: Layout<'_>, renderer: &iced::Renderer, operacion: &mut dyn iced::advanced::widget::Operation) {
        self.contenido.as_widget().operate(&mut arbol.children[0], layout.children().next().unwrap(), renderer, operacion);
    }
    fn on_event(&mut self, arbol: &mut Tree, evento: Event, layout: Layout<'_>, cursor: mouse::Cursor,
        renderer: &iced::Renderer, portapapeles: &mut dyn Clipboard, shell: &mut Shell<'_, M>, visible: &Rectangle) -> iced::event::Status {
        self.contenido.as_widget_mut().on_event(&mut arbol.children[0], evento, layout.children().next().unwrap(), cursor, renderer, portapapeles, shell, visible)
    }
    fn mouse_interaction(&self, arbol: &Tree, layout: Layout<'_>, cursor: mouse::Cursor, visible: &Rectangle, renderer: &iced::Renderer) -> mouse::Interaction {
        self.contenido.as_widget().mouse_interaction(&arbol.children[0], layout.children().next().unwrap(), cursor, visible, renderer)
    }
    fn draw(&self, arbol: &Tree, renderer: &mut iced::Renderer, tema: &iced::Theme,
        estilo: &renderer::Style, layout: Layout<'_>, cursor: mouse::Cursor, visible: &Rectangle) {
        self.contenido.as_widget().draw(&arbol.children[0], renderer, tema, estilo, layout.children().next().unwrap(), cursor, visible);
    }
}
/// Descubre una tarjeta flotante desde su borde inferior: el recorte crece
/// hacia arriba y la tarjeta sube 12 px hasta asentarse. `progreso` de 0 a 1.
pub fn descubrir_desde_abajo<'a, M: 'a>(contenido: Element<'a, M>, progreso: f32) -> Element<'a, M> {
    Element::new(Descubrir { contenido, progreso: progreso.clamp(0.0, 1.0) })
}

struct Descubrir<'a, M> {
    contenido: Element<'a, M>,
    progreso: f32,
}

impl<M> Widget<M, iced::Theme, iced::Renderer> for Descubrir<'_, M> {
    fn size(&self) -> Size<Length> { self.contenido.as_widget().size() }
    fn children(&self) -> Vec<Tree> { vec![Tree::new(&self.contenido)] }
    fn diff(&self, arbol: &mut Tree) { arbol.diff_children(std::slice::from_ref(&self.contenido)); }
    fn layout(&self, arbol: &mut Tree, renderer: &iced::Renderer, limites: &layout::Limits) -> layout::Node {
        self.contenido.as_widget().layout(&mut arbol.children[0], renderer, limites)
    }
    fn operate(&self, arbol: &mut Tree, layout: Layout<'_>, renderer: &iced::Renderer, operacion: &mut dyn iced::advanced::widget::Operation) {
        self.contenido.as_widget().operate(&mut arbol.children[0], layout, renderer, operacion);
    }
    fn on_event(&mut self, arbol: &mut Tree, evento: Event, layout: Layout<'_>, cursor: mouse::Cursor,
        renderer: &iced::Renderer, portapapeles: &mut dyn Clipboard, shell: &mut Shell<'_, M>, visible: &Rectangle) -> iced::event::Status {
        self.contenido.as_widget_mut().on_event(&mut arbol.children[0], evento, layout, cursor, renderer, portapapeles, shell, visible)
    }
    fn mouse_interaction(&self, arbol: &Tree, layout: Layout<'_>, cursor: mouse::Cursor, visible: &Rectangle, renderer: &iced::Renderer) -> mouse::Interaction {
        self.contenido.as_widget().mouse_interaction(&arbol.children[0], layout, cursor, visible, renderer)
    }
    fn draw(&self, arbol: &Tree, renderer: &mut iced::Renderer, tema: &iced::Theme,
        estilo: &renderer::Style, layout: Layout<'_>, cursor: mouse::Cursor, visible: &Rectangle) {
        use iced::advanced::Renderer as _;
        if self.progreso >= 1.0 {
            self.contenido.as_widget().draw(&arbol.children[0], renderer, tema, estilo, layout, cursor, visible);
            return;
        }
        let caja = layout.bounds();
        let k = crate::movimiento::Curva::Salida.aplicar(self.progreso);
        let alto = caja.height * k;
        // Margen para el borde y la sombra dura de la tarjeta.
        let recorte = Rectangle::new(
            Point::new(caja.x - 2.0, caja.y + caja.height - alto),
            Size::new(caja.width + 12.0, alto + 12.0),
        );
        renderer.with_layer(recorte, |renderer| {
            renderer.with_translation(iced::Vector::new(0.0, (1.0 - k) * 12.0), |renderer| {
                self.contenido.as_widget().draw(&arbol.children[0], renderer, tema, estilo, layout, cursor, visible);
            });
        });
    }
}

/// La imagen estándar crea su propio recorte y puede escapar del Rastrillo.
/// Este fondo cruza el viewport recibido con la imagen, sin cambiar su cover.
struct FondoUmbral { opacidad: f32 }
impl<M> Widget<M, iced::Theme, iced::Renderer> for FondoUmbral {
    fn size(&self) -> Size<Length> { Size::new(Length::Fill, Length::Fill) }
    fn layout(&self, _: &mut Tree, _: &iced::Renderer, limites: &layout::Limits) -> layout::Node {
        layout::atomic(limites, Length::Fill, Length::Fill)
    }
    fn draw(&self, _: &Tree, renderer: &mut iced::Renderer, _: &iced::Theme,
        _: &renderer::Style, layout: Layout<'_>, _: mouse::Cursor, visible: &Rectangle) {
        use iced::advanced::image::Renderer as _;
        use iced::advanced::Renderer as _;
        if self.opacidad <= 0.0 { return; }
        let caja = layout.bounds();
        let Some(recorte) = caja.intersection(visible) else { return; };
        let dibujo = movimiento::cubrir(caja.size(), movimiento::UMBRAL_LAMINA);
        let dibujo = Rectangle { x: dibujo.x + caja.x, y: dibujo.y + caja.y, ..dibujo };
        renderer.with_layer(recorte, |renderer| {
            renderer.fill_quad(Quad { bounds: caja, ..Quad::default() },
                Background::Color(con_alfa(Color::from_rgb8(5, 5, 5), self.opacidad)));
            renderer.draw_image(iced::advanced::image::Image {
                handle: lamina_umbral().clone(),
                filter_method: iced::advanced::image::FilterMethod::Linear,
                rotation: iced::Radians(0.0), opacity: self.opacidad, snap: true,
            }, dibujo);
        });
    }
}
impl<M> Widget<M, iced::Theme, iced::Renderer> for RecorteUmbral<'_, M> {
    fn size(&self) -> Size<Length> { self.contenido.as_widget().size() }
    fn children(&self) -> Vec<Tree> { vec![Tree::new(&self.contenido)] }
    fn diff(&self, arbol: &mut Tree) { arbol.diff_children(std::slice::from_ref(&self.contenido)); }
    fn layout(&self, arbol: &mut Tree, renderer: &iced::Renderer, limites: &layout::Limits) -> layout::Node {
        self.contenido.as_widget().layout(&mut arbol.children[0], renderer, limites)
    }
    fn operate(&self, arbol: &mut Tree, layout: Layout<'_>, renderer: &iced::Renderer, operacion: &mut dyn iced::advanced::widget::Operation) {
        self.contenido.as_widget().operate(&mut arbol.children[0], layout, renderer, operacion);
    }
    fn on_event(&mut self, arbol: &mut Tree, evento: Event, layout: Layout<'_>, cursor: mouse::Cursor,
        renderer: &iced::Renderer, portapapeles: &mut dyn Clipboard, shell: &mut Shell<'_, M>, visible: &Rectangle) -> iced::event::Status {
        self.contenido.as_widget_mut().on_event(&mut arbol.children[0], evento, layout, cursor, renderer, portapapeles, shell, visible)
    }
    fn mouse_interaction(&self, arbol: &Tree, layout: Layout<'_>, cursor: mouse::Cursor, visible: &Rectangle, renderer: &iced::Renderer) -> mouse::Interaction {
        self.contenido.as_widget().mouse_interaction(&arbol.children[0], layout, cursor, visible, renderer)
    }
    fn draw(&self, arbol: &Tree, renderer: &mut iced::Renderer, tema: &iced::Theme,
        estilo: &renderer::Style, layout: Layout<'_>, cursor: mouse::Cursor, visible: &Rectangle) {
        use iced::advanced::Renderer as _;
        if self.restante <= 0.0 { return; }
        let caja = layout.bounds();
        let recorte = Rectangle { height: caja.height * self.restante, ..caja };
        let Some(recorte) = recorte.intersection(visible) else { return; };
        renderer.with_layer(recorte, |renderer| {
            self.contenido.as_widget().draw(&arbol.children[0], renderer, tema, estilo, layout, cursor, &recorte);
        });
    }
}

fn hora() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let s = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
        % 86_400;
    format!("{:02}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
}

// --- Prensa ---------------------------------------------------------------

struct CapaPrensa {
    /// Color del fondo sobre el que se imprime la lámina.
    fondo: Color,
    barra: Color,
    t: f32,
}

impl<M> canvas::Program<M> for CapaPrensa {
    type State = ();

    fn draw(
        &self,
        _estado: &(),
        renderer: &iced::Renderer,
        _tema: &iced::Theme,
        limites: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let tam = limites.size();
        let mut frame = Frame::new(renderer, tam);
        let (impreso, alfa_barra) = movimiento::prensa_barra(self.t);
        let y = tam.height * impreso;
        // Lo que aún no pasó por la prensa sigue en blanco.
        frame.fill_rectangle(
            Point::new(0.0, y),
            Size::new(tam.width, tam.height - y),
            self.fondo,
        );
        if alfa_barra > 0.0 {
            let by = (tam.height - 3.0) * impreso;
            frame.fill_rectangle(
                Point::new(0.0, by - 1.0),
                Size::new(tam.width, 5.0),
                con_alfa(self.fondo, alfa_barra),
            );
            frame.fill_rectangle(
                Point::new(0.0, by),
                Size::new(tam.width, 3.0),
                con_alfa(self.barra, alfa_barra),
            );
        }
        vec![frame.into_geometry()]
    }
}

/// Prensa: tapa de `fondo` que se retira de arriba abajo en ocho pasos, con la
/// barra de tinta de 3 px al frente. `t` va de 0 a 1 (`dur-prensa`, 420 ms).
/// Con `t >= 1` no ocupa nada visible.
pub fn prensa<'a, M: 'a>(p: Paleta, fondo: Color, t: f32, alto: f32) -> Element<'a, M> {
    let capa = Canvas::new(CapaPrensa {
        fondo,
        barra: p.text,
        t,
    })
    .width(Length::Fill)
    .height(alto);
    if t >= 1.0 {
        Space::new(Length::Fill, alto).into()
    } else {
        capa.into()
    }
}

// --- Picar y Colocar ------------------------------------------------------

/// Fase de la eliminación manteniendo presionado.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FasePicar {
    /// Etapa de grietas visible, de 0 a 10.
    Grietas(u32),
    /// La tarjeta ya se rompió; `ms` desde la rotura.
    Rota(f32),
}

struct CapaPicar {
    paleta: Paleta,
    semilla: String,
    fase: FasePicar,
}

impl<M> canvas::Program<M> for CapaPicar {
    type State = ();

    fn draw(
        &self,
        _estado: &(),
        renderer: &iced::Renderer,
        _tema: &iced::Theme,
        limites: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let p = self.paleta;
        let tam = limites.size();
        let mut frame = Frame::new(renderer, tam);
        match self.fase {
            FasePicar::Grietas(etapa) => {
                let pts = movimiento::grietas(&self.semilla);
                let n = movimiento::grietas_visibles(pts.len(), etapa);
                let (w, h) = (tam.width / 16.0, tam.height / 16.0);
                let tinta = if p.modo == Modo::Piedra { Color::from_rgb8(0x0d, 0x0d, 0x0d) } else { p.text };
                for (i, (x, y)) in pts.iter().take(n).enumerate() {
                    // Una de cada cinco grietas deja pasar la luz.
                    let color = if i % 5 == 3 { p.bg } else { tinta };
                    frame.fill_rectangle(
                        Point::new(f32::from(*x) * w, f32::from(*y) * h),
                        Size::new(w, h),
                        color,
                    );
                }
            }
            FasePicar::Rota(ms) => {
                for cy in 0..movimiento::ESCOMBROS_FILAS {
                    for cx in 0..movimiento::ESCOMBROS_COLS {
                        if let Some(e) = movimiento::escombro(cx, cy, tam.width, tam.height, ms) {
                            let color = if e.claro { p.text } else { p.text_muted };
                            frame.fill_rectangle(
                                Point::new(e.x + e.dx, e.y + e.dy),
                                Size::new(e.lado, e.lado),
                                con_alfa(color, e.alfa),
                            );
                        }
                    }
                }
            }
        }
        vec![frame.into_geometry()]
    }
}

/// Capa de Picar del tamaño de una tarjeta de instancia: grietas sobre la
/// tarjeta completa o, una vez rota, los escombros cayendo.
pub fn picar<'a, M: 'a>(p: Paleta, semilla: &str, fase: FasePicar) -> Element<'a, M> {
    Canvas::new(CapaPicar { paleta: p, semilla: semilla.to_owned(), fase })
        .width(ANCHO_TARJETA)
        .height(ALTO_TARJETA)
        .into()
}

struct CapaAro {
    color: Color,
    grosor: f32,
}

impl<M> canvas::Program<M> for CapaAro {
    type State = ();

    fn draw(
        &self,
        _estado: &(),
        renderer: &iced::Renderer,
        _tema: &iced::Theme,
        limites: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        let tam = limites.size();
        let mut frame = Frame::new(renderer, tam);
        let g = self.grosor;
        if g > 0.0 {
            for (o, s) in [
                (Point::ORIGIN, Size::new(tam.width, g)),
                (Point::new(0.0, tam.height - g), Size::new(tam.width, g)),
                (Point::ORIGIN, Size::new(g, tam.height)),
                (Point::new(tam.width - g, 0.0), Size::new(g, tam.height)),
            ] {
                frame.fill_rectangle(o, s, self.color);
            }
        }
        vec![frame.into_geometry()]
    }
}

/// Colocar: aro de 2 px que rodea a una tarjeta recién puesta mientras se
/// asienta (`dur-fast`). `t` va de 0 a 1.
pub fn colocar<'a, M: 'a>(p: Paleta, t: f32, ancho: f32, alto: f32) -> Element<'a, M> {
    Canvas::new(CapaAro { color: p.text, grosor: movimiento::colocar_aro(t) })
        .width(ancho)
        .height(alto)
        .into()
}

// --- Lupa -----------------------------------------------------------------

/// Estado de la lupa, guardado en el árbol de widgets.
#[derive(Debug, Default)]
struct EstadoLupa {
    lupa: Lupa,
    activa: bool,
}

struct LupaLamina {
    paleta: Paleta,
    lamina: Lamina,
}

/// Lámina ampliable: una lente cuadrada de 200 px sigue al puntero y la amplía
/// de 2× a 6× (rueda, + y −; flechas la mueven 2 %, con Mayús 10 %). Solo
/// se activa con un clic sobre láminas de 240 px o más.
pub fn lupa<'a, M: 'a>(p: Paleta, lamina: Lamina) -> Element<'a, M> {
    Element::new(LupaLamina { paleta: p, lamina })
}

impl<M> Widget<M, iced::Theme, iced::Renderer> for LupaLamina {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<EstadoLupa>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(EstadoLupa::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn layout(&self, _arbol: &mut Tree, _renderer: &iced::Renderer, limites: &layout::Limits) -> layout::Node {
        layout::atomic(limites, Length::Fill, Length::Fill)
    }

    fn on_event(
        &mut self,
        arbol: &mut Tree,
        evento: Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &iced::Renderer,
        _portapapeles: &mut dyn Clipboard,
        shell: &mut Shell<'_, M>,
        _visible: &Rectangle,
    ) -> iced::event::Status {
        let caja = layout.bounds();
        let estado = arbol.state.downcast_mut::<EstadoLupa>();
        let dentro = cursor.position_in(caja);
        match evento {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))
                if dentro.is_some() && Lupa::cabe(caja.size()) => {
                estado.activa = !estado.activa;
                if let Some(pos) = dentro { estado.lupa.apuntar(pos.x, pos.y, caja.size()); }
                shell.request_redraw(iced::window::RedrawRequest::NextFrame);
                iced::event::Status::Captured
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) if estado.activa => {
                if let Some(pos) = dentro {
                    estado.lupa.apuntar(pos.x, pos.y, caja.size());
                    shell.request_redraw(iced::window::RedrawRequest::NextFrame);
                }
                iced::event::Status::Ignored
            }
            Event::Mouse(mouse::Event::CursorLeft) if estado.activa => {
                estado.activa = false;
                shell.request_redraw(iced::window::RedrawRequest::NextFrame);
                iced::event::Status::Ignored
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) if estado.activa => {
                let y = match delta {
                    mouse::ScrollDelta::Lines { y, .. } | mouse::ScrollDelta::Pixels { y, .. } => y,
                };
                if y != 0.0 {
                    estado.lupa.rueda(y > 0.0);
                    shell.request_redraw(iced::window::RedrawRequest::NextFrame);
                    return iced::event::Status::Captured;
                }
                iced::event::Status::Ignored
            }
            Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) if estado.activa => {
                use keyboard::key::Named;
                let mayus = modifiers.shift();
                let hecho = match key.as_ref() {
                    keyboard::Key::Named(Named::ArrowLeft) => {
                        estado.lupa.mover(-1, 0, mayus);
                        true
                    }
                    keyboard::Key::Named(Named::ArrowRight) => {
                        estado.lupa.mover(1, 0, mayus);
                        true
                    }
                    keyboard::Key::Named(Named::ArrowUp) => {
                        estado.lupa.mover(0, -1, mayus);
                        true
                    }
                    keyboard::Key::Named(Named::ArrowDown) => {
                        estado.lupa.mover(0, 1, mayus);
                        true
                    }
                    keyboard::Key::Character("+" | "=") => {
                        estado.lupa.rueda(true);
                        true
                    }
                    keyboard::Key::Character("-") => {
                        estado.lupa.rueda(false);
                        true
                    }
                    _ => false,
                };
                if hecho {
                    shell.request_redraw(iced::window::RedrawRequest::NextFrame);
                    iced::event::Status::Captured
                } else {
                    iced::event::Status::Ignored
                }
            }
            _ => iced::event::Status::Ignored,
        }
    }

    fn mouse_interaction(
        &self,
        arbol: &Tree,
        _layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _visible: &Rectangle,
        _renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        // El puntero lo reemplaza la cruz de la lente.
        if arbol.state.downcast_ref::<EstadoLupa>().activa {
            mouse::Interaction::Crosshair
        } else {
            mouse::Interaction::default()
        }
    }

    fn draw(
        &self,
        arbol: &Tree,
        renderer: &mut iced::Renderer,
        _tema: &iced::Theme,
        _estilo: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        visible: &Rectangle,
    ) {
        use iced::advanced::image::Renderer as _;
        use iced::advanced::text::Renderer as _;
        use iced::advanced::Renderer as _;

        let caja = layout.bounds();
        let Some(recorte) = caja.intersection(visible) else {
            return;
        };
        let handle = self.lamina.imagen(self.paleta.modo).clone();
        let natural = renderer.measure_image(&handle);
        if natural.width == 0 || natural.height == 0 || caja.width <= 0.0 || caja.height <= 0.0 {
            return;
        }
        let p = self.paleta;
        let (iw, ih) = (natural.width as f32, natural.height as f32);
        let escala = (caja.width / iw).max(caja.height / ih);
        let (w, h) = (iw * escala, ih * escala);
        // Mismo encuadre que `grabado`: centrada y sin deformar.
        let dibujo = Rectangle {
            x: caja.center_x() - w / 2.0,
            y: caja.center_y() - h / 2.0,
            width: w,
            height: h,
        };
        let imagen = |handle: &Handle| iced::advanced::image::Image {
            handle: handle.clone(),
            filter_method: iced::advanced::image::FilterMethod::Linear,
            rotation: iced::Radians(0.0),
            opacity: 1.0,
            snap: true,
        };
        renderer.with_layer(recorte, |renderer| {
            renderer.draw_image(imagen(&handle), dibujo);
        });

        let estado = arbol.state.downcast_ref::<EstadoLupa>();
        if !estado.activa {
            return;
        }
        let lupa = estado.lupa;
        let centro = Point::new(caja.x + lupa.x * caja.width, caja.y + lupa.y * caja.height);
        let lado = movimiento::LUPA_LADO;
        let lente = Rectangle {
            x: centro.x - lado / 2.0,
            y: centro.y - lado / 2.0,
            width: lado,
            height: lado,
        };
        let expandir = |r: Rectangle, d: f32| Rectangle {
            x: r.x - d,
            y: r.y - d,
            width: r.width + 2.0 * d,
            height: r.height + 2.0 * d,
        };
        renderer.with_layer(recorte, |renderer| {
            // Doble regla: 2 px de tinta, 2 px de fondo y el marco de 2 px.
            renderer.fill_quad(Quad { bounds: expandir(lente, 6.0), ..Quad::default() }, Background::Color(p.text));
            renderer.fill_quad(Quad { bounds: expandir(lente, 4.0), ..Quad::default() }, Background::Color(p.bg));
            renderer.fill_quad(
                Quad {
                    bounds: lente,
                    border: Border { color: p.text, width: borde::MEDIO, radius: 0.0.into() },
                    shadow: Shadow::default(),
                },
                Background::Color(p.surface_raised),
            );
            // Lámina limpia, sin la trama de la interfaz, ampliada: el punto
            // apuntado queda en el centro de la lente.
            let z = f32::from(lupa.aumento);
            let ampliada = Rectangle {
                x: lente.x + lado / 2.0 - (centro.x - caja.x) * z + (dibujo.x - caja.x) * z,
                y: lente.y + lado / 2.0 - (centro.y - caja.y) * z + (dibujo.y - caja.y) * z,
                width: dibujo.width * z,
                height: dibujo.height * z,
            };
            // Una capa anidada no hereda el recorte de la exterior: se cruza a mano.
            if let Some(interior) = expandir(lente, -borde::MEDIO).intersection(&recorte) {
                renderer.with_layer(interior, |renderer| {
                    renderer.draw_image(imagen(&handle), ampliada);
                });
            }
        });
        // Segunda capa: la cruz y la ficha quedan por encima de la imagen ampliada.
        renderer.with_layer(recorte, |renderer| {
            // Cruz central de 12 px.
            let cruz = |b: Rectangle| Quad { bounds: b, ..Quad::default() };
            renderer.fill_quad(cruz(Rectangle { x: centro.x - 1.0, y: centro.y - 6.0, width: 2.0, height: 12.0 }), Background::Color(p.text));
            renderer.fill_quad(cruz(Rectangle { x: centro.x - 6.0, y: centro.y - 1.0, width: 12.0, height: 2.0 }), Background::Color(p.text));

            // Lectura en la esquina inferior de la lámina, como una ficha.
            let lectura = lupa.lectura(natural.width, natural.height);
            let ancho = lectura.chars().count() as f32 * 6.4 + 12.0;
            let ficha = Rectangle { x: caja.x + 8.0, y: caja.y + caja.height - 22.0, width: ancho, height: 14.0 + 4.0 };
            renderer.fill_quad(cruz(ficha), Background::Color(p.text));
            renderer.fill_text(
                iced::advanced::Text {
                    content: lectura,
                    bounds: ficha.size(),
                    size: iced::Pixels(10.0),
                    line_height: iced::advanced::text::LineHeight::Absolute(iced::Pixels(18.0)),
                    font: fuentes::MONO_MEDIO,
                    horizontal_alignment: alignment::Horizontal::Left,
                    vertical_alignment: alignment::Vertical::Top,
                    shaping: iced::advanced::text::Shaping::Basic,
                    wrapping: iced::advanced::text::Wrapping::None,
                },
                Point::new(ficha.x + 6.0, ficha.y),
                p.bg,
                ficha,
            );
        });
    }
}
