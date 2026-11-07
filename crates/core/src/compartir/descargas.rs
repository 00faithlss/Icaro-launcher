// SPDX-License-Identifier: GPL-3.0-only
//! Descargar únicamente el plan revisado; la aplicación sigue siendo explícita.
use super::{
    aplicacion::{self, ContextoDestino, FuenteArchivos, PlanPreparado},
    comprobar_cancelacion,
    plan::PlanCompartido,
    transporte::Cliente,
    Artefacto, Error, Procedencia,
};
use crate::modrinth::Catalog;
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
};
use tokio_util::sync::CancellationToken;

struct Descargados(BTreeMap<String, (Artefacto, PathBuf)>);
impl FuenteArchivos for Descargados {
    fn abrir(&self, archivo: &Artefacto) -> Result<Box<dyn Read + Send>, Error> {
        let (esperado, path) = self
            .0
            .get(archivo.ruta.as_str())
            .ok_or(Error::Datos("descarga no preparada"))?;
        if esperado != archivo {
            return Err(Error::CambioExterno);
        }
        if !crate::almacenamiento::regular(path)
            .map_err(|_| Error::Disco)?
            .is_file()
        {
            return Err(Error::Disco);
        }
        Ok(Box::new(fs::File::open(path).map_err(|_| Error::Disco)?))
    }
}

/// Llamar después de revisar diferencias y resolver conflictos. Descarga fuera de
/// la instancia y devuelve una preparación bloqueada, nunca aplica por su cuenta.
pub async fn preparar(
    contexto: &ContextoDestino,
    plan: PlanCompartido,
    cliente: &Cliente,
    catalogo: Option<&Catalog>,
    staging: &Path,
    cancel: &CancellationToken,
) -> Result<PlanPreparado, Error> {
    comprobar_cancelacion(cancel)?;
    plan.revision.validar(&contexto.limites)?;
    if !plan.puede_aplicar() {
        return Err(Error::Datos("hay conflictos sin resolver"));
    }
    let archivos: Vec<_> = plan
        .cambios
        .iter()
        .filter_map(|c| c.despues.clone())
        .collect();
    if archivos.iter().any(|a| !plan.revision.archivos.contains(a)) {
        return Err(Error::Datos("archivo fuera de la revisión"));
    }
    let ids: std::collections::BTreeSet<_> = archivos
        .iter()
        .filter_map(|a| match &a.procedencia {
            Procedencia::Modrinth { version, .. } => Some(version.clone()),
            _ => None,
        })
        .collect();
    let mut versiones = BTreeMap::new();
    let ids: Vec<_> = ids.into_iter().collect();
    for lote in ids.chunks(512) {
        let catalogo = catalogo.ok_or(Error::Datos("catálogo no disponible"))?;
        let respuesta = tokio::select! {
            _ = cancel.cancelled() => return Err(Error::Cancelada),
            r = catalogo.versions_by_ids(lote) => r.map_err(|_| Error::Conexion)?,
        };
        versiones.extend(respuesta);
    }
    let mut descargados = Descargados(BTreeMap::new());
    for archivo in archivos {
        comprobar_cancelacion(cancel)?;
        let path = match &archivo.procedencia {
            Procedencia::Propio => {
                cliente
                    .descargar(&plan.revision, &archivo, staging, cancel)
                    .await?
            }
            Procedencia::Modrinth { proyecto, version } => {
                let encontrada = versiones
                    .get(version)
                    .filter(|v| v.id == *version && v.project_id == *proyecto)
                    .ok_or(Error::Datos("versión de catálogo distinta"))?;
                let file = encontrada
                    .files
                    .iter()
                    .find(|f| f.hashes.sha512 == archivo.sha512 && f.size == archivo.bytes)
                    .ok_or(Error::Datos("archivo de catálogo distinto"))?;
                let carpeta = staging.to_path_buf();
                let privada = tokio::task::spawn_blocking(move || {
                    fs::create_dir_all(&carpeta).map_err(|_| Error::Disco)?;
                    if !crate::almacenamiento::regular(&carpeta)
                        .map_err(|_| Error::Disco)?
                        .is_dir()
                    {
                        return Err(Error::Disco);
                    }
                    let mut nonce = [0u8; 16];
                    getrandom::fill(&mut nonce).map_err(|_| Error::Disco)?;
                    let nombre: String = nonce.iter().map(|b| format!("{b:02x}")).collect();
                    let privada = carpeta.join(nombre);
                    fs::create_dir(&privada).map_err(|_| Error::Disco)?;
                    Ok(privada)
                })
                .await
                .map_err(|_| Error::Disco)??;
                let destino = privada.join("catalogo.part");
                let catalogo = catalogo.ok_or(Error::Datos("catálogo no disponible"))?;
                let hash = tokio::select! {
                    _ = cancel.cancelled() => return Err(Error::Cancelada),
                    r = catalogo.download_mod_to(file, &destino) => r.map_err(|_| Error::Conexion)?,
                };
                if hash != archivo.sha512 {
                    return Err(Error::Datos("integridad del catálogo"));
                }
                destino
            }
        };
        descargados
            .0
            .insert(archivo.ruta.as_str().into(), (archivo, path));
    }
    comprobar_cancelacion(cancel)?;
    let contexto = contexto.clone();
    let cancel = cancel.clone();
    tokio::task::spawn_blocking(move || {
        aplicacion::preparar(&contexto, plan, &descargados, &cancel)
    })
    .await
    .map_err(|_| Error::Disco)?
}
