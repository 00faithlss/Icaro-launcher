// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Lista de servidores guardados con orden manual y bloque de la red privada.

use iced::widget::canvas::{self, Frame, Path};
use iced::widget::{column, container, mouse_area, row, stack, text, Space};
use iced::{mouse, Alignment, Color, Element, Length, Padding, Point, Rectangle, Size};

use crate::componentes::{self, boton_con_icono, insignia, insignia_contorno, punto, Estado, Variante};
use crate::estilo;
use crate::fuentes;
use crate::iconos::{icono, Icono, Tam};
use crate::laminas::{grabado_posicion, Lamina};
use crate::movimiento;
use crate::tema::{borde, espacio, texto, Paleta};

/// Estado de un servidor guardado.
#[derive(Debug, Clone, PartialEq)]
pub enum EstadoServidor {
    EnLinea {
        jugadores: u32,
        maximo: u32,
        /// Latencia en milisegundos.
        ms: u32,
    },
    /// Texto de cuándo dejó de responder, por ejemplo `desde las 03:12`.
    FueraDeLinea(String),
}

/// Servidor guardado.
#[derive(Debug, Clone)]
pub struct Servidor {
    pub nombre: String,
    pub direccion: String,
    /// Mensaje del día.
    pub motd: Option<String>,
    pub estado: EstadoServidor,
    /// Instancia vinculada y si sincroniza mods al iniciar.
    pub vinculo: Option<(String, bool)>,
    /// Versión del servidor cuando difiere de la de la instancia.
    pub version_distinta: Option<String>,
}

/// Alto de una fila: relleno de 16, icono de 48 y regla fina arriba y abajo.
pub const ALTO_FILA: f32 = 100.0;
/// Separación entre filas.
pub const SEPARACION: f32 = 12.0;

/// Orden en curso: la fila `origen` se arrastra y su hueco está en `destino`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Arrastre {
    pub origen: usize,
    pub destino: usize,
}

impl Arrastre {
    /// Hueco más cercano a la altura `y` del puntero, medida desde el borde
    /// superior de la lista, con `total` filas.
    pub fn destino_en(y: f32, total: usize) -> usize {
        let paso = ALTO_FILA + SEPARACION;
        ((y / paso).floor().max(0.0) as usize).min(total.saturating_sub(1))
    }

    /// Texto que anuncia el movimiento a quien usa lector de pantalla.
    pub fn anuncio(&self, nombre: &str, total: usize) -> String {
        format!("{nombre}, posición {} de {total}", self.destino + 1)
    }
}

/// Mensajes que emite la lista.
pub struct AccionesLista<M> {
    pub jugar: fn(usize) -> M,
    pub editar: Option<fn(usize) -> M>,
    pub eliminar: Option<fn(usize) -> M>,
    pub permitir_sin_estado: bool,
    /// El asa de la fila `i` se presionó: empieza el arrastre.
    pub tomar: Option<fn(usize) -> M>,
    /// El puntero entró (`Some`) o salió (`None`) de una fila.
    pub sobre: fn(Option<usize>) -> M,
}

/// Icono procedural de 48 px: una trama de 8 × 8 píxeles simétrica en cinco
/// tonos de tinta, sacada del nombre (FNV-1a y xorshift), con una regla fina
/// por fuera.
pub fn avatar_servidor<'a, M: 'a>(p: Paleta, nombre: &str) -> Element<'a, M> {
    avatar_servidor_de(p, nombre, 50.)
}

/// Mismo icono con el lado dado, regla incluida: 50 (`ic-pixel`) o 34 (`ic-pixel--sm`).
pub fn avatar_servidor_de<'a, M: 'a>(p: Paleta, nombre: &str, lado: f32) -> Element<'a, M> {
    let mut h: u32 = 2_166_136_261;
    for u in nombre.encode_utf16() {
        h = (h ^ u32::from(u)).wrapping_mul(16_777_619);
    }
    let mut r = if h == 0 { 1 } else { h };
    let mut celdas = [0usize; 64];
    for y in 0..8 {
        for x in 0..4 {
            r ^= r << 13;
            r ^= r >> 17;
            r ^= r << 5;
            let n = r % 9;
            let mut t = match n {
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
    let tonos = [p.surface_sunken, p.border, p.border_strong, p.text_muted, p.text];
    let colores: Vec<Color> = celdas.iter().map(|&t| tonos[t]).collect();
    iced::widget::canvas(Avatar { colores, anillo: p.border })
        .width(lado)
        .height(lado)
        .into()
}

struct Avatar {
    colores: Vec<Color>,
    anillo: Color,
}

impl<M> canvas::Program<M> for Avatar {
    type State = ();

    fn draw(
        &self,
        _: &(),
        renderer: &iced::Renderer,
        _: &iced::Theme,
        limites: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut marco = Frame::new(renderer, limites.size());
        // 48 px de trama con 1 px de regla alrededor.
        let lado = (limites.width - 2.0) / 8.0;
        for (i, c) in self.colores.iter().enumerate() {
            let (x, y) = ((i % 8) as f32, (i / 8) as f32);
            marco.fill_rectangle(
                Point::new(1.0 + x * lado, 1.0 + y * lado),
                Size::new(lado, lado),
                *c,
            );
        }
        marco.stroke(
            &Path::rectangle(Point::new(0.5, 0.5), Size::new(limites.width - 1.0, limites.height - 1.0)),
            canvas::Stroke::default().with_color(self.anillo).with_width(1.0),
        );
        vec![marco.into_geometry()]
    }
}

/// Estado visual de una fila.
#[derive(Debug, Clone, Copy, Default)]
pub struct VistaFila {
    /// El puntero está encima: la regla pasa a tinta plena.
    pub encima: bool,
    /// Fila levantada durante el arrastre: contorno de 2 px y superficie
    /// elevada.
    pub levantada: bool,
    /// Avance de Colocar, de 0 a 1, si la fila se acaba de agregar.
    pub colocando: Option<f32>,
}

/// Fila de servidor: asa de orden, icono, datos y Jugar directo.
pub fn tarjeta_servidor<'a, M: Clone + 'a>(
    p: Paleta,
    i: usize,
    s: &Servidor,
    acciones: &AccionesLista<M>,
    vista: VistaFila,
) -> Element<'a, M> {
    let en_linea = matches!(s.estado, EstadoServidor::EnLinea { .. });
    let mut titulo = row![
        punto(p, en_linea),
        text(s.nombre.clone())
            .font(fuentes::CUERPO_NEGRITA)
            .size(texto::HEADING.0)
            .color(p.text),
    ]
    .spacing(espacio::S3)
    .align_y(Alignment::Center);
    if let Some((instancia, sincroniza)) = &s.vinculo {
        let palabra = if *sincroniza {
            format!("{instancia} · sincroniza")
        } else {
            format!("Vinculado a {instancia}")
        };
        titulo = titulo.push(insignia_contorno(p, &palabra));
    }
    if let Some(v) = &s.version_distinta {
        titulo = titulo.push(insignia(p, Estado::Aviso, &format!("Servidor {v}")));
    }
    let linea = match &s.estado {
        EstadoServidor::EnLinea { jugadores, maximo, ms } => {
            format!("{} · {jugadores} / {maximo} · {ms} ms", s.direccion)
        }
        EstadoServidor::FueraDeLinea(estado) => format!("{} · {estado}", s.direccion),
    };
    let mut datos = column![
        titulo,
        text(linea)
            .font(fuentes::MONO)
            .size(texto::MONO_SM.0)
            .color(p.text_muted),
    ]
    .spacing(2)
    .width(Length::Fill);
    if let Some(motd)=&s.motd{
        datos=datos.push(text(motd.clone()).size(texto::BODY_SM.0).color(p.text_muted));
    }

    let jugar = if en_linea || acciones.permitir_sin_estado {
        boton_con_icono(p, Icono::Jugar, "Jugar", Variante::Primario, Some((acciones.jugar)(i)), false)
    } else {
        boton_con_icono(p, Icono::Jugar, "Jugar", Variante::Secundario, None, false)
    };

    let asa:Element<'a,M> = if let Some(tomar)=acciones.tomar {mouse_area(
        container(icono(
            Icono::Arrastrar,
            Tam::Base,
            if vista.levantada { p.text } else { p.text_muted },
        ))
        .center(Length::Fixed(24.0)),
    )
    .on_press(tomar(i))
    .interaction(mouse::Interaction::Grab).into()}else{Space::with_width(0).into()};

    let mut controles=row![jugar].spacing(8).align_y(Alignment::Center);
    if let Some(eliminar)=acciones.eliminar {controles=controles.push(boton_con_icono(p,Icono::Papelera,"",Variante::Fantasma,Some(eliminar(i)),true));}
    let fila = row![asa, avatar_servidor(p, &s.nombre), datos, controles]
        .spacing(espacio::S4)
        .align_y(Alignment::Center);
    let resaltada = vista.levantada || vista.encima || vista.colocando.is_some_and(|t| t < 2.0 / 3.0);
    let ancho = if vista.levantada || vista.colocando.is_some_and(|t| t < 2.0 / 3.0) {
        borde::MEDIO
    } else {
        borde::FINO
    };
    let tarjeta = container(fila)
        .padding(Padding::from([espacio::S4 - (ancho - 1.0), espacio::S5]))
        .width(Length::Fill)
        .height(ALTO_FILA)
        .center_y(ALTO_FILA)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(if vista.levantada {
                p.surface_raised
            } else {
                p.surface
            })),
            border: iced::Border {
                width: ancho,
                color: if resaltada { p.text } else { p.border },
                radius: 0.0.into(),
            },
            ..Default::default()
        });

    // Colocar: el bloque aparece al 88 % y crece en tres pasos hasta el 100 %.
    let tarjeta: Element<'a, M> = match vista.colocando {
        Some(t) => {
            let falta = 1.0 - movimiento::colocar(t);
            container(tarjeta)
                .padding(Padding::from([falta * ALTO_FILA / 2.0, falta * 470.0]))
                .into()
        }
        None => tarjeta.into(),
    };
    let tarjeta=mouse_area(tarjeta)
        .on_enter((acciones.sobre)(Some(i)))
        .on_exit((acciones.sobre)(None));
    if let Some(editar)=acciones.editar{tarjeta.on_press(editar(i)).into()}else{tarjeta.into()}
}

/// Hueco rayado del alto de una fila: el sitio donde caerá la fila que se
/// arrastra.
fn hueco<'a, M: 'a>(p: Paleta) -> Element<'a, M> {
    stack![
        container(Space::new(Length::Fill, Length::Fixed(ALTO_FILA)))
            .style(estilo::bloque(Color::TRANSPARENT)),
        iced::widget::canvas(Hueco { color: p.border }).width(Length::Fill).height(ALTO_FILA)
    ]
    .into()
}

struct Hueco {
    color: Color,
}

impl<M> canvas::Program<M> for Hueco {
    type State = ();

    fn draw(
        &self,
        _: &(),
        renderer: &iced::Renderer,
        _: &iced::Theme,
        limites: Rectangle,
        _: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut marco = Frame::new(renderer, limites.size());
        let (w, h) = (limites.width, limites.height);
        let paso = 8.0 * std::f32::consts::SQRT_2;
        let mut x = -h;
        while x < w {
            marco.stroke(
                &Path::line(Point::new(x, h), Point::new(x + h, 0.0)),
                canvas::Stroke::default().with_color(self.color).with_width(1.0),
            );
            x += paso;
        }
        marco.stroke(
            &Path::rectangle(Point::new(0.5, 0.5), Size::new(w - 1.0, h - 1.0)),
            canvas::Stroke::default().with_color(self.color).with_width(1.0),
        );
        vec![marco.into_geometry()]
    }
}

/// Lista ordenable de servidores. Con `arrastre`, la fila tomada se levanta
/// 12 px a la derecha y 6 px arriba sobre su destino y el hueco rayado ocupa
/// su sitio; el resto se reacomoda sin transición.
pub fn lista_servidores<'a, M: Clone + 'a>(
    p: Paleta,
    servidores: &[Servidor],
    acciones: &AccionesLista<M>,
    arrastre: Option<Arrastre>,
    sobre: Option<usize>,
    colocando: Option<(usize, f32)>,
) -> Element<'a, M> {
    let vista_de = |i: usize| VistaFila {
        encima: sobre == Some(i) && arrastre.is_none(),
        levantada: false,
        colocando: colocando.filter(|(c, _)| *c == i).map(|(_, t)| t),
    };
    let mut filas = column![].spacing(SEPARACION);
    let Some(a) = arrastre else {
        for (i, s) in servidores.iter().enumerate() {
            filas = filas.push(tarjeta_servidor(p, i, s, acciones, vista_de(i)));
        }
        return filas.into();
    };
    let mut resto: Vec<usize> = (0..servidores.len()).filter(|&i| i != a.origen).collect();
    resto.insert(a.destino.min(resto.len()), usize::MAX);
    for i in resto {
        if i == usize::MAX {
            filas = filas.push(hueco(p));
        } else {
            filas = filas.push(tarjeta_servidor(p, i, &servidores[i], acciones, VistaFila::default()));
        }
    }
    let y = a.destino as f32 * (ALTO_FILA + SEPARACION);
    let levantada = tarjeta_servidor(
        p,
        a.origen,
        &servidores[a.origen],
        acciones,
        VistaFila { levantada: true, ..Default::default() },
    );
    let capa = column![
        Space::with_height(Length::Fixed((y - 6.0).max(0.0))),
        row![Space::with_width(Length::Fixed(12.0)), levantada]
    ];
    stack![filas, capa].into()
}

/// Bloque de la red privada (ZeroTier) que conecta a los amigos.
pub fn red_privada<'a, M: Clone + 'a>(
    p: Paleta,
    conectada: bool,
    red: &str,
    ip: &str,
    en_linea: (u32, u32),
    gestionar: M,
) -> Element<'a, M> {
    let titulo = if conectada {
        "Red privada conectada"
    } else {
        "Red privada desconectada"
    };
    let detalle = row![
        text(if ip.is_empty() {red.to_owned()} else {format!("{red} · tu IP ")})
            .size(texto::BODY_SM.0)
            .color(p.text_muted),
        text(ip.to_owned())
            .font(fuentes::MONO)
            .size(texto::MONO_SM.0)
            .color(p.text),
        text(if en_linea.1>0{format!(" · {} de {} en línea", en_linea.0, en_linea.1)}else{String::new()})
            .size(texto::BODY_SM.0)
            .color(p.text_muted),
    ]
    .align_y(Alignment::Center);
    // Marco de 88 × 58 con la máscara de Hermes de los hilos, sin deformarla.
    let marco = container(grabado_posicion(
        p,
        Lamina::HilosMascara,
        Length::Fill,
        Length::Fill,
        (0.5, 0.55),
    ))
    .width(88)
    .height(58)
    .padding(borde::MEDIO)
    .style(estilo::bloque(p.text));
    container(
        row![
            marco,
            column![
                row![
                    punto(p, conectada),
                    text(titulo)
                        .font(fuentes::CUERPO_NEGRITA)
                        .size(texto::HEADING.0)
                        .color(p.text),
                ]
                .spacing(espacio::S3)
                .align_y(Alignment::Center),
                detalle,
            ]
            .spacing(espacio::S1)
            .width(Length::Fill),
            componentes::boton_compacto(p, "Gestionar red", Variante::Secundario, Some(gestionar)),
        ]
        .spacing(espacio::S6)
        .align_y(Alignment::Center),
    )
    .padding(espacio::S6)
    .width(Length::Fill)
    .style(estilo::tarjeta(p, false))
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_destino_sigue_la_altura_del_puntero() {
        let paso = ALTO_FILA + SEPARACION;
        assert_eq!(Arrastre::destino_en(0.0, 4), 0);
        assert_eq!(Arrastre::destino_en(paso - 1.0, 4), 0);
        assert_eq!(Arrastre::destino_en(paso, 4), 1);
        assert_eq!(Arrastre::destino_en(paso * 2.5, 4), 2);
    }

    #[test]
    fn el_destino_no_sale_de_la_lista() {
        assert_eq!(Arrastre::destino_en(-500.0, 3), 0);
        assert_eq!(Arrastre::destino_en(5000.0, 3), 2);
        assert_eq!(Arrastre::destino_en(100.0, 0), 0);
    }

    #[test]
    fn el_anuncio_cuenta_desde_uno() {
        let a = Arrastre { origen: 0, destino: 1 };
        assert_eq!(a.anuncio("Amigos", 5), "Amigos, posición 2 de 5");
    }
}
