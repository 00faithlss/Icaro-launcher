// SPDX-License-Identifier: GPL-3.0-only
use icaro_core::compartir::accesos::Accesos;
use icaro_core::compartir::{
    accesos::Autoridad,
    identidad::{AlmacenClave, Identidad},
    revisiones::{self, ContextoPublicacion},
    transporte::{Cliente, Modo, NodoAnfitrion},
    Error, Limites, Manifiesto,
};
use sha2::{Digest, Sha512};
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, RwLock},
};
use tokio_util::sync::CancellationToken;
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
fn identidad() -> Identidad {
    Identidad::desde_almacen(&Memoria(Mutex::new(None))).unwrap()
}
struct Permiso(bool);
impl Autoridad for Permiso {
    fn autorizado(
        &self,
        _: String,
    ) -> Pin<Box<dyn Future<Output = Result<bool, Error>> + Send + '_>> {
        Box::pin(async move { Ok(self.0) })
    }
}

#[tokio::test]
async fn control_excesivo_se_rechaza_antes_de_leer_el_cuerpo() {
    use icaro_core::compartir::protocolo::{self, Solicitud, CONTROL_BYTES};
    use tokio::io::AsyncWriteExt;
    let (mut send, mut recv) = tokio::io::duplex(8);
    send.write_u32(CONTROL_BYTES as u32 + 1).await.unwrap();
    assert!(matches!(
        protocolo::leer::<Solicitud>(&mut recv, CONTROL_BYTES).await,
        Err(Error::Limite(_))
    ));
}

#[tokio::test]
async fn dos_endpoints_revision_offset_integridad_y_revocacion() {
    let host = identidad();
    let guest =
        Identidad::desde_almacen(&Memoria(Mutex::new(Some(Zeroizing::new(vec![7; 32]))))).unwrap();
    let root = std::env::temp_dir().join(format!("icaro-iroh-{}", host.clave_publica()));
    std::fs::create_dir_all(root.join("contenido")).unwrap();
    let datos = vec![42u8; 8 * 1024 * 1024];
    let contenido = datos.as_slice();
    let hash = format!("{:x}", Sha512::digest(contenido));
    std::fs::write(root.join("contenido").join(&hash), contenido).unwrap();
    let m:Manifiesto=serde_json::from_value(serde_json::json!({
        "esquema":"icaro-compartir/1","origen":{"anfitrion":host.clave_publica(),"publicacion":"ab".repeat(16)},
        "revision":1,"perfil":{"minecraft":"1.21.1","loader":"vanilla","loader_version":null},
        "archivos":[{"ruta":"config/a.toml","bytes":contenido.len(),"sha512":hash,"procedencia":{"tipo":"propio"}}],"packs":[]
    })).unwrap();
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
    acl.autorizar(&guest.clave_publica()).unwrap();
    let nodo = NodoAnfitrion::iniciar(
        host.clone(),
        Arc::new(RwLock::new(revision)),
        acl,
        Arc::new(Permiso(true)),
        Modo::Local,
        Limites::default(),
    )
    .await
    .unwrap();
    let cliente = Cliente::conectar(
        &guest,
        nodo.direccion(),
        &m.origen,
        Modo::Local,
        Limites::default(),
    )
    .await
    .unwrap();
    let consultada = cliente.consultar().await.unwrap();
    assert_eq!(m, consultada);
    // Recorrido completo: la red prepara, solo la confirmación final modifica destino.
    let destino = root.join("destino");
    std::fs::create_dir_all(destino.join("mods")).unwrap();
    std::fs::write(destino.join("mods/personal.jar"), b"personal").unwrap();
    let ctx_destino = icaro_core::compartir::aplicacion::ContextoDestino {
        juego: destino.clone(),
        limites: Limites::default(),
    };
    let inventario = icaro_core::compartir::inventario::analizar(
        &destino,
        &CancellationToken::new(),
        &Limites::default(),
    )
    .unwrap();
    let plan =
        icaro_core::compartir::plan::planificar(&consultada, &Default::default(), &inventario)
            .unwrap();
    let preparada = icaro_core::compartir::descargas::preparar(
        &ctx_destino,
        plan,
        &cliente,
        None,
        &root.join("staging-plan"),
        &CancellationToken::new(),
    )
    .await
    .unwrap();
    assert!(!destino.join("config/a.toml").exists());
    icaro_core::compartir::aplicacion::aplicar(preparada, &CancellationToken::new(), |_| {})
        .unwrap();
    assert_eq!(
        std::fs::read(destino.join("config/a.toml")).unwrap(),
        contenido
    );
    assert_eq!(
        std::fs::read(destino.join("mods/personal.jar")).unwrap(),
        b"personal"
    );
    let mut firmada =
        icaro_core::compartir::firmas::RevisionFirmada::crear(&host, &m, &Limites::default())
            .unwrap();
    firmada.manifiesto.push(' ');
    assert!(firmada
        .verificar(&host.clave_publica(), &Limites::default())
        .is_err());
    let mut falso_origen = m.origen.clone();
    falso_origen.anfitrion = identidad().clave_publica();
    assert!(Cliente::conectar(
        &guest,
        nodo.direccion(),
        &falso_origen,
        Modo::Local,
        Limites::default()
    )
    .await
    .is_err());
    // También se prueba el rechazo en el servidor, sin las validaciones del cliente.
    let raw = iroh::Endpoint::builder(iroh::endpoint::presets::Minimal)
        .secret_key(iroh::SecretKey::from_bytes(&[7; 32]))
        .relay_mode(iroh::RelayMode::Disabled)
        .bind_addr("127.0.0.1:0")
        .unwrap()
        .bind()
        .await
        .unwrap();
    let conn = raw
        .connect(nodo.direccion(), icaro_core::compartir::protocolo::ALPN)
        .await
        .unwrap();
    for (sha, offset) in [
        ("cd".repeat(64), 0),
        (hash.clone(), contenido.len() as u64 + 1),
    ] {
        use icaro_core::compartir::protocolo::{self, Respuesta, Solicitud, CONTROL_BYTES};
        let (mut send, mut recv) = conn.open_bi().await.unwrap();
        protocolo::escribir(
            &mut send,
            &Solicitud::Archivo {
                origen: m.origen.clone(),
                revision: 1,
                sha512: sha,
                offset,
            },
            CONTROL_BYTES,
        )
        .await
        .unwrap();
        send.finish().unwrap();
        assert!(protocolo::leer::<Respuesta>(&mut recv, CONTROL_BYTES)
            .await
            .is_err());
    }
    let staging = root.join("staging");
    std::fs::create_dir(&staging).unwrap();
    let parcial = staging.join(format!("{hash}.part"));
    std::fs::write(&parcial, &contenido[..7]).unwrap();
    let cancel = CancellationToken::new();
    let corte = async {
        loop {
            if std::fs::metadata(&parcial).is_ok_and(|m| m.len() > 64 * 1024) {
                cancel.cancel();
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
    };
    let (resultado, ()) = tokio::join!(
        cliente.descargar(&m, &m.archivos[0], &staging, &cancel),
        corte
    );
    assert!(matches!(resultado, Err(Error::Cancelada)));
    assert!(std::fs::metadata(&parcial).unwrap().len() < (contenido.len() as u64));
    let path = cliente
        .descargar(&m, &m.archivos[0], &staging, &CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(std::fs::read(path).unwrap(), contenido);
    let mut otro = m.archivos[0].clone();
    otro.sha512 = "cd".repeat(64);
    assert!(cliente
        .descargar(&m, &otro, &staging, &CancellationToken::new())
        .await
        .is_err());
    std::fs::write(&parcial, vec![0; contenido.len()]).unwrap();
    assert!(cliente
        .descargar(&m, &m.archivos[0], &staging, &CancellationToken::new())
        .await
        .is_err());
    let desconocido = Cliente::conectar(
        &identidad(),
        nodo.direccion(),
        &m.origen,
        Modo::Local,
        Limites::default(),
    )
    .await
    .unwrap();
    assert!(desconocido.consultar().await.is_err());
    desconocido.cerrar().await;
    let (mut send, mut recv) = conn.open_bi().await.unwrap();
    use icaro_core::compartir::protocolo::{self, Respuesta, Solicitud, CONTROL_BYTES};
    protocolo::escribir(
        &mut send,
        &Solicitud::Archivo {
            origen: m.origen.clone(),
            revision: 1,
            sha512: hash,
            offset: 0,
        },
        CONTROL_BYTES,
    )
    .await
    .unwrap();
    send.finish().unwrap();
    assert!(matches!(
        protocolo::leer::<Respuesta>(&mut recv, CONTROL_BYTES)
            .await
            .unwrap(),
        Respuesta::Archivo { .. }
    ));
    nodo.revocar(&guest.clave_publica()).await.unwrap();
    assert!(recv.read_to_end(contenido.len() + 1).await.is_err());
    assert!(cliente.consultar().await.is_err());
    cliente.cerrar().await;
    raw.close().await;
    nodo.parar().await;
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn revocacion_persiste_tras_reabrir() {
    let root = std::env::temp_dir().join(format!("icaro-acl-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("accesos.json");
    let guest = "ab".repeat(32);
    let acl = Accesos::abrir(path.clone()).unwrap();
    acl.autorizar(&guest).unwrap();
    assert!(acl.permitir(&guest));
    acl.revocar(&guest).unwrap();
    assert!(!Accesos::abrir(path.clone()).unwrap().permitir(&guest));
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn detener_publicacion_persiste_y_solo_reapertura_explicita_limpia_accesos() {
    let root = std::env::temp_dir().join(format!("icaro-detener-{}", identidad().clave_publica()));
    std::fs::create_dir(&root).unwrap();
    let path = root.join("accesos.json");
    let guest = "ab".repeat(32);
    let acl = Accesos::abrir(path.clone()).unwrap();
    acl.autorizar(&guest).unwrap();
    acl.detener().unwrap();
    let reabierta = Accesos::abrir(path.clone()).unwrap();
    assert!(!reabierta.permitir(&guest));
    assert!(reabierta.autorizar(&guest).is_err());
    reabierta.reiniciar().unwrap();
    assert!(!reabierta.permitir(&guest));
    reabierta.autorizar(&guest).unwrap();
    assert!(reabierta.permitir(&guest));
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(root).unwrap();
}
