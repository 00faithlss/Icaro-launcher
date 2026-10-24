// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Grabados y encuadres de las secciones de Ícaro, preparados para los temas
//! Piedra y Tinta.


use iced::widget::image::Handle;
use iced::widget::{column, container, row, stack, text, Space};
use iced::{Alignment, Element, Length, Padding};

pub use crate::imagenes::suscripcion;

use crate::estilo;
use crate::fuentes;
use crate::tema::{borde, espacio, texto, Modo, Paleta};

macro_rules! laminas {
    ($($variante:ident => $archivo:literal $(@ $unica:ident)?),* $(,)?) => {
        /// Láminas empaquetadas con el programa.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum Lamina { $($variante),* }

        #[cfg(test)]
        pub(crate) const TODAS: &[Lamina] = &[$(Lamina::$variante),*];

        impl Lamina {
            /// Píxeles preparados en segundo plano y compartidos entre vistas.
            pub fn imagen(self, modo: Modo) -> Handle {
                crate::imagenes::imagen(self, modo)
            }

            pub(crate) fn bytes(self, modo: Modo) -> &'static [u8] {
                match (self, modo) {
                    $(
                        (Lamina::$variante, Modo::Piedra) => include_bytes!(concat!(env!("OUT_DIR"), "/imagenes/", $archivo, "-piedra.webp")),
                        (Lamina::$variante, Modo::Tinta) => include_bytes!(concat!(env!("OUT_DIR"), "/imagenes/", $archivo, "-tinta.webp")),
                    )*
                }
            }

            fn previa_bytes(self, modo: Modo) -> &'static [u8] {
                match (self, modo) {
                    $(
                        (Lamina::$variante, Modo::Piedra) => include_bytes!(concat!(env!("OUT_DIR"), "/imagenes/", $archivo, "-piedra.rgba")),
                        (Lamina::$variante, Modo::Tinta) => include_bytes!(concat!(env!("OUT_DIR"), "/imagenes/", $archivo, "-tinta.rgba")),
                    )*
                }
            }

            pub(crate) fn previa(self, modo: Modo) -> Handle {
                let bytes = self.previa_bytes(modo);
                let invertir = bytes == crate::derivadas::INVERTIDA;
                let bytes = if invertir || bytes == crate::derivadas::IGUAL {
                    self.previa_bytes(Modo::Piedra)
                } else {
                    bytes
                };
                let w = u32::from_le_bytes(bytes[..4].try_into().unwrap());
                let h = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
                if invertir {
                    let mut pixeles = bytes[8..].to_vec();
                    crate::imagenes::invertir(&mut pixeles);
                    return Handle::from_rgba(w, h, pixeles);
                }
                Handle::from_rgba(w, h, iced::advanced::image::Bytes::from_static(&bytes[8..]))
            }

            /// Tinta que repite Piedra comparte sus píxeles y su textura.
            pub(crate) fn modo_propio(self, modo: Modo) -> Modo {
                if modo == Modo::Tinta && self.bytes(modo) == crate::derivadas::IGUAL {
                    Modo::Piedra
                } else {
                    modo
                }
            }

            /// Anticipa las bandas y los grabados de las pantallas vacías.
            pub fn precargar(modo: Modo) {
                for lamina in [Self::FaetonPaisaje, Self::CaidaCielo,
                    Self::TivoliCiudad, Self::HidraCabezas, Self::MercurioFriso,
                    Self::Mano, Self::Orbita, Self::Alas, Self::Forja,
                    Self::Umbral, Self::JeronimoSanto] {
                    let _ = lamina.imagen(modo);
                }
            }
        }
    };
}

include!("catalogo_laminas.rs");

/// Alto de la banda de sección.
pub const ALTO_BANDA: f32 = 170.0;

/// Banda: lámina a sangre con el título monumental sobre un bloque de fondo
/// que corta la imagen.
pub fn banda<'a, M: 'a>(
    p: Paleta,
    lamina: Lamina,
    titulo: &str,
    conteo: Option<usize>,
) -> Element<'a, M> {
    banda_con(p, lamina, titulo, conteo, ALTO_BANDA, texto::DISPLAY_XL)
}

/// Banda con alto y tamaño de título propios, como la del editor de
/// instancia (150 px con título `display`).
pub fn banda_con<'a, M: 'a>(
    p: Paleta,
    lamina: Lamina,
    titulo: &str,
    conteo: Option<usize>,
    alto: f32,
    tam_titulo: (f32, f32),
) -> Element<'a, M> {
    let dato = conteo.map_or(DatoBanda::Ninguno, DatoBanda::Conteo);
    banda_completa(p, lamina, None, titulo, dato, alto, tam_titulo)
}

/// Dato opcional dentro del rótulo de la banda.
#[derive(Debug, Clone, Copy)]
pub enum DatoBanda<'a> {
    Ninguno,
    /// Cantidad de elementos de la sección, en `mono`.
    Conteo(usize),
    /// Resumen breve en `body-sm`, como "2 activas · 6,1 MB/s".
    Nota(&'a str),
}

/// Banda con todas sus opciones: encuadre propio (`posicion`, como
/// `object-position`) y dato en el rótulo.
///
/// El rótulo es un bloque `bg` anclado abajo a la izquierda, con reglas de
/// 2 px arriba y a la derecha; su texto queda alineado con el margen del
/// contenido. Una regla de 2 px cierra la banda por debajo.
pub fn banda_completa<'a, M: 'a>(
    p: Paleta,
    lamina: Lamina,
    posicion: Option<(f32, f32)>,
    titulo: &str,
    dato: DatoBanda<'_>,
    alto: f32,
    tam_titulo: (f32, f32),
) -> Element<'a, M> {
    let titulo = titulo.to_uppercase();
    let (conteo, nota) = match dato {
        DatoBanda::Ninguno => (None, None),
        DatoBanda::Conteo(n) => (Some(n), None),
        DatoBanda::Nota(t) => (None, Some(t.to_owned())),
    };
    container(iced::widget::responsive(move |tam| {
        let imagen = match posicion {
            Some(pos) => grabado_posicion(p, lamina, Length::Fill, alto, pos),
            None => grabado_prensa(p, lamina, Length::Fill, alto),
        };
        let grabado = stack![imagen, crate::barrido::barrido(p, Length::Fill, Length::Fixed(alto))];

        // El rótulo no se parte: si la nota no cabe junto al título, pasa a un
        // bloque pequeño arriba a la derecha, como el crédito de la lámina.
        let ancho_titulo = titulo.chars().count() as f32 * tam_titulo.0 * 0.92;
        let ancho_nota = nota.as_ref().map_or(0.0, |t| t.chars().count() as f32 * 6.6);
        let cabe = espacio::S12 + ancho_titulo + espacio::S4 + ancho_nota + espacio::S8 + 2.0 <= tam.width;

        let mut contenido = row![container(
            text(titulo.clone())
                .font(fuentes::DISPLAY)
                .size(tam_titulo.0)
                .line_height(iced::widget::text::LineHeight::Absolute(
                    tam_titulo.1.into()
                ))
                .wrapping(iced::widget::text::Wrapping::None)
                .color(p.text)
        )
        // La caja del texto llega 6 px por debajo del rótulo: las mayúsculas casi
        // tocan la regla de abajo, como en el sistema.
        .height(tam_titulo.1 - 6.0)
        .clip(true)]
        .spacing(espacio::S4)
        .align_y(Alignment::End);
        let mut aparte: Option<Element<'_, M>> = conteo.map(|n| {
            text(n.to_string())
                .font(fuentes::MONO)
                .size(texto::MONO.0)
                .color(p.text_muted)
                .into()
        });
        let mut credito: Option<Element<'_, M>> = None;
        if let Some(t) = &nota {
            let texto_nota = text(t.clone())
                .size(texto::BODY_SM.0)
                .wrapping(iced::widget::text::Wrapping::None)
                .color(p.text_muted);
            if cabe {
                aparte = Some(texto_nota.into());
            } else {
                credito = Some(
                    container(
                        container(texto_nota.font(fuentes::MONO).size(texto::MONO_SM.0))
                            .padding(Padding::from([espacio::S1, espacio::S2]))
                            .style(estilo::bloque(p.bg)),
                    )
                    .align_x(iced::alignment::Horizontal::Right)
                    .width(Length::Fill)
                    .padding(Padding { top: espacio::S3, right: espacio::S4, ..Padding::ZERO })
                    .into(),
                );
            }
        }
        if let Some(a) = aparte {
            contenido = contenido.push(container(a).padding(Padding { bottom: 4.0, ..Padding::ZERO }));
        }
        let bloque = container(contenido)
            .padding(Padding {
                top: espacio::S5,
                right: espacio::S8,
                bottom: 0.0,
                left: espacio::S12,
            })
            .style(estilo::bloque(p.bg));
        // Las reglas de arriba y de la derecha son el relleno de un marco de tinta.
        let rotulo = container(bloque)
            .padding(Padding { top: borde::MEDIO, right: borde::MEDIO, ..Padding::ZERO })
            .style(estilo::bloque(p.text));
        let capa_rotulo = column![Space::with_height(Length::Fill), rotulo].height(alto);
        let capa_regla = column![
            Space::with_height(Length::Fill),
            container(Space::new(Length::Fill, Length::Fixed(borde::MEDIO)))
                .style(estilo::bloque(p.text))
        ]
        .height(alto);
        let mut capas = stack![grabado, capa_regla, capa_rotulo].height(alto);
        if let Some(c) = credito {
            capas = capas.push(c);
        }
        capas.into()
    }))
    .width(Length::Fill)
    .height(Length::Fixed(alto))
    .into()
}


/// Grabado que llena su caja recortando el sobrante, centrado y sin
/// deformarlo (como `ContentFit::Cover`).
///
/// El `Image` de iced con `Cover` abre una capa de recorte con sus propios
/// límites, que ignora el recorte del `scrollable`, y la imagen se sale por
/// arriba al desplazar. Este widget dibuja la imagen en una capa limitada a
/// la parte visible.
impl Lamina {
    /// Punto de la imagen que debe quedar visible al recortarla, de 0 a 1
    /// (como `object-position`): evita cortar rostros y figuras.
    pub const fn foco(self) -> (f32, f32) {
        // Centrado, como antes: no se mueve el encuadre de ninguna lámina.
        (0.5, 0.5)
    }
}

/// Grabado que llena su caja recortando el sobrante, sin deformarlo.
///
/// El `Image` de iced con `Cover` abre una capa de recorte con sus propios
/// límites, que ignora el recorte del `scrollable`, y la imagen se sale por
/// arriba al desplazar. Este widget dibuja la imagen en una capa limitada a
/// la parte visible, centrada en el foco de la lámina.
pub fn grabado<'a, M: 'a>(
    p: Paleta,
    lamina: Lamina,
    ancho: impl Into<Length>,
    alto: impl Into<Length>,
) -> Element<'a, M> {
    Element::new(Grabado {
        handle: lamina.imagen(p.modo).clone(),
        ancho: ancho.into(),
        alto: alto.into(),
        foco: lamina.foco(),
        entero: false,
        posicion: None,
    })
}

/// Grabado sin animación de revelado.
pub fn grabado_prensa<'a, M: 'a>(
    p: Paleta, lamina: Lamina,
    ancho: impl Into<Length>, alto: impl Into<Length>,
) -> Element<'a, M> {
    grabado(p, lamina, ancho, alto)
}

/// Grabado entero y centrado, sin recortar (figuras sobre fondo transparente y
/// medallones). Conserva la proporción de la imagen.
pub fn grabado_entero<'a, M: 'a>(
    p: Paleta,
    lamina: Lamina,
    ancho: impl Into<Length>,
    alto: impl Into<Length>,
) -> Element<'a, M> {
    Element::new(Grabado {
        handle: lamina.imagen(p.modo).clone(),
        ancho: ancho.into(),
        alto: alto.into(),
        foco: (0.5, 0.5),
        entero: true,
        posicion: None,
    })
}

/// Grabado estático, sin acercamiento al pasar el puntero.
pub fn grabado_zoom<'a, M: 'a>(
    p: Paleta, lamina: Lamina,
    ancho: impl Into<Length>, alto: impl Into<Length>,
) -> Element<'a, M> {
    grabado(p, lamina, ancho, alto)
}

/// Igual que [`grabado`], con el encuadre de `object-position`: `posicion`
/// es la fracción (de 0 a 1) de la imagen que se alinea con esa misma
/// fracción de la caja. Sirve para no cortar rostros en cuadros pequeños.
pub fn grabado_posicion<'a, M: 'a>(
    p: Paleta,
    lamina: Lamina,
    ancho: impl Into<Length>,
    alto: impl Into<Length>,
    posicion: (f32, f32),
) -> Element<'a, M> {
    Element::new(Grabado {
        handle: lamina.imagen(p.modo).clone(),
        ancho: ancho.into(),
        alto: alto.into(),
        foco: lamina.foco(),
        entero: false,
        posicion: Some(posicion),
    })
}

struct Grabado {
    handle: Handle,
    ancho: Length,
    alto: Length,
    foco: (f32, f32),
    /// Se muestra entera, sin recortar, centrada en la caja.
    entero: bool,
    /// Como `object-position` de CSS: fracción de la imagen que se alinea con
    /// la misma fracción de la caja.
    posicion: Option<(f32, f32)>,
}

impl<M> iced::advanced::Widget<M, iced::Theme, iced::Renderer> for Grabado {
    fn size(&self) -> iced::Size<Length> {
        iced::Size::new(self.ancho, self.alto)
    }

    fn layout(
        &self,
        _arbol: &mut iced::advanced::widget::Tree,
        renderer: &iced::Renderer,
        limites: &iced::advanced::layout::Limits,
    ) -> iced::advanced::layout::Node {
        if self.entero {
            // La caja toma la proporción de la imagen, para que el marco la
            // abrace en vez de dejar bandas vacías.
            use iced::advanced::image::Renderer as _;
            let tam = renderer.measure_image(&self.handle);
            let max = limites.max();
            if tam.width > 0 && tam.height > 0 && max.width.is_finite() {
                let proporcion = tam.height as f32 / tam.width as f32;
                let mut w = max.width;
                let mut h = w * proporcion;
                if max.height.is_finite() && h > max.height {
                    h = max.height;
                    w = h / proporcion;
                }
                return iced::advanced::layout::Node::new(iced::Size::new(w, h));
            }
        }
        iced::advanced::layout::atomic(limites, self.ancho, self.alto)
    }

    fn draw(
        &self,
        _arbol: &iced::advanced::widget::Tree,
        renderer: &mut iced::Renderer,
        _tema: &iced::Theme,
        _estilo: &iced::advanced::renderer::Style,
        layout: iced::advanced::Layout<'_>,
        _cursor: iced::advanced::mouse::Cursor,
        visible: &iced::Rectangle,
    ) {
        use iced::advanced::image::Renderer as _;
        use iced::advanced::Renderer as _;

        let caja = layout.bounds();
        let Some(recorte) = caja.intersection(visible) else {
            return;
        };
        let tam = renderer.measure_image(&self.handle);
        if tam.width == 0 || tam.height == 0 || caja.width <= 0.0 || caja.height <= 0.0 {
            return;
        }
        let (iw, ih) = (tam.width as f32, tam.height as f32);
        let escala = if self.entero {
            (caja.width / iw).min(caja.height / ih)
        } else {
            (caja.width / iw).max(caja.height / ih)
        };
        // Cover debe cubrir ambos ejes incluso si la multiplicación pierde
        // unas fracciones de píxel. De lo contrario clamp recibe min > max.
        let (w, h) = if self.entero {
            (iw * escala, ih * escala)
        } else {
            ((iw * escala).max(caja.width), (ih * escala).max(caja.height))
        };
        // El foco de la imagen queda lo más cerca posible del centro de la caja.
        let (x, y) = if self.entero {
            (caja.center_x() - w / 2.0, caja.center_y() - h / 2.0)
        } else if let Some((px, py)) = self.posicion {
            (caja.x + px * (caja.width - w), caja.y + py * (caja.height - h))
        } else {
            (
                posicion_foco(caja.x, caja.width, w, self.foco.0),
                posicion_foco(caja.y, caja.height, h, self.foco.1),
            )
        };
        let dibujo = iced::Rectangle { x, y, width: w, height: h };
        renderer.with_layer(recorte, |renderer| {
            renderer.draw_image(
                iced::advanced::image::Image {
                    handle: self.handle.clone(),
                    filter_method: iced::advanced::image::FilterMethod::Linear,
                    rotation: iced::Radians(0.0),
                    opacity: 1.0,
                    snap: true,
                },
                dibujo,
            );
        });
    }
}

fn posicion_foco(origen: f32, caja: f32, imagen: f32, foco: f32) -> f32 {
    let minimo = origen + (caja - imagen).min(0.0);
    (origen + caja / 2.0 - foco * imagen).clamp(minimo, origen)
}

#[cfg(test)]
mod pruebas_encuadre {
    use super::posicion_foco;

    #[test]
    fn redondeo_de_cover_no_cierra_el_launcher() {
        // La galería del panel daba min=424.66675 y max=424.66672.
        let origen = 424.66672;
        let caja = 181.33334;
        let imagen = caja - 0.00003;
        assert_eq!(posicion_foco(origen, caja, imagen, 0.5), origen);
    }

    #[test]
    fn foco_siempre_permanece_dentro_del_recorte() {
        for origen in [0.0, 424.66672, 1230.5] {
            for caja in [0.0, 120.0, 181.33334, 952.0] {
                for imagen in [caja, caja + 0.00003, caja * 3.0] {
                    for foco in [0.0, 0.2, 0.5, 1.0] {
                        let x = posicion_foco(origen, caja, imagen, foco);
                        assert!(x >= origen + (caja - imagen).min(0.0));
                        assert!(x <= origen);
                    }
                }
            }
        }
    }
}
