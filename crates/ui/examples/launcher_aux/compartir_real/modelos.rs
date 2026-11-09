// SPDX-License-Identifier: GPL-3.0-only
//! Traducción de datos comprobados a los componentes originales.
use super::tareas::Trabajo;
use icaro_core::{
    compartir::{
        metadatos::{self, MetadatosMod},
        publicacion::Eleccion as E,
        Artefacto, Procedencia, RutaCompartida,
    },
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
/// Tamaño legible con coma decimal: 19,7 KB, 2,2 MB. Nunca bytes crudos.
pub fn tamano(bytes: u64) -> String {
    const KB: f64 = 1024.;
    let b = bytes as f64;
    let texto = if bytes < 1024 {
        format!("{bytes} B")
    } else if b < KB * KB {
        format!("{:.1} KB", b / KB)
    } else if b < KB * KB * KB {
        format!("{:.1} MB", b / (KB * KB))
    } else {
        format!("{:.1} GB", b / (KB * KB * KB))
    };
    texto.replace('.', ",")
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
        tamano: tamano(a.bytes),
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

/// Columna Versión: `0.6.5`, `0.14.3 → 0.14.5` o Modificado.
fn version(
    tipo: v::TipoContenido,
    grupo: v::GrupoPublicacion,
    actual: Option<&MetadatosMod>,
    anterior: Option<&MetadatosMod>,
) -> (String, String) {
    let publicada = anterior.map(|m| m.version.clone()).unwrap_or_default();
    let texto = match grupo {
        v::GrupoPublicacion::Actualizaciones => match (anterior, actual) {
            (Some(a), Some(b)) if !a.version.is_empty() && !b.version.is_empty() && a.version != b.version => {
                format!("{} → {}", a.version, b.version)
            }
            _ => "Modificado".into(),
        },
        v::GrupoPublicacion::Retiradas => anterior
            .or(actual)
            .map(|m| m.version.clone())
            .unwrap_or_default(),
        _ if tipo == v::TipoContenido::Mod => actual.map(|m| m.version.clone()).unwrap_or_default(),
        _ => String::new(),
    };
    (texto, publicada)
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
    let pendientes: BTreeSet<_> = trabajo
        .borrador
        .autorizacion_pendiente()
        .into_iter()
        .map(|a| a.ruta)
        .collect();
    let meta = &trabajo.metadatos;
    // Configuraciones → mod que las usa, por modid de los JAR actuales.
    let mods_por_id: BTreeMap<&str, (&RutaCompartida, &MetadatosMod)> = meta
        .actuales
        .iter()
        .map(|(r, m)| (m.id.as_str(), (r, m)))
        .collect();
    let mut cambios = Vec::new();
    for (ruta, a) in todos {
        let Some(e) = trabajo.borrador.eleccion(&ruta) else {
            continue;
        };
        let actual = trabajo.borrador.archivo_actual(&ruta);
        let anterior = anteriores.get(&ruta);
        let privado_antes = trabajo.seleccion_inicial.es_privado(&ruta);
        let igual = actual
            .is_some_and(|f| anterior.is_some_and(|a| a.sha512 == f.sha512 && a.bytes == f.bytes));
        if igual && e != E::Privado && !privado_antes {
            continue;
        }
        let (grupo, marca, si, no, motivo) = if actual.is_none() {
            (
                v::GrupoPublicacion::Retiradas,
                v::Marca::Retirada,
                v::Eleccion::Retirar,
                v::Eleccion::Posponer,
                "Lo eliminaste de tu instancia",
            )
        } else if anterior.is_some() && (igual || privado_antes) {
            // Publicado antes y ahora privado: retirarlo es dejarlo privado.
            (
                v::GrupoPublicacion::Retiradas,
                v::Marca::Retirada,
                v::Eleccion::Privado,
                v::Eleccion::Incluir,
                "Decidiste dejar de compartirlo",
            )
        } else if anterior.is_some() {
            (
                v::GrupoPublicacion::Actualizaciones,
                v::Marca::Actualizacion,
                v::Eleccion::Incluir,
                v::Eleccion::Posponer,
                "",
            )
        } else if privado_antes {
            (
                v::GrupoPublicacion::Privado,
                v::Marca::Conservado,
                v::Eleccion::Compartir,
                v::Eleccion::Privado,
                "",
            )
        } else {
            (
                v::GrupoPublicacion::Nuevo,
                v::Marca::Alta,
                v::Eleccion::Compartir,
                v::Eleccion::Privado,
                "",
            )
        };
        let tipo = v::TipoContenido::de_ruta(ruta.as_str());
        let m_actual = meta.actuales.get(&ruta);
        let m_anterior = meta.anteriores.get(&ruta);
        let datos = m_actual.or(m_anterior);
        let mut archivo = archivo(&a, trabajo);
        if let Some(d) = datos {
            archivo.nombre = d.nombre.clone();
        }
        if actual.is_none() {
            archivo.tamano = String::new();
        }
        let (version, version_publicada) = version(tipo, grupo, m_actual, m_anterior);
        let version = if version.is_empty()
            && grupo == v::GrupoPublicacion::Actualizaciones
        {
            "Modificado".into()
        } else {
            version
        };
        let de_mod = (tipo == v::TipoContenido::Config)
            .then(|| metadatos::mod_de_configuracion(ruta.as_str(), mods_por_id.keys().copied()))
            .flatten()
            .and_then(|id| mods_por_id.get(id))
            .map(|(r, m)| (r.as_str().to_owned(), m.nombre.clone()));
        let solo_cliente = match tipo {
            v::TipoContenido::Mod => datos.is_some_and(|d| d.solo_cliente),
            v::TipoContenido::Script => ruta.as_str().starts_with("kubejs/client_scripts/"),
            v::TipoContenido::Shader => true,
            _ => false,
        };
        cambios.push(v::CambioPublicacion {
            archivo,
            tipo,
            grupo,
            marca,
            eleccion: eleccion(e),
            si,
            no,
            version,
            version_publicada,
            de_mod,
            propio: matches!(a.procedencia, Procedencia::Propio),
            autorizado: !pendientes.contains(&ruta),
            solo_cliente,
            integrado: false,
            problema: None,
            estado: motivo.into(),
        });
    }
    let packs_base = base.map_or(&[][..], |m| m.packs.as_slice());
    if packs_base != trabajo.borrador.packs_publicados() || trabajo.orden_pospuesto {
        let nuevos: Vec<_> = trabajo
            .activos
            .iter()
            .filter(|p| !packs_base.contains(p))
            .collect();
        let quitados = packs_base
            .iter()
            .filter(|p| !trabajo.activos.contains(p))
            .count();
        let version = match (nuevos.len(), quitados) {
            (0, 0) => "Cambió el orden".into(),
            (n, 0) => format!("{n} activado{}", if n == 1 { "" } else { "s" }),
            (0, q) => format!("{q} desactivado{}", if q == 1 { "" } else { "s" }),
            (n, q) => format!("{n} activados · {q} desactivados"),
        };
        cambios.push(v::CambioPublicacion {
            archivo: v::ArchivoCompartido {
                id: "@orden-resourcepacks".into(),
                nombre: "Activación y orden".into(),
                detalle: "options.txt · resourcePacks".into(),
                carpeta: "resourcepacks".into(),
                ..Default::default()
            },
            tipo: v::TipoContenido::Resourcepack,
            grupo: v::GrupoPublicacion::Cambios,
            marca: v::Marca::Actualizacion,
            eleccion: if trabajo.orden_pospuesto {
                v::Eleccion::Posponer
            } else {
                v::Eleccion::Incluir
            },
            si: v::Eleccion::Incluir,
            no: v::Eleccion::Posponer,
            version,
            version_publicada: String::new(),
            de_mod: None,
            propio: false,
            autorizado: true,
            solo_cliente: false,
            integrado: false,
            problema: None,
            estado: String::new(),
        });
    }
    // Sin cambios solo si lo único que hay es contenido privado desde antes, sin marcar.
    let sin_cambios = cambios
        .iter()
        .all(|c| c.grupo == v::GrupoPublicacion::Privado && !c.marcado());
    v::RevisionPublicacion {
        instancia: instancia(&trabajo.instancia, false),
        base: base.map_or(0, |m| m.revision),
        nueva: base.map_or(1, |m| m.revision.saturating_add(1)),
        estado: if sin_cambios {
            v::EstadoRevisionPublicacion::SinCambios
        } else {
            v::EstadoRevisionPublicacion::Disponible
        },
        cambios,
        confirmar_cancelacion: false,
        guardando: false,
        vista: Default::default(),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn tamano_legible_con_coma_y_sin_bytes_crudos() {
        assert_eq!(super::tamano(900), "900 B");
        assert_eq!(super::tamano(20_154), "19,7 KB");
        assert_eq!(super::tamano(2_301_337), "2,2 MB");
    }
}
