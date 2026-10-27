// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Desplazamiento suave de la rueda, al estilo de Lenis, sin barra visible.
//!
//! La rueda mueve un objetivo y la posición real lo persigue con una
//! interpolación independiente de la tasa de cuadros. Todo el estado vive en
//! el propio widget: la aplicación solo envuelve su contenido con
//! [`desplazable`].

use std::time::Instant;

use iced::advanced::layout::{Layout, Limits, Node};
use iced::advanced::widget::{tree, Operation, Tree};
use iced::advanced::{mouse, overlay, renderer, Clipboard, Shell, Widget};
use iced::widget::scrollable::{self, Direction, Scrollbar};
use iced::window::RedrawRequest;
use iced::{event, Element, Event, Length, Rectangle, Renderer, Size, Theme, Vector};

/// Píxeles que avanza una muesca de la rueda.
const PASO_MUESCA: f32 = 110.0;
/// Rapidez con que la posición alcanza al objetivo (por segundo).
const RAPIDEZ: f32 = 9.0;
/// Distancia bajo la cual se da por alcanzado el objetivo.
const UMBRAL: f32 = 0.3;

/// Lista desplazable sin barra y con rueda suave.
pub fn desplazable<'a, M: 'a>(contenido: impl Into<Element<'a, M>>) -> Element<'a, M> {
    suave(contenido, None)
}

/// Como [`desplazable`], pero vuelve arriba con el mismo movimiento suave
/// cada vez que cambia `clave`, por ejemplo al pasar de página.
pub fn desplazable_con_clave<'a, M: 'a>(
    contenido: impl Into<Element<'a, M>>,
    clave: u64,
) -> Element<'a, M> {
    suave(contenido, Some(clave))
}

fn suave<'a, M: 'a>(contenido: impl Into<Element<'a, M>>, clave: Option<u64>) -> Element<'a, M> {
    let lista = scrollable::Scrollable::with_direction(
        contenido,
        Direction::Vertical(Scrollbar::new().width(0).scroller_width(0).margin(0)),
    )
    .height(Length::Fill);
    Element::new(Suave {
        contenido: lista.into(),
        clave,
    })
}

#[derive(Debug, Default)]
struct Estado {
    objetivo: f32,
    actual: f32,
    ultimo: Option<Instant>,
    clave: Option<u64>,
}

impl Estado {
    /// Ajusta la posición al recorrido disponible. Si el contenido se acorta,
    /// el `scrollable` limita su desplazamiento; sin este ajuste la rueda
    /// partiría de una posición que ya no existe y movería en sentido contrario.
    fn limitar(&mut self, maximo: f32) {
        self.objetivo = self.objetivo.clamp(0.0, maximo);
        self.actual = self.actual.clamp(0.0, maximo);
    }
}

/// Envoltura que convierte la rueda en un movimiento interpolado: captura la
/// rueda, mueve el objetivo y, en cada cuadro, entrega al `scrollable` el
/// tramo que corresponde como un desplazamiento en píxeles.
struct Suave<'a, M> {
    contenido: Element<'a, M>,
    clave: Option<u64>,
}

impl<'a, M> Widget<M, Theme, Renderer> for Suave<'a, M> {
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<Estado>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(Estado {
            clave: self.clave,
            ..Estado::default()
        })
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.contenido)]
    }

    fn diff(&self, arbol: &mut Tree) {
        let estado = arbol.state.downcast_mut::<Estado>();
        if self.clave.is_some() && estado.clave != self.clave {
            // El siguiente cuadro anima desde la posición actual hasta arriba.
            estado.clave = self.clave;
            estado.objetivo = 0.0;
            estado.ultimo = None;
        }
        arbol.diff_children(std::slice::from_ref(&self.contenido));
    }

    fn size(&self) -> Size<Length> {
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
        if let Some(hijo) = layout.children().next() {
            self.contenido
                .as_widget()
                .draw(&arbol.children[0], renderer, tema, estilo, hijo, cursor, visible);
        }
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
        let caja = hijo.bounds();
        // Alto del contenido dentro del scrollable (su primer hijo).
        let alto = hijo.children().next().map_or(caja.height, |c| c.bounds().height);
        let maximo = (alto - caja.height).max(0.0);

        match &evento {
            Event::Mouse(mouse::Event::WheelScrolled { delta }) if cursor.is_over(caja) => {
                let dy = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => *y * PASO_MUESCA,
                    mouse::ScrollDelta::Pixels { y, .. } => *y,
                };
                let estado = arbol.state.downcast_mut::<Estado>();
                estado.limitar(maximo);
                estado.objetivo = (estado.objetivo - dy).clamp(0.0, maximo);
                shell.request_redraw(RedrawRequest::NextFrame);
                return event::Status::Captured;
            }
            Event::Window(iced::window::Event::RedrawRequested(ahora)) => {
                let estado = arbol.state.downcast_mut::<Estado>();
                estado.limitar(maximo);
                if (estado.objetivo - estado.actual).abs() > UMBRAL {
                    let dt = estado
                        .ultimo
                        .map_or(1.0 / 60.0, |u| ahora.duration_since(u).as_secs_f32())
                        .min(0.1);
                    estado.ultimo = Some(*ahora);
                    let k = 1.0 - (-RAPIDEZ * dt).exp();
                    let mut paso = (estado.objetivo - estado.actual) * k;
                    if (estado.objetivo - estado.actual - paso).abs() <= UMBRAL {
                        paso = estado.objetivo - estado.actual;
                    }
                    estado.actual += paso;
                    let pendiente = (estado.objetivo - estado.actual).abs() > UMBRAL;
                    if !pendiente {
                        estado.ultimo = None;
                    }
                    // Un desplazamiento en píxeles hacia abajo es negativo.
                    let sintetico = Event::Mouse(mouse::Event::WheelScrolled {
                        delta: mouse::ScrollDelta::Pixels { x: 0.0, y: -paso },
                    });
                    let sobre = mouse::Cursor::Available(caja.center());
                    self.contenido.as_widget_mut().on_event(
                        &mut arbol.children[0],
                        sintetico,
                        hijo,
                        sobre,
                        renderer,
                        portapapeles,
                        shell,
                        visible,
                    );
                    if pendiente {
                        shell.request_redraw(RedrawRequest::NextFrame);
                    }
                }
            }
            _ => {}
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
mod pruebas_capas {
    use super::*;

    fn vista(con_menu: bool) -> Element<'static, ()> {
        crate::superficies::capa_flotante(
            desplazable(iced::widget::Space::with_height(1600)),
            con_menu.then(|| iced::widget::Space::new(260, 407).into()),
        )
    }

    #[test]
    fn cambiar_la_clave_vuelve_arriba_y_conservarla_no() {
        let vista = |clave| -> Element<'static, ()> {
            desplazable_con_clave(iced::widget::Space::with_height(1600), clave)
        };
        let mut arbol = Tree::new(vista(1).as_widget());
        let estado = arbol.state.downcast_mut::<Estado>();
        estado.actual = 900.0;
        estado.objetivo = 900.0;
        arbol.diff(vista(1).as_widget());
        assert_eq!(arbol.state.downcast_ref::<Estado>().objetivo, 900.0);
        arbol.diff(vista(2).as_widget());
        let estado = arbol.state.downcast_ref::<Estado>();
        // La animación parte de donde estaba hacia arriba.
        assert_eq!((estado.actual, estado.objetivo), (900.0, 0.0));
    }

    #[test]
    fn un_contenido_mas_corto_limita_la_posicion() {
        let mut estado = Estado {
            objetivo: 1200.0,
            actual: 1100.0,
            ..Estado::default()
        };
        estado.limitar(400.0);
        assert_eq!((estado.actual, estado.objetivo), (400.0, 400.0));
    }

    #[test]
    fn abrir_y_cerrar_menu_conserva_el_desplazamiento_y_la_inercia() {
        let cerrada = vista(false);
        let mut arbol = Tree::new(cerrada.as_widget());
        let estado = arbol.children[0].state.downcast_mut::<Estado>();
        estado.actual = 710.0;
        estado.objetivo = 740.0;
        estado.ultimo = Some(Instant::now());
        for con_menu in [true, true, false, true, false] {
            arbol.diff(vista(con_menu).as_widget());
            let estado = arbol.children[0].state.downcast_ref::<Estado>();
            assert_eq!(estado.actual, 710.0);
            assert_eq!(estado.objetivo, 740.0);
            assert!(estado.ultimo.is_some());
        }
    }
}
