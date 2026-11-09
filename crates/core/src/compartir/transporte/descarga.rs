// SPDX-License-Identifier: GPL-3.0-only
use super::Cliente;
use crate::{
    compartir::{
        comprobar_cancelacion,
        protocolo::{self, Respuesta, Solicitud, CONTROL_BYTES},
        Artefacto, Error, Manifiesto, Procedencia,
    },
    descarga_archivo::Escritor,
};
use std::path::{Path, PathBuf};
use tokio::io::AsyncReadExt;
use tokio_util::sync::CancellationToken;

fn cupo_anfitrion(clave: &str) -> Result<std::sync::Arc<tokio::sync::Semaphore>, Error> {
    use std::sync::{Arc, Mutex, OnceLock, Weak};
    static CUPOS: OnceLock<Mutex<std::collections::HashMap<String, Weak<tokio::sync::Semaphore>>>> =
        OnceLock::new();
    let mut cupos = CUPOS
        .get_or_init(Mutex::default)
        .lock()
        .map_err(|_| Error::Disco)?;
    if cupos.len() >= 1024 {
        cupos.retain(|_, cupo| cupo.strong_count() > 0);
    }
    if let Some(cupo) = cupos.get(clave).and_then(Weak::upgrade) {
        return Ok(cupo);
    }
    let cupo = Arc::new(tokio::sync::Semaphore::new(2));
    cupos.insert(clave.to_owned(), Arc::downgrade(&cupo));
    Ok(cupo)
}

#[cfg(test)]
mod pruebas_cupos {
    use super::*;

    #[test]
    fn operaciones_del_mismo_anfitrion_comparten_dos_cupos() {
        let uno = cupo_anfitrion("prueba-cupos-a").unwrap();
        let otro = cupo_anfitrion("prueba-cupos-a").unwrap();
        assert!(std::sync::Arc::ptr_eq(&uno, &otro));
        let primero = uno.clone().try_acquire_owned().unwrap();
        let segundo = otro.clone().try_acquire_owned().unwrap();
        assert!(uno.clone().try_acquire_owned().is_err());
        let distinto = cupo_anfitrion("prueba-cupos-b").unwrap();
        assert!(distinto.try_acquire_owned().is_ok());
        drop(primero);
        assert!(otro.try_acquire_owned().is_ok());
        drop(segundo);
    }
}

/// Revisión validada e indexada una vez para un conjunto de descargas.
pub struct RevisionDescarga<'a> {
    cliente: &'a Cliente,
    manifiesto: &'a Manifiesto,
    archivos: std::collections::BTreeMap<&'a str, &'a Artefacto>,
}

fn error(e: crate::descarga_archivo::Error) -> Error {
    match e {
        crate::descarga_archivo::Error::Disco(e) => crate::compartir::diagnostico::disco(e),
        crate::descarga_archivo::Error::Integridad => Error::Datos("integridad descargada"),
        crate::descarga_archivo::Error::Cancelada => Error::Cancelada,
    }
}
impl Cliente {
    pub fn revision_para_descargar<'a>(
        &'a self,
        manifiesto: &'a Manifiesto,
    ) -> Result<RevisionDescarga<'a>, Error> {
        manifiesto.validar(&self.limites)?;
        if manifiesto.origen != self.origen {
            return Err(Error::Datos("publicación distinta"));
        }
        Ok(RevisionDescarga {
            cliente: self,
            manifiesto,
            archivos: manifiesto
                .archivos
                .iter()
                .map(|a| (a.ruta.as_str(), a))
                .collect(),
        })
    }
    pub(super) async fn descargar_archivo(
        &self,
        manifiesto: &Manifiesto,
        artefacto: &Artefacto,
        staging: &Path,
        cancel: &CancellationToken,
    ) -> Result<PathBuf, Error> {
        self.descargar_con_metricas(manifiesto, artefacto, staging, cancel)
            .await
            .map(|(ruta, _)| ruta)
    }

    /// Bytes de payload recibidos y verificados en esta transferencia exitosa;
    /// excluye el prefijo reanudado, intentos fallidos y sobrecarga de protocolo.
    pub async fn descargar_con_metricas(
        &self,
        manifiesto: &Manifiesto,
        artefacto: &Artefacto,
        staging: &Path,
        cancel: &CancellationToken,
    ) -> Result<(PathBuf, u64), Error> {
        self.revision_para_descargar(manifiesto)?
            .descargar_con_metricas(artefacto, staging, cancel)
            .await
    }
}
impl RevisionDescarga<'_> {
    pub async fn descargar_con_metricas(
        &self,
        artefacto: &Artefacto,
        staging: &Path,
        cancel: &CancellationToken,
    ) -> Result<(PathBuf, u64), Error> {
        let manifiesto = self.manifiesto;
        let cliente = self.cliente;
        comprobar_cancelacion(cancel)?;
        if self.archivos.get(artefacto.ruta.as_str()).copied() != Some(artefacto)
            || !matches!(artefacto.procedencia, Procedencia::Propio)
        {
            return Err(Error::Datos("archivo no publicado"));
        }
        let cupo = cupo_anfitrion(&cliente.origen.anfitrion)?;
        let _permiso = tokio::select! {
            _ = cancel.cancelled() => return Err(Error::Cancelada),
            r = cupo.acquire_owned() => r.map_err(|_| Error::Cancelada)?,
        };
        let path = staging.join(format!("{}.part", artefacto.sha512));
        let mut out = Escritor::iniciar(
            path.clone(),
            artefacto.bytes,
            Some(artefacto.sha512.clone()),
            true,
            cancel.clone(),
        )
        .await
        .map_err(error)?;
        let offset = out.offset();
        let transferencia = async {
            let inicio = async {
                let (mut send, mut recv) = cliente.conn.open_bi().await.map_err(|_| Error::Datos("conexión"))?;
                protocolo::escribir(&mut send, &Solicitud::Archivo { origen: cliente.origen.clone(), revision: manifiesto.revision, sha512: artefacto.sha512.clone(), offset }, CONTROL_BYTES).await?;
                send.finish().map_err(|_| Error::Datos("conexión"))?;
                let respuesta: Respuesta = protocolo::leer(&mut recv, CONTROL_BYTES).await?;
                Ok::<_, Error>((recv, respuesta))
            };
            let (mut recv, respuesta) = tokio::select! {
                _ = cancel.cancelled() => return Err(Error::Cancelada),
                r = tokio::time::timeout(std::time::Duration::from_secs(30), inicio) => r.map_err(|_| Error::Datos("tiempo de descarga"))??,
            };
            if !matches!(respuesta, Respuesta::Archivo { restantes } if restantes == artefacto.bytes-offset) { return Err(Error::Datos("descarga rechazada")); }
            let mut buf = vec![0; 256 * 1024];
            let mut restantes = artefacto.bytes-offset;
            while restantes > 0 {
                let n = tokio::select! {
                    _ = cancel.cancelled() => return Err(Error::Cancelada),
                    r = tokio::time::timeout(std::time::Duration::from_secs(120), AsyncReadExt::read(&mut recv, &mut buf)) => r.map_err(|_| Error::Datos("tiempo de descarga"))?.map_err(|_| Error::Datos("conexión"))?,
                };
                if n == 0 || n as u64 > restantes { return Err(Error::Datos("tamaño descargado")); }
                out.escribir(&buf[..n]).await.map_err(error)?;
                restantes -= n as u64;
            }
            let fin = tokio::select! {
                _ = cancel.cancelled() => return Err(Error::Cancelada),
                r = tokio::time::timeout(std::time::Duration::from_secs(30), AsyncReadExt::read(&mut recv, &mut buf[..1])) => r.map_err(|_| Error::Datos("tiempo de descarga"))?.map_err(|_| Error::Datos("conexión"))?,
            };
            if fin != 0 { return Err(Error::Datos("tamaño descargado")); }
            Ok(())
        }.await;
        if let Err(e) = transferencia {
            out.cancelar().await;
            return Err(e);
        }
        out.finalizar().await.map_err(error)?;
        Ok((path, artefacto.bytes - offset))
    }
}
