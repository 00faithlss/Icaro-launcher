// SPDX-License-Identifier: GPL-3.0-only
//! Túnel del servidor alojado: solo invitados autorizados, abierto/cerrado y
//! reenvío de bytes en ambos sentidos hasta el puerto local del anfitrión.
use icaro_core::compartir::{
    accesos::{Accesos, Autoridad},
    identidad::{AlmacenClave, Identidad},
    revisiones::{self, ContextoPublicacion},
    transporte::{self, servidor, Modo, NodoAnfitrion},
    Error, Limites, Manifiesto,
};
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, RwLock},
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use zeroize::Zeroizing;

struct Memoria(Mutex<Option<Zeroizing<Vec<u8>>>>);
impl AlmacenClave for Memoria {
    fn leer(&self) -> Result<Option<Zeroizing<Vec<u8>>>, Error> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn guardar(&self, b: &[u8]) -> Result<(), Error> {
        *self.0.lock().unwrap() = Some(Zeroizing::new(b.to_vec()));
        Ok(())
    }
}
fn identidad(semilla: Option<u8>) -> Identidad {
    Identidad::desde_almacen(&Memoria(Mutex::new(
        semilla.map(|s| Zeroizing::new(vec![s; 32])),
    )))
    .unwrap()
}
struct Permiso;
impl Autoridad for Permiso {
    fn autorizado(
        &self,
        _: String,
    ) -> Pin<Box<dyn Future<Output = Result<bool, Error>> + Send + '_>> {
        Box::pin(async move { Ok(true) })
    }
}

#[tokio::test]
async fn el_invitado_entra_al_servidor_por_el_tunel_y_un_extrano_no() {
    let host = identidad(None);
    let invitado = identidad(Some(9));
    let extrano = identidad(Some(11));
    let publicacion = "cd".repeat(16);
    let root = std::env::temp_dir().join(format!("icaro-tunel-{}", host.clave_publica()));
    std::fs::create_dir_all(root.join("contenido")).unwrap();
    let m: Manifiesto = serde_json::from_value(serde_json::json!({
        "esquema":"icaro-compartir/1","origen":{"anfitrion":host.clave_publica(),"publicacion":publicacion},
        "revision":1,"perfil":{"minecraft":"1.21.1","loader":"vanilla","loader_version":null},
        "archivos":[],"packs":[]
    }))
    .unwrap();
    std::fs::write(root.join("vigente.json"), serde_json::to_vec(&m).unwrap()).unwrap();
    let ctx = ContextoPublicacion {
        juego: root.join("juego"),
        almacen: root.clone(),
        origen: m.origen.clone(),
        perfil: m.perfil.clone(),
        limites: Limites::default(),
    };
    let revision = revisiones::vigente(&ctx).unwrap().unwrap();
    let acl = Arc::new(Accesos::abrir(root.join("accesos.json")).unwrap());
    acl.autorizar(&invitado.clave_publica()).unwrap();
    let nodo = NodoAnfitrion::iniciar(
        host.clone(),
        Arc::new(RwLock::new(revision)),
        acl,
        Arc::new(Permiso),
        Modo::Local,
        Limites::default(),
    )
    .await
    .unwrap();

    // «Minecraft» del anfitrión: un eco en loopback.
    let mc = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let puerto_mc = mc.local_addr().unwrap().port();
    tokio::spawn(async move {
        while let Ok((mut s, _)) = mc.accept().await {
            tokio::spawn(async move {
                let mut buf = [0u8; 1024];
                while let Ok(n) = s.read(&mut buf).await {
                    if n == 0 || s.write_all(&buf[..n]).await.is_err() {
                        break;
                    }
                }
            });
        }
    });

    let conexion =
        transporte::conectar_servidor(&invitado, nodo.direccion(), &m.origen, Modo::Local)
            .await
            .unwrap();
    assert!(!conexion.abierto().await.unwrap(), "sin servidor abierto");
    servidor::abrir(&publicacion, puerto_mc);
    assert!(conexion.abierto().await.unwrap());

    let tunel = conexion.tunel().await.unwrap();
    let mut juego = tokio::net::TcpStream::connect(tunel.direccion())
        .await
        .unwrap();
    juego.write_all(b"hola servidor").await.unwrap();
    let mut eco = [0u8; 13];
    tokio::time::timeout(
        std::time::Duration::from_secs(10),
        juego.read_exact(&mut eco),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(&eco, b"hola servidor");

    // Al cerrar el servidor, las conexiones nuevas se rechazan.
    servidor::cerrar(&publicacion);
    let mut otra = tokio::net::TcpStream::connect(tunel.direccion())
        .await
        .unwrap();
    let mut b = [0u8; 1];
    let leido = tokio::time::timeout(std::time::Duration::from_secs(10), otra.read(&mut b))
        .await
        .unwrap();
    assert!(
        matches!(leido, Ok(0) | Err(_)),
        "el túnel no debe reenviar con el servidor cerrado"
    );

    // Alguien sin autorización no llega a consultar.
    servidor::abrir(&publicacion, puerto_mc);
    let ajeno =
        transporte::conectar_servidor(&extrano, nodo.direccion(), &m.origen, Modo::Local).await;
    if let Ok(c) = ajeno {
        assert!(c.abierto().await.is_err());
    }
    servidor::cerrar(&publicacion);
    drop(tunel);
    nodo.parar().await;
    let _ = std::fs::remove_dir_all(root);
}
