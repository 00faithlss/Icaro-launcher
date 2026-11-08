// SPDX-License-Identifier: GPL-3.0-only
//! Recorrido real del publicador contra el Worker/D1 local. Ignorada: requiere
//! levantar el Worker de `services/invitaciones` y exportar `ICARO_PRUEBA_WORKER_URL`
//! con una dirección loopback. No usa el almacén seguro del sistema ni `APPDATA`.
use super::{acciones, resultados, servicio::Servicio, tareas, Estado, Etapa};
use icaro_core::{
    compartir::{
        identidad::{AlmacenClave, Identidad, Zeroizing},
        invitaciones::{ClienteInvitaciones, Codigo},
        publicacion::{Eleccion, Seleccion},
        revisiones,
        transporte::Modo,
        Error, RutaCompartida,
    },
    instancias::Almacen,
};
use icaro_ui::compartir as v;
use std::{
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    path::PathBuf,
    sync::Mutex,
};
#[path = "pruebas_invitado_worker.rs"]
mod invitado;

/// Clave solo en memoria: nunca toca el keyring del equipo.
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

/// Exige `http://127.0.0.1:<puerto>` o `http://[::1]:<puerto>`, sin ruta.
fn url_worker() -> String {
    let url = std::env::var("ICARO_PRUEBA_WORKER_URL").unwrap_or_else(|_| {
        panic!(
            "Falta ICARO_PRUEBA_WORKER_URL. Levanta el Worker local de services/invitaciones \
             y exporta, por ejemplo, ICARO_PRUEBA_WORKER_URL=http://127.0.0.1:8787"
        )
    });
    let resto = url.strip_prefix("http://").unwrap_or_else(|| {
        panic!("ICARO_PRUEBA_WORKER_URL debe usar http:// sobre loopback; recibido: {url}")
    });
    let autoridad = resto.strip_suffix('/').unwrap_or(resto);
    let direccion: SocketAddr = autoridad.parse().unwrap_or_else(|_| {
        panic!(
            "ICARO_PRUEBA_WORKER_URL debe ser http://127.0.0.1:<puerto> o \
             http://[::1]:<puerto>, sin ruta; recibido: {url}"
        )
    });
    assert!(
        matches!(direccion.ip(), IpAddr::V4(ip) if ip == Ipv4Addr::LOCALHOST)
            || matches!(direccion.ip(), IpAddr::V6(ip) if ip == Ipv6Addr::LOCALHOST),
        "ICARO_PRUEBA_WORKER_URL debe apuntar a loopback (127.0.0.1 o ::1); recibido: {url}"
    );
    format!("http://{direccion}/")
}

/// Último recurso si una aserción falla: los nodos ya habrán caído con el panic.
struct Carpeta(PathBuf);
impl Drop for Carpeta {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
#[ignore = "requiere el Worker/D1 local y ICARO_PRUEBA_WORKER_URL (loopback)"]
async fn publicador_real_publica_seleccion_y_el_invitado_resuelve_el_codigo() {
    let url = url_worker();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let carpeta = Carpeta(std::env::temp_dir().join(format!(
        "icaro-publicador-worker-{}-{nonce}",
        std::process::id()
    )));
    std::fs::create_dir(&carpeta.0).unwrap();
    let raiz = carpeta.0.join("datos");
    let almacen = Almacen::nuevo(raiz.join("instances"));
    let instancia = almacen.crear("Publicada", "1.21.1", 0).unwrap();
    let juego = almacen.juego(&instancia.id).unwrap();
    let archivos: [(&str, &[u8]); 4] = [
        ("config/compartida.toml", b"valor = 1\n"),
        ("config/privada.toml", b"secreto = \"solo este equipo\"\n"),
        ("mods/propio.jar", b"PK\x03\x04 jar propio de prueba"),
        (
            "resourcepacks/texturas.zip",
            b"PK\x03\x04 pack externo de prueba",
        ),
    ];
    for (ruta, datos) in archivos {
        let path = juego.join(ruta);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, datos).unwrap();
    }
    let opciones = b"fullscreen:true\r\nresourcePacks:[\"vanilla\",\"file/texturas.zip\"]\r\n";
    std::fs::write(juego.join("options.txt"), opciones).unwrap();

    // Inventario real: hashes calculados sobre los archivos, sin catálogo ni keyring.
    let anfitrion = identidad();
    let mut s = Estado::default();
    let id = s.comenzar(instancia.id.clone());
    let analizado = tareas::analizar_con(
        raiz.clone(),
        almacen.clone(),
        instancia.clone(),
        Some(anfitrion.clone()),
        None,
        s.cancel.clone(),
    )
    .await;
    resultados::analizada(&mut s, id, analizado);
    assert_eq!(s.etapa, Etapa::Revision, "{:?}", s.error);
    let t = s.trabajo.as_mut().unwrap();
    let rutas: Vec<_> = t
        .inventario
        .archivos
        .iter()
        .map(|a| a.ruta.as_str())
        .collect();
    assert_eq!(
        rutas,
        [
            "config/compartida.toml",
            "config/privada.toml",
            "mods/propio.jar",
            "resourcepacks/texturas.zip"
        ]
    );

    // Selección con las mismas operaciones que usa la interfaz.
    t.elegir("config/compartida.toml", Eleccion::Compartir)
        .unwrap();
    t.elegir("config/privada.toml", Eleccion::Privado).unwrap();
    t.elegir("mods/propio.jar", Eleccion::Compartir).unwrap();
    t.elegir("resourcepacks/texturas.zip", Eleccion::Compartir)
        .unwrap();
    let pendientes: Vec<_> = t
        .borrador
        .autorizacion_pendiente()
        .into_iter()
        .map(|a| a.ruta.as_str().to_owned())
        .collect();
    assert_eq!(
        pendientes,
        ["mods/propio.jar", "resourcepacks/texturas.zip"]
    );
    t.borrador.autorizar_propios();
    assert!(t.borrador.autorizacion_pendiente().is_empty());

    // Publicación: revisión firmada, nodo iroh local y servicio inyectado.
    let trabajo = s.trabajo.clone().unwrap();
    s.etapa = Etapa::Publicando;
    let servicio = Servicio {
        url: url.clone(),
        modo: Modo::Local,
    };
    let publicada = tareas::publicar_con(trabajo, None, Some(servicio), s.cancel.clone())
        .await
        .expect("publicar la revisión contra el Worker local");
    if let Err(e) = &publicada.anfitrion {
        panic!("la revisión se guardó pero el anfitrión no inició: {e}");
    }
    assert!(
        resultados::publicada(&mut s, id, Ok(publicada)),
        "una publicación activa debe pedir la invitación"
    );
    assert_eq!(s.etapa, Etapa::Panel);
    let local = s.publicacion.clone().unwrap();
    let revision = revisiones::vigente(&local.contexto).unwrap().unwrap();
    let publicadas: Vec<_> = revision
        .manifiesto
        .archivos
        .iter()
        .map(|a| a.ruta.as_str())
        .collect();
    assert_eq!(
        publicadas,
        [
            "config/compartida.toml",
            "mods/propio.jar",
            "resourcepacks/texturas.zip"
        ]
    );
    assert_eq!(revision.manifiesto.packs, ["vanilla", "file/texturas.zip"]);
    assert_eq!(std::fs::read(juego.join("options.txt")).unwrap(), opciones);
    for a in &revision.manifiesto.archivos {
        assert_eq!(
            std::fs::read(local.contexto.almacen.join("contenido").join(&a.sha512)).unwrap(),
            std::fs::read(juego.join(a.ruta.as_str())).unwrap(),
            "el contenido publicado debe ser el archivo real: {}",
            a.ruta.as_str()
        );
    }
    assert!(Seleccion::cargar(&juego)
        .unwrap()
        .es_privado(&RutaCompartida::parse("config/privada.toml").unwrap()));

    // Código: el mismo futuro que lanza la interfaz, aplicado al panel.
    let (solicitud, futuro) =
        acciones::iniciar_invitacion(&mut s).expect("el panel activo debe invitar");
    assert!(s.red_ocupada);
    let creada = futuro
        .await
        .expect("crear la invitación en el Worker local");
    resultados::invitacion(&mut s, solicitud, Ok(creada.clone()));
    assert!(!s.red_ocupada);
    let panel = s.panel.as_ref().unwrap();
    assert!(matches!(panel.estado, v::EstadoPublicacion::Compartiendo));
    assert_eq!(panel.instancia.id, instancia.id);
    assert_eq!(panel.revision, 1);
    assert_eq!(panel.contenido_privado, 1);
    assert_eq!(panel.huella, anfitrion.huella());
    let inv = panel.invitacion.as_ref().unwrap();
    assert!(matches!(inv.estado, v::EstadoInvitacion::Vigente { .. }));
    assert_eq!((inv.cupo, inv.aceptados), (s.cupo, 0));
    let codigo = Codigo::parse(&inv.codigo).expect("el panel muestra un código válido");
    assert_eq!(inv.codigo, codigo.mostrar());
    assert_eq!(
        codigo.as_str(),
        Codigo::parse(&creada.code).unwrap().as_str()
    );

    // Invitado: otra identidad resuelve el código mostrado sin aceptarlo.
    let invitado = ClienteInvitaciones::nuevo(&url, identidad()).unwrap();
    let resuelta = invitado
        .resolver(&codigo)
        .await
        .expect("el invitado resuelve el código en el Worker local");
    assert_eq!(resuelta.host, anfitrion.clave_publica());
    assert_eq!(resuelta.publication, local.contexto.origen.publicacion);
    assert_eq!((resuelta.capacity, resuelta.accepted), (s.cupo, 0));
    assert!(!resuelta.authorized);

    // Aceptar consume un cupo; editar el mismo código no publica otra revisión.
    assert!(invitado.aceptar(&codigo).await.unwrap().authorized);
    let ajustada = super::invitaciones::ajustar_con(
        anfitrion.clone(),
        codigo.clone(),
        v::Caducidad::Dias2,
        3,
        Some(Servicio {
            url: url.clone(),
            modo: Modo::Local,
        }),
    )
    .await
    .unwrap();
    super::invitaciones::ajustada(&mut s, id, v::Caducidad::Dias2, Ok(ajustada));
    let inv = s.panel.as_ref().unwrap().invitacion.as_ref().unwrap();
    assert_eq!((inv.cupo, inv.aceptados), (3, 1));
    assert_eq!(inv.caducidad, v::Caducidad::Dias2);
    assert_eq!(
        Codigo::parse(&inv.codigo).unwrap().as_str(),
        codigo.as_str()
    );
    assert_eq!(
        revisiones::vigente(&local.contexto)
            .unwrap()
            .unwrap()
            .manifiesto,
        revision.manifiesto
    );
    assert_eq!(invitado.resolver(&codigo).await.unwrap().capacity, 3);

    // Detener y reabrir conserva origen/revisión, pero exige nueva aceptación.
    let h = s.anfitrion.take().unwrap();
    h.detener()
        .await
        .expect("detener la publicación en el Worker local");
    drop(h);
    resultados::operada(&mut s, id, super::Operacion::Detener, Ok(()));
    assert!(invitado.resolver(&codigo).await.is_err());
    let (nuevo, creada) = super::invitaciones::reanudar_con(
        local.clone(),
        v::Caducidad::Dias2,
        3,
        Some(Servicio {
            url: url.clone(),
            modo: Modo::Local,
        }),
    )
    .await
    .unwrap();
    let codigo_nuevo = Codigo::parse(&creada.code).unwrap();
    assert_ne!(codigo_nuevo.as_str(), codigo.as_str());
    assert_eq!(creada.accepted, 0);
    assert!(nuevo.accesos().await.unwrap().is_empty());
    assert!(invitado.resolver(&codigo).await.is_err());
    let resuelta = invitado.resolver(&codigo_nuevo).await.unwrap();
    assert!(!resuelta.authorized);
    assert_eq!(resuelta.publication, local.contexto.origen.publicacion);
    assert_eq!(resuelta.host, anfitrion.clave_publica());
    assert!(invitado.aceptar(&codigo_nuevo).await.unwrap().authorized);
    assert_eq!(
        revisiones::vigente(&local.contexto)
            .unwrap()
            .unwrap()
            .manifiesto,
        revision.manifiesto
    );
    invitado::comprobar(&url, &codigo_nuevo, &carpeta.0).await;
    nuevo.detener().await.unwrap();
    drop(nuevo);
    drop(s);
    std::fs::remove_dir_all(&carpeta.0).expect("limpiar la carpeta temporal de la prueba");
}
