// SPDX-License-Identifier: GPL-3.0-only
//! Proceso del servidor dedicado: consola, comandos y cierre ordenado.
//!
//! Detener escribe `stop` para que Minecraft guarde el mundo y solo fuerza el
//! cierre si no termina en 60 s. Soltar el `Servidor` equivale a detenerlo.
use std::{path::Path, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, AsyncRead, AsyncWriteExt, BufReader},
    process::Command,
    sync::mpsc,
};

const LINEA_MAX: usize = 8 * 1024;
const ESPERA_STOP: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Evento {
    Linea(String),
    /// Minecraft terminó de cargar el mundo y acepta jugadores.
    Listo,
    /// El proceso terminó; código de salida si lo hubo.
    Fin(Option<i32>),
}

enum Orden {
    Comando(String),
    Detener,
}

#[derive(Debug, Clone)]
pub struct Servidor {
    ordenes: mpsc::UnboundedSender<Orden>,
}
impl std::fmt::Debug for Orden {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Orden")
    }
}

impl Servidor {
    /// Envía una línea a la consola. Los saltos de línea se descartan para que
    /// un texto no pueda encadenar varios comandos.
    pub fn comando(&self, texto: &str) -> bool {
        let limpio: String = texto.chars().filter(|c| !c.is_control()).collect();
        let limpio = limpio.trim().trim_start_matches('/').to_owned();
        !limpio.is_empty() && self.ordenes.send(Orden::Comando(limpio)).is_ok()
    }
    pub fn detener(&self) {
        let _ = self.ordenes.send(Orden::Detener);
    }
}

pub fn argumentos(jar: &str, memoria_mb: u32) -> Vec<String> {
    let max = memoria_mb.clamp(512, 64 * 1024);
    vec![
        format!("-Xms{}M", max.min(1024)),
        format!("-Xmx{max}M"),
        "-Dstdout.encoding=UTF-8".into(),
        "-Dstderr.encoding=UTF-8".into(),
        "-jar".into(),
        jar.into(),
        "nogui".into(),
    ]
}

pub fn es_listo(linea: &str) -> bool {
    linea.contains("]: Done (") && linea.contains("For help, type")
}

/// Inicia `java -jar <jar> nogui` dentro de `carpeta` sin ventana de consola.
pub fn iniciar(
    java: &Path,
    carpeta: &Path,
    jar: &str,
    memoria_mb: u32,
) -> Result<(Servidor, mpsc::Receiver<Evento>), super::Error> {
    let mut comando = Command::new(java);
    comando
        .args(argumentos(jar, memoria_mb))
        .current_dir(carpeta)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        // Último recurso si Ícaro termina sin pasar por Detener.
        .kill_on_drop(true);
    #[cfg(windows)]
    comando.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let mut hijo = comando.spawn().map_err(|_| super::Error::Java)?;
    let mut entrada = hijo.stdin.take().ok_or(super::Error::Java)?;
    let salida = hijo.stdout.take().ok_or(super::Error::Java)?;
    let errores = hijo.stderr.take().ok_or(super::Error::Java)?;
    let (tx_ordenes, mut rx_ordenes) = mpsc::unbounded_channel();
    let (tx, rx) = mpsc::channel(1024);
    let tx_out = tx.clone();
    let tx_err = tx.clone();
    tokio::spawn(leer(salida, tx_out));
    tokio::spawn(leer(errores, tx_err));
    tokio::spawn(async move {
        let mut limite: Option<tokio::time::Instant> = None;
        let mut ordenes_abiertas = true;
        loop {
            let espera = async {
                match limite {
                    Some(l) => tokio::time::sleep_until(l).await,
                    None => std::future::pending().await,
                }
            };
            tokio::select! {
                estado = hijo.wait() => {
                    let _ = tx.send(Evento::Fin(estado.ok().and_then(|s| s.code()))).await;
                    break;
                }
                orden = rx_ordenes.recv(), if ordenes_abiertas => match orden {
                    Some(Orden::Comando(c)) => {
                        let _ = entrada.write_all(format!("{c}\n").as_bytes()).await;
                        let _ = entrada.flush().await;
                    }
                    Some(Orden::Detener) | None => {
                        ordenes_abiertas = orden.is_some();
                        if limite.is_none() {
                            let _ = entrada.write_all(b"stop\n").await;
                            let _ = entrada.flush().await;
                            limite = Some(tokio::time::Instant::now() + ESPERA_STOP);
                        }
                    }
                },
                _ = espera => {
                    let _ = hijo.start_kill();
                    limite = Some(tokio::time::Instant::now() + Duration::from_secs(3600));
                }
            }
        }
    });
    Ok((
        Servidor {
            ordenes: tx_ordenes,
        },
        rx,
    ))
}

async fn leer(origen: impl AsyncRead + Unpin, tx: mpsc::Sender<Evento>) {
    let mut lector = BufReader::new(origen);
    let mut buf = Vec::new();
    loop {
        buf.clear();
        match lector.read_until(b'\n', &mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(_) => {
                buf.truncate(LINEA_MAX);
                let linea = String::from_utf8_lossy(&buf).trim_end().to_owned();
                if es_listo(&linea) {
                    let _ = tx.send(Evento::Listo).await;
                }
                if tx.send(Evento::Linea(linea)).await.is_err() {
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argumentos_con_memoria_acotada() {
        let a = argumentos("server.jar", 100);
        assert_eq!(a[0], "-Xms512M");
        assert_eq!(a[1], "-Xmx512M");
        assert_eq!(a[a.len() - 3..], ["-jar", "server.jar", "nogui"]);
        let b = argumentos("x.jar", 6144);
        assert!(b.contains(&"-Xms1024M".to_string()) && b.contains(&"-Xmx6144M".to_string()));
    }

    #[test]
    fn reconoce_el_fin_de_la_carga() {
        assert!(es_listo(
            "[12:00:10] [Server thread/INFO]: Done (8.123s)! For help, type \"help\""
        ));
        assert!(!es_listo(
            "[12:00:10] [Server thread/INFO]: <Pato> Done (yo)"
        ));
    }

    #[test]
    fn un_comando_no_encadena_otros() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let s = Servidor { ordenes: tx };
        assert!(s.comando("/say hola\nstop"));
        assert!(!s.comando("  \n "));
        match rx.try_recv().unwrap() {
            Orden::Comando(c) => assert_eq!(c, "say holastop"),
            Orden::Detener => panic!(),
        }
    }
}
