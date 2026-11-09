// SPDX-License-Identifier: GPL-3.0-only
use super::{diario::Huella, estado, rutas, Error};
use std::path::Path;

pub(super) fn preparar(
    juego: &Path,
    anteriores: &[String],
    nuevos: &[String],
    revision: Option<&crate::compartir::packs::RevisionOrden>,
    plan: &crate::compartir::plan::PlanCompartido,
) -> Result<Option<(Option<Huella>, Vec<u8>)>, Error> {
    let path = rutas::resolver(juego, "options.txt", false)?;
    let original = estado::leer_acotado(&path, 4 * 1024 * 1024)?;
    let antes = original.as_ref().map(|b| estado::de_bytes(b));
    let bytes = original.unwrap_or_default();
    let texto = std::str::from_utf8(&bytes).map_err(|_| Error::Datos("options.txt no es UTF-8"))?;
    let mut lineas = texto
        .lines()
        .filter_map(|l| l.strip_prefix("resourcePacks:"));
    let actual: Vec<String> = match lineas.next() {
        Some(valor) => {
            serde_json::from_str(valor).map_err(|_| Error::Datos("selección de resourcepacks"))?
        }
        None => vec![],
    };
    if lineas.next().is_some() {
        return Err(Error::Datos("selección de packs duplicada"));
    }
    let mut orden = match revision {
        Some(revision) => {
            if !revision.corresponde(anteriores, nuevos) {
                return Err(Error::Datos("selección ajena a la revisión"));
            }
            revision.aplicar(&actual)?
        }
        None => crate::compartir::packs::combinar_orden(&actual, anteriores, nuevos)?,
    };
    // No activar ZIP ni carpetas cuyo contenido se decidió mantener ausente.
    // Un alta planificada existe en el estado final aunque aún no se descargó.
    let cambios: std::collections::BTreeMap<_, _> = plan
        .cambios
        .iter()
        .map(|c| (c.ruta.as_str(), c.despues.is_some()))
        .collect();
    let mut ausentes = std::collections::BTreeSet::new();
    for pack in &orden {
        let Some(nombre) = pack.strip_prefix("file/") else {
            continue;
        };
        if !nuevos.contains(pack) {
            continue;
        }
        let raiz = format!("resourcepacks/{nombre}");
        let prefijo = format!("{raiz}/");
        let mut existe = false;
        for a in plan
            .revision
            .archivos
            .iter()
            .filter(|a| a.ruta.as_str() == raiz || a.ruta.as_str().starts_with(&prefijo))
        {
            existe = match cambios.get(a.ruta.as_str()) {
                Some(presente) => *presente,
                None => presente(juego, &a.ruta)?,
            };
            if existe {
                break;
            }
        }
        if !existe {
            ausentes.insert(pack.clone());
        }
    }
    orden.retain(|p| !ausentes.contains(p));
    if orden == actual {
        return Ok(None);
    }
    let valor = format!(
        "resourcePacks:{}",
        serde_json::to_string(&orden).map_err(|_| Error::Datos("selección de packs"))?
    );
    let mut salida = String::with_capacity(texto.len() + valor.len());
    let mut reemplazada = false;
    for linea in texto.split_inclusive('\n') {
        if linea.starts_with("resourcePacks:") {
            salida.push_str(&valor);
            if linea.ends_with("\r\n") {
                salida.push_str("\r\n");
            } else if linea.ends_with('\n') {
                salida.push('\n');
            }
            reemplazada = true;
        } else {
            salida.push_str(linea);
        }
    }
    if !reemplazada {
        let fin = if texto.contains("\r\n") { "\r\n" } else { "\n" };
        if !salida.is_empty() && !salida.ends_with('\n') {
            salida.push_str(fin);
        }
        salida.push_str(&valor);
        salida.push_str(fin);
    }
    Ok(Some((antes, salida.into_bytes())))
}

fn presente(juego: &Path, ruta: &crate::compartir::RutaCompartida) -> Result<bool, Error> {
    let mut path = juego.to_path_buf();
    for parte in ruta.as_str().split('/') {
        path.push(parte);
        match std::fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(crate::compartir::diagnostico::disco(e)),
            Ok(_) => {
                crate::almacenamiento::regular(&path).map_err(|_| Error::CambioExterno)?;
            }
        }
    }
    Ok(true)
}
