// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Java local y runtimes oficiales de Mojang, sin instalar nada en el sistema.
//!
//! Fuente del catálogo: el mismo manifiesto de runtimes que utiliza Minecraft.
//! Cada manifiesto y archivo se comprueba antes de publicar el runtime completo.

use crate::http;
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
    process::Stdio,
    sync::{
        atomic::{AtomicU64, Ordering},
        OnceLock,
    },
    time::Duration,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
pub use tokio_util::sync::CancellationToken;

const CATALOGO: &str = "https://launchermeta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json";
const MANIFEST: &str = ".icaro-runtime.json";
const MAX_MANIFEST: usize = 8 * 1024 * 1024;
const MAX_FILE: u64 = 512 * 1024 * 1024;
const MAX_TOTAL: u64 = 1024 * 1024 * 1024;
const MAX_FILES: usize = 8192;

#[derive(Debug, Clone)]
pub struct Java {
    pub executable: PathBuf,
    pub major: u32,
    pub version: String,
    pub managed: bool,
    pub arquitectura: String,
}

/// Versiones recomendadas para selección automática y mínimo para elección manual.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Perfil {
    pub majors: Vec<u32>,
    pub component: String,
    #[serde(default)]
    pub prism: bool,
}
impl Perfil {
    pub fn acepta(&self, major: u32) -> bool {
        self.majors.contains(&major)
    }
    /// Una elección explícita puede usar un Java más reciente que el recomendado.
    pub fn permite_manual(&self, major: u32) -> bool {
        major <= 255 && self.majors.iter().min().is_some_and(|minimo| major >= *minimo)
    }
    pub fn verificar_manual(&self, java: Java) -> Result<Java, Error> {
        self.validar()?;
        if self.permite_manual(java.major) {
            Ok(java)
        } else {
            self.verificar(java)
        }
    }
    pub fn validar(&self) -> Result<(), Error> {
        if self.majors.is_empty()
            || self.majors.len() > 32
            || self.majors.iter().any(|m| *m == 0 || *m > 255)
            || !componente_valido(&self.component)
        {
            return Err(Error::Datos);
        }
        Ok(())
    }
    pub fn verificar(&self, java: Java) -> Result<Java, Error> {
        self.validar()?;
        if self.acepta(java.major) {
            Ok(java)
        } else if self.majors.len() == 1 {
            Err(Error::Incompatible {
                requerida: self.majors[0],
                encontrada: java.major,
            })
        } else {
            Err(Error::PerfilIncompatible {
                compatibles: self.majors.clone(),
                encontrada: java.major,
            })
        }
    }
}

#[derive(Deserialize)]
struct MetadataJava {
    uid: String,
    version: String,
    #[serde(rename = "formatVersion")]
    format: u32,
    #[serde(rename = "compatibleJavaMajors")]
    majors: Vec<u32>,
    #[serde(rename = "compatibleJavaName")]
    component: String,
}
fn perfil_metadata(bytes: &[u8], version: &str) -> Result<Perfil, Error> {
    let metadata: MetadataJava = serde_json::from_slice(bytes).map_err(|_| Error::Datos)?;
    if metadata.uid != "net.minecraft" || metadata.version != version || metadata.format != 1 {
        return Err(Error::Datos);
    }
    let perfil = Perfil {
        majors: metadata.majors,
        component: metadata.component,
        prism: true,
    };
    perfil.validar()?;
    Ok(perfil)
}

fn ordenar_java(javas: &mut [Java]) {
    javas.sort_by(|a, b| {
        b.major
            .cmp(&a.major)
            .then_with(|| version_numerica(&b.version).cmp(&version_numerica(&a.version)))
            .then_with(|| a.executable.cmp(&b.executable))
    });
}

fn version_numerica(version: &str) -> Vec<u32> {
    version
        .strip_prefix("1.")
        .unwrap_or(version)
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|part| part.parse().ok())
        .collect()
}

/// Complemento de compatibilidad de Prism. Un fallo de red conserva Mojang.
pub async fn perfil_minecraft(
    version: &str,
    fallback: Perfil,
    cancel: &CancellationToken,
) -> Result<Perfil, Error> {
    fallback.validar()?;
    let mut url =
        Url::parse("https://meta.prismlauncher.org/v1/net.minecraft/").map_err(|_| Error::Datos)?;
    url.path_segments_mut()
        .map_err(|_| Error::Datos)?
        .pop_if_empty()
        .push(&format!("{version}.json"));
    let request = async {
        let client = http::client()?;
        let response = client
            .get(url)
            .send()
            .await
            .map_err(|_| http::Error::Network("metadatos de Prism"))?;
        if !response.status().is_success() {
            return Err(Error::Datos);
        }
        let bytes = http::body_bytes(response, "metadatos de Prism", MAX_MANIFEST).await?;
        perfil_metadata(&bytes, version)
    };
    tokio::select! {
        biased;
        _ = cancel.cancelled() => Err(Error::Cancelada),
        result = tokio::time::timeout(Duration::from_secs(8), request) => Ok(result.ok().and_then(Result::ok).unwrap_or(fallback)),
    }
}

/// Las descargas informan bytes; las etapas sin una cantidad conocida usan 0/0.
#[derive(Debug, Clone)]
pub struct Progreso {
    pub etapa: &'static str,
    pub completados: u64,
    pub total: u64,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Red(#[from] http::Error),
    #[error("La preparación de Java se canceló.")]
    Cancelada,
    #[error("No se pudo leer o guardar Java en disco.")]
    Disco,
    #[error("Los metadatos o las rutas del runtime de Java no son válidos.")]
    Datos,
    #[error("El archivo de Java no coincide con su tamaño o SHA-1.")]
    Hash,
    #[error("El Java seleccionado no se pudo ejecutar. Selecciona el ejecutable de Java o su carpeta de instalación.")]
    Ejecutable,
    #[error("Esta versión de Minecraft necesita Java {requerida}; el Java seleccionado es {encontrada}.")]
    Incompatible { requerida: u32, encontrada: u32 },
    #[error("El perfil acepta Java {compatibles:?}; el seleccionado es Java {encontrada}.")]
    PerfilIncompatible {
        compatibles: Vec<u32>,
        encontrada: u32,
    },
    #[error("La prueba de memoria o argumentos de Java falló: {0}")]
    PruebaJvm(String),
    #[error("Java usa arquitectura {encontrada}; se necesita {requerida}.")]
    Arquitectura {
        requerida: String,
        encontrada: String,
    },
    #[error("Falta Java {0} compatible. La descarga automática está desactivada; selecciona una instalación local.")]
    DescargaDesactivada(u32),
    #[error("Mojang no ofrece Java {0} para esta plataforma.")]
    NoDisponible(u32),
    #[error("La descarga automática de Java no está disponible para esta plataforma.")]
    Plataforma,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Descarga {
    sha1: String,
    size: u64,
    url: String,
}

#[derive(Debug, Deserialize)]
struct VersionRuntime {
    name: String,
}

#[derive(Debug, Deserialize)]
struct Runtime {
    manifest: Descarga,
    version: VersionRuntime,
}

type Catalogo = BTreeMap<String, BTreeMap<String, Vec<Runtime>>>;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum Entrada {
    Directory,
    File {
        downloads: Descargas,
        #[serde(default)]
        executable: bool,
    },
    Link {
        target: String,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Descargas {
    raw: Descarga,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct Manifiesto {
    files: BTreeMap<String, Entrada>,
}

#[derive(Clone)]
struct Archivo {
    ruta: PathBuf,
    descarga: Descarga,
    executable: bool,
}

struct Plan {
    carpetas: Vec<PathBuf>,
    archivos: Vec<Archivo>,
    // Los links se materializan como copias internas. Así no necesitan permisos
    // de enlaces en Windows y nunca introducen un enlace que salga del runtime.
    enlaces: Vec<(PathBuf, Archivo)>,
    total: u64,
}

fn comprobar_cancelacion(cancel: &CancellationToken) -> Result<(), Error> {
    if cancel.is_cancelled() {
        Err(Error::Cancelada)
    } else {
        Ok(())
    }
}

fn anunciar(progress: &impl Fn(Progreso), etapa: &'static str) {
    progress(Progreso {
        etapa,
        completados: 0,
        total: 0,
    });
}

fn plataforma() -> Result<&'static str, Error> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => Ok("windows-x64"),
        ("windows", "x86") => Ok("windows-x86"),
        ("windows", "aarch64") => Ok("windows-arm64"),
        ("linux", "x86_64") => Ok("linux"),
        ("linux", "x86") => Ok("linux-i386"),
        _ => Err(Error::Plataforma),
    }
}

fn nombre_java() -> &'static str {
    if cfg!(windows) {
        "java.exe"
    } else {
        "java"
    }
}

fn componente_valido(component: &str) -> bool {
    !component.is_empty()
        && component.len() <= 80
        && component
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// Lee tanto `1.8.0_51` como `17.0.15`, `21-ea` y `25.0.1+8`.
fn major_version(version: &str) -> Option<u32> {
    let version = version.strip_prefix("1.").unwrap_or(version);
    let digits: String = version.chars().take_while(|c| c.is_ascii_digit()).collect();
    let major = digits.parse::<u32>().ok()?;
    (major > 0 && major <= 255).then_some(major)
}

fn parse_version(output: &str) -> Option<(u32, String)> {
    for line in output.lines().take(256) {
        let line = line.trim();
        if !(line.starts_with("java version ")
            || line.starts_with("openjdk version ")
            || line.starts_with("openjdk "))
        {
            continue;
        }
        let version = if let Some(quoted) = line.split('"').nth(1) {
            quoted
        } else {
            line.split_whitespace().nth(1)?
        };
        if version.is_empty()
            || version.len() > 100
            || version.chars().any(|c| c.is_control() || c.is_whitespace())
        {
            continue;
        }
        if let Some(major) = major_version(version) {
            return Some((major, version.into()));
        }
    }
    None
}

fn ejecutable(path: &Path) -> PathBuf {
    if path.is_dir() {
        path.join("bin").join(nombre_java())
    } else if cfg!(windows)
        && path
            .file_name()
            .is_some_and(|s| s.to_string_lossy().eq_ignore_ascii_case("javaw.exe"))
    {
        // Las sondas usan el ejecutable de consola; el lanzamiento del juego
        // puede elegir su acompañante javaw sin cambiar el runtime validado.
        path.with_file_name("java.exe")
    } else {
        path.to_path_buf()
    }
}

async fn inspeccionar(
    path: &Path,
    major: u32,
    managed: bool,
    cancel: &CancellationToken,
) -> Result<Java, Error> {
    inspeccionar_argumentos(path, major, managed, cancel, &[]).await
}

async fn inspeccionar_argumentos(
    path: &Path,
    major: u32,
    managed: bool,
    cancel: &CancellationToken,
    argumentos: &[String],
) -> Result<Java, Error> {
    comprobar_cancelacion(cancel)?;
    let path = fs::canonicalize(ejecutable(path)).map_err(|_| Error::Ejecutable)?;
    if !path.is_file() {
        return Err(Error::Ejecutable);
    }
    let mut command = tokio::process::Command::new(&path);
    command
        .args(argumentos)
        .arg("-XshowSettings:properties")
        .arg("-version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_remove("JAVA_TOOL_OPTIONS")
        .env_remove("_JAVA_OPTIONS")
        .env_remove("JDK_JAVA_OPTIONS")
        .env_remove("CLASSPATH")
        .kill_on_drop(true);
    #[cfg(windows)]
    command.creation_flags(0x08000000); // CREATE_NO_WINDOW
    let mut child = command.spawn().map_err(|_| Error::Ejecutable)?;
    let stdout = child.stdout.take().ok_or(Error::Ejecutable)?;
    let stderr = child.stderr.take().ok_or(Error::Ejecutable)?;
    let read = async {
        let mut stdout = stdout.take(16385);
        let mut stderr = stderr.take(16385);
        let mut out = Vec::new();
        let mut err = Vec::new();
        let (a, b) = tokio::join!(stdout.read_to_end(&mut out), stderr.read_to_end(&mut err));
        a.map_err(|_| Error::Ejecutable)?;
        b.map_err(|_| Error::Ejecutable)?;
        if out.len() > 16384 || err.len() > 16384 {
            return Err(Error::Ejecutable);
        }
        let status = child.wait().await.map_err(|_| Error::Ejecutable)?;
        out.extend_from_slice(&err);
        if !status.success() {
            return Err(if argumentos.is_empty() {
                Error::Ejecutable
            } else {
                Error::PruebaJvm(
                    String::from_utf8_lossy(&out)
                        .trim()
                        .chars()
                        .take(1000)
                        .collect(),
                )
            });
        }
        let output = String::from_utf8_lossy(&out);
        let (major, version) = parse_version(&output).ok_or(Error::Ejecutable)?;
        let arquitectura = output
            .lines()
            .find_map(|line| {
                let (key, value) = line.trim().split_once('=')?;
                (key.trim() == "os.arch").then(|| value.trim().to_owned())
            })
            .ok_or(Error::Ejecutable)?;
        comprobar_arquitectura(&arquitectura)?;
        Ok((major, version, arquitectura))
    };
    let (found, version, arquitectura) = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(Error::Cancelada),
        result = tokio::time::timeout(Duration::from_secs(10), read) => result.map_err(|_| Error::Ejecutable)??,
    };
    if major != 0 && found != major {
        return Err(Error::Incompatible {
            requerida: major,
            encontrada: found,
        });
    }
    Ok(Java {
        executable: path,
        major: found,
        version,
        managed,
        arquitectura,
    })
}

fn comprobar_arquitectura(found: &str) -> Result<(), Error> {
    let normalized = match found {
        "amd64" | "x86_64" => "x86_64",
        "x86" | "i386" | "i486" | "i586" | "i686" => "x86",
        "aarch64" | "arm64" => "aarch64",
        other => other,
    };
    if normalized == std::env::consts::ARCH {
        Ok(())
    } else {
        Err(Error::Arquitectura {
            requerida: std::env::consts::ARCH.into(),
            encontrada: found.into(),
        })
    }
}

/// Inventario local comprobado. Ejecutar desde una tarea, nunca desde la vista.
pub async fn detectar(cancel: CancellationToken) -> Result<Vec<Java>, Error> {
    let mut result = Vec::new();
    for path in candidatos_sistema() {
        match inspeccionar(&path, 0, false, &cancel).await {
            Ok(java) => result.push(java),
            Err(Error::Cancelada) => return Err(Error::Cancelada),
            Err(_) => {}
        }
    }
    ordenar_java(&mut result);
    Ok(result)
}

pub async fn comprobar(path: &Path, major: u32, cancel: CancellationToken) -> Result<Java, Error> {
    inspeccionar(path, major, false, &cancel).await
}

/// Inicializa la JVM con los mismos valores y parser usados al lanzar el juego.
pub async fn probar(
    path: &Path,
    perfil: Option<&Perfil>,
    config: &crate::lanzamiento::Config,
    cancel: CancellationToken,
) -> Result<Java, Error> {
    let args = crate::lanzamiento::argumentos_prueba(config)
        .map_err(|e| Error::PruebaJvm(e.to_string()))?;
    let java = inspeccionar_argumentos(path, 0, false, &cancel, &args).await?;
    match perfil {
        Some(perfil) => perfil.verificar_manual(java),
        None => Ok(java),
    }
}

fn candidatos_sistema() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(home) = std::env::var_os("JAVA_HOME") {
        paths.push(PathBuf::from(home).join("bin").join(nombre_java()));
    }
    if let Some(path) = std::env::var_os("PATH") {
        paths.extend(
            std::env::split_paths(&path)
                .filter(|p| p.is_absolute())
                .take(256)
                .map(|p| p.join(nombre_java())),
        );
    }
    #[cfg(windows)]
    {
        use winreg::{
            enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_32KEY, KEY_WOW64_64KEY},
            RegKey,
        };
        let machine = RegKey::predef(HKEY_LOCAL_MACHINE);
        for view in [KEY_WOW64_64KEY, KEY_WOW64_32KEY] {
            for key in [
                "SOFTWARE\\JavaSoft\\Java Runtime Environment",
                "SOFTWARE\\JavaSoft\\Java Development Kit",
                "SOFTWARE\\JavaSoft\\JRE",
                "SOFTWARE\\JavaSoft\\JDK",
            ] {
                let Ok(root) = machine.open_subkey_with_flags(key, KEY_READ | view) else {
                    continue;
                };
                for version in root.enum_keys().take(32).flatten() {
                    let Ok(entry) = root.open_subkey(version) else {
                        continue;
                    };
                    if let Ok(home) = entry.get_value::<String, _>("JavaHome") {
                        paths.push(PathBuf::from(home).join("bin").join(nombre_java()));
                    }
                }
            }
        }
        for variable in [
            "ProgramFiles",
            "ProgramW6432",
            "ProgramFiles(x86)",
            "LOCALAPPDATA",
        ] {
            if let Some(base) = std::env::var_os(variable) {
                for vendor in [
                    "Java",
                    "Eclipse Adoptium",
                    "Eclipse Foundation",
                    "Microsoft",
                    "Amazon Corretto",
                    "Zulu",
                    "BellSoft",
                    "Programs/Eclipse Adoptium",
                ] {
                    agregar_instalaciones(&PathBuf::from(&base).join(vendor), &mut paths);
                }
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        for base in ["/usr/lib/jvm", "/usr/java", "/opt/java"] {
            agregar_instalaciones(Path::new(base), &mut paths);
        }
        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            agregar_instalaciones(&home.join(".jvm"), &mut paths);
            agregar_instalaciones(&home.join(".sdkman/candidates/java"), &mut paths);
        }
    }
    #[cfg(windows)]
    {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            agregar_runtimes_minecraft(
                &PathBuf::from(appdata).join(".minecraft/runtime"),
                &mut paths,
            );
        }
        if let Some(local) = std::env::var_os("LOCALAPPDATA") {
            agregar_runtimes_minecraft(
                &PathBuf::from(local).join(
                    "Packages/Microsoft.4297127D64EC6_8wekyb3d8bbwe/LocalCache/Local/runtime",
                ),
                &mut paths,
            );
        }
    }
    #[cfg(not(windows))]
    if let Some(home) = std::env::var_os("HOME") {
        let root = if cfg!(target_os = "macos") {
            "Library/Application Support/minecraft/runtime"
        } else {
            ".minecraft/runtime"
        };
        agregar_runtimes_minecraft(&PathBuf::from(home).join(root), &mut paths);
    }
    depurar_candidatos(paths)
}

/// Solo inspecciona un nivel en ubicaciones conocidas; no recorre todo el disco.
fn agregar_instalaciones(base: &Path, paths: &mut Vec<PathBuf>) {
    paths.push(base.join("bin").join(nombre_java()));
    if let Ok(entries) = fs::read_dir(base) {
        let mut homes: Vec<_> = entries
            .take(256)
            .flatten()
            .map(|entry| entry.path())
            .collect();
        homes.sort();
        paths.extend(
            homes
                .into_iter()
                .map(|home| home.join("bin").join(nombre_java())),
        );
    }
}

fn agregar_runtimes_minecraft(root: &Path, paths: &mut Vec<PathBuf>) {
    let Ok(root) = fs::canonicalize(root) else {
        return;
    };
    let mut pending = vec![(root.clone(), 0)];
    let mut visited = BTreeSet::new();
    while let Some((dir, depth)) = pending.pop() {
        if visited.len() >= 512 || !visited.insert(dir.clone()) {
            continue;
        }
        paths.push(dir.join("bin").join(nombre_java()));
        if depth >= 6 {
            continue;
        }
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        let mut children: Vec<_> = entries
            .take(256)
            .flatten()
            .filter_map(|entry| {
                if !entry.file_type().ok()?.is_dir() {
                    return None;
                }
                if matches!(entry.file_name().to_str(), Some("lib" | "legal" | "bin")) {
                    return None;
                }
                let path = fs::canonicalize(entry.path()).ok()?;
                path.starts_with(&root).then_some(path)
            })
            .collect();
        children.sort();
        pending.extend(children.into_iter().rev().map(|path| (path, depth + 1)));
    }
}

fn depurar_candidatos(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = BTreeSet::new();
    paths
        .into_iter()
        .filter_map(|path| {
            let path = fs::canonicalize(path).ok()?;
            (path.is_file() && seen.insert(path.clone())).then_some(path)
        })
        .collect()
}

fn enlace(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        meta.file_type().is_symlink()
    }
}

// Sólo crea directorios normales. No atraviesa junctions, symlinks ni padres `..`.
fn crear_carpeta(path: &Path) -> Result<PathBuf, Error> {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|_| Error::Disco)?
            .join(path)
    };
    let mut current = PathBuf::new();
    for component in path.components() {
        if matches!(component, Component::Prefix(_) | Component::RootDir) {
            current.push(component);
            continue;
        }
        if matches!(component, Component::ParentDir) {
            return Err(Error::Datos);
        }
        if matches!(component, Component::CurDir) {
            continue;
        }
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(m) if m.is_dir() && !enlace(&m) => {}
            Ok(_) => return Err(Error::Datos),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                fs::create_dir(&current).map_err(|_| Error::Disco)?
            }
            Err(_) => return Err(Error::Disco),
        }
    }
    fs::canonicalize(current).map_err(|_| Error::Disco)
}

fn parte_valida(s: &str) -> bool {
    !s.is_empty()
        && s != "."
        && s != ".."
        && s.len() <= 240
        && !s.starts_with('.')
        && !s.ends_with('.')
        && !s.ends_with(' ')
        && !s
            .chars()
            .any(|c| c.is_control() || "<>:\"\\|?*".contains(c))
        && !matches!(
            s.split('.')
                .next()
                .unwrap_or("")
                .to_ascii_uppercase()
                .as_str(),
            "CON"
                | "PRN"
                | "AUX"
                | "NUL"
                | "COM1"
                | "COM2"
                | "COM3"
                | "COM4"
                | "COM5"
                | "COM6"
                | "COM7"
                | "COM8"
                | "COM9"
                | "LPT1"
                | "LPT2"
                | "LPT3"
                | "LPT4"
                | "LPT5"
                | "LPT6"
                | "LPT7"
                | "LPT8"
                | "LPT9"
        )
}

fn ruta_relativa(s: &str) -> Result<PathBuf, Error> {
    if s.is_empty() || s.len() > 512 || !s.split('/').all(parte_valida) {
        return Err(Error::Datos);
    }
    Ok(s.split('/').collect())
}

fn resolver_enlace(name: &str, target: &str) -> Result<String, Error> {
    if target.is_empty() || target.len() > 512 {
        return Err(Error::Datos);
    }
    let mut parts: Vec<&str> = name.split('/').collect();
    parts.pop();
    for part in target.split('/') {
        match part {
            "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return Err(Error::Datos);
                }
            }
            p if parte_valida(p) => parts.push(p),
            _ => return Err(Error::Datos),
        }
    }
    let result = parts.join("/");
    ruta_relativa(&result)?;
    Ok(result)
}

fn sha1_valido(sha: &str) -> bool {
    sha.len() == 40
        && sha
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn url_oficial(download: &Descarga, manifest: bool) -> Result<Url, Error> {
    if !sha1_valido(&download.sha1) || download.size > MAX_FILE {
        return Err(Error::Datos);
    }
    let url = Url::parse(&download.url).map_err(|_| Error::Datos)?;
    let host = if manifest {
        "piston-meta.mojang.com"
    } else {
        "piston-data.mojang.com"
    };
    let prefix = if manifest {
        format!("/v1/packages/{}/", download.sha1)
    } else {
        format!("/v1/objects/{}/", download.sha1)
    };
    if url.scheme() != "https"
        || url.host_str() != Some(host)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.path().starts_with(&prefix)
    {
        return Err(Error::Datos);
    }
    Ok(url)
}

fn archivo_enlace(name: &str, files: &BTreeMap<String, Entrada>) -> Result<Archivo, Error> {
    let original = ruta_relativa(name)?;
    let mut current = name.to_owned();
    let mut visited = BTreeSet::new();
    loop {
        if !visited.insert(current.clone()) || visited.len() > 32 {
            return Err(Error::Datos);
        }
        match files.get(&current).ok_or(Error::Datos)? {
            Entrada::File {
                downloads,
                executable,
            } => {
                return Ok(Archivo {
                    ruta: original,
                    descarga: downloads.raw.clone(),
                    executable: *executable,
                })
            }
            Entrada::Link { target } => current = resolver_enlace(&current, target)?,
            Entrada::Directory => return Err(Error::Datos),
        }
    }
}

fn plan(manifest: &Manifiesto) -> Result<Plan, Error> {
    if manifest.files.is_empty() || manifest.files.len() > MAX_FILES {
        return Err(Error::Datos);
    }
    let mut result = Plan {
        carpetas: Vec::new(),
        archivos: Vec::new(),
        enlaces: Vec::new(),
        total: 0,
    };
    let mut aliases = BTreeSet::new();
    for (name, entry) in &manifest.files {
        let path = ruta_relativa(name)?;
        if !aliases.insert(name.to_ascii_lowercase()) {
            return Err(Error::Datos);
        }
        let mut parent = path.parent();
        while let Some(p) = parent.filter(|p| !p.as_os_str().is_empty()) {
            let key = p
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            if manifest
                .files
                .get(&key)
                .is_some_and(|e| !matches!(e, Entrada::Directory))
            {
                return Err(Error::Datos);
            }
            parent = p.parent();
        }
        match entry {
            Entrada::Directory => result.carpetas.push(path),
            Entrada::File {
                downloads,
                executable,
            } => {
                url_oficial(&downloads.raw, false)?;
                result.total = result
                    .total
                    .checked_add(downloads.raw.size)
                    .ok_or(Error::Datos)?;
                if result.total > MAX_TOTAL {
                    return Err(Error::Datos);
                }
                result.archivos.push(Archivo {
                    ruta: path,
                    descarga: downloads.raw.clone(),
                    executable: *executable,
                });
            }
            Entrada::Link { target } => {
                let source = resolver_enlace(name, target)?;
                let mut file = archivo_enlace(&source, &manifest.files)?;
                // resolver_enlace/archivo_enlace permiten enlaces hacia archivos
                // internos existentes, pero rechazan ciclos y enlaces a directorios.
                file.ruta = ruta_relativa(&source)?;
                // Para cadenas, copiar siempre el archivo final, no un link pendiente.
                let mut resolved = source;
                let mut seen = BTreeSet::new();
                while let Some(Entrada::Link { target }) = manifest.files.get(&resolved) {
                    if !seen.insert(resolved.clone()) {
                        return Err(Error::Datos);
                    }
                    resolved = resolver_enlace(&resolved, target)?;
                }
                file.ruta = ruta_relativa(&resolved)?;
                result.enlaces.push((path, file));
            }
        }
    }
    if !manifest
        .files
        .contains_key(&format!("bin/{}", nombre_java()))
    {
        return Err(Error::Datos);
    }
    Ok(result)
}

fn comprobar_hash(bytes: &[u8], download: &Descarga) -> Result<(), Error> {
    let digest = format!("{:x}", Sha1::digest(bytes));
    if bytes.len() as u64 == download.size && digest == download.sha1 {
        Ok(())
    } else {
        Err(Error::Hash)
    }
}

fn comprobar_archivo(base: &Path, file: &Archivo, cancel: &CancellationToken) -> Result<(), Error> {
    comprobar_cancelacion(cancel)?;
    let path = base.join(&file.ruta);
    let meta = fs::symlink_metadata(&path).map_err(|_| Error::Disco)?;
    if !meta.is_file() || enlace(&meta) || meta.len() != file.descarga.size {
        return Err(Error::Hash);
    }
    let resolved = fs::canonicalize(&path).map_err(|_| Error::Disco)?;
    if !resolved.starts_with(base) {
        return Err(Error::Datos);
    }
    let mut input = fs::File::open(path).map_err(|_| Error::Disco)?;
    let mut buffer = [0u8; 65536];
    let mut hash = Sha1::new();
    loop {
        comprobar_cancelacion(cancel)?;
        let n = input.read(&mut buffer).map_err(|_| Error::Disco)?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    if format!("{:x}", hash.finalize()) != file.descarga.sha1 {
        return Err(Error::Hash);
    }
    Ok(())
}

fn comprobar_runtime(base: &Path, plan: &Plan, cancel: &CancellationToken) -> Result<(), Error> {
    for directory in &plan.carpetas {
        let path = base.join(directory);
        let m = fs::symlink_metadata(&path).map_err(|_| Error::Disco)?;
        if !m.is_dir()
            || enlace(&m)
            || !fs::canonicalize(&path)
                .map_err(|_| Error::Disco)?
                .starts_with(base)
        {
            return Err(Error::Datos);
        }
    }
    for file in &plan.archivos {
        comprobar_archivo(base, file, cancel)?;
    }
    for (target, source) in &plan.enlaces {
        let mut file = source.clone();
        file.ruta = target.clone();
        comprobar_archivo(base, &file, cancel)?;
    }
    Ok(())
}

fn leer_limitado(path: &Path, limit: usize) -> Result<Vec<u8>, Error> {
    let meta = fs::symlink_metadata(path).map_err(|_| Error::Disco)?;
    if enlace(&meta) || !meta.is_file() || meta.len() > limit as u64 {
        return Err(Error::Datos);
    }
    let mut data = Vec::new();
    fs::File::open(path)
        .map_err(|_| Error::Disco)?
        .take(limit as u64 + 1)
        .read_to_end(&mut data)
        .map_err(|_| Error::Disco)?;
    if data.len() > limit {
        return Err(Error::Datos);
    }
    Ok(data)
}

fn version_release(data: &[u8]) -> Option<(u32, String)> {
    let release = std::str::from_utf8(data).ok()?;
    let line = release
        .lines()
        .take(128)
        .find_map(|line| line.strip_prefix("JAVA_VERSION="))?;
    let version = line.trim().strip_prefix('"')?.strip_suffix('"')?;
    if version.is_empty()
        || version.len() > 100
        || version.chars().any(|c| c.is_control() || c.is_whitespace())
    {
        return None;
    }
    Some((major_version(version)?, version.into()))
}

/// Enumera los runtimes de Ícaro que ya están en disco, sin consultar Internet
/// ni iniciar Java. Sirve para poblar los selectores del System Design.
///
/// Comprueba el manifiesto guardado, `release` y el ejecutable. `preparar`
/// comprueba además todas las bibliotecas antes de usar uno para jugar.
/// Ignora carpetas incompletas, enlaces y metadatos alterados.
pub fn gestionados(base: &Path) -> Vec<Java> {
    let Ok(platform) = plataforma() else {
        return Vec::new();
    };
    let root = base.join("java");
    let Ok(root_meta) = fs::symlink_metadata(&root) else {
        return Vec::new();
    };
    if !root_meta.is_dir() || enlace(&root_meta) {
        return Vec::new();
    }
    let Ok(root) = fs::canonicalize(&root) else {
        return Vec::new();
    };
    let Ok(entries) = fs::read_dir(&root) else {
        return Vec::new();
    };
    let mut result = Vec::new();
    let cancel = CancellationToken::new();
    for entry in entries.take(256).flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some((component, suffix)) = name.rsplit_once(&format!("-{platform}-")) else {
            continue;
        };
        let Some(hash) = suffix.get(..40) else {
            continue;
        };
        if !componente_valido(component)
            || !sha1_valido(hash)
            || !(suffix.len() == 40 || suffix.get(40..41) == Some("-"))
        {
            continue;
        }
        let path = entry.path();
        let Ok(meta) = fs::symlink_metadata(&path) else {
            continue;
        };
        if !meta.is_dir() || enlace(&meta) {
            continue;
        }
        let Ok(path) = fs::canonicalize(path) else {
            continue;
        };
        if path.parent() != Some(root.as_path()) {
            continue;
        }
        let Ok(bytes) = leer_limitado(&path.join(MANIFEST), MAX_MANIFEST) else {
            continue;
        };
        if format!("{:x}", Sha1::digest(&bytes)) != hash {
            continue;
        }
        let Ok(manifest) = serde_json::from_slice::<Manifiesto>(&bytes) else {
            continue;
        };
        if plan(&manifest).is_err() {
            continue;
        }
        let Ok(release) = archivo_enlace("release", &manifest.files) else {
            continue;
        };
        if release.descarga.size > 65536 || comprobar_archivo(&path, &release, &cancel).is_err() {
            continue;
        }
        let Ok(release) = leer_limitado(&path.join("release"), 65536) else {
            continue;
        };
        let Some((major, version)) = version_release(&release) else {
            continue;
        };
        let key = format!("bin/{}", nombre_java());
        let Ok(java) = archivo_enlace(&key, &manifest.files) else {
            continue;
        };
        if comprobar_archivo(&path, &java, &cancel).is_err() {
            continue;
        }
        result.push(Java {
            executable: path.join("bin").join(nombre_java()),
            major,
            version,
            managed: true,
            arquitectura: String::from_utf8_lossy(&release)
                .lines()
                .find_map(|line| {
                    line.strip_prefix("OS_ARCH=")
                        .map(|v| v.trim_matches('"').to_owned())
                })
                .unwrap_or_else(|| "Sin comprobar".into()),
        });
    }
    result.sort_by(|a, b| {
        a.major
            .cmp(&b.major)
            .then(a.version.cmp(&b.version))
            .then(a.executable.cmp(&b.executable))
    });
    result
}

async fn runtime_guardado(
    root: &Path,
    prefix: &str,
    major: u32,
    cancel: &CancellationToken,
) -> Result<Option<Java>, Error> {
    let entries = fs::read_dir(root).map_err(|_| Error::Disco)?;
    let mut paths = Vec::new();
    for entry in entries.take(256).flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(suffix) = name.strip_prefix(prefix) else {
            continue;
        };
        let Some(hash) = suffix.get(..40) else {
            continue;
        };
        if !sha1_valido(hash) || !(suffix.len() == 40 || suffix.get(40..41) == Some("-")) {
            continue;
        }
        if !entry.file_type().is_ok_and(|m| m.is_dir())
            || enlace(&fs::symlink_metadata(entry.path()).map_err(|_| Error::Disco)?)
        {
            continue;
        }
        paths.push((entry.path(), hash.to_owned()));
    }
    paths.sort();
    for (path, hash) in paths.into_iter().rev() {
        comprobar_cancelacion(cancel)?;
        let path = fs::canonicalize(&path).map_err(|_| Error::Disco)?;
        if path.parent() != Some(root) {
            continue;
        }
        let metadata = path.join(MANIFEST);
        let Ok(meta) = fs::symlink_metadata(&metadata) else {
            continue;
        };
        if enlace(&meta) || !meta.is_file() || meta.len() > MAX_MANIFEST as u64 {
            continue;
        }
        let Ok(bytes) = leer_limitado(&metadata, MAX_MANIFEST) else {
            continue;
        };
        if format!("{:x}", Sha1::digest(&bytes)) != hash {
            continue;
        }
        let Ok(manifest) = serde_json::from_slice::<Manifiesto>(&bytes) else {
            continue;
        };
        let Ok(plan) = plan(&manifest) else {
            continue;
        };
        match comprobar_runtime(&path, &plan, cancel) {
            Ok(()) => {}
            Err(Error::Cancelada) => return Err(Error::Cancelada),
            Err(_) => continue,
        }
        match inspeccionar(&path.join("bin").join(nombre_java()), major, true, cancel).await {
            Ok(java) => return Ok(Some(java)),
            Err(Error::Cancelada) => return Err(Error::Cancelada),
            Err(_) => {}
        }
    }
    Ok(None)
}

fn client() -> Result<Client, Error> {
    Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(300))
        .user_agent(http::USER_AGENT)
        .build()
        .map_err(|_| Error::Red(http::Error::Network("Mojang")))
}

async fn bytes(
    client: &Client,
    url: Url,
    limit: usize,
    cancel: &CancellationToken,
) -> Result<Vec<u8>, Error> {
    tokio::select! {
        biased;
        _ = cancel.cancelled() => Err(Error::Cancelada),
        result = async {
            let response = client.get(url).send().await.map_err(|_| Error::Red(http::Error::Network("Mojang")))?;
            if !response.status().is_success() { return Err(Error::Red(http::Error::Status { service: "Mojang", status: response.status().as_u16() })); }
            Ok(http::body_bytes(response, "Mojang", limit).await?)
        } => result,
    }
}

fn permiso_ejecutable(path: &Path, executable: bool) -> Result<(), Error> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            path,
            fs::Permissions::from_mode(if executable { 0o755 } else { 0o644 }),
        )
        .map_err(|_| Error::Disco)?;
    }
    #[cfg(not(unix))]
    let _ = (path, executable);
    Ok(())
}

async fn descargar(
    client: &Client,
    stage: &Path,
    file: &Archivo,
    cancel: &CancellationToken,
    count: &AtomicU64,
    total: u64,
    progress: &(impl Fn(Progreso) + Send + Sync),
) -> Result<(), Error> {
    comprobar_cancelacion(cancel)?;
    let url = url_oficial(&file.descarga, false)?;
    let mut response = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(Error::Cancelada),
        r = client.get(url).send() => r.map_err(|_| Error::Red(http::Error::Network("Mojang")))?,
    };
    if !response.status().is_success() {
        return Err(Error::Red(http::Error::Status {
            service: "Mojang",
            status: response.status().as_u16(),
        }));
    }
    if response
        .content_length()
        .is_some_and(|n| n != file.descarga.size)
    {
        return Err(Error::Hash);
    }
    let path = stage.join(&file.ruta);
    crear_carpeta(path.parent().ok_or(Error::Datos)?)?;
    let mut output = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .await
        .map_err(|_| Error::Disco)?;
    let mut received = 0u64;
    let mut hash = Sha1::new();
    loop {
        let chunk = tokio::select! {
            biased;
            _ = cancel.cancelled() => return Err(Error::Cancelada),
            c = response.chunk() => c.map_err(|_| Error::Red(http::Error::Network("Mojang")))?,
        };
        let Some(chunk) = chunk else {
            break;
        };
        received = received
            .checked_add(chunk.len() as u64)
            .ok_or(Error::Hash)?;
        if received > file.descarga.size {
            return Err(Error::Hash);
        }
        output.write_all(&chunk).await.map_err(|_| Error::Disco)?;
        hash.update(&chunk);
        let completed = count.fetch_add(chunk.len() as u64, Ordering::Relaxed) + chunk.len() as u64;
        progress(Progreso {
            etapa: "Descargando Java",
            completados: completed,
            total,
        });
    }
    output.flush().await.map_err(|_| Error::Disco)?;
    output.sync_all().await.map_err(|_| Error::Disco)?;
    drop(output);
    if received != file.descarga.size || format!("{:x}", hash.finalize()) != file.descarga.sha1 {
        return Err(Error::Hash);
    }
    permiso_ejecutable(&path, file.executable)
}

async fn worker(
    client: &Client,
    stage: &Path,
    files: &[&Archivo],
    cancel: &CancellationToken,
    count: &AtomicU64,
    total: u64,
    progress: &(impl Fn(Progreso) + Send + Sync),
) -> Result<(), Error> {
    for file in files {
        descargar(client, stage, file, cancel, count, total, progress).await?;
    }
    Ok(())
}

fn nonce() -> Result<String, Error> {
    let mut bytes = [0u8; 8];
    getrandom::fill(&mut bytes).map_err(|_| Error::Disco)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

struct Temporal {
    root: PathBuf,
    path: PathBuf,
    publicado: bool,
}
impl Drop for Temporal {
    fn drop(&mut self) {
        if self.publicado {
            return;
        }
        // Sólo elimina la carpeta única creada por esta operación, dentro de la
        // raíz comprobada. Un enlace o una carpeta reemplazada se conserva.
        let Ok(m) = fs::symlink_metadata(&self.path) else {
            return;
        };
        if !m.is_dir() || enlace(&m) {
            return;
        }
        if fs::canonicalize(&self.path).is_ok_and(|p| p.parent() == Some(self.root.as_path())) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

/// Reutiliza un Java compatible o descarga el componente oficial indicado por
/// los metadatos de Minecraft. `base` es almacenamiento propio del launcher.
/// Un Java preferido incompatible produce un error; nunca cambia a otro sin aviso.
pub async fn preparar(
    base: &Path,
    major: u32,
    component: &str,
    preferida: Option<&Path>,
    cancel: CancellationToken,
    progress: impl Fn(Progreso) + Send + Sync,
) -> Result<Java, Error> {
    preparar_con_candidatos(
        base,
        major,
        component,
        preferida,
        cancel,
        progress,
        true,
        candidatos_sistema,
    )
    .await
}

/// Permite usar Java local o guardado sin autorizar nuevas descargas.
pub async fn preparar_con_descarga(
    base: &Path,
    major: u32,
    component: &str,
    preferida: Option<&Path>,
    descarga: bool,
    cancel: CancellationToken,
    progress: impl Fn(Progreso) + Send + Sync,
) -> Result<Java, Error> {
    preparar_con_candidatos(
        base,
        major,
        component,
        preferida,
        cancel,
        progress,
        descarga,
        candidatos_sistema,
    )
    .await
}

async fn preparar_con_candidatos(
    base: &Path,
    major: u32,
    component: &str,
    preferida: Option<&Path>,
    cancel: CancellationToken,
    progress: impl Fn(Progreso) + Send + Sync,
    descarga: bool,
    candidatos: impl FnOnce() -> Vec<PathBuf>,
) -> Result<Java, Error> {
    let perfil = Perfil {
        majors: vec![major],
        component: component.into(),
        prism: false,
    };
    preparar_perfil_con_candidatos(
        base, &perfil, preferida, cancel, progress, descarga, candidatos,
    )
    .await
}

pub async fn preparar_perfil(
    base: &Path,
    perfil: &Perfil,
    preferida: Option<&Path>,
    descarga: bool,
    cancel: CancellationToken,
    progress: impl Fn(Progreso) + Send + Sync,
) -> Result<Java, Error> {
    preparar_perfil_con_candidatos(
        base,
        perfil,
        preferida,
        cancel,
        progress,
        descarga,
        candidatos_sistema,
    )
    .await
}

async fn preparar_perfil_con_candidatos(
    base: &Path,
    perfil: &Perfil,
    preferida: Option<&Path>,
    cancel: CancellationToken,
    progress: impl Fn(Progreso) + Send + Sync,
    descarga: bool,
    candidatos: impl FnOnce() -> Vec<PathBuf>,
) -> Result<Java, Error> {
    comprobar_cancelacion(&cancel)?;
    perfil.validar()?;
    let major = perfil.majors[0];
    let component = perfil.component.as_str();
    anunciar(&progress, "Comprobando Java");
    if let Some(path) = preferida {
        let managed = fs::canonicalize(base.join("java"))
            .ok()
            .zip(fs::canonicalize(ejecutable(path)).ok())
            .is_some_and(|(root, executable)| executable.starts_with(root));
        return perfil.verificar_manual(inspeccionar(path, 0, managed, &cancel).await?);
    }
    // Como Prism: la descarga habilitada selecciona el runtime administrado.
    // La selección manual siempre tiene prioridad; sin descargas se detecta Java local.
    if !descarga {
        let mut compatibles = Vec::new();
        for path in candidatos() {
            match inspeccionar(&path, 0, false, &cancel)
                .await
                .and_then(|java| perfil.verificar(java))
            {
                Ok(java) => compatibles.push(java),
                Err(Error::Cancelada) => return Err(Error::Cancelada),
                Err(_) => {}
            }
        }
        ordenar_java(&mut compatibles);
        if let Some(java) = compatibles.into_iter().next() {
            return Ok(java);
        }
    }
    let platform = plataforma()?;
    static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
    let _guard = tokio::select! {
        biased;
        _ = cancel.cancelled() => return Err(Error::Cancelada),
        lock = LOCK.get_or_init(|| tokio::sync::Mutex::new(())).lock() => lock,
    };
    let base = crear_carpeta(base)?;
    let root = crear_carpeta(&base.join("java"))?;
    let prefix = format!("{component}-{platform}-");
    for compatible in &perfil.majors {
        if let Some(java) = runtime_guardado(&root, &prefix, *compatible, &cancel).await? {
            return Ok(java);
        }
    }
    if !descarga {
        // Permitir también runtimes guardados para otros componentes compatibles.
        let mut prefixes = BTreeSet::new();
        for entry in fs::read_dir(&root)
            .map_err(|_| Error::Disco)?
            .take(256)
            .flatten()
        {
            let name = entry.file_name().to_string_lossy().into_owned();
            if let Some((component, _)) = name.split_once(&format!("-{platform}-")) {
                if componente_valido(component) {
                    prefixes.insert(format!("{component}-{platform}-"));
                }
            }
        }
        for prefix in prefixes {
            for compatible in &perfil.majors {
                if let Some(java) = runtime_guardado(&root, &prefix, *compatible, &cancel).await? {
                    return Ok(java);
                }
            }
        }
        return Err(Error::DescargaDesactivada(major));
    }
    anunciar(&progress, "Obteniendo Java de Mojang");
    let client = client()?;
    let catalog: Catalogo = serde_json::from_slice(
        &bytes(
            &client,
            Url::parse(CATALOGO).map_err(|_| Error::Datos)?,
            2 * 1024 * 1024,
            &cancel,
        )
        .await?,
    )
    .map_err(|_| Error::Datos)?;
    let runtime = catalog
        .get(platform)
        .and_then(|p| p.get(component))
        .and_then(|r| {
            r.iter()
                .find(|r| major_version(&r.version.name).is_some_and(|m| perfil.acepta(m)))
        })
        .ok_or(Error::NoDisponible(major))?;
    let major = major_version(&runtime.version.name).ok_or(Error::Datos)?;
    if runtime.manifest.size > MAX_MANIFEST as u64 {
        return Err(Error::Datos);
    }
    let manifest_bytes = bytes(
        &client,
        url_oficial(&runtime.manifest, true)?,
        runtime.manifest.size as usize,
        &cancel,
    )
    .await?;
    comprobar_hash(&manifest_bytes, &runtime.manifest)?;
    let manifest: Manifiesto = serde_json::from_slice(&manifest_bytes).map_err(|_| Error::Datos)?;
    let plan = plan(&manifest)?;
    let stage_path = root.join(format!(".preparando-{}", nonce()?));
    fs::create_dir(&stage_path).map_err(|_| Error::Disco)?;
    let mut stage = Temporal {
        root: root.clone(),
        path: fs::canonicalize(&stage_path).map_err(|_| Error::Disco)?,
        publicado: false,
    };
    for directory in &plan.carpetas {
        crear_carpeta(&stage.path.join(directory))?;
    }
    let mut buckets: [Vec<&Archivo>; 4] = std::array::from_fn(|_| Vec::new());
    for (n, file) in plan.archivos.iter().enumerate() {
        buckets[n % 4].push(file);
    }
    let count = AtomicU64::new(0);
    progress(Progreso {
        etapa: "Descargando Java",
        completados: 0,
        total: plan.total,
    });
    tokio::try_join!(
        worker(
            &client,
            &stage.path,
            &buckets[0],
            &cancel,
            &count,
            plan.total,
            &progress
        ),
        worker(
            &client,
            &stage.path,
            &buckets[1],
            &cancel,
            &count,
            plan.total,
            &progress
        ),
        worker(
            &client,
            &stage.path,
            &buckets[2],
            &cancel,
            &count,
            plan.total,
            &progress
        ),
        worker(
            &client,
            &stage.path,
            &buckets[3],
            &cancel,
            &count,
            plan.total,
            &progress
        ),
    )?;
    anunciar(&progress, "Verificando Java");
    for (target, source) in &plan.enlaces {
        comprobar_cancelacion(&cancel)?;
        let target = stage.path.join(target);
        crear_carpeta(target.parent().ok_or(Error::Datos)?)?;
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .map_err(|_| Error::Disco)?;
        let mut input = fs::File::open(stage.path.join(&source.ruta)).map_err(|_| Error::Disco)?;
        std::io::copy(&mut input, &mut output).map_err(|_| Error::Disco)?;
        output.sync_all().map_err(|_| Error::Disco)?;
        drop(output);
        permiso_ejecutable(&target, source.executable)?;
    }
    fs::write(stage.path.join(MANIFEST), &manifest_bytes).map_err(|_| Error::Disco)?;
    comprobar_runtime(&stage.path, &plan, &cancel)?;
    let mut java = inspeccionar(
        &stage.path.join("bin").join(nombre_java()),
        major,
        true,
        &cancel,
    )
    .await?;
    comprobar_cancelacion(&cancel)?;
    // Una caché dañada no se borra ni se sobrescribe: se publica otra carpeta.
    let final_path = root.join(format!("{prefix}{}-{}", runtime.manifest.sha1, nonce()?));
    if final_path.exists() {
        return Err(Error::Disco);
    }
    fs::rename(&stage.path, &final_path).map_err(|_| Error::Disco)?;
    stage.publicado = true;
    java.executable = final_path.join("bin").join(nombre_java());
    progress(Progreso {
        etapa: "Java preparado",
        completados: plan.total,
        total: plan.total,
    });
    Ok(java)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn download(bytes: &[u8]) -> Descarga {
        let sha1 = format!("{:x}", Sha1::digest(bytes));
        Descarga {
            url: format!("https://piston-data.mojang.com/v1/objects/{sha1}/java"),
            sha1,
            size: bytes.len() as u64,
        }
    }

    #[test]
    fn reconoce_java_legacy_moderno_y_ea_sin_aceptar_texto_ajeno() {
        for (text, major, version) in [
            (
                "java version \"1.8.0_51\"\nJava(TM) SE Runtime Environment",
                8,
                "1.8.0_51",
            ),
            (
                "openjdk version \"17.0.15\" 2025-04-15\nOpenJDK Runtime Environment",
                17,
                "17.0.15",
            ),
            (
                "openjdk 21-ea 2023-09-19\nOpenJDK 64-Bit Server VM",
                21,
                "21-ea",
            ),
            ("openjdk version \"25.0.1+8\" 2025-10-21", 25, "25.0.1+8"),
        ] {
            assert_eq!(parse_version(text), Some((major, version.into())));
        }
        for text in [
            "foo version \"21\"",
            "Picked up JAVA_TOOL_OPTIONS: 21",
            "openjdk version \"abc\"",
            "java version \"0\"",
            "java version \"99999999999999999\"",
        ] {
            assert!(parse_version(text).is_none());
        }
        assert_eq!(major_version("8u51-cacert462b08"), Some(8));
    }

    #[test]
    fn rutas_y_links_no_pueden_salir_del_runtime() {
        for bad in [
            "../java",
            "/java",
            "C:/java",
            "bin\\java",
            "bin//java",
            "bin/../java",
            "bin/java:stream",
            "bin/CON.exe",
            "bin/java.",
            "bin/ java ",
        ] {
            assert!(ruta_relativa(bad).is_err(), "{bad}");
        }
        assert_eq!(
            resolver_enlace("legal/java.desktop/LICENSE", "../java.base/LICENSE").unwrap(),
            "legal/java.base/LICENSE"
        );
        for bad in [
            "../../../outside",
            "C:/outside",
            "/outside",
            "..\\outside",
            "",
        ] {
            assert!(resolver_enlace("legal/LICENSE", bad).is_err());
        }
    }

    #[test]
    fn hash_tamano_y_origen_se_comprueban_por_separado() {
        let data = b"verified runtime";
        let mut d = download(data);
        assert!(comprobar_hash(data, &d).is_ok());
        assert!(url_oficial(&d, false).is_ok());
        assert!(matches!(
            comprobar_hash(b"modified runtime", &d),
            Err(Error::Hash)
        ));
        d.size += 1;
        assert!(matches!(comprobar_hash(data, &d), Err(Error::Hash)));
        for host in [
            "http://piston-data.mojang.com",
            "https://piston-data.mojang.com.attacker.example",
            "https://attacker.example",
        ] {
            d.url = format!("{host}/v1/objects/{}/java", d.sha1);
            assert!(url_oficial(&d, false).is_err());
        }
    }

    #[test]
    fn plan_rechaza_ciclos_aliases_y_padres_que_son_archivos() {
        let java_key = format!("bin/{}", nombre_java());
        let file = Entrada::File {
            downloads: Descargas {
                raw: download(b"java"),
            },
            executable: true,
        };
        let mut m = Manifiesto {
            files: BTreeMap::from([
                (java_key, file.clone()),
                ("lib/a".into(), Entrada::Link { target: "b".into() }),
                ("lib/b".into(), Entrada::Link { target: "a".into() }),
            ]),
        };
        assert!(plan(&m).is_err());
        m.files.remove("lib/b");
        m.files.insert("lib/b".into(), file.clone());
        assert!(plan(&m).is_ok());
        m.files.insert("LIB/B".into(), file.clone());
        assert!(plan(&m).is_err());
        m.files.remove("LIB/B");
        m.files.insert("lib/b/nested".into(), file);
        assert!(plan(&m).is_err());
    }

    #[test]
    fn cache_modificada_y_temporal_ajeno_no_se_aceptan() {
        let root = std::env::temp_dir().join(format!("icaro-java-test-{}", nonce().unwrap()));
        let root = crear_carpeta(&root).unwrap();
        let stage = root.join("stage");
        fs::create_dir(&stage).unwrap();
        let expected = b"untampered";
        fs::write(stage.join("java"), expected).unwrap();
        let file = Archivo {
            ruta: PathBuf::from("java"),
            descarga: download(expected),
            executable: false,
        };
        assert!(comprobar_archivo(&stage, &file, &CancellationToken::new()).is_ok());
        fs::write(stage.join("java"), b"tamperedxx").unwrap();
        assert!(matches!(
            comprobar_archivo(&stage, &file, &CancellationToken::new()),
            Err(Error::Hash)
        ));
        let foreign = root.with_file_name(format!("icaro-java-foreign-{}", nonce().unwrap()));
        fs::create_dir(&foreign).unwrap();
        drop(Temporal {
            root: root.clone(),
            path: foreign.clone(),
            publicado: false,
        });
        assert!(foreign.is_dir());
        fs::remove_dir(&foreign).unwrap();
        drop(Temporal {
            root: root.clone(),
            path: stage.clone(),
            publicado: false,
        });
        assert!(!stage.exists());
        fs::remove_dir(&root).unwrap();
    }

    #[tokio::test]
    async fn cancelacion_previa_no_crea_carpetas_y_java_explicito_invalido_no_descarga() {
        let root = std::env::temp_dir().join(format!("icaro-java-cancel-{}", nonce().unwrap()));
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert!(matches!(
            preparar(&root, 21, "java-runtime-delta", None, cancel, |_| {}).await,
            Err(Error::Cancelada)
        ));
        assert!(!root.exists());
        let missing = root.join("missing.exe");
        assert!(matches!(
            preparar(
                &root,
                21,
                "java-runtime-delta",
                Some(&missing),
                CancellationToken::new(),
                |_| {}
            )
            .await,
            Err(Error::Ejecutable)
        ));
        assert!(!root.exists());
    }

    #[test]
    fn lista_guardada_no_ejecuta_java_y_omite_caches_alteradas() {
        let base = std::env::temp_dir().join(format!("icaro-java-lista-{}", nonce().unwrap()));
        let root = crear_carpeta(&base.join("java")).unwrap();
        let executable = b"Esto no es un ejecutable: la lista no debe lanzarlo.";
        let release = b"JAVA_VERSION=\"25.0.1\"\n";
        let manifest = Manifiesto {
            files: BTreeMap::from([
                ("bin".into(), Entrada::Directory),
                (
                    format!("bin/{}", nombre_java()),
                    Entrada::File {
                        downloads: Descargas {
                            raw: download(executable),
                        },
                        executable: true,
                    },
                ),
                (
                    "release".into(),
                    Entrada::File {
                        downloads: Descargas {
                            raw: download(release),
                        },
                        executable: false,
                    },
                ),
            ]),
        };
        let data = serde_json::to_vec(&manifest).unwrap();
        let path = root.join(format!(
            "java-runtime-epsilon-{}-{:x}-1234",
            plataforma().unwrap(),
            Sha1::digest(&data)
        ));
        fs::create_dir(&path).unwrap();
        fs::create_dir(path.join("bin")).unwrap();
        fs::write(path.join("bin").join(nombre_java()), executable).unwrap();
        fs::write(path.join("release"), release).unwrap();
        fs::write(path.join(MANIFEST), &data).unwrap();
        let list = gestionados(&base);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].major, 25);
        assert_eq!(list[0].version, "25.0.1");
        assert!(list[0].managed);
        fs::write(path.join("release"), b"JAVA_VERSION=\"21.0.7\"\n").unwrap();
        assert!(gestionados(&base).is_empty());
        fs::write(path.join("release"), release).unwrap();
        fs::write(path.join("bin").join(nombre_java()), b"Alterado").unwrap();
        assert!(gestionados(&base).is_empty());
        let absolute_base = fs::canonicalize(&base).unwrap();
        let temp_root = fs::canonicalize(std::env::temp_dir()).unwrap();
        assert_eq!(absolute_base.parent(), Some(temp_root.as_path()));
        fs::remove_dir_all(absolute_base).unwrap();
    }

    #[test]
    fn deteccion_ordenada_elimina_duplicados_y_rutas_ausentes() {
        let root = std::env::temp_dir().join(format!("icaro-java-detect-{}", nonce().unwrap()));
        for name in ["jdk-b", "jdk-a"] {
            let bin = root.join(name).join("bin");
            fs::create_dir_all(&bin).unwrap();
            fs::write(bin.join(nombre_java()), b"fixture").unwrap();
        }
        let preferred = root.join("jdk-b/bin").join(nombre_java());
        let mut paths = vec![preferred.clone()];
        agregar_instalaciones(&root, &mut paths);
        let paths = depurar_candidatos(paths);
        assert_eq!(
            paths,
            vec![
                fs::canonicalize(preferred).unwrap(),
                fs::canonicalize(root.join("jdk-a/bin").join(nombre_java())).unwrap(),
            ]
        );
        let root = fs::canonicalize(root).unwrap();
        assert_eq!(
            root.parent(),
            Some(fs::canonicalize(std::env::temp_dir()).unwrap().as_path())
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn java_local_prioritario_y_respaldo_sin_red() {
        // Ejecutables aislados: no depende del Java del equipo ni modifica PATH.
        let root = std::env::temp_dir().join(format!("icaro-java-priority-{}", nonce().unwrap()));
        fs::create_dir(&root).unwrap();
        let source = root.join("fixture.rs");
        fs::write(&source, r#"fn main() {
            let exe = std::env::current_exe().unwrap();
            let nombre = exe.file_stem().unwrap().to_string_lossy();
            let version = if nombre.contains("17") { "17.0.1" } else if nombre.contains("25") { "25.0.1" } else { "21.0.1" };
            let args: Vec<_> = std::env::args().skip(1).collect();
            for arg in &args {
                if let Some(path) = arg.strip_prefix("-Dtest.capture=") { std::fs::write(path, args.join("\n")).unwrap(); }
            }
            if args.iter().any(|arg| arg == "-XX:IcaroInvalidFlag=true") {
                eprintln!("Unrecognized VM option IcaroInvalidFlag"); std::process::exit(1);
            }
            eprintln!("os.arch = {}", std::env::consts::ARCH);
            eprintln!("openjdk version \"{}\"", version);
        }"#).unwrap();
        let local = root.join(if cfg!(windows) {
            "local21.exe"
        } else {
            "local21"
        });
        let build = tokio::process::Command::new("rustc")
            .arg(&source)
            .arg("-o")
            .arg(&local)
            .output()
            .await
            .unwrap();
        assert!(
            build.status.success(),
            "{}",
            String::from_utf8_lossy(&build.stderr)
        );
        let incompatible = local.with_file_name(if cfg!(windows) {
            "local17.exe"
        } else {
            "local17"
        });
        fs::copy(&local, &incompatible).unwrap();
        let reciente = local.with_file_name(if cfg!(windows) { "local25.exe" } else { "local25" });
        fs::copy(&local, &reciente).unwrap();
        let base = root.join("data");
        let resolve = |candidates: Vec<PathBuf>| {
            preparar_con_candidatos(
                &base,
                21,
                "java-runtime-delta",
                None,
                CancellationToken::new(),
                |_| {},
                false,
                move || candidates,
            )
        };
        let java = resolve(vec![incompatible.clone(), local.clone()])
            .await
            .unwrap();
        assert_eq!(java.executable, fs::canonicalize(&local).unwrap());
        assert!(!java.managed);
        assert!(matches!(
            preparar_con_candidatos(
                &root.join("sin-java"),
                21,
                "java-runtime-delta",
                None,
                CancellationToken::new(),
                |_| {},
                false,
                Vec::new
            )
            .await,
            Err(Error::DescargaDesactivada(21))
        ));
        assert!(
            !base.exists(),
            "Java local no debe crear almacenamiento ni descargar"
        );

        let bytes = fs::read(&local).unwrap();
        let manifest = Manifiesto {
            files: BTreeMap::from([
                ("bin".into(), Entrada::Directory),
                (
                    format!("bin/{}", nombre_java()),
                    Entrada::File {
                        downloads: Descargas {
                            raw: download(&bytes),
                        },
                        executable: true,
                    },
                ),
            ]),
        };
        let data = serde_json::to_vec(&manifest).unwrap();
        let managed = base.join("java").join(format!(
            "java-runtime-delta-{}-{:x}",
            plataforma().unwrap(),
            Sha1::digest(&data)
        ));
        fs::create_dir_all(managed.join("bin")).unwrap();
        let managed_exe = managed.join("bin").join(nombre_java());
        fs::copy(&local, &managed_exe).unwrap();
        fs::write(managed.join(MANIFEST), data).unwrap();
        let java = resolve(vec![local.clone()]).await.unwrap();
        assert_eq!(java.executable, fs::canonicalize(&local).unwrap());
        assert!(
            !java.managed,
            "El sistema tiene prioridad sobre el runtime guardado"
        );
        let profile = Perfil {
            majors: vec![17, 21],
            component: "java-runtime-delta".into(),
            prism: true,
        };
        let automatic = preparar_perfil_con_candidatos(
            &base,
            &profile,
            None,
            CancellationToken::new(),
            |_| {},
            true,
            || panic!("Con descargas no se busca Java del sistema"),
        )
        .await
        .unwrap();
        assert!(automatic.managed);
        assert_eq!(
            automatic.executable,
            fs::canonicalize(&managed_exe).unwrap()
        );
        let manual = preparar_perfil_con_candidatos(
            &base,
            &profile,
            Some(&incompatible),
            CancellationToken::new(),
            |_| {},
            true,
            || panic!("No sustituir selección manual compatible"),
        )
        .await
        .unwrap();
        assert_eq!(manual.major, 17);
        let solo21 = Perfil { majors: vec![21], component: profile.component.clone(), prism: true };
        let manual25 = preparar_perfil_con_candidatos(
            &base, &solo21, Some(&reciente), CancellationToken::new(), |_| {}, true,
            || panic!("No sustituir Java 25 seleccionado manualmente"),
        ).await.unwrap();
        assert_eq!(manual25.major, 25);
        assert_eq!(manual25.executable, fs::canonicalize(&reciente).unwrap());
        probar(&reciente, Some(&solo21), &crate::lanzamiento::Config::default(), CancellationToken::new()).await.unwrap();
        let automatico = preparar_perfil_con_candidatos(
            &base, &solo21, None, CancellationToken::new(), |_| {}, false,
            || vec![reciente.clone(), local.clone()],
        ).await.unwrap();
        assert_eq!(automatico.major, 21);
        let detected = preparar_perfil_con_candidatos(
            &base,
            &profile,
            None,
            CancellationToken::new(),
            |_| {},
            false,
            || vec![incompatible.clone(), local.clone()],
        )
        .await
        .unwrap();
        assert_eq!(detected.major, 21);
        let capture = root.join("argumentos.txt");
        let config = crate::lanzamiento::Config { memoria_min_mb: 256, memoria_max_mb: 512,
            argumentos_jvm: format!("-XX:+UseG1GC -XX:MaxMetaspaceSize=256M -Dtest.label=\"dos palabras\" -Dtest.capture=\"{}\"", capture.display()),
            ..Default::default() };
        probar(&local, Some(&profile), &config, CancellationToken::new())
            .await
            .unwrap();
        let args = fs::read_to_string(&capture).unwrap();
        for arg in [
            "-Xms256M",
            "-Xmx512M",
            "-XX:+UseG1GC",
            "-XX:MaxMetaspaceSize=256M",
            "-Dtest.label=dos palabras",
        ] {
            assert!(args.lines().any(|a| a == arg), "Argumento ausente: {arg}");
        }
        let invalid = crate::lanzamiento::Config {
            argumentos_jvm: "-XX:IcaroInvalidFlag=true".into(),
            ..config.clone()
        };
        assert!(matches!(
            probar(&local, None, &invalid, CancellationToken::new()).await,
            Err(Error::PruebaJvm(_))
        ));
        let invalid_memory = crate::lanzamiento::Config {
            memoria_min_mb: 1024,
            memoria_max_mb: 256,
            ..config
        };
        assert!(matches!(
            probar(&local, None, &invalid_memory, CancellationToken::new()).await,
            Err(Error::PruebaJvm(_))
        ));
        let fallback = resolve(vec![incompatible.clone()]).await.unwrap();
        assert!(fallback.managed);
        assert_eq!(fallback.executable, fs::canonicalize(&managed_exe).unwrap());
        assert!(matches!(
            preparar_con_candidatos(
                &base,
                21,
                "java-runtime-delta",
                Some(&incompatible),
                CancellationToken::new(),
                |_| {},
                false,
                || panic!("No sustituir Java manual")
            )
            .await,
            Err(Error::Incompatible {
                requerida: 21,
                encontrada: 17
            })
        ));
        let root = fs::canonicalize(root).unwrap();
        assert_eq!(
            root.parent(),
            Some(fs::canonicalize(std::env::temp_dir()).unwrap().as_path())
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn arquitectura_java_debe_corresponder_a_los_nativos() {
        assert!(comprobar_arquitectura(std::env::consts::ARCH).is_ok());
        assert!(matches!(
            comprobar_arquitectura("desconocida"),
            Err(Error::Arquitectura { .. })
        ));
        if cfg!(target_arch = "x86_64") {
            assert!(comprobar_arquitectura("amd64").is_ok());
            assert!(matches!(
                comprobar_arquitectura("x86"),
                Err(Error::Arquitectura { .. })
            ));
        }
    }

    #[test]
    fn metadatos_de_perfil_respetan_identidad_y_lista_explicita() {
        let data = br#"{"uid":"net.minecraft","version":"fixture","formatVersion":1,"compatibleJavaMajors":[17,21],"compatibleJavaName":"java-runtime-delta"}"#;
        let profile = perfil_metadata(data, "fixture").unwrap();
        assert!(profile.prism && profile.acepta(17) && profile.acepta(21));
        assert!(!profile.acepta(25));
        assert!(perfil_metadata(data, "otra-version").is_err());
        let empty = br#"{"uid":"net.minecraft","version":"fixture","formatVersion":1,"compatibleJavaMajors":[],"compatibleJavaName":"java-runtime-delta"}"#;
        assert!(perfil_metadata(empty, "fixture").is_err());
        let properties = "    java.property = value\n".repeat(80) + "openjdk version \"25.0.1\"\n";
        assert_eq!(parse_version(&properties).unwrap().0, 25);
    }

    #[test]
    fn runtime_oficial_anidado_se_detecta_sin_recorrer_librerias() {
        let root = std::env::temp_dir().join(format!("icaro-minecraft-java-{}", nonce().unwrap()));
        let bin = root.join("java-runtime-delta/windows-x64/java-runtime-delta/bin");
        fs::create_dir_all(&bin).unwrap();
        fs::write(bin.join(nombre_java()), b"fixture").unwrap();
        let library = root.join("lib/unused/bin");
        fs::create_dir_all(&library).unwrap();
        fs::write(library.join(nombre_java()), b"No es otro runtime").unwrap();
        let mut paths = Vec::new();
        agregar_runtimes_minecraft(&root, &mut paths);
        assert_eq!(
            depurar_candidatos(paths),
            vec![fs::canonicalize(bin.join(nombre_java())).unwrap()]
        );
        let root = fs::canonicalize(root).unwrap();
        assert_eq!(
            root.parent(),
            Some(fs::canonicalize(std::env::temp_dir()).unwrap().as_path())
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    #[ignore = "Ejecuta una JVM local indicada explícitamente en ICARO_QA_JAVA"]
    async fn prueba_jvm_real_con_memoria_argumentos_y_error() {
        let path =
            PathBuf::from(std::env::var_os("ICARO_QA_JAVA").expect("ICARO_QA_JAVA requerido"));
        let config = crate::lanzamiento::Config {
            memoria_min_mb: 256,
            memoria_max_mb: 512,
            argumentos_jvm: "-XX:+UseG1GC -XX:MaxMetaspaceSize=256M -Dtest.label=\"dos palabras\""
                .into(),
            ..Default::default()
        };
        let java = probar(&path, None, &config, CancellationToken::new())
            .await
            .unwrap();
        assert!(!java.version.is_empty());
        let invalid = crate::lanzamiento::Config {
            argumentos_jvm: "-XX:IcaroInvalidFlag=true".into(),
            ..config
        };
        assert!(matches!(
            probar(&path, None, &invalid, CancellationToken::new()).await,
            Err(Error::PruebaJvm(_))
        ));
    }

    #[tokio::test]
    #[ignore = "Consulta de lectura a los metadatos públicos de Prism"]
    async fn perfil_prism_real_por_version() {
        let fallback = Perfil {
            majors: vec![16],
            component: "java-runtime-alpha".into(),
            prism: false,
        };
        let profile = perfil_minecraft("1.17.1", fallback, &CancellationToken::new())
            .await
            .unwrap();
        assert!(profile.prism);
        assert!(profile.validar().is_ok());
    }

    /// Ejecutar sólo de forma explícita: descarga el runtime oficial en Temp,
    /// valida todos sus hashes y comprueba que un segundo uso funciona sin red.
    #[tokio::test]
    #[ignore = "descarga real del runtime oficial de Mojang"]
    async fn runtime_oficial_descarga_y_reutiliza_java() {
        let root = std::env::temp_dir().join(format!("icaro-java-smoke-{}", nonce().unwrap()));
        let java = preparar(
            &root,
            21,
            "java-runtime-delta",
            None,
            CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap();
        assert_eq!(java.major, 21);
        let again = preparar(
            &root,
            21,
            "java-runtime-delta",
            None,
            CancellationToken::new(),
            |_| {},
        )
        .await
        .unwrap();
        assert_eq!(java.executable, again.executable);
        let temp_root = fs::canonicalize(std::env::temp_dir()).unwrap();
        if root.is_dir() && fs::canonicalize(&root).unwrap().parent() == Some(temp_root.as_path()) {
            fs::remove_dir_all(&root).unwrap();
        }
    }
}
