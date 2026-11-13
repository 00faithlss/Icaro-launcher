// SPDX-License-Identifier: GPL-3.0-only
//! Con amigos › servidor alojado: la sección «Servidor» del detalle de lo que
//! compartes, la página «Ajustes del servidor» y el bloque del servidor del
//! anfitrión en lo que recibes. Usa solo piezas de PantallaServidores.
use crate::alojamiento_real::{m, Accion, Editor, Fase, Remoto};
use crate::servidores_vista::pixel;
use crate::{App, Mensaje};
use icaro_core::alojamiento as al;
use icaro_ui::{
    compartir as v,
    componentes::{self, Variante},
    iconos::Icono,
    pantalla_servidores::{self as sv, Tono},
    tema::{borde, Paleta},
};
use iced::widget::{column, container, row, Space};
use iced::{Alignment, Element, Length, Padding};

/// Líneas de consola visibles; el registro completo queda en la carpeta.
const CONSOLA_VISIBLE: usize = 12;

fn bloque<'a>(contenido: impl Into<Element<'a, Mensaje>>) -> Element<'a, Mensaje> {
    container(contenido).width(Length::Fill).into()
}

fn caja<'a>(p: Paleta, contenido: impl Into<Element<'a, Mensaje>>) -> Element<'a, Mensaje> {
    container(contenido)
        .padding(20)
        .width(Length::Fill)
        .style(sv::caja(Some(p.surface_sunken), p.border, borde::FINO))
        .into()
}

fn ocupado<'a>(app: &'a App, texto: impl Into<String>) -> Element<'a, Mensaje> {
    let p = app.paleta();
    row![
        icaro_ui::progreso::spinner(p, app.reducir),
        sv::cuerpo_sm(texto.into(), p.text_muted)
    ]
    .spacing(10)
    .align_y(Alignment::Center)
    .into()
}

fn jugadores_txt(j: &[String]) -> String {
    match j.len() {
        0 => "Nadie jugando".into(),
        1 => format!("Jugando: {}", j[0]),
        n if n <= 4 => format!("Jugando: {}", j.join(", ")),
        n => format!("{n} jugando: {}…", j[..3].join(", ")),
    }
}

/// Sección «Servidor» de una instancia que compartes.
pub fn seccion_anfitrion<'a>(app: &'a App, id: &str, en_pausa: bool) -> Element<'a, Mensaje> {
    let p = app.paleta();
    let s = &app.servicios.servidores.alojar;
    let loader = app
        .servicios
        .instancias
        .registros
        .iter()
        .find(|i| i.id == id)
        .map(|i| i.loader.clone())
        .unwrap_or_else(|| "fabric".into());
    let fase = s.de(id).cloned().unwrap_or_default();
    let otro = s.fase.en_marcha() && s.instancia.as_deref() != Some(id);
    let (insignia, palabra) = match &fase {
        Fase::Abierto => (Some(componentes::Estado::Exito), "Abierto"),
        Fase::Preparando(_) | Fase::Iniciando => (Some(componentes::Estado::Info), "Abriendo"),
        Fase::Deteniendo => (Some(componentes::Estado::Aviso), "Cerrando"),
        Fase::Error(_) => (Some(componentes::Estado::Error), "Con error"),
        Fase::Apagado => (None, "Cerrado"),
    };
    let cab = row![
        sv::subtitulo("Servidor", p.text),
        Space::with_width(Length::Fill),
        match insignia {
            Some(e) => componentes::insignia(p, e, palabra),
            None => componentes::insignia_contorno(p, palabra),
        },
    ]
    .align_y(Alignment::Center);
    let ajustes = componentes::boton_con_icono(
        p,
        Icono::Ajustes,
        "Ajustes",
        Variante::Secundario,
        Some(m(Accion::Ajustes(id.to_owned()))),
        true,
    );
    let cuerpo: Element<'a, Mensaje> = if !al::loader_alojable(&loader) {
        sv::nota(
            p,
            Tono::Info,
            Icono::InfoCaja,
            sv::cuerpo_sm("Por ahora solo se pueden alojar instancias Vanilla o Fabric.", p.text),
            None,
        )
    } else if s.eula.as_deref() == Some(id) {
        // La confirmación ya trae su propio marco: va sin la caja de la sección.
        return column![
            cab,
            sv::confirmar(
                p,
                "¿Aceptas el EULA de Minecraft?",
                "Es obligatorio para abrir un servidor: aka.ms/MinecraftEULA.",
                "Aceptar y abrir",
                m(Accion::AceptarEula),
                m(Accion::CancelarEula),
            ),
        ]
        .spacing(12)
        .into();
    } else {
        match &fase {
            Fase::Apagado | Fase::Error(_) => {
                let mut c = column![].spacing(14);
                if let Fase::Error(e) = &fase {
                    c = c.push(sv::nota(p, Tono::Error, Icono::Alerta, sv::cuerpo_sm(e.clone(), p.text), None));
                }
                let linea = if en_pausa {
                    "La publicación está en pausa: reanúdala para que tus amigos puedan entrar."
                } else if otro {
                    "Ya tienes otro servidor abierto. Ciérralo para abrir este."
                } else {
                    "Abre un servidor con los mods de esta instancia en tu equipo. Tus amigos entran desde Ícaro, sin abrir puertos ni estar en tu red."
                };
                c.push(sv::cuerpo_sm(linea, p.text_muted))
                    .push(
                        row![
                            componentes::boton_con_icono(
                                p,
                                Icono::Jugar,
                                if matches!(fase, Fase::Error(_)) {
                                    "Abrir de nuevo"
                                } else {
                                    "Abrir servidor"
                                },
                                Variante::Primario,
                                (!otro).then(|| m(Accion::Abrir(id.to_owned()))),
                                true,
                            ),
                            ajustes,
                        ]
                        .spacing(8),
                    )
                    .into()
            }
            Fase::Preparando(etapa) => row![
                container(ocupado(app, format!("{etapa}…"))).width(Length::Fill),
                componentes::boton_compacto(p, "Cancelar", Variante::Fantasma, Some(m(Accion::Detener))),
            ]
            .align_y(Alignment::Center)
            .into(),
            Fase::Iniciando => row![
                container(ocupado(app, "Cargando el mundo…")).width(Length::Fill),
                componentes::boton_compacto(p, "Cerrar servidor", Variante::Peligro, Some(m(Accion::Detener))),
            ]
            .align_y(Alignment::Center)
            .into(),
            Fase::Deteniendo => ocupado(app, "Guardando el mundo y cerrando…"),
            Fase::Abierto => column![
                row![
                    sv::marca(p.success),
                    sv::cuerpo_sm(
                        if en_pausa {
                            "Abierto, pero la publicación está en pausa: tus amigos no pueden entrar.".to_owned()
                        } else {
                            "Abierto para las personas de esta instancia".to_owned()
                        },
                        p.text
                    ),
                    Space::with_width(Length::Fill),
                    sv::mono_sm(jugadores_txt(&s.jugadores), p.text_muted),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
                row![
                    componentes::boton_con_icono(
                        p,
                        Icono::Jugar,
                        "Jugar en mi servidor",
                        Variante::Primario,
                        Some(m(Accion::JugarPropio(id.to_owned()))),
                        true,
                    ),
                    ajustes,
                    componentes::boton_compacto(p, "Cerrar servidor", Variante::Peligro, Some(m(Accion::Detener))),
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            ]
            .spacing(14)
            .into(),
        }
    };
    let mut c = column![cab, caja(p, cuerpo)].spacing(12);
    if s.instancia.as_deref() == Some(id) && (fase.en_marcha() || !s.consola.is_empty()) {
        c = c.push(consola(app));
    }
    c.into()
}

fn consola<'a>(app: &'a App) -> Element<'a, Mensaje> {
    let p = app.paleta();
    let s = &app.servicios.servidores.alojar;
    let desde = s.consola.len().saturating_sub(CONSOLA_VISIBLE);
    let mut lineas = column![].spacing(2);
    for l in s.consola.iter().skip(desde) {
        lineas = lineas.push(container(sv::mono_sm(l.clone(), p.text)).height(16).width(Length::Fill).clip(true));
    }
    for _ in s.consola.len().min(CONSOLA_VISIBLE)..CONSOLA_VISIBLE {
        lineas = lineas.push(Space::with_height(16));
    }
    let puede = s.fase == Fase::Abierto || s.fase == Fase::Iniciando;
    let entrada = row![
        container(componentes::campo(p, "Comando, por ejemplo: op Pato_Lucas", &s.comando, |t| m(
            Accion::Comando(t)
        )))
        .width(Length::Fill),
        componentes::boton(
            p,
            "Enviar",
            Variante::Secundario,
            (puede && !s.comando.trim().is_empty()).then(|| m(Accion::Enviar)),
        ),
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    column![
        row![
            sv::rotulo(p, "Consola"),
            Space::with_width(Length::Fill),
            sv::mono_sm(s.puerto.map_or(String::new(), |p| format!("127.0.0.1:{p}")), p.text_muted),
        ],
        container(lineas)
            .padding(Padding::from([12.0, 14.0]))
            .width(Length::Fill)
            .style(sv::caja(Some(p.surface), p.border, borde::FINO)),
        entrada,
    ]
    .spacing(10)
    .into()
}

/// Página «Ajustes del servidor» (en lugar del detalle, sin diálogo).
pub fn ajustes<'a>(app: &'a App, e: &'a Editor, nombre: &str) -> Element<'a, Mensaje> {
    let p = app.paleta();
    let pr = &e.propiedades;
    let abierto = app.servicios.servidores.alojar.de(&e.instancia).is_some_and(|f| f.en_marcha());
    let texto_campo = |rotulo: &str, marcador: &str, clave: &'static str| -> Element<'a, Mensaje> {
        column![
            sv::rotulo(p, rotulo),
            componentes::campo(p, marcador, pr.valor(clave).unwrap_or(""), move |t| m(Accion::Campo(clave, t))),
        ]
        .spacing(10)
        .into()
    };
    let segmentos = |rotulo: &str, clave: &'static str, valores: &'static [&'static str], nombres: &[&str]| -> Element<'a, Mensaje> {
        let actual = pr.valor(clave).unwrap_or(valores[0]).trim().to_ascii_lowercase();
        let activa = valores.iter().position(|v| *v == actual).unwrap_or(0);
        column![
            sv::rotulo(p, rotulo),
            componentes::segmentado_compacto(p, nombres, activa, move |i| m(Accion::Campo(clave, valores[i].to_owned()))),
        ]
        .spacing(10)
        .into()
    };
    let numero = |rotulo: &str, clave: &'static str, min: i64, max: i64, defecto: i64| -> Element<'a, Mensaje> {
        let v = pr.numero(clave).unwrap_or(defecto).clamp(min, max);
        column![
            sv::rotulo(p, rotulo),
            componentes::contador(
                p,
                v as i32,
                if v > min {
                    m(Accion::Campo(clave, (v - 1).to_string()))
                } else {
                    Mensaje::Nada
                },
                if v < max {
                    m(Accion::Campo(clave, (v + 1).to_string()))
                } else {
                    Mensaje::Nada
                },
            ),
        ]
        .spacing(10)
        .into()
    };
    let interruptor = |titulo: &str, detalle: &str, clave: &'static str| -> Element<'a, Mensaje> {
        let on = pr.bool(clave);
        container(
            row![
                column![
                    sv::cuerpo_sm(titulo.to_owned(), p.text),
                    sv::cuerpo_sm(detalle.to_owned(), p.text_muted)
                ]
                .spacing(2)
                .width(Length::Fill),
                componentes::interruptor(p, on, m(Accion::Campo(clave, (!on).to_string()))),
            ]
            .spacing(16)
            .align_y(Alignment::Center),
        )
        .padding(Padding::from([10.0, 0.0]))
        .into()
    };
    let lista = |filas: Vec<Element<'a, Mensaje>>| -> Element<'a, Mensaje> {
        let mut c = column![];
        for (i, f) in filas.into_iter().enumerate() {
            if i > 0 {
                c = c.push(sv::regla(borde::FINO, p.border));
            }
            c = c.push(f);
        }
        c.into()
    };
    let online = pr.bool("online-mode");
    let acceso: Element<'a, Mensaje> = if e.confirmar_online {
        sv::confirmar(
            p,
            "¿Permitir cuentas sin conexión?",
            "El servidor no comprobará las cuentas: cualquiera de tus invitados podrá entrar con el nombre que quiera, incluso el tuyo.",
            "Permitir",
            m(Accion::Campo("online-mode", "false".into())),
            m(Accion::CancelarOffline),
        )
    } else {
        container(
            row![
                column![
                    sv::cuerpo_sm("Comprobar cuentas de Minecraft", p.text),
                    sv::cuerpo_sm(
                        if online {
                            "Solo entran cuentas Microsoft. Desactívalo si algún amigo juega sin conexión."
                        } else {
                            "Las cuentas sin conexión pueden entrar. Nadie comprueba los nombres."
                        },
                        p.text_muted
                    ),
                ]
                .spacing(2)
                .width(Length::Fill),
                componentes::interruptor(
                    p,
                    online,
                    if online {
                        m(Accion::PedirOffline)
                    } else {
                        m(Accion::Campo("online-mode", "true".into()))
                    },
                ),
            ]
            .spacing(16)
            .align_y(Alignment::Center),
        )
        .padding(Padding::from([10.0, 0.0]))
        .into()
    };
    let total = if app.servicios.demo {
        16384
    } else {
        crate::juego_real::memoria_total_mb()
    };
    let maximo = total.saturating_sub(2048).clamp(1024, 32 * 1024);
    let mut c = column![sv::cabecera_detalle(
        p,
        pixel(p, nombre, 56.0),
        "Ajustes del servidor",
        &format!("server.properties de {nombre}"),
        Some(componentes::boton_compacto(
            p,
            "Volver",
            Variante::Fantasma,
            Some(m(Accion::VolverAjustes))
        )),
    )]
    .spacing(22);
    if abierto {
        c = c.push(sv::nota(
            p,
            Tono::Info,
            Icono::InfoCaja,
            sv::cuerpo_sm(
                "El servidor está abierto: los cambios se aplican la próxima vez que lo abras.",
                p.text,
            ),
            None,
        ));
    }
    if !online && !e.confirmar_online {
        c = c.push(sv::nota(
            p,
            Tono::Alerta,
            Icono::Alerta,
            sv::cuerpo_sm("Sin comprobar cuentas, solo invita a personas de confianza.", p.text),
            None,
        ));
    }
    c = c
        .push(texto_campo("Mensaje del servidor", "Servidor de Ícaro", "motd"))
        .push(segmentos(
            "Modo de juego",
            "gamemode",
            &["survival", "creative", "adventure", "spectator"],
            &["Supervivencia", "Creativo", "Aventura", "Espectador"],
        ))
        .push(segmentos(
            "Dificultad",
            "difficulty",
            &["peaceful", "easy", "normal", "hard"],
            &["Pacífica", "Fácil", "Normal", "Difícil"],
        ))
        .push(
            row![
                bloque(numero("Jugadores máximos", "max-players", 1, 100, 10)),
                bloque(numero("Distancia de visión", "view-distance", 3, 32, 10)),
                bloque(numero("Distancia de simulación", "simulation-distance", 3, 32, 10)),
            ]
            .spacing(24),
        )
        .push(texto_campo(
            "Semilla del mundo",
            "Vacía: una al azar al crear el mundo",
            "level-seed",
        ))
        .push(
            column![
                sv::subtitulo("Reglas", p.text),
                lista(vec![
                    interruptor("Combate entre jugadores", "PvP: los jugadores pueden hacerse daño.", "pvp"),
                    interruptor(
                        "Permitir volar",
                        "Evita que el servidor expulse a quien vuela con mods o elytras.",
                        "allow-flight"
                    ),
                    interruptor("Extremo", "Una sola vida: al morir se pasa a espectador.", "hardcore"),
                    interruptor("Bloques de comandos", "Permite usar bloques de comandos.", "enable-command-block"),
                    interruptor(
                        "Lista blanca",
                        "Solo entran los nombres que añadas con «whitelist add».",
                        "white-list"
                    ),
                ]),
                row![
                    bloque(numero("Protección del punto de aparición", "spawn-protection", 0, 64, 0)),
                    bloque(Space::new(0, 0))
                ]
                .spacing(24),
            ]
            .spacing(12),
        )
        .push(column![sv::subtitulo("Acceso", p.text), acceso].spacing(12))
        .push(
            column![
                sv::subtitulo("Memoria", p.text),
                componentes::deslizador_valor(p, e.ajustes.memoria_mb, 1024, maximo, 512, |v| m(Accion::Memoria(v))),
                sv::cuerpo_sm(
                    "Memoria máxima de Java para el servidor. Con muchos mods, 4 GB o más.",
                    p.text_muted
                ),
            ]
            .spacing(12),
        );
    let pie = row![
        componentes::boton(
            p,
            if e.guardando { "Guardando…" } else { "Guardar" },
            Variante::Primario,
            (e.sucio && !e.guardando).then(|| m(Accion::Guardar)),
        ),
        componentes::boton_con_icono(
            p,
            Icono::Carpeta,
            "Abrir carpeta del servidor",
            Variante::Secundario,
            Some(m(Accion::AbrirCarpeta(e.instancia.clone()))),
            false,
        ),
    ]
    .spacing(8);
    c.push(sv::pie(p, pie.into())).into()
}

/// Bloque del servidor del anfitrión en lo que recibes.
pub fn seccion_invitado<'a>(app: &'a App, r: &v::InstanciaQueRecibes) -> Element<'a, Mensaje> {
    let p = app.paleta();
    let s = &app.servicios.servidores.alojar;
    let estado = s.remotos.get(&r.id).cloned().unwrap_or(Remoto::Comprobando);
    let entrando = s.entrando.as_deref() == Some(r.id.as_str());
    let (color, linea) = match estado {
        Remoto::Abierto => (p.success, format!("{} tiene su servidor abierto", r.anfitrion)),
        Remoto::Cerrado => (p.text_disabled, format!("{} no tiene el servidor abierto", r.anfitrion)),
        Remoto::SinConexion => (p.text_disabled, format!("No se pudo consultar el servidor de {}", r.anfitrion)),
        Remoto::Comprobando => (p.text_disabled, "Comprobando el servidor…".to_owned()),
    };
    let accion: Element<'a, Mensaje> = match estado {
        Remoto::Abierto => componentes::boton_con_icono(
            p,
            Icono::Entrar,
            if entrando { "Conectando…" } else { "Entrar al servidor" },
            Variante::Primario,
            (!entrando).then(|| m(Accion::Entrar(r.id.clone()))),
            true,
        ),
        Remoto::Comprobando => Space::new(0, 0).into(),
        _ => componentes::boton_con_icono(
            p,
            Icono::Refrescar,
            "Comprobar",
            Variante::Secundario,
            Some(m(Accion::Consultar(r.id.clone()))),
            true,
        ),
    };
    let mut c = column![
        sv::rotulo(p, "Servidor"),
        row![
            sv::marca(color),
            container(sv::cuerpo_sm(linea, p.text)).width(Length::Fill),
            accion,
        ]
        .spacing(8)
        .align_y(Alignment::Center),
    ]
    .spacing(12);
    if estado == Remoto::Abierto && r.cambios.is_some_and(|n| n > 0) {
        c = c.push(sv::cuerpo_sm(
            "Sincroniza antes de entrar: el servidor usa la revisión nueva.",
            p.text_muted,
        ));
    }
    if s.tunel_activo(&r.id) {
        c = c.push(sv::cuerpo_sm(
            "Conectado. Si cierras Minecraft, vuelve a entrar desde aquí.",
            p.text_muted,
        ));
    }
    c.into()
}
