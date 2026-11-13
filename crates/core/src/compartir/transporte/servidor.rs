// SPDX-License-Identifier: GPL-3.0-only
//! Túnel del servidor alojado sobre el endpoint iroh de Compartir.
//!
//! El anfitrión acepta el ALPN `icaro-servidor/1` con las mismas reglas que la
//! publicación (autorizados por el servicio y no revocados) y reenvía cada
//! flujo al puerto local de su servidor de Minecraft. El invitado abre un
//! puerto en `127.0.0.1` al que se conecta su juego: iroh elige conexión
//! directa o relé, así que no hace falta abrir puertos ni estar en la misma red.
use crate::compartir::{Error, Origen};
use iroh::{endpoint::Connection, Endpoint, EndpointAddr};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use tokio::{
    net::{TcpListener, TcpStream},
    sync::Semaphore,
};
use tokio_util::sync::CancellationToken;

pub const ALPN_SERVIDOR: &[u8] = b"icaro-servidor/1";
const CONSULTA: u8 = 0;
const CONECTAR: u8 = 1;
/// Conexiones de juego simultáneas por invitado (incluye pings de la lista).
const FLUJOS_POR_INVITADO: usize = 8;

/// Puerto local del servidor abierto de cada publicación. Vive fuera del nodo
/// para sobrevivir a reconexiones de la publicación.
static PUERTOS: Mutex<BTreeMap<String, u16>> = Mutex::new(BTreeMap::new());

pub fn abrir(publicacion: &str, puerto: u16) {
    if let Ok(mut p) = PUERTOS.lock() {
        p.insert(publicacion.to_owned(), puerto);
    }
}
pub fn cerrar(publicacion: &str) {
    if let Ok(mut p) = PUERTOS.lock() {
        p.remove(publicacion);
    }
}
pub fn puerto(publicacion: &str) -> Option<u16> {
    PUERTOS.lock().ok()?.get(publicacion).copied()
}

/// Anfitrión: atiende una conexión ya autorizada hasta que se cierre.
pub(super) async fn servir(conn: Connection, publicacion: String) {
    let cupo = Arc::new(Semaphore::new(FLUJOS_POR_INVITADO));
    let mut tareas = tokio::task::JoinSet::new();
    while let Ok((mut send, mut recv)) = conn.accept_bi().await {
        let Ok(permiso) = cupo.clone().try_acquire_owned() else {
            let _ = send.write_all(&[0]).await;
            let _ = send.finish();
            continue;
        };
        let publicacion = publicacion.clone();
        tareas.spawn(async move {
            let _permiso = permiso;
            let mut op = [0u8; 1];
            if tokio::time::timeout(std::time::Duration::from_secs(15), recv.read_exact(&mut op))
                .await
                .map_or(true, |r| r.is_err())
            {
                return;
            }
            let destino = puerto(&publicacion);
            match (op[0], destino) {
                (CONSULTA, p) => {
                    let _ = send.write_all(&[u8::from(p.is_some())]).await;
                    let _ = send.finish();
                }
                (CONECTAR, Some(p)) => match TcpStream::connect(("127.0.0.1", p)).await {
                    Ok(mut tcp) => {
                        let _ = tcp.set_nodelay(true);
                        if send.write_all(&[1]).await.is_err() {
                            return;
                        }
                        let mut flujo = tokio::io::join(recv, send);
                        let _ = tokio::io::copy_bidirectional(&mut flujo, &mut tcp).await;
                    }
                    Err(_) => {
                        let _ = send.write_all(&[0]).await;
                        let _ = send.finish();
                    }
                },
                _ => {
                    let _ = send.write_all(&[0]).await;
                    let _ = send.finish();
                }
            }
        });
        while tareas.try_join_next().is_some() {}
    }
    tareas.abort_all();
}

/// Invitado: conexión con el anfitrión por el ALPN del servidor.
pub struct Conexion {
    conn: Connection,
    /// Endpoint propio cuando este equipo no comparte nada; si comparte, se
    /// reutiliza el del nodo para no abrir otra identidad simultánea.
    endpoint: Option<Endpoint>,
}
impl std::fmt::Debug for Conexion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Conexion").finish_non_exhaustive()
    }
}

impl Conexion {
    pub(super) async fn nueva(
        endpoint: Endpoint,
        propio: bool,
        direccion: EndpointAddr,
        origen: &Origen,
    ) -> Result<Self, Error> {
        origen.validar()?;
        if direccion.id.to_string() != origen.anfitrion {
            return Err(Error::Datos("identidad remota"));
        }
        let conn = tokio::time::timeout(
            std::time::Duration::from_secs(30),
            endpoint.connect(direccion, ALPN_SERVIDOR),
        )
        .await
        .map_err(|_| Error::Datos("tiempo de conexión"))?
        .map_err(|_| Error::Datos("conexión"))?;
        if conn.remote_id().to_string() != origen.anfitrion {
            return Err(Error::Datos("identidad remota"));
        }
        Ok(Self {
            conn,
            endpoint: propio.then_some(endpoint),
        })
    }

    /// ¿Tiene el anfitrión su servidor abierto ahora?
    pub async fn abierto(&self) -> Result<bool, Error> {
        let (mut send, mut recv) = self
            .conn
            .open_bi()
            .await
            .map_err(|_| Error::Datos("conexión"))?;
        send.write_all(&[CONSULTA])
            .await
            .map_err(|_| Error::Datos("conexión"))?;
        let _ = send.finish();
        let mut r = [0u8; 1];
        tokio::time::timeout(std::time::Duration::from_secs(15), recv.read_exact(&mut r))
            .await
            .map_err(|_| Error::Datos("tiempo de consulta"))?
            .map_err(|_| Error::Datos("consulta rechazada"))?;
        Ok(r[0] == 1)
    }

    /// Abre `127.0.0.1:<puerto>` para el juego; vive hasta soltar el túnel.
    pub async fn tunel(self) -> Result<Tunel, Error> {
        let escucha = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|_| Error::Datos("puerto local"))?;
        let puerto = escucha
            .local_addr()
            .map_err(|_| Error::Datos("puerto local"))?
            .port();
        let cancel = CancellationToken::new();
        let conn = self.conn.clone();
        let fin = cancel.clone();
        tokio::spawn(async move {
            let mut tareas = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    _ = fin.cancelled() => break,
                    _ = conn.closed() => break,
                    Some(_) = tareas.join_next(), if !tareas.is_empty() => {},
                    aceptada = escucha.accept() => {
                        let Ok((tcp, _)) = aceptada else { continue };
                        let conn = conn.clone();
                        tareas.spawn(async move {
                            let _ = reenviar(conn, tcp).await;
                        });
                    }
                }
            }
            tareas.abort_all();
        });
        Ok(Tunel {
            puerto,
            cancel,
            conn: self.conn.clone(),
            _conexion: self,
        })
    }
}

impl Drop for Conexion {
    fn drop(&mut self) {
        self.conn.close(0u32.into(), b"fin");
        if let Some(e) = self.endpoint.take() {
            tokio::spawn(async move { e.close().await });
        }
    }
}

async fn reenviar(conn: Connection, mut tcp: TcpStream) -> Result<(), Error> {
    let _ = tcp.set_nodelay(true);
    let (mut send, mut recv) = conn.open_bi().await.map_err(|_| Error::Datos("conexión"))?;
    send.write_all(&[CONECTAR])
        .await
        .map_err(|_| Error::Datos("conexión"))?;
    let mut r = [0u8; 1];
    tokio::time::timeout(std::time::Duration::from_secs(15), recv.read_exact(&mut r))
        .await
        .map_err(|_| Error::Datos("tiempo de conexión"))?
        .map_err(|_| Error::Datos("servidor cerrado"))?;
    if r[0] != 1 {
        return Err(Error::Datos("servidor cerrado"));
    }
    let mut flujo = tokio::io::join(recv, send);
    tokio::io::copy_bidirectional(&mut flujo, &mut tcp)
        .await
        .map(|_| ())
        .map_err(|_| Error::Datos("conexión"))
}

/// Puerto local del invitado hacia el servidor del anfitrión.
pub struct Tunel {
    puerto: u16,
    cancel: CancellationToken,
    conn: Connection,
    _conexion: Conexion,
}
impl std::fmt::Debug for Tunel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tunel")
            .field("puerto", &self.puerto)
            .finish()
    }
}
impl Tunel {
    pub fn puerto(&self) -> u16 {
        self.puerto
    }
    pub fn direccion(&self) -> String {
        format!("127.0.0.1:{}", self.puerto)
    }
    /// Falso cuando el anfitrión cerró o se perdió la conexión.
    pub fn activo(&self) -> bool {
        !self.cancel.is_cancelled() && self.conn.close_reason().is_none()
    }
}
impl Drop for Tunel {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
