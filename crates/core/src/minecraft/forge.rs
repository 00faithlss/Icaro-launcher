// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Ejecuta el instalador oficial en una raíz privada, sin cuentas ni mundos.
use super::*;
use std::process::Stdio;

mod buffer;
mod progreso;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Perfil {
    pub id: String,
    inherits_from: String,
    pub main_class: String,
    #[serde(default)]
    arguments: Argumentos,
    #[serde(default)]
    minecraft_arguments: Option<String>,
    libraries: Vec<Libreria>,
}

#[derive(Serialize, Deserialize)]
struct ArchivoLocal {
    ruta: String,
    size: u64,
    sha1: String,
}

#[derive(Serialize, Deserialize)]
struct Cache {
    schema: u32,
    minecraft: String,
    familia: String,
    loader: String,
    sistema: String,
    instalador: Descarga,
    carpeta: String,
    archivos: Vec<ArchivoLocal>,
}

pub(super) struct Preparado {
    pub perfil: Perfil,
    pub carpeta: PathBuf,
}

fn coordenada(familia: &str, mc: &str, loader: &str) -> Result<String, Error> {
    if !version_minecraft_valida(mc) || !nombre_valido(loader) {
        return Err(Error::Datos);
    }
    match familia {
        "forge" => Ok(format!("https://maven.minecraftforge.net/net/minecraftforge/forge/{mc}-{loader}/forge-{mc}-{loader}-installer.jar")),
        "neoforge" => Ok(format!("https://maven.neoforged.net/releases/net/neoforged/neoforge/{loader}/neoforge-{loader}-installer.jar")),
        _ => Err(Error::Datos),
    }
}

fn json_jar(path: &Path, entry: &str) -> Result<serde_json::Value, Error> {
    let mut jar = zip::ZipArchive::new(File::open(path).map_err(|_| Error::Disco)?)
        .map_err(|_| Error::Datos)?;
    let mut file = jar.by_name(entry).map_err(|_| {
        Error::NoSoportada(
            "el instalador no publica el formato de perfil de cliente admitido".into(),
        )
    })?;
    if file.size() > METADATA_LIMIT {
        return Err(Error::Datos);
    }
    let mut bytes = Vec::new();
    file.by_ref()
        .take(METADATA_LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Disco)?;
    if bytes.len() as u64 > METADATA_LIMIT {
        return Err(Error::Datos);
    }
    serde_json::from_slice(&bytes).map_err(|_| Error::Datos)
}

fn perfil_instalador(path: &Path, mc: &str, familia: &str, loader: &str) -> Result<Perfil, Error> {
    let install = json_jar(path, "install_profile.json")?;
    if install["minecraft"].as_str() != Some(mc) || install["spec"].as_u64() != Some(1) {
        return Err(Error::NoSoportada(
            "este instalador requiere un formato anterior o distinto al admitido".into(),
        ));
    }
    let entry = install["json"]
        .as_str()
        .ok_or(Error::Datos)?
        .trim_start_matches('/');
    relativa(entry)?;
    let profile: Perfil =
        serde_json::from_value(json_jar(path, entry)?).map_err(|_| Error::Datos)?;
    let expected = match familia {
        "forge" => format!("{mc}-forge-{loader}"),
        "neoforge" => format!("neoforge-{loader}"),
        _ => return Err(Error::Datos),
    };
    if profile.id != expected
        || install["version"].as_str() != Some(profile.id.as_str())
        || profile.inherits_from != mc
        || !nombre_valido(&profile.id)
        || profile.main_class.is_empty()
        || profile.main_class.len() > 256
        || !profile
            .main_class
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'$'))
        || profile.libraries.is_empty()
        || profile.libraries.len() > 4096
    {
        return Err(Error::Datos);
    }
    Ok(profile)
}

// Las coordenadas oficiales incluyen clasificadores (client, universal, etc.).
fn ruta_libreria(library: &Libreria) -> Result<PathBuf, Error> {
    let parts: Vec<_> = library.name.split(':').collect();
    if !(3..=4).contains(&parts.len())
        || parts.iter().any(|p| {
            p.is_empty()
                || matches!(*p, "." | "..")
                || !p
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-' | b'+'))
        })
    {
        return Err(Error::Datos);
    }
    if parts[0].split('.').any(|p| p.is_empty()) {
        return Err(Error::Datos);
    }
    let classifier = parts.get(3).map(|c| format!("-{c}")).unwrap_or_default();
    let path = format!(
        "{}/{}/{}/{}-{}{classifier}.jar",
        parts[0].replace('.', "/"),
        parts[1],
        parts[2],
        parts[1],
        parts[2]
    );
    if let Some(artifact) = &library.downloads.artifact {
        if artifact.path != path {
            return Err(Error::Datos);
        }
    }
    relativa(&path)
}

fn comprobar_librerias(
    folder: &Path,
    profile: &Perfil,
    cancel: &CancellationToken,
) -> Result<(), Error> {
    for lib in &profile.libraries {
        let path = folder.join("libraries").join(ruta_libreria(lib)?);
        segura(folder, &path, false)?;
        let (size, hash) = sha1_archivo(&path, cancel)?;
        if let Some(expected) = &lib.downloads.artifact {
            if !hash_valido(&expected.sha1) || size != expected.size || hash != expected.sha1 {
                return Err(Error::Integridad);
            }
        }
    }
    Ok(())
}

fn inventario(
    folder: &Path,
    dir: &Path,
    cancel: &CancellationToken,
    result: &mut Vec<ArchivoLocal>,
) -> Result<(), Error> {
    segura(folder, dir, false)?;
    for entry in fs::read_dir(dir).map_err(|_| Error::Disco)? {
        cancelado(cancel)?;
        let path = entry.map_err(|_| Error::Disco)?.path();
        segura(folder, &path, false)?;
        let meta = fs::symlink_metadata(&path).map_err(|_| Error::Disco)?;
        if enlace(&meta) {
            return Err(Error::Datos);
        }
        if meta.is_dir() {
            inventario(folder, &path, cancel, result)?;
        } else if meta.is_file() {
            if result.len() >= 8192 || meta.len() > FILE_LIMIT {
                return Err(Error::Datos);
            }
            let (size, sha1) = sha1_archivo(&path, cancel)?;
            result.push(ArchivoLocal {
                ruta: path
                    .strip_prefix(folder)
                    .map_err(|_| Error::Datos)?
                    .to_str()
                    .ok_or(Error::Datos)?
                    .replace('\\', "/"),
                size,
                sha1,
            });
        } else {
            return Err(Error::Datos);
        }
    }
    Ok(())
}

fn reutilizar_cache(
    root: &Path,
    cache_root: &Path,
    cache: &Cache,
    mc: &str,
    familia: &str,
    loader: &str,
    cancel: &CancellationToken,
) -> Result<Preparado, Error> {
    if cache.schema != 1
        || cache.minecraft != mc
        || cache.familia != familia
        || cache.loader != loader
        || cache.sistema != format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
        || !nombre_valido(&cache.carpeta)
        || !cache.carpeta.starts_with("preparacion-")
        || cache.archivos.is_empty()
        || cache.archivos.len() > 8192
        || cache.instalador.url != coordenada(familia, mc, loader)?
    {
        return Err(Error::Datos);
    }
    let installer = cache_root.join("installer.jar");
    if !verificado(root, &installer, &cache.instalador, cancel)? {
        return Err(Error::Integridad);
    }
    let profile = perfil_instalador(&installer, mc, familia, loader)?;
    let folder = cache_root.join(&cache.carpeta);
    let client_relative = format!("versions/{0}/{0}.jar", profile.id);
    if !cache
        .archivos
        .iter()
        .any(|file| file.ruta == client_relative)
    {
        return Err(Error::Datos);
    }
    for file in &cache.archivos {
        let path = folder.join(relativa(&file.ruta)?);
        segura(root, &path, false)?;
        if !hash_valido(&file.sha1)
            || sha1_archivo(&path, cancel)? != (file.size, file.sha1.clone())
        {
            return Err(Error::Integridad);
        }
    }
    comprobar_librerias(&folder, &profile, cancel)?;
    Ok(Preparado {
        perfil: profile,
        carpeta: folder,
    })
}

pub(super) async fn preparar(
    client: &reqwest::Client,
    root: &Path,
    instance: &Instancia,
    selected: &str,
    version_json: &Path,
    vanilla_jar: &Path,
    java_profile: &crate::java::Perfil,
    java_options: (Option<&Path>, bool),
    cancel: &CancellationToken,
    progress: &(impl Fn(Progreso) + Sync),
) -> Result<Preparado, Error> {
    let url = coordenada(&instance.loader, &instance.minecraft, selected)?;
    let cache_root = root
        .join("meta/loaders")
        .join(&instance.loader)
        .join(&instance.minecraft)
        .join(selected);
    let cache_path = cache_root.join("instalado.json");
    if let Ok(cache) = leer_json::<Cache>(root, &cache_path) {
        match reutilizar_cache(
            root,
            &cache_root,
            &cache,
            &instance.minecraft,
            &instance.loader,
            selected,
            cancel,
        ) {
            Ok(ready) => return Ok(ready),
            Err(Error::Cancelada) => return Err(Error::Cancelada),
            Err(_) => {}
        }
    }
    progress(Progreso {
        etapa: format!("Descargando instalador oficial de {}", instance.loader),
        completados: 0,
        total: 0,
    });
    let hash_bytes = bytes_oficiales(client, &format!("{url}.sha1"), 256, cancel).await?;
    let hash = std::str::from_utf8(&hash_bytes)
        .map_err(|_| Error::Datos)?
        .split_whitespace()
        .next()
        .ok_or(Error::Datos)?
        .to_lowercase();
    if !hash_valido(&hash) {
        return Err(Error::Datos);
    }
    let installer = cache_root.join("installer.jar");
    let existing = fs::metadata(&installer)
        .ok()
        .map(|m| m.len())
        .filter(|n| *n > 0 && *n <= 64 * 1024 * 1024);
    let size = if let Some(size) =
        existing.filter(|_| sha1_archivo(&installer, cancel).is_ok_and(|(_, h)| h == hash))
    {
        size
    } else {
        let response = tokio::select! {biased; _=cancel.cancelled()=>return Err(Error::Cancelada), r=client.head(oficial(&url)?).send()=>r.map_err(|_|Error::Red)?};
        if !response.status().is_success() {
            return Err(Error::Http(response.status().as_u16()));
        }
        response
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|n| *n > 0 && *n <= 64 * 1024 * 1024)
            .ok_or(Error::Datos)?
    };
    let download = Descarga {
        url,
        sha1: hash,
        size,
        path: String::new(),
        id: String::new(),
    };
    descargar(
        client,
        root,
        &nuevo_archivo(&download, installer.clone(), "Instalador")?,
        cancel,
    )
    .await?;
    let profile = perfil_instalador(&installer, &instance.minecraft, &instance.loader, selected)?;
    let runtime = crate::java::preparar_perfil(
        root,
        java_profile,
        java_options.0,
        java_options.1,
        cancel.clone(),
        |p| {
            progress(Progreso {
                etapa: p.etapa.into(),
                completados: p.completados,
                total: p.total,
            })
        },
    )
    .await
    .map_err(|e| match e {
        crate::java::Error::Cancelada => Error::Cancelada,
        _ => Error::NoSoportada(e.to_string()),
    })?;
    let name = format!("preparacion-{}", random()?);
    let folder = cache_root.join(&name);
    segura(root, &folder, true)?;
    fs::create_dir(&folder).map_err(|_| Error::Disco)?;
    let vanilla = folder.join("versions").join(&instance.minecraft);
    segura(root, &vanilla.join("placeholder"), true)?;
    fs::copy(
        version_json,
        vanilla.join(format!("{}.json", instance.minecraft)),
    )
    .map_err(|_| Error::Disco)?;
    // Copia independiente: el instalador puede modificar sus entradas.
    fs::copy(
        vanilla_jar,
        vanilla.join(format!("{}.jar", instance.minecraft)),
    )
    .map_err(|_| Error::Disco)?;
    escribir_json(
        &folder,
        &folder.join("launcher_profiles.json"),
        &serde_json::json!({"profiles":{}}),
    )?;
    let log = File::create(folder.join("instalador.log")).map_err(|_| Error::Disco)?;
    let err = log.try_clone().map_err(|_| Error::Disco)?;
    let mut command = tokio::process::Command::new(&runtime.executable);
    if instance.loader == "forge" {
        progress(Progreso {
            etapa: "Preparando el instalador de Forge".into(),
            completados: 0,
            total: 0,
        });
        if let Some(argument) = buffer::preparar(&folder, &runtime.executable, cancel).await? {
            command.arg(argument);
        }
    }
    command
        .arg("-Djava.awt.headless=true")
        .arg("-jar")
        .arg(crate::lanzamiento::ruta_java(
            installer.to_str().ok_or(Error::Datos)?,
        ))
        .arg("--installClient")
        .arg(crate::lanzamiento::ruta_java(
            folder.to_str().ok_or(Error::Datos)?,
        ))
        .current_dir(&folder)
        .stdin(Stdio::null())
        .stdout(log)
        .stderr(err)
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000);
    progress(Progreso {
        etapa: format!("Instalando {} y procesando el cliente", instance.loader),
        completados: 0,
        total: 0,
    });
    let mut child = command.spawn().map_err(|_| Error::Disco)?;
    let status = progreso::esperar(
        &mut child,
        &folder.join("instalador.log"),
        &instance.loader,
        cancel,
        progress,
    )
    .await?;
    if !status.success() {
        return Err(Error::Instalador(instance.loader.clone()));
    }
    progress(Progreso {
        etapa: format!("Verificando archivos preparados de {}", instance.loader),
        completados: 0,
        total: 0,
    });
    comprobar_librerias(&folder, &profile, cancel)?;
    let output = folder
        .join("versions")
        .join(&profile.id)
        .join(format!("{}.json", profile.id));
    let installed: serde_json::Value = leer_json(&folder, &output)?;
    let source: serde_json::Value = json_jar(
        &installer,
        json_jar(&installer, "install_profile.json")?["json"]
            .as_str()
            .ok_or(Error::Datos)?
            .trim_start_matches('/'),
    )?;
    if installed != source {
        return Err(Error::Integridad);
    }
    // El launcher oficial usa el nombre del perfil efectivo para el JAR
    // heredado. Forge lo excluye del escaneo con ${version_name}.jar; dejar
    // nuestro nombre genérico client.jar introduce las clases sin parchear.
    let launch_jar = folder
        .join("versions")
        .join(&profile.id)
        .join(format!("{}.jar", profile.id));
    fs::copy(vanilla_jar, &launch_jar).map_err(|_| Error::Disco)?;
    if sha1_archivo(vanilla_jar, cancel)? != sha1_archivo(&launch_jar, cancel)? {
        return Err(Error::Integridad);
    }
    let mut files = Vec::new();
    inventario(&folder, &folder.join("libraries"), cancel, &mut files)?;
    inventario(&folder, &folder.join("versions"), cancel, &mut files)?;
    cancelado(cancel)?;
    let cache = Cache {
        schema: 1,
        minecraft: instance.minecraft.clone(),
        familia: instance.loader.clone(),
        loader: selected.into(),
        sistema: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        instalador: download,
        carpeta: name,
        archivos: files,
    };
    escribir_json(root, &cache_path, &cache)?;
    Ok(Preparado {
        perfil: profile,
        carpeta: folder,
    })
}

impl Preparado {
    pub fn combinar(
        &self,
        base: &[Libreria],
        classpath: &mut Vec<PathBuf>,
        jvm: &mut Vec<String>,
        game: &mut Vec<String>,
        os: &str,
        os_version: &str,
    ) -> Result<(), Error> {
        let client = classpath.last_mut().ok_or(Error::Datos)?;
        *client = self
            .carpeta
            .join("versions")
            .join(&self.perfil.id)
            .join(format!("{}.jar", self.perfil.id));
        let names: BTreeSet<_> = self
            .perfil
            .libraries
            .iter()
            .map(|l| l.name.split(':').take(2).collect::<Vec<_>>().join(":"))
            .collect();
        for lib in base {
            if names.contains(&lib.name.split(':').take(2).collect::<Vec<_>>().join(":")) {
                if let Some(artifact) = &lib.downloads.artifact {
                    let path = relativa(&artifact.path)?;
                    classpath.retain(|p| !p.ends_with(&path));
                }
            }
        }
        let mut loader = Vec::new();
        for lib in &self.perfil.libraries {
            if permitida(lib.rules.as_deref(), os, os_version)? {
                loader.push(self.carpeta.join("libraries").join(ruta_libreria(lib)?));
            }
        }
        loader.append(classpath);
        *classpath = loader;
        jvm.extend(argumentos(&self.perfil.arguments.jvm, os, os_version)?);
        let libraries = crate::lanzamiento::ruta_java(
            self.carpeta
                .join("libraries")
                .to_str()
                .ok_or(Error::Datos)?,
        );
        for argument in jvm {
            *argument = argument.replace("${library_directory}", &libraries);
        }
        if let Some(legacy) = &self.perfil.minecraft_arguments {
            *game = legacy.split_whitespace().map(str::to_owned).collect();
        } else {
            game.extend(argumentos(&self.perfil.arguments.game, os, os_version)?);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FORGE: &str = include_str!("../../tests/fixtures/loaders/forge-version.json");
    const NEOFORGE: &str = include_str!("../../tests/fixtures/loaders/neoforge-version.json");
    const NEOFORGE26: &str = include_str!("../../tests/fixtures/loaders/neoforge26-version.json");

    fn temp() -> PathBuf {
        let root = std::env::temp_dir().join(format!("icaro-forge-test-{}", random().unwrap()));
        fs::create_dir(&root).unwrap();
        root
    }

    fn jar(root: &Path, profile: &str) -> PathBuf {
        let value: serde_json::Value = serde_json::from_str(profile).unwrap();
        let path = root.join("installer.jar");
        let mut jar = zip::ZipWriter::new(File::create(&path).unwrap());
        let options = zip::write::SimpleFileOptions::default();
        jar.start_file("install_profile.json", options).unwrap();
        jar.write_all(serde_json::to_string(&serde_json::json!({
            "spec":1,"minecraft":value["inheritsFrom"],"version":value["id"],"json":"/version.json"
        })).unwrap().as_bytes()).unwrap();
        jar.start_file("version.json", options).unwrap();
        jar.write_all(profile.as_bytes()).unwrap();
        jar.finish().unwrap();
        path
    }

    #[test]
    fn valida_perfiles_oficiales_y_rechaza_cruces_de_minecraft_y_loader() {
        for (text, mc, family, version) in [
            (FORGE, "1.20.1", "forge", "47.4.10"),
            (NEOFORGE, "1.21.1", "neoforge", "21.1.219"),
            (NEOFORGE26, "26.3", "neoforge", "26.3.0.95"),
        ] {
            let root = temp();
            let path = jar(&root, text);
            assert!(perfil_instalador(&path, mc, family, version).is_ok());
            assert!(perfil_instalador(&path, "1.21.4", family, version).is_err());
            assert!(perfil_instalador(&path, mc, family, "999").is_err());
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn mezcla_modulos_y_argumentos_sin_bibliotecas_duplicadas() {
        let profile: Perfil = serde_json::from_str(FORGE).unwrap();
        let base:Libreria=serde_json::from_value(serde_json::json!({
            "name":"org.ow2.asm:asm:9.5","downloads":{"artifact":{
                "path":"org/ow2/asm/asm/9.5/asm-9.5.jar","url":"https://libraries.minecraft.net/asm.jar","sha1":"0".repeat(40),"size":1
            }}
        })).unwrap();
        let old = PathBuf::from("cache/libraries/hash/org/ow2/asm/asm/9.5/asm-9.5.jar");
        let client = PathBuf::from("cache/client.jar");
        let mut cp = vec![old.clone(), client.clone()];
        let mut jvm = vec!["-cp".into(), "${classpath}".into()];
        let mut game = vec!["--username".into(), "${auth_player_name}".into()];
        let ready = Preparado {
            perfil: profile,
            carpeta: PathBuf::from("cache con espacios/forge"),
        };
        ready
            .combinar(&[base], &mut cp, &mut jvm, &mut game, "windows", "10.0")
            .unwrap();
        assert!(!cp.contains(&old));
        assert!(!cp.contains(&client));
        assert!(cp
            .iter()
            .any(|p| p.ends_with("versions/1.20.1-forge-47.4.10/1.20.1-forge-47.4.10.jar")));
        assert!(cp
            .iter()
            .any(|p| p.ends_with("org/ow2/asm/asm/9.8/asm-9.8.jar")));
        assert!(jvm.contains(&"-p".into()));
        assert!(jvm
            .iter()
            .any(|a| a.contains("cache con espacios") && a.contains("bootstraplauncher")));
        assert!(!jvm.iter().any(|a| a.contains("${library_directory}")));
        assert!(game
            .windows(2)
            .any(|p| p == ["--fml.forgeVersion", "47.4.10"]));
        assert_eq!(&game[..2], ["--username", "${auth_player_name}"]);
    }

    #[test]
    fn rechaza_rutas_y_clasificadores_maven_inseguros() {
        for name in [
            "net.minecraftforge:forge:../escape:client",
            "net..minecraft:client:1",
            "a:b:c:../../x",
            "a:b:c:d:e",
        ] {
            let lib: Libreria = serde_json::from_value(serde_json::json!({"name":name})).unwrap();
            assert!(ruta_libreria(&lib).is_err());
        }
        let lib: Libreria = serde_json::from_value(
            serde_json::json!({"name":"net.minecraftforge:forge:1.20.1-47.4.10:client"}),
        )
        .unwrap();
        assert!(ruta_libreria(&lib)
            .unwrap()
            .ends_with("forge-1.20.1-47.4.10-client.jar"));
    }

    #[tokio::test]
    async fn cancelar_antes_de_preparar_no_descarga_ni_publica_cache() {
        let root = temp().canonicalize().unwrap();
        let store = Almacen::nuevo(root.join("instances"));
        let instance = store
            .crear_con_loader("Forge", "1.20.1", "forge", 0)
            .unwrap();
        let cancel = CancellationToken::new();
        cancel.cancel();
        let client = reqwest::Client::new();
        let profile = crate::java::Perfil {
            majors: vec![17],
            component: "java-runtime-gamma".into(),
            prism: false,
        };
        assert!(matches!(
            preparar(
                &client,
                &root,
                &instance,
                "47.4.10",
                &root.join("missing.json"),
                &root.join("missing.jar"),
                &profile,
                (None, false),
                &cancel,
                &|_| {}
            )
            .await,
            Err(Error::Cancelada)
        ));
        assert!(!root.join("meta/loaders").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    #[ignore = "Descarga clientes, Java e instaladores oficiales en Temp; no usa cuentas ni ejecuta partidas"]
    async fn instaladores_oficiales_procesan_cliente_y_reutilizan_cache_verificada() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/qa-loaders");
        fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(root.join(".prueba.lock"))
            .unwrap();
        let _lock = lock.try_lock().unwrap();
        let store = Almacen::nuevo(root.join("instances"));
        let cancel = CancellationToken::new();
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(180))
            .user_agent(crate::http::USER_AGENT)
            .build()
            .unwrap();
        let manifest = crate::manifest::fetch_manifest(&client).await.unwrap();
        for (mc, family, loader) in [
            ("1.20.1", "forge", "47.4.10"),
            ("26.3", "forge", "66.1.0"),
            ("1.21.1", "neoforge", "21.1.219"),
            ("26.3", "neoforge", "26.3.0.95"),
        ] {
            let name = format!("{family}-{mc}");
            let i = store
                .cargar()
                .unwrap()
                .instancias
                .into_iter()
                .find(|i| i.nombre == name)
                .unwrap_or_else(|| store.crear_con_loader(&name, mc, family, 0).unwrap());
            let entry = manifest.find(mc).unwrap();
            let bytes = bytes_oficiales(&client, &entry.url, METADATA_LIMIT as usize, &cancel)
                .await
                .unwrap();
            assert_eq!(crate::manifest::sha1_hex(&bytes), entry.sha1);
            let version: Version = serde_json::from_slice(&bytes).unwrap();
            let version_path = root.join(format!("{mc}.json"));
            fs::write(&version_path, bytes).unwrap();
            let client_path = root.join(format!("{mc}.jar"));
            descargar(
                &client,
                &root,
                &nuevo_archivo(
                    version.downloads.get("client").unwrap(),
                    client_path.clone(),
                    "Cliente",
                )
                .unwrap(),
                &cancel,
            )
            .await
            .unwrap();
            let java = version.java_version.unwrap();
            let runtime = crate::java::Perfil {
                majors: vec![java.major_version],
                component: java.component,
                prism: false,
            };
            let phase = Mutex::new(String::new());
            let ready = preparar(
                &client,
                &root,
                &i,
                loader,
                &version_path,
                &client_path,
                &runtime,
                (None, true),
                &cancel,
                &|p| {
                    let mut phase = phase.lock().unwrap();
                    if *phase != p.etapa {
                        println!("{}", p.etapa);
                        *phase = p.etapa;
                    }
                },
            )
            .await
            .unwrap();
            assert_eq!(ready.perfil.inherits_from, mc);
            if family == "forge" && mc == "26.3" && loader == "66.1.0" {
                use sha2::Digest;
                let path = ready.carpeta.join(
                    "libraries/net/minecraftforge/forge/26.3-66.1.0/forge-26.3-66.1.0-client.jar",
                );
                // Resultado del procesador oficial sin el adaptador de E/S.
                // La optimización debe conservar incluso los bytes del ZIP.
                let hash = sha2::Sha256::digest(fs::read(path).unwrap());
                assert_eq!(
                    format!("{hash:x}"),
                    "4338d02f85af70578bd56aef24b195cad6d4984abedd5459f95f5e814d6adc0d"
                );
            }
            let offline = reqwest::Client::builder()
                .proxy(reqwest::Proxy::all("http://127.0.0.1:9").unwrap())
                .timeout(Duration::from_secs(2))
                .build()
                .unwrap();
            let cached = preparar(
                &offline,
                &root,
                &i,
                loader,
                &version_path,
                &client_path,
                &runtime,
                (None, true),
                &cancel,
                &|_| panic!("La caché válida no descarga"),
            )
            .await
            .unwrap();
            assert_eq!(cached.carpeta, ready.carpeta);
            let first = &ready.perfil.libraries[0];
            let path = ready
                .carpeta
                .join("libraries")
                .join(ruta_libreria(first).unwrap());
            let original = fs::read(&path).unwrap();
            fs::write(&path, b"alterado").unwrap();
            assert!(preparar(
                &offline,
                &root,
                &i,
                loader,
                &version_path,
                &client_path,
                &runtime,
                (None, true),
                &cancel,
                &|_| {}
            )
            .await
            .is_err());
            fs::write(path, original).unwrap();
            println!("Cliente {family} {loader} preparado y caché comprobada");
        }
        // Se conservan en target los instaladores, Java y registros de esta
        // prueba explícita para poder repetirla sin descargas innecesarias.
    }
}
