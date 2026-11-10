// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss
//! MenuAnclado: el menú nace de su botón y los dos forman una sola pieza.
//! Al abrir, el botón toma el mismo borde de 2 px del menú en tres lados y el
//! lado que lo toca queda abierto (una franja tapa el borde del menú en el
//! ancho del botón). Si no cabe hacia abajo, abre hacia arriba. Se descubre
//! desde el lado del botón y baja 6 px. Sigue al botón al desplazar o
//! redimensionar porque toma su posición del layout.
use iced::advanced::widget::tree;
use iced::advanced::{layout, mouse, overlay, renderer, widget::Tree, Clipboard, Layout, Shell, Widget};
use iced::{Background, Border, Element, Event, Length, Point, Rectangle, Size, Theme, Vector};

use crate::tema::{borde, Paleta};

/// A qué borde del botón se alinea el menú.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alineacion {
    Derecha,
    Izquierda,
}

/// Hacia dónde crece el menú.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direccion {
    /// Abajo si cabe; si no, arriba.
    Auto,
    Arriba,
}

/// Menú anclado sencillo, sin unión (se conserva para menús existentes).
pub fn menu<'a, M: Clone + 'a>(boton: Element<'a, M>, contenido: Element<'a, M>, abierto: bool, cerrar: M) -> Element<'a, M> {
    Element::new(Ancla { boton, contenido, abierto, cerrar, union: None, alinear: Alineacion::Izquierda, direccion: Direccion::Auto, progreso: 1.0 })
}

/// MenuAnclado unido a su botón. `progreso` (0 a 1) anima la apertura.
pub fn menu_unido<'a, M: Clone + 'a>(
    p: Paleta,
    boton: Element<'a, M>,
    contenido: Element<'a, M>,
    abierto: bool,
    cerrar: M,
    alinear: Alineacion,
    direccion: Direccion,
    progreso: f32,
) -> Element<'a, M> {
    Element::new(Ancla { boton, contenido, abierto, cerrar, union: Some(p), alinear, direccion, progreso })
}

struct Ancla<'a, M> {
    boton: Element<'a, M>,
    contenido: Element<'a, M>,
    abierto: bool,
    cerrar: M,
    union: Option<Paleta>,
    alinear: Alineacion,
    direccion: Direccion,
    progreso: f32,
}
impl<M: Clone> Widget<M, Theme, iced::Renderer> for Ancla<'_, M> {
    fn tag(&self) -> tree::Tag { tree::Tag::of::<EstadoAncla>() }
    fn state(&self) -> tree::State { tree::State::new(EstadoAncla::default()) }
    fn size(&self) -> Size<Length> { self.boton.as_widget().size() }
    fn children(&self) -> Vec<Tree> { vec![Tree::new(&self.boton), Tree::new(&self.contenido)] }
    fn diff(&self, tree: &mut Tree) { tree.children[0].diff(&self.boton); tree.children[1].diff(&self.contenido); }
    fn layout(&self, tree: &mut Tree, r: &iced::Renderer, limits: &layout::Limits) -> layout::Node {
        self.boton.as_widget().layout(&mut tree.children[0], r, limits)
    }
    fn draw(&self, tree: &Tree, r: &mut iced::Renderer, theme: &Theme, style: &renderer::Style, l: Layout<'_>, c: mouse::Cursor, v: &Rectangle) {
        self.boton.as_widget().draw(&tree.children[0], r, theme, style, l, c, v);
    }
    fn operate(&self, tree: &mut Tree, l: Layout<'_>, r: &iced::Renderer, op: &mut dyn iced::advanced::widget::Operation) {
        self.boton.as_widget().operate(&mut tree.children[0], l, r, op);
    }
    fn on_event(&mut self, tree: &mut Tree, e: Event, l: Layout<'_>, c: mouse::Cursor, r: &iced::Renderer, cb: &mut dyn Clipboard, s: &mut Shell<'_, M>, v: &Rectangle) -> iced::event::Status {
        self.boton.as_widget_mut().on_event(&mut tree.children[0], e, l, c, r, cb, s, v)
    }
    fn mouse_interaction(&self, tree: &Tree, l: Layout<'_>, c: mouse::Cursor, v: &Rectangle, r: &iced::Renderer) -> mouse::Interaction {
        self.boton.as_widget().mouse_interaction(&tree.children[0], l, c, v, r)
    }
    fn overlay<'b>(&'b mut self, tree: &'b mut Tree, l: Layout<'_>, r: &iced::Renderer, translation: Vector) -> Option<overlay::Element<'b, M, Theme, iced::Renderer>> {
        let Tree { state, children, .. } = tree;
        let estado = state.downcast_mut::<EstadoAncla>();
        if !self.abierto {
            *estado = EstadoAncla::default();
            return None;
        }
        let b = l.bounds();
        let boton = Rectangle::new(Point::new(b.x, b.y) + translation, b.size());
        let (arbol_boton, arbol_menu) = children.split_at_mut(1);
        // Layout propio del botón para volver a dibujarlo como pestaña.
        let nodo_boton = self
            .boton
            .as_widget()
            .layout(&mut arbol_boton[0], r, &layout::Limits::new(b.size(), b.size()))
            .move_to(Point::new(boton.x, boton.y));
        Some(overlay::Element::new(Box::new(Menu {
            contenido: &mut self.contenido,
            tree: &mut arbol_menu[0],
            boton_elemento: &self.boton,
            boton_arbol: &arbol_boton[0],
            nodo_boton,
            boton,
            cerrar: self.cerrar.clone(),
            union: self.union,
            alinear: self.alinear,
            direccion: self.direccion,
            progreso: self.progreso.clamp(0.0, 1.0),
            estado,
        })))
    }
}

/// Posición del menú simple: bajo el botón, o encima si no cabe.
fn posicion(ancla: Point, menu: Size, ventana: Size) -> Point {
    Point::new(ancla.x.clamp(8., (ventana.width - menu.width - 8.).max(8.)),
        if ancla.y + menu.height + 8. <= ventana.height { ancla.y + 4. } else { (ancla.y - menu.height - 32.).max(8.) })
}

/// De qué lado del botón queda el menú.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lado {
    Abajo,
    Arriba,
    /// No cabe ni abajo ni arriba: se abre al costado y el botón es la
    /// pestaña lateral.
    Izquierda,
    Derecha,
}

/// Posición del menú unido: pegado 2 px sobre el botón para compartir borde.
/// Abajo si cabe, si no arriba, y si tampoco al costado; nunca tapa su botón.
pub fn posicion_unida(boton: Rectangle, menu: Size, ventana: Size, alinear: Alineacion, direccion: Direccion) -> (Point, Lado) {
    let g = borde::MEDIO;
    // Un botón pegado al borde de la ventana deja su menú también pegado.
    let minimo = boton.x.clamp(0., 8.);
    let x = match alinear {
        Alineacion::Derecha => boton.x + boton.width - menu.width,
        Alineacion::Izquierda => boton.x,
    }
    .clamp(minimo, (ventana.width - menu.width - 8.).max(minimo));
    let abajo = boton.y + boton.height - g;
    let arriba = boton.y - menu.height + g;
    let cabe_abajo = abajo + menu.height + 8. <= ventana.height;
    let cabe_arriba = arriba >= 8.;
    if direccion == Direccion::Arriba {
        return (Point::new(x, arriba.max(8.)), Lado::Arriba);
    }
    if cabe_abajo {
        return (Point::new(x, abajo), Lado::Abajo);
    }
    if cabe_arriba {
        return (Point::new(x, arriba), Lado::Arriba);
    }
    // Al costado: arranca a la altura del botón y sube lo justo para caber.
    let y = boton.y.min(ventana.height - menu.height - 8.).max(8.);
    let izquierda = boton.x - menu.width + g;
    let derecha = boton.x + boton.width - g;
    let a_la_izquierda = match alinear {
        Alineacion::Derecha => izquierda >= 8.,
        Alineacion::Izquierda => derecha + menu.width + 8. > ventana.width && izquierda >= 8.,
    };
    if a_la_izquierda {
        (Point::new(izquierda, y), Lado::Izquierda)
    } else {
        (Point::new(derecha, y), Lado::Derecha)
    }
}

/// Lado en que quedó un menú ya colocado respecto de su botón.
fn lado_de(boton: Rectangle, menu: Rectangle) -> Lado {
    let g = borde::MEDIO + 0.5;
    if menu.y + menu.height <= boton.y + g {
        Lado::Arriba
    } else if menu.y >= boton.y + boton.height - g {
        Lado::Abajo
    } else if menu.x + menu.width <= boton.x + g {
        Lado::Izquierda
    } else {
        Lado::Derecha
    }
}

/// Duración del paso de un lado a otro cuando el menú deja de caber.
const MS_CAMBIO_LADO: f32 = 180.0;

/// Lo que el menú recuerda entre cuadros: el lado en que está, dónde se
/// dibujó respecto de su botón y, si cambió de lado, desde dónde se mueve.
#[derive(Debug, Default)]
struct EstadoAncla {
    lado: Option<Lado>,
    relativa: Vector,
    origen: Vector,
    desde: Option<std::time::Instant>,
}

impl EstadoAncla {
    /// Posición relativa al botón para este cuadro. Al cambiar de lado el
    /// menú se desliza desde donde estaba en vez de saltar.
    fn seguir(&mut self, lado: Lado, objetivo: Vector, ahora: std::time::Instant, reducir: bool) -> Vector {
        match self.lado {
            Some(previo) if previo != lado && !reducir => {
                self.origen = self.relativa;
                self.desde = Some(ahora);
            }
            Some(_) => {}
            None => self.desde = None,
        }
        self.lado = Some(lado);
        let relativa = match self.desde {
            Some(d) => {
                let t = (ahora.saturating_duration_since(d).as_secs_f32() * 1000.0 / MS_CAMBIO_LADO).min(1.0);
                if t >= 1.0 {
                    self.desde = None;
                    objetivo
                } else {
                    let k = crate::movimiento::Curva::Salida.aplicar(t);
                    self.origen + (objetivo - self.origen) * k
                }
            }
            None => objetivo,
        };
        self.relativa = relativa;
        relativa
    }
}

struct Menu<'a, 'b, M> {
    contenido: &'a mut Element<'b, M>,
    tree: &'a mut Tree,
    boton_elemento: &'a Element<'b, M>,
    boton_arbol: &'a Tree,
    nodo_boton: layout::Node,
    boton: Rectangle,
    cerrar: M,
    union: Option<Paleta>,
    alinear: Alineacion,
    direccion: Direccion,
    progreso: f32,
    estado: &'a mut EstadoAncla,
}
impl<M: Clone> overlay::Overlay<M, Theme, iced::Renderer> for Menu<'_, '_, M> {
    fn layout(&mut self, r: &iced::Renderer, bounds: Size) -> layout::Node {
        let mut node = self.contenido.as_widget().layout(self.tree, r, &layout::Limits::new(Size::ZERO, bounds));
        if self.union.is_some() {
            let (pos, lado) = posicion_unida(self.boton, node.size(), bounds, self.alinear, self.direccion);
            let origen = Point::new(self.boton.x, self.boton.y);
            // Con la apertura a medias no hay desde dónde deslizarse.
            let reducir = self.progreso < 1.0;
            let relativa = self.estado.seguir(lado, pos - origen, std::time::Instant::now(), reducir);
            node.move_to_mut(origen + relativa);
        } else {
            node.move_to_mut(posicion(Point::new(self.boton.x, self.boton.y + self.boton.height), node.size(), bounds));
        }
        node
    }
    fn draw(&self, r: &mut iced::Renderer, t: &Theme, s: &renderer::Style, l: Layout<'_>, c: mouse::Cursor) {
        use iced::advanced::Renderer as _;
        let menu = l.bounds();
        let Some(p) = self.union else {
            self.contenido.as_widget().draw(self.tree, r, t, s, l, c, &menu);
            return;
        };
        let b = self.boton;
        // El menú se descubre desde el lado del botón y se asienta 6 px.
        let lado = self.estado.lado.unwrap_or_else(|| lado_de(b, menu));
        let k = crate::movimiento::Curva::Salida.aplicar(self.progreso);
        let (alto, ancho) = (menu.height * k, menu.width * k);
        let recorte = match lado {
            Lado::Abajo => Rectangle::new(Point::new(menu.x - 2., menu.y), Size::new(menu.width + 12., alto + 12.)),
            Lado::Arriba => Rectangle::new(Point::new(menu.x - 2., menu.y + menu.height - alto), Size::new(menu.width + 12., alto + 12.)),
            Lado::Derecha => Rectangle::new(Point::new(menu.x, menu.y - 2.), Size::new(ancho + 12., menu.height + 12.)),
            Lado::Izquierda => Rectangle::new(Point::new(menu.x + menu.width - ancho, menu.y - 2.), Size::new(ancho + 12., menu.height + 12.)),
        };
        let resto = (1. - k) * 6.;
        let desplazar = match lado {
            Lado::Abajo => Vector::new(0., -resto),
            Lado::Arriba => Vector::new(0., resto),
            Lado::Derecha => Vector::new(-resto, 0.),
            Lado::Izquierda => Vector::new(resto, 0.),
        };
        r.with_layer(recorte, |r| {
            r.with_translation(desplazar, |r| {
                self.contenido.as_widget().draw(self.tree, r, t, s, l, c, &menu);
            });
        });
        // Encima, el botón abierto como pestaña: superficie elevada y borde de
        // 2 px en tres lados. El lado que toca el menú queda abierto y su
        // relleno tapa el borde del menú en ese tramo, y también su sombra.
        r.with_layer(Rectangle::new(Point::ORIGIN, Size::new(f32::MAX / 4., f32::MAX / 4.)), |r| {
            let quad = |r: &mut iced::Renderer, x: f32, y: f32, w: f32, h: f32, color| {
                r.fill_quad(
                    renderer::Quad { bounds: Rectangle::new(Point::new(x, y), Size::new(w, h)), ..Default::default() },
                    Background::Color(color),
                );
            };
            quad(r, b.x, b.y, b.width, b.height, p.surface_raised);
            // Sin cursor: abierto, el botón no muestra su estado de hover, que
            // se dibuja en una capa propia y taparía los bordes de la pestaña.
            let estilo = renderer::Style { text_color: p.text };
            self.boton_elemento.as_widget().draw(self.boton_arbol, r, t, &estilo, Layout::new(&self.nodo_boton), mouse::Cursor::Unavailable, &b);
        });
        // Los bordes van en su propia capa, siempre por encima del botón.
        r.with_layer(Rectangle::new(Point::ORIGIN, Size::new(f32::MAX / 4., f32::MAX / 4.)), |r| {
            let quad = |r: &mut iced::Renderer, x: f32, y: f32, w: f32, h: f32, color| {
                r.fill_quad(
                    renderer::Quad { bounds: Rectangle::new(Point::new(x, y), Size::new(w, h)), ..Default::default() },
                    Background::Color(color),
                );
            };
            let g = borde::MEDIO;
            // Borde en los tres lados que no tocan el menú.
            if lado != Lado::Derecha {
                quad(r, b.x + b.width - g, b.y, g, b.height, p.text);
            }
            if lado != Lado::Izquierda {
                quad(r, b.x, b.y, g, b.height, p.text);
            }
            if lado != Lado::Abajo {
                quad(r, b.x, b.y + b.height - g, b.width, g, p.text);
            }
            if lado != Lado::Arriba {
                quad(r, b.x, b.y, b.width, g, p.text);
            }
        });
        let _ = Border::default();
    }
    fn on_event(&mut self, e: Event, l: Layout<'_>, c: mouse::Cursor, r: &iced::Renderer, cb: &mut dyn Clipboard, s: &mut Shell<'_, M>) -> iced::event::Status {
        if matches!(e, Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left))) && !c.is_over(l.bounds()) {
            s.publish(self.cerrar.clone()); return iced::event::Status::Captured;
        }
        if let Event::Keyboard(iced::keyboard::Event::KeyPressed { key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape | iced::keyboard::key::Named::Tab), .. }) = &e {
            s.publish(self.cerrar.clone()); return iced::event::Status::Captured;
        }
        if self.progreso < 1.0 || self.estado.desde.is_some() {
            s.request_redraw(iced::window::RedrawRequest::NextFrame);
        }
        self.contenido.as_widget_mut().on_event(self.tree, e, l, c, r, cb, s, &l.bounds())
    }
    fn mouse_interaction(&self, l: Layout<'_>, c: mouse::Cursor, v: &Rectangle, r: &iced::Renderer) -> mouse::Interaction {
        self.contenido.as_widget().mouse_interaction(self.tree, l, c, v, r)
    }
    fn operate(&mut self, l: Layout<'_>, r: &iced::Renderer, op: &mut dyn iced::advanced::widget::Operation) {
        self.contenido.as_widget().operate(self.tree, l, r, op);
    }
    fn is_over(&self, l: Layout<'_>, _: &iced::Renderer, p: Point) -> bool {
        // El menú y su botón cuentan como la misma pieza.
        l.bounds().contains(p) || self.boton.contains(p) || self.union.is_none()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sigue_el_boton_al_redimensionar_y_respeta_los_bordes() {
        let menu = Size::new(260., 84.);
        assert_eq!(posicion(Point::new(740., 450.), menu, Size::new(1280., 800.)).x, 740.);
        assert_eq!(posicion(Point::new(1000., 434.), menu, Size::new(1920., 1080.)).x, 1000.);
        let p = posicion(Point::new(1850., 1040.), menu, Size::new(1920., 1080.));
        assert!(p.x + menu.width <= 1912. && p.y + menu.height <= 1072.);
    }

    #[test]
    fn el_menu_unido_monta_su_borde_sobre_el_boton_y_abre_arriba_si_no_cabe() {
        let ventana = Size::new(1280., 800.);
        let menu = Size::new(300., 400.);
        let boton = Rectangle::new(Point::new(900., 200.), Size::new(36., 36.));
        let (p, lado) = posicion_unida(boton, menu, ventana, Alineacion::Derecha, Direccion::Auto);
        assert_eq!(lado, Lado::Abajo);
        assert_eq!(p.x + menu.width, boton.x + boton.width);
        assert_eq!(p.y, boton.y + boton.height - borde::MEDIO);
        let bajo = Rectangle::new(Point::new(900., 600.), Size::new(36., 36.));
        let (p, lado) = posicion_unida(bajo, menu, ventana, Alineacion::Derecha, Direccion::Auto);
        assert_eq!(lado, Lado::Arriba);
        assert_eq!(p.y + menu.height, bajo.y + borde::MEDIO);
        let (_, lado) = posicion_unida(boton, menu, ventana, Alineacion::Izquierda, Direccion::Arriba);
        assert_eq!(lado, Lado::Arriba);
    }

    #[test]
    fn al_cambiar_de_lado_el_menu_se_desliza_y_luego_sigue_al_boton() {
        let t = std::time::Instant::now();
        let ms = |n: u64| t + std::time::Duration::from_millis(n);
        let mut e = EstadoAncla::default();
        let arriba = Vector::new(-256., -400.);
        let costado = Vector::new(-298., -200.);
        // Al abrir no hay transición.
        assert_eq!(e.seguir(Lado::Arriba, arriba, t, false), arriba);
        // Cambia de lado: empieza donde estaba y llega al nuevo sitio.
        let inicio = e.seguir(Lado::Izquierda, costado, t, false);
        assert!((inicio.x - arriba.x).abs() < 1. && (inicio.y - arriba.y).abs() < 1.);
        let medio = e.seguir(Lado::Izquierda, costado, ms(90), false);
        assert!(medio.x < arriba.x && medio.x > costado.x);
        assert_eq!(e.seguir(Lado::Izquierda, costado, ms(200), false), costado);
        assert!(e.desde.is_none());
        // Dentro del mismo lado sigue al botón sin animar.
        let otro = Vector::new(-298., -180.);
        assert_eq!(e.seguir(Lado::Izquierda, otro, ms(210), false), otro);
        // Con movimiento reducido salta.
        assert_eq!(e.seguir(Lado::Abajo, Vector::ZERO, ms(220), true), Vector::ZERO);
    }

    #[test]
    fn si_no_cabe_arriba_ni_abajo_abre_al_costado_sin_tapar_el_boton() {
        let ventana = Size::new(1280., 800.);
        let menu = Size::new(300., 620.);
        for y in [200., 400., 567.] {
            let boton = Rectangle::new(Point::new(543., y), Size::new(44., 44.));
            let (p, lado) = posicion_unida(boton, menu, ventana, Alineacion::Derecha, Direccion::Auto);
            assert_eq!(lado, Lado::Izquierda);
            assert_eq!(p.x + menu.width, boton.x + borde::MEDIO);
            assert!(p.y >= 8. && p.y + menu.height <= ventana.height - 8.);
            assert!(p.y <= boton.y && p.y + menu.height >= boton.y + boton.height);
            assert_eq!(lado_de(boton, Rectangle::new(p, menu)), Lado::Izquierda);
        }
        // Pegado al borde izquierdo de la ventana, sale por la derecha.
        let boton = Rectangle::new(Point::new(100., 400.), Size::new(44., 44.));
        let (p, lado) = posicion_unida(boton, menu, ventana, Alineacion::Derecha, Direccion::Auto);
        assert_eq!(lado, Lado::Derecha);
        assert_eq!(p.x, boton.x + boton.width - borde::MEDIO);
    }
}
