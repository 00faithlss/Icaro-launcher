// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

use super::{comun::*, *};
use crate::{
    componentes::{self, Estado, Variante},
    estilo, fuentes,
    iconos::Icono,
    tema::Paleta,
};
use iced::{
    widget::{column, container, row, text, Space},
    Alignment, Element, Length,
};

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invitacion_conserva_altura_propia_en_un_panel_desplazable() {
        for estado in [
            EstadoInvitacion::Generando,
            EstadoInvitacion::Vigente {
                restante: "24 horas".into(),
            },
            EstadoInvitacion::Completa,
            EstadoInvitacion::Vencida {
                fecha: "ayer".into(),
            },
            EstadoInvitacion::Revocada,
        ] {
            let modelo = Invitacion {
                estado,
                codigo: "7QKM-4XPT-R2C9".into(),
                ..Default::default()
            };
            let _vista = iced::widget::scrollable(invitacion(crate::tema::TINTA, &modelo));
        }
    }
}

pub fn invitacion<'a>(p: Paleta, m: &Invitacion) -> Element<'a, Accion> {
    let vigente = matches!(m.estado, EstadoInvitacion::Vigente { .. });
    let (estado, rotulo) = match m.estado {
        EstadoInvitacion::Generando => (None, "Generando"),
        EstadoInvitacion::Vigente { .. } => (Some(Estado::Exito), "Vigente"),
        EstadoInvitacion::Completa => (Some(Estado::Info), "Cupo completo"),
        EstadoInvitacion::Vencida { .. } => (Some(Estado::Aviso), "Vencida"),
        EstadoInvitacion::Revocada => (Some(Estado::Error), "Revocada"),
    };
    let insignia = if let Some(e) = estado {
        componentes::insignia(p, e, rotulo)
    } else {
        componentes::insignia_contorno(p, rotulo)
    };
    let mut c = column![
        row![
            heading(p, "Invitación"),
            Space::new(Length::Fill, 0),
            insignia
        ],
        regla(p, true)
    ]
    .spacing(18);
    if vigente || matches!(m.estado, EstadoInvitacion::Generando) {
        let editar = m.ajustes_editables;
        let opciones = Caducidad::TODAS
            .iter()
            .map(|v| {
                (
                    v.texto(),
                    *v == m.caducidad,
                    editar.then_some(Accion::Caducidad(*v)),
                )
            })
            .collect();
        c = c.push(
            row![
                column![tenue(p, "Caduca en"), segmento(p, opciones)].spacing(8),
                column![
                    tenue(p, "Personas"),
                    row![
                        iconoboton(
                            p,
                            Icono::Menos,
                            "Menos personas",
                            (editar && m.cupo > m.aceptados.max(1))
                                .then_some(Accion::Cupo(m.cupo.saturating_sub(1)))
                        ),
                        container(
                            text(m.cupo.to_string())
                                .font(fuentes::MONO)
                                .size(13)
                                .color(p.text)
                        )
                        .center_x(28),
                        iconoboton(
                            p,
                            Icono::Mas,
                            "Más personas",
                            editar
                                .then(|| m.cupo.checked_add(1))
                                .flatten()
                                .map(Accion::Cupo)
                        )
                    ]
                    .spacing(10)
                    .align_y(Alignment::Center)
                ]
                .spacing(8)
            ]
            .spacing(28),
        );
    }
    let mut codigo = row![].spacing(4);
    let anulado = matches!(
        m.estado,
        EstadoInvitacion::Vencida { .. } | EstadoInvitacion::Revocada
    );
    let caracteres: Vec<char> = m
        .codigo
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(12)
        .collect();
    for n in 0..12 {
        if n == 4 || n == 8 {
            codigo = codigo.push(
                container(text("-").font(fuentes::MONO).size(24).color(p.text_muted))
                    .width(14)
                    .height(48)
                    .align_x(Alignment::Center)
                    .align_y(Alignment::Center),
            );
        }
        let generando = matches!(m.estado, EstadoInvitacion::Generando);
        let caracter = if generando {
            "·".into()
        } else {
            caracteres.get(n).unwrap_or(&'·').to_string()
        };
        let tinta = if anulado {
            p.text_disabled
        } else if generando {
            p.surface_sunken
        } else {
            p.text
        };
        let letra: Element<'a, Accion> = container(
            text(caracter)
                .font(fuentes::MONO_MEDIO)
                .size(24)
                .color(tinta),
        )
        .width(34)
        .height(48)
        .align_x(Alignment::Center)
        .align_y(Alignment::Center)
        .into();
        let celda = if anulado {
            iced::widget::stack![rayado(p.border), letra].into()
        } else {
            letra
        };
        codigo = codigo.push(
            container(celda)
                .width(34)
                .height(48)
                .align_x(Alignment::Center)
                .align_y(Alignment::Center)
                .style(move |_| iced::widget::container::Style {
                    background: Some(p.surface_sunken.into()),
                    border: iced::Border {
                        color: if anulado {
                            p.border_strong
                        } else if generando {
                            p.border
                        } else {
                            p.text
                        },
                        width: 2.,
                        radius: 0.0.into(),
                    },
                    ..Default::default()
                }),
        );
    }
    c = c.push(codigo);
    let mensaje = match &m.estado {
        EstadoInvitacion::Generando => "Creando el código…".into(),
        EstadoInvitacion::Vigente { restante } => format!(
            "{} de {} personas han aceptado · caduca en {restante}",
            m.aceptados, m.cupo
        ),
        EstadoInvitacion::Completa => format!(
            "{} de {} personas han aceptado. El código ya no admite a nadie más.",
            m.aceptados, m.cupo
        ),
        EstadoInvitacion::Vencida { fecha } => format!(
            "Venció el {fecha}. Aceptaron {} de {} personas.",
            m.aceptados, m.cupo
        ),
        EstadoInvitacion::Revocada => "La revocaste. El código ya no admite a nadie más.".into(),
    };
    c = c.push(cuerpo(p, mensaje));
    if vigente || matches!(m.estado, EstadoInvitacion::Completa) {
        // Como la referencia, el indicador compacto usa hasta veinte bloques;
        // el texto conserva el cupo completo, que no tiene máximo de producto.
        let bloques = m.cupo.min(20) as usize;
        let llenos = if m.cupo == 0 {
            0
        } else {
            ((m.aceptados as u64 * bloques as u64) / m.cupo as u64) as usize
        };
        c = c.push(crate::progreso::bloques(p, bloques, llenos, true));
    }
    if !matches!(m.estado, EstadoInvitacion::Generando) {
        c = c.push(tenue(
            p,
            "Quienes ya aceptaron conservan su acceso hasta que lo revoques en Accesos.",
        ));
        if vigente {
            c = c
                .push(
                    row![
                        pequeno(
                            p,
                            if m.copiado {
                                "Copiado"
                            } else {
                                "Copiar código"
                            },
                            Variante::Secundario,
                            Some(Accion::CopiarCodigo)
                        ),
                        pequeno(
                            p,
                            "Revocar código",
                            Variante::Peligro,
                            Some(Accion::RevocarCodigo)
                        ),
                        pequeno(
                            p,
                            "Generar otro",
                            Variante::Secundario,
                            Some(Accion::GenerarInvitacion)
                        )
                    ]
                    .spacing(8),
                )
                .push(mono(
                    p,
                    "Generar otro anula este código. Reconectarse no gasta cupo.",
                ));
        } else {
            c = c.push(pequeno(
                p,
                "Generar otra invitación",
                Variante::Primario,
                Some(Accion::GenerarInvitacion),
            ));
        }
    }
    container(c)
        .padding(24)
        .width(Length::Fill)
        .max_width(640)
        .style(marco(p))
        .into()
}

pub fn accesos<'a>(p: Paleta, m: &Accesos) -> Element<'a, Accion> {
    let mut c = grupo(p, "Accesos", &m.personas.len().to_string());
    if m.personas.is_empty() {
        c = c.push(
            container(cuerpo(
                p,
                "Nadie tiene acceso. Quien acepte una invitación aparecerá aquí.",
            ))
            .padding([20., 16.])
            .style(marco(p)),
        );
    }
    for a in &m.personas {
        let confirmar = m.confirmar.as_ref() == Some(&a.id);
        if confirmar {
            c=c.push(container(column![cuerpo(p,"Su instancia queda como está, pero deja de recibir cambios y se corta lo que esté descargando"),
                row![pequeno(p,"Cancelar",Variante::Secundario,Some(Accion::CancelarRevocacion)),
                pequeno(p,"Revocar acceso",Variante::Peligro,Some(Accion::RevocarAcceso(a.id.clone())))].spacing(8)].spacing(12)).padding(16).style(estilo::bloque(p.error_soft)));
        } else {
            c = c.push(column![
                container(
                    row![
                        crate::servidores::avatar_servidor(p, &a.nombre),
                        container(
                            column![
                                heading(p, &a.nombre),
                                mono(p, &a.huella),
                                row![componentes::punto(p, a.conectado), cuerpo(p, &a.estado)]
                                    .spacing(8)
                            ]
                            .spacing(4)
                        )
                        .width(Length::Fill),
                        mono(p, &a.aceptado),
                        pequeno(
                            p,
                            "Revocar",
                            Variante::Fantasma,
                            Some(Accion::ConfirmarRevocacion(a.id.clone()))
                        )
                    ]
                    .spacing(16)
                    .align_y(Alignment::Center)
                )
                .padding([10., 4.]),
                regla(p, false)
            ]);
        }
    }
    c=c.push(container(row![container(column![heading(p,"Dejar de compartir"),tenue(p,"Revoca todos los accesos e invitaciones. Las instancias de tus amigos quedan como están.")].spacing(6)).width(Length::Fill),
        pequeno(p,"Dejar de compartir",Variante::Peligro,Some(Accion::DejarDeCompartir))].spacing(24).align_y(Alignment::Center)).padding([18.,20.]).style(marco(p)));
    c.into()
}

/// Publicación mientras se recupera o se reconecta: la misma estructura con
/// esqueletos en lugar de datos, para que no salte al llegar.
pub fn panel_publicacion_cargando<'a>(
    p: Paleta,
    m: &PanelPublicacion,
    reducir: bool,
) -> Element<'a, Accion> {
    use crate::progreso::esqueleto;
    let mut datos = row![];
    for label in ["Revisión vigente", "Disponibilidad", "Transferencias", "Tu huella"] {
        datos = datos.push(
            container(
                column![
                    container(tenue(p, label)).height(16).center_x(Length::Fill),
                    container(esqueleto(p, Length::Fixed(140.), 12., reducir))
                        .height(36)
                        .center_x(Length::Fill)
                        .align_y(Alignment::Center),
                    container(esqueleto(p, Length::Fixed(80.), 10., reducir))
                        .height(28)
                        .center_x(Length::Fill)
                        .align_y(Alignment::Center),
                ]
                .spacing(6),
            )
            .padding([14., 16.])
            .width(Length::FillPortion(1))
            .height(120)
            .style(marco(p)),
        );
    }
    let bloque = |alto: f32| -> Element<'a, Accion> {
        container(
            column![
                esqueleto(p, Length::Fixed(120.), 14., reducir),
                esqueleto(p, Length::Fill, 12., reducir),
                esqueleto(p, Length::FillPortion(1), 12., reducir),
            ]
            .spacing(14),
        )
        .padding(24)
        .width(Length::Fill)
        .height(alto)
        .style(marco(p))
        .into()
    };
    let c = column![
        row![
            identidad_instancia(p, &m.instancia),
            Space::new(Length::Fill, 0),
            componentes::insignia_contorno(p, "Conectando"),
        ]
        .spacing(16)
        .align_y(Alignment::Center),
        regla(p, true),
        datos,
        cuerpo(p, "Recuperando la publicación y conectando con el servicio…"),
        row![
            container(bloque(380.)).width(Length::FillPortion(3)),
            container(column![bloque(52.), bloque(200.)].spacing(20))
                .width(Length::FillPortion(2)),
        ]
        .spacing(24),
    ]
    .spacing(20);
    // Sin recuadro: la publicación va sobre el fondo de la página, alineada
    // con las migas.
    crate::scroll::desplazable(container(c).padding([8., 32.]).width(Length::Fill))
}

pub fn panel_publicacion<'a>(p: Paleta, m: &PanelPublicacion) -> Element<'a, Accion> {
    let detenida = matches!(m.estado, EstadoPublicacion::Detenida);
    let mut c = column![
        row![
            identidad_instancia(p, &m.instancia),
            Space::new(Length::Fill, 0),
            if detenida {
                componentes::insignia_contorno(p, "Detenida")
            } else {
                componentes::insignia(p, Estado::Exito, "Compartiendo")
            }
        ]
        .spacing(16)
        .align_y(Alignment::Center),
        regla(p, true)
    ]
    .spacing(20);
    let mut datos = row![];
    for (label, valor) in [
        (
            "Revisión vigente",
            // Sin fecha ni hash conocidos no quedan separadores sueltos.
            [
                format!("Revisión {}", m.revision),
                m.fecha_revision.clone(),
                m.hash_revision.clone(),
            ]
            .into_iter()
            .filter(|s| !s.trim().is_empty())
            .collect::<Vec<_>>()
            .join(" · "),
        ),
        ("Disponibilidad", m.disponibilidad.clone()),
        ("Transferencias", m.transferencias.clone()),
        ("Tu huella", m.huella.clone()),
    ] {
        let accion: Element<'a, Accion> = if label == "Tu huella" {
            pequeno(p, "Copiar", Variante::Fantasma, Some(Accion::CopiarHuella))
        } else {
            Space::new(0, 28).into()
        };
        let d = column![
            container(tenue(p, label)).height(16).center_x(Length::Fill),
            container(mono(p, valor))
                .height(36)
                .center_x(Length::Fill)
                .align_y(Alignment::Center),
            container(accion).height(28).center_x(Length::Fill),
        ]
        .spacing(6);
        datos = datos.push(
            container(d)
                .padding([14., 16.])
                .width(Length::FillPortion(1))
                .height(120)
                .style(marco(p)),
        );
    }
    c = c.push(datos);
    match &m.estado {
        EstadoPublicacion::Preparando {
            revision,
            porcentaje,
        } => {
            c = c.push(avance(
                p,
                &format!("Preparando la revisión {revision}"),
                *porcentaje,
            ))
        }
        EstadoPublicacion::PreparandoNueva {
            revision,
            porcentaje,
        } => {
            c = c.push(
                column![
                    avance(
                        p,
                        &format!("Preparando la revisión {revision}"),
                        *porcentaje
                    ),
                    tenue(
                        p,
                        format!("Tus amigos siguen recibiendo la revisión {}", m.revision)
                    ),
                    pequeno(p, "Cancelar", Variante::Secundario, Some(Accion::Cancelar))
                ]
                .spacing(14),
            )
        }
        EstadoPublicacion::Cambios { resumen } => {
            c = c.push(
                column![
                    componentes::aviso_detallado(
                        p,
                        Estado::Info,
                        "Hay cambios sin publicar",
                        resumen
                    ),
                    pequeno(
                        p,
                        "Revisar cambios",
                        Variante::Primario,
                        Some(Accion::RevisarPublicacion)
                    )
                ]
                .spacing(12),
            )
        }
        EstadoPublicacion::Publicada { revision, resumen } => {
            c = c.push(componentes::aviso_detallado(
                p,
                Estado::Exito,
                &format!("Publicaste la revisión {revision}: {resumen}"),
                "Tus amigos la verán al pulsar Jugar o Sincronizar.",
            ))
        }
        EstadoPublicacion::Error(motivo) => {
            c = c.push(
                column![
                    componentes::aviso_detallado(
                        p,
                        Estado::Error,
                        motivo,
                        &format!(
                            "No se publicó nada. Tus amigos siguen recibiendo la revisión {}.",
                            m.revision
                        )
                    ),
                    pequeno(
                        p,
                        "Reintentar",
                        Variante::Secundario,
                        Some(Accion::Reintentar)
                    )
                ]
                .spacing(12),
            )
        }
        EstadoPublicacion::Detenida => {
            c = c.push(boton(
                p,
                "Volver a compartir",
                Variante::Primario,
                Some(Accion::VolverACompartir),
            ))
        }
        EstadoPublicacion::Compartiendo => {}
    }
    if !detenida {
        let privado = container(
            row![
                cuerpo(p, format!("Contenido privado: {}", m.contenido_privado)),
                Space::new(Length::Fill, 0),
                pequeno(
                    p,
                    "Revisar",
                    Variante::Fantasma,
                    Some(Accion::RevisarPublicacion)
                )
            ]
            .spacing(16)
            .align_y(Alignment::Center),
        )
        .padding([14., 18.])
        .style(marco(p));
        // Todo en una pantalla: la invitación a la izquierda; contenido privado
        // y accesos a la derecha.
        let lado = column![privado, accesos(p, &m.accesos)].spacing(20);
        c = c.push(match &m.invitacion {
            Some(i) => Element::from(
                row![
                    container(invitacion(p, i)).width(Length::FillPortion(3)),
                    container(lado).width(Length::FillPortion(2)),
                ]
                .spacing(24),
            ),
            None => lado.into(),
        });
    }
    // Si la ventana es baja, la página se desplaza con suavidad.
    // Sin recuadro: la publicación va sobre el fondo de la página, alineada
    // con las migas.
    crate::scroll::desplazable(container(c).padding([8., 32.]).width(Length::Fill))
}
