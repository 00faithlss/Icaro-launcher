// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! ArranqueSuave: cinco transiciones tranquilas al pulsar Jugar, como
//! alternativa al Umbral. Ocupan el tiempo real del arranque con fundidos y
//! aperturas lentas, sin rebotes. Todas muestran la misma información: línea
//! «Preparando», nombre de la instancia, una frase, una barra que avanza y el
//! registro de arranque. Con reducir movimiento pasan a un fundido de 120 ms.
//!
//! AjusteArranque usa el mismo dibujo en miniaturas de 16:9 que se repiten
//! cada 3,6 s.

use iced::widget::canvas::{self, Canvas, Frame, Geometry, Path, Text};
use iced::widget::{button, container, opaque, stack, text};
use iced::{mouse, Color, Element, Length, Point, Rectangle, Renderer, Size, Theme};

use crate::fuentes;
use crate::movimiento::{Curva, Preferencia};
use crate::tema::{Modo, Paleta};

/// Qué se ve al pulsar Jugar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Estilo {
    /// La transición cinematográfica de siempre.
    Umbral,
    #[default]
    Marea,
    Horizonte,
    Tinta,
    Bloques,
    Respiro,
}

impl Estilo {
    pub const TODOS: [Estilo; 6] = [
        Estilo::Umbral,
        Estilo::Marea,
        Estilo::Horizonte,
        Estilo::Tinta,
        Estilo::Bloques,
        Estilo::Respiro,
    ];
    pub const SUAVES: [Estilo; 5] = [
        Estilo::Marea,
        Estilo::Horizonte,
        Estilo::Tinta,
        Estilo::Bloques,
        Estilo::Respiro,
    ];
    pub const fn nombre(self) -> &'static str {
        match self {
            Estilo::Umbral => "Umbral",
            Estilo::Marea => "Marea",
            Estilo::Horizonte => "Horizonte",
            Estilo::Tinta => "Tinta",
            Estilo::Bloques => "Bloques",
            Estilo::Respiro => "Respiro",
        }
    }
    pub const fn etiqueta(self) -> &'static str {
        match self {
            Estilo::Umbral => "Cinematográfica",
            Estilo::Respiro => "Mínima",
            _ => "Suave",
        }
    }
    pub const fn descripcion(self) -> &'static str {
        match self {
            Estilo::Umbral => "La luz del arco llena la ventana. La de siempre.",
            Estilo::Marea => "La ventana se funde despacio y el nombre sube un poco.",
            Estilo::Horizonte => "Una línea cruza al centro y se abre hacia arriba y abajo.",
            Estilo::Tinta => "Una mancha nace del botón Jugar y cubre todo.",
            Estilo::Bloques => "Bloques que aparecen en diagonal, uno tras otro.",
            Estilo::Respiro => "Pantalla quieta con el icono de la instancia respirando.",
        }
    }
}

/// Tono de la capa: el del tema de Ícaro o siempre claro u oscuro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tono {
    #[default]
    Tema,
    Claro,
    Oscuro,
}

impl Tono {
    /// Paleta de la capa. Forzar el otro tono usa ese tema con el mismo metal.
    pub fn paleta(self, p: Paleta, metal: crate::tema::Metal) -> Paleta {
        let modo = match self {
            Tono::Tema => return p,
            Tono::Claro => Modo::Piedra,
            Tono::Oscuro => Modo::Tinta,
        };
        if modo == p.modo {
            return p;
        }
        modo.paleta().con_metal(metal)
    }
    pub const TODOS: [Tono; 3] = [Tono::Tema, Tono::Claro, Tono::Oscuro];
    pub const fn nombre(self) -> &'static str {
        match self {
            Tono::Tema => "Según el tema",
            Tono::Claro => "Claro",
            Tono::Oscuro => "Oscuro",
        }
    }
}

/// Lo que muestra la capa.
#[derive(Debug, Clone, Default)]
pub struct Contenido {
    /// «Preparando · 1.21.4 · Fabric 0.16.9».
    pub preparando: String,
    pub nombre: String,
    pub frase: Option<String>,
    /// Hasta cuatro líneas; vacío si el registro está apagado.
    pub registro: Vec<String>,
    /// Patrón 8 × 8 del icono de la instancia (0 = vacío, 1–4 = tonos).
    pub icono: Option<[u8; 64]>,
}

/// Curvas del diseño (`suave`, `entra` y la apertura del Horizonte).
fn suave(t: f32) -> f32 {
    Curva::Salida.aplicar(t)
}
fn entra(t: f32) -> f32 {
    Curva::Salida.aplicar(t)
}
fn apertura(t: f32) -> f32 {
    // cubic-bezier(.65,0,.35,1): entra y sale despacio.
    let t = t.clamp(0.0, 1.0);
    if t < 0.5 {
        4.0 * t * t * t
    } else {
        1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
    }
}
fn tramo(ms: f32, desde: f32, dura: f32) -> f32 {
    if dura <= 0.0 {
        return if ms >= desde { 1.0 } else { 0.0 };
    }
    ((ms - desde) / dura).clamp(0.0, 1.0)
}

const COLS: usize = 16;
const FILAS: usize = 10;

/// Cuándo empieza a aparecer el contenido en cada estilo.
pub fn inicio_contenido(estilo: Estilo, reducir: bool) -> f32 {
    if reducir {
        return 120.0;
    }
    match estilo {
        Estilo::Umbral => 1220.0,
        Estilo::Marea => 420.0,
        Estilo::Horizonte => 1000.0,
        Estilo::Tinta => 650.0,
        Estilo::Bloques => (COLS + FILAS) as f32 * 32.0 + 120.0,
        Estilo::Respiro => 300.0,
    }
}

/// Momento en que aparece «Volver» (y la barra ya llegó al final).
pub fn listo(estilo: Estilo, reducir: bool) -> f32 {
    inicio_contenido(estilo, reducir) + 4500.0
}

struct Capa {
    p: Paleta,
    estilo: Estilo,
    ms: f32,
    reducir: bool,
    /// Opacidad de toda la capa (se retira con fundido de 200 ms).
    opacidad: f32,
    contenido: Contenido,
    /// Miniatura: dibuja un launcher mínimo debajo y sin textos largos.
    mini: bool,
}

fn con_alfa(c: Color, a: f32) -> Color {
    Color { a: c.a * a, ..c }
}

impl Capa {
    /// Dibuja el fondo del estilo a pantalla completa.
    fn fondo(&self, frame: &mut Frame, tam: Size) {
        let p = self.p;
        let a = self.opacidad;
        let fondo = if self.estilo == Estilo::Respiro { p.surface_sunken } else { p.bg };
        let ms = self.ms;
        if self.reducir {
            let t = tramo(ms, 0.0, 120.0);
            frame.fill_rectangle(Point::ORIGIN, tam, con_alfa(fondo, t * a));
            return;
        }
        match self.estilo {
            Estilo::Umbral => {
                // Miniatura del Umbral: la luz del arco se expande desde arriba.
                let t = entra(tramo(ms, 220.0, 700.0));
                let solida = entra(tramo(ms, 320.0, 900.0));
                let arco = Rectangle::new(
                    Point::new(tam.width * 0.45, tam.height * 0.2),
                    Size::new(tam.width * 0.1, tam.height * 0.24),
                );
                let interp = |k: f32| {
                    Rectangle::new(
                        Point::new(arco.x * (1.0 - k), arco.y * (1.0 - k)),
                        Size::new(
                            arco.width + (tam.width - arco.width) * k,
                            arco.height + (tam.height - arco.height) * k,
                        ),
                    )
                };
                let lineas = interp(t);
                let mut y = lineas.y;
                while y < lineas.y + lineas.height {
                    frame.fill_rectangle(
                        Point::new(lineas.x, y),
                        Size::new(lineas.width, 1.0),
                        con_alfa(p.fulgor, a),
                    );
                    y += 3.0;
                }
                let s = interp(solida);
                if solida > 0.0 {
                    frame.fill_rectangle(s.position(), s.size(), con_alfa(p.fulgor, a));
                }
            }
            Estilo::Marea => {
                let t = suave(tramo(ms, 0.0, 900.0));
                frame.fill_rectangle(Point::ORIGIN, tam, con_alfa(fondo, t * a));
            }
            Estilo::Horizonte => {
                // La línea cruza al centro y luego se apaga mientras se abre.
                let l = tramo(ms, 0.0, 900.0);
                let ancho = suave((l / 0.6).min(1.0));
                let alfa_linea = if l < 0.6 { 1.0 } else { 1.0 - (l - 0.6) / 0.4 };
                let abrir = apertura(tramo(ms, 480.0, 800.0));
                let alto = tam.height * abrir;
                if alto > 0.0 {
                    frame.fill_rectangle(
                        Point::new(0.0, (tam.height - alto) / 2.0),
                        Size::new(tam.width, alto),
                        con_alfa(fondo, a),
                    );
                }
                if alfa_linea > 0.0 {
                    let w = tam.width * ancho;
                    frame.fill_rectangle(
                        Point::new((tam.width - w) / 2.0, tam.height / 2.0 - 0.5),
                        Size::new(w, 1.0),
                        con_alfa(p.text, alfa_linea * a),
                    );
                }
            }
            Estilo::Tinta => {
                // Nace del botón Jugar, abajo a la derecha.
                let origen = Point::new(tam.width - tam.width.min(1000.0) * 0.12, tam.height - 40.0_f32.min(tam.height * 0.12));
                let lejos = origen.x.max(tam.width - origen.x).hypot(origen.y.max(tam.height - origen.y));
                let r = lejos * entra(tramo(ms, 0.0, 1100.0));
                if r > 0.0 {
                    frame.fill(&Path::circle(origen, r), con_alfa(fondo, a));
                }
            }
            Estilo::Bloques => {
                let (cw, ch) = (tam.width / COLS as f32, tam.height / FILAS as f32);
                let completo = tramo(ms, (COLS + FILAS) as f32 * 32.0 + 420.0, 1.0);
                if completo >= 1.0 {
                    frame.fill_rectangle(Point::ORIGIN, tam, con_alfa(fondo, a));
                    return;
                }
                for y in 0..FILAS {
                    for x in 0..COLS {
                        let t = suave(tramo(ms, (x + y) as f32 * 32.0, 420.0));
                        if t <= 0.0 {
                            continue;
                        }
                        let escala = 0.6 + 0.42 * t;
                        let (w, h) = (cw * escala, ch * escala);
                        frame.fill_rectangle(
                            Point::new(x as f32 * cw + (cw - w) / 2.0, y as f32 * ch + (ch - h) / 2.0),
                            Size::new(w + 0.5, h + 0.5),
                            con_alfa(fondo, t * a),
                        );
                    }
                }
            }
            Estilo::Respiro => {
                let t = suave(tramo(ms, 0.0, 700.0));
                frame.fill_rectangle(Point::ORIGIN, tam, con_alfa(fondo, t * a));
            }
        }
    }

    /// El contenido común: sube 12 px mientras aparece.
    fn contenido(&self, frame: &mut Frame, tam: Size) {
        let p = if self.estilo == Estilo::Umbral { self.umbral_paleta() } else { self.p };
        let inicio = inicio_contenido(self.estilo, self.reducir);
        let t = if self.reducir {
            tramo(self.ms, 0.0, 120.0)
        } else {
            suave(tramo(self.ms, inicio, 700.0))
        };
        if t <= 0.0 {
            return;
        }
        let a = t * self.opacidad;
        let dy = (1.0 - t) * 12.0 * if self.mini { 0.4 } else { 1.0 };
        let escala: f32 = if self.mini { 0.32 } else { 1.0 };
        let ancho = (tam.width * 0.62).min(720.0 * escala.max(0.5)).max(tam.width * 0.5);
        let x = (tam.width - ancho) / 2.0;
        let mut y = tam.height * if self.mini { 0.28 } else { 0.3 } + dy;
        let texto = |frame: &mut Frame, s: &str, y: f32, tam_letra: f32, fuente: iced::Font, color: Color| {
            // El lienzo no corta líneas: lo que no cabe en la columna se
            // recorta con «…» para no salirse de la pantalla.
            let tam = (tam_letra * escala).max(4.0);
            let por_letra = if fuente == fuentes::MONO {
                0.72
            } else if fuente == fuentes::DISPLAY {
                0.86
            } else {
                0.56
            };
            frame.fill_text(Text {
                content: recortar(s, ancho, tam * por_letra),
                position: Point::new(x, y),
                color,
                size: tam.into(),
                font: fuente,
                ..Text::default()
            });
        };
        if self.estilo == Estilo::Respiro {
            if let Some(patron) = self.contenido.icono {
                let lado = if self.mini { 18.0 } else { 72.0 };
                let respira = if self.reducir {
                    1.0
                } else {
                    let ciclo = ((self.ms - 700.0).max(0.0) / 2200.0) % 2.0;
                    let k = if ciclo < 1.0 { ciclo } else { 2.0 - ciclo };
                    1.0 + 0.06 * (0.5 - 0.5 * (k * std::f32::consts::PI).cos())
                };
                let l = lado * respira;
                let origen = Point::new(x, y - (l - lado) / 2.0);
                let celda = l / 8.0;
                for (i, v) in patron.iter().enumerate() {
                    if *v == 0 {
                        continue;
                    }
                    let tinte = 0.35 + 0.65 * (*v as f32 / 4.0);
                    frame.fill_rectangle(
                        Point::new(origen.x + (i % 8) as f32 * celda, origen.y + (i / 8) as f32 * celda),
                        Size::new(celda + 0.3, celda + 0.3),
                        con_alfa(p.text, a * tinte),
                    );
                }
                y += lado + 16.0 * escala.max(0.4);
            }
        }
        if self.mini {
            // Miniatura: barras en lugar de texto legible.
            for (w, h, alfa) in [(0.4, 3.0, 0.6), (0.7, 8.0, 1.0), (0.5, 3.0, 0.7)] {
                frame.fill_rectangle(Point::new(x, y), Size::new(ancho * w, h), con_alfa(p.text, a * alfa));
                y += h + 5.0;
            }
        } else {
            texto(frame, &self.contenido.preparando, y, 11.0, fuentes::MONO, con_alfa(p.text_muted, a));
            y += 26.0;
            texto(frame, &self.contenido.nombre.to_uppercase(), y, 64.0, fuentes::DISPLAY, con_alfa(p.text, a));
            y += 88.0;
            if let Some(f) = &self.contenido.frase {
                texto(frame, f, y, 14.0, fuentes::CUERPO, con_alfa(p.text_muted, a));
                y += 34.0;
            }
        }
        // Barra de 2 px que se llena en unos 4 s con curva desacelerada.
        let barra = suave(tramo(self.ms, inicio + 200.0, if self.reducir { 0.0 } else { 4200.0 }));
        frame.fill_rectangle(Point::new(x, y), Size::new(ancho, 2.0), con_alfa(p.border, a));
        frame.fill_rectangle(Point::new(x, y), Size::new(ancho * barra, 2.0), con_alfa(p.text, a));
        y += 22.0 * escala.max(0.4);
        if self.mini {
            return;
        }
        for (i, linea) in self.contenido.registro.iter().take(4).enumerate() {
            let tl = if self.reducir {
                1.0
            } else {
                suave(tramo(self.ms, inicio + 500.0 + i as f32 * 650.0, 400.0))
            };
            if tl > 0.0 {
                texto(frame, linea, y, 11.0, fuentes::MONO, con_alfa(p.text_muted, a * tl));
            }
            y += 20.0;
        }
    }

    /// El texto sobre la luz del Umbral usa `on-fulgor`.
    fn umbral_paleta(&self) -> Paleta {
        Paleta {
            text: self.p.on_fulgor,
            text_muted: con_alfa(self.p.on_fulgor, 0.7),
            border: con_alfa(self.p.on_fulgor, 0.25),
            ..self.p
        }
    }

    /// Launcher mínimo de la miniatura: cabecera, pie y botón Jugar.
    fn app_minima(&self, frame: &mut Frame, tam: Size) {
        let p = self.p;
        frame.fill_rectangle(Point::ORIGIN, tam, p.surface);
        frame.fill_rectangle(Point::new(10.0, 12.0), Size::new(tam.width * 0.35, 6.0), con_alfa(p.text, 0.8));
        frame.fill_rectangle(Point::new(10.0, 24.0), Size::new(tam.width * 0.22, 3.0), con_alfa(p.text_muted, 0.6));
        let pie = tam.height - 26.0;
        frame.fill_rectangle(Point::new(0.0, pie), Size::new(tam.width, 1.0), p.border);
        frame.fill_rectangle(
            Point::new(tam.width - 58.0, pie + 6.0),
            Size::new(48.0, 14.0),
            p.accent,
        );
    }
}

impl<M> canvas::Program<M> for Capa {
    type State = ();

    fn draw(
        &self,
        _: &(),
        renderer: &Renderer,
        _: &Theme,
        limites: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let mut frame = Frame::new(renderer, limites.size());
        let tam = limites.size();
        if self.mini {
            self.app_minima(&mut frame, tam);
        }
        self.fondo(&mut frame, tam);
        self.contenido(&mut frame, tam);
        vec![frame.into_geometry()]
    }
}

/// Recorta `s` a las letras que caben en `ancho` (con `letra` de ancho medio)
/// y termina en «…» si sobró algo.
fn recortar(s: &str, ancho: f32, letra: f32) -> String {
    let caben = (ancho / letra.max(1.0)).floor().max(1.0) as usize;
    if s.chars().count() <= caben {
        return s.to_owned();
    }
    let mut corto: String = s.chars().take(caben.saturating_sub(1)).collect();
    corto.truncate(corto.trim_end().len());
    corto.push('…');
    corto
}

/// Capa a pantalla completa. `retirada` son los ms desde que Minecraft abrió
/// su ventana (o falló): la capa se funde en 200 ms. `volver` aparece cuando
/// la barra llega al final, por si el juego tarda.
#[allow(clippy::too_many_arguments)]
pub fn capa<'a, M: Clone + 'a>(
    p: Paleta,
    estilo: Estilo,
    tono: Tono,
    metal: crate::tema::Metal,
    ms: f32,
    pref: Preferencia,
    contenido: Contenido,
    retirada: Option<f32>,
    volver: M,
    cancelar: Option<M>,
) -> Element<'a, M> {
    let paleta = if estilo == Estilo::Umbral { p } else { tono.paleta(p, metal) };
    let opacidad = retirada.map_or(1.0, |r| 1.0 - (r / if pref.reducir { 120.0 } else { 200.0 }).clamp(0.0, 1.0));
    let lienzo = Canvas::new(Capa {
        p: paleta,
        estilo,
        ms,
        reducir: pref.reducir,
        opacidad,
        contenido,
        mini: false,
    })
    .width(Length::Fill)
    .height(Length::Fill);
    let mut pila = stack![lienzo];
    let enlace = |rotulo: &'static str, al_pulsar: M| -> Element<'a, M> {
        button(
            text(rotulo)
                .font(fuentes::ETIQUETA)
                .size(11)
                .color(paleta.text_muted),
        )
        .padding([6.0, 12.0])
        .on_press(al_pulsar)
        .style(move |_, estado| button::Style {
            background: matches!(estado, button::Status::Hovered | button::Status::Pressed)
                .then_some(paleta.surface_hover.into()),
            text_color: paleta.text_muted,
            ..Default::default()
        })
        .into()
    };
    // Cancelar está desde el primer momento; Volver, cuando la barra llega
    // al final por si el juego tarda.
    let mut acciones = iced::widget::row![].spacing(8);
    if retirada.is_none() {
        if let Some(c) = cancelar {
            acciones = acciones.push(enlace("Cancelar", c));
        }
        if ms >= listo(estilo, pref.reducir) {
            acciones = acciones.push(enlace("Volver a Ícaro", volver));
        }
    }
    if retirada.is_none() {
        pila = pila.push(
            container(acciones)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .align_y(iced::alignment::Vertical::Bottom)
            .padding(32),
        );
    }
    // La capa no deja pasar clics a la ventana de abajo.
    opaque(pila)
}

/// Miniatura 16:9 de AjusteArranque: repite la transición cada 3,6 s.
pub fn miniatura<'a, M: 'a>(
    p: Paleta,
    estilo: Estilo,
    tono: Tono,
    metal: crate::tema::Metal,
    ms_bucle: f32,
    reducir: bool,
    icono: [u8; 64],
) -> Element<'a, M> {
    let paleta = if estilo == Estilo::Umbral { p } else { tono.paleta(p, metal) };
    // El bucle de 3,6 s recorre la secuencia a velocidad doble para verla entera.
    let ms = if reducir { 10_000.0 } else { (ms_bucle % 3600.0) * 2.0 };
    Canvas::new(Capa {
        p: paleta,
        estilo,
        ms,
        reducir: false,
        opacidad: 1.0,
        contenido: Contenido {
            icono: Some(icono),
            ..Default::default()
        },
        mini: true,
    })
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::tema::{PIEDRA, TINTA};

    #[test]
    fn los_textos_largos_se_recortan_al_ancho_de_la_columna() {
        assert_eq!(recortar("26.3", 720.0, 7.9), "26.3");
        let linea = "[main] Found graphics adapter: AdapterInfo{vendor=NVIDIA, description='NVIDIA GeForce RTX 3050'}";
        let corta = recortar(linea, 400.0, 8.0);
        assert_eq!(corta.chars().count(), 50);
        assert!(corta.ends_with('…'));
    }

    #[test]
    fn los_tramos_y_las_curvas_quedan_entre_cero_y_uno() {
        assert_eq!(tramo(0.0, 100.0, 200.0), 0.0);
        assert_eq!(tramo(400.0, 100.0, 200.0), 1.0);
        assert!((apertura(0.5) - 0.5).abs() < 0.01);
        assert_eq!(apertura(0.0), 0.0);
        assert_eq!(apertura(1.0), 1.0);
    }

    #[test]
    fn reducir_movimiento_adelanta_el_contenido() {
        for e in Estilo::SUAVES {
            assert_eq!(inicio_contenido(e, true), 120.0);
            assert!(inicio_contenido(e, false) >= 300.0);
        }
    }

    #[test]
    fn el_tono_forzado_cambia_la_paleta_y_el_del_tema_no() {
        assert_eq!(Tono::Tema.paleta(TINTA, crate::tema::Metal::Marmol).bg, TINTA.bg);
        assert_eq!(Tono::Claro.paleta(TINTA, crate::tema::Metal::Marmol).bg, PIEDRA.bg);
        assert_eq!(Tono::Oscuro.paleta(PIEDRA, crate::tema::Metal::Marmol).bg, TINTA.bg);
    }
}
