// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

use super::{comun::*, *};
use crate::{
    componentes::{self, Estado, Variante},
    estilo,
    tema::Paleta,
};
use iced::{
    widget::{checkbox, column, container, pick_list, row, Space},
    Alignment, Element, Length,
};

#[derive(Clone, PartialEq, Eq)]
struct OpcionInstancia {
    id: String,
    nombre: String,
}
impl std::fmt::Display for OpcionInstancia {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.nombre)
    }
}

pub fn compartir_instancia<'a>(p: Paleta, m: &CompartirInstancia) -> Element<'a, Accion> {
    let seleccionada = m
        .instancias
        .iter()
        .find(|i| Some(&i.id) == m.seleccionada.as_ref());
    let opciones: Vec<OpcionInstancia> = m
        .instancias
        .iter()
        .filter(|i| !i.en_uso)
        .map(|i| OpcionInstancia {
            id: i.id.clone(),
            nombre: i.nombre.clone(),
        })
        .collect();
    let actual = seleccionada.filter(|i| !i.en_uso).map(|i| OpcionInstancia {
        id: i.id.clone(),
        nombre: i.nombre.clone(),
    });
    // La acción conserva el ID, no deriva identidad de la etiqueta mostrada.
    let selector = pick_list(opciones, actual, move |instancia: OpcionInstancia| {
        Accion::ElegirInstancia(instancia.id)
    })
    .placeholder("Elige una instancia")
    .width(Length::Fill)
    .padding([12., 16.])
    .style(estilo::selector(p))
    .menu_style(estilo::menu_selector(p));
    let mut c = column![column![tenue(p, "Instancia anfitriona"), selector].spacing(10)];
    for instancia in m.instancias.iter().filter(|i| i.en_uso) {
        c = c.push(pequeno(
            p,
            &format!("{} · En uso", instancia.nombre),
            Variante::Secundario,
            None,
        ));
    }
    if let Some(i) = seleccionada {
        c = c.push(identidad_instancia(p, i));
    }
    for problema in &m.problemas {
        c = c.push(
            column![
                componentes::aviso_detallado(p, Estado::Error, &problema.titulo, &problema.detalle),
                pequeno(
                    p,
                    &problema.rotulo,
                    Variante::Secundario,
                    Some(problema.accion.clone())
                )
            ]
            .spacing(8),
        );
    }
    match m.estado {
        EstadoContenido::Analizando { hechos, total } => {
            c = c.push(avance(
                p,
                &format!("Analizando {hechos} de {total} archivos"),
                porcentaje(hechos, total),
            ));
        }
        _ => {
            c = c.push(mono(
                p,
                format!(
                    "{} incluidos · {} excluidos · {} archivos propios · {} problemas",
                    m.total_incluidos,
                    m.excluidos.len(),
                    m.propios.len(),
                    m.problemas.len()
                ),
            ));
            let mut incluidos = grupo(
                p,
                "Incluidos",
                &format!("{} · se descargan de la plataforma", m.total_incluidos),
            );
            for a in &m.incluidos {
                incluidos = incluidos.push(fila_contenido(p, a, mono(p, &a.procedencia)));
            }
            if m.total_incluidos > m.incluidos.len() {
                incluidos = incluidos.push(pequeno(
                    p,
                    &format!("Ver los {}", m.total_incluidos),
                    Variante::Fantasma,
                    Some(Accion::VerIncluidos),
                ));
            }
            c = c.push(incluidos);
            let mut excluidos = grupo(
                p,
                "Excluidos",
                &format!("{} · no se comparten", m.excluidos.len()),
            );
            for a in &m.excluidos {
                excluidos = excluidos.push(fila_contenido(
                    p,
                    a,
                    pequeno(
                        p,
                        "Incluir",
                        Variante::Fantasma,
                        Some(Accion::IncluirArchivo(a.id.clone())),
                    ),
                ));
            }
            c = c.push(excluidos);
            let mut propios = grupo(
                p,
                "Archivos propios",
                &format!("{} · salen de tu equipo", m.propios.len()),
            );
            for a in &m.propios {
                propios = propios.push(fila_contenido(
                    p,
                    a,
                    componentes::insignia(p, Estado::Aviso, "Requiere tu permiso"),
                ));
            }
            c = c.push(propios).push(
                grupo(p, "Carpetas", &m.tamano_carpetas)
                    .push(
                        row![
                            cuerpo(p, &m.carpetas),
                            Space::new(Length::Fill, 0),
                            componentes::insignia(p, Estado::Exito, "Se comparten"),
                            pequeno(
                                p,
                                "Elegir",
                                Variante::Fantasma,
                                Some(Accion::ElegirCarpetas)
                            )
                        ]
                        .spacing(12),
                    )
                    .push(
                        row![
                            cuerpo(p, "saves, options.txt, screenshots"),
                            componentes::insignia_contorno(p, "Nunca")
                        ]
                        .spacing(16),
                    ),
            );
            if let EstadoContenido::Preparando {
                revision,
                porcentaje,
            } = m.estado
            {
                c = c.push(avance(
                    p,
                    &format!("Preparando la revisión {revision}: hashes y archivos propios"),
                    porcentaje,
                ));
            }
        }
    }
    let listo = matches!(m.estado, EstadoContenido::Listo)
        && m.problemas.is_empty()
        && seleccionada.is_some_and(|i| !i.en_uso);
    let rotulo = if !m.problemas.is_empty() {
        format!("Resuelve {} problemas", m.problemas.len())
    } else if !m.propios.is_empty() {
        "Revisar archivos propios".into()
    } else {
        "Compartir".into()
    };
    let vista = interior_dialogo(
        p,
        "Compartir instancia",
        "Paso 1 de 2 · Contenido",
        c,
        row![
            mono(p, "Los mundos nunca se comparten"),
            Space::new(Length::Fill, 0),
            boton(p, "Cancelar", Variante::Secundario, Some(Accion::Cancelar)),
            boton(
                p,
                &rotulo,
                Variante::Primario,
                listo.then_some(if m.propios.is_empty() {
                    Accion::Compartir
                } else {
                    Accion::RevisarPropios
                })
            )
        ],
    );
    container(row![
        container(
            crate::grabados::Imagen::nueva(p, crate::laminas::Lamina::MercadoCarros)
                .barrido(p)
                .tamano(200, Length::Fill)
        )
        .width(200)
        .height(Length::Fill),
        vista
    ])
    .width(Length::Fill)
    .max_width(900)
    .max_height(800)
    .padding(BORDE_DIALOGO)
    .style(marco_dialogo(p))
    .into()
}

fn porcentaje(hechos: u64, total: u64) -> f32 {
    if total == 0 {
        0.
    } else {
        100. * hechos as f32 / total as f32
    }
}
fn fila_contenido<'a>(
    p: Paleta,
    a: &ArchivoCompartido,
    accion: Element<'a, Accion>,
) -> Element<'a, Accion> {
    column![
        container(
            row![
                container(ficha_archivo(p, a)).width(Length::Fill),
                mono(p, &a.tamano),
                accion
            ]
            .spacing(16)
            .align_y(Alignment::Center)
        )
        .padding([6., 4.]),
        regla(p, false)
    ]
    .into()
}

pub fn confirmar_propios<'a>(p: Paleta, m: &ConfirmarPropios) -> Element<'a, Accion> {
    let mut c=column![componentes::aviso(p,Estado::Aviso,"Estos archivos no están en una plataforma. Saldrán de tu equipo; Ícaro no puede revisar qué hacen.")];
    for a in &m.archivos {
        c = c.push(
            column![
                ficha_archivo(p, a),
                mono(
                    p,
                    format!("{} · SHA-256 {} · {}", a.tamano, a.sha256, a.carpeta)
                ),
                regla(p, false)
            ]
            .spacing(8),
        );
    }
    c = c.push(
        container(
            checkbox(
                "Tengo permiso para compartir estos archivos y confío en su contenido",
                m.autorizado,
            )
            .on_toggle(Accion::PermisoPropios)
            .style(estilo::casilla(p)),
        )
        .padding([16., 18.])
        .style(marco(p)),
    );
    dialogo(
        p,
        720.,
        &format!("Enviar {} archivos propios", m.archivos.len()),
        &format!("Revisión {} de {}", m.revision, m.instancia),
        c,
        row![
            boton(
                p,
                "Excluirlos y continuar",
                Variante::Fantasma,
                Some(Accion::ExcluirPropios)
            ),
            Space::new(Length::Fill, 0),
            boton(p, "Cancelar", Variante::Secundario, Some(Accion::Cancelar)),
            boton(
                p,
                "Autorizar y compartir",
                Variante::Primario,
                m.autorizado.then_some(Accion::AutorizarPropios)
            )
        ],
    )
}
