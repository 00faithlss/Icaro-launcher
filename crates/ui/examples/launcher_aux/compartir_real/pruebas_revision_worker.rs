// SPDX-License-Identifier: GPL-3.0-only
//! RevisionPublicacion por tipo de punta a punta: instancia real con JAR válidos,
//! acciones de la interfaz por el controlador, publicación contra el Worker local
//! y una segunda revisión con un mod actualizado y otro eliminado. Ignorada como
//! el resto de recorridos Worker; usa `APPDATA` y keyring solo si el entorno lo hace.
use super::super::{resultados, tareas};
use super::{identidad, url_worker, Carpeta};
use crate::compartir_real::{actualizar, Accion, Etapa};
use icaro_core::{
    compartir::{revisiones, transporte::Modo},
    instancias::Almacen,
};
use icaro_ui::compartir as v;

use super::super::servicio::Servicio;

fn crc32(datos: &[u8]) -> u32 {
    let mut c = !0u32;
    for &b in datos {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 {
                (c >> 1) ^ 0xEDB8_8320
            } else {
                c >> 1
            };
        }
    }
    !c
}

/// JAR mínimo sin compresión (ZIP «stored») con CRC correcto.
fn jar(entradas: &[(&str, &str)]) -> Vec<u8> {
    let (hora, fecha) = (0u16, ((2026u16 - 1980) << 9) | (11 << 5) | 9);
    let (mut out, mut central) = (Vec::new(), Vec::new());
    for (nombre, datos) in entradas {
        let (n, d) = (nombre.as_bytes(), datos.as_bytes());
        let (crc, largo, inicio) = (crc32(d), d.len() as u32, out.len() as u32);
        out.extend(0x0403_4b50u32.to_le_bytes());
        for x in [20u16, 0, 0, hora, fecha] {
            out.extend(x.to_le_bytes());
        }
        for x in [crc, largo, largo] {
            out.extend(x.to_le_bytes());
        }
        out.extend((n.len() as u16).to_le_bytes());
        out.extend(0u16.to_le_bytes());
        out.extend(n);
        out.extend(d);
        central.extend(0x0201_4b50u32.to_le_bytes());
        for x in [20u16, 20, 0, 0, hora, fecha] {
            central.extend(x.to_le_bytes());
        }
        for x in [crc, largo, largo] {
            central.extend(x.to_le_bytes());
        }
        for x in [n.len() as u16, 0, 0, 0, 0] {
            central.extend(x.to_le_bytes());
        }
        central.extend(0u32.to_le_bytes());
        central.extend(inicio.to_le_bytes());
        central.extend(n);
    }
    let (inicio, largo, total) = (
        out.len() as u32,
        central.len() as u32,
        entradas.len() as u16,
    );
    out.extend(central);
    out.extend(0x0605_4b50u32.to_le_bytes());
    for x in [0u16, 0, total, total] {
        out.extend(x.to_le_bytes());
    }
    out.extend(largo.to_le_bytes());
    out.extend(inicio.to_le_bytes());
    out.extend(0u16.to_le_bytes());
    out
}

fn fabric(id: &str, nombre: &str, version: &str, entorno: &str) -> Vec<u8> {
    jar(&[(
        "fabric.mod.json",
        &format!(
            r#"{{"schemaVersion":1,"id":"{id}","name":"{nombre}","version":"{version}","environment":"{entorno}"}}"#
        ),
    )])
}

fn app() -> crate::App {
    let (mut a, _) = crate::iniciar_con_guardado(
        icaro_ui::shell::Seccion::Servidores,
        true,
        None,
        false,
        crate::servicios::Estado::nuevo(true),
        None,
    );
    a.servicios.demo = false;
    a.servicios.compartir.habilitado = true;
    a
}

fn vista(a: &mut crate::App, accion: v::Accion) {
    let _ = actualizar(a, Accion::Vista(accion));
}

/// Simula lo que hace iced con la tarea de guardado que devolvió la acción.
async fn guardar(a: &mut crate::App, id: u64, confirmar: bool) {
    assert_eq!(a.servicios.compartir.etapa, Etapa::Guardando);
    let t = a.servicios.compartir.trabajo.clone().unwrap();
    let r = tareas::guardar_seleccion(t).await;
    let _ = actualizar(a, Accion::Guardada(id, confirmar, r));
    assert_eq!(a.servicios.compartir.etapa, Etapa::Revision);
}

fn fila<'a>(a: &'a crate::App, id: &str) -> &'a v::CambioPublicacion {
    a.servicios
        .compartir
        .revision
        .as_ref()
        .unwrap()
        .cambios
        .iter()
        .find(|c| c.archivo.id == id)
        .unwrap_or_else(|| panic!("falta la fila {id}"))
}

#[test]
#[ignore = "requiere el Worker/D1 local y ICARO_PRUEBA_WORKER_URL (loopback)"]
fn revision_por_tipo_publica_lo_marcado_y_la_segunda_revision_compara_versiones() {
    std::thread::Builder::new()
        .name("icaro-e2e-revision".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(recorrer())
        })
        .unwrap()
        .join()
        .expect("recorrido de la revisión por tipo");
}

async fn recorrer() {
    let url = url_worker();
    let servicio = || Servicio {
        url: url.clone(),
        modo: Modo::Local,
    };
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let carpeta = Carpeta(std::env::temp_dir().join(format!(
        "icaro-revision-worker-{}-{nonce}",
        std::process::id()
    )));
    std::fs::create_dir(&carpeta.0).unwrap();
    let raiz = carpeta.0.join("datos");
    let almacen = Almacen::nuevo(raiz.join("instances"));
    let instancia = almacen.crear("Amigos", "1.21.1", 0).unwrap();
    let juego = almacen.juego(&instancia.id).unwrap();
    let escribir = |ruta: &str, datos: &[u8]| {
        let path = juego.join(ruta);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, datos).unwrap();
    };
    escribir(
        "mods/sodium-fabric-0.6.5.jar",
        &fabric("sodium", "Sodium", "0.6.5", "client"),
    );
    escribir(
        "mods/waystones-fabric-21.1.4.jar",
        &fabric("waystones", "Waystones", "21.1.4", "*"),
    );
    escribir(
        "mods/boys-banners-1.0.2.jar",
        &fabric("boysbanners", "Boys Banners", "1.0.2", "*"),
    );
    escribir("config/sodium-options.json", b"{\"quality\":2}\n");
    escribir("config/waystones-common.toml", b"cost = 1\n");
    escribir("config/defaultoptions/options.txt", b"fov:90\n");
    escribir("kubejs/server_scripts/recetas.js", b"// recetas\n");
    escribir("kubejs/client_scripts/tooltips.js", b"// tooltips\n");
    escribir("resourcepacks/texturas.zip", b"PK\x03\x04 pack de prueba");
    std::fs::write(
        juego.join("options.txt"),
        b"resourcePacks:[\"vanilla\",\"file/texturas.zip\"]\n",
    )
    .unwrap();

    // 1. Analizar como la interfaz: inventario y metadatos reales de los JAR.
    let anfitrion = identidad();
    let mut a = app();
    let id = a.servicios.compartir.comenzar(instancia.id.clone());
    let r = tareas::analizar_con(
        raiz.clone(),
        almacen.clone(),
        instancia.clone(),
        Some(anfitrion.clone()),
        None,
        a.servicios.compartir.cancel.clone(),
    )
    .await;
    let _ = actualizar(&mut a, Accion::Analizada(id, r));
    assert_eq!(
        a.servicios.compartir.etapa,
        Etapa::Revision,
        "{:?}",
        a.servicios.compartir.error
    );
    {
        let r = a.servicios.compartir.revision.as_ref().unwrap();
        assert!(matches!(r.estado, v::EstadoRevisionPublicacion::Disponible));
        assert_eq!((r.base, r.nueva), (0, 1));
        assert!(r
            .cambios
            .iter()
            .filter(|c| c.archivo.id != "@orden-resourcepacks")
            .all(|c| c.grupo == v::GrupoPublicacion::Nuevo && !c.marcado()));
        // La activación de `vanilla` es un cambio de orden ya marcado, como antes.
        let orden = r
            .cambios
            .iter()
            .find(|c| c.archivo.id == "@orden-resourcepacks")
            .unwrap();
        assert_eq!(
            (orden.grupo, orden.tipo, orden.marcado()),
            (
                v::GrupoPublicacion::Cambios,
                v::TipoContenido::Resourcepack,
                true
            )
        );
    }
    let sodium = fila(&a, "mods/sodium-fabric-0.6.5.jar");
    assert_eq!(
        (
            sodium.archivo.nombre.as_str(),
            sodium.version.as_str(),
            sodium.solo_cliente,
            sodium.tipo
        ),
        ("Sodium", "0.6.5", true, v::TipoContenido::Mod)
    );
    assert!(
        sodium.archivo.tamano.ends_with(" B"),
        "{}",
        sodium.archivo.tamano
    );
    assert_eq!(
        fila(&a, "config/sodium-options.json").de_mod,
        Some(("mods/sodium-fabric-0.6.5.jar".into(), "Sodium".into()))
    );
    assert_eq!(
        fila(&a, "config/waystones-common.toml")
            .de_mod
            .as_ref()
            .map(|m| m.1.as_str()),
        Some("Waystones")
    );
    assert_eq!(fila(&a, "config/defaultoptions/options.txt").de_mod, None);
    assert!(fila(&a, "kubejs/client_scripts/tooltips.js").solo_cliente);
    assert!(!fila(&a, "kubejs/server_scripts/recetas.js").solo_cliente);
    assert_eq!(
        fila(&a, "resourcepacks/texturas.zip").tipo,
        v::TipoContenido::Resourcepack
    );

    // 2. Estado de interfaz: no toca el borrador ni se pierde al recalcular.
    vista(&mut a, v::Accion::BuscarPublicacion("way".into()));
    vista(&mut a, v::Accion::AlternarTipo(v::TipoContenido::Config));
    assert_eq!(
        a.servicios.compartir.etapa,
        Etapa::Revision,
        "filtrar no guarda"
    );

    // 3. Atajo recomendado: deja fuera lo de cliente y sus ajustes.
    vista(
        &mut a,
        v::Accion::AtajoPublicacion(v::AtajoPublicacion::Necesario),
    );
    guardar(&mut a, id, false).await;
    let marcados: Vec<_> = a
        .servicios
        .compartir
        .revision
        .as_ref()
        .unwrap()
        .cambios
        .iter()
        .filter(|c| c.marcado())
        .map(|c| c.archivo.id.as_str())
        .collect();
    assert_eq!(
        marcados,
        [
            "config/defaultoptions/options.txt",
            "config/waystones-common.toml",
            "kubejs/server_scripts/recetas.js",
            "mods/boys-banners-1.0.2.jar",
            "mods/waystones-fabric-21.1.4.jar",
            "@orden-resourcepacks",
        ]
    );
    {
        let r = a.servicios.compartir.revision.as_ref().unwrap();
        assert_eq!(r.vista.busqueda, "way");
        assert!(r.vista.abiertos.contains(&v::TipoContenido::Config));
        assert_eq!(
            r.vista.aplicado,
            "Listo: 2 mods, 2 configuraciones, 1 cambio de resourcepacks y 1 script. Ajusta abajo lo que quieras."
        );
    }

    // 4. Marcar un mod arrastra su configuración; desmarcarlo la devuelve a privado.
    vista(
        &mut a,
        v::Accion::MarcarPublicacion("mods/sodium-fabric-0.6.5.jar".into(), true),
    );
    guardar(&mut a, id, false).await;
    assert!(fila(&a, "config/sodium-options.json").marcado());
    vista(
        &mut a,
        v::Accion::MarcarPublicacion("mods/sodium-fabric-0.6.5.jar".into(), false),
    );
    guardar(&mut a, id, false).await;
    assert!(!fila(&a, "config/sodium-options.json").marcado());

    // 5. Cancelar con cambios confirma en el pie; seguir revisando vuelve.
    vista(&mut a, v::Accion::Cancelar);
    guardar(&mut a, id, true).await;
    assert!(
        a.servicios
            .compartir
            .revision
            .as_ref()
            .unwrap()
            .confirmar_cancelacion
    );
    vista(&mut a, v::Accion::SeguirRevisando);
    assert!(
        !a.servicios
            .compartir
            .revision
            .as_ref()
            .unwrap()
            .confirmar_cancelacion
    );

    // 6. Publicar: primero el permiso para archivos propios (no hay catálogo).
    vista(&mut a, v::Accion::PublicarRevision);
    assert_eq!(a.servicios.compartir.etapa, Etapa::Propios);
    vista(&mut a, v::Accion::PermisoPropios(true));
    vista(&mut a, v::Accion::AutorizarPropios);
    assert_eq!(a.servicios.compartir.etapa, Etapa::Revision);
    assert_eq!(
        a.servicios
            .compartir
            .revision
            .as_ref()
            .unwrap()
            .vista
            .busqueda,
        "way",
        "autorizar no reinicia la vista"
    );
    let t = a.servicios.compartir.trabajo.clone().unwrap();
    a.servicios.compartir.etapa = Etapa::Publicando;
    let publicada = tareas::publicar_con(
        t,
        None,
        Some(servicio()),
        a.servicios.compartir.cancel.clone(),
    )
    .await
    .expect("publicar la revisión 1 contra el Worker local");
    assert!(
        publicada.anfitrion.is_ok(),
        "{:?}",
        publicada.anfitrion.as_ref().err()
    );
    resultados::publicada(&mut a.servicios.compartir, id, Ok(publicada));
    assert_eq!(a.servicios.compartir.etapa, Etapa::Panel);
    let local = a.servicios.compartir.publicacion.clone().unwrap();
    let r1 = revisiones::vigente(&local.contexto).unwrap().unwrap();
    let rutas = |r: &revisiones::Revision| -> Vec<String> {
        r.manifiesto
            .archivos
            .iter()
            .map(|x| x.ruta.as_str().to_owned())
            .collect()
    };
    assert_eq!(r1.manifiesto.revision, 1);
    assert_eq!(
        rutas(&r1),
        [
            "config/defaultoptions/options.txt",
            "config/waystones-common.toml",
            "kubejs/server_scripts/recetas.js",
            "mods/boys-banners-1.0.2.jar",
            "mods/waystones-fabric-21.1.4.jar",
        ]
    );
    let waystones_1 = r1
        .manifiesto
        .archivos
        .iter()
        .find(|x| x.ruta.as_str() == "mods/waystones-fabric-21.1.4.jar")
        .unwrap()
        .sha512
        .clone();

    // 7. Segunda revisión: Waystones se actualiza en el mismo archivo y Boys Banners se elimina.
    escribir(
        "mods/waystones-fabric-21.1.4.jar",
        &fabric("waystones", "Waystones", "21.1.5", "*"),
    );
    std::fs::remove_file(juego.join("mods/boys-banners-1.0.2.jar")).unwrap();
    let id = a.servicios.compartir.comenzar(instancia.id.clone());
    let r = tareas::analizar_con(
        raiz.clone(),
        almacen.clone(),
        instancia.clone(),
        Some(anfitrion.clone()),
        None,
        a.servicios.compartir.cancel.clone(),
    )
    .await;
    let _ = actualizar(&mut a, Accion::Analizada(id, r));
    assert_eq!(
        a.servicios.compartir.etapa,
        Etapa::Revision,
        "{:?}",
        a.servicios.compartir.error
    );
    assert_eq!(
        {
            let r = a.servicios.compartir.revision.as_ref().unwrap();
            (r.base, r.nueva)
        },
        (1, 2)
    );
    let w = fila(&a, "mods/waystones-fabric-21.1.4.jar");
    assert_eq!(
        (
            w.grupo,
            w.version.as_str(),
            w.version_publicada.as_str(),
            w.marcado()
        ),
        (
            v::GrupoPublicacion::Actualizaciones,
            "21.1.4 → 21.1.5",
            "21.1.4",
            true
        )
    );
    let b = fila(&a, "mods/boys-banners-1.0.2.jar");
    assert_eq!(
        (
            b.grupo,
            b.archivo.nombre.as_str(),
            b.estado.as_str(),
            b.marcado()
        ),
        (
            v::GrupoPublicacion::Retiradas,
            "Boys Banners",
            "Lo eliminaste de tu instancia",
            true
        )
    );
    for privado in ["mods/sodium-fabric-0.6.5.jar", "config/sodium-options.json"] {
        let c = fila(&a, privado);
        assert_eq!(
            (c.grupo, c.marcado()),
            (v::GrupoPublicacion::Privado, false),
            "{privado}"
        );
    }

    // Posponer la actualización: los invitados siguen con la 21.1.4.
    vista(
        &mut a,
        v::Accion::MarcarPublicacion("mods/waystones-fabric-21.1.4.jar".into(), false),
    );
    guardar(&mut a, id, false).await;
    let t = a.servicios.compartir.trabajo.clone().unwrap();
    assert!(
        t.borrador.autorizacion_pendiente().is_empty(),
        "lo pospuesto no pide permiso"
    );
    a.servicios.compartir.etapa = Etapa::Publicando;
    let h = a.servicios.compartir.anfitrion.clone();
    let publicada =
        tareas::publicar_con(t, h, Some(servicio()), a.servicios.compartir.cancel.clone())
            .await
            .expect("publicar la revisión 2");
    assert!(publicada.anfitrion.is_ok());
    resultados::publicada(&mut a.servicios.compartir, id, Ok(publicada));
    let r2 = revisiones::vigente(&local.contexto).unwrap().unwrap();
    assert_eq!(r2.manifiesto.revision, 2);
    assert!(
        !rutas(&r2).iter().any(|r| r.contains("boys-banners")),
        "la retirada se publica"
    );
    assert_eq!(
        r2.manifiesto
            .archivos
            .iter()
            .find(|x| x.ruta.as_str() == "mods/waystones-fabric-21.1.4.jar")
            .unwrap()
            .sha512,
        waystones_1,
        "lo pospuesto conserva el contenido publicado"
    );

    // 8. Tercera apertura sin publicar nada nuevo: solo lo privado y lo pospuesto.
    let id = a.servicios.compartir.comenzar(instancia.id.clone());
    let r = tareas::analizar_con(
        raiz.clone(),
        almacen.clone(),
        instancia.clone(),
        Some(anfitrion.clone()),
        None,
        a.servicios.compartir.cancel.clone(),
    )
    .await;
    let _ = actualizar(&mut a, Accion::Analizada(id, r));
    let r = a.servicios.compartir.revision.as_ref().unwrap();
    assert_eq!((r.base, r.nueva), (2, 3));
    assert!(r.cambios.iter().all(|c| matches!(
        c.grupo,
        v::GrupoPublicacion::Privado | v::GrupoPublicacion::Actualizaciones
    )));

    if let Some(h) = a.servicios.compartir.anfitrion.take() {
        // Sin invitación creada, el servicio no conoce la publicación y responde 404;
        // lo que importa aquí es apagar el nodo local.
        let _ = h.detener().await;
    }
    drop(a);
    std::fs::remove_dir_all(&carpeta.0).expect("limpiar la carpeta temporal de la prueba");
}
