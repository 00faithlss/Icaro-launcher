// SPDX-License-Identifier: GPL-3.0-only
use icaro_core::{
    diagnostico::{self, Causa},
    diagnostico_contexto as contexto,
};
use std::{
    fs,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio_util::sync::CancellationToken;
struct Temporal(PathBuf);
impl Temporal {
    fn nueva() -> Self {
        static SIGUIENTE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let p = std::env::temp_dir().join(format!(
            "icaro-contexto-{}-{}-{}",
            std::process::id(),
            SIGUIENTE.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&p).unwrap();
        Self(p)
    }
    fn lineas(&self) -> Vec<serde_json::Value> {
        fs::read_to_string(self.0.join("logs/diagnostico.jsonl"))
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }
}
impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
async fn causa_de_io_sobrevive_al_cambio_de_hilo_y_al_texto_generico() {
    let t = Temporal::nueva();
    let mut s = diagnostico::iniciar(&t.0, "prueba", "0").unwrap();
    let r = contexto::observar_con(
        &s.registro(),
        "compartir",
        "aplicar",
        &CancellationToken::new(),
        async {
            contexto::etapa("transaccion");
            contexto::bloqueante(|| {
                contexto::fallo(Causa::de_io(&std::io::Error::from_raw_os_error(5)));
            })
            .await
            .unwrap();
            contexto::fallo(Causa::Disco {
                tipo: "sin_detalle",
                codigo_os: None,
            });
            Err::<(), _>(
                "secreto https://privado.example/token=credencial C:\\Users\\CuentaPrivada\\mundo"
                    .to_string(),
            )
        },
    )
    .await;
    assert!(r.is_err());
    s.cerrar(Duration::from_secs(3)).unwrap();
    let lineas = t.lineas();
    let fin = lineas.last().unwrap();
    assert_eq!(fin["causa"]["codigo_os"], 5);
    assert_eq!(fin["etapa"], "transaccion");
    let texto = fs::read_to_string(t.0.join("logs/diagnostico.jsonl")).unwrap();
    for privado in [
        "CuentaPrivada",
        "privado.example",
        "credencial",
        "secreto",
        "C:\\\\Users",
    ] {
        assert!(!texto.contains(privado), "{privado}");
    }
}

#[tokio::test]
async fn descartar_futuro_deja_abandono_y_no_contamina_el_siguiente() {
    let t = Temporal::nueva();
    let mut s = diagnostico::iniciar(&t.0, "prueba", "0").unwrap();
    let registro = s.registro();
    let (listo, esperar) = tokio::sync::oneshot::channel();
    let hijo = tokio::spawn(async move {
        contexto::observar_con(
            &registro,
            "compartir",
            "interrumpida",
            &CancellationToken::new(),
            async move {
                contexto::etapa("descarga");
                let _ = listo.send(());
                std::future::pending::<Result<(), ()>>().await
            },
        )
        .await
    });
    esperar.await.unwrap();
    hijo.abort();
    let _ = hijo.await;
    contexto::observar_con(
        &s.registro(),
        "compartir",
        "siguiente",
        &CancellationToken::new(),
        async { Ok::<(), ()>(()) },
    )
    .await
    .unwrap();
    s.cerrar(Duration::from_secs(3)).unwrap();
    let lineas = t.lineas();
    assert!(lineas
        .iter()
        .any(|l| l["accion"] == "interrumpida" && l["resultado"] == "abandonado"));
    assert!(lineas
        .iter()
        .any(|l| l["accion"] == "siguiente" && l["resultado"] == "exito"));
}

#[tokio::test]
async fn llamadas_anidadas_conservan_la_operacion_y_la_causa_original() {
    let t = Temporal::nueva();
    let mut s = diagnostico::iniciar(&t.0, "prueba", "0").unwrap();
    let token = CancellationToken::new();
    let r = contexto::observar_con(&s.registro(), "compartir", "principal", &token, async {
        contexto::observar("compartir", "secundaria", None, &token, async {
            contexto::etapa("transaccion");
            contexto::fallo(Causa::de_io(&std::io::Error::from_raw_os_error(5)));
            Err::<(), ()>(())
        })
        .await
    })
    .await;
    assert!(r.is_err());
    s.cerrar(Duration::from_secs(3)).unwrap();
    let lineas = t.lineas();
    assert_eq!(lineas.last().unwrap()["causa"]["codigo_os"], 5);
    assert!(lineas
        .iter()
        .all(|l| l["operacion"] == lineas[0]["operacion"]));
}

#[tokio::test]
async fn fallo_recuperado_de_otra_etapa_no_oculta_la_causa_final() {
    let t = Temporal::nueva();
    let mut s = diagnostico::iniciar(&t.0, "prueba", "0").unwrap();
    let _ = contexto::observar_con(
        &s.registro(),
        "compartir",
        "reintento",
        &CancellationToken::new(),
        async {
            contexto::etapa("catalogo");
            contexto::fallo(Causa::Servicio {
                estado: 429,
                codigo: "rate_limited",
            });
            contexto::etapa("perfil");
            contexto::fallo(Causa::Validacion("perfil_invalido"));
            Err::<(), ()>(())
        },
    )
    .await;
    s.cerrar(Duration::from_secs(3)).unwrap();
    assert_eq!(
        t.lineas().last().unwrap()["causa"]["codigo"],
        "perfil_invalido"
    );
}

#[tokio::test]
async fn cancelacion_no_oculta_un_fallo_de_recuperacion() {
    let t = Temporal::nueva();
    let mut s = diagnostico::iniciar(&t.0, "prueba", "0").unwrap();
    let cancel = CancellationToken::new();
    cancel.cancel();
    let _ = contexto::observar_con(&s.registro(), "compartir", "aplicar", &cancel, async {
        contexto::etapa("recuperacion");
        contexto::fallo(Causa::Conflicto);
        Err::<(), ()>(())
    })
    .await;
    s.cerrar(Duration::from_secs(3)).unwrap();
    let ls = t.lineas();
    assert_eq!(ls.last().unwrap()["resultado"], "error");
    assert_eq!(ls.last().unwrap()["causa"]["categoria"], "conflicto");
}

#[tokio::test]
async fn preparacion_clasifica_java_y_minecraft_sin_volcar_sus_textos() {
    use icaro_core::{java, minecraft};
    let t = Temporal::nueva();
    let mut s = diagnostico::iniciar(&t.0, "prueba", "0").unwrap();
    let cancel = CancellationToken::new();
    let _ = contexto::observar_con(&s.registro(), "juego", "java", &cancel, async {
        contexto::etapa("java");
        Err::<(), _>(contexto::texto(java::Error::PruebaJvm(
            "credencial-dinamica-privada".into(),
        )))
    })
    .await;
    let _ = contexto::observar_con(&s.registro(), "juego", "minecraft", &cancel, async {
        contexto::etapa("minecraft_loader");
        Err::<(), _>(contexto::texto(minecraft::Error::Integridad))
    })
    .await;
    s.cerrar(Duration::from_secs(3)).unwrap();
    let ls = t.lineas();
    assert!(ls.iter().any(|l| l["accion"] == "java"
        && l["resultado"] == "error"
        && l["causa"]["codigo"] == "prueba_jvm"));
    assert!(ls
        .iter()
        .any(|l| l["accion"] == "minecraft" && l["causa"]["categoria"] == "integridad"));
    assert!(!fs::read_to_string(t.0.join("logs/diagnostico.jsonl"))
        .unwrap()
        .contains("credencial-dinamica-privada"));
}

#[tokio::test]
async fn acceso_revocado_real_deja_http_y_etapa_sin_url_claves_ni_codigo_privado() {
    use icaro_core::compartir::{
        identidad::{AlmacenClave, Identidad, Zeroizing},
        invitaciones::ClienteInvitaciones,
        Error, Origen,
    };
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    struct Memoria;
    impl AlmacenClave for Memoria {
        fn leer(&self) -> Result<Option<Zeroizing<Vec<u8>>>, Error> {
            Ok(Some(Zeroizing::new(vec![31; 32])))
        }
        fn guardar(&self, _: &[u8]) -> Result<(), Error> {
            unreachable!()
        }
    }
    let t = Temporal::nueva();
    let mut s = diagnostico::iniciar(&t.0, "prueba", "0").unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let servidor = tokio::spawn(async move {
        let (mut sock, _) = listener.accept().await.unwrap();
        let mut entrada = Vec::new();
        loop {
            let mut bloque = [0u8; 1024];
            let n = sock.read(&mut bloque).await.unwrap();
            assert!(n > 0 && entrada.len() + n <= 16384);
            entrada.extend_from_slice(&bloque[..n]);
            if let Some(fin) = entrada.windows(4).position(|b| b == b"\r\n\r\n") {
                let cabecera = String::from_utf8_lossy(&entrada[..fin]);
                let bytes = cabecera
                    .lines()
                    .find_map(|l| {
                        let (nombre, valor) = l.split_once(':')?;
                        nombre
                            .eq_ignore_ascii_case("content-length")
                            .then(|| valor.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                if entrada.len() >= fin + 4 + bytes {
                    break;
                }
            }
        }
        let cuerpo = r#"{"error":"access_revoked","detalle":"credencial-secreta-no-registrar"}"#;
        let respuesta=format!("HTTP/1.1 403 Forbidden\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{cuerpo}",cuerpo.len());
        sock.write_all(respuesta.as_bytes()).await.unwrap();
        sock.shutdown().await.unwrap();
    });
    let identidad = Identidad::desde_almacen(&Memoria).unwrap();
    let clave = identidad.clave_publica();
    let origen = Origen {
        anfitrion: clave.clone(),
        publicacion: "ab".repeat(16),
    };
    let cliente = ClienteInvitaciones::nuevo(&url, identidad).unwrap();
    let resultado = contexto::observar_con(
        &s.registro(),
        "compartir",
        "consultar_revision",
        &CancellationToken::new(),
        async {
            contexto::etapa("reconexion_autorizada");
            cliente.reencontrar(&origen).await
        },
    )
    .await;
    assert!(matches!(
        resultado,
        Err(Error::Servicio { estado: 403, .. })
    ));
    servidor.await.unwrap();
    s.cerrar(Duration::from_secs(3)).unwrap();
    let ls = t.lineas();
    let fin = ls.last().unwrap();
    assert_eq!(fin["causa"]["estado"], 403);
    assert_eq!(fin["causa"]["codigo"], "access_revoked");
    assert_eq!(fin["etapa"], "reconexion_autorizada");
    let texto = fs::read_to_string(t.0.join("logs/diagnostico.jsonl")).unwrap();
    for secreto in [&url, &clave, "credencial-secreta-no-registrar"] {
        assert!(!texto.contains(secreto));
    }
}

#[tokio::test]
async fn abandono_no_espera_al_hilo_bloqueante_y_descarta_su_progreso_tardio() {
    let t = Temporal::nueva();
    let mut s = diagnostico::iniciar(&t.0, "prueba", "0").unwrap();
    let registro = s.registro();
    let (empezo, esperar_inicio) = tokio::sync::oneshot::channel();
    let (liberar, pausa) = std::sync::mpsc::channel();
    let (fin, esperar_fin) = tokio::sync::oneshot::channel();
    let hijo = tokio::spawn(async move {
        contexto::observar_con(
            &registro,
            "compartir",
            "interrumpida_bloqueante",
            &CancellationToken::new(),
            async move {
                contexto::etapa("lectura");
                contexto::bloqueante(move || {
                    let _ = empezo.send(());
                    let _ = pausa.recv_timeout(Duration::from_secs(5));
                    contexto::etapa("progreso_tardio");
                    let _ = fin.send(());
                })
                .await
                .unwrap();
                Ok::<(), ()>(())
            },
        )
        .await
    });
    esperar_inicio.await.unwrap();
    hijo.abort();
    let _ = hijo.await;
    s.registro().vaciar(Duration::from_secs(3)).unwrap();
    let ya_abandonada = t.lineas().iter().any(|l| l["resultado"] == "abandonado");
    let _ = liberar.send(());
    let _ = esperar_fin.await;
    s.cerrar(Duration::from_secs(3)).unwrap();
    assert!(
        ya_abandonada,
        "se registra al descartar el futuro, no al terminar su hilo"
    );
    assert!(!t.lineas().iter().any(|l| l["etapa"] == "progreso_tardio"));
}

#[test]
fn incidente_sin_contexto_conserva_causa_del_nucleo_en_registro_global() {
    let t = Temporal::nueva();
    let mut s = diagnostico::iniciar(&t.0, "prueba", "0").unwrap();
    assert!(s.instalar_global());
    contexto::registrar_error(&std::io::Error::from_raw_os_error(5));
    s.cerrar(Duration::from_secs(3)).unwrap();
    let ls = t.lineas();
    assert_eq!(ls.last().unwrap()["causa"]["codigo_os"], 5);
    assert_eq!(ls.last().unwrap()["accion"], "incidente");
}
