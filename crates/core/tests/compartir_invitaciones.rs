// SPDX-License-Identifier: GPL-3.0-only
use icaro_core::compartir::{
    identidad::{AlmacenClave, Identidad},
    invitaciones::{Caducidad, ClienteInvitaciones, Codigo, EncuentroFirmado},
    Error,
};
use std::sync::Mutex;
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

#[test]
fn codigo_se_normaliza_y_no_aparece_en_debug() {
    let codigo = Codigo::parse("7qkm-4xpt-r2c9").unwrap();
    assert_eq!(codigo.as_str(), "7QKM4XPTR2C9");
    assert_eq!(codigo.mostrar(), "7QKM-4XPT-R2C9");
    assert!(!format!("{codigo:?}").contains("7QKM"));
    assert!(Codigo::parse("invalido").is_err());
}
#[test]
fn encuentro_verifica_clave_publicacion_y_firma_original() {
    let host = identidad();
    let pubid = "ab".repeat(16);
    let mut encuentro =
        EncuentroFirmado::crear(&host, &pubid, vec!["endpoint-local".into()]).unwrap();
    let datos = encuentro.verificar(&host.clave_publica(), &pubid).unwrap();
    assert_eq!(datos.endpoint_hints, vec!["endpoint-local"]);
    assert!(encuentro
        .verificar(&identidad().clave_publica(), &pubid)
        .is_err());
    encuentro.payload = encuentro
        .payload
        .replace("endpoint-local", "endpoint-ajeno");
    assert!(encuentro.verificar(&host.clave_publica(), &pubid).is_err());
}
#[test]
fn servicio_requiere_https_o_loopback_explicito() {
    for url in [
        "http://example.com",
        "https://usuario:clave@example.com",
        "https://example.com/?token=algo",
        "https://example.com/#algo",
    ] {
        assert!(
            ClienteInvitaciones::nuevo(url, identidad()).is_err(),
            "{url}"
        );
    }
    assert!(ClienteInvitaciones::nuevo("http://127.0.0.1:8787", identidad()).is_ok());
}

#[tokio::test]
async fn encuentro_conserva_endpoint_sin_permitir_sustituir_identidad() {
    let host = identidad();
    let endpoint = iroh::Endpoint::builder(iroh::endpoint::presets::Minimal)
        .secret_key(iroh::SecretKey::from_bytes(&[17; 32]))
        .relay_mode(iroh::RelayMode::Disabled)
        .bind_addr("127.0.0.1:0")
        .unwrap()
        .bind()
        .await
        .unwrap();
    let publicacion = "ab".repeat(16);
    assert!(EncuentroFirmado::desde_direccion(&host, &publicacion, &endpoint.addr()).is_err());
    let correcta =
        Identidad::desde_almacen(&Memoria(Mutex::new(Some(Zeroizing::new(vec![17; 32]))))).unwrap();
    let encuentro =
        EncuentroFirmado::desde_direccion(&correcta, &publicacion, &endpoint.addr()).unwrap();
    assert_eq!(
        encuentro
            .direccion(&correcta.clave_publica(), &publicacion)
            .unwrap(),
        endpoint.addr()
    );
    endpoint.close().await;
}

#[tokio::test]
#[ignore = "Requiere Worker local explícito; nunca producción"]
async fn rust_interopera_con_worker_d1_local() {
    let url = std::env::var("ICARO_INVITACIONES_TEST_URL").expect("URL de Worker local");
    assert!(url.starts_with("http://127.0.0.1:"));
    let host = identidad();
    let guest = identidad();
    let pubid = host.clave_publica()[..32].to_owned();
    let anfitrion = ClienteInvitaciones::nuevo(&url, host.clone()).unwrap();
    let invitado = ClienteInvitaciones::nuevo(&url, guest.clone()).unwrap();
    let meeting = EncuentroFirmado::crear(&host, &pubid, vec!["endpoint-local".into()]).unwrap();
    let creada = anfitrion
        .nueva(&pubid, &meeting, Caducidad::Horas24, 1)
        .await
        .unwrap();
    let codigo = Codigo::parse(&creada.code).unwrap();
    assert!(!invitado.resolver(&codigo).await.unwrap().authorized);
    assert!(invitado.aceptar(&codigo).await.unwrap().authorized);
    assert_eq!(invitado.aceptar(&codigo).await.unwrap().accepted, 1);
    assert_eq!(
        anfitrion.autorizados(&pubid).await.unwrap(),
        vec![guest.clave_publica()]
    );
    anfitrion.revocar_codigo(&codigo).await.unwrap();
    assert!(invitado.resolver(&codigo).await.unwrap().authorized);
    recorrido_archivos(&url, &host, &guest, &pubid).await;
    anfitrion
        .revocar_acceso(&pubid, &guest.clave_publica())
        .await
        .unwrap();
    assert!(anfitrion.autorizados(&pubid).await.unwrap().is_empty());
    anfitrion.detener(&pubid).await.unwrap();
}

async fn recorrido_archivos(url: &str, host: &Identidad, guest: &Identidad, publicacion: &str) {
    use icaro_core::compartir::{
        self as c,
        publicacion::{Borrador, Eleccion, Seleccion},
        revisiones,
        transporte::Modo,
    };
    use std::fs;
    let root = std::env::temp_dir().join(format!("icaro-recorrido-{}", host.clave_publica()));
    let juego = root.join("anfitrion");
    fs::create_dir_all(juego.join("config")).unwrap();
    fs::create_dir_all(juego.join("resourcepacks")).unwrap();
    fs::write(juego.join("config/mod.toml"), b"valor=1").unwrap();
    fs::write(juego.join("resourcepacks/texturas.zip"), b"pack compartido").unwrap();
    let cancel = tokio_util::sync::CancellationToken::new();
    let inventario = c::inventario::analizar(&juego, &cancel, &c::Limites::default()).unwrap();
    let mut borrador = Borrador::comparar(&inventario, None, &Seleccion::default()).unwrap();
    for archivo in &inventario.archivos {
        borrador.elegir(&archivo.ruta, Eleccion::Compartir).unwrap();
    }
    borrador.seleccionar_packs(vec!["vanilla".into(), "file/texturas.zip".into()]);
    borrador.autorizar_propios();
    let contexto = revisiones::ContextoPublicacion {
        juego,
        almacen: root.join("publicacion"),
        origen: c::Origen {
            anfitrion: host.clave_publica(),
            publicacion: publicacion.into(),
        },
        perfil: c::Perfil {
            minecraft: "1.21.1".into(),
            loader: "vanilla".into(),
            loader_version: None,
        },
        limites: c::Limites::default(),
    };
    let revision = revisiones::publicar_revision(
        revisiones::preparar_revision(&contexto, &borrador, &cancel).unwrap(),
    )
    .unwrap();
    let compartiendo = c::anfitrion::PublicacionActiva::iniciar(
        host.clone(),
        revision,
        &root.join("accesos.json"),
        url,
        Modo::Local,
        c::Limites::default(),
    )
    .await
    .unwrap();
    let encuentro = compartiendo.encuentro().unwrap();
    let direccion = encuentro
        .direccion(&host.clave_publica(), publicacion)
        .unwrap();
    let cliente = c::transporte::Cliente::conectar(
        guest,
        direccion,
        &contexto.origen,
        Modo::Local,
        c::Limites::default(),
    )
    .await
    .unwrap();
    let remoto = cliente.consultar().await.unwrap();
    let store = icaro_core::instancias::Almacen::nuevo(root.join("instancias"));
    let nueva =
        c::vinculos::crear_destino(&store, "Recibida", &remoto, &c::Limites::default()).unwrap();
    let destino = c::aplicacion::ContextoDestino {
        juego: store.juego(&nueva.id).unwrap(),
        limites: c::Limites::default(),
    };
    fs::write(
        destino.juego.join("options.txt"),
        b"fullscreen:true\r\nresourcePacks:[\"vanilla\"]\r\n",
    )
    .unwrap();
    let inv = c::inventario::analizar(&destino.juego, &cancel, &destino.limites).unwrap();
    let plan = c::plan::planificar(&remoto, &Default::default(), &inv).unwrap();
    let preparada = c::descargas::preparar(
        &destino,
        plan,
        &cliente,
        None,
        &root.join("staging"),
        &cancel,
    )
    .await
    .unwrap();
    assert!(!destino.juego.join("config/mod.toml").exists());
    c::aplicacion::aplicar(preparada, &cancel, |_| {}).unwrap();
    assert_eq!(
        fs::read(destino.juego.join("resourcepacks/texturas.zip")).unwrap(),
        b"pack compartido"
    );
    assert!(fs::read_to_string(destino.juego.join("options.txt"))
        .unwrap()
        .contains("fullscreen:true\r\nresourcePacks:[\"vanilla\",\"file/texturas.zip\"]"));
    // El anfitrión revoca primero el transporte local y después el servicio.
    compartiendo.revocar(&guest.clave_publica()).await.unwrap();
    assert!(cliente.consultar().await.is_err());
    cliente.cerrar().await;
    compartiendo.cerrar().await;
    drop(compartiendo);
    if root.parent() == Some(std::env::temp_dir().as_path())
        && root
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("icaro-recorrido-")
    {
        fs::remove_dir_all(root).unwrap();
    }
}
