// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Instalación Vanilla desde los manifiestos oficiales, con caché verificada.
//! Assets, bibliotecas y metadatos se comparten en la raíz de datos.
//! El perfil y los nativos extraídos permanecen dentro de cada instancia.

use crate::instancias::{Almacen, Instancia};
use serde::{Deserialize, Serialize};
use sha1::{Digest, Sha1};
use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::Duration,
};
use tokio_util::sync::CancellationToken;

mod forge;

const METADATA_LIMIT: u64 = 8 * 1024 * 1024;
const FILE_LIMIT: u64 = 1024 * 1024 * 1024;
const MAX_FILES: usize = 100_000;
const MARKER: &str = "instalado.json";
const PRIVADO: &str = ".icaro";
const PRIVADO_ANTERIOR: &str = ".icaro-minecraft";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instalado {
    pub version: String,
    #[serde(default)]
    pub perfil_id: Option<String>,
    pub main_class: String,
    pub classpath: Vec<PathBuf>,
    pub natives: PathBuf,
    pub assets: PathBuf,
    pub asset_index: String,
    pub java_major: u32,
    pub java_component: String,
    #[serde(default)]
    pub java_perfil: Option<crate::java::Perfil>,
    pub game_arguments: Vec<String>,
    #[serde(default)]
    pub quick_play_multiplayer: Option<bool>,
    pub jvm_arguments: Vec<String>,
}

impl Instalado {
    pub fn perfil_java(&self) -> crate::java::Perfil {
        self.java_perfil
            .clone()
            .unwrap_or_else(|| crate::java::Perfil {
                majors: vec![self.java_major],
                component: self.java_component.clone(),
                prism: false,
            })
    }
}

/// Los contadores representan archivos verificados, incluidos los reutilizados.
#[derive(Debug, Clone)]
pub struct Progreso {
    pub etapa: String,
    pub completados: u64,
    pub total: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("La instalación fue cancelada. Los archivos verificados se conservaron.")]
    Cancelada,
    #[error("Esta instancia ya se está preparando.")]
    Ocupada,
    #[error("No se pudo leer o guardar la instalación de Minecraft.")]
    Disco,
    #[error("El manifiesto oficial contiene datos inválidos o rutas no permitidas.")]
    Datos,
    #[error("No se pudo conectar con los servidores de Minecraft. Revisa tu conexión.")]
    Red,
    #[error("Minecraft respondió con HTTP {0}. Inténtalo de nuevo.")]
    Http(u16),
    #[error("El archivo descargado no coincide con el tamaño o SHA-1 oficial.")]
    Integridad,
    #[error("Esta versión o plataforma todavía no está soportada por Ícaro: {0}.")]
    NoSoportada(String),
    #[error("La versión no aparece en el catálogo oficial de Minecraft.")]
    Version,
    #[error("El instalador oficial de {0} no pudo completar el cliente. Revisa instalador.log en la preparación de meta/loaders y vuelve a intentarlo.")]
    Instalador(String),
    #[error("No se pudo instalar Fabric API: {0}")]
    FabricApi(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Descarga {
    url: String,
    sha1: String,
    size: u64,
    #[serde(default)]
    path: String,
    #[serde(default)]
    id: String,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Version {
    id: String,
    #[serde(rename = "type", default)]
    kind: String,
    main_class: String,
    #[serde(default)]
    inherits_from: Option<String>,
    downloads: BTreeMap<String, Descarga>,
    libraries: Vec<Libreria>,
    asset_index: Descarga,
    #[serde(default)]
    java_version: Option<JavaVersion>,
    #[serde(default)]
    arguments: Option<Argumentos>,
    #[serde(default)]
    minecraft_arguments: Option<String>,
    #[serde(default)]
    logging: BTreeMap<String, Logging>,
}
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JavaVersion {
    major_version: u32,
    component: String,
}
#[derive(Debug, Deserialize)]
struct Logging {
    argument: String,
    file: Descarga,
}
#[derive(Debug, Deserialize)]
struct Libreria {
    name: String,
    #[serde(default)]
    rules: Option<Vec<Regla>>,
    #[serde(default)]
    downloads: LibraryDownloads,
    #[serde(default)]
    natives: BTreeMap<String, String>,
    #[serde(default)]
    extract: Extraer,
}
#[derive(Debug, Default, Deserialize)]
struct LibraryDownloads {
    artifact: Option<Descarga>,
    #[serde(default)]
    classifiers: BTreeMap<String, Descarga>,
}
#[derive(Debug, Default, Deserialize)]
struct Extraer {
    #[serde(default)]
    exclude: Vec<String>,
}
#[derive(Debug, Serialize, Deserialize)]
struct Regla {
    action: String,
    os: Option<Os>,
    #[serde(default)]
    features: BTreeMap<String, bool>,
}
#[derive(Debug, Serialize, Deserialize)]
struct Os {
    name: Option<String>,
    arch: Option<String>,
    version: Option<String>,
    #[serde(rename = "versionRange")]
    version_range: Option<Rango>,
}
#[derive(Debug, Serialize, Deserialize)]
struct Rango {
    min: Option<String>,
    max: Option<String>,
}
#[derive(Debug, Default, Serialize, Deserialize)]
struct Argumentos {
    // Quilt omite `jvm` cuando no añade argumentos.
    #[serde(default)]
    game: Vec<Argumento>,
    #[serde(default)]
    jvm: Vec<Argumento>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum Argumento {
    Texto(String),
    Condicional {
        rules: Option<Vec<Regla>>,
        value: ValorArgumento,
    },
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum ValorArgumento {
    Texto(String),
    Lista(Vec<String>),
}
#[derive(Debug, Deserialize)]
struct Indice {
    objects: BTreeMap<String, Objeto>,
    #[serde(default, rename = "virtual")]
    virtual_: bool,
    #[serde(default)]
    map_to_resources: bool,
}
#[derive(Debug, Deserialize)]
struct Objeto {
    hash: String,
    size: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Archivo {
    descarga: Descarga,
    ruta: PathBuf,
    etapa: String,
}
#[derive(Debug, Serialize, Deserialize)]
struct Marcador {
    #[serde(default = "loader_vanilla")]
    loader: String,
    #[serde(default)]
    loader_version: Option<String>,
    schema: u32,
    sistema: String,
    instalado: Instalado,
    archivos: Vec<Archivo>,
    nativos: Vec<(PathBuf, u64, String)>,
}

fn loader_vanilla() -> String {
    "vanilla".into()
}

fn cancelado(cancel: &CancellationToken) -> Result<(), Error> {
    if cancel.is_cancelled() {
        Err(Error::Cancelada)
    } else {
        Ok(())
    }
}
fn plataforma() -> Result<&'static str, Error> {
    if std::env::consts::ARCH != "x86_64" {
        return Err(Error::NoSoportada("se requiere un sistema x64".into()));
    }
    match std::env::consts::OS {
        "windows" => Ok("windows"),
        "linux" => Ok("linux"),
        os => Err(Error::NoSoportada(os.into())),
    }
}
fn version_sistema() -> String {
    #[cfg(windows)]
    {
        use winreg::{enums::HKEY_LOCAL_MACHINE, RegKey};
        let key = RegKey::predef(HKEY_LOCAL_MACHINE)
            .open_subkey("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion");
        if let Ok(key) = key {
            let major: u32 = key.get_value("CurrentMajorVersionNumber").unwrap_or(10);
            let minor: u32 = key.get_value("CurrentMinorVersionNumber").unwrap_or(0);
            let build: String = key.get_value("CurrentBuildNumber").unwrap_or_default();
            return format!("{major}.{minor}.{build}");
        }
        String::new()
    }
    #[cfg(not(windows))]
    {
        std::fs::read_to_string("/proc/sys/kernel/osrelease")
            .unwrap_or_default()
            .trim()
            .into()
    }
}
fn version_partes(version: &str) -> Vec<u64> {
    version.split('.').map(|s| s.parse().unwrap_or(0)).collect()
}
fn permitida(rules: Option<&[Regla]>, os: &str, os_version: &str) -> Result<bool, Error> {
    let Some(rules) = rules else { return Ok(true) };
    let mut allowed = false;
    for rule in rules {
        if !matches!(rule.action.as_str(), "allow" | "disallow") {
            return Err(Error::Datos);
        }
        if rule.features.values().any(|v| *v) {
            continue;
        }
        if let Some(condition) = &rule.os {
            if condition.name.as_deref().is_some_and(|n| n != os)
                || condition
                    .arch
                    .as_deref()
                    .is_some_and(|a| !matches!(a, "x86_64" | "amd64"))
            {
                continue;
            }
            if let Some(pattern) = &condition.version {
                if pattern.len() > 256 {
                    return Err(Error::Datos);
                }
                let regex = regex::Regex::new(pattern).map_err(|_| Error::Datos)?;
                if !regex.is_match(os_version) {
                    continue;
                }
            }
            if let Some(range) = &condition.version_range {
                if range
                    .min
                    .as_ref()
                    .is_some_and(|min| version_partes(os_version) < version_partes(min))
                    || range
                        .max
                        .as_ref()
                        .is_some_and(|max| version_partes(os_version) >= version_partes(max))
                {
                    continue;
                }
            }
        }
        allowed = rule.action == "allow";
    }
    Ok(allowed)
}
fn argumentos(items: &[Argumento], os: &str, version: &str) -> Result<Vec<String>, Error> {
    let mut result = vec![];
    for item in items {
        match item {
            Argumento::Texto(text) => result.push(text.clone()),
            Argumento::Condicional { rules, value }
                if permitida(rules.as_deref(), os, version)? =>
            {
                match value {
                    ValorArgumento::Texto(text) => result.push(text.clone()),
                    ValorArgumento::Lista(values) => result.extend(values.iter().cloned()),
                }
            }
            _ => {}
        }
    }
    if result.len() > 1024 || result.iter().any(|s| s.len() > 16384 || s.contains('\0')) {
        return Err(Error::Datos);
    }
    Ok(result)
}
fn hash_valido(hash: &str) -> bool {
    hash.len() == 40
        && hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn nombre_valido(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        && value != "."
        && value != ".."
}
pub(crate) fn version_minecraft_valida(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && value.trim() == value
        && !value.ends_with('.') && value != "." && value != ".."
        && value.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b,b'.'|b'_'|b'-'|b' '))
}
fn relativa(value: &str) -> Result<PathBuf, Error> {
    if value.is_empty()
        || value.len() > 1024
        || value.contains(['\\', ':', '\0'])
        || value.starts_with('/')
        || value
            .split('/')
            .any(|s| s.is_empty() || s == "." || s == ".." || s.ends_with(['.', ' ']))
    {
        return Err(Error::Datos);
    }
    let path = PathBuf::from(value);
    if path
        .components()
        .any(|p| !matches!(p, Component::Normal(_)))
    {
        return Err(Error::Datos);
    }
    Ok(path)
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
/// Comprueba cada segmento antes de crear carpetas o abrir destinos de escritura.
fn segura(raiz: &Path, ruta: &Path, crear: bool) -> Result<(), Error> {
    let relative = ruta.strip_prefix(raiz).map_err(|_| Error::Datos)?;
    let root_meta = fs::symlink_metadata(raiz).map_err(|_| Error::Disco)?;
    if !root_meta.is_dir() || enlace(&root_meta) {
        return Err(Error::Datos);
    }
    let mut current = raiz.to_owned();
    let components: Vec<_> = relative.components().collect();
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(part) = component else {
            return Err(Error::Datos);
        };
        current.push(part);
        match fs::symlink_metadata(&current) {
            Ok(meta) => {
                if enlace(&meta) || (index + 1 < components.len() && !meta.is_dir()) {
                    return Err(Error::Datos);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if crear && index + 1 < components.len() {
                    if let Err(error) = fs::create_dir(&current) {
                        if error.kind() != std::io::ErrorKind::AlreadyExists {
                            return Err(Error::Disco);
                        }
                        let meta = fs::symlink_metadata(&current).map_err(|_| Error::Disco)?;
                        if !meta.is_dir() || enlace(&meta) {
                            return Err(Error::Datos);
                        }
                    }
                }
            }
            Err(_) => return Err(Error::Disco),
        }
    }
    Ok(())
}
fn oficial(value: &str) -> Result<reqwest::Url, Error> {
    let url = reqwest::Url::parse(value).map_err(|_| Error::Datos)?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(
            url.host_str(),
            Some(
                "piston-meta.mojang.com"
                    | "piston-data.mojang.com"
                    | "launchermeta.mojang.com"
                    | "launcher.mojang.com"
                    | "libraries.minecraft.net"
                    | "resources.download.minecraft.net"
                    | "meta.fabricmc.net"
                    | "maven.fabricmc.net"
                    | "maven.minecraftforge.net"
                    | "maven.neoforged.net"
                    | "meta.quiltmc.org"
                    | "maven.quiltmc.org"
            )
        )
    {
        return Err(Error::Datos);
    }
    Ok(url)
}
fn validar_descarga(download: &Descarga) -> Result<(), Error> {
    oficial(&download.url)?;
    if !hash_valido(&download.sha1) || download.size > FILE_LIMIT {
        return Err(Error::Datos);
    }
    Ok(())
}
fn sha1_archivo(path: &Path, cancel: &CancellationToken) -> Result<(u64, String), Error> {
    let mut file = File::open(path).map_err(|_| Error::Disco)?;
    let mut hash = Sha1::new();
    let mut buffer = [0u8; 64 * 1024];
    let mut len = 0u64;
    loop {
        cancelado(cancel)?;
        let n = file.read(&mut buffer).map_err(|_| Error::Disco)?;
        if n == 0 {
            break;
        }
        len += n as u64;
        if len > FILE_LIMIT {
            return Err(Error::Datos);
        }
        hash.update(&buffer[..n]);
    }
    Ok((len, format!("{:x}", hash.finalize())))
}
fn verificado(
    root: &Path,
    path: &Path,
    download: &Descarga,
    cancel: &CancellationToken,
) -> Result<bool, Error> {
    segura(root, path, false)?;
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(_) => return Err(Error::Disco),
    };
    if !meta.is_file() || enlace(&meta) {
        return Err(Error::Datos);
    }
    if meta.len() != download.size {
        return Ok(false);
    }
    Ok(sha1_archivo(path, cancel)?.1 == download.sha1)
}
fn random() -> Result<String, Error> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| Error::Disco)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
struct Temporal(PathBuf);
impl Temporal {
    fn crear(path: PathBuf) -> Result<(Self, File), Error> {
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|_| Error::Disco)?;
        Ok((Self(path), file))
    }
}
impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}
async fn descargar(
    client: &reqwest::Client,
    root: &Path,
    file: &Archivo,
    cancel: &CancellationToken,
) -> Result<(), Error> {
    descargar_observada(client, root, file, cancel, || {})
        .await
        .map(|_| ())
}

/// Solo anuncia una descarga cuando el archivo falta o no pasa la verificación.
async fn descargar_observada(
    client: &reqwest::Client,
    root: &Path,
    file: &Archivo,
    cancel: &CancellationToken,
    al_descargar: impl Fn(),
) -> Result<bool, Error> {
    validar_descarga(&file.descarga)?;
    cancelado(cancel)?;
    if verificado(root, &file.ruta, &file.descarga, cancel)? {
        return Ok(false);
    }
    al_descargar();
    segura(root, &file.ruta, true)?;
    let (temporary, mut disk) = Temporal::crear(
        file.ruta
            .parent()
            .ok_or(Error::Datos)?
            .join(format!(".icaro-{}.tmp", random()?)),
    )?;
    let mut response = tokio::select! {
        _ = cancel.cancelled() => return Err(Error::Cancelada),
        result = client.get(oficial(&file.descarga.url)?).send() => result.map_err(|_| Error::Red)?,
    };
    if !response.status().is_success() {
        return Err(Error::Http(response.status().as_u16()));
    }
    if response
        .content_length()
        .is_some_and(|len| len != file.descarga.size)
    {
        return Err(Error::Integridad);
    }
    let mut hash = Sha1::new();
    let mut written = 0u64;
    loop {
        let chunk = tokio::select! {
            _ = cancel.cancelled() => return Err(Error::Cancelada),
            chunk = response.chunk() => chunk.map_err(|_| Error::Red)?,
        };
        let Some(chunk) = chunk else { break };
        written += chunk.len() as u64;
        if written > file.descarga.size {
            return Err(Error::Integridad);
        }
        disk.write_all(&chunk).map_err(|_| Error::Disco)?;
        hash.update(&chunk);
    }
    if written != file.descarga.size || format!("{:x}", hash.finalize()) != file.descarga.sha1 {
        return Err(Error::Integridad);
    }
    disk.sync_all().map_err(|_| Error::Disco)?;
    drop(disk);
    cancelado(cancel)?;
    segura(root, &file.ruta, false)?;
    fs::rename(&temporary.0, &file.ruta).map_err(|_| Error::Disco)?;
    Ok(true)
}
async fn bytes_oficiales(
    client: &reqwest::Client,
    url: &str,
    limit: usize,
    cancel: &CancellationToken,
) -> Result<Vec<u8>, Error> {
    let response = tokio::select! {
        _ = cancel.cancelled() => return Err(Error::Cancelada),
        response = client.get(oficial(url)?).send() => response.map_err(|_| Error::Red)?,
    };
    if !response.status().is_success() {
        return Err(Error::Http(response.status().as_u16()));
    }
    tokio::select! {
        _ = cancel.cancelled() => Err(Error::Cancelada),
        bytes = crate::http::body_bytes(response, "Minecraft", limit) => bytes.map_err(|_| Error::Datos),
    }
}
fn leer_json<T: for<'de> Deserialize<'de>>(root: &Path, path: &Path) -> Result<T, Error> {
    segura(root, path, false)?;
    let meta = fs::symlink_metadata(path).map_err(|_| Error::Disco)?;
    if !meta.is_file() || meta.len() > METADATA_LIMIT || enlace(&meta) {
        return Err(Error::Datos);
    }
    let bytes = fs::read(path).map_err(|_| Error::Disco)?;
    serde_json::from_slice(&bytes).map_err(|_| Error::Datos)
}
fn escribir_json<T: Serialize>(root: &Path, path: &Path, value: &T) -> Result<(), Error> {
    segura(root, path, true)?;
    let bytes = serde_json::to_vec(value).map_err(|_| Error::Datos)?;
    if bytes.len() > METADATA_LIMIT as usize {
        return Err(Error::Datos);
    }
    let (temporary, mut file) = Temporal::crear(
        path.parent()
            .ok_or(Error::Datos)?
            .join(format!(".icaro-{}.tmp", random()?)),
    )?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| Error::Disco)?;
    drop(file);
    fs::rename(&temporary.0, path).map_err(|_| Error::Disco)
}
static INSTALANDO: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
// El bloqueo del SO también coordina dos procesos del launcher. El archivo
// permanece: borrarlo permitiría bloquear dos inodos distintos en Linux.
async fn bloqueo_compartido(
    root: &Path,
    cancel: &CancellationToken,
    progress: &impl Fn(Progreso),
) -> Result<File, Error> {
    let path = root.join("meta/minecraft/instalacion.lock");
    segura(root, &path, true)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|_| Error::Disco)?;
    let mut announced = false;
    loop {
        cancelado(cancel)?;
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(std::fs::TryLockError::WouldBlock) => {
                if !announced {
                    progress(Progreso {
                        etapa: "Esperando otra preparación de Minecraft".into(),
                        completados: 0,
                        total: 0,
                    });
                    announced = true;
                }
                tokio::select! {
                    _ = cancel.cancelled() => return Err(Error::Cancelada),
                    _ = tokio::time::sleep(Duration::from_millis(100)) => {}
                }
            }
            Err(_) => return Err(Error::Disco),
        }
    }
}

/// Reutiliza únicamente bytes que coinciden con el manifiesto oficial actual.
/// La copia antigua se retira después de publicar el perfil completo.
fn reutilizar(
    root: &Path,
    private: &Path,
    old: &Path,
    destination: &Path,
    expected: &Descarga,
    cancel: &CancellationToken,
    migrated: &mut Vec<(PathBuf, PathBuf, Descarga)>,
) -> Result<(), Error> {
    validar_descarga(expected)?;
    cancelado(cancel)?;
    if !verificado(private, old, expected, cancel)? {
        return Ok(());
    }
    if !verificado(root, destination, expected, cancel)? {
        segura(root, destination, true)?;
        let (temporary, mut output) = Temporal::crear(
            destination
                .parent()
                .ok_or(Error::Datos)?
                .join(format!(".icaro-{}.tmp", random()?)),
        )?;
        let mut input = File::open(old).map_err(|_| Error::Disco)?;
        std::io::copy(&mut input, &mut output).map_err(|_| Error::Disco)?;
        output.sync_all().map_err(|_| Error::Disco)?;
        drop(output);
        if !verificado(root, &temporary.0, expected, cancel)? {
            return Err(Error::Integridad);
        }
        segura(root, destination, false)?;
        fs::rename(&temporary.0, destination).map_err(|_| Error::Disco)?;
    }
    migrated.push((old.to_owned(), destination.to_owned(), expected.clone()));
    Ok(())
}
struct Guardia(PathBuf);
impl Guardia {
    fn adquirir(path: &Path) -> Result<Self, Error> {
        let mut set = INSTALANDO
            .get_or_init(Default::default)
            .lock()
            .map_err(|_| Error::Ocupada)?;
        if !set.insert(path.to_owned()) {
            return Err(Error::Ocupada);
        }
        Ok(Self(path.to_owned()))
    }
}
impl Drop for Guardia {
    fn drop(&mut self) {
        if let Ok(mut set) = INSTALANDO.get_or_init(Default::default).lock() {
            set.remove(&self.0);
        }
    }
}

fn nuevo_archivo(download: &Descarga, path: PathBuf, stage: &str) -> Result<Archivo, Error> {
    validar_descarga(download)?;
    Ok(Archivo {
        descarga: download.clone(),
        ruta: path,
        etapa: stage.into(),
    })
}
fn nativo_otro_arch(name: &str) -> bool {
    name.contains("natives-")
        && (name.ends_with("-arm64") || name.ends_with("-aarch64") || name.ends_with("-x86"))
}

/// Loaders con perfil oficial `inheritsFrom` y metadatos de tipo Fabric.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Familia {
    Fabric,
    Quilt,
}
impl Familia {
    fn de(loader: &str) -> Option<Self> {
        match loader {
            "fabric" => Some(Self::Fabric),
            "quilt" => Some(Self::Quilt),
            _ => None,
        }
    }
    fn nombre(self) -> &'static str {
        match self {
            Self::Fabric => "Fabric",
            Self::Quilt => "Quilt",
        }
    }
    /// Subcarpeta de `meta/` donde se guarda el perfil verificado.
    fn carpeta(self) -> &'static str {
        match self {
            Self::Fabric => "fabric",
            Self::Quilt => "quilt",
        }
    }
    fn api(self) -> &'static str {
        match self {
            Self::Fabric => "https://meta.fabricmc.net/v2/versions/loader",
            Self::Quilt => "https://meta.quiltmc.org/v3/versions/loader",
        }
    }
    fn id_perfil(self, loader: &str, mc: &str) -> String {
        match self {
            Self::Fabric => format!("fabric-loader-{loader}-{mc}"),
            Self::Quilt => format!("quilt-loader-{loader}-{mc}"),
        }
    }
    fn biblioteca(self, loader: &str) -> String {
        match self {
            Self::Fabric => format!("net.fabricmc:fabric-loader:{loader}"),
            Self::Quilt => format!("org.quiltmc:quilt-loader:{loader}"),
        }
    }
    /// Repositorios Maven desde los que el perfil oficial puede descargar.
    fn repositorios(self) -> &'static [&'static str] {
        match self {
            Self::Fabric => &["https://maven.fabricmc.net/"],
            Self::Quilt => &[
                "https://maven.quiltmc.org/repository/release/",
                "https://maven.fabricmc.net/",
            ],
        }
    }
    /// Mapeos que exige una versión ofuscada de Minecraft.
    fn mapeos(self, mc: &str) -> Vec<String> {
        let intermediary = format!("net.fabricmc:intermediary:{mc}");
        match self {
            Self::Fabric => vec![intermediary],
            Self::Quilt => vec![format!("org.quiltmc:hashed:{mc}"), intermediary],
        }
    }
    fn es_mapeo(self, name: &str) -> bool {
        name.starts_with("net.fabricmc:intermediary:")
            || (self == Self::Quilt && name.starts_with("org.quiltmc:hashed:"))
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct LibreriaCargador {
    name: String,
    url: String,
    #[serde(default)]
    sha1: Option<String>,
    #[serde(default)]
    size: Option<u64>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PerfilCargador {
    id: String,
    inherits_from: String,
    main_class: String,
    #[serde(default)]
    arguments: Argumentos,
    libraries: Vec<LibreriaCargador>,
}
#[derive(Debug, Serialize, Deserialize)]
struct Cargador {
    schema: u32,
    #[serde(default)]
    sin_intermediary: bool,
    minecraft: String,
    loader: String,
    min_java: u32,
    profile: PerfilCargador,
    descargas: Vec<Descarga>,
}
fn ruta_maven(name: &str) -> Result<String, Error> {
    let parts: Vec<_> = name.split(':').collect();
    if parts.len() != 3
        || parts.iter().enumerate().any(|(index,p)| {
            p.is_empty()
                || p.len() > 128
                || matches!(*p, "." | "..")
                || !p
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-' | b'+') || (index==2 && c==b' '))
        })
    {
        return Err(Error::Datos);
    }
    if parts[0].split('.').any(|s| s.is_empty()) {
        return Err(Error::Datos);
    }
    let path = format!(
        "{}/{}/{}/{}-{}.jar",
        parts[0].replace('.', "/"),
        parts[1],
        parts[2],
        parts[1],
        parts[2]
    );
    relativa(&path)?;
    Ok(path)
}
fn validar_cargador(
    familia: Familia,
    bundle: &Cargador,
    mc: &str,
    loader: &str,
) -> Result<(), Error> {
    if bundle.schema != 1
        || bundle.minecraft != mc
        || bundle.loader != loader
        || bundle.min_java == 0
        || bundle.min_java > 255
        || bundle.profile.id != familia.id_perfil(loader, mc)
        || bundle.profile.inherits_from != mc
        || bundle.profile.main_class.len() > 256
        || !bundle
            .profile
            .main_class
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'$'))
        || bundle.profile.libraries.is_empty()
        || bundle.profile.libraries.len() > 128
        || bundle.descargas.len() != bundle.profile.libraries.len()
    {
        return Err(Error::Datos);
    }
    let mut names = BTreeSet::new();
    for (lib, download) in bundle.profile.libraries.iter().zip(&bundle.descargas) {
        let path = ruta_maven(&lib.name)?;
        if !names.insert(lib.name.clone())
            || !familia.repositorios().contains(&lib.url.as_str())
            || download.path != path
            || download.url != format!("{}{}", lib.url, path)
            || lib.sha1.as_ref().is_some_and(|h| h != &download.sha1)
            || lib.size.is_some_and(|n| n != download.size)
            || download.size > 64 * 1024 * 1024
        {
            return Err(Error::Datos);
        }
        validar_descarga(download)?;
    }
    let mapeos_validos = if bundle.sin_intermediary {
        !names.iter().any(|name| familia.es_mapeo(name))
    } else {
        familia.mapeos(mc).iter().all(|mapeo| names.contains(mapeo))
    };
    if !names.contains(&familia.biblioteca(loader)) || !mapeos_validos {
        return Err(Error::Datos);
    }
    Ok(())
}
// Fabric conserva un marcador 0.0.0 para las versiones sin ofuscación y Quilt
// omite `hashed` e `intermediary`. Solo se admite cuando el perfil oficial
// tampoco incluye esos mapeos.
fn validar_detalle(
    familia: Familia,
    profile: &PerfilCargador,
    detail: &serde_json::Value,
    mc: &str,
    loader: &str,
) -> Result<bool, Error> {
    if profile.id != familia.id_perfil(loader, mc)
        || profile.inherits_from != mc
        || detail["loader"]["version"].as_str() != Some(loader)
        || detail["launcherMeta"]["mainClass"]["client"].as_str()
            != Some(profile.main_class.as_str())
    {
        return Err(Error::Datos);
    }
    let sin_intermediary = match familia {
        Familia::Fabric => {
            detail["intermediary"]["version"].as_str() == Some("0.0.0")
                && detail["intermediary"]["maven"].as_str()
                    == Some("net.fabricmc:intermediary:0.0.0")
        }
        Familia::Quilt => detail["hashed"].is_null() && detail["intermediary"].is_null(),
    };
    if sin_intermediary {
        if profile.libraries.iter().any(|lib| familia.es_mapeo(&lib.name)) {
            return Err(Error::Datos);
        }
    } else {
        let intermediary = detail["intermediary"]["version"].as_str() == Some(mc)
            && detail["intermediary"]["maven"].as_str()
                == Some(format!("net.fabricmc:intermediary:{mc}").as_str());
        let hashed = familia != Familia::Quilt
            || (detail["hashed"]["version"].as_str() == Some(mc)
                && detail["hashed"]["maven"].as_str()
                    == Some(format!("org.quiltmc:hashed:{mc}").as_str()));
        if !intermediary || !hashed {
            return Err(Error::Datos);
        }
    }
    Ok(sin_intermediary)
}
async fn preparar_cargador(
    familia: Familia,
    client: &reqwest::Client,
    root: &Path,
    mc: &str,
    loader: &str,
    cancel: &CancellationToken,
) -> Result<Cargador, Error> {
    if !version_minecraft_valida(mc) || !nombre_valido(loader) {
        return Err(Error::Datos);
    }
    let path = root
        .join("meta")
        .join(familia.carpeta())
        .join(mc)
        .join(loader)
        .join("perfil.json");
    if let Ok(bundle) = leer_json::<Cargador>(root, &path) {
        if validar_cargador(familia, &bundle, mc, loader).is_ok() {
            return Ok(bundle);
        }
    }
    let endpoint = format!("{}/{mc}/{loader}", familia.api());
    let profile: PerfilCargador = serde_json::from_slice(
        &bytes_oficiales(
            client,
            &format!("{endpoint}/profile/json"),
            METADATA_LIMIT as usize,
            cancel,
        )
        .await?,
    )
    .map_err(|_| Error::Datos)?;
    let detail: serde_json::Value = serde_json::from_slice(
        &bytes_oficiales(client, &endpoint, METADATA_LIMIT as usize, cancel).await?,
    )
    .map_err(|_| Error::Datos)?;
    let sin_intermediary = validar_detalle(familia, &profile, &detail, mc, loader)?;
    let min_java = detail["launcherMeta"]["min_java_version"]
        .as_u64()
        .filter(|n| *n > 0 && *n <= 255)
        .ok_or(Error::Datos)? as u32;
    if profile.libraries.len() > 128 {
        return Err(Error::Datos);
    }
    let mut downloads = vec![];
    for lib in &profile.libraries {
        cancelado(cancel)?;
        if !familia.repositorios().contains(&lib.url.as_str()) {
            return Err(Error::Datos);
        }
        let relative = ruta_maven(&lib.name)?;
        let url = format!("{}{}", lib.url, relative);
        let hash = if let Some(hash) = &lib.sha1 {
            hash.clone()
        } else {
            let bytes = bytes_oficiales(client, &format!("{url}.sha1"), 256, cancel).await?;
            std::str::from_utf8(&bytes)
                .map_err(|_| Error::Datos)?
                .split_whitespace()
                .next()
                .ok_or(Error::Datos)?
                .to_lowercase()
        };
        if !hash_valido(&hash) {
            return Err(Error::Datos);
        }
        let size = if let Some(size) = lib.size {
            size
        } else {
            let response = tokio::select! {_=cancel.cancelled()=>return Err(Error::Cancelada),r=client.head(oficial(&url)?).send()=>r.map_err(|_|Error::Red)?};
            if !response.status().is_success() {
                return Err(Error::Http(response.status().as_u16()));
            }
            response
                .headers()
                .get(reqwest::header::CONTENT_LENGTH)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .filter(|n| *n > 0 && *n <= 64 * 1024 * 1024)
                .ok_or(Error::Datos)?
        };
        downloads.push(Descarga {
            url,
            sha1: hash,
            size,
            path: relative,
            id: String::new(),
        });
    }
    let bundle = Cargador {
        schema: 1,
        sin_intermediary,
        minecraft: mc.into(),
        loader: loader.into(),
        min_java,
        profile,
        descargas: downloads,
    };
    validar_cargador(familia, &bundle, mc, loader)?;
    escribir_json(root, &path, &bundle)?;
    Ok(bundle)
}

fn directorio_lectura(folder: &Path) -> PathBuf {
    let current = folder.join(PRIVADO);
    if current.exists() {
        current
    } else {
        folder.join(PRIVADO_ANTERIOR)
    }
}

/// Renombra los datos internos y actualiza únicamente referencias a esa carpeta.
/// Nunca combina dos carpetas que puedan contener perfiles diferentes.
pub fn migrar_directorio(store: &Almacen, instance: &Instancia) -> Result<PathBuf, Error> {
    let folder = store
        .carpeta(&instance.id)
        .map_err(|_| Error::Disco)?
        .canonicalize()
        .map_err(|_| Error::Disco)?;
    if !folder.join(PRIVADO_ANTERIOR).exists() {
        return Ok(folder.join(PRIVADO));
    }
    let _lock = Guardia::adquirir(&folder)?;
    migrar_directorio_bloqueado(store, instance)
}
fn migrar_directorio_bloqueado(store: &Almacen, instance: &Instancia) -> Result<PathBuf, Error> {
    let folder = store
        .carpeta(&instance.id)
        .map_err(|_| Error::Disco)?
        .canonicalize()
        .map_err(|_| Error::Disco)?;
    let old = folder.join(PRIVADO_ANTERIOR);
    let target = folder.join(PRIVADO);
    segura(&folder, &old, false)?;
    segura(&folder, &target, false)?;
    if !old.exists() {
        return Ok(target);
    }
    if target.exists() {
        return Err(Error::NoSoportada(
            "existen .icaro y .icaro-minecraft; revisa sus perfiles antes de combinarlos".into(),
        ));
    }
    if !fs::symlink_metadata(&old)
        .map_err(|_| Error::Disco)?
        .is_dir()
    {
        return Err(Error::Datos);
    }
    let marker = old.join(MARKER);
    let mut value: Option<serde_json::Value> = if marker.exists() {
        Some(leer_json(&old, &marker)?)
    } else {
        None
    };
    fn actualizar(value: &mut serde_json::Value, old: &str, new: &str) {
        match value {
            serde_json::Value::String(text) => *text = text.replace(old, new),
            serde_json::Value::Array(values) => {
                for v in values {
                    actualizar(v, old, new)
                }
            }
            serde_json::Value::Object(values) => {
                for v in values.values_mut() {
                    actualizar(v, old, new)
                }
            }
            _ => {}
        }
    }
    if let Some(v) = &mut value {
        actualizar(
            v,
            old.to_str().ok_or(Error::Datos)?,
            target.to_str().ok_or(Error::Datos)?,
        );
    }
    fs::rename(&old, &target).map_err(|_| Error::Disco)?;
    if let Some(value) = value {
        if let Err(error) = escribir_json(&target, &target.join(MARKER), &value) {
            let _ = fs::rename(&target, &old);
            return Err(error);
        }
    }
    Ok(target)
}

/// Loaders que Ícaro sabe instalar y ejecutar con su perfil oficial.
pub fn loader_instalable(loader: &str) -> bool {
    matches!(loader, "vanilla" | "forge" | "neoforge") || Familia::de(loader).is_some()
}

/// Instala o repara todos los archivos necesarios y publica el marcador al final.
pub async fn instalar(
    store: &Almacen,
    instance: &Instancia,
    cancel: CancellationToken,
    progress: impl Fn(Progreso) + Send + Sync,
) -> Result<Instalado, Error> {
    instalar_con_java(store, instance, None, true, cancel, progress).await
}

/// Los instaladores que ejecutan procesadores respetan la selección y el permiso
/// de descarga de Java de la instancia, igual que el lanzamiento posterior.
pub async fn instalar_con_java(
    store: &Almacen,
    instance: &Instancia,
    java_preferida: Option<&Path>,
    descargar_java: bool,
    cancel: CancellationToken,
    progress: impl Fn(Progreso) + Send + Sync,
) -> Result<Instalado, Error> {
    if !loader_instalable(&instance.loader) {
        return Err(Error::NoSoportada(
            "el loader de esta instancia aún no se instala".into(),
        ));
    }
    let os = plataforma()?;
    let os_version = version_sistema();
    if !version_minecraft_valida(&instance.minecraft) {
        return Err(Error::Datos);
    }
    cancelado(&cancel)?;
    let folder = store
        .carpeta(&instance.id)
        .map_err(|_| Error::Disco)?
        .canonicalize()
        .map_err(|_| Error::Disco)?;
    let game = store
        .juego(&instance.id)
        .map_err(|_| Error::Disco)?
        .canonicalize()
        .map_err(|_| Error::Disco)?;
    let _lock = Guardia::adquirir(&folder)?;
    let private = migrar_directorio_bloqueado(store, instance)?;
    let root = store.datos().map_err(|_| Error::Disco)?;
    let _shared_lock = bloqueo_compartido(&root, &cancel, &progress).await?;
    segura(&root, &private, false)?;
    if !private.exists() {
        fs::create_dir(&private).map_err(|_| Error::Disco)?
    }
    if !fs::symlink_metadata(&private)
        .map_err(|_| Error::Disco)?
        .is_dir()
    {
        return Err(Error::Datos);
    }
    let marker_path = private.join(MARKER);
    // Solo la primera instalación normal añade el mod sugerido. Las reparaciones
    // y los modpacks conservan el conjunto que eligió el usuario o el autor.
    let default_fabric_api = instance.loader == "fabric"
        && !marker_path.exists()
        && !folder.join("modpack.json").exists();
    let loader_selected = if instance.loader != "vanilla" {
        Some(
            crate::versiones_instancia::leer_loader(store, instance).ok_or_else(|| {
                Error::NoSoportada(
                    "selecciona una versión oficial del loader para esta instancia".into(),
                )
            })?,
        )
    } else {
        None
    };
    let mut migrated = Vec::new();
    progress(Progreso {
        etapa: "Comprobando archivos de Minecraft".into(),
        completados: 0,
        total: 0,
    });
    let client = reqwest::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(180))
        .user_agent(crate::http::USER_AGENT)
        .build()
        .map_err(|_| Error::Red)?;

    // La copia del catálogo solo identifica metadatos cuyo SHA-1 se vuelve a comprobar.
    let manifest_path = root.join("meta/minecraft/catalogo.json");
    if !manifest_path.exists() {
        if let Ok(manifest) =
            leer_json::<crate::manifest::VersionManifest>(&private, &private.join("catalogo.json"))
        {
            escribir_json(&root, &manifest_path, &manifest)?;
        }
    }
    let cached: Option<crate::manifest::VersionManifest> = leer_json(&root, &manifest_path).ok();
    let cached_entry = cached
        .as_ref()
        .and_then(|m| m.versions.iter().find(|v| v.id == instance.minecraft))
        .cloned();
    let fresh = fs::metadata(&manifest_path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|date| date.elapsed().ok())
        .is_some_and(|age| age.as_secs() < 24 * 60 * 60);
    let entry = if fresh && cached_entry.is_some() {
        cached_entry.unwrap()
    } else {
        let fetched = bytes_oficiales(
            &client,
            crate::manifest::VERSION_MANIFEST_URL,
            METADATA_LIMIT as usize,
            &cancel,
        )
        .await;
        match fetched {
            Ok(bytes) => {
                let manifest =
                    crate::manifest::VersionManifest::parse(&bytes).map_err(|_| Error::Datos)?;
                let entry = manifest
                    .find(&instance.minecraft)
                    .map_err(|_| Error::Version)?
                    .clone();
                escribir_json(&root, &manifest_path, &manifest)?;
                entry
            }
            Err(Error::Cancelada) => return Err(Error::Cancelada),
            Err(error) => cached_entry.ok_or(error)?,
        }
    };
    oficial(&entry.url)?;
    if !hash_valido(&entry.sha1) {
        return Err(Error::Datos);
    }
    let version_path = root
        .join("meta/minecraft/versions")
        .join(&instance.minecraft)
        .join(&entry.sha1)
        .join("version.json");
    let old_version = private
        .join("versions")
        .join(&instance.minecraft)
        .join("version.json");
    if let Ok(meta) = fs::symlink_metadata(&old_version) {
        if meta.is_file() && meta.len() <= METADATA_LIMIT {
            let expected = Descarga {
                url: entry.url.clone(),
                sha1: entry.sha1.clone(),
                size: meta.len(),
                path: String::new(),
                id: String::new(),
            };
            reutilizar(
                &root,
                &private,
                &old_version,
                &version_path,
                &expected,
                &cancel,
                &mut migrated,
            )?;
        }
    }
    let mut version_file = None;
    if let Ok(meta) = fs::symlink_metadata(&version_path) {
        segura(&root, &version_path, false)?;
        if meta.is_file()
            && meta.len() <= METADATA_LIMIT
            && !enlace(&meta)
            && sha1_archivo(&version_path, &cancel)?.1 == entry.sha1
        {
            version_file = Some(nuevo_archivo(
                &Descarga {
                    url: entry.url.clone(),
                    sha1: entry.sha1.clone(),
                    size: meta.len(),
                    path: String::new(),
                    id: String::new(),
                },
                version_path.clone(),
                "Metadatos",
            )?);
        }
    }
    let version_file = match version_file {
        Some(file) => file,
        None => {
            let bytes =
                bytes_oficiales(&client, &entry.url, METADATA_LIMIT as usize, &cancel).await?;
            if crate::manifest::sha1_hex(&bytes) != entry.sha1 {
                return Err(Error::Integridad);
            }
            let download = Descarga {
                url: entry.url.clone(),
                sha1: entry.sha1.clone(),
                size: bytes.len() as u64,
                path: String::new(),
                id: String::new(),
            };
            segura(&root, &version_path, true)?;
            let (temporary, mut file) = Temporal::crear(
                version_path
                    .parent()
                    .ok_or(Error::Datos)?
                    .join(format!(".icaro-{}.tmp", random()?)),
            )?;
            file.write_all(&bytes)
                .and_then(|_| file.sync_all())
                .map_err(|_| Error::Disco)?;
            drop(file);
            fs::rename(&temporary.0, &version_path).map_err(|_| Error::Disco)?;
            nuevo_archivo(&download, version_path.clone(), "Metadatos")?
        }
    };
    let version: Version = leer_json(&root, &version_path)?;
    if version.id != instance.minecraft
        || version.inherits_from.is_some()
        || !nombre_valido(&version.asset_index.id)
        || version.main_class.len() > 256
        || !version
            .main_class
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'$'))
    {
        return Err(Error::Datos);
    }
    if version.libraries.len() > 4096 || version.asset_index.size > METADATA_LIMIT {
        return Err(Error::Datos);
    }
    let mut files = vec![version_file];
    let assets = root.join("assets");
    let asset_id = format!("{}-{}", version.asset_index.id, version.asset_index.sha1);
    let index_path = assets.join("indexes").join(format!("{asset_id}.json"));
    let index_file = nuevo_archivo(
        &version.asset_index,
        index_path.clone(),
        "Índice de recursos",
    )?;
    progress(Progreso {
        etapa: "Preparando los recursos de Minecraft".into(),
        completados: 0,
        total: 0,
    });
    reutilizar(
        &root,
        &private,
        &private
            .join("assets/indexes")
            .join(format!("{}.json", version.asset_index.id)),
        &index_path,
        &version.asset_index,
        &cancel,
        &mut migrated,
    )?;
    descargar(&client, &root, &index_file, &cancel).await?;
    files.push(index_file);
    let index: Indice = leer_json(&root, &index_path)?;
    if index.objects.len() > MAX_FILES {
        return Err(Error::Datos);
    }
    let mut hashes = BTreeSet::new();
    for (name, object) in &index.objects {
        relativa(name)?;
        if !hash_valido(&object.hash) {
            return Err(Error::Datos);
        }
        if hashes.insert(object.hash.clone()) {
            let path = assets
                .join("objects")
                .join(&object.hash[..2])
                .join(&object.hash);
            let download = Descarga {
                url: format!(
                    "https://resources.download.minecraft.net/{}/{}",
                    &object.hash[..2],
                    object.hash
                ),
                sha1: object.hash.clone(),
                size: object.size,
                path: String::new(),
                id: String::new(),
            };
            files.push(nuevo_archivo(&download, path, "Descargando recursos")?);
        }
    }
    let client_file = nuevo_archivo(
        version
            .downloads
            .get("client")
            .ok_or_else(|| Error::NoSoportada("la versión no ofrece cliente".into()))?,
        root.join("meta/minecraft/clients")
            .join(&version.downloads.get("client").ok_or(Error::Datos)?.sha1)
            .join("client.jar"),
        "Descargando Minecraft",
    )?;
    let mut classpath = vec![];
    let mut native_archives: Vec<(PathBuf, Vec<String>)> = vec![];
    for library in &version.libraries {
        if !permitida(library.rules.as_deref(), os, &os_version)? || nativo_otro_arch(&library.name)
        {
            continue;
        }
        if let Some(artifact) = &library.downloads.artifact {
            let path = root
                .join("libraries")
                .join(&artifact.sha1)
                .join(relativa(&artifact.path)?);
            files.push(nuevo_archivo(
                artifact,
                path.clone(),
                "Descargando librerías",
            )?);
            classpath.push(path);
        }
        if let Some(classifier) = library.natives.get(os) {
            let classifier = classifier.replace("${arch}", "64");
            let download = library
                .downloads
                .classifiers
                .get(&classifier)
                .ok_or(Error::Datos)?;
            let path = root
                .join("libraries")
                .join(&download.sha1)
                .join(relativa(&download.path)?);
            files.push(nuevo_archivo(
                download,
                path.clone(),
                "Descargando componentes nativos",
            )?);
            native_archives.push((path, library.extract.exclude.clone()));
        }
    }
    classpath.push(client_file.ruta.clone());
    files.push(client_file);
    let mut jvm = if let Some(args) = &version.arguments {
        argumentos(&args.jvm, os, &os_version)?
    } else {
        vec![
            "-Djava.library.path=${natives_directory}".into(),
            "-cp".into(),
            "${classpath}".into(),
        ]
    };
    let mut game_arguments = if let Some(args) = &version.arguments {
        argumentos(&args.game, os, &os_version)?
    } else {
        version
            .minecraft_arguments
            .as_ref()
            .ok_or_else(|| {
                Error::NoSoportada(
                    "esta versión usa un lanzamiento anterior a Minecraft 1.6".into(),
                )
            })?
            .split_whitespace()
            .map(str::to_owned)
            .collect()
    };
    if let Some(logging) = version.logging.get("client") {
        let log = root
            .join("assets")
            .join("log_configs")
            .join(&logging.file.sha1)
            .join(relativa(&logging.file.id)?);
        jvm.push(logging.argument.replace("${path}", &log.to_string_lossy()));
        files.push(nuevo_archivo(
            &logging.file,
            log,
            "Configurando los registros",
        )?);
    }
    let familia = Familia::de(&instance.loader);
    let fabric = if let Some(familia) = familia {
        let selected = loader_selected.as_deref().ok_or(Error::Datos)?;
        progress(Progreso {
            etapa: format!("Preparando el perfil oficial de {}", familia.nombre()),
            completados: 0,
            total: 0,
        });
        let bundle = preparar_cargador(
            familia,
            &client,
            &root,
            &instance.minecraft,
            &selected,
            &cancel,
        )
        .await?;
        let mut loader_classpath = Vec::new();
        for download in &bundle.descargas {
            let path = root
                .join("libraries")
                .join(&download.sha1)
                .join(relativa(&download.path)?);
            files.push(nuevo_archivo(download, path.clone(), familia.nombre())?);
            loader_classpath.push(path);
        }
        loader_classpath.extend(classpath);
        classpath = loader_classpath;
        jvm.extend(argumentos(&bundle.profile.arguments.jvm, os, &os_version)?);
        game_arguments.extend(argumentos(&bundle.profile.arguments.game, os, &os_version)?);
        Some(bundle)
    } else {
        None
    };
    let mut unique = BTreeMap::<PathBuf, Archivo>::new();
    for file in files {
        if let Some(previous) = unique.insert(file.ruta.clone(), file.clone()) {
            if previous.descarga.sha1 != file.descarga.sha1
                || previous.descarga.size != file.descarga.size
            {
                return Err(Error::Datos);
            }
        }
    }
    let files: Vec<_> = unique.into_values().collect();
    if files.len() > MAX_FILES
        || files
            .iter()
            .try_fold(0u64, |sum, file| sum.checked_add(file.descarga.size))
            .is_none_or(|size| size > 16 * 1024 * 1024 * 1024)
    {
        return Err(Error::Datos);
    }
    for (position, file) in files.iter().enumerate() {
        if position % 32 == 0 {
            progress(Progreso {
                etapa: "Reorganizando archivos verificados".into(),
                completados: position as u64,
                total: files.len() as u64,
            });
        }
        let old = if file.ruta.starts_with(root.join("libraries")) {
            Some(
                private
                    .join("libraries")
                    .join(relativa(&file.descarga.path)?),
            )
        } else if let Ok(relative) = file.ruta.strip_prefix(assets.join("objects")) {
            Some(private.join("assets/objects").join(relative))
        } else if file.ruta.starts_with(assets.join("log_configs")) {
            Some(
                private
                    .join("assets/log_configs")
                    .join(relativa(&file.descarga.id)?),
            )
        } else if file.ruta.starts_with(root.join("meta/minecraft/clients")) {
            Some(
                private
                    .join("versions")
                    .join(&version.id)
                    .join("client.jar"),
            )
        } else {
            None
        };
        if let Some(old) = old {
            reutilizar(
                &root,
                &private,
                &old,
                &file.ruta,
                &file.descarga,
                &cancel,
                &mut migrated,
            )?;
        }
    }
    let total = files.len() as u64;
    let mut completed = 0u64;
    let downloads_cancel = cancel.child_token();
    let mut tasks = tokio::task::JoinSet::new();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let mut remaining = files.iter().cloned();
    loop {
        while tasks.len() < 8 {
            let Some(file) = remaining.next() else { break };
            progress(Progreso {
                etapa: "Verificando archivos de Minecraft".into(),
                completados: completed,
                total,
            });
            let client = client.clone();
            let root = root.clone();
            let token = downloads_cancel.clone();
            let tx = tx.clone();
            tasks.spawn(async move {
                let result = descargar_observada(&client, &root, &file, &token, || {
                    let _ = tx.send((format!("Descargando {}", file.etapa), completed));
                })
                .await;
                (file.etapa, result)
            });
        }
        let result = tokio::select! {
            aviso = rx.recv() => {
                if let Some((etapa, completados)) = aviso {
                    progress(Progreso { etapa, completados, total });
                }
                continue;
            }
            result = tasks.join_next() => result,
        };
        let Some(result) = result else {
            break;
        };
        let (stage, result) = result.map_err(|_| Error::Disco)?;
        let descargado = match result {
            Ok(descargado) => descargado,
            Err(error) => {
                downloads_cancel.cancel();
                while tasks.join_next().await.is_some() {}
                return Err(error);
            }
        };
        completed += 1;
        progress(Progreso {
            etapa: if descargado {
                format!("Descargando {stage}")
            } else {
                "Verificando archivos de Minecraft".into()
            },
            completados: completed,
            total,
        });
    }
    cancelado(&cancel)?;
    let fallback_java = version
        .java_version
        .as_ref()
        .map(|java| crate::java::Perfil {
            majors: vec![java.major_version],
            component: java.component.clone(),
            prism: false,
        })
        .unwrap_or(crate::java::Perfil {
            majors: vec![8],
            component: "jre-legacy".into(),
            prism: false,
        });
    let mut forge_java = None;
    let forge = if matches!(instance.loader.as_str(), "forge" | "neoforge") {
        let runtime_profile = crate::java::perfil_minecraft(&version.id, fallback_java, &cancel)
            .await
            .map_err(|e| match e {
                crate::java::Error::Cancelada => Error::Cancelada,
                _ => Error::Datos,
            })?;
        let vanilla_jar = root
            .join("meta/minecraft/clients")
            .join(&version.downloads.get("client").ok_or(Error::Datos)?.sha1)
            .join("client.jar");
        let bundle = forge::preparar(
            &client,
            &root,
            instance,
            loader_selected.as_deref().ok_or(Error::Datos)?,
            &version_path,
            &vanilla_jar,
            &runtime_profile,
            (java_preferida, descargar_java),
            &cancel,
            &progress,
        )
        .await?;
        bundle.combinar(
            &version.libraries,
            &mut classpath,
            &mut jvm,
            &mut game_arguments,
            os,
            &os_version,
        )?;
        forge_java = Some(runtime_profile);
        Some(bundle)
    } else {
        None
    };
    progress(Progreso {
        etapa: "Preparando componentes nativos".into(),
        completados: completed,
        total,
    });
    let previous: Option<Marcador> = leer_json(&root, &marker_path).ok();
    let reuse = previous.filter(|marker| {
        marker.schema == 2
            && marker.sistema == format!("{os}-x86_64")
            && marker.instalado.version == version.id
            && marker.instalado.natives.parent() == Some(private.as_path())
            && segura(&root, &marker.instalado.natives, false).is_ok()
            && marker.instalado.natives.is_dir()
            && marker.archivos.len() == files.len()
            && marker.archivos.iter().zip(&files).all(|(old, current)| {
                old.ruta == current.ruta && old.descarga.sha1 == current.descarga.sha1
            })
            && marker.nativos.iter().all(|(path, size, hash)| {
                path.starts_with(&marker.instalado.natives)
                    && segura(&root, path, false).is_ok()
                    && sha1_archivo(path, &cancel)
                        .is_ok_and(|actual| actual.0 == *size && actual.1 == *hash)
            })
    });
    let (natives, natives_files) = if let Some(previous) = reuse {
        (previous.instalado.natives, previous.nativos)
    } else {
        let natives = private.join(format!("natives-{}", random()?));
        fs::create_dir(&natives).map_err(|_| Error::Disco)?;
        match extraer_nativos(&native_archives, &natives, &cancel) {
            Ok(files) => (natives, files),
            Err(error) => {
                // Solo se elimina la carpeta aleatoria creada en esta operación.
                if natives.parent() == Some(private.as_path()) {
                    let _ = fs::remove_dir_all(&natives);
                }
                return Err(error);
            }
        }
    };
    let game_assets = if index.virtual_ {
        assets.join("virtual").join(&asset_id)
    } else {
        assets.clone()
    };
    if index.virtual_ || index.map_to_resources {
        progress(Progreso {
            etapa: "Preparando recursos de esta versión".into(),
            completados: completed,
            total,
        });
        materializar(&root, &assets, &index, &game_assets, &game, &cancel)?;
    }
    for argument in &mut game_arguments {
        *argument = argument
            .replace("${version_type}", &version.kind)
            .replace("${game_assets}", &game_assets.to_string_lossy());
    }
    let java = version.java_version.unwrap_or(JavaVersion {
        major_version: 8,
        component: "jre-legacy".into(),
    });
    if (java.major_version == 0 || java.major_version > 255) || !nombre_valido(&java.component) {
        return Err(Error::NoSoportada(format!("Java {}", java.major_version)));
    }
    let fallback = crate::java::Perfil {
        majors: vec![java.major_version],
        component: java.component.clone(),
        prism: false,
    };
    let cached = leer_json::<Marcador>(&root, &marker_path)
        .ok()
        .filter(|m| m.loader == instance.loader && m.loader_version == loader_selected)
        .filter(|m| m.instalado.version == version.id)
        .and_then(|m| m.instalado.java_perfil)
        .filter(|p| p.prism && p.validar().is_ok());
    let mut java_perfil = if let Some(perfil) = forge_java.or(cached) {
        perfil
    } else {
        crate::java::perfil_minecraft(&version.id, fallback, &cancel)
            .await
            .map_err(|e| match e {
                crate::java::Error::Cancelada => Error::Cancelada,
                _ => Error::Datos,
            })?
    };
    if let (Some(bundle), Some(familia)) = (&fabric, familia) {
        java_perfil.majors.retain(|major| *major >= bundle.min_java);
        java_perfil.validar().map_err(|_| {
            Error::NoSoportada(format!(
                "{} necesita Java {} o superior y el perfil de Minecraft no lo admite",
                familia.nombre(),
                bundle.min_java
            ))
        })?;
    }
    let quick_play = version.arguments.as_ref().is_some_and(|a|
        serde_json::to_value(&a.game).ok().is_some_and(|v|v.to_string().contains("\"--quickPlayMultiplayer\"")));
    let installed = Instalado {
        quick_play_multiplayer: Some(quick_play),
        version: version.id,
        perfil_id: forge
            .as_ref()
            .map(|b| b.perfil.id.clone())
            .or_else(|| fabric.as_ref().map(|b| b.profile.id.clone())),
        main_class: forge
            .as_ref()
            .map(|b| b.perfil.main_class.clone())
            .or_else(|| fabric.as_ref().map(|b| b.profile.main_class.clone()))
            .unwrap_or(version.main_class),
        classpath,
        natives,
        assets,
        asset_index: asset_id,
        java_major: java_perfil.majors[0],
        java_component: java_perfil.component.clone(),
        java_perfil: Some(java_perfil),
        game_arguments,
        jvm_arguments: jvm,
    };
    cancelado(&cancel)?;
    if default_fabric_api {
        crate::mods::fabric_api::instalar_por_defecto(store, instance, cancel.clone(), |text| {
            progress(Progreso {
                etapa: text.into(),
                completados: 0,
                total: 0,
            })
        })
        .await
        .map_err(|e| {
            if cancel.is_cancelled() {
                Error::Cancelada
            } else {
                Error::FabricApi(e)
            }
        })?;
    }
    cancelado(&cancel)?;
    let current = store.leer(&instance.id).map_err(|_| Error::Disco)?;
    if current.minecraft != instance.minecraft
        || current.loader != instance.loader
        || crate::versiones_instancia::leer_loader(store, &current) != loader_selected
    {
        return Err(Error::Datos);
    }
    escribir_json(
        &root,
        &marker_path,
        &Marcador {
            loader: instance.loader.clone(),
            loader_version: loader_selected,
            schema: 2,
            sistema: format!("{os}-x86_64"),
            instalado: installed.clone(),
            archivos: files,
            nativos: natives_files,
        },
    )?;
    // Solo copias verificadas y ya publicadas en el almacén común.
    for (old, destination, expected) in migrated {
        if segura(&private, &old, false).is_ok()
            && verificado(&root, &destination, &expected, &cancel).unwrap_or(false)
            && verificado(&private, &old, &expected, &cancel).unwrap_or(false)
        {
            let _ = fs::remove_file(&old);
            let mut parent = old.parent();
            while let Some(dir) = parent.filter(|p| *p != private && p.starts_with(&private)) {
                if fs::remove_dir(dir).is_err() {
                    break;
                }
                parent = dir.parent();
            }
        }
    }
    progress(Progreso {
        etapa: "Minecraft preparado".into(),
        completados: total,
        total,
    });
    Ok(installed)
}

fn extraer_nativos(
    archives: &[(PathBuf, Vec<String>)],
    target: &Path,
    cancel: &CancellationToken,
) -> Result<Vec<(PathBuf, u64, String)>, Error> {
    let mut result = vec![];
    let mut seen = HashSet::new();
    let mut expanded = 0u64;
    for (path, excluded) in archives {
        cancelado(cancel)?;
        let file = File::open(path).map_err(|_| Error::Disco)?;
        let mut archive = zip::ZipArchive::new(file).map_err(|_| Error::Datos)?;
        if archive.len() > 10_000 {
            return Err(Error::Datos);
        }
        for i in 0..archive.len() {
            cancelado(cancel)?;
            let mut member = archive.by_index(i).map_err(|_| Error::Datos)?;
            let name = member.name().map_err(|_| Error::Datos)?.into_owned();
            if name.starts_with("META-INF/")
                || excluded.iter().any(|s| name.starts_with(s))
                || member.is_dir()
            {
                continue;
            }
            let relative = relativa(&name)?;
            if member
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 == 0o120000)
                || !seen.insert(relative.clone())
            {
                return Err(Error::Datos);
            }
            expanded = expanded.checked_add(member.size()).ok_or(Error::Datos)?;
            if member.size() > 128 * 1024 * 1024 || expanded > 512 * 1024 * 1024 {
                return Err(Error::Datos);
            }
            let destination = target.join(relative);
            segura(target, &destination, true)?;
            let mut output = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&destination)
                .map_err(|_| Error::Disco)?;
            let mut total = 0u64;
            let mut hash = Sha1::new();
            let mut buffer = [0u8; 64 * 1024];
            loop {
                cancelado(cancel)?;
                let n = member.read(&mut buffer).map_err(|_| Error::Datos)?;
                if n == 0 {
                    break;
                }
                total += n as u64;
                if total > member.size() {
                    return Err(Error::Datos);
                }
                output.write_all(&buffer[..n]).map_err(|_| Error::Disco)?;
                hash.update(&buffer[..n]);
            }
            if total != member.size() {
                return Err(Error::Integridad);
            }
            output.sync_all().map_err(|_| Error::Disco)?;
            result.push((destination, total, format!("{:x}", hash.finalize())));
        }
    }
    Ok(result)
}

fn materializar(
    root: &Path,
    assets: &Path,
    index: &Indice,
    virtual_root: &Path,
    game: &Path,
    cancel: &CancellationToken,
) -> Result<(), Error> {
    for (name, object) in &index.objects {
        cancelado(cancel)?;
        let relative = relativa(name)?;
        let source = assets
            .join("objects")
            .join(&object.hash[..2])
            .join(&object.hash);
        for (base, destination) in [
            (root, index.virtual_.then(|| virtual_root.join(&relative))),
            (
                game,
                index
                    .map_to_resources
                    .then(|| game.join("resources").join(&relative)),
            ),
        ] {
            let Some(destination) = destination else {
                continue;
            };
            segura(base, &destination, true)?;
            if destination.exists() {
                let meta = fs::symlink_metadata(&destination).map_err(|_| Error::Disco)?;
                if !meta.is_file() || enlace(&meta) {
                    return Err(Error::Datos);
                }
                // Minecraft anterior a 1.7 puede contener recursos personalizados del usuario.
                if base == game {
                    continue;
                }
                if sha1_archivo(&destination, cancel)?.1 == object.hash {
                    continue;
                }
            }
            let (temporary, mut file) = Temporal::crear(
                destination
                    .parent()
                    .ok_or(Error::Datos)?
                    .join(format!(".icaro-{}.tmp", random()?)),
            )?;
            let mut input = File::open(&source).map_err(|_| Error::Disco)?;
            std::io::copy(&mut input, &mut file).map_err(|_| Error::Disco)?;
            file.sync_all().map_err(|_| Error::Disco)?;
            drop(file);
            cancelado(cancel)?;
            segura(base, &destination, false)?;
            // Las copias privadas se reparan; nunca se sustituye un recurso del usuario.
            if base == game {
                // Publicación exclusiva: ni siquiera una creación simultánea del
                // juego puede ser sustituida por la copia de recursos legacy.
                match fs::hard_link(&temporary.0, &destination) {
                    Ok(()) => {}
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(_) => return Err(Error::Disco),
                }
            } else {
                fs::rename(&temporary.0, &destination).map_err(|_| Error::Disco)?;
            }
        }
    }
    Ok(())
}

/// Consulta rápida para las tarjetas; antes de lanzar `instalar` comprueba SHA-1.
pub fn listo(store: &Almacen, instance: &Instancia) -> bool {
    if !crate::modpacks::completo(store,instance){return false;}
    if !loader_instalable(&instance.loader) {
        return false;
    }
    let Ok(folder) = store.carpeta(&instance.id).and_then(|path| {
        path.canonicalize()
            .map_err(|_| crate::instancias::Error::Disco)
    }) else {
        return false;
    };
    let root = directorio_lectura(&folder);
    let Ok(marker): Result<Marcador, _> = leer_json(&folder, &root.join(MARKER)) else {
        return false;
    };
    let Ok(os) = plataforma() else { return false };
    if marker.loader != instance.loader
        || marker.loader_version != crate::versiones_instancia::leer_loader(store, instance)
        || !matches!(marker.schema, 1 | 2)
        || marker.sistema != format!("{os}-x86_64")
        || marker.instalado.version != instance.minecraft
        || marker.archivos.is_empty()
    {
        return false;
    }
    let Ok(shared) = store.datos() else {
        return false;
    };
    marker.archivos.iter().all(|file| {
        let allowed = if marker.schema == 1 {
            file.ruta.starts_with(&root)
        } else {
            ["assets", "libraries", "meta/minecraft", "meta/fabric", "meta/quilt"]
                .iter()
                .any(|p| file.ruta.starts_with(shared.join(p)))
        };
        allowed
            && segura(
                if marker.schema == 1 { &root } else { &shared },
                &file.ruta,
                false,
            )
            .is_ok()
            && fs::symlink_metadata(&file.ruta).is_ok_and(|meta| {
                meta.is_file() && !enlace(&meta) && meta.len() == file.descarga.size
            })
    }) && marker.instalado.classpath.iter().all(|path| {
        let base = if marker.schema == 1 { &root } else { &shared };
        segura(base, path, false).is_ok()
            && fs::symlink_metadata(path)
                .is_ok_and(|meta| meta.is_file() && !enlace(&meta) && meta.len() > 0)
    }) && segura(&root, &marker.instalado.natives, false).is_ok()
        && marker.instalado.natives.is_dir()
        && marker.nativos.iter().all(|(path, size, _)| {
            segura(&root, path, false).is_ok()
                && fs::metadata(path).is_ok_and(|meta| meta.is_file() && meta.len() == *size)
        })
}

/// Recupera la descripción local del lanzamiento sin red. `instalar` debe
/// volver a verificar los hashes antes de ejecutar el proceso.
pub fn leer(store: &Almacen, instance: &Instancia) -> Option<Instalado> {
    if !listo(store, instance) {
        return None;
    }
    let folder = store.carpeta(&instance.id).ok()?.canonicalize().ok()?;
    let marker: Marcador = leer_json(&folder, &directorio_lectura(&folder).join(MARKER)).ok()?;
    Some(marker.instalado)
}

#[cfg(test)]
mod tests;
