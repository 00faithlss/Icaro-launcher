// SPDX-License-Identifier: GPL-3.0-only
//! Escritor acotado: el hilo conserva archivo y bloqueo hasta cerrar toda E/S.
use sha2::{Digest, Sha512};
use std::{
    fs,
    io::{self, BufWriter, Read, Seek, Write},
    path::PathBuf,
};
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

const BLOQUE: usize = 256 * 1024;
#[derive(Debug)]
pub(crate) enum Error {
    Disco(io::Error),
    Integridad,
    Cancelada,
}
impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Self::Disco(e)
    }
}
enum Comando {
    Bloque(Vec<u8>),
    Fin,
}

pub(crate) struct Escritor {
    envio: Option<mpsc::Sender<Comando>>,
    resultado: Option<oneshot::Receiver<Result<String, Error>>>,
    cancel: CancellationToken,
    offset: u64,
}
impl Drop for Escritor {
    fn drop(&mut self) {
        self.cancel.cancel();
        self.envio.take();
    }
}
impl Escritor {
    pub(crate) async fn iniciar(
        path: PathBuf,
        bytes: u64,
        hash: Option<String>,
        reanudar: bool,
        padre: CancellationToken,
    ) -> Result<Self, Error> {
        let cancel = padre.child_token();
        static CUPOS: std::sync::OnceLock<std::sync::Arc<tokio::sync::Semaphore>> =
            std::sync::OnceLock::new();
        let cupos = CUPOS
            .get_or_init(|| std::sync::Arc::new(tokio::sync::Semaphore::new(6)))
            .clone();
        let permiso = tokio::select! {
            _ = cancel.cancelled() => return Err(Error::Cancelada),
            r = cupos.acquire_owned() => r.map_err(|_| Error::Cancelada)?,
        };
        let (envio, recepcion) = mpsc::channel(4);
        let (inicio, listo) = oneshot::channel();
        let (fin, resultado) = oneshot::channel();
        let mut out = Self {
            envio: Some(envio),
            resultado: Some(resultado),
            cancel: cancel.clone(),
            offset: 0,
        };
        tokio::task::spawn_blocking(move || {
            let _permiso = permiso;
            let r = ejecutar(path, bytes, hash, reanudar, cancel, recepcion, inicio);
            let _ = fin.send(r);
        });
        let offset = listo.await.map_err(|_| Error::Cancelada)??;
        out.offset = offset;
        Ok(out)
    }
    pub(crate) fn offset(&self) -> u64 {
        self.offset
    }
    pub(crate) async fn escribir(&mut self, bytes: &[u8]) -> Result<(), Error> {
        for bloque in bytes.chunks(BLOQUE) {
            tokio::select! {
                _ = self.cancel.cancelled() => return Err(Error::Cancelada),
                r = self.envio.as_ref().ok_or(Error::Cancelada)?.send(Comando::Bloque(bloque.to_vec())) => r.map_err(|_| Error::Integridad)?,
            }
        }
        Ok(())
    }
    pub(crate) async fn finalizar(mut self) -> Result<String, Error> {
        let envio = self.envio.take().ok_or(Error::Cancelada)?;
        if envio.send(Comando::Fin).await.is_err() {
            drop(envio);
            return self
                .resultado
                .take()
                .ok_or(Error::Cancelada)?
                .await
                .map_err(|_| Error::Cancelada)?;
        }
        drop(envio);
        self.resultado
            .take()
            .ok_or(Error::Cancelada)?
            .await
            .map_err(|_| Error::Cancelada)?
    }
    pub(crate) async fn cancelar(mut self) {
        self.cancel.cancel();
        self.envio.take();
        if let Some(r) = self.resultado.take() {
            let _ = r.await;
        }
    }
}
fn regular_si_existe(path: &std::path::Path) -> Result<(), Error> {
    match fs::symlink_metadata(path) {
        Ok(_) => {
            if !crate::almacenamiento::regular(path)
                .map_err(|_| Error::Integridad)?
                .is_file()
            {
                return Err(Error::Integridad);
            }
        }
        Err(e) if e.kind() == io::ErrorKind::NotFound => {}
        Err(e) => return Err(e.into()),
    }
    Ok(())
}
fn ejecutar(
    path: PathBuf,
    bytes: u64,
    hash: Option<String>,
    reanudar: bool,
    cancel: CancellationToken,
    mut canal: mpsc::Receiver<Comando>,
    inicio: oneshot::Sender<Result<u64, Error>>,
) -> Result<String, Error> {
    let abrir = || -> Result<_, Error> {
        if cancel.is_cancelled() {
            return Err(Error::Cancelada);
        }
        if let Some(p) = path.parent() {
            fs::create_dir_all(p)?;
            for a in p.ancestors() {
                if !crate::almacenamiento::regular(a)
                    .map_err(|_| Error::Integridad)?
                    .is_dir()
                {
                    return Err(Error::Integridad);
                }
            }
        }
        let lock = path.with_extension("lock");
        regular_si_existe(&lock)?;
        let bloqueo = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(lock)?;
        // Espera cooperativa: otro intento puede seguir drenando su escritor.
        loop {
            match bloqueo.try_lock() {
                Ok(()) => break,
                Err(std::fs::TryLockError::WouldBlock) => {
                    if cancel.is_cancelled() {
                        return Err(Error::Cancelada);
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(std::fs::TryLockError::Error(e)) => return Err(e.into()),
            }
        }
        if cancel.is_cancelled() {
            return Err(Error::Cancelada);
        }
        regular_si_existe(&path)?;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .truncate(!reanudar)
            .read(true)
            .write(true)
            .open(&path)?;
        let offset = file.metadata()?.len();
        if offset > bytes {
            return Err(Error::Integridad);
        }
        let mut sha = Sha512::new();
        let mut buf = vec![0; BLOQUE];
        loop {
            if cancel.is_cancelled() {
                return Err(Error::Cancelada);
            }
            let n = file.read(&mut buf)?;
            if n == 0 {
                break;
            }
            sha.update(&buf[..n]);
        }
        file.seek(std::io::SeekFrom::Start(offset))?;
        Ok((bloqueo, file, offset, sha))
    };
    let (_bloqueo, file, mut total, mut sha) = match abrir() {
        Ok(r) => r,
        Err(e) => {
            let _ = inicio.send(Err(e));
            return Err(Error::Cancelada);
        }
    };
    if inicio.send(Ok(total)).is_err() {
        return Err(Error::Cancelada);
    }
    let mut out = BufWriter::with_capacity(BLOQUE, file);
    let r = (|| {
        while let Some(comando) = canal.blocking_recv() {
            if cancel.is_cancelled() {
                return Err(Error::Cancelada);
            }
            match comando {
                Comando::Bloque(bloque) => {
                    total = total
                        .checked_add(bloque.len() as u64)
                        .ok_or(Error::Integridad)?;
                    if total > bytes {
                        return Err(Error::Integridad);
                    }
                    out.write_all(&bloque)?;
                    sha.update(&bloque);
                }
                Comando::Fin => {
                    let calculado = format!("{:x}", sha.finalize());
                    if total != bytes
                        || hash
                            .as_ref()
                            .is_some_and(|h| !h.eq_ignore_ascii_case(&calculado))
                    {
                        return Err(Error::Integridad);
                    }
                    return Ok(calculado);
                }
            }
        }
        Err(Error::Cancelada)
    })();
    // Incluso al cancelar: cerrar el archivo antes de liberar el bloqueo.
    let flush = out.flush();
    if r.is_ok() {
        flush?;
        out.get_ref().sync_all()?;
    }
    drop(out);
    if matches!(r, Err(Error::Integridad)) {
        // Se elimina bajo el mismo bloqueo: otro productor no puede sustituirlo.
        fs::remove_file(&path)?;
    }
    if cancel.is_cancelled() && r.is_ok() {
        return Err(Error::Cancelada);
    }
    r
}

#[cfg(test)]
mod tests;
