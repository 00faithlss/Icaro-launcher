// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Transición entre fases dentro de una misma página: la fase nueva entra
//! deslizándose 24 px y aparece sobre el fondo, sin rebotes. Avanzar entra
//! desde la derecha; volver, desde la izquierda. Con movimiento reducido el
//! cambio es inmediato.
//!
//! El estado vive en el widget: la aplicación solo cambia `clave` al pasar
//! de una fase a otra.

use std::time::{Duration, Instant};

use iced::advanced::layout::{Layout, Limits, Node};
use iced::advanced::renderer::{self, Quad};
use iced::advanced::widget::{tree, Operation, Tree};
use iced::advanced::{mouse, overlay, Clipboard, Renderer as _, Shell, Widget};
use iced::window::RedrawRequest;
use iced::{event, Color, Element, Event, Rectangle, Renderer, Size, Theme, Vector};

use crate::movimiento::Curva;

/// Desplazamiento inicial de la fase que entra.
const DESPLAZAMIENTO: f32 = 24.0;
/// Duración de la entrada (`dur-base`).
const DURACION: Duration = Duration::from_millis(crate::tema::duracion::BASE);

/// Envuelve el contenido de una fase. Cuando `clave` cambia, la fase nueva
/// entra con el movimiento; `fondo` es el color sobre el que aparece.
/// `clave` sigue el orden del flujo: una clave mayor avanza y entra desde la
/// derecha; una menor vuelve y entra desde la izquierda.
pub fn fase<'a, M: 'a>(
    fondo: Color,
    clave: u64,
    contenido: impl Into<Element<'a, M>>,
) -> Element<'a, M> {
    Element::new(Fase {
        contenido: contenido.into(),
        clave,
        fondo,
        subir: None,
        duracion: DURACION,
    })
}

/// Entrada vertical: el contenido nuevo aparece sobre el fondo y sube `dy`
/// píxeles en `ms` milisegundos cada vez que cambia `clave` (pestañas y
/// paneles de PantallaServidores: 8 px en 260 ms, 6 px en 260 ms).
pub fn subir<'a, M: 'a>(
    fondo: Color,
    clave: u64,
    dy: f32,
    ms: u64,
    contenido: impl Into<Element<'a, M>>,
) -> Element<'a, M> {
    Element::new(Fase {
        contenido: contenido.into(),
        clave,
        fondo,
        subir: Some(dy),
        duracion: Duration::from_millis(ms),
    })
}

struct Fase<'a, M> {
    contenido: Element<'a, M>,
    clave: u64,
    fondo: Color,
    /// `Some(dy)`: entra subiendo; `None`: entra de lado.
    subir: Option<f32>,
    duracion: Duration,
}

#[derive(Debug, Default)]
struct Estado {
    clave: u64,
    inicio: Option<Instant>,
    adelante: bool,
}

impl Estado {
    /// Progreso de 0 a 1 de la entrada; `None` si no anima.
    #[cfg(test)]
    fn progreso(&self, ahora: Instant) -> Option<f32> {
        self.progreso_en(ahora, DURACION)
    }

    fn progreso_en(&self, ahora: Instant, duracion: Duration) -> Option<f32> {
        let inicio = self.inicio?;
        let t = ahora.saturating_duration_since(inicio).as_secs_f32() / duracion.as_secs_f32();
        (t < 1.0).then_some(t)
    }
}

/// Desplazamiento horizontal y opacidad del velo para un progreso `t`.
fn aspecto(t: f32, adelante: bool) -> (f32, f32) {
    let k = Curva::Salida.aplicar(t);
    let lado = if adelante { 1.0 } else { -1.0 };
    ((1.0 - k) * DESPLAZAMIENTO * lado, 1.0 - k)
}

impl<'a, M> Widget<M, Theme, Renderer> for Fase<'a, M> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Estado>()
    }

    fn state(&self) -> tree::State {
        // La primera fase aparece sin movimiento: la página ya entró.
        tree::State::new(Estado {
            clave: self.clave,
            inicio: None,
            adelante: true,
        })
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.contenido)]
    }

    fn diff(&self, arbol: &mut Tree) {
        let estado = arbol.state.downcast_mut::<Estado>();
        if estado.clave != self.clave {
            estado.adelante = self.clave > estado.clave;
            estado.clave = self.clave;
            estado.inicio = (!crate::movimiento::preferencia().reducir).then(Instant::now);
        }
        arbol.diff_children(std::slice::from_ref(&self.contenido));
    }

    fn size(&self) -> Size<iced::Length> {
        self.contenido.as_widget().size()
    }

    fn layout(&self, arbol: &mut Tree, renderer: &Renderer, limites: &Limits) -> Node {
        let hijo = self
            .contenido
            .as_widget()
            .layout(&mut arbol.children[0], renderer, limites);
        Node::with_children(hijo.size(), vec![hijo])
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
        let Some(hijo) = layout.children().next() else {
            return;
        };
        let estado = arbol.state.downcast_ref::<Estado>();
        let Some(t) = estado.progreso_en(Instant::now(), self.duracion) else {
            self.contenido.as_widget().draw(
                &arbol.children[0],
                renderer,
                tema,
                estilo,
                hijo,
                cursor,
                visible,
            );
            return;
        };
        let (dx, velo) = aspecto(t, estado.adelante);
        let traslado = match self.subir {
            Some(dy) => Vector::new(0.0, (1.0 - Curva::Salida.aplicar(t)) * dy),
            None => Vector::new(dx, 0.0),
        };
        let caja = layout.bounds();
        renderer.with_layer(caja, |r| {
            r.with_translation(traslado, |r| {
                self.contenido.as_widget().draw(
                    &arbol.children[0],
                    r,
                    tema,
                    estilo,
                    hijo,
                    mouse::Cursor::Unavailable,
                    visible,
                );
            });
        });
        // El velo va en una capa propia para quedar sobre el texto.
        renderer.with_layer(caja, |r| {
            r.fill_quad(
                Quad {
                    bounds: caja,
                    ..Quad::default()
                },
                Color {
                    a: velo,
                    ..self.fondo
                },
            );
        });
    }

    fn operate(
        &self,
        arbol: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operacion: &mut dyn Operation,
    ) {
        if let Some(hijo) = layout.children().next() {
            self.contenido
                .as_widget()
                .operate(&mut arbol.children[0], hijo, renderer, operacion);
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
        let Some(hijo) = layout.children().next() else {
            return event::Status::Ignored;
        };
        if let Event::Window(iced::window::Event::RedrawRequested(ahora)) = &evento {
            let estado = arbol.state.downcast_mut::<Estado>();
            if estado.progreso_en(*ahora, self.duracion).is_some() {
                shell.request_redraw(RedrawRequest::NextFrame);
            } else {
                estado.inicio = None;
            }
        }
        self.contenido.as_widget_mut().on_event(
            &mut arbol.children[0],
            evento,
            hijo,
            cursor,
            renderer,
            portapapeles,
            shell,
            visible,
        )
    }

    fn mouse_interaction(
        &self,
        arbol: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        visible: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        layout
            .children()
            .next()
            .map_or(mouse::Interaction::None, |hijo| {
                self.contenido.as_widget().mouse_interaction(
                    &arbol.children[0],
                    hijo,
                    cursor,
                    visible,
                    renderer,
                )
            })
    }

    fn overlay<'b>(
        &'b mut self,
        arbol: &'b mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        traslado: Vector,
    ) -> Option<overlay::Element<'b, M, Theme, Renderer>> {
        let hijo = layout.children().next()?;
        self.contenido
            .as_widget_mut()
            .overlay(&mut arbol.children[0], hijo, renderer, traslado)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn avanzar_entra_desde_la_derecha_y_volver_desde_la_izquierda() {
        let (dx, velo) = aspecto(0.0, true);
        assert!((dx - DESPLAZAMIENTO).abs() < 0.01 && (velo - 1.0).abs() < 0.01);
        assert!(aspecto(0.0, false).0 < 0.0);
        let (dx, velo) = aspecto(1.0, true);
        assert!(dx.abs() < 0.01 && velo < 0.01);
    }

    #[test]
    fn cambiar_la_clave_inicia_la_entrada_y_conservarla_no() {
        let vista = |clave| -> Element<'static, ()> {
            fase(Color::BLACK, clave, iced::widget::Space::with_height(10))
        };
        let mut arbol = Tree::new(vista(1).as_widget());
        assert!(arbol.state.downcast_ref::<Estado>().inicio.is_none());
        arbol.diff(vista(1).as_widget());
        assert!(arbol.state.downcast_ref::<Estado>().inicio.is_none());
        arbol.diff(vista(2).as_widget());
        let estado = arbol.state.downcast_ref::<Estado>();
        assert_eq!(estado.clave, 2);
        assert!(estado.progreso(Instant::now()).is_some());
    }
}
