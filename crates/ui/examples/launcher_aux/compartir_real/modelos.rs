// SPDX-License-Identifier: GPL-3.0-only
//! Traducción de datos comprobados a los componentes originales.
use super::tareas::Trabajo;
use icaro_core::{
    compartir::{publicacion::Eleccion as E, Artefacto, Procedencia},
    instancias::Instancia,
};
use icaro_ui::compartir as v;
use std::collections::{BTreeMap, BTreeSet};

pub fn instancia(i: &Instancia, en_uso: bool) -> v::InstanciaCompartida {
    v::InstanciaCompartida {
        id: i.id.clone(),
        nombre: i.nombre.clone(),
        minecraft: i.minecraft.clone(),
        loader: i.loader.clone(),
        mods: 0,
        en_uso,
    }
}
pub fn archivo(a: &Artefacto, trabajo: &Trabajo) -> v::ArchivoCompartido {
    v::ArchivoCompartido {
        id: a.ruta.as_str().into(),
        nombre: a
            .ruta
            .as_str()
            .rsplit('/')
            .next()
            .unwrap_or_default()
            .into(),
        detalle: a.ruta.as_str().into(),
        tamano: format!("{} bytes", a.bytes),
        procedencia: match a.procedencia {
            Procedencia::Propio => "Archivo propio",
            _ => "Modrinth",
        }
        .into(),
        sha256: trabajo
            .inventario
            .archivos
            .binary_search_by(|f| f.ruta.cmp(&a.ruta))
            .ok()
            .and_then(|i| trabajo.inventario.archivos.get(i))
            .filter(|f| f.sha512 == a.sha512)
            .map_or_else(String::new, |f| f.sha256.clone()),
        carpeta: a.ruta.as_str().split('/').next().unwrap_or_default().into(),
    }
}
pub fn eleccion(e: E) -> v::Eleccion {
    match e {
        E::Compartir => v::Eleccion::Compartir,
        E::Privado => v::Eleccion::Privado,
        E::Incluir => v::Eleccion::Incluir,
        E::Posponer => v::Eleccion::Posponer,
        E::Retirar => v::Eleccion::Retirar,
    }
}
pub fn desde_vista(e: v::Eleccion) -> E {
    match e {
        v::Eleccion::Compartir => E::Compartir,
        v::Eleccion::Incluir => E::Incluir,
        v::Eleccion::Posponer => E::Posponer,
        v::Eleccion::Retirar => E::Retirar,
        _ => E::Privado,
    }
}
pub fn revision(trabajo: &Trabajo) -> v::RevisionPublicacion {
    let base = trabajo.anterior.as_ref().map(|r| &r.manifiesto);
    let anteriores: BTreeMap<_, _> = base
        .into_iter()
        .flat_map(|m| &m.archivos)
        .map(|a| (a.ruta.clone(), a))
        .collect();
    let mut todos: BTreeMap<_, _> = anteriores
        .iter()
        .map(|(r, a)| (r.clone(), (*a).clone()))
        .collect();
    for f in &trabajo.inventario.archivos {
        if let Some(a) = trabajo.borrador.archivo_actual(&f.ruta) {
            todos.insert(f.ruta.clone(), a.clone());
        }
    }
    let publicados: BTreeMap<_, _> = trabajo
        .borrador
        .archivos_publicados()
        .into_iter()
        .map(|a| (a.ruta.clone(), a))
        .collect();
    let pendientes: BTreeSet<_> = trabajo
        .borrador
        .autorizacion_pendiente()
        .into_iter()
        .map(|a| a.ruta)
        .collect();
    let mut cambios = Vec::new();
    for (ruta, mut a) in todos {
        let Some(e) = trabajo.borrador.eleccion(&ruta) else {
            continue;
        };
        let actual = trabajo.borrador.archivo_actual(&ruta);
        let anterior = anteriores.get(&ruta);
        if actual
            .is_some_and(|f| anterior.is_some_and(|a| a.sha512 == f.sha512 && a.bytes == f.bytes))
            && e != E::Privado
        {
            continue;
        }
        if let Some(publicado) = publicados.get(&ruta) {
            a = publicado.clone();
        }
        let (grupo, marca) = if actual.is_none() {
            (v::GrupoPublicacion::Retiradas, v::Marca::Retirada)
        } else if anterior.is_some() {
            (
                if ruta.as_str().starts_with("resourcepacks/") {
                    v::GrupoPublicacion::Resourcepacks
                } else {
                    v::GrupoPublicacion::Actualizaciones
                },
                v::Marca::Actualizacion,
            )
        } else {
            (
                if trabajo.seleccion.es_privado(&ruta) {
                    v::GrupoPublicacion::Privado
                } else {
                    v::GrupoPublicacion::Nuevo
                },
                v::Marca::Alta,
            )
        };
        cambios.push(v::CambioPublicacion {
            archivo: archivo(&a, trabajo),
            grupo,
            marca,
            eleccion: eleccion(e),
            propio: matches!(a.procedencia, Procedencia::Propio),
            autorizado: !pendientes.contains(&ruta),
            solo_cliente: false,
            integrado: false,
            problema: None,
            estado: String::new(),
        });
    }
    if base.map_or(&[][..], |m| m.packs.as_slice()) != trabajo.borrador.packs_publicados()
        || trabajo.orden_pospuesto
    {
        cambios.push(v::CambioPublicacion {
            archivo: v::ArchivoCompartido {
                id: "@orden-resourcepacks".into(),
                nombre: "Activación y orden de resourcepacks".into(),
                detalle: format!("{} packs activos en la instancia", trabajo.activos.len()),
                carpeta: "resourcepacks".into(),
                ..Default::default()
            },
            grupo: v::GrupoPublicacion::Resourcepacks,
            marca: v::Marca::Actualizacion,
            eleccion: if trabajo.orden_pospuesto {
                v::Eleccion::Posponer
            } else {
                v::Eleccion::Incluir
            },
            propio: false,
            autorizado: true,
            solo_cliente: false,
            integrado: false,
            problema: None,
            estado: String::new(),
        });
    }
    v::RevisionPublicacion {
        instancia: instancia(&trabajo.instancia, false),
        base: base.map_or(0, |m| m.revision),
        nueva: base.map_or(1, |m| m.revision.saturating_add(1)),
        estado: if cambios.is_empty() {
            v::EstadoRevisionPublicacion::SinCambios
        } else {
            v::EstadoRevisionPublicacion::Disponible
        },
        cambios,
        confirmar_cancelacion: false,
    }
}
