// SPDX-License-Identifier: GPL-3.0-only
use super::{Estado, Etapa};
use icaro_core::compartir::{Artefacto, Procedencia};
use icaro_ui::{compartir as v, tema::Paleta};
use iced::Element;
fn archivo(ruta: &str, a: Option<&Artefacto>) -> v::ArchivoCompartido {
    v::ArchivoCompartido {
        id: ruta.into(),
        nombre: ruta.rsplit('/').next().unwrap_or(ruta).into(),
        detalle: ruta.into(),
        tamano: a.map_or_else(|| "Local".into(), |a| format!("{} bytes", a.bytes)),
        procedencia: a
            .map_or("Personal", |a| {
                if matches!(a.procedencia, Procedencia::Propio) {
                    "Del anfitrión"
                } else {
                    "Modrinth"
                }
            })
            .into(),
        carpeta: ruta.split('/').next().unwrap_or_default().into(),
        ..Default::default()
    }
}
pub fn revision(s: &Estado) -> Option<v::RevisionSincronizacion> {
    let r = s.sincronizar.revision.as_ref()?;
    let mut diferencias: Vec<_> = r
        .plan
        .conflictos
        .iter()
        .map(|c| v::Diferencia {
            archivo: archivo(c.ruta.as_str(), c.remoto.as_ref().or(c.anterior.as_ref())),
            marca: v::Marca::Conflicto,
            del_anfitrion: false,
            resolucion: s.sincronizar.elecciones.get(c.ruta.as_str()).copied(),
        })
        .collect();
    if let Some(orden) = r.plan.orden_packs.as_ref().filter(|o| o.pendiente()) {
        diferencias.push(v::Diferencia {
            archivo: v::ArchivoCompartido { id: "@resourcepacks".into(), nombre: "Selección y orden de resourcepacks".into(), detalle: format!("Tu selección: {}. Compartida: {}. Se conservan tus packs personales; los retirados dejan de estar activos.", orden.actual().join(" → "), orden.nuevo().join(" → ")), carpeta: "resourcepacks".into(), ..Default::default() },
            marca: v::Marca::Conflicto, del_anfitrion: false,
            resolucion: s.sincronizar.elecciones.get("@resourcepacks").copied(),
        });
    }
    diferencias.extend(r.plan.cambios.iter().map(|c| {
        v::Diferencia {
            archivo: archivo(c.ruta.as_str(), c.despues.as_ref()),
            marca: if c.despues.is_none() {
                v::Marca::Retirada
            } else if c.antes.is_some() {
                v::Marca::Actualizacion
            } else {
                v::Marca::Alta
            },
            del_anfitrion: c
                .despues
                .as_ref()
                .is_some_and(|a| matches!(a.procedencia, Procedencia::Propio)),
            resolucion: None,
        }
    }));
    diferencias.extend(r.plan.conservados.iter().map(|c| v::Diferencia {
        archivo: archivo(c.as_str(), None),
        marca: v::Marca::Conservado,
        del_anfitrion: false,
        resolucion: Some(v::Resolucion::ConservarMia),
    }));
    if diferencias.is_empty() && !r.sin_cambios() {
        diferencias.push(v::Diferencia {
            archivo: v::ArchivoCompartido {
                id: "@revision".into(),
                nombre: "Registro de la revisión".into(),
                detalle: "No hay archivos que descargar; se guardará la revisión comprobada."
                    .into(),
                ..Default::default()
            },
            marca: v::Marca::Actualizacion,
            del_anfitrion: false,
            resolucion: None,
        });
    }
    Some(v::RevisionSincronizacion {
        instancia: r.instancia.nombre.clone(),
        anfitrion: "Equipo anfitrión".into(),
        revision: r.plan.revision.revision,
        estado: if r.sin_cambios() || s.sincronizar.completada {
            v::EstadoRevisionInvitado::SinCambios
        } else {
            v::EstadoRevisionInvitado::Diferencias
        },
        diferencias,
        conexion: None,
    })
}
pub fn vista<'a>(p: Paleta, s: &Estado) -> Option<Element<'a, v::Accion>> {
    match s.etapa {
        Etapa::RevisionVinculo => Some(v::revision_sincronizacion(p, &revision(s)?)),
        Etapa::ErrorVinculo | Etapa::ConsultandoVinculo | Etapa::AplicandoVinculo => {
            Some(v::progreso_sincronizacion(
                p,
                &v::ProgresoSincronizacion {
                    instancia: s.sincronizar.instancia.as_ref()?.nombre.clone(),
                    anfitrion: "Equipo anfitrión".into(),
                    revision: s
                        .sincronizar
                        .revision
                        .as_ref()
                        .map_or(0, |r| r.plan.revision.revision),
                    estado: if s.etapa == Etapa::ErrorVinculo {
                        v::EstadoProgreso::Error(Box::new(error(s)))
                    } else {
                        v::EstadoProgreso::Activo(s.invitado.avance.fase)
                    },
                    porcentaje: s.invitado.avance.porcentaje(),
                    detalle: if s.etapa == Etapa::ConsultandoVinculo {
                        "Consultando la publicación; todavía no se descarga contenido.".into()
                    } else {
                        s.invitado.avance.descripcion()
                    },
                    archivo: s.invitado.avance.archivo.clone(),
                },
            ))
        }
        _ => None,
    }
}

fn error(s: &Estado) -> v::ErrorCompartir {
    v::ErrorCompartir {
        tipo: v::TipoError::Hash,
        titulo: "No se pudo comprobar o actualizar la publicación".into(),
        explicacion: s.sincronizar.error.clone().unwrap_or_default(),
        conservacion: "La instancia y tus archivos personales se conservan. Vuelve a revisar antes de jugar.".into(),
        detalle: s.sincronizar.detalle.then(|| "La consulta requiere al anfitrión conectado y tu acceso vigente. No se inicia el juego después de un error.".into()),
    }
}
