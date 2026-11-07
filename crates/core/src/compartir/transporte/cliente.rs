// SPDX-License-Identifier: GPL-3.0-only
use super::{endpoint, Modo};
use crate::compartir::{
    comprobar_cancelacion,
    identidad::Identidad,
    protocolo::{self, Respuesta, Solicitud, ALPN, CONTROL_BYTES},
    Artefacto, Error, Limites, Manifiesto, Origen, Procedencia,
};
use iroh::{endpoint::Connection, Endpoint, EndpointAddr};
use sha2::{Digest, Sha512};
use std::path::{Path, PathBuf};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::sync::CancellationToken;

pub struct Cliente {
    endpoint: Endpoint,
    endpoint_propio: bool,
    conn: Connection,
    origen: Origen,
    limites: Limites,
}
impl Drop for Cliente {
    fn drop(&mut self) {
        self.conn.close(0u32.into(), b"fin");
    }
}
impl Cliente {
    pub async fn conectar(
        identidad: &Identidad,
        direccion: EndpointAddr,
        origen: &Origen,
        modo: Modo,
        limites: Limites,
    ) -> Result<Self, Error> {
        let endpoint = endpoint(identidad, modo).await?;
        Self::desde_endpoint(endpoint, direccion, origen, limites, true).await
    }
    pub(super) async fn desde_endpoint(
        endpoint: Endpoint,
        direccion: EndpointAddr,
        origen: &Origen,
        limites: Limites,
        endpoint_propio: bool,
    ) -> Result<Self, Error> {
        origen.validar()?;
        if direccion.id.to_string() != origen.anfitrion {
            return Err(Error::Datos("identidad remota"));
        }
        let conn = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            endpoint.connect(direccion, ALPN),
        )
        .await
        .map_err(|_| Error::Datos("tiempo de conexión"))?
        .map_err(|_| Error::Datos("conexión"))?;
        if conn.remote_id().to_string() != origen.anfitrion {
            return Err(Error::Datos("identidad remota"));
        }
        Ok(Self {
            endpoint,
            endpoint_propio,
            conn,
            origen: origen.clone(),
            limites,
        })
    }
    pub async fn consultar(&self) -> Result<Manifiesto, Error> {
        let (mut send, mut recv) = self
            .conn
            .open_bi()
            .await
            .map_err(|_| Error::Datos("conexión"))?;
        protocolo::escribir(
            &mut send,
            &Solicitud::Revision {
                origen: self.origen.clone(),
            },
            CONTROL_BYTES,
        )
        .await?;
        send.finish().map_err(|_| Error::Datos("conexión"))?;
        let max = self
            .limites
            .manifiesto_bytes
            .saturating_mul(2)
            .saturating_add(CONTROL_BYTES);
        let respuesta: Respuesta = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            protocolo::leer(&mut recv, max),
        )
        .await
        .map_err(|_| Error::Datos("tiempo de consulta"))??;
        let Respuesta::Revision { firmada } = respuesta else {
            return Err(Error::Datos("consulta rechazada"));
        };
        let manifiesto = firmada.verificar(&self.origen.anfitrion, &self.limites)?;
        if manifiesto.origen != self.origen {
            return Err(Error::Datos("publicación distinta"));
        }
        Ok(manifiesto)
    }
    /// Descarga al staging por hash. Conserva el parcial en cancelación/corte y verifica el
    /// archivo completo antes de devolverlo; nunca escribe dentro de la instancia.
    pub async fn descargar(
        &self,
        manifiesto: &Manifiesto,
        artefacto: &Artefacto,
        staging: &Path,
        cancel: &CancellationToken,
    ) -> Result<PathBuf, Error> {
        comprobar_cancelacion(cancel)?;
        manifiesto.validar(&self.limites)?;
        if manifiesto.origen != self.origen
            || !manifiesto.archivos.contains(artefacto)
            || !matches!(artefacto.procedencia, Procedencia::Propio)
        {
            return Err(Error::Datos("archivo no publicado"));
        }
        let directorio = staging.to_owned();
        let hash = artefacto.sha512.clone();
        let _bloqueo = tokio::task::spawn_blocking(move || {
            std::fs::create_dir_all(&directorio).map_err(|_| Error::Disco)?;
            if !crate::almacenamiento::regular(&directorio)
                .map_err(|_| Error::Disco)?
                .is_dir()
            {
                return Err(Error::Disco);
            }
            let lock = directorio.join(format!("{hash}.lock"));
            if lock.exists() {
                crate::almacenamiento::regular(&lock).map_err(|_| Error::Disco)?;
            }
            let file = std::fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .write(true)
                .open(lock)
                .map_err(|_| Error::Disco)?;
            file.try_lock()
                .map_err(|_| Error::Datos("descarga ocupada"))?;
            Ok(file)
        })
        .await
        .map_err(|_| Error::Disco)??;
        let path = staging.join(format!("{}.part", artefacto.sha512));
        let check = path.clone();
        let offset = tokio::task::spawn_blocking(move || match std::fs::symlink_metadata(&check) {
            Ok(_) => crate::almacenamiento::regular(&check)
                .map(|m| m.len())
                .map_err(|_| Error::Disco),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(0),
            Err(_) => Err(Error::Disco),
        })
        .await
        .map_err(|_| Error::Disco)??;
        if offset > artefacto.bytes {
            return Err(Error::Datos("parcial demasiado grande"));
        }
        let transferencia = async {
            let inicio = async {
                let (mut send, mut recv) = self
                    .conn
                    .open_bi()
                    .await
                    .map_err(|_| Error::Datos("conexión"))?;
                protocolo::escribir(
                    &mut send,
                    &Solicitud::Archivo {
                        origen: self.origen.clone(),
                        revision: manifiesto.revision,
                        sha512: artefacto.sha512.clone(),
                        offset,
                    },
                    CONTROL_BYTES,
                )
                .await?;
                send.finish().map_err(|_| Error::Datos("conexión"))?;
                let respuesta: Respuesta = protocolo::leer(&mut recv, CONTROL_BYTES).await?;
                Ok::<_, Error>((recv, respuesta))
            };
            let (mut recv, respuesta) = tokio::select! {
                _ = cancel.cancelled() => return Err(Error::Cancelada),
                resultado = tokio::time::timeout(std::time::Duration::from_secs(30),inicio) => resultado.map_err(|_|Error::Datos("tiempo de descarga"))??,
            };
            if !matches!(respuesta,Respuesta::Archivo{restantes} if restantes==artefacto.bytes-offset)
            {
                return Err(Error::Datos("descarga rechazada"));
            }
            let mut out = tokio::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .await
                .map_err(|_| Error::Disco)?;
            let mut buf = vec![0; 64 * 1024];
            let mut restantes = artefacto.bytes - offset;
            while restantes > 0 {
                let n = tokio::select! {
                    _ = cancel.cancelled() => return Err(Error::Cancelada),
                    resultado = tokio::time::timeout(std::time::Duration::from_secs(120),AsyncReadExt::read(&mut recv,&mut buf)) => resultado.map_err(|_| Error::Datos("tiempo de descarga"))?.map_err(|_| Error::Datos("conexión"))?,
                };
                if n == 0 || n as u64 > restantes {
                    return Err(Error::Datos("tamaño descargado"));
                }
                out.write_all(&buf[..n]).await.map_err(|_| Error::Disco)?;
                out.flush().await.map_err(|_| Error::Disco)?;
                restantes -= n as u64;
            }
            let fin = tokio::select! {
                _ = cancel.cancelled() => return Err(Error::Cancelada),
                resultado = tokio::time::timeout(std::time::Duration::from_secs(30),AsyncReadExt::read(&mut recv,&mut buf[..1])) => resultado.map_err(|_|Error::Datos("tiempo de descarga"))?.map_err(|_|Error::Datos("conexión"))?,
            };
            if fin != 0 {
                return Err(Error::Datos("tamaño descargado"));
            }
            out.sync_all().await.map_err(|_| Error::Disco)?;
            Ok(())
        };
        transferencia.await?;
        let mut archivo = tokio::fs::File::open(&path)
            .await
            .map_err(|_| Error::Disco)?;
        let mut sha = Sha512::new();
        let mut buf = vec![0; 64 * 1024];
        loop {
            comprobar_cancelacion(cancel)?;
            let n = archivo.read(&mut buf).await.map_err(|_| Error::Disco)?;
            if n == 0 {
                break;
            }
            sha.update(&buf[..n]);
        }
        if format!("{:x}", sha.finalize()) != artefacto.sha512 {
            // El parcial corrupto no se reutiliza indefinidamente en el siguiente intento.
            drop(archivo);
            tokio::fs::remove_file(&path)
                .await
                .map_err(|_| Error::Disco)?;
            return Err(Error::Datos("integridad descargada"));
        }
        Ok(path)
    }
    pub async fn cerrar(&self) {
        self.conn.close(0u32.into(), b"fin");
        if self.endpoint_propio {
            self.endpoint.close().await;
        }
    }
}
