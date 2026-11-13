// SPDX-License-Identifier: GPL-3.0-only
//! Túnel del servidor alojado sobre el endpoint iroh de Compartir.
//!
//! El anfitrión acepta el ALPN `icaro-servidor/1` con las mismas reglas que la
//! publicación (autorizados por el servicio y no revocados) y reenvía cada
//! flujo al puerto local de su servidor de Minecraft. El invitado abre un
//! puerto en `127.0.0.1` al que se conecta su juego: iroh elige conexión
//! directa o relé, así que no hace falta abrir puertos ni estar en la misma red.
use crate::compartir::{Error, Origen};
use iroh::{
    endpoint::{Connection, RecvStream, SendStream},
    Endpoint, EndpointAddr,
};
use std::{
    collections::BTreeMap,
    future::Future,
    pin::Pin,
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

fn anotar(gravedad: crate::diagnostico::Gravedad, texto: &str) {
    crate::diagnostico::anotar(gravedad, "servidor", texto);
}
use crate::diagnostico::Gravedad::{Aviso, Info};

/// Anfitrión: atiende una conexión ya autorizada hasta que se cierre.
pub(super) async fn servir(conn: Connection, publicacion: String) {
    anotar(Info, "un invitado se conectó al túnel del servidor");
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
                        anotar(Info, &format!("juego del invitado unido a 127.0.0.1:{p}"));
                        let mut flujo = tokio::io::join(recv, send);
                        let fin = match tokio::io::copy_bidirectional(&mut flujo, &mut tcp).await {
                            Ok((a, b)) => format!("juego del invitado desconectado ({a} B al servidor, {b} B al invitado)"),
                            Err(e) => format!("juego del invitado desconectado: {e}"),
                        };
                        anotar(Info, &fin);
                    }
                    Err(e) => {
                        anotar(Aviso, &format!("no se pudo abrir 127.0.0.1:{p} para el invitado: {e}"));
                        let _ = send.write_all(&[0]).await;
                        let _ = send.finish();
                    }
                },
                (CONECTAR, None) => {
                    anotar(Aviso, "un invitado intentó entrar con el servidor cerrado");
                    let _ = send.write_all(&[0]).await;
                    let _ = send.finish();
                }
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
    /// Sin forma de reconectar: si la conexión se pierde, el túnel termina.
    pub async fn tunel(self) -> Result<Tunel, Error> {
        Tunel::abrir(self, None).await
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

/// Vuelve a pedir el encuentro al servicio y a conectar con el anfitrión.
pub type Reconectar =
    Arc<dyn Fn() -> Pin<Box<dyn Future<Output = Result<Conexion, Error>> + Send>> + Send + Sync>;

/// Conexión vigente del túnel. Entre abrir el túnel y que el juego termine de
/// cargar pasa tiempo, y otra conexión de esta misma identidad (por ejemplo,
/// la comprobación previa a Jugar) puede dejar inservible la primera: el túnel
/// se reconecta cuando el juego llega y la conexión ya no sirve.
struct Vigente {
    actual: tokio::sync::Mutex<Conexion>,
    reconectar: Option<Reconectar>,
}

impl Vigente {
    async fn conn(&self, forzar: bool) -> Result<Connection, Error> {
        let mut actual = self.actual.lock().await;
        if forzar || actual.conn.close_reason().is_some() {
            let Some(reconectar) = &self.reconectar else {
                return Err(Error::Datos("conexión"));
            };
            anotar(Info, "reconectando el túnel con el anfitrión");
            match reconectar().await {
                Ok(nueva) => *actual = nueva,
                Err(e) => {
                    anotar(Aviso, &format!("no se pudo reconectar el túnel: {e}"));
                    return Err(e);
                }
            }
        }
        Ok(actual.conn.clone())
    }
}

/// Pide un flujo hacia el servidor. `Ok(None)`: el anfitrión lo tiene cerrado.
async fn pedir_flujo(conn: &Connection) -> Result<Option<(SendStream, RecvStream)>, Error> {
    let (mut send, mut recv) = conn.open_bi().await.map_err(|_| Error::Datos("conexión"))?;
    send.write_all(&[CONECTAR])
        .await
        .map_err(|_| Error::Datos("conexión"))?;
    let mut r = [0u8; 1];
    tokio::time::timeout(std::time::Duration::from_secs(15), recv.read_exact(&mut r))
        .await
        .map_err(|_| Error::Datos("tiempo de conexión"))?
        .map_err(|_| Error::Datos("conexión"))?;
    Ok((r[0] == 1).then_some((send, recv)))
}

async fn reenviar(vigente: Arc<Vigente>, mut tcp: TcpStream) {
    let _ = tcp.set_nodelay(true);
    anotar(Info, "el juego se conectó al túnel");
    let mut flujo = match vigente.conn(false).await {
        Ok(conn) => pedir_flujo(&conn).await,
        Err(e) => Err(e),
    };
    if flujo.is_err() && vigente.reconectar.is_some() {
        flujo = match vigente.conn(true).await {
            Ok(conn) => pedir_flujo(&conn).await,
            Err(e) => Err(e),
        };
    }
    match flujo {
        Ok(Some((send, recv))) => {
            let mut puente = tokio::io::join(recv, send);
            let fin = match tokio::io::copy_bidirectional(&mut puente, &mut tcp).await {
                Ok((a, b)) => {
                    format!("el juego salió del servidor ({a} B enviados, {b} B recibidos)")
                }
                Err(e) => format!("el juego salió del servidor: {e}"),
            };
            anotar(Info, &fin);
        }
        Ok(None) => anotar(Aviso, "el anfitrión tiene el servidor cerrado"),
        Err(e) => anotar(Aviso, &format!("el túnel no pudo llegar al anfitrión: {e}")),
    }
}

/// Puerto local del invitado hacia el servidor del anfitrión.
pub struct Tunel {
    puerto: u16,
    cancel: CancellationToken,
    vigente: Arc<Vigente>,
}
impl std::fmt::Debug for Tunel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tunel")
            .field("puerto", &self.puerto)
            .finish()
    }
}
impl Tunel {
    /// Abre el puerto local; con `reconectar`, se recupera de una conexión
    /// perdida en vez de terminar.
    pub async fn abrir(inicial: Conexion, reconectar: Option<Reconectar>) -> Result<Self, Error> {
        let escucha = TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|_| Error::Datos("puerto local"))?;
        let puerto = escucha
            .local_addr()
            .map_err(|_| Error::Datos("puerto local"))?
            .port();
        let vigente = Arc::new(Vigente {
            actual: tokio::sync::Mutex::new(inicial),
            reconectar,
        });
        let cancel = CancellationToken::new();
        let (fin, v) = (cancel.clone(), vigente.clone());
        anotar(Info, &format!("túnel abierto en 127.0.0.1:{puerto}"));
        tokio::spawn(async move {
            let mut tareas = tokio::task::JoinSet::new();
            loop {
                tokio::select! {
                    _ = fin.cancelled() => break,
                    Some(_) = tareas.join_next(), if !tareas.is_empty() => {},
                    aceptada = escucha.accept() => {
                        let Ok((tcp, _)) = aceptada else { continue };
                        tareas.spawn(reenviar(v.clone(), tcp));
                    }
                }
            }
            tareas.abort_all();
            anotar(Info, "túnel cerrado");
        });
        Ok(Tunel {
            puerto,
            cancel,
            vigente,
        })
    }
    pub fn puerto(&self) -> u16 {
        self.puerto
    }
    pub fn direccion(&self) -> String {
        format!("127.0.0.1:{}", self.puerto)
    }
    /// Falso cuando se soltó o, sin forma de reconectar, se perdió la conexión.
    pub fn activo(&self) -> bool {
        !self.cancel.is_cancelled()
            && (self.vigente.reconectar.is_some()
                || self
                    .vigente
                    .actual
                    .try_lock()
                    .map_or(true, |a| a.conn.close_reason().is_none()))
    }
}
impl Drop for Tunel {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
