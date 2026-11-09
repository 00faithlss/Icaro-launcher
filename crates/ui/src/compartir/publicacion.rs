// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

use super::{comun::*, *};
use crate::{
    componentes::{self, Estado, Variante},
    fuentes,
    tema::Paleta,
};
use iced::{
    widget::{column, container, row, text, Space},
    Alignment, Element, Length,
};

#[derive(Default)]
struct Resumen {
    seleccionados: [usize; 4],
    detectados: [usize; 4],
    privados: usize,
    bloqueantes: usize,
    problemas_privados: usize,
    autorizaciones: usize,
}
impl Resumen {
    fn calcular(cambios: &[CambioPublicacion]) -> Self {
        let mut r = Self::default();
        for c in cambios {
            let seleccionado = c.eleccion.publica() && !c.integrado;
            let indice = match c.grupo {
                GrupoPublicacion::Nuevo => Some(0),
                GrupoPublicacion::Actualizaciones => Some(1),
                GrupoPublicacion::Retiradas => Some(2),
                GrupoPublicacion::Resourcepacks => Some(3),
                GrupoPublicacion::Privado => None,
            };
            if let Some(i) = indice {
                if !c.integrado {
                    r.detectados[i] += 1;
                }
                if seleccionado {
                    r.seleccionados[i] += 1;
                }
            } else if seleccionado {
                r.seleccionados[0] += 1;
            }
            if matches!(c.grupo, GrupoPublicacion::Nuevo | GrupoPublicacion::Privado)
                && !seleccionado
            {
                r.privados += 1;
            }
            if c.problema.is_some() {
                if seleccionado {
                    r.bloqueantes += 1;
                } else {
                    r.problemas_privados += 1;
                }
            }
            if seleccionado && c.propio && !c.autorizado {
                r.autorizaciones += 1;
            }
        }
        r
    }
    fn efectivos(&self) -> usize {
        self.seleccionados.iter().sum()
    }
}

fn resumen<'a>(p: Paleta, r: &Resumen) -> Element<'a, Accion> {
    resumen_filas(p, r, 6)
}
fn resumen_filas<'a>(p: Paleta, r: &Resumen, columnas: usize) -> Element<'a, Accion> {
    let mut filas = column![];
    let mut celdas = row![];
    for (i, nombre) in [
        "Altas",
        "Actualizaciones",
        "Retiradas",
        "Resourcepacks",
        "Privado",
        "Problemas",
    ]
    .iter()
    .enumerate()
    {
        let (n, detalle) = if i < 4 {
            (
                r.seleccionados[i],
                format!(
                    "de {} detectad{}",
                    r.detectados[i],
                    if i == 3 { "os" } else { "as" }
                ),
            )
        } else if i == 4 {
            (r.privados, "se queda en tu equipo".into())
        } else {
            (
                r.bloqueantes,
                format!("bloquean · {} en privado", r.problemas_privados),
            )
        };
        celdas = celdas.push(
            container(
                column![
                    text(nombre.to_uppercase())
                        .font(fuentes::ETIQUETA)
                        .size(9)
                        .width(Length::Fill)
                        .height(24)
                        .align_x(Alignment::Center)
                        .align_y(Alignment::Center)
                        .color(p.text_muted),
                    text(n.to_string())
                        .font(fuentes::MONO_MEDIO)
                        .size(28)
                        .width(Length::Fill)
                        .height(34)
                        .align_x(Alignment::Center)
                        .align_y(Alignment::Center)
                        .color(if i == 5 && n > 0 { p.error } else { p.text }),
                    text(detalle)
                        .font(fuentes::MONO)
                        .size(11)
                        .line_height(1.)
                        .width(Length::Fill)
                        .height(36)
                        .align_x(Alignment::Center)
                        .align_y(Alignment::Center)
                        .color(p.text_muted)
                ]
                .spacing(6),
            )
            .width(Length::FillPortion(1))
            .height(132)
            .padding(12)
            .align_y(Alignment::Center)
            .style(marco(p)),
        );
        if (i + 1) % columnas == 0 {
            filas = filas.push(celdas);
            celdas = row![];
        }
    }
    column![mono(p, "Seleccionado para publicar · detectado"), filas]
        .spacing(10)
        .into()
}

fn fila<'a>(
    p: Paleta,
    c: &CambioPublicacion,
    nueva: u64,
    base: u64,
    ocupado: bool,
) -> Element<'a, Accion> {
    let seleccionado = c.eleccion.publica();
    let apagado =
        c.eleccion == Eleccion::Posponer || (c.grupo == GrupoPublicacion::Nuevo && !seleccionado);
    let mut tinta = p;
    if apagado {
        tinta.text = p.text_muted;
        tinta.text_muted = p.text_disabled;
    }
    let mut titulo = row![heading(tinta, &c.archivo.nombre)]
        .spacing(10)
        .align_y(Alignment::Center);
    if c.propio || !c.archivo.procedencia.is_empty() {
        titulo = titulo.push(insignia_origen(p, &c.archivo.procedencia, c.propio));
    }
    if c.solo_cliente {
        titulo = titulo.push(componentes::insignia(p, Estado::Info, "Solo cliente"));
    }
    let estado = if c.integrado {
        "Viene dentro del mod: no se descarga aparte".into()
    } else if c.eleccion == Eleccion::SinDecidir {
        "Sin decidir · queda privado".into()
    } else if c.grupo == GrupoPublicacion::Nuevo && c.eleccion == Eleccion::Privado {
        "Privado · guardado".into()
    } else if c.eleccion == Eleccion::Posponer {
        if c.estado.is_empty() {
            format!("Se mantiene como en la revisión {base}")
        } else {
            c.estado.clone()
        }
    } else if seleccionado {
        format!("Se publica en la revisión {nueva}")
    } else {
        c.estado.clone()
    };
    let mut info = column![
        titulo,
        mono(tinta, &c.archivo.detalle),
        cuerpo(tinta, estado)
    ]
    .spacing(4);
    if let Some(problema) = &c.problema {
        info = info.push(
            text(format!(
                "{}{}",
                problema.titulo,
                if seleccionado {
                    ""
                } else {
                    " · no bloquea: es privado"
                }
            ))
            .font(fuentes::CUERPO)
            .size(13)
            .color(if seleccionado { p.error } else { p.text_muted }),
        );
    }
    let accion = |e| (!ocupado).then(|| Accion::EleccionPublicacion(c.archivo.id.clone(), e));
    let elecciones: Element<'a, Accion> = if c.integrado {
        Space::new(0, 0).into()
    } else {
        match c.grupo {
            GrupoPublicacion::Nuevo => segmento(
                p,
                vec![
                    ("Compartir", seleccionado, accion(Eleccion::Compartir)),
                    (
                        "Mantener privado",
                        c.eleccion == Eleccion::Privado,
                        accion(Eleccion::Privado),
                    ),
                ],
            ),
            GrupoPublicacion::Actualizaciones | GrupoPublicacion::Resourcepacks => segmento(
                p,
                vec![
                    (
                        "Incluir",
                        c.eleccion == Eleccion::Incluir,
                        accion(Eleccion::Incluir),
                    ),
                    (
                        "Posponer",
                        c.eleccion == Eleccion::Posponer,
                        accion(Eleccion::Posponer),
                    ),
                ],
            ),
            GrupoPublicacion::Retiradas => segmento(
                p,
                vec![
                    (
                        "Retirar",
                        c.eleccion == Eleccion::Retirar,
                        accion(Eleccion::Retirar),
                    ),
                    (
                        "Posponer",
                        c.eleccion == Eleccion::Posponer,
                        accion(Eleccion::Posponer),
                    ),
                ],
            ),
            GrupoPublicacion::Privado => pequeno(
                p,
                if seleccionado { "Deshacer" } else { "Incluir" },
                Variante::Secundario,
                accion(if seleccionado {
                    Eleccion::Privado
                } else {
                    Eleccion::Incluir
                }),
            ),
        }
    };
    column![
        container(
            row![
                container(marca(tinta, c.marca)).width(24),
                container(info).width(Length::Fill),
                mono(tinta, &c.archivo.tamano),
                elecciones
            ]
            .spacing(16)
            .align_y(Alignment::Start)
        )
        .padding([16., 4.]),
        regla(p, false)
    ]
    .into()
}

pub fn revision_publicacion<'a>(p: Paleta, m: &RevisionPublicacion) -> Element<'a, Accion> {
    let r = Resumen::calcular(&m.cambios);
    let ocupado = matches!(
        m.estado,
        EstadoRevisionPublicacion::Analizando { .. } | EstadoRevisionPublicacion::Preparando { .. }
    );
    let mut c = column![row![
        identidad_instancia(p, &m.instancia),
        Space::new(Length::Fill, 0),
        mono(
            p,
            format!("Revisión publicada {} → Nueva revisión {}", m.base, m.nueva)
        )
    ]
    .spacing(16)
    .align_y(Alignment::Center)];
    match &m.estado {
        EstadoRevisionPublicacion::Analizando{hechos,total}=>c=c.push(column![
            cuerpo(p,format!("Comparando tu instancia con la revisión {}",m.base)),mono(p,format!("{hechos} de {total} archivos")),
            crate::progreso::continuo(p,if *total==0{0.}else{*hechos as f32 / *total as f32},false)].spacing(10)),
        EstadoRevisionPublicacion::Preparando{porcentaje}=>c=c.push(column![
            avance(p,&format!("Preparando la revisión {}",m.nueva),*porcentaje),
            tenue(p,format!("Tus amigos siguen recibiendo la revisión {}",m.base))].spacing(14)),
        EstadoRevisionPublicacion::Publicada{resumen}=>c=c.push(column![
            componentes::aviso_detallado(p,Estado::Exito,&format!("Publicaste la revisión {}",m.nueva),resumen),
            cuerpo(p,"Tus amigos la verán al pulsar Jugar o Sincronizar, y revisarán los cambios antes de aplicarlos. Nunca se toca una instancia mientras Minecraft la usa.")].spacing(16)),
        EstadoRevisionPublicacion::Error(motivo)=>c=c.push(column![
            componentes::aviso_detallado(p,Estado::Error,&format!("No se pudo preparar la revisión {}. {motivo}",m.nueva),
                &format!("No se publicó nada: tus amigos siguen recibiendo la revisión {}. Tu selección se conserva.",m.base)),
            pequeno(p,"Reintentar",Variante::Secundario,Some(Accion::Reintentar))].spacing(12)),
        EstadoRevisionPublicacion::Disponible|EstadoRevisionPublicacion::SinCambios=>{
            c=c.push(componentes::aviso(p,Estado::Info,"Los cambios de tu instancia no se comparten hasta que publiques una revisión. El contenido que mantengas privado se queda en tu equipo."));
            if r.bloqueantes>0 {
                let mut problemas=column![componentes::aviso(p,Estado::Error,&format!("{} problemas bloquean la selección actual",r.bloqueantes))].spacing(12);
                for cambio in &m.cambios {
                    if !cambio.eleccion.publica(){continue;}
                    if let Some(problema)=&cambio.problema {
                        problemas=problemas.push(row![marca(p,Marca::Conflicto),container(column![heading(p,&problema.titulo),mono(p,&problema.archivo),cuerpo(p,&problema.detalle)].spacing(4)).width(Length::Fill),
                            pequeno(p,&problema.rotulo,Variante::Secundario,Some(problema.accion.clone()))].spacing(16));
                    }
                }
                c=c.push(problemas);
            }
            if matches!(m.estado,EstadoRevisionPublicacion::SinCambios) {
                c=c.push(row![marca(p,Marca::Conservado),cuerpo(p,format!("Tu instancia está igual que la revisión {}",m.base))].spacing(16));
            } else {
                let seleccionar_mods = (!m.confirmar_cancelacion
                    && m.cambios.iter().any(|c| c.eleccion_para_seleccionar_mod().is_some()))
                    .then_some(Accion::SeleccionarTodosLosMods);
                c = c.push(column![
                    resumen(p, &r),
                    container(pequeno(
                        p,
                        "Seleccionar todos los mods",
                        Variante::Secundario,
                        seleccionar_mods,
                    ))
                    .width(Length::Fill)
                    .align_x(Alignment::End),
                ].spacing(12));
            }
            for g in GrupoPublicacion::TODOS {
                if matches!(m.estado,EstadoRevisionPublicacion::SinCambios)&&g!=GrupoPublicacion::Privado{continue;}
                let cambios: Vec<_> = m.cambios.iter().filter(|c| c.grupo_visible() == g).collect();
                let mut grupo=grupo(p,g.titulo(),&cambios.len().to_string()).push(tenue(p,g.nota()));
                if cambios.is_empty()&&g==GrupoPublicacion::Nuevo {grupo=grupo.push(cuerpo(p,"Todo lo nuevo quedó privado. Lo encuentras abajo, en Contenido privado."));}
                for cambio in cambios {
                    if g==GrupoPublicacion::Privado&&cambio.grupo==GrupoPublicacion::Nuevo {
                        let mut privado=cambio.clone();privado.grupo=GrupoPublicacion::Privado;privado.marca=Marca::Conservado;
                        grupo=grupo.push(fila(p,&privado,m.nueva,m.base,ocupado));
                    }else{grupo=grupo.push(fila(p,cambio,m.nueva,m.base,ocupado));}
                }
                c=c.push(grupo);
            }
        }
    }
    let motivo = if r.bloqueantes > 0 {
        format!("Resuelve {} problemas para publicar", r.bloqueantes)
    } else if r.efectivos() == 0 {
        "Nada que publicar: lo nuevo sigue privado".into()
    } else {
        String::new()
    };
    let primario = if r.autorizaciones > 0 {
        format!(
            "Autorizar {} archivo{} propio{}",
            r.autorizaciones,
            if r.autorizaciones == 1 { "" } else { "s" },
            if r.autorizaciones == 1 { "" } else { "s" }
        )
    } else {
        format!("Publicar revisión {}", m.nueva)
    };
    let accion = if r.autorizaciones > 0 {
        Accion::RevisarPropios
    } else {
        Accion::PublicarRevision
    };
    let permitido = !ocupado
        && r.bloqueantes == 0
        && r.efectivos() > 0
        && matches!(m.estado, EstadoRevisionPublicacion::Disponible);
    let acciones = if m.confirmar_cancelacion {
        row![container(cuerpo(p,"¿Cerrar sin publicar? Tu selección se pierde. Lo que marcaste como privado ya quedó guardado.")).width(Length::Fill),
            boton(p,"Seguir revisando",Variante::Secundario,Some(Accion::SeguirRevisando)),boton(p,"Cerrar sin publicar",Variante::Peligro,Some(Accion::CerrarSinPublicar))]
    } else if matches!(m.estado, EstadoRevisionPublicacion::Publicada { .. }) {
        row![
            Space::new(Length::Fill, 0),
            boton(p, "Cerrar", Variante::Primario, Some(Accion::Cerrar))
        ]
    } else {
        row![
            tenue(p, motivo),
            Space::new(Length::Fill, 0),
            boton(
                p,
                "Cancelar",
                Variante::Secundario,
                Some(Accion::ConfirmarCancelarPublicacion)
            ),
            boton(
                p,
                &primario,
                Variante::Primario,
                permitido.then_some(accion)
            )
        ]
    };
    dialogo(p, 960., "Revisar publicación", "", c, acciones)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn revision_se_construye_con_resumen_dentro_del_area_desplazable() {
        let modelo = RevisionPublicacion {
            instancia: InstanciaCompartida::default(),
            base: 1,
            nueva: 2,
            cambios: vec![cambio(Eleccion::Compartir)],
            estado: EstadoRevisionPublicacion::Disponible,
            confirmar_cancelacion: false,
        };
        let _vista = revision_publicacion(crate::tema::TINTA, &modelo);
    }
    fn cambio(eleccion: Eleccion) -> CambioPublicacion {
        CambioPublicacion {
            archivo: ArchivoCompartido::default(),
            grupo: GrupoPublicacion::Nuevo,
            marca: Marca::Alta,
            eleccion,
            propio: true,
            autorizado: false,
            solo_cliente: false,
            integrado: false,
            problema: Some(Problema {
                archivo: String::new(),
                titulo: String::new(),
                detalle: String::new(),
                accion: Accion::Reintentar,
                rotulo: String::new(),
            }),
            estado: String::new(),
        }
    }
    #[test]
    fn el_problema_privado_no_bloquea_y_solo_lo_elegido_pide_permiso() {
        let r = Resumen::calcular(&[cambio(Eleccion::Privado), cambio(Eleccion::SinDecidir)]);
        assert_eq!(
            (
                r.bloqueantes,
                r.problemas_privados,
                r.autorizaciones,
                r.efectivos()
            ),
            (0, 2, 0, 0)
        );
        let r = Resumen::calcular(&[cambio(Eleccion::Compartir)]);
        assert_eq!((r.bloqueantes, r.autorizaciones, r.efectivos()), (1, 1, 1));
    }
}
