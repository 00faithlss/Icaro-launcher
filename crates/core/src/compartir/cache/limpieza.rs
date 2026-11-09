// SPDX-License-Identifier: GPL-3.0-only
use super::{disco, CacheContenido, InformeCache};
use crate::compartir::{comprobar_cancelacion, diagnostico, Error};
use std::{fs, time::UNIX_EPOCH};
use tokio_util::sync::CancellationToken;

pub(super) fn limpiar(
    cache: &CacheContenido,
    cancel: &CancellationToken,
) -> Result<InformeCache, Error> {
    comprobar_cancelacion(cancel)?;
    if !cache.raiz.exists() {
        return Ok(InformeCache::default());
    }
    disco::directorio(&cache.raiz)?;
    let Some(_guardia) = disco::bloqueo(&cache.raiz.join(".uso.lock"), false)? else {
        return Ok(InformeCache {
            ocupada: true,
            ..Default::default()
        });
    };
    let ahora = disco::ahora();
    let edad = cache.edad.as_millis().min(u64::MAX as u128) as u64;
    let mut informe = InformeCache::default();
    let objetos = cache.raiz.join("objetos");
    let metadatos = cache.raiz.join("metadatos");
    if metadatos.exists() {
        disco::directorio(&metadatos)?;
    }
    let mut candidatos = Vec::new();
    let mut total = 0u64;
    if objetos.exists() {
        disco::directorio(&objetos)?;
        for (indice, entrada) in fs::read_dir(&objetos)
            .map_err(diagnostico::disco)?
            .enumerate()
        {
            if indice >= 100_000 {
                return Err(Error::Limite("entradas de caché"));
            }
            comprobar_cancelacion(cancel)?;
            let entrada = entrada.map_err(diagnostico::disco)?;
            let nombre = entrada.file_name();
            let Some(hash) = nombre.to_str().filter(|h| disco::hash_valido(h)) else {
                continue;
            };
            let Ok(Some(m)) = disco::archivo_regular_opcional(&entrada.path()) else {
                continue;
            };
            let metadata = cache.raiz.join("metadatos").join(format!("{hash}.json"));
            let usado = if metadata.exists() {
                disco::leer_meta(&metadata, hash, m.len())?.usado
            } else {
                m.modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_millis().min(u64::MAX as u128) as u64)
                    .unwrap_or(ahora)
            };
            total = total.saturating_add(m.len());
            candidatos.push((usado, entrada.path(), Some(metadata), m.len(), None));
        }
    }
    let parciales = cache.raiz.join("parciales");
    if parciales.exists() {
        disco::directorio(&parciales)?;
        for (indice, entrada) in fs::read_dir(&parciales)
            .map_err(diagnostico::disco)?
            .enumerate()
        {
            if indice >= 100_000 {
                return Err(Error::Limite("entradas de caché"));
            }
            comprobar_cancelacion(cancel)?;
            let entrada = entrada.map_err(diagnostico::disco)?;
            let nombre = entrada.file_name();
            let Some(hash) = nombre
                .to_str()
                .and_then(|n| n.strip_suffix(".part"))
                .filter(|h| disco::hash_valido(h))
            else {
                continue;
            };
            let Ok(Some(meta)) = disco::archivo_regular_opcional(&entrada.path()) else {
                continue;
            };
            let modificado = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_millis().min(u64::MAX as u128) as u64)
                .unwrap_or(ahora);
            total = total.saturating_add(meta.len());
            candidatos.push((
                modificado,
                entrada.path(),
                None,
                meta.len(),
                Some(parciales.join(format!("{hash}.lock"))),
            ));
        }
    }
    candidatos.sort_by_key(|c| c.0);
    for (usado, path, metadata, bytes, bloqueo) in candidatos {
        comprobar_cancelacion(cancel)?;
        if total <= cache.bytes && ahora.saturating_sub(usado) < edad {
            continue;
        }
        let _hash = if let Some(lock) = bloqueo {
            let Some(lock) = disco::bloqueo(&lock, false)? else {
                continue;
            };
            Some(lock)
        } else {
            None
        };
        if disco::archivo_regular_opcional(&path)?.is_some() {
            fs::remove_file(path).map_err(diagnostico::disco)?;
            total = total.saturating_sub(bytes);
            informe.archivos += 1;
            informe.bytes = informe.bytes.saturating_add(bytes);
            if let Some(metadata) = metadata {
                if disco::archivo_regular_opcional(&metadata)?.is_some() {
                    fs::remove_file(metadata).map_err(diagnostico::disco)?;
                }
            }
        }
    }
    Ok(informe)
}
