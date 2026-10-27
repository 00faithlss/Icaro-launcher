    use super::*;
    struct Fake {
        versions: BTreeMap<String, Version>,
        hashes: BTreeMap<String, Version>,
        bytes: BTreeMap<String, Vec<u8>>,
        bad: bool,
    }
    impl Fuente for Fake {
        async fn versiones_por_hash(&self, hashes: &[String]) -> Result<BTreeMap<String, Version>, Error> {
            Ok(self.hashes.iter().filter(|(h, _)| hashes.contains(h)).map(|(h,v)| (h.clone(),v.clone())).collect())
        }
        async fn version(&self, id: &str) -> Result<Version, Error> {
            self.versions.get(id).cloned().ok_or(Error::Datos)
        }
        async fn versiones(&self, project: &str, mc: &str, loader: &str) -> Result<Vec<Version>, Error> {
            Ok(self.versions.values().filter(|v| v.project_id == project && compatible(v,mc,loader)).cloned().collect())
        }
        async fn nombre(&self, id: &str) -> Result<String, Error> {
            Ok(id.into())
        }
        async fn archivo(&self, f: &ModFile) -> Result<Vec<u8>, Error> {
            if self.bad {
                Ok(b"corrupto".to_vec())
            } else {
                self.bytes.get(&f.filename).cloned().ok_or(Error::Datos)
            }
        }
    }
    fn fake() -> Fake {
        let mut f = Fake {
            versions: BTreeMap::new(),
            hashes: BTreeMap::new(),
            bytes: BTreeMap::new(),
            bad: false,
        };
        for (id, filename, body) in [
            ("v1", "example-1.jar", b"primero".as_slice()),
            ("v2", "example-2.jar", b"segundo".as_slice()),
        ] {
            f.versions.insert(
                id.into(),
                Version {
                    id: id.into(),
                    project_id: "example".into(),
                    name: "Example".into(),
                    version_number: id.into(),
                    version_type: "release".into(),
                    game_versions: vec!["1.20.1".into()],
                    loaders: vec!["fabric".into()],
                    files: vec![ModFile {
                        filename: filename.into(),
                        url: format!(
                            "https://cdn.modrinth.com/data/example/versions/{id}/{filename}"
                        ),
                        size: body.len() as u64,
                        primary: true,
                        hashes: crate::modrinth::Hashes { sha512: hash(body) },
                    }],
                    dependencies: vec![],
                    changelog: None,
                    date_published: String::new(),
                    downloads: 0,
                },
            );
            f.bytes.insert(filename.into(), body.to_vec());
        }
        f
    }
    struct Temp(PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn store() -> (Temp, Almacen, Instancia) {
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes).unwrap();
        let temp = Temp(std::env::temp_dir().join(format!("icaro-sync-test-{}", hash(&bytes))));
        let store = Almacen::nuevo(temp.0.join("instances"));
        let i = store
            .crear_con_loader("Amigos", "1.20.1", "fabric", 0)
            .unwrap();
        crate::versiones_instancia::guardar_loader(&store, &i, "0.16.14").unwrap();
        (temp, store, i)
    }
    fn index(f: &Fake, id: &str) -> Indice {
        let v = &f.versions[id];
        Indice {
            esquema: 1,
            minecraft: "1.20.1".into(),
            loader: "fabric".into(),
            loader_version: Some("0.16.14".into()),
            mods: vec![Entrada {
                proyecto: v.project_id.clone(),
                version: v.id.clone(),
                sha512: v.files[0].hashes.sha512.clone(),
            }],
            archivos: vec![],
            avisos: vec![],
        }
    }
    #[tokio::test]
    async fn actualiza_retira_solo_gestionados_y_conserva_archivos_locales() {
        let (_temp, store, i) = store();
        let f = fake();
        super::super::instalar(
            &f,
            &store,
            &i.id,
            vec![f.versions["v1"].clone()],
            &i.minecraft,
            &i.loader,
            |_| {},
        )
        .await
        .unwrap();
        let dir = store.juego(&i.id).unwrap().join("mods");
        fs::write(dir.join("local.jar"), b"personal").unwrap();
        let plan = preparar(&f, &store, &i, index(&f, "v2"), "127.0.0.1:25566".into())
            .await
            .unwrap();
        assert_eq!(plan.cambios.len(), 1);
        assert_eq!(plan.cambios[0].tipo, Tipo::Actualizar);
        assert_eq!(plan.conservados, vec!["local.jar"]);
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {})
            .await
            .unwrap();
        assert_eq!(fs::read(dir.join("example-2.jar")).unwrap(), b"segundo");
        assert!(!dir.join("example-1.jar").exists());
        let mut empty = index(&f, "v2");
        empty.mods.clear();
        let plan = preparar(&f, &store, &i, empty, "127.0.0.1:25566".into())
            .await
            .unwrap();
        assert_eq!(plan.cambios[0].tipo, Tipo::Quitar);
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {})
            .await
            .unwrap();
        assert!(!dir.join("example-2.jar").exists());
        assert_eq!(fs::read(dir.join("local.jar")).unwrap(), b"personal");
    }
    #[tokio::test]
    async fn dos_jar_importados_del_mismo_proyecto_no_rompen_el_inventario() {
        let (_temp, store, i) = store();
        let mut f = fake();
        let dir = store.juego(&i.id).unwrap().join("mods");
        fs::create_dir_all(&dir).unwrap();
        // Dos versiones de Sodium importadas como JAR locales, reconocidas por hash.
        for (id, archivo, cuerpo) in [("s1", "sodium-a.jar", b"sodium-a".as_slice()), ("s2", "sodium-b.jar", b"sodium-b".as_slice())] {
            let mut v = f.versions["v1"].clone();
            v.project_id = "AANobbMI".into();
            v.id = id.into();
            v.files[0].filename = archivo.into();
            v.files[0].size = cuerpo.len() as u64;
            v.files[0].hashes.sha512 = hash(cuerpo);
            f.hashes.insert(hash(cuerpo), v.clone());
            f.versions.insert(id.into(), v);
            fs::write(dir.join(archivo), cuerpo).unwrap();
        }
        let plan = preparar(&f, &store, &i, index(&f, "v1"), "127.0.0.1:25566".into()).await.unwrap();
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {}).await.unwrap();
        let registros: Vec<Instalado> = serde_json::from_slice(&fs::read(dir.join(META)).unwrap()).unwrap();
        assert_eq!(registros.iter().filter(|m| m.project_id == "AANobbMI").count(), 1);
        let inventario = super::super::inventario(&store, &i.id).unwrap();
        for archivo in ["sodium-a.jar", "sodium-b.jar", "example-1.jar"] {
            assert!(inventario.iter().any(|m| m.archivo == archivo), "{archivo}");
        }
        // Un inventario ya guardado con el proyecto repetido (0.3.1) se sigue leyendo.
        let mut duplicado = registros.clone();
        let mut otro = registros.iter().find(|m| m.project_id == "AANobbMI").unwrap().clone();
        otro.archivo = if otro.archivo == "sodium-a.jar" { "sodium-b.jar".into() } else { "sodium-a.jar".into() };
        otro.sha512 = hash(if otro.archivo == "sodium-a.jar" { b"sodium-a".as_slice() } else { b"sodium-b".as_slice() });
        duplicado.push(otro);
        fs::write(dir.join(META), serde_json::to_vec(&duplicado).unwrap()).unwrap();
        let inventario = super::super::inventario(&store, &i.id).unwrap();
        assert_eq!(inventario.iter().filter(|m| m.project_id == "AANobbMI").count(), 1);
        assert!(dir.join("sodium-a.jar").exists() && dir.join("sodium-b.jar").exists());
    }
    #[tokio::test]
    async fn sincroniza_litemoretica_sin_dependencias_declaradas_desde_cero() {
        for propio in [false, true] {
            let (_temp, store, i) = store();
            let mut f = fake();
            for (project, id, name) in [
                ("OYR5DMCo", "nhi2RkZ9", "litemoretica"),
                ("bEpr0Arc", "litematica1", "litematica"),
                ("GcWjdA9I", "malilib1", "malilib"),
            ] {
                let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
                writer.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default()).unwrap();
                write!(writer, "{{\"id\":\"{name}\",\"version\":\"1\",\"environment\":\"*\"}}").unwrap();
                let bytes = writer.finish().unwrap().into_inner();
                let mut v = f.versions["v1"].clone();
                v.project_id = project.into(); v.id = id.into();
                v.files[0].filename = format!("{name}.jar");
                v.files[0].size = bytes.len() as u64; v.files[0].hashes.sha512 = hash(&bytes);
                if name == "litematica" {
                    v.dependencies = serde_json::from_value(serde_json::json!([
                        {"project_id":"GcWjdA9I","dependency_type":"required"}
                    ])).unwrap();
                }
                f.hashes.insert(hash(&bytes), v.clone());
                f.bytes.insert(v.files[0].filename.clone(), bytes);
                f.versions.insert(id.into(), v);
            }
            let mut indice = index(&f, "nhi2RkZ9");
            let host = store.datos().unwrap().join("host");
            fs::create_dir_all(&host).unwrap();
            if propio {
                let v = &f.versions["nhi2RkZ9"];
                fs::write(host.join("litemoretica.jar"), &f.bytes["litemoretica.jar"]).unwrap();
                indice.esquema = 2; indice.mods.clear();
                indice.archivos.push(ArchivoPropio { id:"icaro-host-litemoretica".into(),
                    nombre:"Litemoretica".into(), version:"1.2.5".into(), archivo:"litemoretica.jar".into(),
                    bytes:v.files[0].size, sha512:v.files[0].hashes.sha512.clone() });
            }
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap().to_string();
            let cancel = CancellationToken::new();
            let servicio = tokio::spawn(servir_archivos(listener, Arc::new(RwLock::new(Some(indice.clone()))), cancel.clone(), Some(host)));
            let dir = store.juego(&i.id).unwrap().join("mods");
            assert!(!dir.exists());
            let plan = preparar(&f, &store, &i, indice.clone(), addr.clone()).await.unwrap();
            for project in ["bEpr0Arc", "GcWjdA9I"] {
                assert!(plan.cambios.iter().any(|c| c.proyecto == project && c.tipo == Tipo::Agregar));
            }
            aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_,_| {}).await.unwrap();
            for name in ["litemoretica.jar", "litematica.jar", "malilib.jar"] {
                assert!(dir.join(name).is_file());
            }
            let plan = preparar(&f, &store, &i, indice, addr).await.unwrap();
            assert!(plan.cambios.is_empty());
            cancel.cancel(); servicio.await.unwrap().unwrap();
        }
        // No extender la excepción a otros proyectos o versiones desconocidas.
        let mut v = fake().versions["v1"].clone();
        assert!(dependencias_cliente(&v).is_empty());
        v.project_id = "OYR5DMCo".into();
        assert!(dependencias_cliente(&v).is_empty());
    }

    #[tokio::test]
    #[ignore = "Descarga Litemoretica y dependencias reales en una instancia temporal; no inicia Minecraft"]
    async fn litemoretica_real_sincroniza_dependencias_cliente() {
        let (_temp, store, _) = store();
        let i = store.crear_con_loader("Cliente real", "1.21.1", "fabric", 0).unwrap();
        crate::versiones_instancia::guardar_loader(&store, &i, "0.19.5").unwrap();
        let source = Catalog::new().unwrap();
        let v = source.version("nhi2RkZ9").await.unwrap();
        assert_eq!(v.project_id, "OYR5DMCo");
        let f = v.files.iter().find(|f| f.primary).unwrap();
        let indice = Indice { esquema:1, minecraft:i.minecraft.clone(), loader:i.loader.clone(),
            loader_version:Some("0.19.5".into()), mods:vec![Entrada {
                proyecto:v.project_id.clone(), version:v.id.clone(), sha512:f.hashes.sha512.clone()
            }], archivos:vec![], avisos:vec![] };
        let plan = preparar(&source, &store, &i, indice.clone(), "127.0.0.1:25566".into()).await.unwrap();
        assert_eq!(plan.cambios.len(), 3);
        aplicar(&source, &store, &i, &plan, CancellationToken::new(), |_,_| {}).await.unwrap();
        let installed = inventario(&store, &i.id).unwrap();
        for project in ["OYR5DMCo", "bEpr0Arc", "GcWjdA9I"] {
            let m = installed.iter().find(|m| m.project_id == project).unwrap();
            let path = store.juego(&i.id).unwrap().join("mods").join(&m.archivo);
            assert_eq!(hash(&fs::read(&path).unwrap()), m.sha512);
            if project == "GcWjdA9I" {
                let mut jar = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
                assert!(jar.by_name("fi/dy/masa/malilib/event/InitializationHandler.class").is_ok());
            }
        }
        assert!(preparar(&source, &store, &i, indice, "127.0.0.1:25566".into()).await.unwrap().cambios.is_empty());
    }

    #[tokio::test]
    async fn indice_antiguo_recupera_bibliotecas_desde_cero_sin_cambiar_pins_del_anfitrion() {
        fn jar(id: &str, entorno: &str) -> Vec<u8> {
            let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
            writer.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default()).unwrap();
            write!(writer, "{{\"id\":\"{id}\",\"version\":\"1\",\"environment\":\"{entorno}\"}}").unwrap();
            writer.finish().unwrap().into_inner()
        }
        let (_temp, store, i) = store();
        let mut f = fake();
        let body = jar("raiz", "*");
        let mut root = f.versions["v1"].clone();
        root.id = "raiz1".into(); root.project_id = "raiz".into();
        root.files[0].filename = "raiz.jar".into();
        root.files[0].hashes.sha512 = hash(&body); root.files[0].size = body.len() as u64;
        root.dependencies = serde_json::from_value(serde_json::json!([
            {"project_id":"biblioteca","dependency_type":"required"},
            {"version_id":"otra1","dependency_type":"required"},
            {"project_id":"example","version_id":"v2","dependency_type":"required"}
        ])).unwrap();
        f.hashes.insert(hash(&body), root);
        for project in ["biblioteca", "otra"] {
            let bytes = jar(project, "*");
            let mut v = f.versions["v1"].clone();
            v.id = format!("{project}1"); v.project_id = project.into();
            v.files[0].filename = format!("{project}.jar");
            v.files[0].size = bytes.len() as u64; v.files[0].hashes.sha512 = hash(&bytes);
            f.bytes.insert(v.files[0].filename.clone(), bytes);
            f.versions.insert(v.id.clone(), v);
        }
        let host = store.datos().unwrap().join("host-mods");
        fs::create_dir_all(&host).unwrap(); fs::write(host.join("raiz.jar"), &body).unwrap();
        let mut indice = index(&f, "v1"); indice.esquema = 2;
        indice.archivos.push(ArchivoPropio { id:"icaro-host-raiz".into(), nombre:"Raíz".into(), version:"1".into(), archivo:"raiz.jar".into(), bytes:body.len() as u64, sha512:hash(&body) });
        let revision = indice.revision().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let cancel = CancellationToken::new();
        let servicio = tokio::spawn(servir_archivos(listener, Arc::new(RwLock::new(Some(indice.clone()))), cancel.clone(), Some(host)));
        let dir = store.juego(&i.id).unwrap().join("mods");
        assert!(!dir.exists());
        let plan = preparar(&f, &store, &i, indice.clone(), addr.to_string()).await.unwrap();
        assert_eq!(plan.revision_origen(), revision);
        assert_eq!(plan.indice.mods.len(), 3);
        assert!(plan.indice.mods.iter().any(|m| m.proyecto == "example" && m.version == "v1"));
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_,_| {}).await.unwrap();
        for name in ["raiz.jar", "biblioteca.jar", "otra.jar", "example-1.jar"] { assert!(dir.join(name).is_file()); }
        assert!(!dir.join("example-2.jar").exists());
        let plan = preparar(&f, &store, &i, indice.clone(), addr.to_string()).await.unwrap();
        assert!(plan.cambios.is_empty());
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_,_| panic!("No copiar un conjunto ya completo")).await.unwrap();
        let anterior = huella(&store, &i.id).unwrap();
        // El registro verificado caducó: la revisión vuelve a consultar Modrinth.
        fs::remove_file(store.juego(&i.id).unwrap().join(VERIFICADO)).unwrap();
        let server = jar("biblioteca", "server");
        let v = f.versions.get_mut("biblioteca1").unwrap();
        v.files[0].hashes.sha512 = hash(&server); v.files[0].size = server.len() as u64;
        f.bytes.insert("biblioteca.jar".into(), server);
        let plan = preparar(&f, &store, &i, indice, addr.to_string()).await.unwrap();
        assert!(aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_,_| {}).await.unwrap_err().contains("solo admite servidor"));
        assert_eq!(anterior, huella(&store, &i.id).unwrap());
        cancel.cancel(); servicio.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn sincronizar_conserva_mods_personales_de_catalogo_manuales_y_desactivados() {
        let (_temp, store, i) = store();
        let mut f = fake();
        let mut cliente = f.versions["v1"].clone();
        cliente.id = "cliente1".into();
        cliente.project_id = "cliente".into();
        cliente.files[0].filename = "cliente.jar".into();
        cliente.files[0].size = 7;
        cliente.files[0].hashes.sha512 = hash(b"cliente");
        f.bytes.insert("cliente.jar".into(), b"cliente".to_vec());
        f.versions.insert(cliente.id.clone(), cliente.clone());
        super::super::instalar(
            &f, &store, &i.id, vec![f.versions["v1"].clone(), cliente],
            &i.minecraft, &i.loader, |_| {},
        ).await.unwrap();
        let dir = store.juego(&i.id).unwrap().join("mods");
        fs::write(dir.join("manual.jar"), b"manual").unwrap();
        fs::write(dir.join("desactivado.jar.disabled"), b"desactivado").unwrap();
        // Primero registra el conjunto, después lo actualiza y finalmente retira
        // todos los mods del anfitrión. Los personales no pasan a ser gestionados.
        for version in [Some("v1"), Some("v2"), None] {
            let mut indice = index(&f, version.unwrap_or("v2"));
            if version.is_none() { indice.mods.clear(); }
            let plan = preparar(&f, &store, &i, indice, "127.0.0.1:25566".into()).await.unwrap();
            assert!(!plan.cambios.iter().any(|c| c.proyecto == "cliente"));
            aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {}).await.unwrap();
            assert_eq!(fs::read(dir.join("cliente.jar")).unwrap(), b"cliente");
            assert_eq!(fs::read(dir.join("manual.jar")).unwrap(), b"manual");
            assert_eq!(fs::read(dir.join("desactivado.jar.disabled")).unwrap(), b"desactivado");
            assert!(!dir.join("desactivado.jar").exists());
            assert!(inventario(&store, &i.id).unwrap().iter().any(|m| m.project_id == "cliente"));
            let gestionados: Gestionados = serde_json::from_slice(&fs::read(dir.join(SYNC)).unwrap()).unwrap();
            assert!(!gestionados.proyectos.contains("cliente"));
        }
        assert!(!dir.join("example-1.jar").exists());
        assert!(!dir.join("example-2.jar").exists());
    }

    #[tokio::test]
    async fn conjunto_al_dia_registra_el_origen_una_vez_y_no_vuelve_a_publicar() {
        let (_temp, store, i) = store();
        let f = fake();
        super::super::instalar(
            &f, &store, &i.id, vec![f.versions["v1"].clone()],
            &i.minecraft, &i.loader, |_| {},
        ).await.unwrap();
        let dir = store.juego(&i.id).unwrap().join("mods");
        fs::write(dir.join("personal.jar"), b"personal").unwrap();
        let mut indice = index(&f, "v1");
        indice.esquema = 2;
        indice.archivos.push(ArchivoPropio {
            id: "icaro-host-personal".into(), nombre: "Personal".into(), version: "1".into(),
            archivo: "personal.jar".into(), bytes: 8, sha512: hash(b"personal"),
        });
        let origen = "127.0.0.1:25566";
        let plan = preparar(&f, &store, &i, indice.clone(), origen.into()).await.unwrap();
        assert!(plan.cambios.is_empty());
        assert!(!dir.join(SYNC).exists());
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {}).await.unwrap();
        assert!(dir.join(SYNC).exists());
        let plan = preparar(&f, &store, &i, indice, origen.into()).await.unwrap();
        assert!(plan.cambios.is_empty());
        let registro = fs::metadata(dir.join(SYNC)).unwrap().modified().unwrap();
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {
            panic!("Un conjunto al día no debe volver a preparar archivos");
        }).await.unwrap();
        assert_eq!(registro, fs::metadata(dir.join(SYNC)).unwrap().modified().unwrap());
        fs::write(dir.join("personal.jar"), b"modificado").unwrap();
        assert!(aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {}).await.is_err());
    }

    #[tokio::test]
    async fn fallo_cancelacion_y_plan_obsoleto_no_sustituyen_el_conjunto() {
        let (_temp, store, i) = store();
        let mut f = fake();
        super::super::instalar(
            &f,
            &store,
            &i.id,
            vec![f.versions["v1"].clone()],
            &i.minecraft,
            &i.loader,
            |_| {},
        )
        .await
        .unwrap();
        let plan = preparar(&f, &store, &i, index(&f, "v2"), "host".into())
            .await
            .unwrap();
        let before = huella(&store, &i.id).unwrap();
        f.bad = true;
        assert!(
            aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {})
                .await
                .is_err()
        );
        assert_eq!(huella(&store, &i.id).unwrap(), before);
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert!(aplicar(&f, &store, &i, &plan, cancel, |_, _| {})
            .await
            .is_err());
        assert_eq!(huella(&store, &i.id).unwrap(), before);
        let dir = store.juego(&i.id).unwrap().join("mods");
        fs::write(dir.join("nuevo-local.jar"), b"nuevo").unwrap();
        assert!(
            aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {})
                .await
                .is_err()
        );
        assert_eq!(fs::read(dir.join("example-1.jar")).unwrap(), b"primero");
    }
    #[tokio::test]
    async fn modificar_un_mod_despues_de_revisar_invalida_el_plan_sin_tocarlo() {
        let (_temp, store, i) = store();
        let f = fake();
        super::super::instalar(&f, &store, &i.id, vec![f.versions["v1"].clone()], &i.minecraft, &i.loader, |_| {})
            .await
            .unwrap();
        let plan = preparar(&f, &store, &i, index(&f, "v2"), "host".into()).await.unwrap();
        let dir = store.juego(&i.id).unwrap().join("mods");
        fs::write(dir.join("example-1.jar"), b"editado por otra herramienta").unwrap();
        let error = aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {})
            .await
            .unwrap_err();
        assert!(error.contains("cambiaron"), "{error}");
        assert_eq!(fs::read(dir.join("example-1.jar")).unwrap(), b"editado por otra herramienta");
        assert!(!dir.join("example-2.jar").exists());
    }
    #[tokio::test]
    async fn analisis_local_previo_detecta_cambios_durante_la_consulta() {
        let (_temp, store, i) = store();
        let f = fake();
        super::super::instalar(&f, &store, &i.id, vec![f.versions["v1"].clone()], &i.minecraft, &i.loader, |_| {})
            .await
            .unwrap();
        let local = analizar(&store, &i).await.unwrap();
        let dir = store.juego(&i.id).unwrap().join("mods");
        fs::write(dir.join("nuevo-local.jar"), b"nuevo").unwrap();
        let error = preparar_desde(&f, &store, &i, index(&f, "v2"), "host".into(), local.clone())
            .await
            .unwrap_err();
        assert!(error.contains("cambiaron"), "{error}");
        let otra = store.crear_con_loader("Otra", "1.20.1", "fabric", 0).unwrap();
        assert!(preparar_desde(&f, &store, &otra, index(&f, "v2"), "host".into(), local).await.is_err());
    }
    #[tokio::test]
    async fn actualizar_no_copia_ni_recrea_los_archivos_no_afectados() {
        let (_temp, store, i) = store();
        let f = fake();
        let dir = store.juego(&i.id).unwrap().join("mods");
        let plan = preparar(&f, &store, &i, index(&f, "v1"), "host".into()).await.unwrap();
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {}).await.unwrap();
        fs::write(dir.join("personal.jar"), vec![7u8; 256 * 1024]).unwrap();
        let identidad = |nombre: &str| {
            let meta = fs::metadata(dir.join(nombre)).unwrap();
            estado_archivo(nombre.into(), &meta)
        };
        let personal = identidad("personal.jar");
        let plan = preparar(&f, &store, &i, index(&f, "v2"), "host".into()).await.unwrap();
        let avisos = std::sync::Mutex::new(vec![]);
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |id, hecho| avisos.lock().unwrap().push((id.to_owned(), hecho)))
            .await
            .unwrap();
        // El mismo archivo, no una copia: conserva creación, fecha e identidad.
        assert_eq!(identidad("personal.jar"), personal);
        assert!(dir.join("example-2.jar").exists() && !dir.join("example-1.jar").exists());
        assert_eq!(*avisos.lock().unwrap(), vec![("example".to_owned(), false), ("example".to_owned(), true)]);
        let juego = store.juego(&i.id).unwrap();
        assert!(!fs::read_dir(&juego).unwrap().any(|e| e.unwrap().file_name().to_string_lossy().starts_with(".icaro-mods-tx-")));
    }
    #[tokio::test]
    #[ignore = "Mide revisión y aplicación con 500 MiB de mods personales; escribe archivos temporales grandes"]
    async fn medir_actualizacion_con_carpeta_grande() {
        let (_temp, store, i) = store();
        let f = fake();
        let plan = preparar(&f, &store, &i, index(&f, "v1"), "host".into()).await.unwrap();
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {}).await.unwrap();
        let dir = store.juego(&i.id).unwrap().join("mods");
        let bloque = (0..2 * 1024 * 1024u32).map(|n| (n % 251) as u8).collect::<Vec<_>>();
        for n in 0..250 {
            let mut contenido = bloque.clone();
            contenido[..4].copy_from_slice(&(n as u32).to_le_bytes());
            fs::write(dir.join(format!("personal-{n:03}.jar")), contenido).unwrap();
            // Una instancia real tiene mods antiguos; los recién escritos no entran en el inventario.
            let hace = std::time::SystemTime::now() - Duration::from_secs(3600);
            fs::File::options().write(true).open(dir.join(format!("personal-{n:03}.jar"))).unwrap().set_modified(hace).unwrap();
        }
        let inicio = std::time::Instant::now();
        let plan = preparar(&f, &store, &i, index(&f, "v2"), "host".into()).await.unwrap();
        let revision = inicio.elapsed();
        let inicio = std::time::Instant::now();
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {}).await.unwrap();
        let aplicacion = inicio.elapsed();
        let inicio = std::time::Instant::now();
        let plan = preparar(&f, &store, &i, index(&f, "v2"), "host".into()).await.unwrap();
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {}).await.unwrap();
        let sin_cambios = inicio.elapsed();
        let inicio = std::time::Instant::now();
        let plan = preparar(&f, &store, &i, index(&f, "v2"), "host".into()).await.unwrap();
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {}).await.unwrap();
        let sin_cambios_2 = inicio.elapsed();
        println!("MEDICION revision={revision:?} aplicacion={aplicacion:?} jugar_sin_cambios={sin_cambios:?} jugar_sin_cambios_2={sin_cambios_2:?}");
    }
    fn transaccion(juego: &Path, confirmada: bool, movimientos: &[(&str, &str)]) -> PathBuf {
        let tx = juego.join(format!("{PREFIJO_TX}prueba"));
        fs::create_dir_all(tx.join("nuevos")).unwrap();
        fs::create_dir_all(tx.join("retirados")).unwrap();
        let movimientos = movimientos.iter().map(|(o, d)| Movimiento { origen: (*o).into(), destino: (*d).into() }).collect();
        escribir_diario(&tx, &Diario { esquema: 1, confirmada, movimientos }).unwrap();
        tx
    }
    #[test]
    fn cierre_abrupto_deshace_el_commit_parcial_al_abrir_la_instancia() {
        let (_temp, store, i) = store();
        let juego = store.juego(&i.id).unwrap();
        let mods = juego.join("mods");
        fs::create_dir_all(&mods).unwrap();
        fs::write(mods.join("personal.jar"), b"personal").unwrap();
        // Se retiró a.jar y se colocó b.jar; faltaba colocar c.jar.
        let tx = transaccion(&juego, false, &[
            ("mods/a.jar", ".icaro-mods-tx-prueba/retirados/0"),
            (".icaro-mods-tx-prueba/nuevos/1", "mods/b.jar"),
            (".icaro-mods-tx-prueba/nuevos/2", "mods/c.jar"),
        ]);
        fs::write(tx.join("retirados/0"), b"anterior").unwrap();
        fs::write(mods.join("b.jar"), b"nuevo").unwrap();
        fs::write(tx.join("nuevos/2"), b"pendiente").unwrap();
        let inventario = inventario(&store, &i.id).unwrap();
        assert_eq!(fs::read(mods.join("a.jar")).unwrap(), b"anterior");
        assert!(!mods.join("b.jar").exists() && !mods.join("c.jar").exists());
        assert_eq!(fs::read(mods.join("personal.jar")).unwrap(), b"personal");
        assert!(!tx.exists());
        assert_eq!(inventario.len(), 2);
    }
    #[test]
    fn commit_confirmado_no_se_deshace_y_un_archivo_ajeno_no_se_sobrescribe() {
        let (_temp, store, i) = store();
        let juego = store.juego(&i.id).unwrap();
        let mods = juego.join("mods");
        fs::create_dir_all(&mods).unwrap();
        let tx = transaccion(&juego, true, &[(".icaro-mods-tx-prueba/nuevos/0", "mods/b.jar")]);
        fs::write(mods.join("b.jar"), b"nuevo").unwrap();
        inventario(&store, &i.id).unwrap();
        assert_eq!(fs::read(mods.join("b.jar")).unwrap(), b"nuevo");
        assert!(!tx.exists());
        // Si alguien volvió a crear el original, la recuperación se detiene sin pisarlo.
        let tx = transaccion(&juego, false, &[("mods/a.jar", ".icaro-mods-tx-prueba/retirados/0")]);
        fs::write(tx.join("retirados/0"), b"anterior").unwrap();
        fs::write(mods.join("a.jar"), b"del jugador").unwrap();
        assert!(inventario(&store, &i.id).is_err());
        assert_eq!(fs::read(mods.join("a.jar")).unwrap(), b"del jugador");
        assert_eq!(fs::read(tx.join("retirados/0")).unwrap(), b"anterior");
    }
    #[test]
    fn fallo_durante_el_commit_restaura_lo_movido() {
        let (_temp, _store, _i) = store();
        let juego = _store.juego(&_i.id).unwrap();
        let mods = juego.join("mods");
        fs::create_dir_all(&mods).unwrap();
        let tx = transaccion(&juego, false, &[]);
        fs::write(mods.join("a.jar"), b"anterior").unwrap();
        fs::write(mods.join("ocupado.jar"), b"ajeno").unwrap();
        fs::write(tx.join("nuevos/1"), b"nuevo").unwrap();
        let movimientos = [
            Movimiento { origen: "mods/a.jar".into(), destino: ".icaro-mods-tx-prueba/retirados/0".into() },
            Movimiento { origen: ".icaro-mods-tx-prueba/nuevos/1".into(), destino: "mods/ocupado.jar".into() },
        ];
        let error = ejecutar(&juego, ".icaro-mods-tx-prueba", &movimientos).unwrap_err();
        assert!(error.contains("conserva"), "{error}");
        assert_eq!(fs::read(mods.join("a.jar")).unwrap(), b"anterior");
        assert_eq!(fs::read(mods.join("ocupado.jar")).unwrap(), b"ajeno");
        assert!(!tx.join(DIARIO).exists());
    }
    #[tokio::test]
    async fn el_registro_describe_plan_descargas_commit_errores_y_recuperacion() {
        // El registro es global: las demás pruebas pueden escribir aquí, por eso
        // se filtran las líneas por la instancia de esta prueba.
        let raiz = std::env::temp_dir().join(format!("icaro-sync-registro-{}", std::process::id()));
        activar_registro(&raiz);
        let (_temp, store, i) = store();
        let mut f = fake();
        let plan = preparar(&f, &store, &i, index(&f, "v1"), "host".into()).await.unwrap();
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {}).await.unwrap();
        let plan = preparar(&f, &store, &i, index(&f, "v2"), "host".into()).await.unwrap();
        f.bad = true;
        assert!(aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {}).await.is_err());
        let juego = store.juego(&i.id).unwrap();
        let tx = transaccion(&juego, false, &[("mods/perdido.jar", ".icaro-mods-tx-prueba/retirados/0")]);
        fs::write(tx.join("retirados/0"), b"anterior").unwrap();
        inventario(&store, &i.id).unwrap();
        let carpeta = juego.parent().unwrap().file_name().unwrap().to_string_lossy().into_owned();
        let registro = fs::read_to_string(raiz.join("logs/sincronizacion.log")).unwrap();
        let propias: Vec<_> = registro
            .lines()
            .filter(|l| l.contains(&format!("instancia={} ", i.id)) || l.contains(&format!("carpeta={carpeta} ")))
            .collect();
        for esperado in [
            "revision-local instancia=",
            "plan instancia=",
            "agregar=1",
            "aplicar-inicio instancia=",
            "descarga instancia=",
            "fuente=modrinth",
            "commit instancia=",
            "aplicar-fin instancia=",
            "error fase=aplicar instancia=",
            "accion=deshecha movimientos=1",
        ] {
            assert!(propias.iter().any(|l| l.contains(esperado)), "falta {esperado}:\n{}", propias.join("\n"));
        }
        assert!(propias.iter().all(|l| !l.contains(&*raiz.to_string_lossy())), "el registro no debe incluir rutas absolutas");
    }
    #[test]
    fn el_diario_solo_acepta_rutas_de_la_transaccion() {
        let juego = Path::new("juego");
        for valida in ["mods/a.jar", "mods/a.jar.disabled", "mods/.icaro-mods.json", ".icaro-mods-tx-x/nuevos/3", ".icaro-mods-conflictos-x/a.jar"] {
            assert!(ruta_diario(juego, valida).is_ok(), "{valida}");
        }
        for invalida in ["../a.jar", "mods/../a.jar", "mods/sub/a.jar", "C:/a.jar", "mods/a\\b.jar", "options.txt", "saves/mundo/level.dat", ".icaro-mods-tx-x/nuevos/a", ".icaro-mods-tx-x/otros/1", "/mods/a.jar", ""] {
            assert!(ruta_diario(juego, invalida).is_err(), "{invalida}");
        }
    }
    #[tokio::test]
    async fn la_segunda_revision_reutiliza_el_inventario_y_detecta_mods_modificados() {
        let (_temp, store, i) = store();
        let f = fake();
        let plan = preparar(&f, &store, &i, index(&f, "v1"), "host".into()).await.unwrap();
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {}).await.unwrap();
        let dir = store.juego(&i.id).unwrap().join("mods");
        fs::write(dir.join("personal.jar"), b"mod personal").unwrap();
        let envejecer = |nombre: &std::ffi::OsStr| {
            let hace = std::time::SystemTime::now() - Duration::from_secs(3600);
            fs::File::options().write(true).open(dir.join(nombre)).unwrap().set_modified(hace).unwrap();
        };
        for entrada in fs::read_dir(&dir).unwrap() {
            envejecer(&entrada.unwrap().file_name());
        }
        let primera = analizar(&store, &i).await.unwrap();
        assert_eq!(primera.reutilizados, 0);
        let segunda = analizar(&store, &i).await.unwrap();
        assert_eq!((segunda.releidos, segunda.reutilizados), (0, primera.releidos));
        assert_eq!(segunda.huella, primera.huella);
        assert_eq!(segunda.identidades, primera.identidades);
        // Un mod gestionado editado se relee y la revisión lo rechaza como antes.
        fs::write(dir.join("example-1.jar"), b"editado").unwrap();
        envejecer(std::ffi::OsStr::new("example-1.jar"));
        let error = analizar(&store, &i).await.unwrap_err();
        assert!(error.contains("fue modificado"), "{error}");
    }
    #[tokio::test]
    async fn las_descargas_respetan_el_limite_y_se_detienen_con_el_primer_error() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let (activas, maximo) = (AtomicUsize::new(0), AtomicUsize::new(0));
        let trabajo = |n: usize, falla: bool| -> (String, Trabajo<'_>) {
            let (activas, maximo) = (&activas, &maximo);
            (n.to_string(), Box::pin(async move {
                let ahora = activas.fetch_add(1, Ordering::SeqCst) + 1;
                maximo.fetch_max(ahora, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(20)).await;
                activas.fetch_sub(1, Ordering::SeqCst);
                if falla { Err(format!("falló {n}")) } else { Ok(n.to_string()) }
            }))
        };
        let avisos = std::sync::Mutex::new(vec![]);
        let aviso = |id: &str, hecho: bool| avisos.lock().unwrap().push((id.to_owned(), hecho));
        en_paralelo((0..10).map(|n| trabajo(n, false)).collect(), 4, &aviso).await.unwrap();
        assert_eq!(maximo.load(Ordering::SeqCst), 4);
        let terminados = avisos.lock().unwrap().iter().filter(|(_, hecho)| *hecho).count();
        assert_eq!(terminados, 10);

        // Con un error, el resto no llega a empezar.
        avisos.lock().unwrap().clear();
        let trabajos = (0..10).map(|n| trabajo(n, n == 1)).collect();
        assert_eq!(en_paralelo(trabajos, 2, &aviso).await.unwrap_err(), "falló 1");
        assert!(avisos.lock().unwrap().iter().all(|(id, _)| id.parse::<usize>().unwrap() < 3));
    }
    /// Envía una solicitud por una conexión ya abierta y lee cabeceras y cuerpo
    /// según Content-Length, sin esperar a que el servicio cierre la conexión.
    async fn pedir(conexion: &mut tokio::net::TcpStream, solicitud: &str) -> (String, Vec<u8>) {
        conexion.write_all(solicitud.as_bytes()).await.unwrap();
        let mut datos = vec![];
        let fin = loop {
            if let Some(fin) = datos.windows(4).position(|w| w == b"\r\n\r\n") { break fin + 4; }
            let mut buf = [0u8; 4096];
            let n = conexion.read(&mut buf).await.unwrap();
            assert!(n > 0, "conexión cerrada antes de las cabeceras");
            datos.extend_from_slice(&buf[..n]);
        };
        let cabeceras = String::from_utf8_lossy(&datos[..fin]).into_owned();
        let largo = cabeceras.lines().find_map(|l| l.strip_prefix("Content-Length: ")).map_or(0, |v| v.trim().parse::<usize>().unwrap());
        let mut cuerpo = datos[fin..].to_vec();
        while cuerpo.len() < largo {
            let mut buf = [0u8; 4096];
            let n = conexion.read(&mut buf).await.unwrap();
            assert!(n > 0);
            cuerpo.extend_from_slice(&buf[..n]);
        }
        (cabeceras, cuerpo)
    }
    #[tokio::test]
    async fn el_indice_responde_304_y_varias_solicitudes_comparten_conexion() {
        let f = fake();
        let indice = index(&f, "v2");
        let etiqueta = format!("\"{}\"", indice.revision().unwrap());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let cancel = CancellationToken::new();
        let servicio = tokio::spawn(servir(listener, Arc::new(RwLock::new(Some(indice.clone()))), cancel.clone()));
        let mut conexion = tokio::net::TcpStream::connect(addr).await.unwrap();
        let (cabeceras, cuerpo) = pedir(&mut conexion, &format!("GET {RUTA} HTTP/1.1\r\nHost: x\r\n\r\n")).await;
        assert!(cabeceras.starts_with("HTTP/1.1 200") && cabeceras.contains(&format!("ETag: {etiqueta}")), "{cabeceras}");
        assert_eq!(serde_json::from_slice::<Indice>(&cuerpo).unwrap().revision().unwrap(), indice.revision().unwrap());
        // Misma conexión, índice sin cambios: 304 sin cuerpo.
        let (cabeceras, cuerpo) = pedir(&mut conexion, &format!("GET {RUTA} HTTP/1.1\r\nHost: x\r\nIf-None-Match: {etiqueta}\r\n\r\n")).await;
        assert!(cabeceras.starts_with("HTTP/1.1 304") && cuerpo.is_empty(), "{cabeceras}");
        // El cliente guarda la etiqueta y reutiliza su copia en la siguiente consulta.
        assert_eq!(consultar(addr, CancellationToken::new()).await.unwrap().revision().unwrap(), indice.revision().unwrap());
        assert_eq!(consultar(addr, CancellationToken::new()).await.unwrap().revision().unwrap(), indice.revision().unwrap());
        cancel.cancel();
        servicio.await.unwrap().unwrap();
    }
    #[tokio::test]
    async fn los_archivos_propios_admiten_rangos_validados_y_rechazan_rangos_imposibles() {
        let (_temp, store, _i) = store();
        let mods = store.datos().unwrap().join("servidor-mods");
        fs::create_dir_all(&mods).unwrap();
        let contenido = b"0123456789abcdef".to_vec();
        fs::write(mods.join("propio.jar"), &contenido).unwrap();
        let sha = hash(&contenido);
        let mut indice = index(&fake(), "v1");
        indice.esquema = 2;
        indice.archivos.push(ArchivoPropio { id: "icaro-host-propio".into(), nombre: "Propio".into(), version: "1".into(), archivo: "propio.jar".into(), bytes: contenido.len() as u64, sha512: sha.clone() });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let cancel = CancellationToken::new();
        let servicio = tokio::spawn(servir_archivos(listener, Arc::new(RwLock::new(Some(indice))), cancel.clone(), Some(mods.clone())));
        let ruta = format!("/icaro/v1/files/{sha}");
        let mut conexion = tokio::net::TcpStream::connect(addr).await.unwrap();
        let (cabeceras, cuerpo) = pedir(&mut conexion, &format!("GET {ruta} HTTP/1.1\r\nHost: x\r\nRange: bytes=10-\r\nIf-Range: \"{sha}\"\r\n\r\n")).await;
        assert!(cabeceras.starts_with("HTTP/1.1 206") && cabeceras.contains("Content-Range: bytes 10-15/16"), "{cabeceras}");
        assert_eq!(cuerpo, b"abcdef");
        // Un validador distinto entrega el archivo completo, nunca un tramo de otro contenido.
        let (cabeceras, cuerpo) = pedir(&mut conexion, &format!("GET {ruta} HTTP/1.1\r\nHost: x\r\nRange: bytes=10-\r\nIf-Range: \"otro\"\r\n\r\n")).await;
        assert!(cabeceras.starts_with("HTTP/1.1 200"), "{cabeceras}");
        assert_eq!(cuerpo, contenido);
        let (cabeceras, _) = pedir(&mut conexion, &format!("GET {ruta} HTTP/1.1\r\nHost: x\r\nRange: bytes=99-\r\n\r\n")).await;
        assert!(cabeceras.starts_with("HTTP/1.1 416") && cabeceras.contains("Content-Range: bytes */16"), "{cabeceras}");
        // Un archivo cambiado tras publicar no se entrega con el hash anterior.
        fs::write(mods.join("propio.jar"), b"0123456789ABCDEF").unwrap();
        let mut conexion = tokio::net::TcpStream::connect(addr).await.unwrap();
        let (cabeceras, _) = pedir(&mut conexion, &format!("GET {ruta} HTTP/1.1\r\nHost: x\r\n\r\n")).await;
        assert!(cabeceras.starts_with("HTTP/1.1 410"), "{cabeceras}");
        cancel.cancel();
        servicio.await.unwrap().unwrap();
    }
    /// Anfitrión de prueba que corta la primera descarga a la mitad. Con `rangos`
    /// atiende la reanudación con 206; sin ellos responde 200, como un servicio antiguo.
    async fn anfitrion_que_corta(contenido: Vec<u8>, rangos: bool) -> (SocketAddr, Arc<std::sync::Mutex<Vec<String>>>, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let solicitudes = Arc::new(std::sync::Mutex::new(vec![]));
        let registro = solicitudes.clone();
        let tarea = tokio::spawn(async move {
            for intento in 0.. {
                let (mut s, _) = listener.accept().await.unwrap();
                let mut datos = vec![];
                while !datos.windows(4).any(|w| w == b"\r\n\r\n") {
                    let mut buf = [0u8; 1024];
                    let n = s.read(&mut buf).await.unwrap();
                    if n == 0 { break; }
                    datos.extend_from_slice(&buf[..n]);
                }
                let texto = String::from_utf8_lossy(&datos).into_owned();
                registro.lock().unwrap().push(texto.clone());
                let total = contenido.len();
                let desde = texto.lines().find_map(|l| l.strip_prefix("range: bytes=").or_else(|| l.strip_prefix("Range: bytes=")))
                    .and_then(|r| r.trim_end_matches('-').parse::<usize>().ok());
                let (cabecera, cuerpo) = match desde.filter(|_| rangos) {
                    Some(d) => (format!("HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {d}-{}/{total}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", total - 1, total - d), contenido[d..].to_vec()),
                    None => (format!("HTTP/1.1 200 OK\r\nContent-Length: {total}\r\nConnection: close\r\n\r\n"), contenido.clone()),
                };
                s.write_all(cabecera.as_bytes()).await.unwrap();
                // La primera respuesta se corta tras la mitad del cuerpo.
                let enviar = if intento == 0 { &cuerpo[..cuerpo.len() / 2] } else { &cuerpo[..] };
                s.write_all(enviar).await.unwrap();
            }
        });
        (addr, solicitudes, tarea)
    }
    #[tokio::test]
    async fn una_descarga_cortada_del_anfitrion_se_reanuda_o_reinicia_sin_mezclar() {
        let contenido: Vec<u8> = (0..200_000u32).map(|n| (n % 251) as u8).collect();
        let archivo = ArchivoPropio { id: "icaro-host-grande".into(), nombre: "Grande".into(), version: "1".into(), archivo: "grande.jar".into(), bytes: contenido.len() as u64, sha512: hash(&contenido) };
        let carpeta = std::env::temp_dir().join(format!("icaro-reanudar-{}", hash(&contenido)[..16].to_owned()));
        fs::create_dir_all(&carpeta).unwrap();
        for rangos in [true, false] {
            let (addr, solicitudes, tarea) = anfitrion_que_corta(contenido.clone(), rangos).await;
            // El origen es el puerto de Minecraft + 1: el del servicio de prueba.
            let origen = addr.to_string();
            let destino = carpeta.join(format!("rangos-{rangos}"));
            descargar_propio_a(&origen, &archivo, &destino).await.unwrap();
            assert_eq!(fs::read(&destino).unwrap(), contenido);
            let pedidas = solicitudes.lock().unwrap().clone();
            assert_eq!(pedidas.len(), 2, "{pedidas:?}");
            let segunda = pedidas[1].to_ascii_lowercase();
            assert!(segunda.contains("range: bytes=") && segunda.contains(&format!("if-range: \"{}\"", archivo.sha512)), "{segunda}");
            tarea.abort();
        }
        fs::remove_dir_all(carpeta).unwrap();
    }
    #[test]
    fn el_host_reutiliza_hashes_de_mods_sin_cambios() {
        let (_temp, store, _i) = store();
        let carpeta = store.datos().unwrap().join("servidor");
        let mods = carpeta.join("mods");
        fs::create_dir_all(&mods).unwrap();
        fs::write(mods.join("a.jar"), b"uno").unwrap();
        let hace = std::time::SystemTime::now() - Duration::from_secs(3600);
        fs::File::options().write(true).open(mods.join("a.jar")).unwrap().set_modified(hace).unwrap();
        let servidor = dedicado::ServidorDedicado { carpeta, minecraft: "1.20.1".into(), loader: "fabric".into(), loader_version: "0.16.14".into(), excluir: BTreeSet::new(), incluir: BTreeSet::new(), incluir_servidor: false };
        assert_eq!(servidor.huella().unwrap(), fingerprint(&mods).unwrap());
        assert_eq!(dedicado::sha_vigente(&mods, "a.jar"), Some(hash(b"uno")));
        // Un cambio invalida el hash guardado y la siguiente huella lo relee.
        fs::write(mods.join("a.jar"), b"dos").unwrap();
        assert_eq!(dedicado::sha_vigente(&mods, "a.jar"), None);
        assert_eq!(servidor.huella().unwrap(), fingerprint(&mods).unwrap());
    }
    /// Fuente que falla ante cualquier consulta: prueba que no se usó la red.
    struct SinRed;
    impl Fuente for SinRed {
        async fn version(&self, _: &str) -> Result<Version, Error> { Err(Error::Datos) }
        async fn versiones(&self, _: &str, _: &str, _: &str) -> Result<Vec<Version>, Error> { Err(Error::Datos) }
        async fn nombre(&self, _: &str) -> Result<String, Error> { Err(Error::Datos) }
        async fn archivo(&self, _: &ModFile) -> Result<Vec<u8>, Error> { Err(Error::Datos) }
        async fn versiones_por_hash(&self, _: &[String]) -> Result<BTreeMap<String, Version>, Error> { Err(Error::Datos) }
        async fn solo_cliente(&self, _: &str) -> Result<bool, Error> { Err(Error::Datos) }
    }
    #[tokio::test]
    async fn sincronizacion_verificada_se_confirma_sin_red_hasta_que_cambia_la_carpeta() {
        let (_temp, store, i) = store();
        let f = fake();
        let plan = preparar(&f, &store, &i, index(&f, "v1"), "host".into()).await.unwrap();
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {}).await.unwrap();
        let juego = store.juego(&i.id).unwrap();
        assert!(juego.join(VERIFICADO).exists());
        let antes = huella(&store, &i.id).unwrap();

        // Mismo anfitrión y misma carpeta: plan vacío sin una sola consulta.
        let plan = preparar(&SinRed, &store, &i, index(&f, "v1"), "host".into()).await.unwrap();
        assert!(plan.sin_red && plan.cambios.is_empty());
        aplicar(&SinRed, &store, &i, &plan, CancellationToken::new(), |_, _| {}).await.unwrap();
        assert_eq!(huella(&store, &i.id).unwrap(), antes);

        // Otro origen o una revisión distinta vuelven al camino normal.
        assert!(preparar(&SinRed, &store, &i, index(&f, "v1"), "otro".into()).await.is_err());
        assert!(preparar(&SinRed, &store, &i, index(&f, "v2"), "host".into()).await.is_err());

        // Un mod personal nuevo invalida el atajo y un plan sin red obsoleto se rechaza.
        let dir = juego.join("mods");
        fs::write(dir.join("personal.jar"), b"nuevo").unwrap();
        assert!(preparar(&SinRed, &store, &i, index(&f, "v1"), "host".into()).await.is_err());
        let error = aplicar(&SinRed, &store, &i, &plan, CancellationToken::new(), |_, _| {}).await.unwrap_err();
        assert!(error.contains("cambiaron") || error.contains("Comprueba de nuevo"), "{error}");
        assert!(dir.join("personal.jar").exists() && dir.join("example-1.jar").exists());

        // Con la red, el plan normal conserva el mod personal y renueva el registro.
        let plan = preparar(&f, &store, &i, index(&f, "v1"), "host".into()).await.unwrap();
        assert!(!plan.sin_red && plan.cambios.is_empty());
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {}).await.unwrap();
        assert!(preparar(&SinRed, &store, &i, index(&f, "v1"), "host".into()).await.unwrap().sin_red);

        // Pasadas 24 h el registro caduca y se vuelve a confirmar con Modrinth.
        let mut registro = leer_verificado(&juego).unwrap();
        registro.fecha -= VIGENCIA_VERIFICADO;
        fs::write(juego.join(VERIFICADO), serde_json::to_vec(&registro).unwrap()).unwrap();
        assert!(preparar(&SinRed, &store, &i, index(&f, "v1"), "host".into()).await.is_err());
    }
    #[tokio::test]
    async fn descartar_la_revision_local_detiene_el_trabajo_bloqueante() {
        let (aviso, detenido) = std::sync::mpsc::channel();
        let espera = tokio::time::timeout(
            Duration::from_millis(50),
            bloqueante(move |token| {
                let inicio = std::time::Instant::now();
                while !token.is_cancelled() && inicio.elapsed() < Duration::from_secs(10) {
                    std::thread::sleep(Duration::from_millis(5));
                }
                let _ = aviso.send(token.is_cancelled());
                Ok(())
            }),
        )
        .await;
        assert!(espera.is_err());
        assert!(detenido.recv_timeout(Duration::from_secs(5)).unwrap());
    }
    #[tokio::test]
    async fn apogee_renombrado_y_duplicado_se_actualiza_por_id_y_hash_sin_perder_mods_personales() {
        fn jar(id: &str, contenido: &str) -> Vec<u8> {
            let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
            writer
                .start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
                .unwrap();
            write!(
                writer,
                "{{\"id\":\"{id}\",\"version\":\"1.1.1\",\"environment\":\"*\"}}"
            )
            .unwrap();
            writer
                .start_file("registros.txt", zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(contenido.as_bytes()).unwrap();
            writer.finish().unwrap().into_inner()
        }
        for exacto_presente in [false, true] {
            let (_temp, store, i) = store();
            let f = fake();
            let game = store.juego(&i.id).unwrap();
            let dir = game.join("mods");
            fs::create_dir(&dir).unwrap();
            let viejo = jar("apogee", "registros anteriores");
            let nuevo = jar("apogee", "23 registros nuevos del servidor");
            let personal = jar("solo_cliente", "personal");
            fs::write(dir.join("apogee-1.1.1.jar"), &viejo).unwrap();
            fs::write(dir.join("apogee-copia.jar"), &viejo).unwrap();
            fs::write(dir.join("apogee-antiguo.jar.disabled"), &viejo).unwrap();
            fs::write(dir.join("personal.jar"), &personal).unwrap();
            if exacto_presente {
                fs::write(dir.join("apogee-1.2.0.jar"), &nuevo).unwrap();
            }
            let host = game.join("host");
            fs::create_dir(&host).unwrap();
            fs::write(host.join("apogee-1.2.0.jar"), &nuevo).unwrap();
            let mut indice = index(&f, "v1");
            indice.esquema = 2;
            indice.mods.clear();
            indice.archivos.push(ArchivoPropio {
                id: "icaro-host-apogee".into(),
                nombre: "Apogee".into(),
                version: "1.1.1".into(),
                archivo: "apogee-1.2.0.jar".into(),
                bytes: nuevo.len() as u64,
                sha512: hash(&nuevo),
            });
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap().to_string();
            let cancel = CancellationToken::new();
            let servicio = tokio::spawn(servir_archivos(
                listener,
                Arc::new(RwLock::new(Some(indice.clone()))),
                cancel.clone(),
                Some(host),
            ));
            let plan = preparar(&f, &store, &i, indice.clone(), addr.clone())
                .await
                .unwrap();
            assert!(plan.puede_aplicar() && plan.conflictos.is_empty());
            assert_eq!(plan.reemplazos.len(), 2);
            if !exacto_presente {
                assert!(plan.cambios.iter().any(|c| c.tipo == Tipo::Actualizar));
            }
            aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {})
                .await
                .unwrap();
            assert_eq!(fs::read(dir.join("apogee-1.2.0.jar")).unwrap(), nuevo);
            assert!(!dir.join("apogee-1.1.1.jar").exists() && !dir.join("apogee-copia.jar").exists());
            assert_eq!(fs::read(dir.join("personal.jar")).unwrap(), personal);
            assert!(dir.join("apogee-antiguo.jar.disabled").exists());
            let respaldo = fs::read_dir(&game)
                .unwrap()
                .flatten()
                .find(|e| {
                    e.file_name()
                        .to_string_lossy()
                        .starts_with(".icaro-mods-conflictos-")
                })
                .unwrap()
                .path();
            assert_eq!(fs::read(respaldo.join("apogee-1.1.1.jar")).unwrap(), viejo);
            let plan = preparar(&f, &store, &i, indice, addr).await.unwrap();
            assert!(plan.cambios.is_empty() && plan.reemplazos.is_empty());
            cancel.cancel();
            servicio.await.unwrap().unwrap();
        }
    }

    #[tokio::test]
    async fn conflicto_local_muestra_el_plan_y_solo_reemplaza_con_autorizacion_y_respaldo() {
        let (_temp, store, i) = store();
        let f = fake();
        let game = store.juego(&i.id).unwrap();
        let dir = game.join("mods");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("survivor.jar"), b"mi mod personal").unwrap();
        fs::write(dir.join("cliente.jar"), b"solo cliente").unwrap();
        let host = game.join("host");
        fs::create_dir(&host).unwrap();
        fs::write(host.join("survivor.jar"), b"mod del anfitrion").unwrap();
        let mut indice = index(&f, "v2");
        indice.esquema = 2;
        indice.archivos.push(ArchivoPropio {
            id: "icaro-host-survivor".into(),
            nombre: "Survivor".into(),
            version: "2".into(),
            archivo: "survivor.jar".into(),
            bytes: 17,
            sha512: hash(b"mod del anfitrion"),
        });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let cancel = CancellationToken::new();
        let servicio = tokio::spawn(servir_archivos(
            listener,
            Arc::new(RwLock::new(Some(indice.clone()))),
            cancel.clone(),
            Some(host),
        ));
        let mut plan = preparar(&f, &store, &i, indice.clone(), addr.clone())
            .await
            .unwrap();
        assert_eq!(plan.conflictos, ["survivor.jar"]);
        assert_eq!(plan.cambios.len(), 2);
        assert!(!plan.puede_aplicar());
        assert!(
            aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {})
                .await
                .is_err()
        );
        assert_eq!(
            fs::read(dir.join("survivor.jar")).unwrap(),
            b"mi mod personal"
        );
        plan.autorizar_reemplazos(true);
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {})
            .await
            .unwrap();
        assert_eq!(
            fs::read(dir.join("survivor.jar")).unwrap(),
            b"mod del anfitrion"
        );
        assert_eq!(fs::read(dir.join("cliente.jar")).unwrap(), b"solo cliente");
        let respaldo = fs::read_dir(&game)
            .unwrap()
            .flatten()
            .find(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with(".icaro-mods-conflictos-")
            })
            .unwrap()
            .path();
        assert_eq!(
            fs::read(respaldo.join("survivor.jar")).unwrap(),
            b"mi mod personal"
        );
        let plan = preparar(&f, &store, &i, indice, addr).await.unwrap();
        assert!(plan.conflictos.is_empty() && plan.cambios.is_empty());
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {})
            .await
            .unwrap();
        assert!(respaldo.join("survivor.jar").is_file());
        cancel.cancel();
        servicio.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn no_adivina_versiones_ni_sobrescribe_nombres_locales() {
        let (_temp, store, i) = store();
        let f = fake();
        let mut invalid = index(&f, "v2");
        invalid.mods[0].sha512 = "a".repeat(128);
        assert!(preparar(&f, &store, &i, invalid, "host".into())
            .await
            .is_err());
        let mut invalid = index(&f, "v2");
        invalid.loader_version = Some("0.16.13".into());
        assert!(preparar(&f, &store, &i, invalid, "host".into())
            .await
            .is_err());
        let dir = store.juego(&i.id).unwrap().join("mods");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("example-2.jar"), b"local").unwrap();
        let plan = preparar(&f, &store, &i, index(&f, "v2"), "host".into())
            .await
            .unwrap();
        assert!(
            aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {})
                .await
                .is_err()
        );
        assert_eq!(fs::read(dir.join("example-2.jar")).unwrap(), b"local");
    }
    #[tokio::test]
    async fn servicio_solo_entrega_indice_y_rechaza_rutas_y_metodos() {
        let f = fake();
        let value = index(&f, "v2");
        let publication = Arc::new(RwLock::new(Some(value.clone())));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let cancel = CancellationToken::new();
        let worker = tokio::spawn(servir(listener, publication.clone(), cancel.clone()));
        assert_eq!(
            consultar(addr, CancellationToken::new())
                .await
                .unwrap()
                .revision()
                .unwrap(),
            value.revision().unwrap()
        );
        for request in [
            "POST /icaro/v1/mods HTTP/1.1\r\nHost: localhost\r\n\r\n",
            "GET /../instance.json HTTP/1.1\r\nHost: localhost\r\n\r\n",
        ] {
            let mut c = tokio::net::TcpStream::connect(addr).await.unwrap();
            c.write_all(request.as_bytes()).await.unwrap();
            let mut response = vec![];
            c.read_to_end(&mut response).await.unwrap();
            assert!(response.starts_with(b"HTTP/1.1 404"));
        }
        *publication.write().await = None;
        assert!(consultar(addr, CancellationToken::new()).await.is_err());
        cancel.cancel();
        worker.await.unwrap().unwrap();
        assert!(endpoint("8.8.8.8").is_err());
        assert!(endpoint("0.0.0.0").is_err());
        assert!(endpoint("10.147.17.1:65535").is_err());
        assert_eq!(endpoint("10.147.17.1").unwrap().port(), 25566);
    }
    #[tokio::test]
    #[ignore = "Descarga dos mods reales de Modrinth en instancias temporales aisladas"]
    async fn modrinth_real_publica_indice_http_y_sincroniza_contenido_exacto() {
        let (_host, host, hi) = store();
        let (_client, client, ci) = store();
        let catalog = Catalog::new().unwrap();
        let api = catalog
            .versions("fabric-api", Some("1.20.1"), Some("fabric"))
            .await
            .unwrap()
            .into_iter()
            .find(|v| v.version_type == "release")
            .unwrap();
        let sodium = catalog
            .versions("sodium", Some("1.20.1"), Some("fabric"))
            .await
            .unwrap()
            .into_iter()
            .find(|v| v.version_type == "release")
            .unwrap();
        super::super::instalar(
            &catalog,
            &host,
            &hi.id,
            vec![api.clone(), sodium.clone()],
            &hi.minecraft,
            &hi.loader,
            |_| {},
        )
        .await
        .unwrap();
        let value = publicar(&catalog, &host, &hi).await.unwrap();
        assert!(value.mods.iter().any(|m| m.proyecto == api.project_id));
        assert!(!value.mods.iter().any(|m| m.proyecto == sodium.project_id));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let token = CancellationToken::new();
        let worker = tokio::spawn(servir(
            listener,
            Arc::new(RwLock::new(Some(value))),
            token.clone(),
        ));
        let received = consultar(addr, CancellationToken::new()).await.unwrap();
        let plan = preparar(&catalog, &client, &ci, received, addr.to_string())
            .await
            .unwrap();
        aplicar(
            &catalog,
            &client,
            &ci,
            &plan,
            CancellationToken::new(),
            |_, _| {},
        )
        .await
        .unwrap();
        let installed = inventario(&client, &ci.id).unwrap();
        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].version_id, api.id);
        let host_file = host
            .juego(&hi.id)
            .unwrap()
            .join("mods")
            .join(&installed[0].archivo);
        let client_file = client
            .juego(&ci.id)
            .unwrap()
            .join("mods")
            .join(&installed[0].archivo);
        assert_eq!(
            hash_archivo(&host_file).unwrap(),
            hash_archivo(&client_file).unwrap()
        );
        token.cancel();
        worker.await.unwrap().unwrap();
    }
