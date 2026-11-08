// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

use super::{comun::*, *};
use crate::{
    componentes::{self, Estado, Variante},
    estilo, fuentes,
    tema::{Modo, Paleta},
};
use iced::{
    widget::{checkbox, column, container, image, row, text, text_input, Space},
    Alignment, Element, Length,
};
use std::sync::OnceLock;

/// Sólo formato: la existencia, caducidad y cupo se comprueban en el servicio.
/// No trunca una entrada demasiado larga ni convierte letras ambiguas.
pub fn normalizar_codigo(entrada: &str) -> String {
    let entrada = entrada.trim();
    let valor = entrada
        .strip_prefix("icaro://invitacion/")
        .unwrap_or(entrada);
    let limpio: Vec<char> = valor
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .flat_map(char::to_uppercase)
        .collect();
    let mut salida = String::new();
    for (i, c) in limpio.iter().enumerate() {
        if i > 0 && i % 4 == 0 {
            salida.push('-');
        }
        salida.push(*c);
    }
    salida
}

fn puente<'a>(p: Paleta) -> Element<'a, Accion> {
    static PIEDRA: OnceLock<image::Handle> = OnceLock::new();
    static TINTA: OnceLock<image::Handle> = OnceLock::new();
    let h = match p.modo {
        Modo::Piedra => PIEDRA.get_or_init(|| {
            image::Handle::from_bytes(include_bytes!("recursos/puente-pasarela.webp").as_slice())
        }),
        Modo::Tinta => TINTA.get_or_init(|| {
            image::Handle::from_bytes(
                include_bytes!("recursos/puente-pasarela-negativo.webp").as_slice(),
            )
        }),
    };
    image(h.clone())
        .width(220)
        .height(Length::Fill)
        .content_fit(iced::ContentFit::Cover)
        .into()
}

pub fn entrada_invitacion<'a>(p: Paleta, m: &EntradaInvitacion) -> Element<'a, Accion> {
    let error = matches!(
        m.estado,
        EstadoEntrada::Invalido | EstadoEntrada::Vencido { .. } | EstadoEntrada::Completo { .. }
    );
    let estilo_campo = move |t: &iced::Theme, s| {
        let mut v = estilo::campo(p)(t, s);
        if error {
            v.border.color = p.error;
        }
        v
    };
    let campo = text_input("XXXX-XXXX-XXXX", &m.codigo)
        .on_input(|s| Accion::Codigo(normalizar_codigo(&s)))
        .font(fuentes::MONO)
        .size(22)
        .padding([14., 16.])
        .style(estilo_campo);
    let (mensaje,color)=match &m.estado {
        EstadoEntrada::Vacio=>("12 caracteres, como 7QKM-4XPT-R2C9.".into(),p.text_muted),
        EstadoEntrada::Validando=>("Comprobando con el anfitrión…".into(),p.text_muted),
        EstadoEntrada::Invalido=>("Este código no existe. Revisa que esté completo: son 12 caracteres, como 7QKM-4XPT-R2C9.".into(),p.error),
        EstadoEntrada::Vencido{fecha,anfitrion}=>(if fecha.is_empty() {format!("Esta invitación ya no está vigente. Pide a {anfitrion} que genere otra.")} else {format!("Esta invitación venció el {fecha}. Pide a {anfitrion} que genere otra.")},p.error),
        EstadoEntrada::Completo{anfitrion}=>(format!("Esta invitación ya no admite a más personas: se completó su cupo. Pide a {anfitrion} que genere otra."),p.error),
        EstadoEntrada::Valido{anfitrion,disponibles}=>(format!("Invitación vigente de {anfitrion} · quedan {disponibles} cupos."),p.success),
        EstadoEntrada::Inaccesible(motivo)=>(motivo.clone(),p.warning),
    };
    let c=column![cuerpo(p,"Pega el código que te envió quien comparte la instancia. Ícaro la copia en tu equipo y la mantiene al día."),
        column![heading(p,"Código de invitación"),row![campo,pequeno(p,"Pegar",Variante::Fantasma,Some(Accion::PegarCodigo))].spacing(8).align_y(Alignment::Center),
            text(mensaje).font(fuentes::CUERPO).size(13).color(color)].spacing(8),
        tenue(p,"Si ya aceptaste esta invitación, reconectarte no gasta otro cupo.")];
    let d = interior_dialogo(
        p,
        "Aceptar invitación",
        "Paso 1 de 4 · Código",
        c,
        row![
            Space::new(Length::Fill, 0),
            boton(p, "Cancelar", Variante::Secundario, Some(Accion::Cancelar)),
            boton(
                p,
                "Continuar",
                Variante::Primario,
                matches!(m.estado, EstadoEntrada::Valido { .. }).then_some(Accion::Continuar)
            )
        ],
    );
    container(row![puente(p), d])
        .width(Length::Fill)
        .max_width(820)
        .max_height(800)
        .style(marco_dialogo(p))
        .into()
}

pub fn identidad_anfitrion<'a>(p: Paleta, m: &IdentidadAnfitrion) -> Element<'a, Accion> {
    let mut sello = column![].spacing(0);
    for fila in m.sello {
        let mut r = row![].spacing(0);
        for ocupado in fila {
            r = r.push(
                container(Space::new(9, 9)).style(estilo::bloque(if ocupado {
                    p.text
                } else {
                    p.bg
                })),
            );
        }
        sello = sello.push(r);
    }
    let huella = container(
        row![
            container(sello)
                .padding(8)
                .width(88)
                .height(88)
                .style(marco(p)),
            column![
                text(m.huella.clone())
                    .font(fuentes::MONO_MEDIO)
                    .size(20)
                    .color(p.text),
                pequeno(
                    p,
                    "Copiar huella",
                    Variante::Fantasma,
                    Some(Accion::CopiarHuella)
                )
            ]
            .spacing(12)
        ]
        .spacing(20)
        .align_y(Alignment::Center),
    )
    .padding(20)
    .style(marco(p));
    let mut c = column![
        row![
            crate::servidores::avatar_servidor(p, &m.anfitrion),
            column![
                titulo(p, &m.anfitrion),
                tenue(
                    p,
                    format!(
                        "Comparte {} · Revisión {} · {} · {}",
                        m.instancia.nombre, m.revision, m.instancia.minecraft, m.instancia.loader
                    )
                )
            ]
            .spacing(4)
        ]
        .spacing(16)
        .align_y(Alignment::Center),
        huella
    ];
    let (estado,mensaje)=match &m.confianza {
        Confianza::PrimeraVez=>(Estado::Info,format!("Primera vez que aceptas de este equipo. Confirma la huella con {} por otro medio (Discord, en persona).",m.anfitrion)),
        Confianza::Conocido{fecha}=>(Estado::Exito,format!("Ya aceptaste de este equipo el {fecha}. La huella coincide.")),
        Confianza::Distinto{fecha}=>(Estado::Error,format!("Este equipo dice ser {}, pero su huella no es la que guardaste el {fecha}. Puede ser un equipo nuevo o alguien haciéndose pasar por él.",m.anfitrion)),
    };
    c = c.push(componentes::aviso(p, estado, &mensaje));
    let distinto = matches!(m.confianza, Confianza::Distinto { .. });
    if distinto {
        c = c.push(
            checkbox(
                "He comprobado la nueva huella con el anfitrión",
                m.confirmar_nueva,
            )
            .on_toggle(Accion::ConfirmarHuella)
            .style(estilo::casilla(p)),
        );
    }
    let acciones = if distinto {
        row![
            peligro_fantasma(
                p,
                "Confiar en la nueva huella",
                m.confirmar_nueva.then_some(Accion::ConfiarAnfitrion)
            ),
            Space::new(Length::Fill, 0),
            boton(p, "Cancelar", Variante::Primario, Some(Accion::Cancelar))
        ]
    } else {
        row![
            boton(
                p,
                "No es quien esperaba",
                Variante::Secundario,
                Some(Accion::Cancelar)
            ),
            Space::new(Length::Fill, 0),
            boton(
                p,
                if matches!(m.confianza, Confianza::Conocido { .. }) {
                    "Continuar"
                } else {
                    "Confiar y continuar"
                },
                Variante::Primario,
                Some(Accion::ConfiarAnfitrion)
            )
        ]
    };
    dialogo(
        p,
        680.,
        "¿Es quien esperabas?",
        "Paso 2 de 4 · Anfitrión",
        c,
        acciones,
    )
}

pub fn destino_nuevo<'a>(p: Paleta, m: &DestinoNuevo) -> Element<'a, Accion> {
    let campo = column![
        heading(p, "Nombre"),
        componentes::campo(
            p,
            "Nombre de la instancia",
            &m.nombre,
            Accion::NombreDestino
        ),
        tenue(p, "Puedes cambiarlo después sin romper el vínculo.")
    ]
    .spacing(8);
    let mut c = column![row![
        crate::servidores::avatar_servidor(p, &m.instancia.nombre),
        container(campo).width(Length::Fill)
    ]
    .spacing(16)];
    if let Some(e) = &m.error_nombre {
        c = c.push(componentes::aviso(p, Estado::Error, e));
    }
    let g = grupo(
        p,
        "Se va a instalar",
        &format!("Revisión {} de {}", m.revision, m.anfitrion),
    )
    .push(dato(p, "Minecraft", &m.instancia.minecraft))
    .push(dato(p, "Loader", &m.instancia.loader))
    .push(dato(p, "Mods", &m.mods))
    .push(dato(p, "Carpetas", &m.carpetas))
    .push(dato(p, "Descarga", &m.descarga));
    c=c.push(g).push(componentes::aviso(p,Estado::Info,&if m.recuperando {"Se encontró tu instalación incompleta. Continuar conserva su identidad y sus archivos; no crea otra copia. Revisa los cambios antes de aplicarlos.".into()} else {format!("Esta instancia queda vinculada a {}. Las próximas sincronizaciones la actualizan, y siempre verás los cambios antes de aplicarlos.",m.anfitrion)}));
    dialogo(
        p,
        720.,
        if m.recuperando {"Continuar instalación"}else{"Tu nueva instancia"},
        "Paso 3 de 4 · Instancia de destino",
        c,
        row![
            boton(p, "Atrás", Variante::Secundario, Some(Accion::Atras)),
            Space::new(Length::Fill, 0),
            boton(p, "Cancelar", Variante::Secundario, Some(Accion::Cancelar)),
            boton(
                p,
                "Revisar e instalar",
                Variante::Primario,
                (m.error_nombre.is_none() && !m.nombre.trim().is_empty())
                    .then_some(Accion::RevisarInstalacion)
            )
        ],
    )
}

pub fn revision_sincronizacion<'a>(p: Paleta, m: &RevisionSincronizacion) -> Element<'a, Accion> {
    let mut c = column![];
    if let Some(conexion) = &m.conexion {
        c = c.push(resumen_conexion(p, conexion));
    }
    let pendientes = m
        .diferencias
        .iter()
        .filter(|d| d.marca == Marca::Conflicto && d.resolucion.is_none())
        .count();
    match m.estado {
        EstadoRevisionInvitado::Revisando => {
            c = c.push(
                row![
                    crate::progreso::spinner(p, false),
                    cuerpo(p, format!("Comparando con la revisión {}", m.revision))
                ]
                .spacing(12),
            )
        }
        EstadoRevisionInvitado::SinCambios => {
            c = c.push(componentes::aviso(
                p,
                Estado::Exito,
                &format!("Ya está al día con la revisión {}", m.revision),
            ))
        }
        EstadoRevisionInvitado::Diferencias => {
            for marca_fila in [
                Marca::Conflicto,
                Marca::Alta,
                Marca::Actualizacion,
                Marca::Retirada,
                Marca::Conservado,
            ] {
                let filas: Vec<_> = m
                    .diferencias
                    .iter()
                    .filter(|d| d.marca == marca_fila)
                    .collect();
                if filas.is_empty() {
                    continue;
                }
                let nombre = match marca_fila {
                    Marca::Alta => "Altas",
                    Marca::Actualizacion => "Actualizaciones",
                    Marca::Retirada => "Retiradas",
                    Marca::Conflicto => "Conflictos",
                    Marca::Conservado => "Se conserva",
                };
                let mut g = grupo(p, nombre, &filas.len().to_string());
                for d in filas {
                    let mut info = ficha_archivo(p, &d.archivo);
                    if d.del_anfitrion {
                        info = info.push(componentes::insignia(p, Estado::Aviso, "Del anfitrión"));
                    }
                    let mut r = row![
                        container(marca(p, d.marca)).width(24),
                        container(info).width(Length::Fill),
                        mono(p, &d.archivo.tamano)
                    ]
                    .spacing(16)
                    .align_y(Alignment::Center);
                    if d.marca == Marca::Conflicto {
                        r = r.push(segmento(
                            p,
                            vec![
                                (
                                    "Del anfitrión",
                                    d.resolucion == Some(Resolucion::Anfitrion),
                                    Some(Accion::ResolverConflicto(
                                        d.archivo.id.clone(),
                                        Resolucion::Anfitrion,
                                    )),
                                ),
                                (
                                    "Conservar la mía",
                                    d.resolucion == Some(Resolucion::ConservarMia),
                                    Some(Accion::ResolverConflicto(
                                        d.archivo.id.clone(),
                                        Resolucion::ConservarMia,
                                    )),
                                ),
                            ],
                        ));
                    }
                    g = g.push(column![container(r).padding([12., 16.]), regla(p, false)]);
                }
                c = c.push(g);
            }
        }
    }
    let rotulo = if pendientes > 0 {
        format!("Resuelve {pendientes} conflictos")
    } else if matches!(m.estado, EstadoRevisionInvitado::SinCambios) {
        "Jugar".into()
    } else {
        "Sincronizar".into()
    };
    let accion = if matches!(m.estado, EstadoRevisionInvitado::SinCambios) {
        Accion::Jugar
    } else {
        Accion::Sincronizar
    };
    dialogo(
        p,
        760.,
        "Revisar cambios",
        &format!(
            "Paso 4 de 4 · {} ← {}, revisión {}",
            m.instancia, m.anfitrion, m.revision
        ),
        c,
        row![
            boton(p, "Atrás", Variante::Secundario, Some(Accion::Atras)),
            Space::new(Length::Fill, 0),
            boton(p, "Cancelar", Variante::Secundario, Some(Accion::Cancelar)),
            boton(
                p,
                &rotulo,
                Variante::Primario,
                (pendientes == 0 && !matches!(m.estado, EstadoRevisionInvitado::Revisando))
                    .then_some(accion)
            )
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normaliza_codigo_o_enlace_sin_truncar_errores() {
        assert_eq!(normalizar_codigo(" 7qkm 4xpt r2c9 "), "7QKM-4XPT-R2C9");
        assert_eq!(
            normalizar_codigo("icaro://invitacion/7qkm-4xpt-r2c9"),
            "7QKM-4XPT-R2C9"
        );
        assert_eq!(normalizar_codigo("1234567890123"), "1234-5678-9012-3");
    }
}
