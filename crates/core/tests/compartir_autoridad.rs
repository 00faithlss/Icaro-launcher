// SPDX-License-Identifier: GPL-3.0-only
use icaro_core::compartir::{
    accesos::{Accesos, Autoridad},
    identidad::{AlmacenClave, Identidad},
    invitaciones::{AutoridadServicio, ClienteInvitaciones},
    Error,
};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use zeroize::Zeroizing;

struct Memoria(Mutex<Option<Zeroizing<Vec<u8>>>>);
impl AlmacenClave for Memoria {
    fn leer(&self) -> Result<Option<Zeroizing<Vec<u8>>>, Error> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn guardar(&self, bytes: &[u8]) -> Result<(), Error> {
        *self.0.lock().unwrap() = Some(Zeroizing::new(bytes.to_vec()));
        Ok(())
    }
}

#[tokio::test]
async fn autoridad_consulta_servicio_sin_resucitar_revocados_ni_bloquear_futuras_aceptaciones() {
    let identidad = Identidad::desde_almacen(&Memoria(Mutex::new(None))).unwrap();
    let carpeta =
        std::env::temp_dir().join(format!("icaro-autoridad-{}", identidad.clave_publica()));
    std::fs::create_dir_all(&carpeta).unwrap();
    let acl = Arc::new(Accesos::abrir(carpeta.join("accesos.json")).unwrap());
    let guest = "ac".repeat(32);
    let publicacion = "ab".repeat(16);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let clave = guest.clone();
    let tarea = tokio::spawn(async move {
        for permitida in [false, true, true, false, true] {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut bytes = Vec::new();
            let mut buf = [0; 2048];
            loop {
                let n = stream.read(&mut buf).await.unwrap();
                assert!(n > 0);
                bytes.extend_from_slice(&buf[..n]);
                if let Some(fin) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                    let cabecera = std::str::from_utf8(&bytes[..fin]).unwrap();
                    let longitud: usize = cabecera
                        .lines()
                        .find_map(|l| {
                            l.to_ascii_lowercase()
                                .strip_prefix("content-length:")
                                .map(|s| s.trim().parse().unwrap())
                        })
                        .unwrap();
                    if bytes.len() >= fin + 4 + longitud {
                        break;
                    }
                }
            }
            let cuerpo = serde_json::json!({"publication":publicacion,"active":true,"guests":if permitida {vec![clave.clone()]} else {vec![]},"next":null}).to_string();
            stream
                .write_all(
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        cuerpo.len(),
                        cuerpo
                    )
                    .as_bytes(),
                )
                .await
                .unwrap();
        }
    });
    let autoridad = AutoridadServicio::nueva(
        ClienteInvitaciones::nuevo(&url, identidad).unwrap(),
        "ab".repeat(16),
        acl.clone(),
    )
    .unwrap();
    assert!(!autoridad.autorizado(guest.clone()).await.unwrap());
    assert!(autoridad.autorizado(guest.clone()).await.unwrap());
    assert!(acl.permitir(&guest));
    acl.revocar(&guest).unwrap();
    assert!(!autoridad.autorizado(guest.clone()).await.unwrap());
    acl.reiniciar().unwrap();
    assert!(!autoridad.autorizado(guest.clone()).await.unwrap());
    assert!(autoridad.autorizado(guest.clone()).await.unwrap());
    tarea.await.unwrap();
    // Un fallo de red nunca usa el permiso local como sustituto del servicio.
    assert!(autoridad.autorizado(guest).await.is_err());
    std::fs::remove_file(carpeta.join("accesos.json")).unwrap();
    std::fs::remove_dir(carpeta).unwrap();
}
