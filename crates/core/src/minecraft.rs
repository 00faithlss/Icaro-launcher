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
mod tests {
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
}
