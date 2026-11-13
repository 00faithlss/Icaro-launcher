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

/// Como [`desplazable`], con un identificador para llevar a la vista una parte
/// del contenido con el mismo movimiento suave (ver [`llevar_a`]).
pub fn desplazable_con_id<'a, M: 'a>(
    contenido: impl Into<Element<'a, M>>,
    id: &'static str,
) -> Element<'a, M> {
    let mut e = suave_widget(contenido, None);
    e.id = Some(iced::advanced::widget::Id::new(id));
    Element::new(e)
}

fn suave<'a, M: 'a>(contenido: impl Into<Element<'a, M>>, clave: Option<u64>) -> Element<'a, M> {
    Element::new(suave_widget(contenido, clave))
}

/// Como [`desplazable`] con alto fijo, avisando la posición en cada cuadro:
/// sirve a listas virtualizadas que dibujan solo las filas visibles.
/// `clave` vuelve arriba con el mismo movimiento al cambiar (otro filtro o búsqueda).
pub fn desplazable_con_avisos<'a, M: 'a>(
    contenido: impl Into<Element<'a, M>>,
    alto: f32,
    id: &'static str,
    clave: u64,
    al_desplazar: impl Fn(scrollable::Viewport) -> M + 'a,
) -> Element<'a, M> {
    let lista = scrollable::Scrollable::with_direction(
        contenido,
        Direction::Vertical(Scrollbar::new().width(0).scroller_width(0).margin(0)),
    )
    .id(scrollable::Id::new(id))
    .height(alto)
    .on_scroll(al_desplazar);
    Element::new(Suave {
        contenido: lista.into(),
        clave: Some(clave),
        avisos: true,
        id: None,
    })
}

fn suave_widget<'a, M: 'a>(
    contenido: impl Into<Element<'a, M>>,
    clave: Option<u64>,
) -> Suave<'a, M> {
    let lista = scrollable::Scrollable::with_direction(
        contenido,
        Direction::Vertical(Scrollbar::new().width(0).scroller_width(0).margin(0)),
    )
    .height(Length::Fill);
    Suave {
        contenido: lista.into(),
        clave,
        avisos: false,
        id: None,
    }
}

/// Lleva a la vista el contenedor `objetivo` dentro del desplazable `id`, con
/// `margen` píxeles por encima. Mide su posición y mueve el objetivo de la
/// animación: la posición real lo persigue como con la rueda. `avisar` se
/// emite al terminar para que la ventana vuelva a dibujarse y arranque el
/// movimiento.
pub fn llevar_a<M: Clone + Send + 'static>(
    id: &'static str,
    objetivo: impl Into<std::borrow::Cow<'static, str>>,
    margen: f32,
    avisar: M,
) -> iced::Task<M> {
    use iced::advanced::widget::{operate, operation::Outcome, operation::Scrollable, Id};
    struct Medir {
        desplazable: Id,
        objetivo: Id,
        margen: f32,
        dentro: bool,
        inicio: Option<f32>,
        y: Option<f32>,
    }
    impl Operation<()> for Medir {
        fn container(
            &mut self,
            id: Option<&Id>,
            bounds: Rectangle,
            hijos: &mut dyn FnMut(&mut dyn Operation<()>),
        ) {
            let entra = id == Some(&self.desplazable);
            if entra {
                self.dentro = true;
            }
            if self.dentro && id == Some(&self.objetivo) {
                self.y = Some(bounds.y);
            }
            hijos(self);
            if entra {
                self.dentro = false;
            }
        }
        fn scrollable(
            &mut self,
            _: &mut dyn Scrollable,
            _: Option<&Id>,
            _: Rectangle,
            contenido: Rectangle,
            _: Vector,
        ) {
            if self.dentro && self.inicio.is_none() {
                self.inicio = Some(contenido.y);
            }
        }
        fn finish(&self) -> Outcome<()> {
            match self.y.zip(self.inicio) {
                Some((y, inicio)) => Outcome::Chain(Box::new(Mover {
                    desplazable: self.desplazable.clone(),
                    valor: (y - inicio - self.margen).max(0.0),
                })),
                None => Outcome::None,
            }
        }
    }
    struct Mover {
        desplazable: Id,
        valor: f32,
    }
    impl Operation<()> for Mover {
        fn container(
            &mut self,
            _: Option<&Id>,
            _: Rectangle,
            hijos: &mut dyn FnMut(&mut dyn Operation<()>),
        ) {
            hijos(self);
        }
        fn custom(&mut self, estado: &mut dyn std::any::Any, id: Option<&Id>) {
            if id == Some(&self.desplazable) {
                if let Some(estado) = estado.downcast_mut::<Estado>() {
                    estado.objetivo = self.valor;
                    estado.ultimo = None;
                }
            }
        }
        fn finish(&self) -> Outcome<()> {
            Outcome::Some(())
        }
    }
    operate(Medir {
        desplazable: Id::new(id),
        objetivo: Id::new(objetivo),
        margen,
        dentro: false,
        inicio: None,
        y: None,
    })
    .map(move |()| avisar.clone())
}

thread_local! {
    /// Un desplazable de dentro ya se quedó con la muesca de rueda en curso.
    static RUEDA_TOMADA: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Lee o fija el desplazamiento del `scrollable` propio (el primero que
/// aparece al recorrer el contenido); no entra en los anidados.
struct Primero {
    fijar: Option<Vector>,
    leido: Option<Vector>,
    hecho: bool,
}

impl Operation for Primero {
    fn container(&mut self, _: Option<&iced::advanced::widget::Id>, _: Rectangle, hijos: &mut dyn FnMut(&mut dyn Operation)) {
        if !self.hecho {
            hijos(self);
        }
    }
    fn scrollable(
        &mut self,
        estado: &mut dyn iced::advanced::widget::operation::Scrollable,
        _: Option<&iced::advanced::widget::Id>,
        _: Rectangle,
        _: Rectangle,
        traslado: Vector,
    ) {
        if self.hecho {
            return;
        }
        self.hecho = true;
        self.leido = Some(traslado);
        if let Some(v) = self.fijar {
            estado.scroll_to(scrollable::AbsoluteOffset { x: v.x, y: v.y });
        }
    }
}

fn desplazamiento<M>(contenido: &Element<'_, M>, arbol: &mut Tree, layout: Layout<'_>, renderer: &Renderer) -> Option<Vector> {
    let mut op = Primero { fijar: None, leido: None, hecho: false };
    contenido.as_widget().operate(arbol, layout, renderer, &mut op);
    op.leido
}

fn fijar_desplazamiento<M>(contenido: &Element<'_, M>, arbol: &mut Tree, layout: Layout<'_>, renderer: &Renderer, a: Option<Vector>) {
    let Some(v) = a else { return };
    let mut op = Primero { fijar: Some(v), leido: None, hecho: false };
    contenido.as_widget().operate(arbol, layout, renderer, &mut op);
}

/// Lleva arriba, sin animación, todos los desplazables de la ventana. La
/// aplicación lo pide al cambiar de sección, página, pestaña o ajuste, para
/// que lo nuevo no aparezca a media altura.
pub fn volver_arriba<M: Send + 'static>() -> iced::Task<M> {
    struct Arriba;
    impl Operation for Arriba {
        fn container(&mut self, _: Option<&iced::advanced::widget::Id>, _: Rectangle, hijos: &mut dyn FnMut(&mut dyn Operation)) {
            hijos(self);
        }
        fn scrollable(
            &mut self,
            estado: &mut dyn iced::advanced::widget::operation::Scrollable,
            _: Option<&iced::advanced::widget::Id>,
            _: Rectangle,
            _: Rectangle,
            _: Vector,
        ) {
            estado.snap_to(scrollable::RelativeOffset::START);
        }
        fn custom(&mut self, estado: &mut dyn std::any::Any, _: Option<&iced::advanced::widget::Id>) {
            if let Some(e) = estado.downcast_mut::<Estado>() {
                e.objetivo = 0.0;
                e.actual = 0.0;
                e.ultimo = None;
            }
        }
    }
    iced::advanced::widget::operate(Arriba).discard()
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
    /// Lista virtualizada con `on_scroll`: se mueve con eventos de rueda.
    avisos: bool,
    /// Permite que [`llevar_a`] encuentre este desplazable.
    id: Option<iced::advanced::widget::Id>,
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
        let Some(hijo) = layout.children().next() else {
            return;
        };
        // [`volver_arriba`] encuentra el estado de todos, tengan o no id.
        operacion.custom(arbol.state.downcast_mut::<Estado>(), self.id.as_ref());
        if self.id.is_none() {
            self.contenido
                .as_widget()
                .operate(&mut arbol.children[0], hijo, renderer, operacion);
            return;
        }
        let contenido = &self.contenido;
        let hijos = &mut arbol.children;
        operacion.container(self.id.as_ref(), layout.bounds(), &mut |op| {
            contenido
                .as_widget()
                .operate(&mut hijos[0], hijo, renderer, op);
        });
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
                // Primero el contenido: un desplazable anidado (el árbol de
                // Archivos dentro de su página) se mueve antes que este. Si
                // nadie lo toma, el `scrollable` interno se mueve solo y ese
                // salto se deshace para animarlo aquí.
                let antes = desplazamiento(&self.contenido, &mut arbol.children[0], hijo, renderer);
                RUEDA_TOMADA.with(|t| t.set(false));
                let _ = self.contenido.as_widget_mut().on_event(
                    &mut arbol.children[0],
                    evento.clone(),
                    hijo,
                    cursor,
                    renderer,
                    portapapeles,
                    shell,
                    visible,
                );
                let despues = desplazamiento(&self.contenido, &mut arbol.children[0], hijo, renderer);
                if RUEDA_TOMADA.with(|t| t.get()) {
                    // Un desplazable de dentro se movió.
                    if despues != antes {
                        fijar_desplazamiento(&self.contenido, &mut arbol.children[0], hijo, renderer, antes);
                    }
                    return event::Status::Captured;
                }
                if despues != antes {
                    fijar_desplazamiento(&self.contenido, &mut arbol.children[0], hijo, renderer, antes);
                }
                let estado = arbol.state.downcast_mut::<Estado>();
                // Sin animación en curso, la posición real manda: si algo más
                // movió la lista (otro contenido, teclado, volver arriba), se
                // parte de ahí y no de una posición vieja.
                if estado.ultimo.is_none() && !self.avisos {
                    if let Some(real) = antes {
                        if (real.y - estado.actual).abs() > 1.0 {
                            estado.actual = real.y;
                            estado.objetivo = real.y;
                        }
                    }
                }
                estado.limitar(maximo);
                // En el borde no se queda con la rueda: sigue el desplazable de
                // fuera, para no dejar la página fija bajo un panel.
                let sube = dy > 0.0;
                if (sube && estado.objetivo <= 0.0) || (!sube && estado.objetivo >= maximo) {
                    return event::Status::Ignored;
                }
                estado.objetivo = (estado.objetivo - dy).clamp(0.0, maximo);
                RUEDA_TOMADA.with(|t| t.set(true));
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
                    if self.avisos {
                        // La lista virtualizada necesita su aviso `on_scroll`,
                        // que solo llega con un evento de rueda.
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
                    } else {
                        // La posición va directo al desplazable propio: un
                        // evento de rueda en el centro podía quedárselo un
                        // desplazable de dentro, y este quedaba desfasado y sin
                        // poder volver arriba.
                        let y = estado.actual;
                        fijar_desplazamiento(&self.contenido, &mut arbol.children[0], hijo, renderer, Some(Vector::new(0.0, y)));
                    }
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
