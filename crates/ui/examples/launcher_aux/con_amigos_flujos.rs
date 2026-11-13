// SPDX-License-Identifier: GPL-3.0-only
//! Con amigos › Compartir una instancia y Unirme con un código, como los
//! dibuja PantallaServidores: tres pasos con barra (Instancia, Qué compartes
//! y Código) y el código en tres grupos de 4 que se valida solo. Cada botón
//! lleva a la acción real de `compartir_real`.
use crate::compartir_real::{self as cr, Etapa};
use crate::con_amigos::{m, Accion};
use crate::servidores_vista::pixel;
use crate::{App, Mensaje};
use icaro_ui::{
    compartir as v,
    componentes::{self, Variante},
    fuentes,
    iconos::{icono, Icono, Tam},
    pantalla_servidores::{self as sv, Tono},
    tema::{borde, texto},
};
use iced::widget::{column, container, row, text, text_input, Space};
use iced::{Alignment, Element, Length, Padding};

/// Cuerpo desplazable y pie fijo abajo, dentro del panel.
fn con_pie<'a>(p: icaro_ui::tema::Paleta, cuerpo: Element<'a, Mensaje>, pie: Element<'a, Mensaje>) -> Element<'a, Mensaje> {
    column![
        container(icaro_ui::scroll::desplazable(cuerpo)).height(Length::Fill),
        sv::pie(p, pie),
    ]
    .spacing(18)
    .height(Length::Fill)
    .into()
}

fn negrita<'a>(s: impl Into<String>, c: iced::Color) -> Element<'a, Mensaje> {
    text(s.into()).font(fuentes::CUERPO_NEGRITA).size(texto::BODY_SM.0).color(c).into()
}

/// Paso del flujo de compartir según la etapa.
pub fn paso(e: Etapa) -> Option<usize> {
    match e {
        Etapa::Selector => Some(1),
        Etapa::Analizando | Etapa::Revision | Etapa::Guardando | Etapa::Propios => Some(2),
        Etapa::Publicando | Etapa::Listo => Some(3),
        _ => None,
    }
}

/// Contenido del paso actual (bajo la cabecera y la barra de pasos).
pub fn compartir(app: &App) -> Element<'_, Mensaje> {
    let s = &app.servicios.compartir;
    match s.etapa {
        Etapa::Selector => paso_instancia(app),
        Etapa::Analizando => analizando(app),
        Etapa::Revision if app.servicios.servidores.amigos.completa => revision_completa(app),
        Etapa::Revision | Etapa::Guardando => paso_que(app),
        Etapa::Publicando => preparando(app),
        Etapa::Listo => listo(app),
        // Archivos propios: su autorización, como siempre.
        _ => cr::fase_pura(app)
            .map(|f| f.map(|a| Mensaje::Compartir(cr::Accion::Vista(a))))
            .unwrap_or_else(|| Space::new(0, 0).into()),
    }
}

fn paso_instancia(app: &App) -> Element<'_, Mensaje> {
    let p = app.paleta();
    let modelo = cr::modelo_selector(app);
    let mut lista = column![].spacing(6);
    if modelo.opciones.is_empty() {
        lista = lista.push(sv::cuerpo_sm("No tienes instancias. Crea una para compartirla.", p.text_muted));
    }
    for o in &modelo.opciones {
        let detalle = [o.version.as_str(), o.mods.as_str()].iter().filter(|t| !t.is_empty()).cloned().collect::<Vec<_>>().join(" · ");
        let motivo: Option<Element<'_, Mensaje>> = match o.estado {
            v::EstadoOpcion::Compartida { .. } => Some(sv::cuerpo_sm("Ya la compartes", p.text_muted)),
            v::EstadoOpcion::EnUso => Some(sv::cuerpo_sm("Minecraft abierto", p.warning)),
            _ => None,
        };
        let contenido = row![
            pixel(p, &o.nombre, 32.0),
            column![negrita(o.nombre.clone(), p.text), sv::mono_sm(detalle, p.text_muted)].spacing(2),
        ]
        .spacing(14)
        .align_y(Alignment::Center)
        .into();
        lista = lista.push(sv::opcion(
            p,
            modelo.elegida.as_deref() == Some(o.id.as_str()),
            contenido,
            motivo,
            o.elegible().then(|| m(Accion::Opcion(o.id.clone()))),
        ));
    }
    let elegida = modelo.elegida.as_ref().and_then(|id| modelo.opciones.iter().find(|o| &o.id == id));
    let frase: Element<'_, Mensaje> = match elegida {
        Some(o) => row![sv::cuerpo_sm("Vas a compartir ", p.text_muted), negrita(o.nombre.clone(), p.text)].into(),
        None => sv::cuerpo_sm("Elige una instancia", p.text_muted),
    };
    con_pie(
        p,
        lista.into(),
        row![
            container(frase).width(Length::Fill),
            componentes::boton_con_icono(p, Icono::Adelante, "Continuar", Variante::Primario, elegida.map(|_| m(Accion::Continuar)), false),
        ]
        .align_y(Alignment::Center)
        .into(),
    )
}

fn nombre_instancia(app: &App) -> String {
    let s = &app.servicios.compartir;
    s.seleccionada
        .as_ref()
        .and_then(|id| app.servicios.instancias.registros.iter().find(|i| &i.id == id))
        .map(|i| i.nombre.clone())
        .or_else(|| s.revision.as_ref().map(|r| r.instancia.nombre.clone()))
        .unwrap_or_else(|| "la instancia".into())
}

fn barra_progreso<'a>(p: icaro_ui::tema::Paleta, titulo: String, avance: f32, nota: &str) -> Element<'a, Mensaje> {
    column![
        row![
            sv::cuerpo_sm(titulo, p.text),
            Space::with_width(Length::Fill),
            sv::mono_sm(format!("{} %", (avance * 100.0).round() as u32), p.text_muted),
        ],
        componentes::progreso(p, avance),
        sv::cuerpo_sm(nota.to_owned(), p.text_muted),
    ]
    .spacing(12)
    .into()
}

fn analizando(app: &App) -> Element<'_, Mensaje> {
    let p = app.paleta();
    let s = &app.servicios.compartir;
    if let Some(e) = &s.error {
        return sv::nota(
            p,
            Tono::Error,
            Icono::Alerta,
            sv::cuerpo_sm(e.clone(), p.text),
            Some(componentes::boton_compacto(p, "Reintentar", Variante::Secundario, Some(Mensaje::Compartir(cr::Accion::Vista(v::Accion::Reintentar))))),
        );
    }
    let (hechos, total) = match s.revision.as_ref().map(|r| &r.estado) {
        Some(v::EstadoRevisionPublicacion::Analizando { hechos, total }) => (*hechos, *total),
        _ => (0, 0),
    };
    barra_progreso(
        p,
        format!("Revisando {}", nombre_instancia(app)),
        if total == 0 { 0.0 } else { hechos as f32 / total as f32 },
        "Leemos los mods, configuraciones y scripts para proponer qué compartir.",
    )
}

/// Cuántos de cada tipo se publican y cuántos hay.
fn por_tipo(r: &v::RevisionPublicacion) -> [(usize, usize); 5] {
    let mut c = [(0, 0); 5];
    for x in r.cambios.iter().filter(|x| !x.integrado) {
        let i = v::TipoContenido::TODOS.iter().position(|t| *t == x.tipo).unwrap_or(0);
        c[i].1 += 1;
        if x.marcado() {
            c[i].0 += 1;
        }
    }
    c
}

fn paso_que(app: &App) -> Element<'_, Mensaje> {
    let p = app.paleta();
    let s = &app.servicios.compartir;
    let a = &app.servicios.servidores.amigos;
    let Some(r) = s.revision.as_ref() else {
        return row![icaro_ui::progreso::spinner(p, app.reducir), sv::cuerpo_sm("Guardando…", p.text_muted)].spacing(10).into();
    };
    let activa = if a.mano {
        2
    } else {
        match r.vista.atajo {
            Some(v::AtajoPublicacion::Todo) => 1,
            _ => 0,
        }
    };
    let explicacion = match activa {
        0 => "Mods del servidor, sus configuraciones y scripts. Quedan fuera shaders y mods solo visuales. Recomendado.",
        1 => "Todo tal como está tu instancia, incluidos shaders y resourcepacks.",
        _ => "Abre la revisión completa para marcar uno por uno.",
    };
    let cuenta = por_tipo(r);
    let nombres = ["Mods", "Configuraciones", "Resourcepacks", "Shaders", "Scripts"];
    let mut tipos = row![].spacing(8);
    for (i, nombre) in nombres.iter().enumerate() {
        let (si, total) = cuenta[i];
        let on = si > 0;
        tipos = tipos.push(
            container(
                column![
                    row![
                        text(si.to_string()).font(fuentes::MONO).size(texto::MONO.0).color(p.text),
                        text(format!(" / {total}")).font(fuentes::MONO).size(texto::MONO.0).color(p.text_muted),
                    ],
                    text(*nombre).font(fuentes::CUERPO).size(12).color(p.text_muted).wrapping(iced::widget::text::Wrapping::None),
                ]
                .spacing(4),
            )
            .clip(true)
            .padding(Padding::from([12.0, 10.0]))
            .width(Length::FillPortion(1))
            .style(sv::caja(Some(if on { p.surface } else { p.surface_sunken }), if on { p.text } else { p.border }, borde::FINO)),
        );
    }
    let mut que = column![
        sv::rotulo(p, "Qué reciben tus amigos"),
        componentes::segmentado(p, &["Lo necesario para jugar", "Todo", "Elegir a mano"], activa, |i| match i {
            0 => m(Accion::Atajo(v::AtajoPublicacion::Necesario)),
            1 => m(Accion::Atajo(v::AtajoPublicacion::Todo)),
            _ => m(Accion::Mano),
        }),
        sv::cuerpo_sm(explicacion, p.text_muted),
        tipos,
    ]
    .spacing(12);
    if a.mano {
        que = que.push(componentes::boton_con_icono(p, Icono::Ajustes, "Abrir la revisión completa", Variante::Secundario, Some(m(Accion::RevisionCompleta(true))), true));
    }
    let activa_cad = v::Caducidad::TODAS.iter().position(|c| *c == s.caducidad).unwrap_or(0);
    let ajustes = row![
        container(
            column![
                sv::rotulo(p, "El código caduca en"),
                componentes::segmentado_compacto(p, &["24 horas", "2 días", "1 semana"], activa_cad, |i| m(Accion::FCaducidad(v::Caducidad::TODAS[i]))),
            ]
            .spacing(12)
        )
        .width(Length::Fill),
        container(
            column![
                sv::rotulo(p, "Pueden unirse"),
                row![
                    componentes::contador(
                        p,
                        s.cupo as i32,
                        if s.cupo > 1 { m(Accion::FCupo(s.cupo - 1)) } else { Mensaje::Nada },
                        if s.cupo < 50 { m(Accion::FCupo(s.cupo + 1)) } else { Mensaje::Nada },
                    ),
                    sv::cuerpo_sm("personas", p.text_muted),
                ]
                .spacing(16)
                .align_y(Alignment::Center),
            ]
            .spacing(12)
        )
        .width(Length::Fill),
    ]
    .spacing(24);
    let puede = !r.guardando
        && s.etapa == Etapa::Revision
        && matches!(r.estado, v::EstadoRevisionPublicacion::Disponible | v::EstadoRevisionPublicacion::SinCambios);
    let mut cuerpo = column![que, ajustes].spacing(22);
    if let v::EstadoRevisionPublicacion::Error(e) = &r.estado {
        cuerpo = cuerpo.push(sv::nota(p, Tono::Error, Icono::Alerta, sv::cuerpo_sm(e.clone(), p.text), None));
    }
    con_pie(
        p,
        cuerpo.into(),
        row![
            componentes::boton_con_icono(p, Icono::Atras, "Atrás", Variante::Secundario, Some(m(Accion::Atras)), false),
            container(sv::cuerpo_sm(v::frase_publicacion(r), p.text_muted)).width(Length::Fill).align_x(Alignment::End),
            componentes::boton_con_icono(p, Icono::Subir, "Compartir", Variante::Primario, puede.then(|| Mensaje::Compartir(cr::Accion::Vista(v::Accion::Compartir))), false),
        ]
        .spacing(12)
        .align_y(Alignment::Center)
        .into(),
    )
}

/// «Elegir a mano»: la revisión completa, con un enlace para volver.
fn revision_completa(app: &App) -> Element<'_, Mensaje> {
    let p = app.paleta();
    let mut c = column![componentes::boton_con_icono(p, Icono::Atras, "Volver al resumen", Variante::Fantasma, Some(m(Accion::RevisionCompleta(false))), true)].spacing(12).height(Length::Fill);
    if let Some(f) = cr::fase_pura(app) {
        c = c.push(container(f.map(|a| Mensaje::Compartir(cr::Accion::Vista(a)))).height(Length::Fill));
    }
    c.into()
}

fn preparando(app: &App) -> Element<'_, Mensaje> {
    let p = app.paleta();
    let avance = match app.servicios.compartir.revision.as_ref().map(|r| &r.estado) {
        Some(v::EstadoRevisionPublicacion::Preparando { porcentaje }) => porcentaje / 100.0,
        _ => 0.0,
    };
    barra_progreso(p, format!("Preparando {}", nombre_instancia(app)), avance.clamp(0.0, 1.0), "Puedes cambiar de pestaña; sigue en segundo plano.")
}

fn listo(app: &App) -> Element<'_, Mensaje> {
    let p = app.paleta();
    let s = &app.servicios.compartir;
    let Some(l) = cr::modelo_listo(s) else {
        return preparando(app);
    };
    let marca = container(icono(Icono::Check, Tam::Lg, p.bg)).center_x(48).center_y(48).style(sv::caja(Some(p.success), p.success, 0.0));
    let titulo = column![
        sv::titulo(format!("Ya compartes {}", l.instancia), p.text),
        sv::cuerpo_sm(
            format!(
                "Envía el código. Caduca en {} y sirve para {} {}.",
                l.caducidad.texto(),
                l.cupo,
                if l.cupo == 1 { "persona" } else { "personas" }
            ),
            p.text_muted,
        ),
    ]
    .spacing(4);
    let generando = matches!(l.estado, v::EstadoCodigo::Generando);
    let codigo: Element<'_, Mensaje> = if generando {
        row![icaro_ui::progreso::spinner(p, app.reducir), sv::cuerpo_sm("Generando el código…", p.text_muted)].spacing(10).into()
    } else {
        v::celdas(p, &l.codigo, !matches!(l.estado, v::EstadoCodigo::Vigente | v::EstadoCodigo::Nuevo))
    };
    let vista = |a: v::Accion| Mensaje::Compartir(cr::Accion::Vista(a));
    let bloque = container(
        column![
            codigo,
            row![
                componentes::boton_con_icono(p, if l.copiado { Icono::Check } else { Icono::Copiar }, if l.copiado { "Copiado" } else { "Copiar código" }, Variante::Primario, (!generando).then(|| vista(v::Accion::CopiarCodigo)), true),
                componentes::boton_con_icono(p, if l.copiado_mensaje { Icono::Check } else { Icono::Documento }, if l.copiado_mensaje { "Copiado" } else { "Copiar mensaje" }, Variante::Secundario, (!generando).then(|| vista(v::Accion::CopiarMensaje)), true),
            ]
            .spacing(8),
        ]
        .spacing(12),
    )
    .padding(20)
    .width(Length::Fill)
    .style(sv::caja(Some(p.surface_sunken), p.border, borde::FINO));
    con_pie(
        p,
        column![row![marca, titulo].spacing(18).align_y(Alignment::Center), bloque].spacing(22).into(),
        row![
            Space::with_width(Length::Fill),
            componentes::boton_con_icono(p, Icono::Adelante, "Ver publicación", Variante::Primario, Some(m(Accion::VerNueva)), false),
        ]
        .into(),
    )
}

/// Unirme con un código; `None` deja la fase como estaba (revisión de lo que
/// se instala y su progreso).
pub fn unirme(app: &App) -> Option<Element<'_, Mensaje>> {
    let s = &app.servicios.compartir;
    match s.etapa {
        Etapa::Entrada => Some(entrada(app)),
        Etapa::Identidad => Some(invitacion(app)),
        Etapa::Destino if s.invitado.auto && s.invitado.error.is_none() => {
            let p = app.paleta();
            Some(row![icaro_ui::progreso::spinner(p, app.reducir), sv::cuerpo_sm("Preparando la instancia…", p.text_muted)].spacing(10).into())
        }
        _ => None,
    }
}

/// Tres campos de 4 que avanzan solos y reparten lo pegado.
fn entrada(app: &App) -> Element<'_, Mensaje> {
    let p = app.paleta();
    let s = &app.servicios.compartir;
    let a = &app.servicios.servidores.amigos;
    let estado = &s.invitado.entrada;
    let error = matches!(
        estado,
        v::EstadoEntrada::Invalido | v::EstadoEntrada::Vencido { .. } | v::EstadoEntrada::Completo { .. } | v::EstadoEntrada::Inaccesible(_)
    );
    let mut campos = row![].spacing(8).align_y(Alignment::Center);
    for i in 0..3 {
        if i > 0 {
            campos = campos.push(text("-").font(fuentes::MONO).size(20).color(p.text_muted));
        }
        let entrada = text_input("XXXX", &a.grupos[i])
            .id(text_input::Id::new(ID_GRUPO[i]))
            .on_input(move |t| m(Accion::Grupo(i, t)))
            .font(fuentes::MONO_MEDIO)
            .size(22)
            .width(104)
            .padding(Padding::from([8.0, 12.0]))
            .style(move |tema, e| {
                let mut st = icaro_ui::estilo::campo(p)(tema, e);
                st.border.width = borde::MEDIO;
                if error {
                    st.border.color = p.error;
                } else if matches!(estado, v::EstadoEntrada::Valido { .. }) {
                    st.border.color = p.success;
                }
                st
            });
        campos = campos.push(entrada);
    }
    campos = campos.push(componentes::boton_compacto(p, "Pegar", Variante::Fantasma, Some(Mensaje::Compartir(cr::Accion::Vista(v::Accion::PegarCodigo)))));
    let mut c = column![campos].spacing(12);
    match estado {
        v::EstadoEntrada::Validando => {
            c = c.push(row![icaro_ui::progreso::spinner(p, app.reducir), sv::cuerpo_sm("Buscando la invitación…", p.text_muted)].spacing(10));
        }
        v::EstadoEntrada::Valido { .. } => {
            c = c.push(row![icaro_ui::progreso::spinner(p, app.reducir), sv::cuerpo_sm("Consultando a quien comparte…", p.text_muted)].spacing(10));
        }
        v::EstadoEntrada::Invalido | v::EstadoEntrada::Vencido { .. } => {
            c = c.push(sv::nota(p, Tono::Error, Icono::Alerta, sv::cuerpo_sm("Ese código no existe o ya venció. Pídele uno nuevo a quien te invitó.", p.text), None));
        }
        v::EstadoEntrada::Completo { .. } => {
            c = c.push(sv::nota(p, Tono::Error, Icono::Alerta, sv::cuerpo_sm("Esa invitación ya no tiene cupo. Pídele a quien te invitó que la amplíe.", p.text), None));
        }
        v::EstadoEntrada::Inaccesible(e) => {
            c = c.push(sv::nota(p, Tono::Error, Icono::Alerta, sv::cuerpo_sm(e.clone(), p.text), None));
        }
        v::EstadoEntrada::Vacio => {}
    }
    c.into()
}

pub const ID_GRUPO: [&str; 3] = ["codigo-0", "codigo-1", "codigo-2"];

/// La tarjeta de quien invita y dónde se instala.
fn invitacion(app: &App) -> Element<'_, Mensaje> {
    let p = app.paleta();
    let s = &app.servicios.compartir;
    let Some(d) = cr::datos_visita(s) else {
        return Space::new(0, 0).into();
    };
    let nombre = s.invitado.nombre.clone();
    let tamano = cr::tamano(d.bytes);
    let tarjeta = container(
        row![
            pixel(p, &nombre, 40.0),
            column![
                row![negrita("Equipo anfitrión", p.text), sv::cuerpo_sm(" comparte ", p.text), negrita(nombre.clone(), p.text)],
                sv::mono_sm(format!("{} · {} · {} mods · {} · revisión {}", d.minecraft, d.loader, d.mods, tamano, d.revision), p.text_muted),
                sv::mono_sm(format!("Huella {}", d.huella), p.text_muted),
            ]
            .spacing(3),
        ]
        .spacing(16)
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([16.0, 18.0]))
    .width(Length::Fill)
    .style(sv::caja(Some(p.accent_soft), p.text, borde::MEDIO));
    let mut cuerpo = column![tarjeta].spacing(22);
    let mut puede = cr::puede_confiar(s) && !s.red_ocupada;
    match &s.invitado.confianza {
        v::Confianza::Conocido { fecha } => {
            cuerpo = cuerpo.push(sv::nota(p, Tono::Exito, Icono::Check, sv::cuerpo_sm(format!("Ya confiaste en este equipo el {fecha}. La huella coincide."), p.text), None));
        }
        v::Confianza::Distinto { .. } => {
            cuerpo = cuerpo.push(sv::nota(
                p,
                Tono::Alerta,
                Icono::Aviso,
                sv::cuerpo_sm("La huella no coincide con la que confirmaste antes para esta instancia. Compárala con tu amigo antes de seguir.", p.text),
                Some(componentes::boton_compacto(
                    p,
                    if s.invitado.confirmar_nueva { "Confirmada" } else { "Es la correcta" },
                    Variante::Secundario,
                    Some(m(Accion::ConfirmarHuella(!s.invitado.confirmar_nueva))),
                )),
            ));
            puede &= s.invitado.confirmar_nueva;
        }
        _ => {}
    }
    let nueva = row![column![
        negrita(format!("Instancia nueva \"{nombre}\""), p.text),
        sv::cuerpo_sm("Recomendado. No toca tus otras instancias.", p.text_muted),
    ]
    .spacing(2)]
    .into();
    let reemplazar = row![column![
        negrita("Reemplazar los mods de una instancia que ya tienes", p.text_muted),
        sv::cuerpo_sm("Tus mundos se conservan.", p.text_muted),
    ]
    .spacing(2)]
    .into();
    cuerpo = cuerpo.push(
        column![
            sv::rotulo(p, "Dónde se instala"),
            sv::opcion(p, true, nueva, None, Some(Mensaje::Nada)),
            sv::opcion(p, false, reemplazar, Some(sv::cuerpo_sm("Aún no disponible", p.text_muted)), None),
        ]
        .spacing(6),
    );
    if let Some(e) = &s.invitado.error {
        cuerpo = cuerpo.push(sv::nota(p, Tono::Error, Icono::Alerta, sv::cuerpo_sm(e.clone(), p.text), None));
    }
    con_pie(
        p,
        cuerpo.into(),
        row![
            Space::with_width(Length::Fill),
            componentes::boton_con_icono(p, Icono::Descargas, &format!("Unirme y descargar {tamano}"), Variante::Primario, puede.then(|| m(Accion::UnirmeDescargar)), false),
        ]
        .into(),
    )
}

/// «AB12CD» repartido en grupos de 4 (lo pegado llena los tres).
pub fn repartir(grupos: &mut [String; 3], i: usize, texto: &str) {
    let limpio: String = texto.chars().filter(|c| c.is_ascii_alphanumeric()).map(|c| c.to_ascii_uppercase()).collect();
    if limpio.chars().count() > 4 {
        let todo: Vec<char> = limpio.chars().collect();
        for (k, g) in grupos.iter_mut().enumerate().skip(i) {
            let desde = (k - i) * 4;
            *g = todo.iter().skip(desde).take(4).collect();
        }
    } else {
        grupos[i] = limpio;
    }
}

#[cfg(test)]
mod pruebas {
    use super::repartir;

    #[test]
    fn lo_pegado_se_reparte_en_los_tres_grupos() {
        let mut g: [String; 3] = Default::default();
        repartir(&mut g, 0, "7qkm-4xpt-r2c9");
        assert_eq!(g, ["7QKM".to_owned(), "4XPT".into(), "R2C9".into()]);
        repartir(&mut g, 1, "ab");
        assert_eq!(g[1], "AB");
    }
}
