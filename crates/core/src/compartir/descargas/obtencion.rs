// SPDX-License-Identifier: GPL-3.0-only
use super::{grupos::Grupo, OpcionesDescarga};
use crate::{
    compartir::{
        cache::{CacheContenido, GuardiaUso},
        transporte::RevisionDescarga,
        Error, Procedencia,
    },
    diagnostico_contexto as d,
    modrinth::Catalog,
};
use std::{
    path::{Path, PathBuf},
    sync::{atomic::Ordering, Arc},
};
use tokio_util::sync::CancellationToken;

pub(super) async fn buscar(
    grupos: &mut [Grupo],
    cache: Option<&CacheContenido>,
    uso: Option<Arc<GuardiaUso>>,
    cancel: &CancellationToken,
) -> Result<(), Error> {
    for g in grupos {
        crate::compartir::comprobar_cancelacion(cancel)?;
        if let (Some(cache), Some(uso)) = (cache, uso.clone()) {
            let r = cache
                .reservar(&g.archivo.sha512, g.archivo.bytes, uso, cancel)
                .await?;
            g.cache = r.buscar(cancel).await?;
            for a in &g.destinos {
                if matches!(a.procedencia, Procedencia::Modrinth { .. })
                    && !g.confirmar.contains(&a.procedencia)
                    && (g.cache.is_none()
                        || !r.procedencia_confirmada(&a.procedencia, cancel).await?)
                {
                    g.confirmar.push(a.procedencia.clone());
                }
            }
        } else {
            for a in &g.destinos {
                if matches!(a.procedencia, Procedencia::Modrinth { .. })
                    && !g.confirmar.contains(&a.procedencia)
                {
                    g.confirmar.push(a.procedencia.clone());
                }
            }
        }
    }
    Ok(())
}
pub(super) async fn obtener(
    g: Grupo,
    revision: &RevisionDescarga<'_>,
    catalogo: Option<&Catalog>,
    staging: &Path,
    opciones: &OpcionesDescarga,
    uso: Option<Arc<GuardiaUso>>,
    cancel: &CancellationToken,
) -> Result<(Grupo, PathBuf), Error> {
    let m = &opciones.metricas;
    if let (Some(cache), Some(uso)) = (&opciones.cache, uso) {
        let r = cache
            .reservar(&g.archivo.sha512, g.archivo.bytes, uso, cancel)
            .await?;
        if let Some(path) = r.buscar(cancel).await? {
            for p in &g.confirmar {
                r.confirmar_procedencia(p, cancel).await?;
            }
            m.0.aciertos_cache.fetch_add(1, Ordering::Relaxed);
            m.0.bytes_cache_reutilizados
                .fetch_add(g.archivo.bytes, Ordering::Relaxed);
            return Ok((g, path));
        }
        m.0.fallos_cache.fetch_add(1, Ordering::Relaxed);
        let directorio = r.staging();
        descargar(&g, revision, catalogo, &directorio, opciones, cancel).await?;
        let path = r.publicar(cancel).await?;
        for a in &g.destinos {
            if matches!(a.procedencia, Procedencia::Modrinth { .. }) {
                r.confirmar_procedencia(&a.procedencia, cancel).await?;
            }
        }
        Ok((g, path))
    } else {
        let directorio = if g.propio() {
            staging.to_path_buf()
        } else {
            privado(staging).await?
        };
        let path = descargar(&g, revision, catalogo, &directorio, opciones, cancel).await?;
        Ok((g, path))
    }
}
async fn descargar(
    g: &Grupo,
    revision: &RevisionDescarga<'_>,
    catalogo: Option<&Catalog>,
    staging: &Path,
    opciones: &OpcionesDescarga,
    cancel: &CancellationToken,
) -> Result<PathBuf, Error> {
    let m = &opciones.metricas;
    let _actividad = m.iniciar(g.propio());
    let (path, bytes) = if g.propio() {
        m.0.descargas_propias.fetch_add(1, Ordering::Relaxed);
        revision
            .descargar_con_metricas(&g.archivo, staging, cancel)
            .await?
    } else {
        let file = g
            .catalogo
            .as_ref()
            .ok_or(Error::Datos("archivo de catálogo no revisado"))?;
        let catalogo = catalogo.ok_or(Error::Datos("catálogo no disponible"))?;
        let destino = staging.join(if opciones.cache.is_some() {
            format!("{}.part", g.archivo.sha512)
        } else {
            "catalogo.part".into()
        });
        let carpeta = staging.to_path_buf();
        d::bloqueante(move || {
            std::fs::create_dir_all(&carpeta).map_err(crate::compartir::diagnostico::disco)
        })
        .await
        .map_err(|_| Error::Disco)??;
        m.0.descargas_cdn.fetch_add(1, Ordering::Relaxed);
        let hash = tokio::select! {
            _=cancel.cancelled()=>return Err(Error::Cancelada),
            r=catalogo.download_mod_to(file,&destino)=>r.map_err(|e|{d::registrar_error(&e);Error::Conexion})?,
        };
        if hash != g.archivo.sha512 {
            return Err(Error::Datos("integridad del catálogo"));
        }
        (destino, g.archivo.bytes)
    };
    m.0.payload_red_verificado
        .fetch_add(bytes, Ordering::Relaxed);
    Ok(path)
}
async fn privado(staging: &Path) -> Result<PathBuf, Error> {
    let carpeta = staging.to_path_buf();
    d::bloqueante(move || {
        let mut nonce = [0u8; 16];
        getrandom::fill(&mut nonce).map_err(|_| Error::Disco)?;
        let nombre: String = nonce.iter().map(|b| format!("{b:02x}")).collect();
        let privada = carpeta.join(format!("catalogo-{nombre}"));
        std::fs::create_dir(&privada).map_err(crate::compartir::diagnostico::disco)?;
        Ok(privada)
    })
    .await
    .map_err(|_| Error::Disco)?
}
