// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Visor 3D de skins: cajas texturizadas con el mapeo UV de Minecraft, la
//! capa exterior inflada medio texel y, opcionalmente, la capa (cape).
//!
//! El giro solo descansa en múltiplos de 90° (Frente, Lado derecho, Espalda,
//! Lado izquierdo): de lado a medio girar la skin se lee mal. La aplicación
//! guarda un [`Giro`] y pasa al visor el ángulo animado de cada cuadro.

use std::time::Instant;

use iced::mouse;
use iced::widget::canvas::{self, Frame, Geometry, Path};
use iced::{keyboard, Color, Point, Rectangle, Renderer, Theme};

use crate::tema::Paleta;

/// Píxeles de arrastre horizontal por cada giro de 90°.
pub const PX_POR_GIRO: f32 = 70.0;
/// Duración del giro animado entre dos caras.
pub const MS_GIRO: f32 = 280.0;

/// Nombre de la cara que mira a la cámara en un ángulo múltiplo de 90°.
pub fn nombre_cara(grados: i32) -> &'static str {
    match grados.rem_euclid(360) {
        90 => "Lado derecho",
        180 => "Espalda",
        270 => "Lado izquierdo",
        _ => "Frente",
    }
}

/// Giro de a 90° con su animación. `pasos` puede crecer sin límite; el ángulo
/// mostrado se normaliza a 0..360.
#[derive(Debug, Clone, Copy, Default)]
pub struct Giro {
    pasos: i32,
    desde: f32,
    inicio: Option<Instant>,
}

impl Giro {
    /// Ángulo de reposo en grados, de 0 a 270.
    pub fn grados(&self) -> i32 {
        (self.pasos * 90).rem_euclid(360)
    }

    /// Gira `n` pasos de 90° desde el ángulo visible ahora.
    pub fn girar(&mut self, n: i32, ahora: Instant, reducir: bool) {
        if n == 0 {
            return;
        }
        self.desde = self.angulo(ahora);
        self.pasos += n;
        self.inicio = (!reducir).then_some(ahora);
    }

    /// Va a una cara concreta (0, 90, 180 o 270) por el camino más corto.
    pub fn ir(&mut self, grados: i32, ahora: Instant, reducir: bool) {
        let delta = (grados.rem_euclid(360) - self.grados()).rem_euclid(360);
        let n = match delta {
            0 => 0,
            90 => 1,
            180 => 2,
            _ => -1,
        };
        self.girar(n, ahora, reducir);
    }

    /// Ángulo animado, en grados sin normalizar.
    pub fn angulo(&self, ahora: Instant) -> f32 {
        let fin = (self.pasos * 90) as f32;
        let Some(inicio) = self.inicio else {
            return fin;
        };
        let t = (ahora.saturating_duration_since(inicio).as_secs_f32() * 1000.0 / MS_GIRO).min(1.0);
        // Salida exponencial: arranca rápido y se asienta sin rebote.
        let k = if t >= 1.0 {
            1.0
        } else {
            1.0 - 2f32.powf(-10.0 * t)
        };
        self.desde + (fin - self.desde) * k
    }

    pub fn animando(&self, ahora: Instant) -> bool {
        self.inicio
            .is_some_and(|i| ahora.saturating_duration_since(i).as_secs_f32() * 1000.0 < MS_GIRO)
    }
}

/// Lo que dibuja el visor.
pub struct Visor<'a, M> {
    pub paleta: Paleta,
    /// RGBA de 64 × 64.
    pub textura: Option<&'a [u8]>,
    /// RGBA de la capa (cape), 64 × 32.
    pub capa: Option<&'a [u8]>,
    pub delgado: bool,
    /// Sombrero, chaqueta, mangas y pantalones.
    pub capas: [bool; 4],
    /// Ángulo en grados (de [`Giro::angulo`]).
    pub angulo: f32,
    /// Vista ortogonal de frente exacto en lugar de perspectiva.
    pub plano: bool,
    /// Encuadre de cabeza y hombros al doble.
    pub busto: bool,
    /// Fase de la caminata en segundos; `None` quieto.
    pub caminar: Option<f32>,
    /// Suelo de líneas de grabado bajo el cuerpo.
    pub suelo: bool,
    /// Contorno de zona de soltar.
    pub soltando: bool,
    pub al_girar: Box<dyn Fn(i32) -> M + 'a>,
}

#[derive(Default)]
pub struct Arrastre {
    origen: Option<f32>,
}

impl<'a, M> canvas::Program<M> for Visor<'a, M> {
    type State = Arrastre;

    fn update(
        &self,
        estado: &mut Arrastre,
        evento: canvas::Event,
        limites: Rectangle,
        cursor: mouse::Cursor,
    ) -> (iced::event::Status, Option<M>) {
        use iced::event::Status;
        match evento {
            canvas::Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                if let Some(pos) = cursor.position_in(limites) {
                    estado.origen = Some(pos.x);
                    return (Status::Captured, None);
                }
            }
            canvas::Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                if let (Some(origen), Some(pos)) = (estado.origen, cursor.position_in(limites)) {
                    let pasos = ((pos.x - origen) / PX_POR_GIRO).trunc() as i32;
                    if pasos != 0 {
                        estado.origen = Some(origen + pasos as f32 * PX_POR_GIRO);
                        // Arrastrar a la derecha gira el cuerpo hacia su izquierda.
                        return (Status::Captured, Some((self.al_girar)(-pasos.signum())));
                    }
                }
            }
            canvas::Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                estado.origen = None;
            }
            canvas::Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(tecla),
                ..
            }) if cursor.is_over(limites) => {
                let n = match tecla {
                    keyboard::key::Named::ArrowLeft => -1,
                    keyboard::key::Named::ArrowRight => 1,
                    _ => 0,
                };
                if n != 0 {
                    return (Status::Captured, Some((self.al_girar)(n)));
                }
            }
            _ => {}
        }
        (Status::Ignored, None)
    }

    fn draw(
        &self,
        _: &Arrastre,
        renderer: &Renderer,
        _: &Theme,
        limites: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<Geometry> {
        let p = self.paleta;
        let mut f = Frame::new(renderer, limites.size());
        f.fill_rectangle(Point::ORIGIN, limites.size(), p.surface_sunken);
        let suelo_alto = if self.suelo {
            (limites.height * 0.16).clamp(32.0, 72.0)
        } else {
            0.0
        };
        if self.suelo {
            let mut y = limites.height - suelo_alto;
            while y < limites.height {
                f.fill_rectangle(
                    Point::new(0.0, y),
                    iced::Size::new(limites.width, 1.0),
                    p.border,
                );
                y += 8.0;
            }
        }
        dibujar(
            &mut f,
            self,
            limites.width,
            limites.height - suelo_alto * 0.5,
        );
        if self.soltando {
            let (a, b) = (limites.width, limites.height);
            for (x, y, w, h) in [
                (0.0, 0.0, a, 2.0),
                (0.0, b - 2.0, a, 2.0),
                (0.0, 0.0, 2.0, b),
                (a - 2.0, 0.0, 2.0, b),
            ] {
                f.fill_rectangle(Point::new(x, y), iced::Size::new(w, h), p.text);
            }
        }
        vec![f.into_geometry()]
    }

    fn mouse_interaction(
        &self,
        estado: &Arrastre,
        limites: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        if estado.origen.is_some() {
            mouse::Interaction::Grabbing
        } else if cursor.is_over(limites) {
            mouse::Interaction::Grab
        } else {
            mouse::Interaction::default()
        }
    }
}

/// Skin de ejemplo para la maqueta y las capturas: piel, camisa, pantalón y
/// cara simple, con cuatro combinaciones de color. RGBA de 64 × 64.
pub fn muestra(variante: u8) -> Vec<u8> {
    let tonos: [[u32; 3]; 4] = [
        [0xc69c7a, 0x2f6f6a, 0x2d3a5c],
        [0x8d5b3f, 0x8a3b2a, 0x3b3b38],
        [0xe0b896, 0x4c4c8f, 0x5a4632],
        [0xb07a55, 0x6b6b2f, 0x24324a],
    ];
    let [piel, camisa, pantalon] = tonos[usize::from(variante) % 4];
    let mut rgba = vec![0u8; 64 * 64 * 4];
    let mut pintar = |x0: usize, y0: usize, w: usize, h: usize, c: u32| {
        for y in y0..y0 + h {
            for x in x0..x0 + w {
                let i = (y * 64 + x) * 4;
                rgba[i..i + 4].copy_from_slice(&[(c >> 16) as u8, (c >> 8) as u8, c as u8, 255]);
            }
        }
    };
    pintar(0, 0, 32, 16, piel);
    pintar(8, 0, 16, 8, 0x3a2a1e);
    pintar(0, 8, 32, 2, 0x3a2a1e);
    pintar(9, 12, 2, 1, 0xffffff);
    pintar(13, 12, 2, 1, 0xffffff);
    pintar(10, 12, 1, 1, 0x2a3a6a);
    pintar(13, 12, 1, 1, 0x2a3a6a);
    pintar(11, 14, 2, 1, 0x7a4a3a);
    pintar(16, 16, 24, 16, camisa);
    pintar(40, 16, 16, 16, camisa);
    pintar(44, 28, 8, 4, piel);
    pintar(32, 48, 16, 16, camisa);
    pintar(36, 60, 8, 4, piel);
    pintar(0, 16, 16, 16, pantalon);
    pintar(16, 48, 16, 16, pantalon);
    pintar(4, 28, 8, 4, 0x2a2420);
    pintar(20, 60, 8, 4, 0x2a2420);
    rgba
}

// ------------------------------------------------------------------ modelo

type V3 = [f32; 3];

/// Una cara de un texel ya coloreada, en coordenadas del modelo.
struct Quad {
    esquinas: [V3; 4],
    normal: V3,
    color: Color,
    /// Pieza a la que pertenece, para aplicar su pivote de caminata.
    pieza: Pieza,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Pieza {
    Fija,
    BrazoDerecho,
    BrazoIzquierdo,
    PiernaDerecha,
    PiernaIzquierda,
}

/// Caja con su origen UV en una textura de `ancho` × `alto` texels.
struct Caja {
    min: V3,
    tam: V3,
    uv: (usize, usize),
    inflar: f32,
    exterior: bool,
    pieza: Pieza,
}

fn texel(rgba: &[u8], ancho: usize, u: usize, v: usize) -> Option<Color> {
    let i = (v * ancho + u) * 4;
    let px = rgba.get(i..i + 4)?;
    Some(Color::from_rgba8(
        px[0],
        px[1],
        px[2],
        f32::from(px[3]) / 255.0,
    ))
}

/// Genera los quads visibles de una caja. Las caras siguen la red de
/// Minecraft: lado derecho, frente, lado izquierdo y espalda en la fila
/// inferior; arriba y abajo en la superior. El frente mira a +z.
fn quads_caja(c: &Caja, rgba: &[u8], ancho_tex: usize, salida: &mut Vec<Quad>) {
    let [w, h, d] = c.tam;
    let (wi, hi, di) = (w as usize, h as usize, d as usize);
    let (u0, v0) = c.uv;
    let i = c.inflar;
    let (x0, y0, z0) = (c.min[0] - i, c.min[1] - i, c.min[2] - i);
    let (sx, sy, sz) = ((w + 2.0 * i) / w, (h + 2.0 * i) / h, (d + 2.0 * i) / d);
    let mut cara =
        |n: V3, cols: usize, filas: usize, uv: (usize, usize), punto: &dyn Fn(f32, f32) -> V3| {
            for fy in 0..filas {
                for fx in 0..cols {
                    let Some(mut color) = texel(rgba, ancho_tex, uv.0 + fx, uv.1 + fy) else {
                        continue;
                    };
                    if c.exterior {
                        if color.a < 0.5 {
                            continue;
                        }
                    } else {
                        color.a = 1.0;
                    }
                    color.a = 1.0;
                    let (a, b) = (fx as f32, fy as f32);
                    salida.push(Quad {
                        esquinas: [
                            punto(a, b),
                            punto(a + 1.0, b),
                            punto(a + 1.0, b + 1.0),
                            punto(a, b + 1.0),
                        ],
                        normal: n,
                        color,
                        pieza: c.pieza,
                    });
                }
            }
        };
    let top = y0 + h * sy;
    // Frente (+z): u crece con x, v hacia abajo.
    cara([0.0, 0.0, 1.0], wi, hi, (u0 + di, v0 + di), &|a, b| {
        [x0 + a * sx, top - b * sy, z0 + d * sz]
    });
    // Espalda (-z): u crece hacia -x.
    cara(
        [0.0, 0.0, -1.0],
        wi,
        hi,
        (u0 + 2 * di + wi, v0 + di),
        &|a, b| [x0 + (w - a) * sx, top - b * sy, z0],
    );
    // Lado derecho del personaje (-x): u crece hacia el frente.
    cara([-1.0, 0.0, 0.0], di, hi, (u0, v0 + di), &|a, b| {
        [x0, top - b * sy, z0 + a * sz]
    });
    // Lado izquierdo (+x): u crece hacia la espalda.
    cara([1.0, 0.0, 0.0], di, hi, (u0 + di + wi, v0 + di), &|a, b| {
        [x0 + w * sx, top - b * sy, z0 + (d - a) * sz]
    });
    // Arriba: v crece hacia el frente.
    cara([0.0, 1.0, 0.0], wi, di, (u0 + di, v0), &|a, b| {
        [x0 + a * sx, top, z0 + b * sz]
    });
    // Abajo.
    cara([0.0, -1.0, 0.0], wi, di, (u0 + di + wi, v0), &|a, b| {
        [x0 + a * sx, y0, z0 + b * sz]
    });
}

/// Cajas del cuerpo en texels, con los pies en y = 0.
fn cajas(delgado: bool, capas: [bool; 4]) -> Vec<Caja> {
    let b = if delgado { 3.0 } else { 4.0 };
    let base = |min, tam, uv, pieza| Caja {
        min,
        tam,
        uv,
        inflar: 0.0,
        exterior: false,
        pieza,
    };
    let ext = |min, tam, uv, inflar, pieza| Caja {
        min,
        tam,
        uv,
        inflar,
        exterior: true,
        pieza,
    };
    let mut v = vec![
        base([-4.0, 24.0, -4.0], [8.0, 8.0, 8.0], (0, 0), Pieza::Fija),
        base([-4.0, 12.0, -2.0], [8.0, 12.0, 4.0], (16, 16), Pieza::Fija),
        base(
            [-4.0 - b, 12.0, -2.0],
            [b, 12.0, 4.0],
            (40, 16),
            Pieza::BrazoDerecho,
        ),
        base(
            [4.0, 12.0, -2.0],
            [b, 12.0, 4.0],
            (32, 48),
            Pieza::BrazoIzquierdo,
        ),
        base(
            [-4.0, 0.0, -2.0],
            [4.0, 12.0, 4.0],
            (0, 16),
            Pieza::PiernaDerecha,
        ),
        base(
            [0.0, 0.0, -2.0],
            [4.0, 12.0, 4.0],
            (16, 48),
            Pieza::PiernaIzquierda,
        ),
    ];
    if capas[0] {
        v.push(ext(
            [-4.0, 24.0, -4.0],
            [8.0, 8.0, 8.0],
            (32, 0),
            0.5,
            Pieza::Fija,
        ));
    }
    if capas[1] {
        v.push(ext(
            [-4.0, 12.0, -2.0],
            [8.0, 12.0, 4.0],
            (16, 32),
            0.25,
            Pieza::Fija,
        ));
    }
    if capas[2] {
        v.push(ext(
            [-4.0 - b, 12.0, -2.0],
            [b, 12.0, 4.0],
            (40, 32),
            0.25,
            Pieza::BrazoDerecho,
        ));
        v.push(ext(
            [4.0, 12.0, -2.0],
            [b, 12.0, 4.0],
            (48, 48),
            0.25,
            Pieza::BrazoIzquierdo,
        ));
    }
    if capas[3] {
        v.push(ext(
            [-4.0, 0.0, -2.0],
            [4.0, 12.0, 4.0],
            (0, 32),
            0.25,
            Pieza::PiernaDerecha,
        ));
        v.push(ext(
            [0.0, 0.0, -2.0],
            [4.0, 12.0, 4.0],
            (0, 48),
            0.25,
            Pieza::PiernaIzquierda,
        ));
    }
    v
}

/// Quads del modelo completo. Sin textura usa una silueta neutra.
fn modelo(v: &Visor<'_, impl Sized>) -> Vec<Quad> {
    let mut quads = Vec::with_capacity(4096);
    let neutra;
    let rgba = match v.textura.filter(|t| t.len() == 64 * 64 * 4) {
        Some(t) => t,
        None => {
            let c = v.paleta.border_strong;
            let px = [
                (c.r * 255.0) as u8,
                (c.g * 255.0) as u8,
                (c.b * 255.0) as u8,
                255,
            ];
            neutra = px.repeat(64 * 64);
            &neutra
        }
    };
    let capas = if v.textura.is_some() {
        v.capas
    } else {
        [false; 4]
    };
    for c in cajas(v.delgado, capas) {
        quads_caja(&c, rgba, 64, &mut quads);
    }
    if let Some(capa) = v.capa.filter(|c| c.len() == 64 * 32 * 4) {
        // La capa cuelga detrás del torso; su cara exterior mira a -z.
        let caja = Caja {
            min: [-5.0, 8.0, -3.0],
            tam: [10.0, 16.0, 1.0],
            uv: (0, 0),
            inflar: 0.0,
            exterior: false,
            pieza: Pieza::Fija,
        };
        quads_caja(&caja, capa, 64, &mut quads);
    }
    quads
}

fn rotar_x(p: V3, centro: V3, a: f32) -> V3 {
    let (s, c) = a.sin_cos();
    let (y, z) = (p[1] - centro[1], p[2] - centro[2]);
    [p[0], centro[1] + y * c - z * s, centro[2] + y * s + z * c]
}

fn dibujar(f: &mut Frame, v: &Visor<'_, impl Sized>, ancho: f32, alto: f32) {
    let quads = modelo(v);
    let ang = v.angulo.to_radians();
    let (sa, ca) = ang.sin_cos();
    // Cámara apenas por encima en 3D; de frente exacto en plano.
    let inclinacion: f32 = if v.plano { 0.0 } else { 10f32.to_radians() };
    let (si, ci) = inclinacion.sin_cos();
    let vaiven = v
        .caminar
        .map_or(0.0, |t| (t * std::f32::consts::TAU / 1.1).sin() * 0.55);
    let (centro_y, alto_modelo) = if v.busto { (24.0, 16.0) } else { (16.0, 34.0) };
    let escala = (alto * 0.82 / alto_modelo).min(ancho * 0.82 / 22.0);
    let distancia = 90.0;
    let (cx, cy) = (ancho / 2.0, alto / 2.0);
    let transformar = |p: V3, pieza: Pieza| -> V3 {
        let p = match pieza {
            Pieza::BrazoDerecho | Pieza::PiernaIzquierda => {
                let pivote = if pieza == Pieza::BrazoDerecho {
                    [0.0, 22.0, 0.0]
                } else {
                    [0.0, 12.0, 0.0]
                };
                rotar_x(p, pivote, vaiven)
            }
            Pieza::BrazoIzquierdo | Pieza::PiernaDerecha => {
                let pivote = if pieza == Pieza::BrazoIzquierdo {
                    [0.0, 22.0, 0.0]
                } else {
                    [0.0, 12.0, 0.0]
                };
                rotar_x(p, pivote, -vaiven)
            }
            Pieza::Fija => p,
        };
        // Giro sobre el eje vertical: a 90° el lado derecho mira a la cámara.
        let (x, y, z) = (
            p[0] * ca + p[2] * sa,
            p[1] - centro_y,
            -p[0] * sa + p[2] * ca,
        );
        [x, y * ci - z * si, y * si + z * ci]
    };
    let proyectar = |q: V3| -> (Point, f32) {
        let k = if v.plano {
            1.0
        } else {
            distancia / (distancia - q[2])
        };
        (
            Point::new(cx + q[0] * escala * k, cy - q[1] * escala * k),
            q[2],
        )
    };
    let mut visibles: Vec<([Point; 4], f32, Color)> = Vec::with_capacity(quads.len() / 2);
    for q in &quads {
        let n = transformar([q.normal[0], q.normal[1], q.normal[2]], Pieza::Fija);
        // La normal no se traslada: se quita el desplazamiento del centro.
        let origen = transformar([0.0, 0.0, 0.0], Pieza::Fija);
        let mut n = [n[0] - origen[0], n[1] - origen[1], n[2] - origen[2]];
        if q.pieza != Pieza::Fija && vaiven != 0.0 {
            let signo = if matches!(q.pieza, Pieza::BrazoDerecho | Pieza::PiernaIzquierda) {
                1.0
            } else {
                -1.0
            };
            let r = rotar_x(q.normal, [0.0; 3], vaiven * signo);
            let t = transformar(r, Pieza::Fija);
            n = [t[0] - origen[0], t[1] - origen[1], t[2] - origen[2]];
        }
        let esquinas = q.esquinas.map(|e| transformar(e, q.pieza));
        let centro = [
            (esquinas[0][0] + esquinas[2][0]) / 2.0,
            (esquinas[0][1] + esquinas[2][1]) / 2.0,
            (esquinas[0][2] + esquinas[2][2]) / 2.0,
        ];
        // Cara trasera: mira hacia el lado contrario de la cámara.
        let hacia_camara = if v.plano {
            n[2]
        } else {
            n[0] * -centro[0] + n[1] * -centro[1] + n[2] * (distancia - centro[2])
        };
        if hacia_camara <= 0.0 {
            continue;
        }
        // Luz desde arriba y de frente: los lados quedan más oscuros.
        let luz = (0.62 + 0.38 * (n[2].max(0.0) * 0.7 + n[1].max(0.0))).min(1.0);
        let c = q.color;
        let color = Color::from_rgb(c.r * luz, c.g * luz, c.b * luz);
        let pts = esquinas.map(|e| proyectar(e).0);
        visibles.push((pts, centro[2], color));
    }
    // De atrás hacia delante.
    visibles.sort_by(|a, b| a.1.total_cmp(&b.1));
    for (pts, _, color) in visibles {
        // Se agranda cada texel una fracción para que no queden costuras.
        let mx = (pts[0].x + pts[2].x) / 2.0;
        let my = (pts[0].y + pts[2].y) / 2.0;
        let crecer = |p: Point| Point::new(mx + (p.x - mx) * 1.06, my + (p.y - my) * 1.06);
        let camino = Path::new(|b| {
            b.move_to(crecer(pts[0]));
            b.line_to(crecer(pts[1]));
            b.line_to(crecer(pts[2]));
            b.line_to(crecer(pts[3]));
            b.close();
        });
        f.fill(&camino, color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_giro_descansa_en_multiplos_de_90() {
        let t = Instant::now();
        let mut g = Giro::default();
        g.girar(1, t, true);
        assert_eq!(g.grados(), 90);
        assert_eq!(nombre_cara(g.grados()), "Lado derecho");
        g.girar(-2, t, true);
        assert_eq!(g.grados(), 270);
        g.ir(180, t, true);
        assert_eq!(g.grados(), 180);
        assert_eq!(nombre_cara(180), "Espalda");
    }

    #[test]
    fn la_animacion_termina_en_el_angulo_de_reposo() {
        let t = Instant::now();
        let mut g = Giro::default();
        g.girar(1, t, false);
        assert!(g.animando(t));
        let fin = t + std::time::Duration::from_millis(MS_GIRO as u64 + 10);
        assert!(!g.animando(fin));
        assert_eq!(g.angulo(fin), 90.0);
        assert!(g.angulo(t + std::time::Duration::from_millis(60)) > 0.0);
    }

    #[test]
    fn el_frente_de_la_cabeza_usa_su_region_de_la_textura() {
        // Textura con el frente de la cabeza (8..16, 8..16) en rojo.
        let mut rgba = vec![0u8; 64 * 64 * 4];
        for v in 8..16 {
            for u in 8..16 {
                let i = (v * 64 + u) * 4;
                rgba[i..i + 4].copy_from_slice(&[255, 0, 0, 255]);
            }
        }
        let caja = Caja {
            min: [-4.0, 24.0, -4.0],
            tam: [8.0; 3],
            uv: (0, 0),
            inflar: 0.0,
            exterior: false,
            pieza: Pieza::Fija,
        };
        let mut quads = Vec::new();
        quads_caja(&caja, &rgba, 64, &mut quads);
        let frente: Vec<_> = quads
            .iter()
            .filter(|q| q.normal == [0.0, 0.0, 1.0])
            .collect();
        assert_eq!(frente.len(), 64);
        assert!(frente.iter().all(|q| q.color.r == 1.0 && q.color.g == 0.0));
        assert!(quads
            .iter()
            .filter(|q| q.normal != [0.0, 0.0, 1.0])
            .all(|q| q.color.r == 0.0));
    }

    #[test]
    fn la_capa_exterior_omite_texels_transparentes() {
        let rgba = vec![0u8; 64 * 64 * 4];
        let caja = Caja {
            min: [-4.0, 24.0, -4.0],
            tam: [8.0; 3],
            uv: (32, 0),
            inflar: 0.5,
            exterior: true,
            pieza: Pieza::Fija,
        };
        let mut quads = Vec::new();
        quads_caja(&caja, &rgba, 64, &mut quads);
        assert!(quads.is_empty());
    }
}
