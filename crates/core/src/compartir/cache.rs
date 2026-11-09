// SPDX-License-Identifier: GPL-3.0-only
//! Almacén por contenido independiente de instancias y publicaciones.
mod disco;
mod limpieza;
use super::{comprobar_cancelacion, Error, Procedencia};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio_util::sync::CancellationToken;

#[derive(Clone)]
pub struct CacheContenido {
    raiz: PathBuf,
    bytes: u64,
    edad: Duration,
}
pub struct GuardiaUso {
    raiz: PathBuf,
    _archivo: fs::File,
}
#[derive(Clone)]
pub struct ReservaHash(Arc<Reserva>);
struct Reserva {
    cache: CacheContenido,
    hash: String,
    bytes: u64,
    _uso: Arc<GuardiaUso>,
    _archivo: fs::File,
    verificada: AtomicBool,
}
#[derive(Debug, Default)]
pub struct InformeCache {
    pub archivos: usize,
    pub bytes: u64,
    pub ocupada: bool,
}
impl CacheContenido {
    pub fn nueva(raiz: PathBuf) -> Self {
        Self {
            raiz,
            bytes: 1024 * 1024 * 1024,
            edad: Duration::from_secs(7 * 86400),
        }
    }
    pub fn raiz(&self) -> &Path {
        &self.raiz
    }
    pub fn con_limites(mut self, bytes: u64, edad: Duration) -> Self {
        self.bytes = bytes;
        self.edad = edad;
        self
    }
    pub async fn sesion(&self, cancel: &CancellationToken) -> Result<Arc<GuardiaUso>, Error> {
        comprobar_cancelacion(cancel)?;
        let cache = self.clone();
        let token = cancel.clone();
        let raiz = crate::diagnostico_contexto::bloqueante(move || {
            comprobar_cancelacion(&token)?;
            disco::directorio(&cache.raiz)?;
            for nombre in ["objetos", "parciales", "metadatos", "bloqueos"] {
                disco::directorio(&cache.raiz.join(nombre))?;
            }
            cache.raiz.canonicalize().map_err(super::diagnostico::disco)
        })
        .await
        .map_err(|_| Error::Disco)??;
        let archivo = disco::esperar_bloqueo(raiz.join(".uso.lock"), true, cancel).await?;
        Ok(Arc::new(GuardiaUso {
            raiz,
            _archivo: archivo,
        }))
    }
    pub async fn reservar(
        &self,
        hash: &str,
        bytes: u64,
        uso: Arc<GuardiaUso>,
        cancel: &CancellationToken,
    ) -> Result<ReservaHash, Error> {
        comprobar_cancelacion(cancel)?;
        if !disco::hash_valido(hash) {
            return Err(Error::Datos("hash de caché"));
        }
        let cache = self.clone();
        let guardia = uso.clone();
        let coincide = crate::diagnostico_contexto::bloqueante(move || {
            cache
                .raiz
                .canonicalize()
                .map(|r| r == guardia.raiz)
                .map_err(super::diagnostico::disco)
        })
        .await
        .map_err(|_| Error::Disco)??;
        if !coincide {
            return Err(Error::Datos("reserva de otra caché"));
        }
        let archivo = disco::esperar_bloqueo(
            uso.raiz.join("bloqueos").join(format!("{hash}.lock")),
            false,
            cancel,
        )
        .await?;
        let mut cache = self.clone();
        cache.raiz = uso.raiz.clone();
        Ok(ReservaHash(Arc::new(Reserva {
            cache,
            hash: hash.into(),
            bytes,
            _uso: uso,
            _archivo: archivo,
            verificada: AtomicBool::new(false),
        })))
    }
    pub async fn limpiar(&self, cancel: &CancellationToken) -> Result<InformeCache, Error> {
        let cache = self.clone();
        let token = cancel.clone();
        crate::diagnostico_contexto::bloqueante(move || limpieza::limpiar(&cache, &token))
            .await
            .map_err(|_| Error::Disco)?
    }
}
impl ReservaHash {
    pub fn staging(&self) -> PathBuf {
        self.0.cache.raiz.join("parciales")
    }
    fn objeto(&self) -> PathBuf {
        self.0.cache.raiz.join("objetos").join(&self.0.hash)
    }
    fn metadata(&self) -> PathBuf {
        self.0
            .cache
            .raiz
            .join("metadatos")
            .join(format!("{}.json", self.0.hash))
    }
    pub async fn buscar(&self, cancel: &CancellationToken) -> Result<Option<PathBuf>, Error> {
        let reserva = self.clone();
        let token = cancel.clone();
        crate::diagnostico_contexto::bloqueante(move || reserva.buscar_disco(&token))
            .await
            .map_err(|_| Error::Disco)?
    }
    fn buscar_disco(&self, cancel: &CancellationToken) -> Result<Option<PathBuf>, Error> {
        self.0.verificada.store(false, Ordering::Release);
        if !disco::verificar(&self.objeto(), &self.0.hash, self.0.bytes, cancel)? {
            return Ok(None);
        }
        let mut meta = disco::leer_meta(&self.metadata(), &self.0.hash, self.0.bytes)?;
        meta.usado = disco::ahora();
        disco::guardar_meta(&self.metadata(), &meta)?;
        self.0.verificada.store(true, Ordering::Release);
        Ok(Some(self.objeto()))
    }
    // La certeza dura únicamente esta reserva bloqueada; la transacción vuelve a
    // verificar mientras copia. Una nueva reserva siempre relee el contenido.
    fn comprobar_verificada(&self, cancel: &CancellationToken) -> Result<bool, Error> {
        comprobar_cancelacion(cancel)?;
        if self.0.verificada.load(Ordering::Acquire) {
            return Ok(disco::archivo_regular_opcional(&self.objeto())?
                .is_some_and(|m| m.len() == self.0.bytes));
        }
        let verificada = disco::verificar(&self.objeto(), &self.0.hash, self.0.bytes, cancel)?;
        self.0.verificada.store(verificada, Ordering::Release);
        Ok(verificada)
    }
    pub async fn publicar(&self, cancel: &CancellationToken) -> Result<PathBuf, Error> {
        let reserva = self.clone();
        let token = cancel.clone();
        crate::diagnostico_contexto::bloqueante(move || {
            reserva.0.verificada.store(false, Ordering::Release);
            let parcial = reserva.staging().join(format!("{}.part", reserva.0.hash));
            if !disco::verificar(&parcial, &reserva.0.hash, reserva.0.bytes, &token)? {
                return Err(Error::Datos("integridad de caché"));
            }
            comprobar_cancelacion(&token)?;
            disco::archivo_regular_opcional(&reserva.objeto())?;
            fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&parcial)
                .and_then(|f| f.sync_all())
                .map_err(super::diagnostico::disco)?;
            fs::rename(&parcial, reserva.objeto()).map_err(super::diagnostico::disco)?;
            let mut meta = disco::leer_meta(&reserva.metadata(), &reserva.0.hash, reserva.0.bytes)?;
            meta.usado = disco::ahora();
            disco::guardar_meta(&reserva.metadata(), &meta)?;
            reserva.0.verificada.store(true, Ordering::Release);
            Ok(reserva.objeto())
        })
        .await
        .map_err(|_| Error::Disco)?
    }
    pub async fn procedencia_confirmada(
        &self,
        procedencia: &Procedencia,
        cancel: &CancellationToken,
    ) -> Result<bool, Error> {
        comprobar_cancelacion(cancel)?;
        if matches!(procedencia, Procedencia::Propio) {
            return Ok(true);
        }
        let reserva = self.clone();
        let origen = procedencia.clone();
        let token = cancel.clone();
        crate::diagnostico_contexto::bloqueante(move || {
            if !reserva.comprobar_verificada(&token)? {
                return Ok(false);
            }
            let meta = disco::leer_meta(&reserva.metadata(), &reserva.0.hash, reserva.0.bytes)?;
            Ok(meta.procedencias.contains(&origen))
        })
        .await
        .map_err(|_| Error::Disco)?
    }
    pub async fn confirmar_procedencia(
        &self,
        procedencia: &Procedencia,
        cancel: &CancellationToken,
    ) -> Result<(), Error> {
        comprobar_cancelacion(cancel)?;
        if matches!(procedencia, Procedencia::Propio) {
            return Ok(());
        }
        let reserva = self.clone();
        let origen = procedencia.clone();
        let token = cancel.clone();
        crate::diagnostico_contexto::bloqueante(move || {
            disco::validar_procedencia(&origen)?;
            if !reserva.comprobar_verificada(&token)? {
                return Err(Error::Datos("contenido de caché no verificado"));
            }
            let mut meta = disco::leer_meta(&reserva.metadata(), &reserva.0.hash, reserva.0.bytes)?;
            if !meta.procedencias.contains(&origen) {
                if meta.procedencias.len() >= 128 {
                    return Err(Error::Limite("procedencias de caché"));
                }
                meta.procedencias.push(origen);
            }
            meta.usado = disco::ahora();
            disco::guardar_meta(&reserva.metadata(), &meta)
        })
        .await
        .map_err(|_| Error::Disco)?
    }
}
