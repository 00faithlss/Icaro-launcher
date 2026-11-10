// SPDX-License-Identifier: GPL-3.0-only
//! PantallaServidores del System Design: banda de 150 px con «Fase
//! experimental», pestañas Servidores, Con amigos y Red privada, y el
//! contenido de cada una. Todo se abre en la misma página, sin diálogos.
use crate::servidores_reales::{self as sr, Accion, Prueba};
use crate::{App, Mensaje};
use icaro_core::servidores;
use icaro_ui::{
    componentes::{self, Variante},
    estilo, fuentes,
    iconos::{icono, Icono, Tam},
    laminas::{banda_completa, DatoBanda, Lamina},
    pantalla_servidores::{self as sv, Pestana, Tono},
    tema::{borde, espacio, medida, texto, Paleta},
};
use iced::widget::{button, column, container, pick_list, row, stack, text, text_input, Space};
use iced::{Alignment, Element, Length, Padding};
use std::time::Instant;

fn m(a: Accion) -> Mensaje {
    Mensaje::Servidores(a)
}

/// Icono de píxeles a partir de un nombre.
pub fn pixel<'a>(p: Paleta, nombre: &str, lado: f32) -> Element<'a, Mensaje> {
    sv::pixel(p, icaro_core::icono_instancia::patron(nombre, 0), lado)
}

/// Opción de «Jugar con»: una instancia o elegir según la versión.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpcionInstancia(pub Option<sr::Instancia>);
impl std::fmt::Display for OpcionInstancia {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.0 {
            Some(i) => write!(f, "{}", i.nombre),
            None => f.write_str("Elegir según la versión"),
        }
    }
}

fn opciones_instancia(app: &App) -> Vec<OpcionInstancia> {
    std::iter::once(OpcionInstancia(None))
        .chain(app.servicios.instancias.registros.iter().map(|i| {
            OpcionInstancia(Some(sr::Instancia { id: i.id.clone(), nombre: i.nombre.clone() }))
        }))
        .collect()
}

/// Campo con rótulo arriba (`ic-campo`).
fn campo<'a>(p: Paleta, rotulo: &str, opcional: bool, control: Element<'a, Mensaje>) -> Element<'a, Mensaje> {
    let mut cab = row![sv::rotulo(p, rotulo)].spacing(6).align_y(Alignment::Center);
    if opcional {
        cab = cab.push(text("(opcional)").font(fuentes::CUERPO).size(texto::LABEL.0).line_height(iced::widget::text::LineHeight::Absolute(texto::LABEL.1.into())).color(p.text_muted));
    }
    column![cab, control].spacing(espacio::S2).width(Length::Fill).into()
}

fn entrada<'a>(
    p: Paleta,
    id: &'static str,
    marcador: &str,
    valor: &str,
    mono: bool,
    error: bool,
    al_cambiar: impl Fn(String) -> Mensaje + 'a,
    al_enviar: Option<Mensaje>,
) -> Element<'a, Mensaje> {
    let mut e = text_input(marcador, valor)
        .id(text_input::Id::new(id))
        .on_input(al_cambiar)
        .font(if mono { fuentes::MONO } else { fuentes::CUERPO })
        .size(if mono { texto::MONO.0 } else { texto::BODY.0 })
        .padding(Padding::from([8.0, espacio::S3]))
        .style(move |tema, estado| {
            let mut s = estilo::campo(p)(tema, estado);
            if error {
                s.border.color = p.error;
            }
            s
        });
    if let Some(enviar) = al_enviar {
        e = e.on_submit(enviar);
    }
    container(e).height(medida::CONTROL).center_y(medida::CONTROL).into()
}

fn selector_instancia<'a>(
    p: Paleta,
    app: &App,
    elegida: Option<&sr::Instancia>,
) -> Element<'a, Mensaje> {
    let actual = OpcionInstancia(elegida.cloned());
    pick_list(opciones_instancia(app), Some(actual), |o: OpcionInstancia| match o.0 {
        Some(i) => m(Accion::Instancia(i)),
        None => Mensaje::Nada,
    })
    .width(Length::Fill)
    .padding(Padding::from([8.0, espacio::S3]))
    .text_size(texto::BODY.0)
    .style(estilo::selector(p))
    .menu_style(estilo::menu_selector(p))
    .into()
}

/// Sección Servidores completa.
pub fn vista(app: &App) -> Element<'_, Mensaje> {
    let p = app.paleta();
    let s = &app.servicios.servidores;
    let banda = stack![
        banda_completa(
            p,
            Lamina::TivoliCiudad,
            Some((0.5, 0.4)),
            "Servidores",
            DatoBanda::Ninguno,
            150.0,
            texto::DISPLAY_XL,
        ),
        // Crédito de la banda: Fase experimental.
        container(
            container(
                row![icono(Icono::Aviso, Tam::Sm, p.text), sv::mono_sm("Fase experimental", p.text)]
                    .spacing(6)
                    .align_y(Alignment::Center),
            )
            .padding(Padding::from([4.0, 8.0]))
            .style(sv::caja(Some(p.bg), p.bg, 0.0)),
        )
        .width(Length::Fill)
        .height(150)
        .align_x(Alignment::End)
        .align_y(Alignment::End)
        .padding(Padding { top: 0.0, right: espacio::S12, bottom: espacio::S3, left: 0.0 }),
    ];
    let amigos = crate::con_amigos::cantidad(app);
    let red = sr::red_conectada(s.zero.as_ref()).is_some();
    let progreso = sv::avance(s.pestana_t, app.ahora, sr::MS_PESTANA);
    let pestanas = sv::pestanas(
        p,
        s.pestana,
        if progreso >= 1.0 { s.pestana } else { s.pestana_desde },
        progreso,
        s.registros.len(),
        amigos,
        red,
        |t| m(Accion::Pestana(t)),
    );
    let contenido: Element<'_, Mensaje> = match s.pestana {
        Pestana::Servidores => pestana_servidores(app),
        Pestana::ConAmigos => crate::con_amigos::vista(app),
        Pestana::Red => pestana_red(app),
    };
    // El contenido de la pestaña entra con un fundido de 260 ms y sube 8 px.
    let cuerpo = icaro_ui::transicion::subir(p.bg, s.pestana.indice() as u64, 8.0, 260, contenido);
    // Con amigos es maestro y detalle de alto fijo: cada columna se desplaza
    // por su cuenta y los flujos de compartir ocupan el alto del panel.
    let cuerpo: Element<'_, Mensaje> = if s.pestana == Pestana::ConAmigos {
        container(cuerpo)
            .padding(Padding { top: 28.0, right: espacio::S12, bottom: espacio::S6, left: espacio::S12 })
            .height(Length::Fill)
            .into()
    } else {
        crate::suave(
            app,
            container(cuerpo).padding(Padding { top: 28.0, right: espacio::S12, bottom: espacio::S12, left: espacio::S12 }),
        )
    };
    column![
        banda,
        container(pestanas).padding(Padding { top: 28.0, right: espacio::S12, bottom: 0.0, left: espacio::S12 }),
        cuerpo,
    ]
    .height(Length::Fill)
    .into()
}

fn pestana_servidores(app: &App) -> Element<'_, Mensaje> {
    let p = app.paleta();
    let s = &app.servicios.servidores;
    let formulario_abierto = s.editor.as_ref().is_some_and(|e| e.id.is_none());
    let mut barra = row![container(componentes::campo_con_icono(
        p,
        Icono::Buscar,
        "Buscar servidor",
        &s.busqueda,
        |q| m(Accion::Buscar(q)),
    ))
    .max_width(420)
    .width(Length::Fill)]
    .spacing(12)
    .align_y(Alignment::Center);
    barra = barra.push(Space::with_width(Length::Fill));
    if !formulario_abierto {
        barra = barra.push(componentes::boton_con_icono(
            p,
            Icono::Mas,
            "Agregar servidor",
            Variante::Primario,
            (s.loaded && s.error.is_none() && !s.saving).then(|| m(Accion::Nuevo)),
            false,
        ));
    }
    let mut col = column![barra].spacing(16);
    if formulario_abierto {
        let k = sv::avance(s.editor_t, app.ahora, sr::MS_DESPLIEGUE);
        col = col.push(sv::revelar(formulario(app), k));
    }
    if let Some(error) = &s.error {
        col = col.push(sv::nota(
            p,
            Tono::Error,
            Icono::Alerta,
            sv::cuerpo_sm(error.clone(), p.text),
            Some(componentes::boton_compacto(p, "Releer lista", Variante::Secundario, (!s.loading).then(|| m(Accion::Abrir)))),
        ));
    }
    if s.loading && s.registros.is_empty() {
        col = col.push(sv::cuerpo_sm("Cargando servidores…", p.text_muted));
        return col.into();
    }
    if s.registros.is_empty() && s.saliendo.is_none() && s.error.is_none() {
        col = col.push(vacio(app));
        return col.into();
    }
    let q = s.busqueda.trim().to_lowercase();
    let mut lista = column![].spacing(8);
    let mut filas: Vec<(usize, &servidores::Servidor)> = s
        .registros
        .iter()
        .enumerate()
        .filter(|(_, v)| q.is_empty() || format!("{}{}", v.nombre, v.direccion).to_lowercase().contains(&q))
        .collect();
    let mut hay = !filas.is_empty();
    // La fila quitada sigue en su lugar mientras sale.
    let saliendo = s.saliendo.as_ref().map(|(v, i, t)| (v, *i, sv::avance(Some(*t), app.ahora, sr::MS_SALIDA)));
    if let Some((v, i, _)) = saliendo {
        let pos = filas.iter().position(|(j, _)| *j >= i).unwrap_or(filas.len());
        filas.insert(pos, (usize::MAX, v));
        hay = true;
    }
    for (i, v) in filas {
        if i == usize::MAX {
            let k = saliendo.map_or(1.0, |s| s.2);
            lista = lista.push(sv::salir(p.bg, fila(app, usize::MAX, v), k));
            continue;
        }
        let f = fila(app, i, v);
        lista = lista.push(match &s.nuevo {
            Some((id, t)) if *id == v.id => sv::halo(p.accent_soft, f, sv::avance(Some(*t), app.ahora, sr::MS_NUEVO)),
            _ => f,
        });
    }
    if !hay {
        lista = lista.push(sv::cuerpo_sm(format!("Ningún servidor coincide con «{}».", s.busqueda.trim()), p.text_muted));
    }
    col.push(lista).into()
}

fn vacio(app: &App) -> Element<'_, Mensaje> {
    let p = app.paleta();
    container(
        row![
            container(
                iced::widget::image(Lamina::Alas.imagen(p.modo))
                    .width(220)
                    .height(147)
                    .content_fit(iced::ContentFit::Cover),
            )
            .style(sv::caja(None, p.border, borde::FINO)),
            column![
                sv::titulo("Todavía no guardas servidores", p.text),
                container(sv::cuerpo_sm(
                    "Guarda la dirección de un servidor para ver quién está conectado y entrar con un clic. Para jugar con amigos sin servidor, usa la pestaña Con amigos.",
                    p.text_muted,
                ))
                .max_width(46.0 * 7.5),
                componentes::boton_con_icono(p, Icono::Mas, "Agregar servidor", Variante::Primario, Some(m(Accion::Nuevo)), false),
            ]
            .spacing(espacio::S2),
        ]
        .spacing(32)
        .align_y(Alignment::Center),
    )
    .padding(36)
    .width(Length::Fill)
    .style(sv::caja(Some(p.surface_sunken), p.border, borde::FINO))
    .into()
}

fn formulario(app: &App) -> Element<'_, Mensaje> {
    let p = app.paleta();
    let s = &app.servicios.servidores;
    let Some(e) = s.editor.as_ref() else {
        return Space::new(0, 0).into();
    };
    let formato = matches!(e.prueba, Prueba::Formato);
    let campos = row![
        container(campo(
            p,
            "Dirección",
            false,
            entrada(p, "servidor-direccion", "play.ejemplo.cl", &e.direccion, true, formato, |v| m(Accion::Direccion(v)), Some(m(Accion::Guardar))),
        ))
        .width(Length::FillPortion(13)),
        container(campo(
            p,
            "Nombre",
            true,
            entrada(p, "servidor-nombre", "Se completa con el del servidor", &e.nombre, false, false, |v| m(Accion::Nombre(v)), Some(m(Accion::Guardar))),
        ))
        .width(Length::FillPortion(10)),
        container(campo(p, "Jugar con", false, selector_instancia(p, app, e.instancia.as_ref()))).width(Length::FillPortion(10)),
    ]
    .spacing(16);
    let prueba: Element<'_, Mensaje> = match &e.prueba {
        Prueba::Vacia => container(sv::cuerpo_sm("Escribe una dirección para probarla.", p.text_muted)).center_y(44).into(),
        Prueba::Formato => sv::nota(
            p,
            Tono::Error,
            Icono::Alerta,
            row![
                sv::cuerpo_sm("Una dirección se ve como ", p.text),
                sv::mono_sm("play.ejemplo.cl", p.text),
                sv::cuerpo_sm(" o ", p.text),
                sv::mono_sm("10.0.0.5:25565", p.text),
                sv::cuerpo_sm(".", p.text),
            ]
            .wrap()
            .into(),
            None,
        ),
        Prueba::Probando => container(
            row![
                icaro_ui::progreso::spinner(p, app.reducir),
                sv::cuerpo_sm(format!("Probando {}…", e.direccion.trim()), p.text_muted),
            ]
            .spacing(10)
            .align_y(Alignment::Center),
        )
        .center_y(44)
        .into(),
        Prueba::Responde(estado) => sv::con_regla(p.success, p.success_soft, container(
            row![
                pixel(p, e.direccion.trim(), 32.0),
                column![
                    text(format!(
                        "Responde · {} / {} jugando · {} ms",
                        componentes::miles(estado.jugadores),
                        componentes::miles(estado.maximo),
                        estado.ms
                    ))
                    .font(fuentes::CUERPO_NEGRITA)
                    .size(texto::BODY_SM.0)
                    .color(p.text),
                    sv::mono_sm(
                        if estado.motd.trim().is_empty() {
                            estado.version.clone()
                        } else {
                            format!("{} · \"{}\"", estado.version, estado.motd.replace('\n', " ").trim())
                        },
                        p.text_muted
                    ),
                ]
                .spacing(2),
            ]
            .spacing(12)
            .align_y(Alignment::Center),
        )
        .padding(Padding::from([10.0, 14.0]))
        .width(Length::Fill)
        .into()),
        Prueba::NoResponde => sv::nota(
            p,
            Tono::Alerta,
            Icono::Aviso,
            sv::cuerpo_sm("No responde ahora. Puedes guardarlo igual.", p.text),
            None,
        ),
    };
    let puede = matches!(e.prueba, Prueba::Responde(_) | Prueba::NoResponde) && !s.saving;
    let mut cuerpo = column![
        column![
            text("Agregar servidor").font(fuentes::CUERPO_NEGRITA).size(texto::HEADING.0).color(p.text),
            sv::cuerpo_sm("Probamos la dirección mientras escribes.", p.text_muted),
        ]
        .spacing(4),
        campos,
        container(prueba).width(Length::Fill),
    ]
    .spacing(18);
    if let Some(error) = &e.error {
        cuerpo = cuerpo.push(sv::nota(p, Tono::Error, Icono::Alerta, sv::cuerpo_sm(error.clone(), p.text), None));
    }
    cuerpo = cuerpo.push(
        row![
            Space::with_width(Length::Fill),
            componentes::boton(p, "Cancelar", Variante::Secundario, Some(m(Accion::Cerrar))),
            componentes::boton(p, "Guardar servidor", Variante::Primario, puede.then(|| m(Accion::Guardar))),
        ]
        .spacing(8),
    );
    container(cuerpo)
        .padding(24)
        .width(Length::Fill)
        .style(sv::caja(Some(p.surface), p.text, borde::MEDIO))
        .into()
}

/// Descripción de los mods con que se entra, a partir de la instancia.
fn mods_de(app: &App, v: &servidores::Servidor) -> String {
    let Some(i) = v.instancia.as_ref().and_then(|id| app.servicios.instancias.registros.iter().find(|i| &i.id == id)) else {
        return "Elige con qué instancia entrar.".into();
    };
    if i.loader.eq_ignore_ascii_case("vanilla") {
        "Vanilla: entra con cualquier instancia de esa versión".into()
    } else {
        format!("Usa los mods de {} ({})", i.nombre, i.loader)
    }
}

/// Fila de un servidor; `i` es su posición (o `usize::MAX` si sale).
fn fila<'a>(app: &'a App, i: usize, v: &'a servidores::Servidor) -> Element<'a, Mensaje> {
    let p = app.paleta();
    let s = &app.servicios.servidores;
    let estado = s.estados.get(&v.id);
    let (off, consultando) = match estado {
        Some(Ok(_)) => (false, false),
        Some(Err(_)) => (true, false),
        None => (false, true),
    };
    let abierto = i != usize::MAX && s.editor.as_ref().and_then(|e| e.id.as_deref()) == Some(v.id.as_str());
    let tinta_nombre = if off { p.text_muted } else { p.text };
    let (jugadores, version, ms) = match estado {
        Some(Ok(e)) => (
            format!("{} / {} jugando", componentes::miles(e.jugadores), componentes::miles(e.maximo)),
            e.version.clone(),
            Some(e.ms.min(u32::MAX as u64) as u32),
        ),
        Some(Err(_)) => ("Sin respuesta".into(), "—".into(), None),
        None => ("Consultando…".into(), "—".into(), None),
    };
    let cab = row![
        container(pixel(p, &v.nombre, 32.0)).style(if off {
            sv::caja(None, p.bg, 0.0)
        } else {
            sv::caja(None, p.bg, 0.0)
        }),
        column![
            text(v.nombre.clone()).font(fuentes::CUERPO_NEGRITA).size(15).color(tinta_nombre),
            sv::mono_sm(v.direccion.clone(), p.text_muted),
        ]
        .spacing(3)
        .width(Length::Fill),
        column![sv::cuerpo_sm(jugadores, p.text), sv::mono_sm(version, p.text_muted)].spacing(2).width(200),
        row![
            sv::senal(p, if off || consultando { None } else { ms }),
            sv::mono_sm(ms.map_or("--".into(), |ms| format!("{ms} ms")), p.text_muted),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .width(96),
        container(icono(if abierto { Icono::Flecha } else { Icono::ChevronAbajo }, Tam::Base, if abierto { p.text } else { p.text_muted }))
            .center_x(20),
    ]
    .spacing(16)
    .align_y(Alignment::Center);
    let cab = button(container(cab).padding(Padding { top: 14.0, right: 8.0, bottom: 14.0, left: 18.0 }))
        .width(Length::Fill)
        .padding(0)
        .on_press(m(Accion::Alternar(v.id.clone())))
        .style(estilo::sin_estilo(p.text));
    let jugar = componentes::boton_con_icono(
        p,
        Icono::Jugar,
        "Jugar",
        if off { Variante::Secundario } else { Variante::Primario },
        (!off && i != usize::MAX).then(|| m(Accion::Jugar(i))),
        true,
    );
    let encabezado = row![cab, jugar].spacing(12).align_y(Alignment::Center).padding(Padding { top: 0.0, right: 16.0, bottom: 0.0, left: 0.0 });
    let mut col = column![encabezado];
    if abierto {
        let k = sv::avance(s.editor_t, app.ahora, sr::MS_DESPLIEGUE);
        col = col.push(sv::revelar(detalle_fila(app, v, off), k));
    }
    let borde_c = if abierto { p.text } else { p.border_strong };
    container(col)
        .width(Length::Fill)
        .style(sv::caja(Some(p.surface), borde_c, if abierto { borde::MEDIO } else { borde::FINO }))
        .into()
}

fn detalle_fila<'a>(app: &'a App, v: &'a servidores::Servidor, off: bool) -> Element<'a, Mensaje> {
    let p = app.paleta();
    let s = &app.servicios.servidores;
    let Some(e) = s.editor.as_ref() else {
        return Space::new(0, 0).into();
    };
    let mut col = column![].spacing(18);
    if off {
        col = col.push(sv::nota(
            p,
            Tono::Alerta,
            Icono::Aviso,
            sv::cuerpo_sm("No responde ahora. Puede estar apagado o cambió de dirección.", p.text),
            Some(componentes::boton_con_icono(p, Icono::Refrescar, "Probar de nuevo", Variante::Secundario, Some(m(Accion::Reintentar(v.id.clone()))), true)),
        ));
    } else {
        let motd = s
            .estados
            .get(&v.id)
            .and_then(|r| r.as_ref().ok())
            .map(|e| e.motd.replace('\n', " ").trim().to_owned())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| "Sin mensaje".into());
        let dato = |rot: &str, valor: String| -> Element<'a, Mensaje> {
            column![sv::rotulo(p, rot), text(valor).font(fuentes::CUERPO).size(13).color(p.text)]
                .spacing(4)
                .width(Length::Fill)
                .into()
        };
        col = col.push(row![dato("Mensaje", motd), dato("Mods", mods_de(app, v))].spacing(16));
    }
    col = col.push(
        row![
            container(campo(p, "Jugar con", false, selector_instancia(p, app, e.instancia.as_ref()))).width(Length::Fill),
            container(campo(
                p,
                "Nombre",
                false,
                entrada(p, "servidor-nombre", "Nombre del servidor", &e.nombre, false, false, |t| m(Accion::Nombre(t)), Some(m(Accion::Guardar))),
            ))
            .width(Length::Fill),
        ]
        .spacing(16),
    );
    if let Some(error) = &e.error {
        col = col.push(sv::nota(p, Tono::Error, Icono::Alerta, sv::cuerpo_sm(error.clone(), p.text), None));
    }
    // Quitar confirma en el mismo lugar: la franja reemplaza los botones.
    let zona: Element<'a, Mensaje> = if s.confirmar.as_deref() == Some(v.id.as_str()) {
        sv::confirmar(
            p,
            &format!("¿Quitar {}?", v.nombre),
            "Solo se borra de tu lista; tus instancias no cambian.",
            "Quitar",
            m(Accion::Quitar(v.id.clone())),
            m(Accion::CancelarQuitar),
        )
    } else {
        let copiado = s.copiado.as_ref().is_some_and(|c| c.0 == v.id);
        row![
            componentes::boton_con_icono(
                p,
                if copiado { Icono::Check } else { Icono::Copiar },
                if copiado { "Copiada" } else { "Copiar dirección" },
                Variante::Secundario,
                Some(m(Accion::CopiarDireccion(v.id.clone()))),
                true,
            ),
            componentes::boton_con_icono(p, Icono::Papelera, "Quitar", Variante::Peligro, Some(m(Accion::PedirQuitar(v.id.clone()))), true),
        ]
        .spacing(8)
        .into()
    };
    col = col.push(zona);
    container(col).padding(Padding { top: 4.0, right: 18.0, bottom: 20.0, left: 72.0 }).width(Length::Fill).into()
}

fn pestana_red(app: &App) -> Element<'_, Mensaje> {
    use icaro_core::zerotier::Estado as Z;
    let p = app.paleta();
    let s = &app.servicios.servidores;
    let conectada = sr::red_conectada(s.zero.as_ref());
    let mut col = column![].spacing(24);
    match (&s.zero, conectada) {
        (None, _) => {
            col = col.push(row![icaro_ui::progreso::spinner(p, app.reducir), sv::cuerpo_sm("Comprobando ZeroTier…", p.text_muted)].spacing(10));
        }
        (Some(Z::Ausente), _) => {
            col = col.push(sv::nota(
                p,
                Tono::Alerta,
                Icono::Aviso,
                sv::cuerpo_sm("ZeroTier no está instalado. Instálalo para que tus amigos vean tu mundo abierto a LAN.", p.text),
                Some(componentes::boton_con_icono(p, Icono::Externo, "Descargar ZeroTier", Variante::Secundario, Some(Mensaje::RedPrivada(crate::red_privada_real::Accion::Descargar)), true)),
            ));
        }
        (Some(Z::SinAcceso), _) => {
            col = col.push(sv::nota(
                p,
                Tono::Error,
                Icono::Alerta,
                sv::cuerpo_sm("El servicio de ZeroTier no responde. Ábrelo o reinícialo y vuelve a intentar.", p.text),
                Some(componentes::boton_con_icono(p, Icono::Refrescar, "Probar de nuevo", Variante::Secundario, Some(m(Accion::ConsultarZero)), true)),
            ));
        }
        (Some(Z::Disponible { .. }), Some(red)) => {
            let ip = red.ips.first().map(|ip| ip.split('/').next().unwrap_or(ip).to_owned()).unwrap_or_default();
            let copiada = s.copiado.as_ref().is_some_and(|c| c.0 == "ip");
            let mut cab = column![
                row![sv::marca(p.success), sv::titulo(format!("Conectada a \"{}\"", red.nombre), p.text)]
                    .spacing(12)
                    .align_y(Alignment::Center),
                sv::cuerpo_sm("Tus amigos de esta red pueden entrar a tu mundo abierto a LAN con tu IP, como si estuvieran en tu casa.", p.text_muted),
            ]
            .spacing(12);
            if !ip.is_empty() {
                cab = cab.push(
                    row![
                        sv::rotulo(p, "Tu IP"),
                        text(ip.clone()).font(fuentes::MONO).size(texto::MONO.0).color(p.text),
                        componentes::boton_con_icono(
                            p,
                            if copiada { Icono::Check } else { Icono::Copiar },
                            if copiada { "Copiada" } else { "Copiar" },
                            Variante::Secundario,
                            Some(m(Accion::CopiarIp(ip.clone()))),
                            true,
                        ),
                    ]
                    .spacing(12)
                    .align_y(Alignment::Center),
                );
            }
            col = col.push(column![cab, Space::with_height(20), sv::regla(borde::MEDIO, p.text)]);
            // ZeroTier solo informa este equipo: los demás miembros los ve el
            // administrador de la red en ZeroTier Central.
            let yo = app.servicios.nombre("Este equipo").to_owned();
            col = col.push(
                column![
                    row![sv::subtitulo("En la red", p.text), Space::with_width(Length::Fill), sv::mono_sm("Este equipo", p.text_muted)],
                    sv::persona(p, &format!("{yo} (tú)"), icaro_core::icono_instancia::patron(&yo, 0), Some(&ip), true, "en línea", None),
                ]
                .spacing(12),
            );
            let zona: Element<'_, Mensaje> = if s.red_confirmar {
                sv::confirmar(
                    p,
                    "¿Desconectar de la red?",
                    "Quien esté en tu mundo por la red se desconecta.",
                    "Desconectar",
                    m(Accion::Desconectar),
                    m(Accion::CancelarDesconectar),
                )
            } else {
                row![
                    componentes::boton_con_icono(p, Icono::UsuarioMas, "Invitar a la red", Variante::Secundario, Some(m(Accion::InvitarRed)), true),
                    componentes::boton_con_icono(p, Icono::Salir, "Desconectar", Variante::Peligro, (!s.red_ocupada).then(|| m(Accion::PedirDesconectar)), true),
                ]
                .spacing(8)
                .into()
            };
            col = col.push(sv::pie(p, zona));
        }
        (Some(Z::Disponible { .. }), None) => {
            col = col.push(column![
                row![sv::marca(p.text_disabled), sv::titulo("Desconectada", p.text)].spacing(12).align_y(Alignment::Center),
                Space::with_height(12),
                sv::cuerpo_sm("Conéctate para que tus amigos vean tu mundo abierto a LAN.", p.text_muted),
                Space::with_height(20),
                sv::regla(borde::MEDIO, p.text),
            ]);
            col = col.push(sv::pie(
                p,
                componentes::boton_con_icono(p, Icono::Monitor, "Conectar", Variante::Primario, Some(m(Accion::GestionarRed)), false),
            ));
        }
    }
    container(col)
        .padding(Padding::from([28.0, 32.0]))
        .max_width(880)
        .width(Length::Fill)
        .style(sv::caja(Some(p.surface), p.border_strong, borde::FINO))
        .into()
}

/// Para pruebas: el avance de la salida de una fila.
#[allow(dead_code)]
pub fn avance_salida(desde: Instant, ahora: Instant) -> f32 {
    sv::avance(Some(desde), ahora, sr::MS_SALIDA)
}
