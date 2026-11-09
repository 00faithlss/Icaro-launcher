// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

use super::{comun::*, *};
use crate::{
    componentes::{self, Estado, Variante},
    estilo,
    iconos::Icono,
    tema::Paleta,
};
use iced::{
    widget::{button, column, container, row, text, Space},
    Alignment, Element, Length,
};

pub fn cierre_transferencias<'a>(p: Paleta, m: &CierreTransferencias) -> Element<'a, Accion> {
    let mut c = column![];
    for t in &m.transferencias {
        c = c.push(
            column![
                row![
                    heading(p, &t.nombre),
                    Space::new(Length::Fill, 0),
                    mono(p, &t.fase)
                ]
                .spacing(16),
                avance(p, "", t.porcentaje)
            ]
            .spacing(8),
        );
    }
    let esperar = if m.cerrar_icaro {
        "Seguir en segundo plano"
    } else {
        "Esperar a que terminen"
    };
    let ahora = if m.cerrar_icaro {
        "Cerrar ahora"
    } else {
        "Detener ahora"
    };
    let consecuencia_esperar = if m.cerrar_icaro {
        "Ícaro queda en la bandeja hasta que terminen y luego se cierra."
    } else {
        "No acepta nuevas descargas y deja de compartir cuando terminen las que están en curso."
    };
    let consecuencia_ahora = if m.cerrar_icaro {
        "Se cortan las transferencias. Tus amigos pueden retomarlas cuando vuelvas a compartir la misma revisión.".into()
    } else {
        format!("Se cortan {} transferencias y se revocan {} accesos. Sus instancias quedan como están.",m.transferencias.len(),m.accesos)
    };
    for (opcion, titulo, detalle) in [
        (OpcionCierre::Esperar, esperar, consecuencia_esperar),
        (OpcionCierre::Ahora, ahora, consecuencia_ahora.as_str()),
    ] {
        let elegido = m.opcion == opcion;
        c = c.push(
            button(
                container(
                    row![
                        container(if elegido {
                            crate::botones::icono_heredado(Icono::Check, crate::iconos::Tam::Base)
                        } else {
                            Space::new(24, 24).into()
                        })
                        .width(24)
                        .height(24)
                        .align_x(Alignment::Center)
                        .align_y(Alignment::Center),
                        // Heredar tinta del botón también en hover/inversión.
                        column![
                            text(titulo.to_owned())
                                .font(crate::fuentes::CUERPO_NEGRITA)
                                .size(14),
                            text(detalle.to_owned())
                                .font(crate::fuentes::CUERPO)
                                .size(13)
                        ]
                        .spacing(6)
                    ]
                    .spacing(14)
                    .align_y(Alignment::Center),
                )
                .center_y(Length::Fill)
                .padding([16., 18.]),
            )
            .width(Length::Fill)
            .height(112)
            .padding(0)
            .style(estilo::segmento(p, elegido))
            .on_press(Accion::OpcionCierre(opcion)),
        );
    }
    dialogo(
        p,
        640.,
        &if m.cerrar_icaro {
            "¿Cerrar Ícaro?".into()
        } else {
            format!("¿Dejar de compartir {}?", m.instancia)
        },
        &format!("Hay {} transferencias en curso", m.transferencias.len()),
        c,
        row![
            Space::new(Length::Fill, 0),
            boton(p, "Cancelar", Variante::Secundario, Some(Accion::Cancelar)),
            boton(
                p,
                if m.opcion == OpcionCierre::Esperar {
                    esperar
                } else {
                    ahora
                },
                if m.opcion == OpcionCierre::Ahora {
                    Variante::Peligro
                } else {
                    Variante::Primario
                },
                Some(Accion::ConfirmarCierre)
            )
        ],
    )
}

pub fn resumen_conexion<'a>(p: Paleta, m: &ResumenConexion) -> Element<'a, Accion> {
    let (anfitrion, detalle_anfitrion) = match m.anfitrion {
        EstadoAnfitrion::Buscando => ("Buscando", "Comprobando si su Ícaro está disponible"),
        EstadoAnfitrion::Disponible => ("Disponible", "Su Ícaro está abierto y en línea"),
        EstadoAnfitrion::Inaccesible => ("Inaccesible", "Su Ícaro está cerrado o sin red"),
    };
    let (sync, detalle_sync) = match &m.sincronizacion {
        ConexionSync::Conectando => ("Conectando", String::new()),
        ConexionSync::Conectada => ("Conectada", String::new()),
        ConexionSync::SinConexion => ("Sin conexión", String::new()),
        ConexionSync::Interrumpida => (
            "Interrumpida",
            "Lo descargado se guarda para continuar".into(),
        ),
        ConexionSync::Reconectando { intento, espera } => {
            ("Reconectando", format!("intento {intento} en {espera} s"))
        }
    };
    let (juego, detalle_juego) = match &m.juego {
        ConexionJuego::SinComprobar => ("Sin comprobar", String::new()),
        ConexionJuego::Alcanzable { direccion, ping } => {
            ("Servidor alcanzable", format!("{direccion} · {ping} ms"))
        }
        ConexionJuego::SinServidor => ("Sin servidor", "El anfitrión no está jugando".into()),
        ConexionJuego::RedDesconectada => ("Red privada desconectada", String::new()),
    };
    let mut columnas = row![];
    for (nombre, valor, detalle) in [
        ("Anfitrión", anfitrion, detalle_anfitrion.to_string()),
        ("Sincronización", sync, detalle_sync),
        ("Juego", juego, detalle_juego),
    ] {
        columnas = columnas.push(
            container(column![tenue(p, nombre), heading(p, valor), mono(p, detalle)].spacing(14))
                .width(Length::FillPortion(1))
                .padding(20)
                .style(marco(p)),
        );
    }
    container(column![row![icono_estado(p,Icono::Servidores),heading(p,"Estado de conexión")].spacing(16),regla(p,true),columnas,
        tenue(p,"Poder sincronizar no significa poder jugar: el juego va por la red privada o la dirección del servidor.")].spacing(24))
        .padding(28).width(Length::Fill).style(marco(p)).into()
}

pub fn progreso_sincronizacion<'a>(p: Paleta, m: &ProgresoSincronizacion) -> Element<'a, Accion> {
    let fases = [
        Fase::Preparacion,
        Fase::Descarga,
        Fase::Verificacion,
        Fase::Aplicacion,
    ];
    let actual = match m.estado {
        EstadoProgreso::Activo(f) => fases.iter().position(|x| *x == f),
        _ => None,
    };
    let mut riel = row![];
    for (i, (fase, nombre)) in fases
        .iter()
        .zip(["Preparación", "Descarga", "Verificación", "Aplicación"])
        .enumerate()
    {
        let hecha =
            matches!(m.estado, EstadoProgreso::Completado { .. }) || actual.is_some_and(|a| i < a);
        let marca_fase = if hecha {
            icono_estado(p, Icono::Check)
        } else if actual == Some(i) {
            crate::progreso::spinner(p, false)
        } else {
            mono(p, (i + 1).to_string())
        };
        let mut celda = column![
            container(Space::new(Length::Fill, 3)).style(estilo::bloque(if actual == Some(i) {
                p.text
            } else if hecha {
                p.text_muted
            } else {
                p.border
            })),
            row![
                container(marca_fase).width(24).height(24).center(24),
                cuerpo(p, nombre)
            ]
            .spacing(10)
        ]
        .spacing(12);
        if *fase == Fase::Aplicacion {
            celda = celda.width(Length::FillPortion(1));
        }
        riel = riel.push(celda.width(Length::FillPortion(1)));
    }
    let mut c = column![riel];
    let acciones = match &m.estado {
        EstadoProgreso::Error(error) => {
            c = column![error_compartir(p, error)];
            row![
                Space::new(Length::Fill, 0),
                boton(p, "Cerrar", Variante::Secundario, Some(Accion::Cerrar))
            ]
        }
        EstadoProgreso::Activo(fase) => {
            c = c
                .push(avance(p, &m.detalle, m.porcentaje))
                .push(mono(p, &m.archivo));
            if *fase == Fase::Aplicacion {
                c = c.push(tenue(p, "Aplicando: no se puede cancelar en este paso"));
            }
            row![
                Space::new(Length::Fill, 0),
                boton(
                    p,
                    "Cancelar",
                    Variante::Secundario,
                    (*fase != Fase::Aplicacion).then_some(Accion::Cancelar)
                )
            ]
        }
        EstadoProgreso::Cancelado => {
            c = c.push(componentes::aviso(
                p,
                Estado::Info,
                &format!(
                    "No se cambió nada en {}. Lo descargado queda guardado para la próxima vez.",
                    m.instancia
                ),
            ));
            row![
                Space::new(Length::Fill, 0),
                boton(p, "Cerrar", Variante::Secundario, Some(Accion::Cerrar)),
                boton(
                    p,
                    "Reintentar",
                    Variante::Primario,
                    Some(Accion::Reintentar)
                )
            ]
        }
        EstadoProgreso::Recuperando => {
            c = c.push(componentes::aviso(
                p,
                Estado::Aviso,
                &format!(
                    "Ícaro se cerró mientras aplicaba. Restaurando la copia de seguridad de {}…",
                    m.instancia
                ),
            ));
            row![]
        }
        EstadoProgreso::Completado { resumen } => {
            c = c.push(
                container(
                    row![
                        container(icono_estado(p, Icono::Check))
                            .width(56)
                            .height(56)
                            .center(56),
                        column![
                            titulo(
                                p,
                                format!("{} está en la revisión {}", m.instancia, m.revision)
                            ),
                            tenue(p, resumen)
                        ]
                        .spacing(6)
                    ]
                    .spacing(20),
                )
                .padding(24)
                .style(marco(p)),
            );
            row![
                boton(
                    p,
                    "Ver cambios",
                    Variante::Secundario,
                    Some(Accion::VerCambios)
                ),
                Space::new(Length::Fill, 0),
                boton(p, "Jugar", Variante::Primario, Some(Accion::Jugar))
            ]
        }
    };
    dialogo(
        p,
        720.,
        &format!("Sincronizando {}", m.instancia),
        &if m.revision == 0 {
            m.anfitrion.clone()
        } else {
            format!("Revisión {} de {}", m.revision, m.anfitrion)
        },
        c,
        acciones,
    )
}

pub fn error_compartir<'a>(p: Paleta, m: &ErrorCompartir) -> Element<'a, Accion> {
    let estado = if matches!(m.tipo, TipoError::Limite | TipoError::RevisionNueva) {
        Estado::Aviso
    } else {
        Estado::Error
    };
    let mut acciones = row![].spacing(8);
    let botones: Vec<(&str, Accion)> = match m.tipo {
        TipoError::Espacio => vec![("Elegir otra unidad", Accion::ElegirUnidad)],
        TipoError::ArchivoCambiado => vec![("Volver a revisar", Accion::RevisarPublicacion)],
        TipoError::Hash => vec![
            ("Reintentar", Accion::Reintentar),
            ("Ver detalle", Accion::VerDetalle),
        ],
        TipoError::Limite => vec![("Esperar", Accion::Esperar)],
        TipoError::RevisionNueva => vec![("Revisar la nueva", Accion::RevisarPublicacion)],
        TipoError::Revocado => vec![
            ("Entendido", Accion::Cerrar),
            ("Desvincular", Accion::Desvincular),
        ],
    };
    for (s, a) in botones {
        acciones = acciones.push(pequeno(p, s, Variante::Secundario, Some(a)));
    }
    let mut c = column![
        componentes::aviso_detallado(p, estado, &m.titulo, &m.explicacion),
        cuerpo(p, &m.conservacion),
        acciones
    ]
    .spacing(12);
    if let Some(d) = &m.detalle {
        c = c.push(mono(p, d));
    }
    c.into()
}

/// Barra adicional para Servidores; la lista existente se compone debajo.
/// Repite la misma tarjeta por cada publicación guardada.
pub fn accesos_servidores<'a>(
    p: Paleta,
    publicaciones: &[ResumenPublicacion],
    ancho: f32,
) -> Element<'a, Accion> {
    // Con menos espacio se acorta el rótulo para mantener los tres en una línea.
    let agregar_servidor = if ancho < 752.0 {
        "Agregar servidor"
    } else {
        "Agregar servidor por dirección"
    };
    let mut c = column![row![
        boton(
            p,
            "Compartir instancia",
            Variante::Secundario,
            Some(Accion::AbrirCompartir)
        ),
        boton(
            p,
            "Aceptar invitación",
            Variante::Secundario,
            Some(Accion::AceptarInvitacion)
        ),
        Space::new(Length::Fill, 0),
        boton(
            p,
            agregar_servidor,
            Variante::Primario,
            Some(Accion::AgregarServidor)
        )
    ]
    .spacing(12)]
    .spacing(20);
    for m in publicaciones {
        c = c.push(
            container(
                row![
                    container(
                        column![
                            heading(p, &m.instancia),
                            mono(
                                p,
                                format!(
                                    "Revisión {} · {} accesos · {} transferencias",
                                    m.revision, m.sesiones, m.transferencias
                                )
                            )
                        ]
                        .spacing(4)
                    )
                    .width(Length::Fill),
                    pequeno(
                        p,
                        "Ver publicación",
                        Variante::Secundario,
                        Some(Accion::VerPublicacionDe(m.id.clone()))
                    )
                ]
                .spacing(16)
                .align_y(Alignment::Center),
            )
            .padding([16., 20.])
            .style(marco(p)),
        );
    }
    c.into()
}
