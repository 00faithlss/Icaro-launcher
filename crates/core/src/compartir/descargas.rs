// SPDX-License-Identifier: GPL-3.0-only
//! Descarga por contenido, caché verificada y concurrencia acotada por proveedor.
mod cola;
mod grupos;
mod metricas;
mod obtencion;
use super::{
    aplicacion::{self, ContextoDestino, FuenteArchivos, PlanPreparado},
    cache::{CacheContenido, GuardiaUso},
    comprobar_cancelacion,
    plan::PlanCompartido,
    transporte::Cliente,
    Artefacto, Error,
};
use crate::{diagnostico_contexto as d, modrinth::Catalog};
pub use metricas::{EstadisticasDescarga, MetricasDescarga};
use std::{
    collections::BTreeMap,
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::{atomic::Ordering, Arc},
};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct OpcionesDescarga {
    pub cache: Option<CacheContenido>,
    pub limite_anfitrion: usize,
    pub limite_cdn: usize,
    /// Crear opciones nuevas por operación; lectura segura desde cualquier hilo.
    pub metricas: MetricasDescarga,
}
impl Default for OpcionesDescarga {
    fn default() -> Self {
        Self {
            cache: None,
            limite_anfitrion: 2,
            limite_cdn: 4,
            metricas: Default::default(),
        }
    }
}
struct Descargados {
    archivos: BTreeMap<String, (Artefacto, PathBuf)>,
    // La fuente se mueve al trabajo bloqueante: no liberar por drop del futuro.
    _uso: Option<Arc<GuardiaUso>>,
    _staging: fs::File,
}
#[derive(Debug, Clone)]
pub enum EventoDescarga {
    Archivos {
        ruta: String,
        hechos: usize,
        total: usize,
    },
    Verificacion {
        total: usize,
    },
}
impl FuenteArchivos for Descargados {
    fn abrir(&self, archivo: &Artefacto) -> Result<Box<dyn Read + Send>, Error> {
        let (esperado, path) = self
            .archivos
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
        Ok(Box::new(
            fs::File::open(path).map_err(super::diagnostico::disco)?,
        ))
    }
}
pub async fn preparar(
    contexto: &ContextoDestino,
    plan: PlanCompartido,
    cliente: &Cliente,
    catalogo: Option<&Catalog>,
    staging: &Path,
    cancel: &CancellationToken,
) -> Result<PlanPreparado, Error> {
    preparar_con_progreso(contexto, plan, cliente, catalogo, staging, cancel, |_| {}).await
}
pub async fn preparar_con_progreso(
    contexto: &ContextoDestino,
    plan: PlanCompartido,
    cliente: &Cliente,
    catalogo: Option<&Catalog>,
    staging: &Path,
    cancel: &CancellationToken,
    progreso: impl Fn(EventoDescarga) + Send + Sync,
) -> Result<PlanPreparado, Error> {
    preparar_configurado(
        contexto,
        plan,
        cliente,
        catalogo,
        staging,
        cancel,
        &OpcionesDescarga::default(),
        progreso,
    )
    .await
}
pub async fn preparar_configurado(
    contexto: &ContextoDestino,
    plan: PlanCompartido,
    cliente: &Cliente,
    catalogo: Option<&Catalog>,
    staging: &Path,
    cancel: &CancellationToken,
    opciones: &OpcionesDescarga,
    progreso: impl Fn(EventoDescarga) + Send + Sync,
) -> Result<PlanPreparado, Error> {
    let token = cancel.child_token();
    let _detener = token.clone().drop_guard();
    let resultado = preparar_interno(
        contexto, plan, cliente, catalogo, staging, &token, opciones, &progreso,
    )
    .await;
    if resultado.is_err() {
        token.cancel();
    }
    opciones.metricas.registrar();
    // Mejor esfuerzo: una caché ocupada/cancelada no convierte éxito en fallo.
    if let Some(cache) = &opciones.cache {
        if let Err(e) = cache.limpiar(cancel).await {
            if !matches!(e, Error::Cancelada) {
                d::registrar_error(&e);
            }
        }
    }
    resultado
}
async fn preparar_interno(
    contexto: &ContextoDestino,
    plan: PlanCompartido,
    cliente: &Cliente,
    catalogo: Option<&Catalog>,
    staging: &Path,
    cancel: &CancellationToken,
    opciones: &OpcionesDescarga,
    progreso: &(impl Fn(EventoDescarga) + Send + Sync),
) -> Result<PlanPreparado, Error> {
    comprobar_cancelacion(cancel)?;
    plan.revision.validar(&contexto.limites)?;
    if !plan.puede_aplicar() {
        return Err(Error::Datos("hay conflictos sin resolver"));
    }
    let ctx = contexto.clone();
    let comprobacion = plan.clone();
    d::bloqueante(move || aplicacion::comprobar_opciones(&ctx, &comprobacion))
        .await
        .map_err(|_| Error::Disco)??;
    let directorio = staging.to_path_buf();
    let guardia = d::bloqueante(move || {
        let guardia = super::mantenimiento::reservar(&directorio)?;
        super::mantenimiento::limpiar(
            &directorio,
            std::time::SystemTime::now(),
            std::time::Duration::from_secs(7 * 86400),
        )?;
        Ok::<_, Error>(guardia)
    })
    .await
    .map_err(|_| Error::Disco)??;
    let archivos: Vec<_> = plan
        .cambios
        .iter()
        .filter_map(|c| c.despues.clone())
        .collect();
    let indice: BTreeMap<_, _> = plan
        .revision
        .archivos
        .iter()
        .map(|a| (a.ruta.as_str(), a))
        .collect();
    if archivos
        .iter()
        .any(|a| indice.get(a.ruta.as_str()).copied() != Some(a))
    {
        return Err(Error::Datos("archivo fuera de la revisión"));
    }
    let total = archivos.len();
    let mut grupos = grupos::agrupar(archivos)?;
    let m = &opciones.metricas;
    m.0.archivos.store(total as u64, Ordering::Relaxed);
    m.0.objetos.store(grupos.len() as u64, Ordering::Relaxed);
    m.0.duplicados_evitados
        .store((total - grupos.len()) as u64, Ordering::Relaxed);
    d::etapa("cache_contenido");
    let uso = if let Some(cache) = &opciones.cache {
        if let Err(e) = cache.limpiar(cancel).await {
            if !matches!(e, Error::Cancelada) {
                d::registrar_error(&e);
            }
        }
        Some(cache.sesion(cancel).await?)
    } else {
        None
    };
    obtencion::buscar(&mut grupos, opciones.cache.as_ref(), uso.clone(), cancel).await?;
    let necesarios = grupos::versiones_necesarias(&grupos);
    let mut versiones = BTreeMap::new();
    d::etapa("catalogo_faltante");
    for lote in necesarios.chunks(512) {
        let catalogo = catalogo.ok_or(Error::Datos("catálogo no disponible"))?;
        let respuesta = tokio::select! {
            _=cancel.cancelled()=>return Err(Error::Cancelada),
            r=catalogo.versions_by_ids(lote)=>r.map_err(|e|{d::registrar_error(&e);Error::Conexion})?,
        };
        m.0.lotes_catalogo.fetch_add(1, Ordering::Relaxed);
        versiones.extend(respuesta);
    }
    grupos::validar_catalogo(&mut grupos, &versiones)?;
    drop(versiones);
    let tamanos_tx: Vec<_> = grupos
        .iter()
        .flat_map(|g| &g.destinos)
        .map(|a| a.bytes)
        .collect();
    let tamanos_red: Vec<_> = grupos
        .iter()
        .filter(|g| g.cache.is_none())
        .map(|g| g.archivo.bytes)
        .collect();
    let ctx = contexto.clone();
    let disco_red = opciones
        .cache
        .as_ref()
        .map_or_else(|| staging.to_path_buf(), |c| c.raiz().to_path_buf());
    d::bloqueante(move || {
        super::espacio::comprobar(&ctx.juego, &tamanos_tx)?;
        if !tamanos_red.is_empty() {
            super::espacio::comprobar(&disco_red, &tamanos_red)?;
        }
        Ok::<_, Error>(())
    })
    .await
    .map_err(|_| Error::Disco)??;
    let mut descargados = Descargados {
        archivos: BTreeMap::new(),
        _uso: uso.clone(),
        _staging: guardia,
    };
    let mut hechos = 0;
    progreso(EventoDescarga::Archivos {
        ruta: String::new(),
        hechos,
        total,
    });
    let mut trabajos = Vec::new();
    for g in grupos {
        if let Some(path) = g.cache.clone().filter(|_| g.confirmar.is_empty()) {
            m.0.aciertos_cache.fetch_add(1, Ordering::Relaxed);
            m.0.bytes_cache_reutilizados
                .fetch_add(g.archivo.bytes, Ordering::Relaxed);
            completar(&mut descargados, g, path, &mut hechos, total, progreso);
        } else {
            trabajos.push((g.propio(), g));
        }
    }
    // Una sola validación/índice de la revisión para todas las descargas propias.
    let revision = cliente.revision_para_descargar(&plan.revision)?;
    d::etapa("descargas_paralelas");
    cola::ejecutar(
        trabajos,
        opciones.limite_anfitrion,
        opciones.limite_cdn,
        cancel,
        |g| {
            Box::pin(obtencion::obtener(
                g,
                &revision,
                catalogo,
                staging,
                opciones,
                uso.clone(),
                cancel,
            ))
        },
        |(g, path)| completar(&mut descargados, g, path, &mut hechos, total, progreso),
    )
    .await?;
    drop(revision);
    drop(uso);
    comprobar_cancelacion(cancel)?;
    progreso(EventoDescarga::Verificacion { total });
    d::etapa("preparacion_verificada");
    let ctx = contexto.clone();
    let token = cancel.clone();
    d::bloqueante(move || aplicacion::preparar(&ctx, plan, &descargados, &token))
        .await
        .map_err(|_| Error::Disco)?
}
fn completar(
    descargados: &mut Descargados,
    g: grupos::Grupo,
    path: PathBuf,
    hechos: &mut usize,
    total: usize,
    progreso: &impl Fn(EventoDescarga),
) {
    let ruta = g.archivo.ruta.as_str().to_string();
    for a in g.destinos {
        descargados
            .archivos
            .insert(a.ruta.as_str().into(), (a, path.clone()));
        *hechos += 1;
    }
    progreso(EventoDescarga::Archivos {
        ruta,
        hechos: *hechos,
        total,
    });
}
