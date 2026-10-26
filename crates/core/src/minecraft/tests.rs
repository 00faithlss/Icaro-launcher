    use super::*;
    fn temp() -> PathBuf {
        let path = std::env::temp_dir().join(format!("icaro-minecraft-test-{}", random().unwrap()));
        fs::create_dir(&path).unwrap();
        path
    }
    fn fixture_legacy(store: &Almacen, instance: &Instancia) -> PathBuf {
        use serde_json::json;
        let root = store.carpeta(&instance.id).unwrap().join(PRIVADO);
        fs::create_dir_all(&root).unwrap();
        let download = |bytes: &[u8], path: &str| {
            json!({
                "url": "https://piston-data.mojang.com/fixture",
                "sha1": crate::manifest::sha1_hex(bytes), "size": bytes.len(), "path": path
            })
        };
        let resource = b"recurso compartido";
        let hash = crate::manifest::sha1_hex(resource);
        let index = serde_json::to_vec(
            &json!({"objects": {"sound/test": {"hash": hash, "size": resource.len()}}}),
        )
        .unwrap();
        let mut asset = download(&index, "");
        asset["id"] = json!("fixture");
        let version = serde_json::to_vec(&json!({
            "id": instance.minecraft, "type": "release", "mainClass": "test.Main",
            "downloads": {"client": download(b"client", "")},
            "libraries": [{"name": "test:shared:1", "downloads": {"artifact": download(b"library", "test/shared/1/shared.jar")}}],
            "assetIndex": asset, "javaVersion": {"majorVersion": 21, "component": "java-runtime-delta"},
            "arguments": {"jvm": ["-cp", "${classpath}", "-DlibraryDirectory=${library_directory}"], "game": ["--assetsDir", "${assets_root}", "--assetIndex", "${assets_index_name}"]}
        })).unwrap();
        let manifest = json!({"latest":{"release": instance.minecraft, "snapshot": instance.minecraft}, "versions":[{
            "id":instance.minecraft, "type":"release", "url":"https://piston-meta.mojang.com/fixture", "sha1":crate::manifest::sha1_hex(&version)
        }]});
        for (path, bytes) in [
            (
                root.join("catalogo.json"),
                serde_json::to_vec(&manifest).unwrap(),
            ),
            (
                root.join("versions")
                    .join(&instance.minecraft)
                    .join("version.json"),
                version,
            ),
            (
                root.join("versions")
                    .join(&instance.minecraft)
                    .join("client.jar"),
                b"client".to_vec(),
            ),
            (
                root.join("libraries/test/shared/1/shared.jar"),
                b"library".to_vec(),
            ),
            (root.join("assets/indexes/fixture.json"), index),
            (
                root.join("assets/objects").join(&hash[..2]).join(hash),
                resource.to_vec(),
            ),
        ] {
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }
        // Evita consultas complementarias de Java: el fixture ya tiene perfil.
        escribir_json(&root, &root.join(MARKER), &json!({
            "schema": 1, "sistema": "fixture", "archivos": [], "nativos": [],
            "instalado": {"version": instance.minecraft, "main_class":"test.Main", "classpath":[], "natives":root.join("old-natives"),
            "assets":root.join("assets"), "asset_index":"fixture", "java_major":21, "java_component":"java-runtime-delta",
            "java_perfil":{"majors":[21], "component":"java-runtime-delta", "prism":true}, "game_arguments":[], "jvm_arguments":[]}
        })).unwrap();
        root
    }

    #[test]
    fn renombre_preserva_nativos_y_configuracion_y_rechaza_conflictos() {
        let root = temp();
        let store = Almacen::nuevo(root.join("instances"));
        let instance = store.crear("Renombre", "fixture", 0).unwrap();
        let folder = store.carpeta(&instance.id).unwrap().canonicalize().unwrap();
        let old = folder.join(PRIVADO_ANTERIOR);
        fs::create_dir_all(old.join("natives")).unwrap();
        fs::write(old.join("natives/test.dll"), b"nativo").unwrap();
        let config = fs::read(folder.join("instance.json")).unwrap();
        escribir_json(
            &old,
            &old.join(MARKER),
            &serde_json::json!({
                "natives": old.join("natives"), "custom": {"conservar": 42},
                "argument": format!("-Djava.library.path={}", old.join("natives").display())
            }),
        )
        .unwrap();
        let target = migrar_directorio(&store, &instance).unwrap();
        assert_eq!(target, folder.join(PRIVADO));
        assert!(!old.exists());
        let marker: serde_json::Value = leer_json(&target, &target.join(MARKER)).unwrap();
        assert_eq!(
            marker["natives"],
            target.join("natives").to_string_lossy().as_ref()
        );
        assert_eq!(marker["custom"]["conservar"], 42);
        assert_eq!(
            marker["argument"],
            format!("-Djava.library.path={}", target.join("natives").display())
        );
        assert_eq!(
            fs::read(target.join("natives/test.dll")).unwrap(),
            b"nativo"
        );
        assert_eq!(fs::read(folder.join("instance.json")).unwrap(), config);
        assert_eq!(migrar_directorio(&store, &instance).unwrap(), target);
        fs::create_dir(&old).unwrap();
        assert!(matches!(
            migrar_directorio(&store, &instance),
            Err(Error::NoSoportada(_))
        ));
        assert!(old.exists() && target.exists());
        fs::remove_dir_all(root).unwrap();
    }

    fn fixture_fabric(root: &Path, mc: &str, loader: &str) -> Cargador {
        let names = [
            format!("net.fabricmc:fabric-loader:{loader}"),
            format!("net.fabricmc:intermediary:{mc}"),
        ];
        let mut libraries = vec![];
        let mut descargas = vec![];
        for name in names {
            let bytes = name.as_bytes();
            let hash = crate::manifest::sha1_hex(bytes);
            let path = ruta_maven(&name).unwrap();
            let local = root.join("libraries").join(&hash).join(&path);
            fs::create_dir_all(local.parent().unwrap()).unwrap();
            fs::write(local, bytes).unwrap();
            descargas.push(Descarga {
                url: format!("https://maven.fabricmc.net/{path}"),
                sha1: hash,
                size: bytes.len() as u64,
                path,
                id: String::new(),
            });
            libraries.push(LibreriaCargador {
                name,
                url: "https://maven.fabricmc.net/".into(),
                sha1: None,
                size: None,
            });
        }
        Cargador{schema:1,sin_intermediary:false,minecraft:mc.into(),loader:loader.into(),min_java:8,profile:PerfilCargador{
            id:format!("fabric-loader-{loader}-{mc}"),inherits_from:mc.into(),main_class:"net.fabricmc.loader.impl.launch.knot.KnotClient".into(),
            arguments:serde_json::from_value(serde_json::json!({"jvm":["-DFabricMcEmu= net.minecraft.client.main.Main "],"game":[]})).unwrap(),libraries},descargas}
    }
    #[tokio::test]
    async fn fabric_instala_perfil_fijado_y_reutiliza_componentes_sin_red() {
        let root = temp();
        let store = Almacen::nuevo(root.join("instances"));
        let mut instance = store.crear("Fabric", "fixture", 0).unwrap();
        instance.loader = "fabric".into();
        store.guardar(&instance).unwrap();
        crate::versiones_instancia::guardar_loader(&store, &instance, "0.16.14").unwrap();
        let private = fixture_legacy(&store, &instance);
        let mut marker: serde_json::Value = leer_json(&private, &private.join(MARKER)).unwrap();
        marker["loader"] = serde_json::json!("fabric");
        marker["loader_version"] = serde_json::json!("0.16.14");
        escribir_json(&private, &private.join(MARKER), &marker).unwrap();
        let bundle = fixture_fabric(&root, "fixture", "0.16.14");
        escribir_json(
            &root,
            &root.join("meta/fabric/fixture/0.16.14/perfil.json"),
            &bundle,
        )
        .unwrap();
        for _ in 0..2 {
            let installed = instalar(&store, &instance, CancellationToken::new(), |p| {
                assert!(!p.etapa.starts_with("Descargando"))
            })
            .await
            .unwrap();
            assert!(listo(&store, &instance));
            assert_eq!(installed.main_class, bundle.profile.main_class);
            assert_eq!(
                installed.perfil_id.as_deref(),
                Some("fabric-loader-0.16.14-fixture")
            );
            assert!(installed.classpath[0].ends_with("fabric-loader-0.16.14.jar"));
            assert!(installed
                .jvm_arguments
                .contains(&"-DFabricMcEmu= net.minecraft.client.main.Main ".into()));
            assert_eq!(installed.java_perfil.unwrap().majors, vec![21]);
        }
        let before = fs::read(private.join(MARKER)).unwrap();
        let mut incompatible = fixture_fabric(&root, "fixture", "0.16.14");
        incompatible.min_java = 22;
        escribir_json(
            &root,
            &root.join("meta/fabric/fixture/0.16.14/perfil.json"),
            &incompatible,
        )
        .unwrap();
        assert!(matches!(
            instalar(&store, &instance, CancellationToken::new(), |_| {}).await,
            Err(Error::NoSoportada(_))
        ));
        assert_eq!(fs::read(private.join(MARKER)).unwrap(), before);
        escribir_json(
            &root,
            &root.join("meta/fabric/fixture/0.16.14/perfil.json"),
            &bundle,
        )
        .unwrap();
        let cancel = CancellationToken::new();
        let observer = cancel.clone();
        assert!(matches!(
            instalar(&store, &instance, cancel, |p| {
                if p.etapa == "Preparando el perfil oficial de Fabric" {
                    observer.cancel();
                }
            })
            .await,
            Err(Error::Cancelada)
        ));
        assert_eq!(fs::read(private.join(MARKER)).unwrap(), before);
        crate::versiones_instancia::guardar_loader(&store, &instance, "0.16.13").unwrap();
        assert!(!listo(&store, &instance));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn fabric_rechaza_perfiles_cruzados_y_rutas_maven_invalidas() {
        let root = temp();
        let mut bundle = fixture_fabric(&root, "1.21.1", "0.16.14");
        assert!(validar_cargador(Familia::Fabric, &bundle, "1.21.1", "0.16.14").is_ok());
        assert!(validar_cargador(Familia::Fabric, &bundle, "1.21.4", "0.16.14").is_err());
        assert!(validar_cargador(Familia::Fabric, &bundle, "1.21.1", "0.16.13").is_err());
        bundle.descargas[0].url = "https://example.com/malware.jar".into();
        assert!(validar_cargador(Familia::Fabric, &bundle, "1.21.1", "0.16.14").is_err());
        for name in [
            "net.fabricmc:../../escape:1",
            "net..fabricmc:loader:1",
            "net.fabricmc:loader:1:extra",
            "net.fabricmc:loader:../1",
        ] {
            assert!(ruta_maven(name).is_err());
        }
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn versiones_antiguas_con_espacios_se_conservan_sin_permitir_escape() {
        let root=temp();let store=Almacen::nuevo(root.join("instances"));
        let i=store.crear_con_loader("Pre release","1.14.2 Pre-Release 4","fabric",0).unwrap();
        crate::versiones_instancia::guardar_loader(&store,&i,"0.19.5").unwrap();
        assert_eq!(store.cargar().unwrap().instancias[0].minecraft,"1.14.2 Pre-Release 4");
        assert_eq!(crate::versiones_instancia::leer_loader(&store,&i).as_deref(),Some("0.19.5"));
        assert!(ruta_maven("net.fabricmc:intermediary:1.14.2 Pre-Release 4").unwrap().contains("1.14.2 Pre-Release 4"));
        for v in ["../escape","a/b","a\\b","a?b","version ",". "," a","a."] {assert!(!version_minecraft_valida(v),"{v}");}
        assert!(ruta_maven("net.fabricmc:intermediary:../escape").is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn fabric_sin_ofuscacion_omite_intermediary_solo_con_metadatos_coherentes() {
        let root = temp();
        let mut bundle = fixture_fabric(&root, "26.3", "0.19.5");
        bundle.profile.libraries.pop();
        bundle.descargas.pop();
        let mut detail = serde_json::json!({"loader":{"version":"0.19.5"},"intermediary":{"version":"0.0.0","maven":"net.fabricmc:intermediary:0.0.0"},"launcherMeta":{"mainClass":{"client":bundle.profile.main_class}}});
        bundle.sin_intermediary =
            validar_detalle(Familia::Fabric, &bundle.profile, &detail, "26.3", "0.19.5").unwrap();
        assert!(bundle.sin_intermediary);
        assert!(validar_cargador(Familia::Fabric, &bundle, "26.3", "0.19.5").is_ok());
        bundle.sin_intermediary = false;
        assert!(validar_cargador(Familia::Fabric, &bundle, "26.3", "0.19.5").is_err());
        detail["intermediary"]["version"] = serde_json::json!("1.21.1");
        assert!(validar_detalle(Familia::Fabric, &bundle.profile, &detail, "26.3", "0.19.5").is_err());
        let old = fixture_fabric(&root, "1.21.1", "0.16.14");
        detail["loader"]["version"] = serde_json::json!("0.16.14");
        detail["intermediary"] =
            serde_json::json!({"version":"1.21.1","maven":"net.fabricmc:intermediary:1.21.1"});
        assert!(!validar_detalle(Familia::Fabric, &old.profile, &detail, "1.21.1", "0.16.14").unwrap());
        detail["intermediary"] =
            serde_json::json!({"version":"0.0.0","maven":"net.fabricmc:intermediary:0.0.0"});
        assert!(validar_detalle(Familia::Fabric, &old.profile, &detail, "1.21.1", "0.16.14").is_err());
        fs::remove_dir_all(root).unwrap();
    }

    fn fixture_quilt(root: &Path, mc: &str, loader: &str, sin_mapeos: bool) -> Cargador {
        let fabric = "https://maven.fabricmc.net/";
        let quilt = "https://maven.quiltmc.org/repository/release/";
        let mut nombres = vec![
            (format!("org.quiltmc:quilt-loader:{loader}"), quilt),
            ("net.fabricmc:sponge-mixin:0.17.3+mixin.0.8.7".to_owned(), fabric),
        ];
        if !sin_mapeos {
            nombres.push((format!("org.quiltmc:hashed:{mc}"), quilt));
            nombres.push((format!("net.fabricmc:intermediary:{mc}"), fabric));
        }
        let mut libraries = vec![];
        let mut descargas = vec![];
        for (name, repositorio) in nombres {
            let bytes = name.as_bytes();
            let hash = crate::manifest::sha1_hex(bytes);
            let path = ruta_maven(&name).unwrap();
            let local = root.join("libraries").join(&hash).join(&path);
            fs::create_dir_all(local.parent().unwrap()).unwrap();
            fs::write(local, bytes).unwrap();
            descargas.push(Descarga {
                url: format!("{repositorio}{path}"),
                sha1: hash,
                size: bytes.len() as u64,
                path,
                id: String::new(),
            });
            libraries.push(LibreriaCargador {
                name,
                url: repositorio.into(),
                sha1: None,
                size: None,
            });
        }
        Cargador {
            schema: 1,
            sin_intermediary: sin_mapeos,
            minecraft: mc.into(),
            loader: loader.into(),
            min_java: 8,
            profile: PerfilCargador {
                id: format!("quilt-loader-{loader}-{mc}"),
                inherits_from: mc.into(),
                main_class: "org.quiltmc.loader.impl.launch.knot.KnotClient".into(),
                arguments: serde_json::from_value(serde_json::json!({"jvm":["-DQuiltEmu=1"],"game":[]})).unwrap(),
                libraries,
            },
            descargas,
        }
    }
    #[tokio::test]
    async fn quilt_instala_perfil_fijado_y_reutiliza_componentes_sin_red() {
        let root = temp();
        let store = Almacen::nuevo(root.join("instances"));
        let mut instance = store.crear("Quilt", "fixture", 0).unwrap();
        instance.loader = "quilt".into();
        store.guardar(&instance).unwrap();
        crate::versiones_instancia::guardar_loader(&store, &instance, "0.30.1").unwrap();
        let private = fixture_legacy(&store, &instance);
        let mut marker: serde_json::Value = leer_json(&private, &private.join(MARKER)).unwrap();
        marker["loader"] = serde_json::json!("quilt");
        marker["loader_version"] = serde_json::json!("0.30.1");
        escribir_json(&private, &private.join(MARKER), &marker).unwrap();
        let bundle = fixture_quilt(&root, "fixture", "0.30.1", false);
        let perfil = root.join("meta/quilt/fixture/0.30.1/perfil.json");
        escribir_json(&root, &perfil, &bundle).unwrap();
        for _ in 0..2 {
            let installed = instalar(&store, &instance, CancellationToken::new(), |p| {
                assert!(!p.etapa.starts_with("Descargando"))
            })
            .await
            .unwrap();
            assert!(listo(&store, &instance));
            assert_eq!(installed.main_class, "org.quiltmc.loader.impl.launch.knot.KnotClient");
            assert_eq!(installed.perfil_id.as_deref(), Some("quilt-loader-0.30.1-fixture"));
            assert!(installed.classpath[0].ends_with("quilt-loader-0.30.1.jar"));
            assert!(installed.jvm_arguments.contains(&"-DQuiltEmu=1".into()));
        }
        // Un perfil de Fabric no sirve para una instancia Quilt y no cambia el marcador.
        let antes = fs::read(private.join(MARKER)).unwrap();
        let fabric = fixture_fabric(&root, "fixture", "0.30.1");
        escribir_json(&root, &perfil, &fabric).unwrap();
        assert!(validar_cargador(Familia::Quilt, &fabric, "fixture", "0.30.1").is_err());
        let cancel = CancellationToken::new();
        let observador = cancel.clone();
        escribir_json(&root, &perfil, &bundle).unwrap();
        assert!(matches!(
            instalar(&store, &instance, cancel, |p| {
                if p.etapa == "Preparando el perfil oficial de Quilt" {
                    observador.cancel();
                }
            })
            .await,
            Err(Error::Cancelada)
        ));
        assert_eq!(fs::read(private.join(MARKER)).unwrap(), antes);
        crate::versiones_instancia::guardar_loader(&store, &instance, "0.30.0").unwrap();
        assert!(!listo(&store, &instance));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn quilt_exige_sus_repositorios_y_mapeos() {
        let root = temp();
        let bundle = fixture_quilt(&root, "1.21.4", "0.30.1", false);
        assert!(validar_cargador(Familia::Quilt, &bundle, "1.21.4", "0.30.1").is_ok());
        assert!(validar_cargador(Familia::Quilt, &bundle, "1.21.1", "0.30.1").is_err());
        assert!(validar_cargador(Familia::Quilt, &bundle, "1.21.4", "0.30.0").is_err());
        // Los repositorios de Quilt no valen para Fabric ni al revés.
        assert!(validar_cargador(Familia::Fabric, &bundle, "1.21.4", "0.30.1").is_err());
        let mut sin_hashed = fixture_quilt(&root, "1.21.4", "0.30.1", false);
        let n = sin_hashed
            .profile
            .libraries
            .iter()
            .position(|l| l.name.starts_with("org.quiltmc:hashed:"))
            .unwrap();
        sin_hashed.profile.libraries.remove(n);
        sin_hashed.descargas.remove(n);
        assert!(validar_cargador(Familia::Quilt, &sin_hashed, "1.21.4", "0.30.1").is_err());
        let mut ajeno = fixture_quilt(&root, "1.21.4", "0.30.1", false);
        ajeno.profile.libraries[0].url = "https://maven.example.com/".into();
        ajeno.descargas[0].url = format!("https://maven.example.com/{}", ajeno.descargas[0].path);
        assert!(validar_cargador(Familia::Quilt, &ajeno, "1.21.4", "0.30.1").is_err());
        let mut cruzado = fixture_quilt(&root, "1.21.4", "0.30.1", false);
        cruzado.profile.id = "fabric-loader-0.30.1-1.21.4".into();
        assert!(validar_cargador(Familia::Quilt, &cruzado, "1.21.4", "0.30.1").is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn quilt_sin_ofuscacion_omite_los_mapeos_solo_con_metadatos_coherentes() {
        let root = temp();
        let bundle = fixture_quilt(&root, "26.3", "0.30.1", true);
        let detalle = |hashed: serde_json::Value, intermediary: serde_json::Value| serde_json::json!({
            "loader":{"version":"0.30.1"},"hashed":hashed,"intermediary":intermediary,
            "launcherMeta":{"mainClass":{"client":bundle.profile.main_class}}});
        let nulo = serde_json::Value::Null;
        assert!(validar_detalle(Familia::Quilt, &bundle.profile, &detalle(nulo.clone(), nulo.clone()), "26.3", "0.30.1").unwrap());
        assert!(validar_cargador(Familia::Quilt, &bundle, "26.3", "0.30.1").is_ok());
        // Declarar sin ofuscación pero exigir mapeos, o al revés, se rechaza.
        let mut contradictorio = fixture_quilt(&root, "26.3", "0.30.1", true);
        contradictorio.sin_intermediary = false;
        assert!(validar_cargador(Familia::Quilt, &contradictorio, "26.3", "0.30.1").is_err());
        let con_mapeos = fixture_quilt(&root, "1.21.4", "0.30.1", false);
        let hashed = serde_json::json!({"version":"1.21.4","maven":"org.quiltmc:hashed:1.21.4"});
        let intermediary = serde_json::json!({"version":"1.21.4","maven":"net.fabricmc:intermediary:1.21.4"});
        assert!(!validar_detalle(Familia::Quilt, &con_mapeos.profile, &detalle(hashed.clone(), intermediary.clone()), "1.21.4", "0.30.1").unwrap());
        // Un solo mapeo en los metadatos, o uno de otra versión, no es coherente.
        assert!(validar_detalle(Familia::Quilt, &con_mapeos.profile, &detalle(nulo.clone(), intermediary.clone()), "1.21.4", "0.30.1").is_err());
        assert!(validar_detalle(Familia::Quilt, &con_mapeos.profile, &detalle(hashed, serde_json::json!({"version":"1.21.1","maven":"net.fabricmc:intermediary:1.21.1"})), "1.21.4", "0.30.1").is_err());
        // Declarar que no hay mapeos cuando el perfil los trae también se rechaza.
        assert!(validar_detalle(Familia::Quilt, &con_mapeos.profile, &detalle(nulo.clone(), nulo), "1.21.4", "0.30.1").is_err());
        fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    #[ignore = "Consulta la API oficial de Quilt y verifica los perfiles de versiones ofuscadas y sin ofuscar; no descarga el juego"]
    async fn quilt_oficial_se_prepara_y_se_reutiliza_sin_red() {
        let client = reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).timeout(Duration::from_secs(90)).build().unwrap();
        let cancel = CancellationToken::new();
        let juegos: Vec<serde_json::Value> = serde_json::from_slice(&bytes_oficiales(&client, "https://meta.quiltmc.org/v3/versions/game", METADATA_LIMIT as usize, &cancel).await.unwrap()).unwrap();
        let estable = juegos.iter().find(|g| g["stable"] == true).unwrap()["version"].as_str().unwrap().to_owned();
        let loaders: Vec<serde_json::Value> = serde_json::from_slice(&bytes_oficiales(&client, "https://meta.quiltmc.org/v3/versions/loader", METADATA_LIMIT as usize, &cancel).await.unwrap()).unwrap();
        let loader = loaders.iter().find(|l| !l["version"].as_str().unwrap().contains('-')).unwrap()["version"].as_str().unwrap().to_owned();
        let raiz = temp();
        for mc in ["1.21.4", estable.as_str()] {
            let bundle = preparar_cargador(Familia::Quilt, &client, &raiz, mc, &loader, &cancel).await.unwrap();
            assert!(bundle.profile.libraries.iter().any(|l| l.name == format!("org.quiltmc:quilt-loader:{loader}")));
            println!("{mc}: {} bibliotecas, sin mapeos: {}", bundle.descargas.len(), bundle.sin_intermediary);
            let sin_red = reqwest::Client::builder().proxy(reqwest::Proxy::all("http://127.0.0.1:9").unwrap()).build().unwrap();
            let guardado = preparar_cargador(Familia::Quilt, &sin_red, &raiz, mc, &loader, &cancel).await.unwrap();
            assert_eq!(guardado.descargas.len(), bundle.descargas.len());
        }
        fs::remove_dir_all(raiz).unwrap();
    }

    #[tokio::test]
    #[ignore = "Audita perfiles y metadatos del catálogo completo de Fabric; no descarga el juego"]
    async fn fabric_catalogo_completo_respeta_contrato_oficial() {
        let client=reqwest::Client::builder().redirect(reqwest::redirect::Policy::none()).timeout(Duration::from_secs(90)).build().unwrap();
        let cancel=CancellationToken::new();
        let games:Vec<serde_json::Value>=serde_json::from_slice(&bytes_oficiales(&client,"https://meta.fabricmc.net/v2/versions/game",METADATA_LIMIT as usize,&cancel).await.unwrap()).unwrap();
        let loaders:Vec<serde_json::Value>=serde_json::from_slice(&bytes_oficiales(&client,"https://meta.fabricmc.net/v2/versions/loader",METADATA_LIMIT as usize,&cancel).await.unwrap()).unwrap();
        let loader=loaders.iter().find(|l|l["stable"]==true).unwrap()["version"].as_str().unwrap().to_owned();
        let mut jobs=tokio::task::JoinSet::new();let mut checked=0;let total=games.len();
        for game in games {
            let mc=game["version"].as_str().unwrap().to_owned();
            assert!(version_minecraft_valida(&mc),"Minecraft: {mc}");
            let client=client.clone();let cancel=cancel.clone();let loader=loader.clone();
            jobs.spawn(async move {
                let endpoint=format!("https://meta.fabricmc.net/v2/versions/loader/{mc}/{loader}");
                let profile:PerfilCargador=serde_json::from_slice(&bytes_oficiales(&client,&format!("{endpoint}/profile/json"),METADATA_LIMIT as usize,&cancel).await.unwrap_or_else(|e|panic!("{mc}: {e}"))).unwrap();
                let detail:serde_json::Value=serde_json::from_slice(&bytes_oficiales(&client,&endpoint,METADATA_LIMIT as usize,&cancel).await.unwrap()).unwrap();
                let no_intermediary=validar_detalle(Familia::Fabric, &profile,&detail,&mc,&loader).unwrap_or_else(|e|panic!("{mc}: {e}"));
                let min=detail["launcherMeta"]["min_java_version"].as_u64().unwrap();assert!((1..=255).contains(&min),"{mc}");
                let mut names=BTreeSet::new();
                for lib in &profile.libraries {
                    assert!(names.insert(lib.name.clone()),"{mc}: duplicada");
                    assert_eq!(lib.url,"https://maven.fabricmc.net/","{mc}");
                    ruta_maven(&lib.name).unwrap_or_else(|e|panic!("{mc}: {e}"));
                    if let Some(hash)=&lib.sha1 {assert!(hash_valido(hash),"{mc}");}
                    if let Some(size)=lib.size {assert!(size>0&&size<=64*1024*1024,"{mc}");}
                }
                assert!(names.contains(&format!("net.fabricmc:fabric-loader:{loader}")),"{mc}");
                assert!(no_intermediary || names.contains(&format!("net.fabricmc:intermediary:{mc}")),"{mc}");
            });
            if jobs.len()==8 {jobs.join_next().await.unwrap().unwrap();checked+=1;if checked%50==0{println!("Perfiles comprobados: {checked}/{total}");}}
        }
        while let Some(job)=jobs.join_next().await {job.unwrap();checked+=1;}
        assert_eq!(checked,total);println!("Catálogo completo: {checked} versiones con Fabric {loader}");
    }

    #[tokio::test]
    #[ignore = "Descarga y verifica bibliotecas oficiales de Fabric; no inicia Minecraft"]
    async fn fabric_oficial_descarga_verifica_y_reutiliza_perfil() {
        let root = temp();
        let cancel = CancellationToken::new();
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(90))
            .build()
            .unwrap();
        let bundle = preparar_cargador(Familia::Fabric, &client, &root, "1.21.1", "0.16.14", &cancel)
            .await
            .unwrap();
        for download in &bundle.descargas {
            let path = root
                .join("libraries")
                .join(&download.sha1)
                .join(&download.path);
            let file = nuevo_archivo(download, path.clone(), "Fabric").unwrap();
            descargar(&client, &root, &file, &cancel).await.unwrap();
            assert_eq!(
                sha1_archivo(&path, &cancel).unwrap(),
                (download.size, download.sha1.clone())
            );
            if path.file_name().unwrap() == "fabric-loader-0.16.14.jar" {
                let mut zip = zip::ZipArchive::new(File::open(path).unwrap()).unwrap();
                assert!(zip
                    .by_name("net/fabricmc/loader/impl/launch/knot/KnotClient.class")
                    .is_ok());
            }
        }
        let offline = reqwest::Client::builder()
            .proxy(reqwest::Proxy::all("http://127.0.0.1:1").unwrap())
            .timeout(Duration::from_millis(100))
            .build()
            .unwrap();
        let cached = preparar_cargador(Familia::Fabric, &offline, &root, "1.21.1", "0.16.14", &cancel)
            .await
            .unwrap();
        assert_eq!(cached.profile.id, bundle.profile.id);
        assert_eq!(bundle.descargas.len(), 8);
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn dos_instancias_migran_comparten_y_reparan_sin_red() {
        let root = temp();
        let store = Almacen::nuevo(root.join("instances"));
        let mut a = store.crear("Primera", "fixture", 0).unwrap();
        let b = store.crear("Segunda", "fixture", 0).unwrap();
        let old_a = fixture_legacy(&store, &a);
        let old_b = fixture_legacy(&store, &b);
        let options = store.juego(&a.id).unwrap().join("options.txt");
        fs::write(&options, "personal").unwrap();
        let no_network = |p: Progreso| assert!(!p.etapa.starts_with("Descargando"), "{}", p.etapa);
        let first = instalar(&store, &a, CancellationToken::new(), no_network)
            .await
            .unwrap();
        assert!(listo(&store, &a));
        assert!(!old_a.join("libraries").exists());
        assert!(!old_a.join("assets").exists());
        // La segunda copia válida repara incluso corrupción del mismo tamaño.
        fs::write(&first.classpath[0], b"corrupt").unwrap();
        let second = instalar(&store, &b, CancellationToken::new(), no_network)
            .await
            .unwrap();
        assert_eq!(first.assets, second.assets);
        assert_eq!(first.classpath, second.classpath);
        assert_ne!(first.natives, second.natives);
        assert_eq!(fs::read(&first.classpath[0]).unwrap(), b"library");
        assert!(!old_b.join("libraries").exists());
        assert!(listo(&store, &a) && listo(&store, &b));
        // Un cambio de nombre solo reconstruye el estado privado; conserva el común.
        a.nombre = "Renombrada".into();
        store.guardar(&a).unwrap();
        let renamed = instalar(&store, &a, CancellationToken::new(), no_network)
            .await
            .unwrap();
        assert_eq!(first.classpath, renamed.classpath);
        assert_eq!(
            fs::read_to_string(store.juego(&a.id).unwrap().join("options.txt")).unwrap(),
            "personal"
        );
        assert_eq!(leer(&store, &a).unwrap().assets, first.assets);
        store.eliminar(&b.id).unwrap();
        assert!(listo(&store, &a));
        store.restaurar(&b.id).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn migracion_cancelada_conserva_copias_y_perfil_anterior() {
        let root = temp();
        let store = Almacen::nuevo(root.join("instances"));
        let instance = store.crear("Cancelada", "fixture", 0).unwrap();
        let old = fixture_legacy(&store, &instance);
        let before = fs::read(old.join(MARKER)).unwrap();
        let token = CancellationToken::new();
        let cancel = token.clone();
        let result = instalar(&store, &instance, token, |p| {
            if p.etapa == "Preparando componentes nativos" {
                cancel.cancel();
            }
        })
        .await;
        assert!(matches!(result, Err(Error::Cancelada)));
        assert_eq!(fs::read(old.join(MARKER)).unwrap(), before);
        assert_eq!(
            fs::read(old.join("libraries/test/shared/1/shared.jar")).unwrap(),
            b"library"
        );
        instalar(&store, &instance, CancellationToken::new(), |p| {
            assert!(!p.etapa.starts_with("Descargando"));
        })
        .await
        .unwrap();
        assert!(listo(&store, &instance));
        assert!(!old.join("libraries").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn bloqueo_compartido_espera_y_cancela_sin_publicar() {
        let root = temp();
        let held = bloqueo_compartido(&root, &CancellationToken::new(), &|_| {})
            .await
            .unwrap();
        let token = CancellationToken::new();
        let other = token.clone();
        let result = bloqueo_compartido(&root, &token, &|_| other.cancel()).await;
        assert!(matches!(result, Err(Error::Cancelada)));
        drop(held);
        let reacquired = bloqueo_compartido(&root, &CancellationToken::new(), &|_| {})
            .await
            .unwrap();
        drop(reacquired);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn reglas_de_so_arch_features_y_ultimo_match() {
        let rules: Vec<Regla> = serde_json::from_str(r#"[{"action":"allow"},{"action":"disallow","os":{"name":"osx"}},{"action":"disallow","features":{"is_demo_user":true}}]"#).unwrap();
        assert!(permitida(Some(&rules), "windows", "10.0.26100").unwrap());
        assert!(!permitida(Some(&rules), "osx", "15.0").unwrap());
        let args: Vec<Argumento> = serde_json::from_str(r#"["-cp","${classpath}",{"rules":[{"action":"allow","os":{"arch":"x86"}}],"value":"-Xss1M"},{"rules":[{"action":"allow","features":{"has_custom_resolution":true}}],"value":["--width","${resolution_width}"]}]"#).unwrap();
        assert_eq!(
            argumentos(&args, "windows", "10.0.26100").unwrap(),
            vec!["-cp", "${classpath}"]
        );
    }
    #[test]
    fn rutas_y_urls_rechazan_escape() {
        for path in [
            "../secret",
            "a/../secret",
            "/tmp/x",
            "C:/x",
            "a\\x",
            "a//x",
            "a/CON:foo",
        ] {
            assert!(relativa(path).is_err(), "{path}")
        }
        for url in [
            "http://libraries.minecraft.net/x",
            "https://libraries.minecraft.net.evil.test/x",
            "https://piston-data.mojang.com/x?token=x",
            "https://user@libraries.minecraft.net/x",
        ] {
            assert!(oficial(url).is_err(), "{url}")
        }
        assert!(oficial("https://libraries.minecraft.net/com/mojang/x.jar").is_ok());
    }
    #[test]
    fn cache_se_reutiliza_solo_con_tamano_y_hash() {
        let path = temp();
        let file = path.join("client.jar");
        fs::write(&file, b"good").unwrap();
        let download = Descarga {
            url: "https://piston-data.mojang.com/a".into(),
            sha1: crate::manifest::sha1_hex(b"good"),
            size: 4,
            path: String::new(),
            id: String::new(),
        };
        assert!(verificado(&path, &file, &download, &CancellationToken::new()).unwrap());
        fs::write(&file, b"evil").unwrap();
        assert!(!verificado(&path, &file, &download, &CancellationToken::new()).unwrap());
        let token = CancellationToken::new();
        token.cancel();
        assert!(matches!(sha1_archivo(&file, &token), Err(Error::Cancelada)));
        fs::remove_dir_all(path).unwrap();
    }

    #[tokio::test]
    async fn archivo_guardado_no_anuncia_descarga_ni_consulta_red() {
        let path = temp();
        let ruta = path.join("client.jar");
        fs::write(&ruta, b"good").unwrap();
        let file = nuevo_archivo(
            &Descarga {
                url: "https://piston-data.mojang.com/a".into(),
                sha1: crate::manifest::sha1_hex(b"good"),
                size: 4,
                path: String::new(),
                id: String::new(),
            },
            ruta,
            "Cliente de Minecraft",
        )
        .unwrap();
        let client = crate::http::client().unwrap();
        let descargado =
            descargar_observada(&client, &path, &file, &CancellationToken::new(), || {
                panic!("Una caché válida solo se prepara")
            })
            .await
            .unwrap();
        assert!(!descargado);
        fs::remove_dir_all(path).unwrap();
    }
    #[test]
    fn zip_nativo_rechaza_traversal_y_no_sobrescribe() {
        let path = temp();
        let archive = path.join("natives.jar");
        let mut zip = zip::ZipWriter::new(File::create(&archive).unwrap());
        zip.start_file("../escape.dll", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"bad").unwrap();
        zip.finish().unwrap();
        let target = path.join("out");
        fs::create_dir(&target).unwrap();
        assert!(extraer_nativos(
            &[(archive.clone(), vec![])],
            &target,
            &CancellationToken::new()
        )
        .is_err());
        assert!(!path.join("escape.dll").exists());
        let mut zip = zip::ZipWriter::new(File::create(&archive).unwrap());
        zip.start_file("native.dll", zip::write::SimpleFileOptions::default())
            .unwrap();
        zip.write_all(b"native").unwrap();
        zip.finish().unwrap();
        fs::write(target.join("native.dll"), b"keep").unwrap();
        assert!(extraer_nativos(&[(archive, vec![])], &target, &CancellationToken::new()).is_err());
        assert_eq!(fs::read(target.join("native.dll")).unwrap(), b"keep");
        fs::remove_dir_all(path).unwrap();
    }

    #[tokio::test]
    #[ignore = "Consulta manifiestos oficiales; no instala ni ejecuta el juego"]
    async fn metadatos_oficiales_actuales_y_legacy() {
        let client = crate::http::client().unwrap();
        let token = CancellationToken::new();
        let bytes = bytes_oficiales(
            &client,
            crate::manifest::VERSION_MANIFEST_URL,
            METADATA_LIMIT as usize,
            &token,
        )
        .await
        .unwrap();
        let manifest = crate::manifest::VersionManifest::parse(&bytes).unwrap();
        for id in [
            manifest.latest.release.as_str(),
            "1.21.1",
            "1.12.2",
            "1.6.4",
        ] {
            let entry = manifest.find(id).unwrap();
            let bytes = bytes_oficiales(&client, &entry.url, METADATA_LIMIT as usize, &token)
                .await
                .unwrap();
            assert_eq!(crate::manifest::sha1_hex(&bytes), entry.sha1);
            let version: Version = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(version.id, id);
            validar_descarga(version.downloads.get("client").unwrap()).unwrap();
            let assets = bytes_oficiales(
                &client,
                &version.asset_index.url,
                METADATA_LIMIT as usize,
                &token,
            )
            .await
            .unwrap();
            assert_eq!(assets.len() as u64, version.asset_index.size);
            assert_eq!(crate::manifest::sha1_hex(&assets), version.asset_index.sha1);
            let index: Indice = serde_json::from_slice(&assets).unwrap();
            for name in index.objects.keys() {
                relativa(name).unwrap();
            }
            for os in ["windows", "linux"] {
                if let Some(args) = &version.arguments {
                    argumentos(&args.game, os, "10.0.26100").unwrap();
                    argumentos(&args.jvm, os, "10.0.26100").unwrap();
                }
                for lib in &version.libraries {
                    if permitida(lib.rules.as_deref(), os, "10.0.26100").unwrap() {
                        if let Some(download) = &lib.downloads.artifact {
                            validar_descarga(download).unwrap();
                            relativa(&download.path).unwrap();
                        }
                        if let Some(classifier) = lib.natives.get(os) {
                            let native = lib
                                .downloads
                                .classifiers
                                .get(&classifier.replace("${arch}", "64"))
                                .unwrap();
                            validar_descarga(native).unwrap();
                            relativa(&native.path).unwrap();
                        }
                    }
                }
            }
        }
    }

    #[tokio::test]
    #[ignore = "Instala Fabric y Minecraft completos en una carpeta temporal, sin cuentas ni arranque"]
    async fn instala_fabric_real_y_reutiliza_sin_descargas() {
        struct Limpieza(PathBuf);
        impl Drop for Limpieza {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let root = Limpieza(temp());
        let store = Almacen::nuevo(root.0.join("instances"));
        let mut instance = store.crear("Fabric aislada", "1.21.1", 0).unwrap();
        instance.loader = "fabric".into();
        store.guardar(&instance).unwrap();
        crate::versiones_instancia::guardar_loader(&store, &instance, "0.16.14").unwrap();
        let first = instalar(&store, &instance, CancellationToken::new(), |_| {})
            .await
            .unwrap();
        assert!(listo(&store, &instance));
        assert_eq!(
            first.perfil_id.as_deref(),
            Some("fabric-loader-0.16.14-1.21.1")
        );
        assert_eq!(
            first.main_class,
            "net.fabricmc.loader.impl.launch.knot.KnotClient"
        );
        assert_eq!(first.java_major, 21);
        assert!(first.natives.is_dir());
        let second = instalar(&store, &instance, CancellationToken::new(), |p| {
            assert!(!p.etapa.starts_with("Descargando"))
        })
        .await
        .unwrap();
        assert_eq!(first.classpath, second.classpath);
        assert_eq!(first.natives, second.natives);
        let loader = first
            .classpath
            .iter()
            .find(|p| p.ends_with("fabric-loader-0.16.14.jar"))
            .unwrap();
        let mut zip = zip::ZipArchive::new(File::open(loader).unwrap()).unwrap();
        assert!(zip
            .by_name("net/fabricmc/loader/impl/launch/knot/KnotClient.class")
            .is_ok());
    }

    #[test]
    fn el_perfil_de_quilt_sin_argumentos_jvm_se_lee() {
        let perfil: PerfilCargador = serde_json::from_str(r#"{"id":"quilt-loader-0.30.1-1.21.4","inheritsFrom":"1.21.4","type":"release","mainClass":"org.quiltmc.loader.impl.launch.knot.KnotClient","arguments":{"game":[]},"libraries":[{"name":"org.quiltmc:quilt-loader:0.30.1","url":"https://maven.quiltmc.org/repository/release/"}]}"#).unwrap();
        assert!(perfil.arguments.jvm.is_empty() && perfil.arguments.game.is_empty());
        assert_eq!(perfil.libraries.len(), 1);
    }

    #[tokio::test]
    #[ignore = "Instala Quilt y Minecraft completos en una carpeta temporal, sin cuentas ni arranque"]
    async fn instala_quilt_real_y_reutiliza_sin_descargas() {
        struct Limpieza(PathBuf);
        impl Drop for Limpieza {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let root = Limpieza(temp());
        let store = Almacen::nuevo(root.0.join("instances"));
        let mut instance = store.crear("Quilt aislada", "1.21.1", 0).unwrap();
        instance.loader = "quilt".into();
        store.guardar(&instance).unwrap();
        crate::versiones_instancia::guardar_loader(&store, &instance, "0.30.1").unwrap();
        let first = instalar(&store, &instance, CancellationToken::new(), |_| {})
            .await
            .unwrap();
        assert!(listo(&store, &instance));
        assert_eq!(first.perfil_id.as_deref(), Some("quilt-loader-0.30.1-1.21.1"));
        assert_eq!(first.main_class, "org.quiltmc.loader.impl.launch.knot.KnotClient");
        assert_eq!(first.java_major, 21);
        assert!(first.natives.is_dir());
        let second = instalar(&store, &instance, CancellationToken::new(), |p| {
            assert!(!p.etapa.starts_with("Descargando"))
        })
        .await
        .unwrap();
        assert_eq!(first.classpath, second.classpath);
        let loader = first
            .classpath
            .iter()
            .find(|p| p.ends_with("quilt-loader-0.30.1.jar"))
            .unwrap();
        let mut zip = zip::ZipArchive::new(File::open(loader).unwrap()).unwrap();
        assert!(zip
            .by_name("org/quiltmc/loader/impl/launch/knot/KnotClient.class")
            .is_ok());
        for mapeo in ["hashed-1.21.1.jar", "intermediary-1.21.1.jar"] {
            assert!(first.classpath.iter().any(|p| p.ends_with(mapeo)), "{mapeo}");
        }
    }

    #[tokio::test]
    #[ignore = "Descarga Minecraft 1.21.1 completo desde Mojang en una carpeta temporal aislada; no inicia el juego ni usa cuentas"]
    async fn instala_vanilla_real_y_reutiliza_archivos_verificados() {
        struct Limpieza(PathBuf);
        impl Drop for Limpieza {
            fn drop(&mut self) {
                // temp() creó y devolvió exclusivamente esta carpeta de prueba.
                let _ = fs::remove_dir_all(&self.0);
            }
        }
        let root = Limpieza(temp());
        let store = Almacen::nuevo(root.0.join("instances"));
        let instance = store.crear("Vanilla aislada", "1.21.1", 0).unwrap();
        let before = fs::read(store.carpeta(&instance.id).unwrap().join("instance.json")).unwrap();
        let user_file = store.juego(&instance.id).unwrap().join("options.txt");
        fs::write(&user_file, b"usuario:conservar\n").unwrap();
        let progress = std::sync::Arc::new(Mutex::new(None));
        let observer = progress.clone();
        let first = instalar(&store, &instance, CancellationToken::new(), move |p| {
            *observer.lock().unwrap() = Some(p);
        })
        .await
        .unwrap();
        assert!(listo(&store, &instance));
        assert_eq!(first.version, "1.21.1");
        assert_eq!(first.java_major, 21);
        assert_eq!(first.java_component, "java-runtime-delta");
        assert_eq!(leer(&store, &instance).unwrap().natives, first.natives);
        let client = first.classpath.last().unwrap();
        assert_eq!(client.file_name().unwrap(), "client.jar");
        let mut zip = zip::ZipArchive::new(File::open(client).unwrap()).unwrap();
        assert!(zip.by_name("net/minecraft/client/main/Main.class").is_ok());
        drop(zip);
        let native = first
            .classpath
            .iter()
            .find(|path| {
                let name = path.file_name().unwrap().to_string_lossy();
                name.contains(if cfg!(windows) {
                    "natives-windows"
                } else {
                    "natives-linux"
                }) && !name.contains("-arm64")
                    && !name.contains("-x86")
            })
            .unwrap();
        let mut zip = zip::ZipArchive::new(File::open(native).unwrap()).unwrap();
        assert!((0..zip.len()).any(|i| {
            let member = zip.by_index(i).unwrap();
            let name = member.name().unwrap();
            name.ends_with(if cfg!(windows) { ".dll" } else { ".so" })
        }));
        drop(zip);
        assert!(first.natives.is_dir());
        let index: Indice = leer_json(
            first.assets.parent().unwrap(),
            &first
                .assets
                .join("indexes")
                .join(format!("{}.json", first.asset_index)),
        )
        .unwrap();
        assert!(!index.objects.is_empty());
        let object = index.objects.values().next().unwrap();
        let resource = first
            .assets
            .join("objects")
            .join(&object.hash[..2])
            .join(&object.hash);
        let actual = sha1_archivo(&resource, &CancellationToken::new()).unwrap();
        assert_eq!(actual, (object.size, object.hash.clone()));
        let client_time = fs::metadata(client).unwrap().modified().unwrap();
        let resource_time = fs::metadata(&resource).unwrap().modified().unwrap();
        let done = progress.lock().unwrap().clone().unwrap();
        assert!(done.total > 100);
        assert_eq!(done.completados, done.total);
        let second = instalar(&store, &instance, CancellationToken::new(), |_| {})
            .await
            .unwrap();
        assert_eq!(second.natives, first.natives);
        assert_eq!(second.classpath, first.classpath);
        assert_eq!(
            fs::metadata(client).unwrap().modified().unwrap(),
            client_time
        );
        assert_eq!(
            fs::metadata(resource).unwrap().modified().unwrap(),
            resource_time
        );
        assert_eq!(fs::read(&user_file).unwrap(), b"usuario:conservar\n");
        assert_eq!(
            fs::read(store.carpeta(&instance.id).unwrap().join("instance.json")).unwrap(),
            before
        );
        // Un archivo alterado con el mismo tamaño tampoco pasa la verificación.
        let mut jar = fs::read(client).unwrap();
        jar[0] ^= 0xff;
        fs::write(client, jar).unwrap();
        let repaired = instalar(&store, &instance, CancellationToken::new(), |_| {})
            .await
            .unwrap();
        assert!(listo(&store, &instance));
        assert_eq!(repaired.natives, first.natives);
        let mut zip = zip::ZipArchive::new(File::open(client).unwrap()).unwrap();
        assert!(zip.by_name("net/minecraft/client/main/Main.class").is_ok());
    }
