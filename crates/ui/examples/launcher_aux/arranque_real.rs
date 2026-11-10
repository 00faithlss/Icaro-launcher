// SPDX-License-Identifier: GPL-3.0-only
//! AjusteArranque (Ajustes › Apariencia) y la capa de ArranqueSuave al pulsar
//! Jugar. El Umbral sigue siendo una de las opciones.
use crate::{App, Mensaje};
use icaro_ui::ajustes::fila_ajuste;
use icaro_ui::arranque::{self as a, Estilo, Tono};
use icaro_ui::componentes::{self, Variante};
use icaro_ui::iconos::Icono;
use icaro_ui::tema::{borde, espacio, texto, Paleta};
use icaro_ui::fuentes;
use iced::widget::{button, column, container, row, text, Space};
use iced::{Alignment, Background, Border, Element, Length, Task};
use std::sync::OnceLock;
use std::time::Instant;

#[derive(Debug, Clone)]
pub enum Accion {
    Elegir(Estilo),
    Tono(Tono),
    Azar(bool),
    Frase(bool),
    Registro(bool),
    /// Reproduce la transición marcada sin abrir Minecraft.
    Probar,
}

pub fn estilo_de(s: &str) -> Estilo {
    Estilo::TODOS
        .into_iter()
        .find(|e| texto_estilo(*e) == s)
        .unwrap_or(Estilo::Umbral)
}
pub fn texto_estilo(e: Estilo) -> &'static str {
    match e {
        Estilo::Umbral => "umbral",
        Estilo::Marea => "marea",
        Estilo::Horizonte => "horizonte",
        Estilo::Tinta => "tinta",
        Estilo::Bloques => "bloques",
        Estilo::Respiro => "respiro",
    }
}
pub fn tono_de(s: &str) -> Tono {
    match s {
        "claro" => Tono::Claro,
        "oscuro" => Tono::Oscuro,
        _ => Tono::Tema,
    }
}
pub fn texto_tono(t: Tono) -> &'static str {
    match t {
        Tono::Tema => "tema",
        Tono::Claro => "claro",
        Tono::Oscuro => "oscuro",
    }
}

/// Transición de esta pulsación de Jugar: la de prueba, una al azar entre
/// las suaves o la marcada.
pub fn estilo_para(app: &App, pulsado: Instant) -> Estilo {
    if let Some(e) = app.arranque_prueba {
        return e;
    }
    if app.aj.arranque_azar {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        pulsado.hash(&mut h);
        return Estilo::SUAVES[(h.finish() % Estilo::SUAVES.len() as u64) as usize];
    }
    app.aj.arranque
}

/// Lo que muestra la capa: instancia elegida, frase y registro.
pub fn contenido(app: &App) -> a::Contenido {
    let datos = app
        .seleccionada
        .and_then(|i| crate::con_instancias(|l| l.get(i).cloned()));
    let nombre = datos.as_ref().map_or_else(|| "Minecraft".into(), |d| d.nombre.clone());
    let version = datos.as_ref().map_or_else(String::new, |d| d.version.clone());
    let frase = app.aj.frase_al_cargar.then(|| {
        crate::saludo::datos(app)
            .frase
            .iter()
            .map(|t| match t {
                icaro_ui::saludo::Trozo::Texto(s) | icaro_ui::saludo::Trozo::Nombre(s) => s.as_str(),
            })
            .collect::<String>()
    });
    let registro = if !app.aj.registro_al_cargar {
        Vec::new()
    } else if app.servicios.demo || app.arranque_prueba.is_some() {
        vec![
            "Java 21.0.5 · 6.144 MB asignados".into(),
            "214 mods verificados · 0 conflictos".into(),
            "Cargando Fabric Loader 0.16.9".into(),
            "Minecraft 1.21.4 arrancando".into(),
        ]
    } else {
        let lineas = crate::juego_real::lineas(app);
        lineas
            .iter()
            .rev()
            .take(4)
            .rev()
            .map(|l| l.mensaje.clone())
            .collect()
    };
    a::Contenido {
        preparando: if version.is_empty() {
            "Preparando".into()
        } else {
            format!("Preparando · {version}")
        },
        icono: Some(icaro_core::icono_instancia::patron(&nombre, 0)),
        nombre,
        frase,
        registro,
    }
}

fn metal(app: &App) -> icaro_ui::tema::Metal {
    icaro_ui::tema::Metal::desde_indice(app.aj.metal)
}

/// Capa a pantalla completa de una transición suave.
pub fn capa<'a>(app: &'a App, estilo: Estilo, ms: f32, retirada: Option<f32>) -> Element<'a, Mensaje> {
    a::capa(
        app.paleta(),
        estilo,
        app.aj.tono_arranque,
        metal(app),
        ms,
        icaro_ui::movimiento::Preferencia { reducir: app.reducir },
        contenido(app),
        retirada,
        Mensaje::UmbralVolver,
        Some(Mensaje::UmbralCancelar),
    )
}

/// Las miniaturas se animan solo mientras Apariencia está a la vista.
pub fn animando(app: &App) -> bool {
    !app.reducir
        && app.seccion == crate::Seccion::Ajustes
        && app.ajuste == 1
        && !app.novedades.abierta
        && app.pantalla.is_none()
}

fn reloj() -> Instant {
    static INICIO: OnceLock<Instant> = OnceLock::new();
    *INICIO.get_or_init(Instant::now)
}

/// Tarjeta de selección: miniatura 16:9, radio, nombre, etiqueta y línea.
fn tarjeta<'a>(app: &'a App, p: Paleta, e: Estilo) -> Element<'a, Mensaje> {
    let marcada = app.aj.arranque == e;
    let ms = app.ahora.saturating_duration_since(reloj()).as_secs_f32() * 1000.0;
    let mini = container(a::miniatura(
        p,
        e,
        app.aj.tono_arranque,
        metal(app),
        ms,
        app.reducir,
        icaro_core::icono_instancia::patron("Supervivencia", 0),
    ))
    .width(Length::Fill)
    .height(Length::Fixed(135.0));
    let info = container(
        column![
            row![
                componentes::rombo(p, marcada, false),
                text(e.nombre())
                    .font(fuentes::TITULO)
                    .size(15)
                    .color(p.text),
                Space::with_width(Length::Fill),
                text(e.etiqueta())
                    .font(fuentes::MONO)
                    .size(texto::MONO_SM.0)
                    .color(p.text_muted),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
            text(e.descripcion())
                .size(texto::BODY_SM.0)
                .color(p.text_muted),
        ]
        .spacing(espacio::S2),
    )
    .width(Length::Fill)
    .height(Length::Fixed(100.0))
    .padding([espacio::S3, espacio::S4])
    .style(move |_: &iced::Theme| iced::widget::container::Style {
        background: marcada.then_some(Background::Color(p.accent_soft)),
        ..Default::default()
    });
    let grosor = if marcada { borde::MEDIO } else { borde::FINO };
    button(column![mini, info])
        .width(Length::FillPortion(1))
        .padding(grosor)
        .on_press(Mensaje::Arranque(Accion::Elegir(e)))
        .style(move |_, estado| button::Style {
            background: Some(Background::Color(p.surface)),
            text_color: p.text,
            border: Border {
                color: if marcada {
                    p.text
                } else if matches!(estado, button::Status::Hovered) {
                    p.border_strong
                } else {
                    p.border
                },
                width: grosor,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into()
}

/// AjusteArranque: cabecera, seis tarjetas, ajustes y «Probar».
pub fn seccion(app: &App) -> Element<'_, Mensaje> {
    let p = app.paleta();
    let ajustes = &app.aj;
    let cab = column![
        text("Al pulsar Jugar")
            .font(fuentes::TITULO)
            .size(28)
            .line_height(iced::widget::text::LineHeight::Absolute(32.0.into()))
            .color(p.text),
        text("Lo que ves mientras Minecraft arranca. Todas respetan reducir movimiento.")
            .size(texto::BODY_SM.0)
            .color(p.text_muted),
    ]
    .spacing(espacio::S1);
    let mut rejilla = column![].spacing(espacio::S4);
    for fila in Estilo::TODOS.chunks(3) {
        let mut r = row![].spacing(espacio::S4);
        for e in fila {
            r = r.push(tarjeta(app, p, *e));
        }
        rejilla = rejilla.push(r);
    }
    let indice_tono = Tono::TODOS.iter().position(|t| *t == ajustes.tono_arranque).unwrap_or(0);
    let opciones = column![
        fila_ajuste(
            p,
            "Tono de la transición",
            "Puede seguir el tema de Ícaro o ser siempre clara u oscura. El Umbral siempre usa la luz del arco.",
            componentes::segmentado_compacto(
                p,
                &Tono::TODOS.map(Tono::nombre),
                indice_tono,
                |i| Mensaje::Arranque(Accion::Tono(Tono::TODOS[i])),
            ),
        ),
        fila_ajuste(
            p,
            "Una distinta cada vez",
            "Elige al azar entre las suaves en cada partida. La que marcaste arriba se usa para Probar.",
            componentes::interruptor(p, ajustes.arranque_azar, Mensaje::Arranque(Accion::Azar(!ajustes.arranque_azar))),
        ),
        fila_ajuste(
            p,
            "Mostrar una frase mientras carga",
            "Usa las mismas frases del saludo de inicio.",
            componentes::interruptor(p, ajustes.frase_al_cargar, Mensaje::Arranque(Accion::Frase(!ajustes.frase_al_cargar))),
        ),
        fila_ajuste(
            p,
            "Mostrar el registro de arranque",
            "Las cuatro líneas de Java, mods y loader. Útil si algo tarda.",
            componentes::interruptor(p, ajustes.registro_al_cargar, Mensaje::Arranque(Accion::Registro(!ajustes.registro_al_cargar))),
        ),
    ];
    let probar = row![
        componentes::boton_con_icono(
            p,
            Icono::Jugar,
            &format!("Probar {}", ajustes.arranque.nombre()),
            Variante::Secundario,
            Some(Mensaje::Arranque(Accion::Probar)),
            false,
        ),
        text("Se reproduce sobre esta pantalla sin abrir Minecraft.")
            .size(texto::BODY_SM.0)
            .color(p.text_muted),
    ]
    .spacing(espacio::S2)
    .align_y(Alignment::Center);
    column![cab, rejilla, opciones, probar]
        .spacing(espacio::S6)
        .width(Length::Fill)
        .into()
}

pub fn actualizar(app: &mut App, accion: Accion) -> Task<Mensaje> {
    match accion {
        Accion::Elegir(e) => app.aj.arranque = e,
        Accion::Tono(t) => app.aj.tono_arranque = t,
        Accion::Azar(v) => app.aj.arranque_azar = v,
        Accion::Frase(v) => app.aj.frase_al_cargar = v,
        Accion::Registro(v) => app.aj.registro_al_cargar = v,
        Accion::Probar => {
            let ahora = Instant::now();
            app.arranque_prueba = Some(app.aj.arranque);
            app.umbral = Some(ahora);
            app.ahora = ahora;
        }
    }
    Task::none()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_textos_guardados_vuelven_al_mismo_estilo_y_tono() {
        for e in Estilo::TODOS {
            assert_eq!(estilo_de(texto_estilo(e)), e);
        }
        for t in Tono::TODOS {
            assert_eq!(tono_de(texto_tono(t)), t);
        }
        assert_eq!(estilo_de("desconocido"), Estilo::Umbral);
    }
}
