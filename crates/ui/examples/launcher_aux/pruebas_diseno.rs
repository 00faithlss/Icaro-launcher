// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss
//! Capturas de widgets reales con fixtures; sin login/red/guardado de usuario.
use crate::*;
pub fn iniciar(piedra: bool) -> (App, Task<Mensaje>) {
    let piedra=std::env::var("ICARO_TEMA_DISENO").map_or(piedra,|v|v=="piedra");
    let (mut a, _) = iniciar_con_guardado(
        Seccion::Instancias,
        piedra,
        None,
        false,
        servicios::Estado::nuevo(true),
        None,
    );
    a.reducir = true;
    a.modo=if piedra {Modo::Piedra}else{Modo::Tinta};a.tema=usize::from(piedra);
    a.pantalla = None;
    a.umbral = None;
    let scene = std::env::var("ICARO_VISTA_DISENO").unwrap_or_else(|_| "asistente".into());
    match scene.as_str() {
        "compartir-cierre-real" | "compartir-cierre-real-angosto" => {
            crate::servidores_reales::preparar_diseno(&mut a, "servidores");
            a.cierre.solicitado = true;
            a.cierre.transferencias = 2;
        },
        s if s.starts_with("compartir-controlador-invitado")=>crate::compartir_real::preparar_invitado_diseno(&mut a,s),
        s if s.starts_with("compartir-controlador")=>crate::compartir_diseno::preparar_controlador(&mut a),
        s if s.starts_with("skin-local")=>crate::skin_sin_conexion::preparar_diseno(&mut a,s),
        s if s.starts_with("cuenta-local") => crate::cuenta_sin_conexion::preparar_diseno(&mut a,s),
        s if s.starts_with("importar-local") => crate::modpacks_reales::preparar_diseno(&mut a,s),
        "cuenta-original" => {a.pantalla=Some(pantallas::Pantalla::Cuenta);a.sesion.estado=icaro_ui::cuenta::EstadoSesion::Activa;},
        s if s.starts_with("estudio-original") => {a.pantalla=Some(pantallas::Pantalla::Estudio);a.sesion.estado=icaro_ui::cuenta::EstadoSesion::Activa;},
        s if s.starts_with("modpacks-") => preparar_modpacks(&mut a, s),
        s if s.starts_with("sincronizacion-")=>crate::sincronizacion_real::preparar_diseno(&mut a,s),
        s if s.starts_with("red-privada")=>{crate::servidores_reales::preparar_diseno(&mut a,"servidores");crate::red_privada_real::preparar_diseno(&mut a,s);},
        s if s.starts_with("servidores")=>crate::servidores_reales::preparar_diseno(&mut a,s),
        "java-instancia" => { a.editor = Some(1); a.editor_inst = 0; },
        "almacenamiento" => { a.seccion=Seccion::Ajustes;a.ajuste=3;a.almacenamiento.informe=Some(icaro_core::almacenamiento::Informe{categorias:vec![("Assets".into(),484486173),("Bibliotecas".into(),80695893),("Java".into(),250000000),("Instancias".into(),2000000),("Caché".into(),1000000)],total:818182066,papeleras:vec![std::path::PathBuf::from("fixture/.papelera")]}); },
        "java-global" => { a.seccion = Seccion::Ajustes; a.ajuste = 2; },
        "acerca-de" => { a.seccion = Seccion::Ajustes; a.ajuste = 7; },
        s if s.starts_with("exportar-lista-") => crate::exportar_lista_mods::preparar_diseno(&mut a,s),
        "icono-generado" | "icono-imagen" => crate::icono_instancia::preparar_diseno(&mut a,&scene),
        "exportar-lista-html" | "exportar-lista-json" | "exportar-lista-personalizado" => crate::exportar_lista_mods::preparar_diseno(&mut a,&scene),
        "noverif" => crate::verificacion_real::preparar_diseno(&mut a),
        "actualizacion" => crate::actualizacion_real::preparar_diseno(&mut a),
        "actualizacion-al-cerrar" => crate::actualizacion_real::preparar_diseno_al_cerrar(&mut a),
        s if s.starts_with("actualizacion-") => crate::actualizacion_real::preparar_diseno_estado(&mut a,s),
        s if s.starts_with("recursos-") || s.starts_with("shaders-") => {
            crate::mods_reales::preparar_diseno(&mut a, &scene)
        }
        s if s.starts_with("juego-") => crate::juego_real::preparar_diseno(&mut a, &scene),
        "asistente" | "asistente-version" | "asistente-loader" => {
            a.asistente = Some(pantallas::Asistente {
                paso: if scene == "asistente-version" { 0 } else { 1 },
                tipo_version: 0,
                busqueda: String::new(),
                version: 0,
                loader: 1,
                nombre: String::new(),
                portada: 0,
            })
        }
        "cambiar-loader" => {
            preparar_cambio(&mut a,"cambiar-version");
            let mc=a.servicios.instancias.registros[0].minecraft.clone();
            let c=a.servicios.instancias.cambio.as_mut().unwrap();
            c.solo_loader=true;c.destino=mc.clone();c.loader_version_destino=Some("0.16.8".into());c.confirmando=false;
            a.servicios.instancias.loaders.get_mut(&mc).unwrap().versiones[1].push("0.16.8".into());
        }
        s if s.starts_with("cambiar-version") => preparar_cambio(&mut a, &scene),
        "exportar" => {
            a.exportar = Some(pantallas::Exportar {
                instancia: 0,
                nombre: "Supervivencia".into(),
                formato: 0,
                destino: "C:\\Users\\Mineral\\Descargas".into(),
                error: None,
                guardando: false,
                arbol: Some(pantallas::exportacion_nueva::demo()),
                preset: 0,
                hechos: 0,
                total: 0,
            })
        }
        "archivos" | "archivos-renombrar" => {
            a.editor = Some(7);
            a.editor_inst = 0;
            a.exp = Explorador::abrir("qa-diseno");
            a.exp.elegir(1);
            if scene == "archivos-renombrar" {
                a.exp.iniciar_renombrar();
            }
        }
        _ => preparar_mods(&mut a, &scene),
    }
    let cambiar_versiones = scene.starts_with("pagina-versiones-");
    if cambiar_versiones {
        a.reducir=false;
        a.servicios.ficha.tab=crate::ficha::Pestana::Descripcion;
        let (tab,kind,loader)=if scene.contains("recursos"){(3,"resourcepack","minecraft")}else if scene.contains("shaders"){(8,"shader","iris")}else{(2,"mod","fabric")};
        a.editor=Some(tab);
        if let Some((project,versions))=&mut a.servicios.detail {
            project.project_type=kind.into();
            for v in versions {v.loaders=vec![loader.into()];}
        }
    }
    crate::componentes_nuevos_qa::preparar(&mut a,&scene);
    if scene == "capturas-instancia-vacia" {
        a.editor = Some(5);
        a.capt.capturas.clear();
    }
    if scene.starts_with("mods-menu") {
        a.servicios.mods.diseno.menu_barra = true;
        if scene.contains("seleccion") {
            let id = a.servicios.instancias.registros[0].id.clone();
            a.servicios.mods.diseno.seleccion.insert(id, std::collections::BTreeSet::from(["sodium.jar".into()]));
        }
    }
    // Las escenas de la revisión llevan a la vista su sección, como al tocar la tarjeta del tipo.
    let seccion = match scene.trim_end_matches("-angosto") {
        "compartir-publicacion-filas" => Some(icaro_ui::compartir::TipoContenido::Mod),
        "compartir-publicacion-configuraciones" => Some(icaro_ui::compartir::TipoContenido::Config),
        "compartir-packs" => Some(icaro_ui::compartir::TipoContenido::Resourcepack),
        _ => None,
    };
    let desplazar = seccion.map_or_else(Task::none, |t| {
        Task::perform(async {tokio::time::sleep(std::time::Duration::from_millis(300)).await;}, |_| ())
            .then(move |_| icaro_ui::compartir::llevar_a_tipo(t))
    });
    let transicion = if cambiar_versiones {Task::perform(async {tokio::time::sleep(std::time::Duration::from_millis(200)).await;}, |_|Mensaje::Servicio(crate::servicios::Accion::FichaPestana(crate::ficha::Pestana::Versiones)))} else {Task::none()};
    (
        a,
        Task::batch([transicion, desplazar,
        if scene == "mods-menu-maximizar" {
            Task::perform(async {tokio::time::sleep(std::time::Duration::from_millis(200)).await;}, |_| ())
                .then(|_| iced::window::get_latest().and_then(|id| iced::window::resize(id, Size::new(1920., 1000.))))
        } else { Task::none() },
        Task::perform(
            async {
                tokio::time::sleep(std::time::Duration::from_millis(800)).await;
            },
            |_| Mensaje::CapturarDiseno,
        )]),
    )
}

pub(crate) fn preparar_cambio(a: &mut App, scene: &str) {
    use icaro_core::{
        instancias::Instancia,
        manifest::VersionEntry,
        modrinth::Version,
        versiones_instancia::{Compatibilidad, Loaders, Revision},
    };
    use std::collections::{BTreeMap, BTreeSet};
    // Sólo la captura usa los datos de muestra del preview del usuario.
    // No se escriben archivos ni se programa ninguna consulta a Modrinth.
    a.servicios.demo = false;
    a.servicios.restoring = false;
    let record = Instancia {
        id: "00000000000000000000000000000001".into(),
        nombre: "Supervivencia".into(),
        minecraft: "1.21.4".into(),
        loader: "fabric".into(),
        portada: 0,
        creada: 0,
    };
    a.servicios.instancias.registros = vec![record.clone()];
    let muestras: [(&str, &str, usize); 8] = [
        ("26.3", "2026-10-08", 61),
        ("26.2", "2026-08-19", 142),
        ("26.1.2", "2026-06-24", 187),
        ("1.21.5", "2026-03-25", 206),
        ("1.21.4", "2025-12-03", 214),
        ("1.21.3", "2025-10-23", 211),
        ("1.21.1", "2025-08-08", 209),
        ("1.20.1", "2023-06-12", 173),
    ];
    a.servicios.instancias.versiones = muestras
        .iter()
        .map(|(version, fecha, _)| VersionEntry {
            id: (*version).into(),
            kind: "release".into(),
            url: String::new(),
            sha1: String::new(),
            release_time: format!("{fecha}T00:00:00Z"),
        })
        .collect();
    let detalle: Vec<String> = [
        "Xaero's Minimap",
        "Replay Mod",
        "Dynamic FPS",
        "Chat Heads",
        "Inventory HUD",
        "Better Ping",
        "Zoomify",
        "Krypton",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect();
    let mut compatibilidades = BTreeMap::new();
    for (mc, _, compatibles) in muestras {
        let es_actual = mc == record.minecraft;
        let actualizados = if es_actual {
            0
        } else {
            compatibles.saturating_sub(8)
        };
        let actualizar: Vec<Version> = (0..actualizados).map(|index| {
            serde_json::from_value(serde_json::json!({
                "id":format!("qa-{mc}-{index}"), "project_id":format!("qa-mod-{index}"),
                "name":format!("Mod de muestra {index}"), "version_number":"1.0.0", "version_type":"release",
                "game_versions":[mc], "loaders":["fabric"], "downloads":0, "date_published":"2026-03-25T00:00:00Z",
                "files":[], "dependencies":[], "changelog":""
            })).unwrap()
        }).collect();
        let desactivar: Vec<String> = (0..214 - compatibles)
            .map(|i| format!("qa-incompatible-{i}.jar"))
            .collect();
        let nombres = (0..desactivar.len())
            .map(|i| {
                detalle
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| format!("Mod no compatible {}", i + 1))
            })
            .collect::<Vec<_>>();
        compatibilidades.insert(
            mc.into(),
            Compatibilidad {
                total: 214,
                compatibles,
                no_cambian: if es_actual { 214 } else { 8 },
                actualizar,
                desactivar_sin_actualizar: desactivar.clone(),
                nombres_sin_actualizar: nombres.clone(),
                desactivar,
                nombres,
                ..Default::default()
            },
        );
        let mut loaders = Loaders::default();
        loaders.versiones[1] = vec![if es_actual { "0.16.9" } else { "0.16.14" }.into()];
        loaders.versiones[2] = vec!["0.27.1".into()];
        a.servicios.instancias.loaders.insert(mc.into(), loaders);
    }
    let cargando = scene == "cambiar-version-carga";
    let confirmando = scene == "cambiar-version";
    let destino = if confirmando { "1.20.1" } else { "1.21.5" };
    let revision = confirmando.then(|| {
        let compatibilidad = compatibilidades.get(destino).unwrap().clone();
        Revision {
            mundos: 3,
            desactivar: compatibilidad.desactivar.clone(),
            nombres: compatibilidad.nombres.clone(),
            baja: true,
            loader_version: Some("0.16.14".into()),
            loader: "fabric".into(),
            compatibilidad,
        }
    });
    let cargando_compatibilidad = if cargando {
        compatibilidades.keys().cloned().collect()
    } else {
        BTreeSet::new()
    };
    if cargando {
        compatibilidades.clear();
    }
    a.cambiar = Some(0);
    a.servicios.instancias.cambio = Some(instancias_reales::Cambio {
        instancia: record.id,
        tipo: 0,
        busqueda: String::new(),
        destino: destino.into(),
        revision,
        revisando: false,
        respaldo: true,
        error: None,
        compatibilidades,
        cargando_compatibilidad,
        errores_compatibilidad: BTreeMap::new(),
        loader_destino: "fabric".into(),
        loader_actual_version: Some("0.16.9".into()),
        solo_loader: false,
        loader_version_destino: None,
        actualizar_mods: true,
        copia: false,
        detalle_desactivar: scene == "cambiar-version-detalle",
        confirmando,
        compat_generacion: 0,
        mods: Some(vec![]),
        destino_manual: true,
        cancelar_compatibilidad: tokio_util::sync::CancellationToken::new(),
    });
    if scene == "cambiar-version-seleccion-angosto" {
        a.ventana = Size::new(940., 720.);
    }
}
pub(crate) fn preparar_modpacks(a: &mut App, scene: &str) {
    preparar_mods(a, if scene.contains("versiones") { "pagina-modpack" } else { "tienda" });
    a.editor = None;
    a.mods_vista = 0;
    a.seccion = Seccion::Modpacks;
    a.tienda_cat.clear();
    a.tienda_lado = 2;
    if let Some(results) = &mut a.servicios.results {
        for (hit, title) in results.hits.iter_mut().zip(["Fabulously Optimized", "Simply Optimized", "Adrenaline", "Cobblemon Official Modpack"]) {
            hit.title = title.into();
            hit.description = "Un modpack preparado con sus mods y configuración. Elige qué versión instalar.".into();
        }
    }
    a.servicios.tags = Some(icaro_core::modrinth::Tags {
        versions: ["26.3", "1.21.4", "1.20.1"].into_iter().map(|version| icaro_core::modrinth::GameVersion { version: version.into(), version_type: "release".into() }).collect(),
        loaders: ["fabric", "quilt", "neoforge", "forge"].into_iter().map(|name| icaro_core::modrinth::Loader { name: name.into(), supported_project_types: vec!["modpack".into()] }).collect(),
    });
    if let Some((project, versions)) = &mut a.servicios.detail {
        project.id = "optimized-pack".into();
        for (n, version) in versions.iter_mut().enumerate() {
            version.project_id = project.id.clone();
            version.name = format!("Fabulously Optimized 6.{}", 10 - n);
            version.version_number = format!("6.{}", 10 - n);
            version.game_versions = vec![if n < 4 { "1.21.4" } else { "1.20.1" }.into()];
            version.files = serde_json::from_value(serde_json::json!([{"filename":"optimized.mrpack", "url":"https://cdn.modrinth.com/data/fixture/versions/sample/optimized.mrpack", "primary":true, "size":24000, "hashes":{"sha512":"aa".repeat(64)}}])).unwrap();
        }
        a.servicios.ficha.minecraft = "Todas las versiones".into();
        a.servicios.ficha.loader = "Todos los loaders".into();
    }
}

pub(crate) fn preparar_mods(a: &mut App, scene: &str) {
    use icaro_core::{
        instancias::Instancia,
        modrinth::{Hit, Project, Version},
        mods::Instalado,
    };
    a.editor = Some(2);
    a.editor_inst = 0;
    a.servicios.demo = false;
    a.servicios.restoring = false;
    a.servicios.instancias.almacen = icaro_core::instancias::Almacen::nuevo(
        std::env::temp_dir().join("icaro-design-fixture-instances"),
    );
    let instance = Instancia {
        id: "00000000000000000000000000000001".into(),
        nombre: "Supervivencia".into(),
        minecraft: "1.21.4".into(),
        loader: "fabric".into(),
        portada: 0,
        creada: 0,
    };
    a.servicios.instancias.registros = vec![instance.clone()];
    if let Some(card) = crate::lista_instancias().lock().unwrap().first_mut() {
        card.mods = "4 mods".into();
    }
    a.servicios
        .mods
        .tamanos
        .insert(instance.id.clone(), 312 * 1048576);
    let mut hits = vec![];
    let mut installed = vec![];
    let mut versions = vec![];
    for (id, title, author, description, v, downloads) in [
        (
            "sodium",
            "Sodium",
            "CaffeineMC",
            "Render más rápido sin cambiar el aspecto del juego.",
            "0.6.5",
            78_412_330,
        ),
        (
            "lithium",
            "Lithium",
            "CaffeineMC",
            "Optimiza la lógica del mundo sin cambiar mecánicas.",
            "0.14.3",
            39_880_551,
        ),
        (
            "iris",
            "Iris Shaders",
            "IMS",
            "Shaders compatibles con Sodium.",
            "1.8.1",
            12_000_000,
        ),
        (
            "replay",
            "Replay Mod",
            "john0702",
            "Graba y reproduce partidas.",
            "2.6.20",
            9_000_000,
        ),
    ] {
        let p:Project=serde_json::from_value(serde_json::json!({"id":id,"slug":id,"title":title,"description":description,"body":"## Rendimiento\n\nRender más rápido sin cambiar el aspecto del juego.\n\n### Características\n\n- Compatibilidad con tu instancia.\n- Opciones de calidad y rendimiento.","license":{"id":"MIT","name":"MIT"},"downloads":downloads,"followers":41288,"game_versions":["1.21.4","1.21.3","1.21.1"],"loaders":["fabric","quilt","neoforge"],"categories":["optimization"],"published":"2021-08-20T00:00:00Z","updated":"2026-10-06T00:00:00Z","client_side":"required","server_side":"unsupported","source_url":"https://github.com/CaffeineMC/sodium","issues_url":"https://github.com/CaffeineMC/sodium/issues"})).unwrap();
        let version:Version=serde_json::from_value(serde_json::json!({"id":format!("{id}065"),"project_id":id,"name":v,"version_number":v,"version_type":"release","game_versions":["1.21.4"],"loaders":["fabric"],"downloads":2_110_000,"date_published":"2026-10-06T00:00:00Z","files":[],"dependencies":[],"changelog":"## Cambios\n\n- Corrección de errores."})).unwrap();
        hits.push(Hit {
            project_id: id.into(),
            slug: id.into(),
            title: title.into(),
            description: description.into(),
            author: author.into(),
            downloads,
            icon_url: None,
            categories: vec!["optimization".into()],
            versions: vec!["1.21.4".into()],
        });
        installed.push(Instalado {
            project_id: id.into(),
            nombre: title.into(),
            version_id: version.id.clone(),
            version: v.into(),
            publicada: version.date_published.clone(),
            archivo: format!("{id}.jar"),
            sha512: String::new(),
            activo: id != "replay",
            dependencias: vec![],
        });
        a.servicios
            .mods
            .diseno
            .proyectos
            .insert(id.into(), (p, author.into()));
        versions.push(version);
    }
    if matches!(scene, "mods" | "seleccion") {
        installed[2].dependencias=serde_json::from_value(serde_json::json!([{"project_id":"sodium","version_id":null,"dependency_type":"incompatible"}])).unwrap();
    }
    let mut newer = versions[1].clone();
    newer.id = "lithiumnew".into();
    newer.version_number = "0.14.5".into();
    a.servicios
        .mods
        .updates
        .insert(instance.id.clone(), vec![newer]);
    a.servicios
        .mods
        .listas
        .insert(instance.id.clone(), installed);
    if scene == "seleccion" {
        a.servicios.mods.diseno.seleccion.insert(
            instance.id.clone(),
            std::collections::BTreeSet::from(["sodium.jar".into(), "lithium.jar".into()]),
        );
    }
    if matches!(scene, "tienda" | "ficha" | "ficha-creadores") || scene.starts_with("pagina") {
        a.mods_vista = 1;
        a.servicios.order = icaro_core::modrinth::Order::Downloads;
        a.servicios.results = Some(icaro_core::modrinth::Results {
            hits,
            offset: 0,
            limit: 20,
            total_hits: 1284,
        });
        let v = versions[0].clone();
        a.servicios.mods.diseno.cestas.insert(
            instance.id.clone(),
            crate::diseno_mods::Cesta::para_prueba(v.clone(), "Sodium".into()),
        );
        a.servicios
            .mods
            .listas
            .get_mut(&instance.id)
            .unwrap()
            .retain(|m| m.project_id != "sodium");
        if scene != "tienda" {
            let mut p = a.servicios.mods.diseno.proyectos["sodium"].0.clone();
            for (n, file) in ["piranesi-arco.webp", "faeton.webp", "tivoli.webp"]
                .iter()
                .enumerate()
            {
                let url = format!("https://cdn.modrinth.com/data/fixture/images/{n}.webp");
                if let Ok(bytes) = std::fs::read(
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("assets/originales")
                        .join(file),
                ) {
                    a.servicios
                        .images
                        .recibir(url.clone(), catalogo::decodificar(bytes));
                }
                p.gallery.push(
                    serde_json::from_value(
                        serde_json::json!({"url":url,"featured":n==0,"title":file}),
                    )
                    .unwrap(),
                );
            }
            if scene.starts_with("pagina-modpack") {
                p.project_type="modpack".into();p.title="Fabulously Optimized".into();
                p.license.name="BSD 3-Clause \"New\" or \"Revised\" License".into();p.license.url=None;
                p.game_versions=["26.3","26.2","26.1.2","26.1.1","26.1","1.21.11","1.21.10","1.20.6","1.18.2","1.17.1","1.16.5"].iter().map(|v|v.to_string()).collect();
                p.loaders=vec!["fabric".into()];
                if scene=="pagina-modpack-detalles" {p.game_versions.clear();p.loaders.clear();p.client_side.clear();p.server_side.clear();p.source_url=None;p.issues_url=None;}
                a.editor=None;a.seccion=Seccion::Modpacks;
            }
            a.servicios.ficha = ficha::Estado::nuevo(None, &versions[..1]);
            a.servicios.ficha.minecraft = "1.21.4".into();
            a.servicios.ficha.loader = "Fabric".into();
            a.servicios.ficha.miembros=serde_json::from_value(serde_json::json!([{"user":{"id":"caffeine","username":"CaffeineMC","name":"CaffeineMC"},"role":"Owner","is_owner":true}])).unwrap();
            a.servicios.document = catalogo::Documento::parse(&p.body);
            let mut vv = versions[..1].to_vec();
            for n in 0..7 {
                let mut old = v.clone();
                old.id = format!("sodiumold{n}");
                old.version_number = format!("0.6.{}", 4_usize.saturating_sub(n));
                vv.push(old);
            }
            vv[0].dependencies=serde_json::from_value(serde_json::json!([{"project_id":"fabric-api","version_id":null,"dependency_type":"required"},{"project_id":"sodium-extra","version_id":null,"dependency_type":"optional"}])).unwrap();
            a.servicios.dependencias.insert(
                "p:fabric-api".into(),
                Ok(("fabric-api".into(), "Fabric API".into())),
            );
            a.servicios.dependencias.insert(
                "p:sodium-extra".into(),
                Ok(("sodium-extra".into(), "Sodium Extra".into())),
            );
            a.servicios.detail = Some((p, vv));
            if scene == "ficha-creadores" || scene.starts_with("pagina-creadores") {
                servicios::preparar_creadores_diseno(a);
            }
            if scene == "pagina-creadores-minima" {
                // Fixture sin secciones opcionales para revisar todos los
                // avatares sin operar ni desplazar una ventana del usuario.
                if let Some((project, _)) = &mut a.servicios.detail {
                    project.game_versions.clear(); project.loaders.clear(); project.categories.clear();
                    project.client_side.clear(); project.server_side.clear();
                    project.source_url=None; project.issues_url=None;
                }
            }
            if scene == "ficha" {
                a.mod_panel = Some("sodium".into());
            } else {
                a.servicios.ficha.tab = match scene {
                    "pagina-galeria" => ficha::Pestana::Galeria,
                    "pagina-descripcion" => ficha::Pestana::Descripcion,
                    "pagina-dependencias" => ficha::Pestana::Dependencias,
                    _ => ficha::Pestana::Versiones,
                };
            }
        }
    }
}
