// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Cuenta del jugador, Estudio de skin y estados de sesión.
//!
//! El cuerpo y la cabeza de la skin se dibujan en píxeles con `canvas`, a
//! escalas enteras para que no se emborronen. Mientras no haya una skin real
//! se usa el busto de mármol en grises que trae el sistema de diseño.

use iced::mouse;
use iced::widget::canvas::{self, Canvas, Frame, Geometry, Path, Stroke};
use iced::widget::{button, column, container, hover, row, stack, text, Space};
use iced::{Alignment, Background, Border, Color, Element, Length, Padding, Point, Rectangle, Renderer, Size, Theme};

use crate::estilo;
use crate::fuentes;
use crate::iconos::{icono, Icono, Tam};
use crate::laminas::{grabado_posicion, Lamina};
use crate::tema::{borde, espacio, hex, medida, texto, Paleta};

/// Estado de la sesión del jugador. Cada vista consulta el estado y dibuja
/// su variante.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EstadoSesion {
    #[default]
    Activa,
    SinConexion,
    Vencida,
    CodigoVencido,
    SinJuego,
}

impl EstadoSesion {
    /// Nombre que muestra la barra lateral.
    pub const fn nombre(self) -> &'static str {
        match self {
            EstadoSesion::Activa => "Mineral_7",
            EstadoSesion::SinConexion => "Sin cuenta",
            EstadoSesion::Vencida | EstadoSesion::CodigoVencido | EstadoSesion::SinJuego => "Forjador",
        }
    }
}

// ------------------------------------------------------------------ píxeles

const BUSTO: [&str; 8] = [
    "hhhhhhhh", "hhhhhhhh", "hssssssh", "ssssssss", "sepssPes", "sssnnsss", "ssmmmmss", "ssssssss",
];
const BARBA: [&str; 8] = [
    "hhhhhhhh", "hhhhhhhh", "hssssssh", "ssssssss", "sepssPes", "sssnnsss", "bbmmmmbb", "bbbbbbbb",
];

/// Rostro de ejemplo de la cabeza.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cara {
    Busto,
    Barba,
}

fn tono(c: char) -> Option<Color> {
    Some(match c {
        'h' => hex(0x2b2b2a),
        's' => hex(0xc9c9c4),
        'e' => hex(0xf4f4f0),
        'p' | 'P' => hex(0x111111),
        'n' => hex(0xa3a39e),
        'm' => hex(0x6e6e69),
        'b' => hex(0x4a4a47),
        'c' => hex(0x8a8a85),
        'k' => hex(0x5c5c58),
        'l' => hex(0x4a4a47),
        'z' | 't' => hex(0x2b2b2a),
        _ => return None,
    })
}

fn mezcla(a: Color, b: Color, t: f32) -> Color {
    Color::from_rgba(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a + (b.a - a.a) * t,
    )
}

/// Tono de la skin sin capa exterior.
const PIEL: char = 's';

/// Parte del cuerpo a la que pertenece un píxel, para las capas visibles.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Capa {
    Sombrero,
    Chaqueta,
    Mangas,
    Pantalones,
}

/// Rejilla de 16 × 32 del cuerpo: carácter por píxel y capa a la que pertenece.
fn rejilla_cuerpo(espalda: bool) -> Vec<Vec<Option<(char, Option<Capa>)>>> {
    let cara = if espalda { ["hhhhhhhh"; 8] } else { BUSTO };
    let mut filas = Vec::with_capacity(32);
    for y in 0..32usize {
        let mut fila = Vec::with_capacity(16);
        for x in 0..16usize {
            let celda = if y < 8 {
                (4..12).contains(&x).then(|| {
                    let c = cara[y].as_bytes()[x - 4] as char;
                    (c, (c == 'h').then_some(Capa::Sombrero))
                })
            } else if y < 20 {
                if (4..12).contains(&x) {
                    Some((if y == 16 { 'k' } else { 'c' }, Some(Capa::Chaqueta)))
                } else if y < 11 {
                    Some(('c', Some(Capa::Mangas)))
                } else {
                    Some(('s', None))
                }
            } else if (4..12).contains(&x) {
                Some((if y >= 29 { 'z' } else { 'l' }, Some(Capa::Pantalones)))
            } else {
                None
            };
            fila.push(celda);
        }
        filas.push(fila);
    }
    filas
}

fn color_celda(celda: (char, Option<Capa>), capas: [bool; 4]) -> Option<Color> {
    let (mut c, capa) = celda;
    if capa == Some(Capa::Mangas) && capas[2] {
        // La capa de mangas encendida añade el puño más oscuro.
        c = 'k';
    }
    let visible = match capa {
        Some(Capa::Sombrero) => capas[0],
        Some(Capa::Chaqueta) => capas[1],
        Some(Capa::Mangas) => true,
        Some(Capa::Pantalones) => capas[3],
        None => true,
    };
    tono(if visible { c } else { PIEL })
}

fn rellenar(f: &mut Frame, x: f32, y: f32, w: f32, h: f32, c: Color) {
    f.fill_rectangle(Point::new(x, y), Size::new(w, h), c);
}

/// Regla de 1 px alrededor de un cuadro, como la sombra de contorno del sistema.
fn contorno(f: &mut Frame, x: f32, y: f32, w: f32, h: f32, c: Color) {
    rellenar(f, x, y, w, 1.0, c);
    rellenar(f, x, y + h - 1.0, w, 1.0, c);
    rellenar(f, x, y, 1.0, h, c);
    rellenar(f, x + w - 1.0, y, 1.0, h, c);
}

/// Rayado de grabado a 135°: líneas de `grosor` px cada `periodo` px medidos
/// en perpendicular. `inicio` es dónde arranca la línea dentro del periodo.
fn rayado(f: &mut Frame, x: f32, y: f32, w: f32, h: f32, color: Color, periodo: f32, inicio: f32, grosor: f32) {
    let raiz = std::f32::consts::SQRT_2;
    let mut c = (inicio + grosor / 2.0) * raiz;
    let paso = periodo * raiz;
    let limite = w + h;
    while c < limite + paso {
        // Recta x' + y' = c en coordenadas del cuadro, recortada al cuadro.
        let (x0, y0) = if c <= h { (0.0, c) } else { (c - h, h) };
        let (x1, y1) = if c <= w { (c, 0.0) } else { (w, c - w) };
        if x0 <= w && y1 <= h {
            f.stroke(
                &Path::line(Point::new(x + x0, y + y0), Point::new(x + x1, y + y1)),
                Stroke::default().with_width(grosor).with_color(color),
            );
        }
        c += paso;
    }
}

// ---- cabeza

struct ProgCabeza {
    celdas: Option<Vec<Color>>,
    lado: f32,
    margen: f32,
    contorno: Color,
    rayas: Option<Color>,
    marca: Option<Color>,
}

impl<M> canvas::Program<M> for ProgCabeza {
    type State = ();

    fn draw(&self, _: &(), renderer: &Renderer, _: &Theme, bounds: Rectangle, _: mouse::Cursor) -> Vec<Geometry> {
        let mut f = Frame::new(renderer, bounds.size());
        let o = self.margen + 1.0;
        contorno(&mut f, self.margen, self.margen, self.lado + 2.0, self.lado + 2.0, self.contorno);
        if let Some(celdas) = &self.celdas {
            let c = self.lado / 8.0;
            for (i, color) in celdas.iter().enumerate() {
                let (x, y) = ((i % 8) as f32, (i / 8) as f32);
                rellenar(&mut f, o + x * c, o + y * c, c, c, *color);
            }
        }
        if let Some(r) = self.rayas {
            let (l, m) = (self.lado, o);
            rayado(&mut f, m, m, l, l, r, 4.0, 3.0, 1.0);
        }
        if let Some(m) = self.marca {
            // Contorno de 2 px separado 3 px del cuadro.
            let (x, y, l) = (o - 5.0, o - 5.0, self.lado + 10.0);
            rellenar(&mut f, x, y, l, 2.0, m);
            rellenar(&mut f, x, y + l - 2.0, l, 2.0, m);
            rellenar(&mut f, x, y, 2.0, l, m);
            rellenar(&mut f, x + l - 2.0, y, 2.0, l, m);
        }
        vec![f.into_geometry()]
    }
}

fn lienzo<'a, M: 'a>(prog: impl canvas::Program<M> + 'a, ancho: f32, alto: f32) -> Element<'a, M> {
    Canvas::new(prog).width(Length::Fixed(ancho)).height(Length::Fixed(alto)).into()
}

fn cabeza_con<'a, M: 'a>(
    p: Paleta,
    celdas: Option<Vec<Color>>,
    rayas: Option<Color>,
    lado: f32,
    margen: f32,
    marca: Option<Color>,
) -> Element<'a, M> {
    let tam = lado + 2.0 + 2.0 * margen;
    lienzo(ProgCabeza { celdas, lado, margen, contorno: p.border, rayas, marca }, tam, tam)
}

/// Cabeza de skin de 8 × 8 píxeles escalada sin suavizado. El lado es un
/// múltiplo de 8 (24, 32, 48 o 64); la regla de 1 px queda fuera del lado.
pub fn cabeza<'a, M: 'a>(p: Paleta, cara: Cara, lado: f32) -> Element<'a, M> {
    cabeza_con(p, Some(celdas_cara(cara)), None, lado, 0.0, None)
}

fn celdas_cara(cara: Cara) -> Vec<Color> {
    let filas = if cara == Cara::Busto { BUSTO } else { BARBA };
    filas
        .iter()
        .flat_map(|f| f.chars())
        .map(|c| tono(c).unwrap_or(Color::TRANSPARENT))
        .collect()
}

/// Cabeza rayada de las cuentas sin conexión.
pub fn cabeza_vacia<'a, M: 'a>(p: Paleta, lado: f32) -> Element<'a, M> {
    cabeza_con(p, None, Some(p.border_strong), lado, 0.0, None)
}

fn fnv1a(s: &str) -> u32 {
    let mut h: u32 = 2_166_136_261;
    for b in s.encode_utf16() {
        h ^= u32::from(b);
        h = h.wrapping_mul(16_777_619);
    }
    h
}

/// Patrón simétrico de 8 × 8 en cuatro tonos de tinta derivado del nombre
/// (FNV-1a y xorshift), el mismo del icono procedural de instancia.
fn celdas_nombre(p: Paleta, nombre: &str) -> Vec<Color> {
    let tonos = [p.surface_sunken, p.border, p.border_strong, p.text_muted, p.text];
    let mut r = fnv1a(nombre);
    if r == 0 {
        r = 1;
    }
    let mut celdas = [0usize; 64];
    for y in 0..8 {
        for x in 0..4 {
            r ^= r << 13;
            r ^= r >> 17;
            r ^= r << 5;
            let mut t = match r % 9 {
                0..=2 => 0,
                3..=4 => 1,
                5..=6 => 2,
                7 => 3,
                _ => 4,
            };
            if (x == 0 || x == 3) && (y == 0 || y == 7) {
                t = 0;
            }
            celdas[y * 8 + x] = t;
            celdas[y * 8 + 7 - x] = t;
        }
    }
    celdas.iter().map(|&t| tonos[t]).collect()
}

/// Cabeza del historial: una skin guardada, identificada por su nombre. Lleva
/// 5 px de margen por lado para dibujar el contorno de la que está en uso.
pub fn cabeza_historial<'a, M: 'a>(p: Paleta, nombre: &str, lado: f32, en_uso: bool) -> Element<'a, M> {
    let celdas = match nombre {
        "busto" => celdas_cara(Cara::Busto),
        "barba" => celdas_cara(Cara::Barba),
        otro => celdas_nombre(p, otro),
    };
    cabeza_con(p, Some(celdas), None, lado, 5.0, en_uso.then_some(p.text))
}

// ---- cuerpo

struct ProgCuerpo {
    escala: f32,
    espalda: bool,
    textura: Option<std::sync::Arc<[u8]>>,
    delgado: bool,
}

impl<M> canvas::Program<M> for ProgCuerpo {
    type State = ();

    fn draw(&self, _: &(), renderer: &Renderer, _: &Theme, bounds: Rectangle, _: mouse::Cursor) -> Vec<Geometry> {
        let mut f = Frame::new(renderer, bounds.size());
        dibujar_cuerpo(&mut f, 0.0, 0.0, self.escala, if self.espalda { 4 } else { 0 }, if self.textura.is_some() {[true;4]} else {[true,true,false,true]}, self.delgado, self.textura.as_deref());
        vec![f.into_geometry()]
    }
}

/// Cuerpo completo de 16 × 32 píxeles, de frente o de espalda.
pub fn cuerpo_skin_local<'a,M:'a>(p:Paleta,espalda:bool,escala:f32,textura:Option<&[u8]>,delgada:bool,exterior:bool)->Element<'a,M> {
    struct Vista {p:Paleta,espalda:bool,escala:f32,textura:Option<Vec<u8>>,delgada:bool,exterior:bool}
    impl<M> canvas::Program<M> for Vista {
        type State=();
        fn draw(&self,_:&(),renderer:&Renderer,_:&Theme,bounds:Rectangle,_:mouse::Cursor)->Vec<Geometry> {
            let mut f=Frame::new(renderer,bounds.size());
            if let Some(bytes)=&self.textura {
                let grid=pixeles_skin_local(bytes,self.espalda,self.delgada,self.exterior);
                for (y,fila) in grid.iter().enumerate(){for(x,color)in fila.iter().enumerate(){if let Some(color)=color{rellenar(&mut f,x as f32*self.escala,y as f32*self.escala,self.escala,self.escala,*color);}}}
            }else{
                for (x,y,w,h) in [(4,0,8,8),(0,8,16,12),(4,20,8,12)] {
                    for py in y* self.escala as usize..(y+h)*self.escala as usize {for px in x*self.escala as usize..(x+w)*self.escala as usize {
                        if (px+py)%6==0 {rellenar(&mut f,px as f32,py as f32,1.,1.,self.p.border_strong);}
                    }}
                }
            }
            vec![f.into_geometry()]
        }
    }
    lienzo(Vista{p,espalda,escala,textura:textura.map(ToOwned::to_owned),delgada,exterior},16.*escala,32.*escala)
}

fn pixeles_skin_local(bytes:&[u8],espalda:bool,delgada:bool,exterior:bool)->Vec<Vec<Option<Color>>> {
    let mut grid=pixeles_skin(bytes,espalda,delgada,[exterior;4]);
    if espalda {
        let brazo=if delgada {3}else{4};
        for fila in &mut grid[8..20] {for x in 0..brazo {fila.swap(4-brazo+x,12+x);}}
        for fila in &mut grid[20..32] {for x in 0..4 {fila.swap(4+x,8+x);}}
    }
    grid
}

/// Cuerpo completo de 16 × 32 píxeles, de frente o de espalda.
pub fn cuerpo<'a, M: 'a>(espalda: bool, escala: f32) -> Element<'a, M> {
    cuerpo_textura(espalda, escala, None, false)
}
fn cuerpo_textura<'a, M: 'a>(espalda: bool, escala: f32, textura: Option<&[u8]>, delgado: bool) -> Element<'a, M> {
    lienzo(ProgCuerpo { escala, espalda, textura: textura.map(Into::into), delgado }, 16.0 * escala, 32.0 * escala)
}
fn pixeles_skin(bytes: &[u8], espalda: bool, delgado: bool, capas: [bool;4]) -> Vec<Vec<Option<Color>>> {
    let mut grid = vec![vec![None;16];32];
    if bytes.len()!=64*64*4 {return grid;}
    let arm = if delgado {3} else {4};
    // Destino, tamaño, UV base (frente/espalda), UV exterior y capa visible.
    for (dx,dy,w,h,fx,bx,ty,ox,oy,layer) in [
        (4,0,8,8,8,24,8,32,0,0),
        (4,8,8,12,20,32,20,0,16,1),
        (4-arm,8,arm,12,44,48+arm,20,0,16,2),
        (12,8,arm,12,36,40+arm,52,16,0,2),
        (4,20,4,12,4,12,20,0,16,3),
        (8,20,4,12,20,28,52,-16,0,3),
    ] {
        let tx=if espalda {bx} else {fx};
        for y in 0..h {for x in 0..w {
            let sample=|u:i32,v:i32| {let i=((v*64+u)*4) as usize;Color::from_rgba8(bytes[i],bytes[i+1],bytes[i+2],f32::from(bytes[i+3])/255.)};
            let mut base=sample(tx+x,ty+y);base.a=1.;
            if capas[layer] {let outer=sample(tx+x+ox,ty+y+oy);base=mezcla(base,outer,outer.a);base.a=1.;}
            grid[(dy+y) as usize][(dx+x) as usize]=Some(base);
        }}
    }
    grid
}
pub fn cabeza_textura<'a, M:'a>(p:Paleta, bytes:&[u8], lado:f32, marcada:bool)->Element<'a,M> {
    let grid=pixeles_skin(bytes,false,false,[true;4]);
    let cells=(0..8).flat_map(|y|(4..12).map(move|x|(y,x))).map(|(y,x)|grid[y][x].unwrap_or(Color::TRANSPARENT)).collect();
    cabeza_con(p,Some(cells),None,lado,if lado==32. {0.} else {4.},marcada.then_some(p.text))
}

/// Dibuja el cuerpo con el giro `paso` (de 0 a 7, pasos de 45°): cada pieza se
/// proyecta con su ancho y su fondo, y se muestrea de la vista de frente o de
/// espalda según hacia dónde mire.
fn dibujar_cuerpo(f: &mut Frame, x: f32, y: f32, e: f32, paso: u8, capas: [bool; 4], delgado: bool, textura: Option<&[u8]>) {
    let ang = f32::from(paso % 8) * std::f32::consts::FRAC_PI_4;
    let (cos, sen) = (ang.cos(), ang.sin().abs());
    let delante = cos > -0.001;
    let rejilla = rejilla_cuerpo(!delante && cos < -0.001);
    let real = textura.map(|t| pixeles_skin(t, !delante && cos < -0.001, delgado, capas));
    let ancho_brazo = if delgado { 3.0 } else { 4.0 };
    let lado_oscuro = 1.0 - cos.abs();
    // (centro, ancho, fondo, fila inicial, fila final, columnas de la rejilla)
    let piezas: [(f32, f32, f32, usize, usize, (usize, usize)); 7] = [
        (-2.0, 4.0, 4.0, 20, 32, (4, 8)),
        (2.0, 4.0, 4.0, 20, 32, (8, 12)),
        (0.0, 8.0, 4.0, 8, 20, (4, 12)),
        (-6.0 + (4.0 - ancho_brazo) / 2.0, ancho_brazo, 4.0, 8, 20,
            if delgado { (1, 4) } else { (0, 4) }),
        (6.0 - (4.0 - ancho_brazo) / 2.0, ancho_brazo, 4.0, 8, 20,
            if delgado { (12, 15) } else { (12, 16) }),
        (0.0, 8.0, 8.0, 0, 8, (4, 12)),
        (0.0, 0.0, 0.0, 0, 0, (0, 0)),
    ];
    let cx = x + 8.0 * e;
    for (xc, w, d, y0, y1, (c0, c1)) in piezas {
        if w == 0.0 {
            continue;
        }
        let proy = w * cos.abs() + d * sen;
        let centro = cx + xc * cos * e;
        let izq = centro - proy * e / 2.0;
        let n = c1 - c0;
        let ancho_col = proy * e / n as f32;
        for fila in y0..y1 {
            for i in 0..n {
                // De lado se ve la arista de la pieza, más oscura.
                let col = if cos.abs() < 0.3 { c0 } else { c0 + i };
                let color=if let Some(real)=&real {real[fila][col]} else {rejilla[fila][col].and_then(|c|color_celda(c,capas))};
                let Some(color) = color else {continue};
                let color = if lado_oscuro > 0.0 { mezcla(color, Color::BLACK, 0.35 * lado_oscuro) } else { color };
                // Se redondea al píxel de pantalla para que no queden costuras.
                let (a, b) = (izq + ancho_col * i as f32, izq + ancho_col * (i + 1) as f32);
                let (a, b) = (a.round(), b.round());
                if b > a {
                    rellenar(f, a, y + fila as f32 * e, b - a, e, color);
                }
            }
        }
    }
}

// ---- escena del estudio

/// Datos de la escena del estudio.
pub struct Escena<'a, M> {
    pub paleta: Paleta,
    pub giro: u8,
    pub capas: [bool; 4],
    pub delgado: bool,
    pub soltando: bool,
    pub textura: Option<&'a [u8]>,
    pub al_girar: Box<dyn Fn(i32) -> M + 'a>,
}

#[derive(Default)]
pub struct ArrastreEscena {
    origen: Option<f32>,
}

/// Cada 40 px de arrastre horizontal es un paso de 45°.
const PX_POR_PASO: f32 = 40.0;

impl<'a, M> canvas::Program<M> for Escena<'a, M> {
    type State = ArrastreEscena;

    fn update(
        &self,
        estado: &mut ArrastreEscena,
        evento: canvas::Event,
        limites: Rectangle,
        cursor: mouse::Cursor,
    ) -> (iced::event::Status, Option<M>) {
        use iced::event::Status;
        if let canvas::Event::Mouse(e) = evento {
            match e {
                mouse::Event::ButtonPressed(mouse::Button::Left) => {
                    if let Some(pos) = cursor.position_in(limites) {
                        estado.origen = Some(pos.x);
                        return (Status::Captured, None);
                    }
                }
                mouse::Event::CursorMoved { .. } => {
                    if let (Some(origen), Some(pos)) = (estado.origen, cursor.position_in(limites)) {
                        let pasos = ((pos.x - origen) / PX_POR_PASO).trunc() as i32;
                        if pasos != 0 {
                            estado.origen = Some(origen + pasos as f32 * PX_POR_PASO);
                            return (Status::Captured, Some((self.al_girar)(pasos)));
                        }
                    }
                }
                mouse::Event::ButtonReleased(mouse::Button::Left) => estado.origen = None,
                _ => {}
            }
        }
        (Status::Ignored, None)
    }

    fn draw(&self, _: &ArrastreEscena, renderer: &Renderer, _: &Theme, bounds: Rectangle, _: mouse::Cursor) -> Vec<Geometry> {
        let p = self.paleta;
        let mut f = Frame::new(renderer, bounds.size());
        rellenar(&mut f, 0.0, 0.0, bounds.width, bounds.height, p.surface_sunken);
        // Suelo de líneas de grabado: 72 px de alto, una regla cada 8 px.
        let suelo = (bounds.height - 72.0).max(0.0);
        let mut yy = suelo;
        while yy < bounds.height {
            rellenar(&mut f, 0.0, yy, bounds.width, 1.0, p.border);
            yy += 8.0;
        }
        let e = 8.0;
        let (w, h) = (16.0 * e, 32.0 * e);
        let y = ((bounds.height - h) / 2.0).round();
        let centro = bounds.width / 2.0;
        // Dos cuerpos de 128 px separados 56 px; el segundo mira al lado contrario.
        dibujar_cuerpo(&mut f, (centro - 28.0 - w).round(), y, e, self.giro, self.capas, self.delgado, self.textura);
        dibujar_cuerpo(&mut f, (centro + 28.0).round(), y, e, self.giro + 4, self.capas, self.delgado, self.textura);
        if self.soltando {
            // Zona de soltar: contorno sólido de 2 px dentro de la escena.
            let (a, b) = (bounds.width, bounds.height);
            rellenar(&mut f, 0.0, 0.0, a, 2.0, p.text);
            rellenar(&mut f, 0.0, b - 2.0, a, 2.0, p.text);
            rellenar(&mut f, 0.0, 0.0, 2.0, b, p.text);
            rellenar(&mut f, a - 2.0, 0.0, 2.0, b, p.text);
        }
        vec![f.into_geometry()]
    }

    fn mouse_interaction(&self, estado: &ArrastreEscena, limites: Rectangle, cursor: mouse::Cursor) -> mouse::Interaction {
        if estado.origen.is_some() {
            mouse::Interaction::Grabbing
        } else if cursor.is_over(limites) {
            mouse::Interaction::Grab
        } else {
            mouse::Interaction::default()
        }
    }
}

// ------------------------------------------------------------------ piezas

fn txt<'a, M: 'a>(s: impl Into<String>, fuente: iced::Font, tam: (f32, f32), color: Color) -> iced::widget::Text<'a, Theme, Renderer> {
    text(s.into())
        .font(fuente)
        .size(tam.0)
        .line_height(iced::widget::text::LineHeight::Absolute(tam.1.into()))
        .color(color)
}

/// `heading` de 14 px, el de nombres de cuenta.
fn nombre_cuenta<'a, M: 'a>(p: Paleta, s: &str) -> Element<'a, M> {
    txt::<M>(s, fuentes::CUERPO_NEGRITA, (14.0, 22.0), p.text).into()
}

fn mono_sm<'a, M: 'a>(s: &str, color: Color) -> Element<'a, M> {
    txt::<M>(s, fuentes::MONO, texto::MONO_SM, color).into()
}

/// Etiqueta de control o de grupo, en mayúsculas.
fn etiqueta<'a, M: 'a>(s: &str, color: Color) -> Element<'a, M> {
    txt::<M>(s.to_uppercase(), fuentes::ETIQUETA, texto::LABEL, color).into()
}

/// Insignia de alerta de 20 px de alto con regla interior de 1 px.
fn insignia_alerta<'a, M: 'a>(p: Paleta, palabra: &str) -> Element<'a, M> {
    container(txt::<M>(palabra.to_uppercase(), fuentes::ETIQUETA, (10.0, 12.0), p.warning))
        .height(20)
        .padding(Padding::from([0.0, 6.0]))
        .center_y(Length::Fixed(20.0))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(p.warning_soft)),
            border: Border { color: p.warning, width: 1.0, radius: 0.0.into() },
            ..Default::default()
        })
        .into()
}

/// Variante de un botón pequeño.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tipo {
    Primario,
    Normal,
    Fantasma,
    Peligro,
}

/// Botón de alto `control-sm` (28 px) o `control` (36 px) con icono opcional.
pub fn boton_icono<'a, M: Clone + 'a>(
    p: Paleta,
    glifo: Option<Icono>,
    etiqueta_: &str,
    tipo: Tipo,
    alto: f32,
    al_pulsar: Option<M>,
) -> Element<'a, M> {
    boton_base(p, glifo, etiqueta_, tipo, alto, Length::Shrink, al_pulsar)
}

/// Botón que ocupa todo el ancho disponible, con el contenido centrado.
pub fn boton_ancho<'a, M: Clone + 'a>(
    p: Paleta,
    glifo: Option<Icono>,
    etiqueta_: &str,
    tipo: Tipo,
    alto: f32,
    al_pulsar: Option<M>,
) -> Element<'a, M> {
    boton_base(p, glifo, etiqueta_, tipo, alto, Length::Fill, al_pulsar)
}

fn boton_base<'a, M: Clone + 'a>(
    p: Paleta,
    glifo: Option<Icono>,
    etiqueta_: &str,
    tipo: Tipo,
    alto: f32,
    ancho: Length,
    al_pulsar: Option<M>,
) -> Element<'a, M> {
    let mut fila = row![].align_y(Alignment::Center).spacing(4.0);
    if let Some(g) = glifo {
        fila = fila.push(crate::iconos::icono_texto(g, Tam::Base));
    }
    fila = fila.push(text(etiqueta_.to_uppercase()).font(fuentes::ETIQUETA)
        .size(texto::LABEL.0)
        .line_height(iced::widget::text::LineHeight::Absolute(texto::LABEL.1.into())));
    let relleno = if glifo.is_some() {
        Padding { top: 0.0, right: 12.0, bottom: 0.0, left: 8.0 }
    } else {
        Padding::from([0.0, 12.0])
    };
    let estilo = move |tema: &Theme, st: button::Status| match tipo {
        Tipo::Primario => estilo::boton_primario(p)(tema, st),
        Tipo::Normal => estilo::boton_secundario(p)(tema, st),
        Tipo::Fantasma => estilo::boton_fantasma(p)(tema, st),
        Tipo::Peligro => estilo::boton_peligro(p)(tema, st),
    };
    let _ = medida::CONTROL;
    let contenido = if ancho == Length::Fill {
        container(fila).center(Length::Fill)
    } else {
        container(fila).center_y(Length::Fill)
    };
    let mut b = button(contenido).width(ancho).height(alto).padding(relleno).style(estilo);
    if let Some(m) = al_pulsar {
        b = b.on_press(m);
    }
    b.into()
}

/// Botón solo de icono de 28 px.
fn boton_solo_icono<'a, M: Clone + 'a>(p: Paleta, glifo: Icono, al_pulsar: M) -> Element<'a, M> {
    boton_cuadrado(p, glifo, medida::CONTROL_SM, Some(al_pulsar))
}

/// Botón cuadrado de icono con el glifo centrado en ambos ejes. Sin acción
/// queda deshabilitado.
fn boton_cuadrado<'a, M: Clone + 'a>(p: Paleta, glifo: Icono, lado: f32, al_pulsar: Option<M>) -> Element<'a, M> {
    let estilo = move |_: &Theme, st: button::Status| {
        let hov = matches!(st, button::Status::Hovered | button::Status::Pressed);
        let apagado = matches!(st, button::Status::Disabled);
        button::Style {
            background: hov.then_some(Background::Color(p.text)),
            text_color: if apagado { p.text_disabled } else if hov { p.bg } else { p.text },
            border: Border { color: if apagado { p.border } else { p.text }, width: borde::MEDIO, radius: 0.0.into() },
            ..Default::default()
        }
    };
    let centro = container(crate::iconos::icono_texto(glifo, Tam::Base)).center(Length::Fill);
    button(centro)
        .width(lado)
        .height(lado)
        .padding(0)
        .on_press_maybe(al_pulsar)
        .style(estilo)
        .into()
}

/// Cargando: cuatro bloques de 8 px; se enciende uno cada 200 ms.
pub fn cargando<'a, M: 'a>(p: Paleta, ms: u64) -> Element<'a, M> {
    let activo = ((ms / 200) % 4) as usize;
    // Sentido de las agujas: arriba izquierda, arriba derecha, abajo derecha, abajo izquierda.
    let orden = [0usize, 1, 3, 2];
    let bloque = |i: usize| -> Element<'a, M> {
        let mut c = p.text;
        c.a = if orden[activo] == i { 1.0 } else { 0.2 };
        container(Space::new(8, 8)).style(estilo::bloque(c)).into()
    };
    column![row![bloque(0), bloque(1)].spacing(2), row![bloque(2), bloque(3)].spacing(2)]
        .spacing(2)
        .into()
}

/// Regla horizontal.
fn regla<'a, M: 'a>(color: Color, grosor: f32) -> Element<'a, M> {
    container(Space::new(Length::Fill, Length::Fixed(grosor))).style(estilo::bloque(color)).into()
}

// ------------------------------------------------- bloque y selector de cuenta

/// Cabeza que corresponde a cada estado de sesión.
fn cabeza_de<'a, M: 'a>(p: Paleta, estado: EstadoSesion, lado: f32) -> Element<'a, M> {
    match estado {
        EstadoSesion::Activa => cabeza_con(p, Some(celdas_cara(Cara::Busto)), None, lado, 0.0, None),
        EstadoSesion::SinConexion => cabeza_vacia(p, lado),
        _ => cabeza_con(p, Some(celdas_cara(Cara::Barba)), None, lado, 0.0, None),
    }
}

/// Bloque de cuenta al pie de la barra lateral: cabeza de 32 px, nombre y
/// proveedor. La cuenta vencida muestra la insignia en lugar del proveedor.
pub fn bloque_cuenta<'a, M: Clone + 'a>(
    p: Paleta,
    estado: EstadoSesion,
    nombre: &str,
    proveedor: &str,
    textura: Option<&[u8]>,
    al_pulsar: M,
) -> Element<'a, M> {
    let contenido = |fondo: Option<Color>| -> Element<'a, M> {
        container(
            row![
                if let Some(rgba)=textura {cabeza_textura(p,rgba,32.,false)}else{cabeza_de(p, estado, 32.0)},
                column![nombre_cuenta(p, nombre), detalle_sesion(p, estado, proveedor)].spacing(
                    if matches!(estado, EstadoSesion::Vencida | EstadoSesion::CodigoVencido) { 4.0 } else { 0.0 }
                ),
            ]
            .spacing(espacio::S3)
            .align_y(Alignment::Center),
        )
        .padding(espacio::S2)
        .width(Length::Fill)
        .style(move |_: &Theme| container::Style {
            background: fondo.map(Background::Color),
            ..Default::default()
        })
        .into()
    };
    button(hover(contenido(None), contenido(Some(p.surface_hover))))
        .padding(0)
        .width(Length::Fill)
        .on_press(al_pulsar)
        .style(estilo::sin_estilo(p.text))
        .into()
}

fn detalle_sesion<'a, M: 'a>(p: Paleta, estado: EstadoSesion, proveedor: &str) -> Element<'a, M> {
    match estado {
        EstadoSesion::Vencida | EstadoSesion::CodigoVencido => insignia_alerta(p, "Volver a entrar"),
        EstadoSesion::SinConexion => mono_sm("Sin conexión", p.text_muted),
        EstadoSesion::SinJuego => mono_sm("Sin Minecraft", p.warning),
        EstadoSesion::Activa => mono_sm(proveedor, p.text_muted),
    }
}

/// Mensajes del menú de cuentas.
pub struct MensajesSelector<M> {
    pub activa: M,
    pub vencida: M,
    pub invitado: M,
    pub agregar: M,
    pub gestionar: M,
    pub cerrar_sesion: M,
}

/// Una cuenta visible en el menú; el nombre y la selección proceden de la sesión.
pub struct CuentaSelector<M> {
    pub textura: Option<std::sync::Arc<[u8]>>,
    pub estado: EstadoSesion,
    pub nombre: String,
    pub seleccionada: bool,
    pub al_pulsar: Option<M>,
}

/// Acción del menú de cuentas, con icono y contraste compartidos.
pub struct AccionSelector<M> {
    pub icono: Icono,
    pub etiqueta: String,
    pub peligro: bool,
    pub al_pulsar: Option<M>,
}

/// Menú de cuentas que se abre desde el bloque de la barra lateral: la activa
/// marcada, la vencida con su arreglo, el modo invitado y las acciones.
pub fn selector_cuenta<'a, M: Clone + 'a>(p: Paleta, estado: EstadoSesion, m: MensajesSelector<M>) -> Element<'a, M> {
    let actual = |e: EstadoSesion| match (e, estado) {
        (EstadoSesion::Vencida, EstadoSesion::CodigoVencido | EstadoSesion::SinJuego) => true,
        (a, b) => a == b,
    };
    selector_cuentas(
        p,
        vec![
            CuentaSelector { textura: None, estado: EstadoSesion::Activa, nombre: "Mineral_7".into(), seleccionada: actual(EstadoSesion::Activa), al_pulsar: Some(m.activa) },
            CuentaSelector { textura: None, estado: EstadoSesion::Vencida, nombre: "Forjador".into(), seleccionada: actual(EstadoSesion::Vencida), al_pulsar: Some(m.vencida) },
            CuentaSelector { textura: None, estado: EstadoSesion::SinConexion, nombre: "Jugador_1".into(), seleccionada: actual(EstadoSesion::SinConexion), al_pulsar: Some(m.invitado) },
        ],
        vec![
            AccionSelector { icono: Icono::UsuarioMas, etiqueta: "Agregar cuenta".into(), peligro: false, al_pulsar: Some(m.agregar) },
            AccionSelector { icono: Icono::Usuario, etiqueta: "Gestionar cuenta y skin".into(), peligro: false, al_pulsar: Some(m.gestionar) },
            AccionSelector { icono: Icono::Salir, etiqueta: format!("Cerrar sesión de {}", if estado == EstadoSesion::SinConexion { "Jugador_1" } else { estado.nombre() }), peligro: true, al_pulsar: Some(m.cerrar_sesion) },
        ],
    )
}

/// Diseño común para cuentas reales y las variantes del modo de demostración.
pub fn selector_cuentas<'a, M: Clone + 'a>(p: Paleta, cuentas: Vec<CuentaSelector<M>>, acciones: Vec<AccionSelector<M>>) -> Element<'a, M> {
    let fila_cuenta = |e: EstadoSesion, nombre: &str, textura:Option<&[u8]>, elegida: bool, msg: Option<M>| -> Element<'a, M> {
        let construir = |invertida: bool| -> Element<'a, M> {
            let mut fila = row![
                if let Some(rgba)=textura {cabeza_textura(p,rgba,32.,false)} else {cabeza_de(p, e, 32.0)},
                column![txt::<M>(nombre.to_owned(), fuentes::CUERPO_NEGRITA, (14.0, 22.0), if invertida { p.bg } else { p.text }), detalle_para(p, e, invertida)]
            ]
            .spacing(espacio::S3)
            .align_y(Alignment::Center);
            fila = fila.push(Space::with_width(Length::Fill));
            if elegida {
                fila = fila.push(icono(Icono::Check, Tam::Base, if invertida { p.bg } else { p.success }));
            }
            container(fila)
                .height(56)
                .padding(Padding::from([0.0, espacio::S3]))
                .center_y(Length::Fixed(56.0))
                .width(Length::Fill)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(if invertida {
                        p.text
                    } else if elegida {
                        p.bg
                    } else {
                        Color::TRANSPARENT
                    })),
                    ..Default::default()
                })
                .into()
        };
        button(hover(construir(false), construir(true)))
            .padding(0)
            .width(Length::Fill)
            .on_press_maybe(msg)
            .style(estilo::sin_estilo(p.text))
            .into()
    };
    let accion = |g: Icono, etiqueta_: &str, peligro: bool, msg: Option<M>| -> Element<'a, M> {
        let tinta = if peligro { p.error } else { p.text };
        let construir = |invertida: bool| -> Element<'a, M> {
            let (fondo, color) = if invertida {
                if peligro { (p.error, p.on_error) } else { (p.text, p.bg) }
            } else {
                (Color::TRANSPARENT, tinta)
            };
            container(
                row![icono(g, Tam::Base, color), txt::<M>(etiqueta_.to_owned(), fuentes::CUERPO, texto::BODY_SM, color)]
                    .spacing(espacio::S3)
                    .align_y(Alignment::Center),
            )
            .height(34)
            .padding(Padding::from([0.0, espacio::S3]))
            .center_y(Length::Fixed(34.0))
            .width(Length::Fill)
            .style(estilo::bloque(fondo))
            .into()
        };
        button(hover(construir(false), construir(true)))
            .padding(0)
            .width(Length::Fill)
            .on_press_maybe(msg)
            .style(estilo::sin_estilo(tinta))
            .into()
    };
    let mut col = column![];
    for cuenta in cuentas {
        col = col.push(fila_cuenta(cuenta.estado, &cuenta.nombre, cuenta.textura.as_deref(), cuenta.seleccionada, cuenta.al_pulsar));
    }
    col = col.push(container(regla(p.border, borde::FINO)).padding(Padding::from([4.0, 0.0])));
    for a in acciones {
        col = col.push(accion(a.icono, &a.etiqueta, a.peligro, a.al_pulsar));
    }
    container(col)
        .width(320)
        .padding(Padding::from([4.0, borde::MEDIO]))
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(p.bg)),
            border: Border { color: p.text, width: borde::MEDIO, radius: 0.0.into() },
            text_color: Some(p.text),
            ..Default::default()
        })
        .into()
}

/// Segunda línea de una fila de cuenta.
fn detalle_para<'a, M: 'a>(p: Paleta, e: EstadoSesion, invertida: bool) -> Element<'a, M> {
    // En reposo, texto atenuado con contraste AA; invertida, el color del fondo.
    let tinta = if invertida { p.bg } else { p.text_muted };
    match e {
        EstadoSesion::Vencida => insignia_alerta(p, "Volver a entrar"),
        EstadoSesion::SinConexion => mono_sm("Sin conexión", tinta),
        _ => mono_sm("Microsoft", tinta),
    }
}

// ------------------------------------------------------------ pantalla Cuenta

/// Una capa (cape) de la cuenta.
pub struct CapaCuenta<'a> {
    pub nombre: &'a str,
    /// RGBA de 64 × 32, si ya se descargó.
    pub rgba: Option<&'a [u8]>,
}

/// Datos que necesita la pantalla de cuenta. No incluye UUID, correo ni
/// detalles de la sesión: la pantalla no los muestra.
pub struct DatosCuenta<'a> {
    pub estado: EstadoSesion,
    pub nombre: &'a str,
    /// Fecha en que se podrá cambiar de nuevo el nombre (DD-MM-AAAA).
    pub proximo_cambio: &'a str,
    /// Nombres de las últimas skins; cada uno genera su cabeza.
    pub historial: Vec<&'a str>,
    /// Skin en uso y skin elegida en el historial.
    pub en_uso: usize,
    pub elegida: usize,
    /// Textura que muestra el visor: la elegida en el historial o la actual.
    pub textura: Option<&'a [u8]>,
    pub historial_real: Vec<std::sync::Arc<[u8]>>,
    pub delgado: bool,
    /// Ángulo animado del visor y cara en reposo (0, 90, 180 o 270).
    pub angulo: f32,
    pub grados: i32,
    pub capas: Vec<CapaCuenta<'a>>,
    /// Capa puesta (`None`: ninguna) y la que se está cambiando.
    pub capa_puesta: Option<usize>,
    pub capa_cambiando: Option<Option<usize>>,
    /// «41 h», o «—» si no hay datos.
    pub tiempo_total: String,
    /// Instancia más jugada con su tiempo.
    pub mas_jugada: Option<(String, String)>,
}

/// Mensajes de la pantalla de cuenta.
pub struct MensajesCuenta<'a, M> {
    pub abrir_estudio: M,
    pub elegir_skin: Box<dyn Fn(usize) -> M + 'a>,
    pub usar_de_nuevo: M,
    pub descartar: M,
    pub girar: fn(i32) -> M,
    pub elegir_capa: Box<dyn Fn(Option<usize>) -> M + 'a>,
    pub descargar: Option<M>,
    pub cerrar_sesion: M,
    pub iniciar_sesion: M,
    pub volver_a_entrar: M,
    pub comprar: M,
    pub jugar_invitado: M,
    pub cambiar_nombre: Option<M>,
}

/// Encabezado de la pantalla: título monumental y, a la derecha, la lámina
/// de 120 px con una regla de 2 px debajo.
fn encabezado<'a, M: 'a>(p: Paleta, titulo: &str, detalle: &str, lamina: Lamina) -> Element<'a, M> {
    column![
        row![
            column![
                txt::<M>(titulo.to_uppercase(), fuentes::DISPLAY, texto::DISPLAY_XL, p.text),
                txt::<M>(detalle.to_owned(), fuentes::CUERPO, texto::BODY_SM, p.text_muted),
            ]
            .spacing(espacio::S2)
            .width(Length::Fill),
            container(grabado_posicion(p, lamina, Length::Fill, Length::Fill, (0.5, 0.2)))
                .width(120)
                .height(120)
                .clip(true),
        ]
        .align_y(Alignment::End)
        .spacing(espacio::S8),
        Space::with_height(espacio::S5),
        regla(p.text, borde::MEDIO),
    ]
    .into()
}

/// Aviso de cuenta vencida, con su arreglo.
pub fn banner_vencida<'a, M: Clone + 'a>(p: Paleta, volver: M, titulo: &str, detalle: &str) -> Element<'a, M> {
    container(
        row![
            icono(Icono::Aviso, Tam::Base, p.warning),
            column![
                txt::<M>(titulo.to_owned(), fuentes::CUERPO_NEGRITA, texto::HEADING, p.text),
                txt::<M>(
                    detalle.to_owned(),
                    fuentes::CUERPO,
                    texto::BODY_SM,
                    p.text_muted
                ),
            ]
            .spacing(4)
            .width(Length::Fill),
            boton_icono(p, Some(Icono::Refrescar), "Volver a entrar", Tipo::Primario, medida::CONTROL_SM, Some(volver)),
        ]
        .spacing(espacio::S4)
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([espacio::S4, espacio::S5]))
    .width(Length::Fill)
    .style(move |_: &Theme| container::Style {
        background: Some(Background::Color(p.warning_soft)),
        border: Border { color: p.warning, width: borde::FINO, radius: 0.0.into() },
        ..Default::default()
    })
    .into()
}

/// Tarjeta de estado: título, explicación y acciones.
fn tarjeta_estado<'a, M: 'a>(p: Paleta, titulo: &str, cuerpo_: &str, acciones: Vec<Element<'a, M>>) -> Element<'a, M> {
    let mut fila = row![].spacing(espacio::S2);
    for a in acciones {
        fila = fila.push(a);
    }
    container(
        column![
            txt::<M>(titulo.to_owned(), fuentes::CUERPO_NEGRITA, texto::HEADING, p.text),
            txt::<M>(cuerpo_.to_owned(), fuentes::CUERPO, texto::BODY_SM, p.text_muted).width(Length::Fill),
            fila,
        ]
        .spacing(espacio::S4),
    )
    .padding(espacio::S6)
    .width(Length::Fill)
    .style(estilo::tarjeta(p, false))
    .into()
}

/// Miniatura 3:4 de una capa: su cara exterior (10 × 16 texels desde 1, 1).
/// Sin textura, «Ninguna», queda rayada.
struct ProgCapa<'a> {
    paleta: Paleta,
    rgba: Option<&'a [u8]>,
}

impl<M> canvas::Program<M> for ProgCapa<'_> {
    type State = ();

    fn draw(&self, _: &(), renderer: &Renderer, _: &Theme, bounds: Rectangle, _: mouse::Cursor) -> Vec<Geometry> {
        let p = self.paleta;
        let mut f = Frame::new(renderer, bounds.size());
        rellenar(&mut f, 0.0, 0.0, bounds.width, bounds.height, p.surface_sunken);
        match self.rgba.filter(|t| t.len() == 64 * 32 * 4) {
            Some(t) => {
                let (cw, ch) = (bounds.width / 10.0, bounds.height / 16.0);
                for v in 0..16 {
                    for u in 0..10 {
                        let i = ((1 + v) * 64 + 1 + u) * 4;
                        let c = Color::from_rgba8(t[i], t[i + 1], t[i + 2], 1.0);
                        let (x0, x1) = ((u as f32 * cw).round(), ((u + 1) as f32 * cw).round());
                        let (y0, y1) = ((v as f32 * ch).round(), ((v + 1) as f32 * ch).round());
                        rellenar(&mut f, x0, y0, x1 - x0, y1 - y0, c);
                    }
                }
            }
            None => rayado(&mut f, 0.0, 0.0, bounds.width, bounds.height, p.border, 8.0, 4.0, 1.0),
        }
        vec![f.into_geometry()]
    }
}

/// Escena del visor 3D con suelo de grabado, sobre el pozo.
fn escena_visor<'a, M: 'a>(
    p: Paleta,
    textura: Option<&'a [u8]>,
    capa: Option<&'a [u8]>,
    delgado: bool,
    angulo: f32,
    al_girar: Box<dyn Fn(i32) -> M + 'a>,
) -> Element<'a, M> {
    container(
        Canvas::new(crate::visor_skin::Visor {
            paleta: p,
            textura,
            capa,
            delgado,
            capas: [true; 4],
            angulo,
            plano: false,
            busto: false,
            caminar: None,
            suelo: true,
            soltando: false,
            al_girar,
        })
        .width(Length::Fill)
        .height(Length::Fill),
    )
    .width(Length::Fill)
    .height(420)
    .style(move |_: &Theme| container::Style {
        border: Border { color: p.border, width: borde::FINO, radius: 0.0.into() },
        ..Default::default()
    })
    .into()
}

/// Girar a la izquierda, el ángulo y el nombre de la cara, girar a la derecha.
fn controles_giro<'a, M: Clone + 'a>(p: Paleta, grados: i32, girar: &dyn Fn(i32) -> M) -> Element<'a, M> {
    row![
        boton_solo_icono(p, Icono::Deshacer, girar(-1)),
        column![
            txt::<M>(format!("{grados}°"), fuentes::MONO, texto::MONO, p.text),
            txt::<M>(crate::visor_skin::nombre_cara(grados), fuentes::CUERPO, texto::BODY_SM, p.text_muted),
        ]
        .align_x(Alignment::Center)
        .width(Length::Fill),
        boton_solo_icono(p, Icono::Rehacer, girar(1)),
    ]
    .align_y(Alignment::Center)
    .into()
}

/// Bloque con su etiqueta y, a la derecha, un dato breve.
fn bloque<'a, M: 'a>(p: Paleta, titulo: &str, dato: Option<String>, contenido: Element<'a, M>) -> Element<'a, M> {
    let mut cabecera = row![etiqueta(titulo, p.text), Space::with_width(Length::Fill)].align_y(Alignment::Center);
    if let Some(d) = dato {
        cabecera = cabecera.push(mono_sm(&d, p.text_muted));
    }
    column![cabecera, contenido].spacing(espacio::S4).into()
}

/// Fila de dato de 52 px: rótulo a la izquierda y valor.
fn fila_dato<'a, M: 'a>(p: Paleta, rotulo: &str, valor: String) -> Element<'a, M> {
    container(
        row![
            txt::<M>(rotulo.to_owned(), fuentes::CUERPO, texto::BODY_SM, p.text_muted).width(160),
            txt::<M>(valor, fuentes::CUERPO, texto::BODY_SM, p.text).width(Length::Fill),
        ]
        .align_y(Alignment::Center),
    )
    .height(52)
    .align_y(iced::alignment::Vertical::Center)
    .style(move |_: &Theme| container::Style {
        border: Border { color: p.border, width: 0.0, radius: 0.0.into() },
        ..Default::default()
    })
    .into()
}

/// Pantalla de cuenta en sus cuatro variantes de sesión.
pub fn pantalla_cuenta<'a, M: Clone + 'a>(p: Paleta, d: DatosCuenta<'a>, m: MensajesCuenta<'a, M>) -> Element<'a, M> {
    let sin_skin = matches!(d.estado, EstadoSesion::SinConexion | EstadoSesion::SinJuego);
    let detalle = match d.estado {
        EstadoSesion::SinConexion => "Sin conexión · sin cuenta de Microsoft",
        _ => "Minecraft Java · Microsoft",
    };
    let mut pagina = column![encabezado(p, "Cuenta", detalle, Lamina::ApoloBelvedere)].spacing(espacio::S8);
    if matches!(d.estado, EstadoSesion::Vencida | EstadoSesion::CodigoVencido) {
        pagina = pagina.push(banner_vencida(
            p,
            m.volver_a_entrar.clone(),
            "Tu cuenta venció",
            "Para jugar en línea tienes que volver a entrar. Tus instancias y mundos siguen donde estaban.",
        ));
    }

    // Columna del visor.
    let izquierda: Element<M> = if sin_skin {
        container(
            column![
                cabeza_vacia(p, 64.0),
                txt::<M>("Sin skin que mostrar", fuentes::CUERPO, texto::BODY_SM, p.text_muted),
            ]
            .spacing(espacio::S4)
            .align_x(Alignment::Center),
        )
        .width(360)
        .height(420)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(p.surface_sunken)),
            border: Border { color: p.border, width: borde::FINO, radius: 0.0.into() },
            ..Default::default()
        })
        .into()
    } else {
        let capa = d.capa_puesta.and_then(|i| d.capas.get(i)).and_then(|c| c.rgba);
        let mut escena = column![escena_visor(p, d.textura, capa, d.delgado, d.angulo, Box::new(m.girar))];
        if d.elegida != d.en_uso {
            // Vista previa de una skin del historial, con sus dos salidas.
            escena = escena.push(
                container(
                    row![
                        txt::<M>("Vista previa de una skin anterior", fuentes::CUERPO, texto::BODY_SM, p.text)
                            .width(Length::Fill),
                        boton_icono(p, None, "Descartar", Tipo::Fantasma, medida::CONTROL_SM, Some(m.descartar.clone())),
                        boton_icono(p, None, "Usar esta skin", Tipo::Primario, medida::CONTROL_SM, Some(m.usar_de_nuevo.clone())),
                    ]
                    .spacing(espacio::S2)
                    .align_y(Alignment::Center),
                )
                .padding(Padding::from([espacio::S2, espacio::S3]))
                .width(Length::Fill)
                .style(move |_: &Theme| container::Style {
                    background: Some(Background::Color(p.surface_raised)),
                    border: Border { color: p.text, width: borde::FINO, radius: 0.0.into() },
                    ..Default::default()
                }),
            );
        }
        column![
            escena,
            controles_giro(p, d.grados, &m.girar),
            row![
                boton_ancho(p, Some(Icono::Pincel), "Abrir estudio", Tipo::Normal, medida::CONTROL, Some(m.abrir_estudio.clone())),
                boton_solo_icono_opcional(p, Icono::Descargas, m.descargar.clone()),
            ]
            .spacing(espacio::S2),
        ]
        .spacing(espacio::S4)
        .width(360)
        .into()
    };

    let derecha: Element<M> = match d.estado {
        EstadoSesion::SinConexion => tarjeta_estado(
            p,
            "Cuenta sin conexión",
            "Elige un nombre para jugar en mundos locales y servidores que admitan cuentas sin conexión.",
            vec![boton_icono(p, Some(Icono::Usuario), "Usar cuenta sin conexión", Tipo::Primario, medida::CONTROL_SM, Some(m.jugar_invitado.clone())), boton_icono(p, Some(Icono::Entrar), "Iniciar sesión", Tipo::Normal, medida::CONTROL_SM, Some(m.iniciar_sesion.clone()))],
        ),
        EstadoSesion::SinJuego => tarjeta_estado(
            p,
            "Esta cuenta no tiene Minecraft",
            "Entraste con una cuenta de Microsoft que no tiene el juego. Puedes comprarlo o usar una cuenta sin conexión.",
            vec![
                boton_icono(p, Some(Icono::Externo), "Comprar Minecraft", Tipo::Normal, medida::CONTROL_SM, Some(m.comprar.clone())),
                boton_icono(p, None, "Usar cuenta sin conexión", Tipo::Fantasma, medida::CONTROL_SM, Some(m.jugar_invitado.clone())),
            ],
        ),
        _ => {
            // Perfil: nombre y actividad en Ícaro.
            let mut perfil = column![
                row![
                    column![
                        txt::<M>(d.nombre.to_owned(), fuentes::TITULO, (28.0, 32.0), p.text),
                        txt::<M>(
                            if d.proximo_cambio.is_empty() {"Gestiona tu nombre de usuario en Minecraft.net.".into()} else {format!("Puedes cambiarlo de nuevo el {}.", d.proximo_cambio)},
                            fuentes::CUERPO,
                            texto::BODY_SM,
                            p.text_muted
                        ),
                    ]
                    .spacing(6)
                    .width(Length::Fill),
                    boton_icono::<M>(p, Some(Icono::Editar), "Cambiar", Tipo::Normal, medida::CONTROL_SM, m.cambiar_nombre.clone()),
                ]
                .spacing(espacio::S6)
                .align_y(Alignment::Center),
                fila_dato(p, "Tiempo jugado", d.tiempo_total.clone()),
            ];
            if let Some((nombre, tiempo)) = &d.mas_jugada {
                perfil = perfil.push(fila_dato(p, "Más jugada", format!("{nombre} · {tiempo}")));
            }

            // Capa: Ninguna y las capas de la cuenta.
            let mut capas = row![].spacing(espacio::S3);
            let opciones = std::iter::once((None, "Ninguna", None))
                .chain(d.capas.iter().enumerate().map(|(i, c)| (Some(i), c.nombre, c.rgba)));
            for (indice, nombre, rgba) in opciones {
                let puesta = d.capa_puesta == indice;
                let cambiando = d.capa_cambiando == Some(indice);
                let miniatura = container(Canvas::new(ProgCapa { paleta: p, rgba }).width(45).height(60))
                    .padding(2)
                    .style(move |_: &Theme| container::Style {
                        border: Border {
                            color: if puesta { p.text } else { p.border },
                            width: if puesta { borde::MEDIO } else { borde::FINO },
                            radius: 0.0.into(),
                        },
                        ..Default::default()
                    });
                let pie: Element<M> = if cambiando {
                    cargando(p, 0)
                } else {
                    txt::<M>(nombre.to_owned(), fuentes::CUERPO, texto::BODY_SM, if puesta { p.text } else { p.text_muted }).into()
                };
                let contenido = column![miniatura, pie].spacing(espacio::S2).align_x(Alignment::Center);
                capas = capas.push(
                    button(contenido)
                        .padding(0)
                        .on_press_maybe((!puesta && d.capa_cambiando.is_none()).then(|| (m.elegir_capa)(indice)))
                        .style(estilo::sin_estilo(p.text)),
                );
            }

            // Historial de skins, con la actual marcada.
            let mut cabezas = row![].spacing(espacio::S3).align_y(Alignment::Start);
            for (i, n) in d.historial.iter().enumerate() {
                let cara: Element<M> = if let Some(rgba) = d.historial_real.get(i) {
                    cabeza_textura(p, rgba, 48., i == d.elegida)
                } else {
                    cabeza_historial(p, n, 48., i == d.elegida)
                };
                let mut celda = column![cara].spacing(6).align_x(Alignment::Center);
                if i == d.en_uso {
                    celda = celda.push(crate::componentes::insignia_solida(p, "Actual"));
                }
                cabezas = cabezas.push(
                    button(celda).padding(0).on_press((m.elegir_skin)(i)).style(estilo::sin_estilo(p.text)),
                );
            }

            let sesion = row![
                column![
                    txt::<M>("Cerrar sesión en este equipo", fuentes::CUERPO, texto::BODY_SM, p.text),
                    txt::<M>("Tus instancias y mundos se quedan aquí.", fuentes::CUERPO, texto::BODY_SM, p.text_muted),
                ]
                .spacing(4)
                .width(Length::Fill),
                boton_icono(p, Some(Icono::Salir), &format!("Cerrar sesión de {}", d.nombre), Tipo::Peligro, medida::CONTROL_SM, Some(m.cerrar_sesion.clone())),
            ]
            .spacing(espacio::S6)
            .align_y(Alignment::Center);

            column![
                bloque(p, "Perfil", None, perfil.into()),
                bloque(p, "Capa", Some(format!("{} de la cuenta", d.capas.len())), capas.into()),
                bloque(p, "Historial de skins", Some(format!("{} de 8", d.historial.len().max(d.historial_real.len()))), cabezas.into()),
                column![regla(p.border, borde::FINO), sesion].spacing(espacio::S6),
            ]
            .spacing(40)
            .width(Length::Fill)
            .into()
        }
    };

    pagina = pagina.push(
        row![izquierda, container(derecha).width(Length::Fill).align_y(iced::alignment::Vertical::Top)]
            .spacing(48)
            .height(Length::Shrink),
    );
    pagina.padding(Padding { top: 40.0, right: 48.0, bottom: 48.0, left: 48.0 }).into()
}

/// Botón de solo icono, deshabilitado sin acción.
/// Botón de icono del mismo alto que los botones de acción (`control`).
fn boton_solo_icono_opcional<'a, M: Clone + 'a>(p: Paleta, glifo: Icono, al_pulsar: Option<M>) -> Element<'a, M> {
    boton_cuadrado(p, glifo, medida::CONTROL, al_pulsar)
}

// ----------------------------------------------------------- Estudio de skin

/// Datos del estudio de skin.
pub struct DatosEstudio<'a> {
    pub modelo: usize,
    /// Sombrero, chaqueta, mangas y pantalones.
    pub capas: [bool; 4],
    pub angulo: f32,
    pub grados: i32,
    pub plano: bool,
    pub busto: bool,
    /// Fase de la caminata en segundos, si está activa.
    pub caminar: Option<f32>,
    /// Milisegundos desde que empezó la subida; `None` si no se está subiendo.
    pub subiendo: Option<u64>,
    /// Archivo cargado, para el encabezado.
    pub textura: Option<&'a str>,
    pub error: Option<&'a str>,
    pub soltando: bool,
    pub rgba: Option<&'a [u8]>,
    pub puede_aplicar: bool,
    /// La skin se acaba de aplicar: el botón lo confirma unos segundos.
    pub aplicada: bool,
    pub capas_cuenta: Vec<CapaCuenta<'a>>,
    pub capa_puesta: Option<usize>,
    pub capa_cambiando: Option<Option<usize>>,
    /// Skins guardadas en Ícaro.
    pub biblioteca: Vec<std::sync::Arc<[u8]>>,
    pub biblioteca_elegida: Option<usize>,
    /// Capa visible cuya zona se marca en la textura plana.
    pub zona: Option<usize>,
}

/// Mensajes del estudio de skin.
pub struct MensajesEstudio<M> {
    pub volver: M,
    pub modelo: fn(usize) -> M,
    pub capa: fn(usize, bool) -> M,
    pub girar: fn(i32) -> M,
    pub ir: fn(i32) -> M,
    pub vista: fn(usize) -> M,
    pub encuadre: fn(usize) -> M,
    pub caminar: fn(bool) -> M,
    pub zona: fn(Option<usize>) -> M,
    pub elegir_capa: fn(Option<usize>) -> M,
    pub elegir_biblioteca: fn(usize) -> M,
    pub guardar_biblioteca: Option<M>,
    pub cargar: M,
    pub descargar: Option<M>,
    pub aplicar: M,
}

/// Zonas de la textura de cada capa visible, en texels (x, y, ancho, alto).
const ZONAS: [(f32, f32, f32, f32); 4] =
    [(32.0, 0.0, 32.0, 16.0), (16.0, 32.0, 24.0, 16.0), (40.0, 32.0, 16.0, 16.0), (0.0, 32.0, 16.0, 16.0)];

/// Textura plana a 3×, con píxeles nítidos, rejilla de 8 px y la zona marcada.
struct ProgTextura<'a> {
    paleta: Paleta,
    rgba: Option<&'a [u8]>,
    zona: Option<usize>,
}

impl<M> canvas::Program<M> for ProgTextura<'_> {
    type State = ();

    fn draw(&self, _: &(), renderer: &Renderer, _: &Theme, bounds: Rectangle, _: mouse::Cursor) -> Vec<Geometry> {
        let p = self.paleta;
        let mut f = Frame::new(renderer, bounds.size());
        rayado(&mut f, 0.0, 0.0, bounds.width, bounds.height, p.border, 8.0, 4.0, 1.0);
        let e = bounds.width / 64.0;
        if let Some(t) = self.rgba.filter(|t| t.len() == 64 * 64 * 4) {
            for v in 0..64 {
                for u in 0..64 {
                    let i = (v * 64 + u) * 4;
                    if t[i + 3] == 0 {
                        continue;
                    }
                    let c = Color::from_rgba8(t[i], t[i + 1], t[i + 2], f32::from(t[i + 3]) / 255.0);
                    rellenar(&mut f, u as f32 * e, v as f32 * e, e, e, c);
                }
            }
        }
        let mut x = 0.0;
        while x <= 64.0 {
            let mut g = p.border;
            g.a = 0.6;
            rellenar(&mut f, x * e, 0.0, 1.0, bounds.height, g);
            rellenar(&mut f, 0.0, x * e, bounds.width, 1.0, g);
            x += 8.0;
        }
        if let Some((zx, zy, zw, zh)) = self.zona.and_then(|z| ZONAS.get(z).copied()) {
            contorno(&mut f, zx * e, zy * e, zw * e, zh * e, p.text);
            contorno(&mut f, zx * e + 2.0, zy * e + 2.0, zw * e - 4.0, zh * e - 4.0, p.text);
        }
        vec![f.into_geometry()]
    }
}

/// Estudio de skin: escena grande con el visor 3D, controles de vista arriba,
/// caras y giro de a 90° abajo, y el panel con modelo, capas, textura, capa,
/// biblioteca y acciones.
pub fn estudio_skin<'a, M: Clone + 'a>(p: Paleta, d: DatosEstudio<'a>, m: MensajesEstudio<M>) -> Element<'a, M> {
    let bloqueado = d.subiendo.is_some();
    let capa_visor = d.capa_puesta.and_then(|i| d.capas_cuenta.get(i)).and_then(|c| c.rgba);
    let visor = Canvas::new(crate::visor_skin::Visor {
        paleta: p,
        textura: d.rgba,
        capa: capa_visor,
        delgado: d.modelo == 1,
        capas: d.capas,
        angulo: d.angulo,
        plano: d.plano,
        busto: d.busto,
        caminar: d.caminar,
        suelo: !d.busto,
        soltando: d.soltando,
        al_girar: Box::new(m.girar),
    })
    .width(Length::Fill)
    .height(Length::Fill);
    let barra = row![
        crate::componentes::segmentado(p, &["3D", "Plano"], usize::from(d.plano), m.vista),
        crate::componentes::segmentado(p, &["Cuerpo", "Busto"], usize::from(d.busto), m.encuadre),
        Space::with_width(Length::Fill),
        crate::componentes::casilla(p, "Caminar", d.caminar.is_some(), m.caminar),
    ]
    .spacing(espacio::S3)
    .align_y(Alignment::Center);
    let pie = row![
        crate::componentes::segmentado(p, &["Frente", "Derecho", "Espalda", "Izquierdo"], (d.grados / 90) as usize, {
            let ir = m.ir;
            move |i: usize| ir(i as i32 * 90)
        }),
        Space::with_width(Length::Fill),
        container(controles_giro(p, d.grados, &m.girar)).width(180),
    ]
    .spacing(espacio::S4)
    .align_y(Alignment::Center);
    let ayuda = container(mono_sm("Arrastra o usa las flechas · de a 90°", p.text_muted)).center_x(Length::Fill);
    let escena = container(column![barra, visor, pie, ayuda].spacing(espacio::S4))
        .padding(espacio::S5)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(estilo::bloque(p.surface_sunken));

    // Capas visibles: al pasar sobre una, la textura marca su zona.
    let mut capas = column![etiqueta("Capas visibles", p.text)].spacing(10);
    for (i, n) in ["Sombrero", "Chaqueta", "Mangas", "Pantalones"].iter().enumerate() {
        let f = m.capa;
        capas = capas.push(
            iced::widget::mouse_area(crate::componentes::casilla(p, n, d.capas[i], move |v: bool| f(i, v)))
                .on_enter((m.zona)(Some(i)))
                .on_exit((m.zona)(None)),
        );
    }
    let textura = column![
        etiqueta("Textura", p.text),
        container(Canvas::new(ProgTextura { paleta: p, rgba: d.rgba, zona: d.zona }).width(144).height(144))
            .style(move |_: &Theme| container::Style {
                border: Border { color: p.border, width: borde::FINO, radius: 0.0.into() },
                ..Default::default()
            }),
    ]
    .spacing(10);

    // Capa de la cuenta, en miniaturas pequeñas.
    let mut tiras = row![].spacing(espacio::S2);
    let opciones = std::iter::once((None, None)).chain(d.capas_cuenta.iter().enumerate().map(|(i, c)| (Some(i), c.rgba)));
    for (indice, rgba) in opciones {
        let puesta = d.capa_puesta == indice;
        let miniatura: Element<M> = if d.capa_cambiando == Some(indice) {
            container(cargando(p, 0)).width(30).height(40).center(Length::Fixed(40.0)).into()
        } else {
            Canvas::new(ProgCapa { paleta: p, rgba }).width(30).height(40).into()
        };
        tiras = tiras.push(
            button(container(miniatura).padding(2).style(move |_: &Theme| container::Style {
                border: Border {
                    color: if puesta { p.text } else { p.border },
                    width: if puesta { borde::MEDIO } else { borde::FINO },
                    radius: 0.0.into(),
                },
                ..Default::default()
            }))
            .padding(0)
            .on_press_maybe((!puesta && d.capa_cambiando.is_none()).then(|| (m.elegir_capa)(indice)))
            .style(estilo::sin_estilo(p.text)),
        );
    }

    // Biblioteca de skins guardadas en Ícaro.
    let mut cabezas = row![].spacing(espacio::S2);
    for (i, rgba) in d.biblioteca.iter().enumerate() {
        cabezas = cabezas.push(
            button(cabeza_textura(p, rgba, 36.0, d.biblioteca_elegida == Some(i)))
                .padding(0)
                .on_press_maybe((!bloqueado).then(|| (m.elegir_biblioteca)(i)))
                .style(estilo::sin_estilo(p.text)),
        );
    }
    let biblioteca = column![
        row![
            etiqueta("Biblioteca", p.text),
            Space::with_width(Length::Fill),
            boton_icono(p, Some(Icono::Mas), "Guardar actual", Tipo::Fantasma, medida::CONTROL_SM, m.guardar_biblioteca.clone()),
        ]
        .align_y(Alignment::Center),
        if d.biblioteca.is_empty() {
            Element::from(txt::<M>("Aún no hay skins guardadas.", fuentes::CUERPO, texto::BODY_SM, p.text_muted))
        } else {
            cabezas.wrap().into()
        },
    ]
    .spacing(10);

    let mut acciones = column![row![
        boton_ancho(p, Some(Icono::Subir), "Cargar .png", Tipo::Normal, medida::CONTROL, (!bloqueado).then_some(m.cargar.clone())),
        boton_solo_icono_opcional(p, Icono::Descargas, m.descargar.clone()),
    ]
    .spacing(espacio::S2)]
    .spacing(espacio::S2);
    if let Some(e) = d.error {
        acciones = acciones.push(txt::<M>(e.to_owned(), fuentes::CUERPO, texto::BODY_SM, p.error));
    }
    let aplicar: Element<M> = match d.subiendo {
        Some(ms) => container(
            row![cargando(p, ms), txt::<M>("SUBIENDO SKIN", fuentes::ETIQUETA, texto::LABEL, p.text_muted)]
                .spacing(espacio::S3)
                .align_y(Alignment::Center),
        )
        .height(medida::CONTROL)
        .width(Length::Fill)
        .align_x(iced::alignment::Horizontal::Center)
        .align_y(iced::alignment::Vertical::Center)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(p.surface_sunken)),
            border: Border { color: p.border, width: borde::MEDIO, radius: 0.0.into() },
            ..Default::default()
        })
        .into(),
        None if d.aplicada => boton_ancho(p, Some(Icono::Check), "Skin aplicada", Tipo::Primario, medida::CONTROL, None),
        None => boton_ancho(
            p,
            Some(Icono::Check),
            "Aplicar a la cuenta",
            Tipo::Primario,
            medida::CONTROL,
            d.puede_aplicar.then_some(m.aplicar.clone()),
        ),
    };
    acciones = acciones.push(aplicar);

    let archivo = d.textura.map_or_else(|| "Skin de la cuenta · 64 × 64".to_owned(), |t| format!("{t} · 64 × 64"));
    let panel = container(
        iced::widget::scrollable(
            column![
                row![
                    boton_solo_icono(p, Icono::Flecha, m.volver.clone()),
                    column![
                        txt::<M>("Estudio de skin", fuentes::TITULO, (22.0, 28.0), p.text),
                        mono_sm(&archivo, p.text_muted),
                    ]
                    .spacing(6),
                ]
                .spacing(espacio::S3)
                .align_y(Alignment::Center),
                column![
                    etiqueta("Modelo", p.text),
                    crate::componentes::segmentado(p, &["Clásico", "Delgado"], d.modelo, m.modelo),
                ]
                .spacing(10),
                row![capas.width(Length::Fill), textura].spacing(espacio::S5),
                column![etiqueta("Capa", p.text), tiras].spacing(10),
                biblioteca,
                acciones,
            ]
            .spacing(espacio::S6),
        )
        .height(Length::Fill),
    )
    .padding(espacio::S6)
    .width(380)
    .height(Length::Fill)
    .style(estilo::bloque(p.surface));

    row![escena, regla_vertical(p.border), panel].height(Length::Fill).into()
}

fn regla_vertical<'a, M: 'a>(color: Color) -> Element<'a, M> {
    container(Space::new(Length::Fixed(1.0), Length::Fill)).style(estilo::bloque(color)).into()
}

// ------------------------------------------------------------ código de dispositivo

/// Celda del código de dispositivo: 44 × 56 px, contorno de 2 px y fondo de
/// pozo. Vencida va rayada y apagada.
fn celda_codigo<'a, M: 'a>(p: Paleta, c: char, vencido: bool) -> Element<'a, M> {
    let apagar = move |color: Color| if vencido { mezcla(color, p.surface, 0.3) } else { color };
    let letra = txt::<M>(
        c.to_string(),
        fuentes::MONO_MEDIO,
        texto::CODE_XL,
        apagar(if vencido { p.text_disabled } else { p.text }),
    );
    let relleno: Element<M> = if vencido {
        stack![
            Canvas::new(ProgRayas { color: apagar(p.border), periodo: 6.0, inicio: 5.0 })
                .width(Length::Fill)
                .height(Length::Fill),
            container(letra).center(Length::Fill),
        ]
        .into()
    } else {
        container(letra).center(Length::Fill).into()
    };
    container(relleno)
        .width(44)
        .height(56)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(apagar(p.surface_sunken))),
            border: Border { color: apagar(p.text), width: borde::MEDIO, radius: 0.0.into() },
            ..Default::default()
        })
        .into()
}

struct ProgRayas {
    color: Color,
    periodo: f32,
    inicio: f32,
}

impl<M> canvas::Program<M> for ProgRayas {
    type State = ();

    fn draw(&self, _: &(), renderer: &Renderer, _: &Theme, bounds: Rectangle, _: mouse::Cursor) -> Vec<Geometry> {
        let mut f = Frame::new(renderer, bounds.size());
        rayado(&mut f, 0.0, 0.0, bounds.width, bounds.height, self.color, self.periodo, self.inicio, 1.0);
        vec![f.into_geometry()]
    }
}

/// Código de dispositivo en celdas separadas por un guion. Con `vencido` las
/// celdas van rayadas y el texto en `text-disabled`.
pub fn codigo<'a, M: 'a>(p: Paleta, codigo: &str, vencido: bool) -> Element<'a, M> {
    let mut fila = row![].spacing(6).align_y(Alignment::Center);
    for c in codigo.chars() {
        if c == '-' {
            fila = fila.push(
                container(txt::<M>("-", fuentes::MONO, texto::MONO, p.text_muted)).width(16).center_x(Length::Fixed(16.0)),
            );
        } else {
            fila = fila.push(celda_codigo(p, c, vencido));
        }
    }
    fila.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn espalda_local_invierte_los_lados_de_brazos_y_piernas() {
        let mut rgba=vec![0;64*64*4];
        for (x,y,color) in [(52,20,[255,0,0,255]),(44,52,[0,255,0,255]),(12,20,[0,0,255,255]),(28,52,[255,255,0,255])] {
            let i=(y*64+x)*4;rgba[i..i+4].copy_from_slice(&color);
        }
        let espalda=pixeles_skin_local(&rgba,true,false,false);
        assert_eq!(espalda[8][12],Some(Color::from_rgb8(255,0,0)));
        assert_eq!(espalda[8][0],Some(Color::from_rgb8(0,255,0)));
        assert_eq!(espalda[20][8],Some(Color::from_rgb8(0,0,255)));
        assert_eq!(espalda[20][4],Some(Color::from_rgb8(255,255,0)));
    }
    #[test]
    fn textura_real_muestra_frente_espalda_modelo_y_capas_sin_reemplazar_el_diseno() {
        let mut rgba=vec![0;64*64*4];
        for pixel in rgba.chunks_exact_mut(4){pixel.copy_from_slice(&[20,30,40,255]);}
        let paint=|bytes:&mut Vec<u8>,x:usize,y:usize,color:[u8;4]|bytes[(y*64+x)*4..(y*64+x)*4+4].copy_from_slice(&color);
        paint(&mut rgba,8,8,[200,10,10,255]);
        paint(&mut rgba,24,8,[10,200,10,255]);
        paint(&mut rgba,40,8,[10,10,200,255]);
        let front=pixeles_skin(&rgba,false,false,[false;4]);
        let back=pixeles_skin(&rgba,true,false,[false;4]);
        assert_eq!(front[0][4],Some(Color::from_rgb8(200,10,10)));
        assert_eq!(back[0][4],Some(Color::from_rgb8(10,200,10)));
        let overlay=pixeles_skin(&rgba,false,false,[true,false,false,false])[0][4].unwrap();
        assert!((overlay.r-10./255.).abs()<1e-6 && (overlay.g-10./255.).abs()<1e-6 && (overlay.b-200./255.).abs()<1e-6);
        assert!(pixeles_skin(&rgba,false,true,[false;4])[8][0].is_none());
        assert!(front[8][0].is_some());
        assert!(pixeles_skin(&[],false,false,[true;4]).iter().flatten().all(Option::is_none));
    }
    use crate::tema::TINTA;

    #[test]
    fn el_icono_por_nombre_es_simetrico_y_estable() {
        let a = celdas_nombre(TINTA, "skin-a");
        let b = celdas_nombre(TINTA, "skin-a");
        assert_eq!(a, b);
        for y in 0..8 {
            for x in 0..4 {
                assert_eq!(a[y * 8 + x], a[y * 8 + 7 - x]);
            }
        }
    }

    #[test]
    fn la_rejilla_del_cuerpo_mide_16_por_32() {
        for espalda in [false, true] {
            let r = rejilla_cuerpo(espalda);
            assert_eq!(r.len(), 32);
            assert!(r.iter().all(|f| f.len() == 16));
        }
    }

    #[test]
    fn las_capas_apagadas_dejan_ver_la_piel() {
        let celda = ('l', Some(Capa::Pantalones));
        assert_eq!(color_celda(celda, [true; 4]), tono('l'));
        assert_eq!(color_celda(celda, [true, true, true, false]), tono(PIEL));
    }
}
