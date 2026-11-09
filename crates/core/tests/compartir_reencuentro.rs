// SPDX-License-Identifier: GPL-3.0-only
//! Reencuentro de una publicación ya aceptada sin conservar el código de invitación.
use icaro_core::compartir::{
    identidad::{AlmacenClave, Identidad},
    invitaciones::{ClienteInvitaciones, EncuentroFirmado, Reencuentro},
    Error, Origen,
};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use zeroize::Zeroizing;

struct Memoria(Mutex<Option<Zeroizing<Vec<u8>>>>);
impl AlmacenClave for Memoria {
    fn leer(&self) -> Result<Option<Zeroizing<Vec<u8>>>, Error> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn guardar(&self, valor: &[u8]) -> Result<(), Error> {
        *self.0.lock().unwrap() = Some(Zeroizing::new(valor.to_vec()));
        Ok(())
    }
}
fn identidad() -> Identidad {
    Identidad::desde_almacen(&Memoria(Mutex::new(None))).unwrap()
}
fn identidad_fija(semilla: u8) -> Identidad {
    Identidad::desde_almacen(&Memoria(Mutex::new(Some(Zeroizing::new(vec![
        semilla;
        32
    ])))))
    .unwrap()
}
async fn endpoint(semilla: u8) -> iroh::Endpoint {
    iroh::Endpoint::builder(iroh::endpoint::presets::Minimal)
        .secret_key(iroh::SecretKey::from_bytes(&[semilla; 32]))
        .relay_mode(iroh::RelayMode::Disabled)
        .bind_addr("127.0.0.1:0")
        .unwrap()
        .bind()
        .await
        .unwrap()
}

struct Peticion {
    ruta: String,
    cuerpo: Value,
}
type Recibidas = Arc<Mutex<Vec<Peticion>>>;

/// Servidor HTTP/1.1 mínimo en loopback que registra cada petición y responde
/// siempre lo mismo. Cierra la conexión tras cada respuesta.
async fn servidor(estado: u16, cuerpo: String) -> (String, Recibidas) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());
    let recibidas: Recibidas = Arc::default();
    let copia = recibidas.clone();
    tokio::spawn(async move {
        while let Ok((mut socket, _)) = listener.accept().await {
            if let Some(peticion) = leer(&mut socket).await {
                copia.lock().unwrap().push(peticion);
            }
            let respuesta = format!(
                "HTTP/1.1 {estado} Respuesta\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{cuerpo}",
                cuerpo.len()
            );
            let _ = socket.write_all(respuesta.as_bytes()).await;
            let _ = socket.shutdown().await;
        }
    });
    (url, recibidas)
}
async fn leer(socket: &mut tokio::net::TcpStream) -> Option<Peticion> {
    let mut datos = Vec::new();
    let mut buf = [0u8; 4096];
    let fin = loop {
        let n = socket.read(&mut buf).await.ok()?;
        if n == 0 {
            return None;
        }
        datos.extend_from_slice(&buf[..n]);
        if let Some(p) = datos.windows(4).position(|w| w == b"\r\n\r\n") {
            break p + 4;
        }
    };
    let cabeceras = String::from_utf8_lossy(&datos[..fin]).into_owned();
    let largo = cabeceras
        .lines()
        .find_map(|l| {
            let (k, v) = l.split_once(':')?;
            k.trim()
                .eq_ignore_ascii_case("content-length")
                .then(|| v.trim().parse::<usize>().ok())
                .flatten()
        })
        .unwrap_or(0);
    while datos.len() < fin + largo {
        let n = socket.read(&mut buf).await.ok()?;
        if n == 0 {
            break;
        }
        datos.extend_from_slice(&buf[..n]);
    }
    let primera = cabeceras.lines().next().unwrap_or_default().to_owned();
    let mut partes = primera.split(' ');
    if partes.next() != Some("POST") {
        return None;
    }
    Some(Peticion {
        ruta: partes.next().unwrap_or_default().to_owned(),
        cuerpo: serde_json::from_slice(datos.get(fin..fin + largo)?).ok()?,
    })
}

fn verificar_peticion(p: &Peticion, invitado: &Identidad, origen: &Origen) {
    assert_eq!(p.ruta, "/v1/reconnect");
    let c = &p.cuerpo;
    let clave = c["public_key"].as_str().unwrap();
    assert_eq!(clave, invitado.clave_publica());
    let payload = c["payload"].as_str().unwrap();
    let mensaje = format!(
        "icaro-invitaciones/1\nPOST\n/v1/reconnect\n{clave}\n{}\n{}\n{payload}",
        c["nonce"].as_str().unwrap(),
        c["issued_at"].as_u64().unwrap()
    );
    Identidad::verificar(clave, mensaje.as_bytes(), c["signature"].as_str().unwrap())
        .expect("la petición va firmada por la identidad del invitado");
    let datos: Value = serde_json::from_str(payload).unwrap();
    assert_eq!(
        datos,
        json!({"host": origen.anfitrion, "publication": origen.publicacion})
    );
}
fn respuesta(host: &str, publicacion: &str, meeting: &EncuentroFirmado) -> String {
    json!({"host": host, "publication": publicacion, "meeting": meeting}).to_string()
}
fn publicacion() -> String {
    identidad().clave_publica()[..32].to_owned()
}

#[tokio::test]
async fn reencontrar_envia_peticion_firmada_y_devuelve_encuentro_verificado() {
    let host = identidad_fija(17);
    let invitado = identidad();
    let ep = endpoint(17).await;
    let pubid = publicacion();
    let origen = Origen {
        anfitrion: host.clave_publica(),
        publicacion: pubid.clone(),
    };
    let meeting = EncuentroFirmado::desde_direccion(&host, &pubid, &ep.addr()).unwrap();
    let (url, recibidas) = servidor(200, respuesta(&origen.anfitrion, &pubid, &meeting)).await;
    let cliente = ClienteInvitaciones::nuevo(&url, invitado.clone()).unwrap();
    let r: Reencuentro = cliente.reencontrar(&origen).await.unwrap();
    assert_eq!(r.host, origen.anfitrion);
    assert_eq!(r.publication, origen.publicacion);
    assert_eq!(r.meeting.payload, meeting.payload);
    assert_eq!(
        r.meeting.direccion(&origen.anfitrion, &pubid).unwrap(),
        ep.addr()
    );
    let recibidas = recibidas.lock().unwrap();
    assert_eq!(recibidas.len(), 1);
    verificar_peticion(&recibidas[0], &invitado, &origen);
    drop(recibidas);
    ep.close().await;
}

#[tokio::test]
async fn reencontrar_rechaza_respuestas_que_no_corresponden_al_origen() {
    let host = identidad_fija(17);
    let otro = identidad_fija(18);
    let ep = endpoint(17).await;
    let pubid = publicacion();
    let otra_pub = publicacion();
    let origen = Origen {
        anfitrion: host.clave_publica(),
        publicacion: pubid.clone(),
    };
    let valido = EncuentroFirmado::desde_direccion(&host, &pubid, &ep.addr()).unwrap();
    let hint = serde_json::to_string(&ep.addr()).unwrap();
    let de_otro = EncuentroFirmado::crear(&otro, &pubid, vec![hint.clone()]).unwrap();
    let mut alterado = valido.clone();
    alterado.signature = "0".repeat(128);
    let mut alterado_payload = valido.clone();
    alterado_payload.payload = alterado_payload.payload.replace(&pubid, &otra_pub);
    let sin_direccion =
        EncuentroFirmado::crear(&host, &pubid, vec!["iroh:test-local".into()]).unwrap();
    let mut extra: Value =
        serde_json::from_str(&respuesta(&origen.anfitrion, &pubid, &valido)).unwrap();
    extra["expires_at"] = json!(1);
    let casos = [
        // Otro anfitrión, aunque su encuentro sea válido para sí mismo.
        respuesta(&otro.clave_publica(), &pubid, &de_otro),
        // Otra publicación del mismo anfitrión.
        respuesta(
            &origen.anfitrion,
            &otra_pub,
            &EncuentroFirmado::desde_direccion(&host, &otra_pub, &ep.addr()).unwrap(),
        ),
        // Encuentro firmado por otra clave con el anfitrión esperado.
        respuesta(&origen.anfitrion, &pubid, &de_otro),
        respuesta(&origen.anfitrion, &pubid, &alterado),
        respuesta(&origen.anfitrion, &pubid, &alterado_payload),
        // Encuentro del origen correcto pero sin dirección utilizable.
        respuesta(&origen.anfitrion, &pubid, &sin_direccion),
        extra.to_string(),
        "{}".into(),
        "no es json".into(),
    ];
    for (i, cuerpo) in casos.into_iter().enumerate() {
        let (url, recibidas) = servidor(200, cuerpo).await;
        let cliente = ClienteInvitaciones::nuevo(&url, identidad()).unwrap();
        let resultado = cliente.reencontrar(&origen).await;
        assert!(
            matches!(resultado, Err(Error::Datos(_))),
            "caso {i}: {resultado:?}"
        );
        assert_eq!(recibidas.lock().unwrap().len(), 1, "caso {i}");
    }
    ep.close().await;
}

#[tokio::test]
async fn reencontrar_rechaza_endpoint_ajeno_aunque_lo_firme_el_anfitrion() {
    // El anfitrión (semilla 18) firma una pista cuyo endpoint es otra identidad.
    let host = identidad_fija(18);
    let ep = endpoint(17).await;
    let pubid = publicacion();
    let origen = Origen {
        anfitrion: host.clave_publica(),
        publicacion: pubid.clone(),
    };
    let hint = serde_json::to_string(&ep.addr()).unwrap();
    let meeting = EncuentroFirmado::crear(&host, &pubid, vec![hint]).unwrap();
    let (url, recibidas) = servidor(200, respuesta(&origen.anfitrion, &pubid, &meeting)).await;
    let cliente = ClienteInvitaciones::nuevo(&url, identidad()).unwrap();
    assert!(matches!(
        cliente.reencontrar(&origen).await,
        Err(Error::Datos(_))
    ));
    assert_eq!(recibidas.lock().unwrap().len(), 1);
    ep.close().await;
}

#[tokio::test]
async fn reencontrar_valida_el_origen_antes_de_pedir() {
    let (url, recibidas) = servidor(200, "{}".into()).await;
    let cliente = ClienteInvitaciones::nuevo(&url, identidad()).unwrap();
    let valido = identidad().clave_publica();
    for origen in [
        Origen {
            anfitrion: valido.to_uppercase(),
            publicacion: "ab".repeat(16),
        },
        Origen {
            anfitrion: valido.clone(),
            publicacion: "ab".repeat(15),
        },
        Origen {
            anfitrion: valido[..62].to_owned(),
            publicacion: "ab".repeat(16),
        },
        Origen {
            anfitrion: valido.clone(),
            publicacion: "zz".repeat(16),
        },
    ] {
        assert!(matches!(
            cliente.reencontrar(&origen).await,
            Err(Error::Datos(_))
        ));
    }
    assert!(recibidas.lock().unwrap().is_empty());
}

#[tokio::test]
async fn reencontrar_propaga_los_errores_del_servicio() {
    let origen = Origen {
        anfitrion: identidad().clave_publica(),
        publicacion: publicacion(),
    };
    for (estado, codigo) in [
        (403, "access_revoked"),
        (403, "wrong_host"),
        (404, "not_found"),
        (410, "sharing_stopped"),
    ] {
        let (url, _) = servidor(estado, json!({ "error": codigo }).to_string()).await;
        let cliente = ClienteInvitaciones::nuevo(&url, identidad()).unwrap();
        match cliente.reencontrar(&origen).await {
            Err(Error::Servicio {
                estado: e,
                codigo: c,
                ..
            }) => {
                assert_eq!(e, estado);
                assert_eq!(c, codigo);
            }
            otro => panic!("{estado} {codigo}: {otro:?}"),
        }
    }
}

#[tokio::test]
#[ignore = "Requiere Worker local explícito; nunca producción"]
async fn reencuentro_interopera_con_worker_d1_local() {
    use icaro_core::compartir::invitaciones::{Caducidad, Codigo};
    let url = std::env::var("ICARO_INVITACIONES_TEST_URL").expect("URL de Worker local");
    assert!(url.starts_with("http://127.0.0.1:"));
    let host = identidad_fija(17);
    let ep = endpoint(17).await;
    let guest = identidad();
    let ajeno = identidad();
    let pubid = guest.clave_publica()[..32].to_owned();
    let origen = Origen {
        anfitrion: host.clave_publica(),
        publicacion: pubid.clone(),
    };
    let anfitrion = ClienteInvitaciones::nuevo(&url, host.clone()).unwrap();
    let invitado = ClienteInvitaciones::nuevo(&url, guest).unwrap();
    let desconocido = ClienteInvitaciones::nuevo(&url, ajeno).unwrap();
    let meeting = EncuentroFirmado::desde_direccion(&host, &pubid, &ep.addr()).unwrap();
    let creada = anfitrion
        .nueva(&pubid, &meeting, Caducidad::Horas24, 1)
        .await
        .unwrap();
    let codigo = Codigo::parse(&creada.code).unwrap();
    assert!(matches!(
        invitado.reencontrar(&origen).await,
        Err(Error::Servicio { estado: 403, .. })
    ));
    invitado.aceptar(&codigo).await.unwrap();
    anfitrion.revocar_codigo(&codigo).await.unwrap();
    let r = invitado.reencontrar(&origen).await.unwrap();
    assert_eq!(
        r.meeting.direccion(&origen.anfitrion, &pubid).unwrap(),
        ep.addr()
    );
    assert!(matches!(
        desconocido.reencontrar(&origen).await,
        Err(Error::Servicio { estado: 403, .. })
    ));
    anfitrion.detener(&pubid).await.unwrap();
    assert!(matches!(
        invitado.reencontrar(&origen).await,
        Err(Error::Servicio { estado: 410, .. })
    ));
    ep.close().await;
}
