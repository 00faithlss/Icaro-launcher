// SPDX-License-Identifier: GPL-3.0-only
use super::{endpoint, Modo};
use crate::compartir::{
    accesos::{Accesos, Autoridad},
    firmas::RevisionFirmada,
    identidad::Identidad,
    protocolo::{self, Respuesta, Solicitud, CONTROL_BYTES},
    revisiones::Revision,
    Error, Limites, Procedencia,
};
use iroh::{endpoint::Connection, Endpoint, EndpointAddr};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, RwLock},
};
use tokio::{
    io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt},
    sync::{Notify, Semaphore},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

struct Estado {
    identidad: Identidad,
    revision: Arc<RwLock<Revision>>,
    accesos: Arc<Accesos>,
    autoridad: Arc<dyn Autoridad>,
    limites: Limites,
    conexiones: Mutex<HashMap<String, Vec<Connection>>>,
    cupos: Mutex<HashMap<String, Arc<Semaphore>>>,
    actividad: Mutex<Actividad>,
    fin: Notify,
}
#[derive(Default)]
struct Actividad {
    drenando: bool,
    activas: usize,
}
struct Transferencia(Arc<Estado>);
impl Transferencia {
    fn iniciar(estado: Arc<Estado>) -> Result<Self, Error> {
        {
            let mut actividad = estado.actividad.lock().map_err(|_| Error::Disco)?;
            if actividad.drenando {
                return Err(Error::Datos("publicación terminando"));
            }
            actividad.activas += 1;
        }
        Ok(Self(estado))
    }
}
impl Drop for Transferencia {
    fn drop(&mut self) {
        if let Ok(mut actividad) = self.0.actividad.lock() {
            actividad.activas = actividad.activas.saturating_sub(1);
        }
        self.0.fin.notify_waiters();
    }
}
pub struct NodoAnfitrion {
    endpoint: Endpoint,
    estado: Arc<Estado>,
    cancel: CancellationToken,
    tarea: JoinHandle<()>,
}
impl NodoAnfitrion {
    pub async fn iniciar(
        identidad: Identidad,
        revision: Arc<RwLock<Revision>>,
        accesos: Arc<Accesos>,
        autoridad: Arc<dyn Autoridad>,
        modo: Modo,
        limites: Limites,
    ) -> Result<Self, Error> {
        {
            let vigente = revision.read().map_err(|_| Error::Disco)?;
            vigente.manifiesto.validar(&limites)?;
            if vigente.manifiesto.origen.anfitrion != identidad.clave_publica() {
                return Err(Error::Datos("identidad del publicador"));
            }
        }
        let endpoint = endpoint(&identidad, modo).await?;
        let estado = Arc::new(Estado {
            identidad,
            revision,
            accesos,
            autoridad,
            limites,
            conexiones: Mutex::new(HashMap::new()),
            cupos: Mutex::new(HashMap::new()),
            actividad: Mutex::new(Actividad::default()),
            fin: Notify::new(),
        });
        let cancel = CancellationToken::new();
        let tarea = tokio::spawn(aceptar(endpoint.clone(), estado.clone(), cancel.clone()));
        Ok(Self {
            endpoint,
            estado,
            cancel,
            tarea,
        })
    }
    pub fn direccion(&self) -> EndpointAddr {
        self.endpoint.addr()
    }
    pub fn contar_transferencias(&self) -> usize {
        self.estado.actividad.lock().map_or(0, |a| a.activas)
    }
    /// Rechaza nuevas conexiones/descargas y espera la entrega de las ya admitidas.
    /// Al terminar, el llamante puede cerrar el nodo conservando las autorizaciones.
    pub async fn drenar(&self) -> Result<(), Error> {
        self.estado
            .actividad
            .lock()
            .map_err(|_| Error::Disco)?
            .drenando = true;
        loop {
            let fin = self.estado.fin.notified();
            if self.contar_transferencias() == 0 {
                return Ok(());
            }
            fin.await;
        }
    }
    /// Reutiliza el endpoint y su identidad al conectar con otra publicación.
    pub async fn conectar(
        &self,
        direccion: EndpointAddr,
        origen: &crate::compartir::Origen,
    ) -> Result<super::Cliente, Error> {
        super::Cliente::desde_endpoint(
            self.endpoint.clone(),
            direccion,
            origen,
            self.estado.limites.clone(),
            false,
        )
        .await
    }
    pub async fn revocar(&self, invitado: &str) -> Result<(), Error> {
        let acl = self.estado.accesos.clone();
        let key = invitado.to_owned();
        tokio::task::spawn_blocking(move || acl.revocar(&key))
            .await
            .map_err(|_| Error::Disco)??;
        if let Some(conns) = self
            .estado
            .conexiones
            .lock()
            .map_err(|_| Error::Disco)?
            .remove(invitado)
        {
            for conn in conns {
                conn.close(1u32.into(), b"revocado");
            }
        }
        Ok(())
    }
    pub async fn parar(&self) {
        self.cancel.cancel();
        self.endpoint.close().await;
    }
}
impl Drop for NodoAnfitrion {
    fn drop(&mut self) {
        self.cancel.cancel();
        if let Ok(conexiones) = self.estado.conexiones.lock() {
            for conn in conexiones.values().flatten() {
                conn.close(0u32.into(), b"fin");
            }
        }
        self.tarea.abort();
    }
}
async fn aceptar(endpoint: Endpoint, estado: Arc<Estado>, cancel: CancellationToken) {
    let slots = Arc::new(Semaphore::new(32));
    let mut tareas = tokio::task::JoinSet::new();
    loop {
        tokio::select! {
            _ = cancel.cancelled() => break,
            Some(_) = tareas.join_next(), if !tareas.is_empty() => {},
            incoming = endpoint.accept() => {
                let Some(incoming) = incoming else { break };
                if estado.actividad.lock().map_or(true,|a|a.drenando) { incoming.refuse(); continue }
                let Ok(permiso) = slots.clone().try_acquire_owned() else { incoming.refuse(); continue };
                let estado = estado.clone();
                tareas.spawn(async move {
                    let _permiso = permiso;
                    let _ = conexion(incoming,estado).await;
                });
            }
        }
    }
    tareas.abort_all();
    while tareas.join_next().await.is_some() {}
}
async fn conexion(incoming: iroh::endpoint::Incoming, estado: Arc<Estado>) -> Result<(), Error> {
    let conn = tokio::time::timeout(std::time::Duration::from_secs(30), incoming)
        .await
        .map_err(|_| Error::Datos("tiempo de conexión"))?
        .map_err(|_| Error::Datos("conexión"))?;
    let key = conn.remote_id().to_string();
    let autorizado = match tokio::time::timeout(
        std::time::Duration::from_secs(30),
        estado.autoridad.autorizado(key.clone()),
    )
    .await
    {
        Ok(Ok(valor)) => valor,
        _ => {
            conn.close(1u32.into(), b"autoridad no disponible");
            return Err(Error::Datos("autoridad no disponible"));
        }
    };
    if !autorizado || !estado.accesos.permitir(&key) {
        conn.close(1u32.into(), b"sin autorizacion");
        return Err(Error::Datos("acceso no autorizado"));
    }
    let cupo = {
        let mut cupos = estado.cupos.lock().map_err(|_| Error::Disco)?;
        cupos
            .entry(key.clone())
            .or_insert_with(|| Arc::new(Semaphore::new(2)))
            .clone()
    };
    estado
        .conexiones
        .lock()
        .map_err(|_| Error::Disco)?
        .entry(key.clone())
        .or_default()
        .push(conn.clone());
    // Segunda comprobación cubre revocación concurrente con el registro de la conexión.
    if !estado.accesos.permitir(&key) {
        conn.close(1u32.into(), b"revocado");
        return Ok(());
    }
    let mut tareas = tokio::task::JoinSet::new();
    while let Ok((send, recv)) = conn.accept_bi().await {
        if !estado.accesos.permitir(&key) {
            break;
        }
        let Ok(permiso) = cupo.clone().try_acquire_owned() else {
            drop(send);
            drop(recv);
            continue;
        };
        let estado = estado.clone();
        tareas.spawn(async move {
            let _permiso = permiso;
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(120),
                servir(send, recv, estado),
            )
            .await;
        });
        while tareas.try_join_next().is_some() {}
    }
    conn.close(0u32.into(), b"fin");
    tareas.abort_all();
    if let Ok(mut conexiones) = estado.conexiones.lock() {
        if let Some(lista) = conexiones.get_mut(&key) {
            lista.retain(|c| c.stable_id() != conn.stable_id());
        }
    }
    Ok(())
}
async fn servir(
    mut send: iroh::endpoint::SendStream,
    mut recv: iroh::endpoint::RecvStream,
    estado: Arc<Estado>,
) -> Result<(), Error> {
    let solicitud: Solicitud = protocolo::leer(&mut recv, CONTROL_BYTES).await?;
    let _transferencia = if matches!(&solicitud, Solicitud::Archivo { .. }) {
        Some(Transferencia::iniciar(estado.clone())?)
    } else {
        None
    };
    let revision = estado.revision.read().map_err(|_| Error::Disco)?.clone();
    let max = estado
        .limites
        .manifiesto_bytes
        .saturating_mul(2)
        .saturating_add(CONTROL_BYTES);
    match solicitud {
        Solicitud::Revision { origen } if origen == revision.manifiesto.origen => {
            let firmada =
                RevisionFirmada::crear(&estado.identidad, &revision.manifiesto, &estado.limites)?;
            protocolo::escribir(&mut send, &Respuesta::Revision { firmada }, max).await?;
        }
        Solicitud::Archivo {
            origen,
            revision: numero,
            sha512,
            offset,
        } if origen == revision.manifiesto.origen && numero == revision.manifiesto.revision => {
            let artefacto = revision
                .manifiesto
                .archivos
                .iter()
                .find(|a| a.sha512 == sha512 && matches!(a.procedencia, Procedencia::Propio))
                .ok_or(Error::Datos("archivo no publicado"))?;
            if offset > artefacto.bytes || artefacto.bytes > estado.limites.archivo_bytes {
                return Err(Error::Datos("desplazamiento"));
            }
            let r = revision.clone();
            let ruta = artefacto.ruta.clone();
            let path = tokio::task::spawn_blocking(move || r.archivo(&ruta))
                .await
                .map_err(|_| Error::Disco)??;
            let mut archivo = tokio::fs::File::open(path)
                .await
                .map_err(|_| Error::Disco)?;
            archivo
                .seek(std::io::SeekFrom::Start(offset))
                .await
                .map_err(|_| Error::Disco)?;
            protocolo::escribir(
                &mut send,
                &Respuesta::Archivo {
                    restantes: artefacto.bytes - offset,
                },
                CONTROL_BYTES,
            )
            .await?;
            let mut buf = vec![0; 64 * 1024];
            let mut quedan = artefacto.bytes - offset;
            while quedan > 0 {
                let take = quedan.min(buf.len() as u64) as usize;
                let n = archivo
                    .read(&mut buf[..take])
                    .await
                    .map_err(|_| Error::Disco)?;
                if n == 0 {
                    return Err(Error::CambioExterno);
                }
                AsyncWriteExt::write_all(&mut send, &buf[..n])
                    .await
                    .map_err(|_| Error::Datos("conexión"))?;
                quedan -= n as u64;
            }
        }
        _ => protocolo::escribir(&mut send, &Respuesta::Rechazada, CONTROL_BYTES).await?,
    }
    send.finish().map_err(|_| Error::Datos("conexión"))?;
    if send
        .stopped()
        .await
        .map_err(|_| Error::Datos("conexión"))?
        .is_some()
    {
        return Err(Error::Cancelada);
    }
    Ok(())
}
