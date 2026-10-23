// SPDX-License-Identifier: GPL-3.0-only
//! Instalación de MRPACK de Modrinth en una instancia nueva.
use crate::{
    instancias::{Almacen, Instancia},
    modrinth::Version,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio_util::sync::CancellationToken;
const LIMITE: u64 = 256 * 1024 * 1024;
const ESTADO: &str = "modpack.json";
pub mod local;
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Indice {
    format_version: u32,
    game: String,
    name: String,
    version_id: String,
    files: Vec<Archivo>,
    dependencies: BTreeMap<String, String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Archivo {
    path: String,
    hashes: BTreeMap<String, String>,
    downloads: Vec<String>,
    file_size: u64,
    #[serde(default)]
    env: BTreeMap<String, String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Fuente {
    url: String,
    sha512: String,
    size: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub nombre: String,
    pub version: String,
    pub minecraft: String,
    pub loader: String,
    pub loader_version: Option<String>,
    pub opcionales: bool,
    fuente: Fuente,
    indice: Indice,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    local: Option<local::ArchivoLocal>,
}
#[derive(Serialize, Deserialize)]
struct Guardado {
    schema: u32,
    completo: bool,
    plan: Plan,
}
fn cancelado(c: &CancellationToken) -> Result<(), String> {
    if c.is_cancelled() {
        Err("Instalación del modpack cancelada; los archivos verificados se conservan.".into())
    } else {
        Ok(())
    }
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha512::digest(bytes))
}
fn hex(s: &str, n: usize) -> bool {
    s.len() == n && s.bytes().all(|c| c.is_ascii_hexdigit())
}
fn ruta(value: &str) -> Result<PathBuf, String> {
    if value.is_empty()
        || value.len() > 1024
        || value.contains(['\\', ':', '\0'])
        || value.starts_with('/')
        || value.chars().any(char::is_control)
        || value.split('/').any(|p| {
            p.is_empty()
                || p == "."
                || p == ".."
                || p.ends_with(['.', ' '])
                || p.contains(['<', '>', '"', '|', '?', '*'])
                || matches!(
                    p.split('.')
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
        })
    {
        return Err("El modpack contiene una ruta no permitida.".into());
    }
    Ok(PathBuf::from(value))
}
fn url(value: &str, redirect: bool) -> Result<reqwest::Url, String> {
    let u = reqwest::Url::parse(value).map_err(|_| "El modpack contiene una URL inválida.")?;
    let host = matches!(
        u.host_str(),
        Some("cdn.modrinth.com" | "github.com" | "raw.githubusercontent.com" | "gitlab.com")
    ) || redirect
        && matches!(
            u.host_str(),
            Some("release-assets.githubusercontent.com" | "objects.githubusercontent.com")
        );
    if !host
        || u.scheme() != "https"
        || !u.username().is_empty()
        || u.password().is_some()
        || u.port().is_some()
        || u.fragment().is_some()
        || value.chars().any(char::is_whitespace)
    {
        return Err("El modpack usa un origen de descarga no permitido.".into());
    }
    Ok(u)
}
fn cliente() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .https_only(true)
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(180))
        .user_agent(crate::http::USER_AGENT)
        .redirect(reqwest::redirect::Policy::custom(|a| {
            if a.previous().len() > 5 || url(a.url().as_str(), true).is_err() {
                a.stop()
            } else {
                a.follow()
            }
        }))
        .build()
        .map_err(|_| "No se pudo iniciar la descarga del modpack.".into())
}
fn segura(root: &Path, path: &Path, crear: bool) -> Result<(), String> {
    crate::almacenamiento::regular(root)?;
    let rel = path
        .strip_prefix(root)
        .map_err(|_| "Ruta fuera de la instancia.")?;
    let mut current = root.to_owned();
    let components: Vec<_> = rel.components().collect();
    for (n, c) in components.iter().enumerate() {
        if !matches!(c, Component::Normal(_)) {
            return Err("Ruta no permitida.".into());
        }
        current.push(c);
        if current.exists() {
            let meta = crate::almacenamiento::regular(&current)?;
            if n + 1 < components.len() && !meta.is_dir() {
                return Err("Un archivo ocupa una carpeta del modpack.".into());
            }
        } else if fs::symlink_metadata(&current).is_ok() {
            return Err("No se admiten enlaces en el modpack.".into());
        } else if crear && n + 1 < components.len() {
            fs::create_dir(&current).map_err(|_| "No se pudo crear una carpeta del modpack.")?;
            crate::almacenamiento::regular(&current)?;
        }
    }
    Ok(())
}
fn escribir(root: &Path, path: &Path, bytes: &[u8], reemplazar: bool) -> Result<(), String> {
    segura(root, path, true)?;
    if path.exists() && !reemplazar {
        if fs::metadata(path).map_err(|e| e.to_string())?.len() == bytes.len() as u64
            && fs::read(path).map_err(|e| e.to_string())? == bytes
        {
            return Ok(());
        }
        return Err("Un archivo local del modpack cambió; se conserva y no se sobrescribe.".into());
    }
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let temp = path.with_file_name(format!(".icaro-pack-{}-{nonce}.tmp", std::process::id()));
    let result = (|| {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        f.write_all(bytes)
            .and_then(|_| f.sync_all())
            .map_err(|e| e.to_string())?;
        drop(f);
        segura(root, path, false)?;
        if reemplazar {
            fs::rename(&temp, path).map_err(|e| e.to_string())?;
        } else {
            fs::hard_link(&temp, path)
                .map_err(|_| "El destino del modpack ya existe; sus archivos se conservan.")?;
        }
        Ok(())
    })();
    let _ = fs::remove_file(temp);
    result
}
async fn descargar(
    client: &reqwest::Client,
    source: &Fuente,
    cache: &Path,
    c: &CancellationToken,
    anunciar: impl Fn(),
) -> Result<Vec<u8>, String> {
    cancelado(c)?;
    url(&source.url, false)?;
    if source.size > LIMITE || !hex(&source.sha512, 128) {
        return Err("Los datos de integridad del modpack son inválidos.".into());
    }
    let target = cache.join("cache/modpacks/archivos").join(&source.sha512);
    segura(cache, &target, false)?;
    if target.is_file() && fs::metadata(&target).map_err(|e| e.to_string())?.len() == source.size {
        let bytes = fs::read(&target).map_err(|e| e.to_string())?;
        if hash(&bytes) == source.sha512.to_lowercase() {
            return Ok(bytes);
        }
    }
    anunciar();
    let mut response = tokio::select! {_=c.cancelled()=>return cancelado(c).map(|_|vec![]),r=client.get(url(&source.url,false)?).send()=>r.map_err(|_|"No se pudo descargar un archivo del modpack.")?};
    if !response.status().is_success() {
        return Err(format!(
            "La descarga del modpack respondió HTTP {}.",
            response.status().as_u16()
        ));
    }
    let mut bytes = Vec::new();
    loop {
        let chunk = tokio::select! {_=c.cancelled()=>return cancelado(c).map(|_|vec![]),r=response.chunk()=>r.map_err(|_|"La descarga del modpack se interrumpió.")?};
        let Some(chunk) = chunk else {
            break;
        };
        if bytes.len() as u64 + chunk.len() as u64 > source.size {
            return Err("Un archivo del modpack supera su tamaño declarado.".into());
        }
        bytes.extend(chunk);
    }
    cancelado(c)?;
    if bytes.len() as u64 != source.size || hash(&bytes) != source.sha512.to_lowercase() {
        return Err("Un archivo del modpack no pasó la verificación SHA-512.".into());
    }
    escribir(cache, &target, &bytes, true)?;
    Ok(bytes)
}
fn zip(bytes: &[u8]) -> Result<(Indice, BTreeMap<String, Vec<u8>>), String> {
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes))
        .map_err(|_| "El MRPACK no es un ZIP válido.")?;
    if archive.len() > 10_000 {
        return Err("El MRPACK contiene demasiadas entradas.".into());
    }
    let mut names = BTreeSet::new();
    let mut layers = [BTreeMap::new(), BTreeMap::new()];
    let mut index = None;
    let mut total = 0u64;
    for n in 0..archive.len() {
        let mut file = archive
            .by_index(n)
            .map_err(|_| "No se pudo leer el MRPACK.")?;
        let name = file
            .name()
            .map_err(|_| "El MRPACK contiene un nombre inválido.")?
            .trim_end_matches('/')
            .to_owned();
        ruta(&name)?;
        if !names.insert(name.to_ascii_lowercase())
            || file
                .unix_mode()
                .is_some_and(|m| !matches!(m & 0o170000, 0 | 0o100000 | 0o040000))
        {
            return Err("El MRPACK contiene enlaces o entradas duplicadas.".into());
        }
        total = total
            .checked_add(file.size())
            .ok_or("El MRPACK supera el tamaño permitido.")?;
        if total > 512 * 1024 * 1024 || file.size() > LIMITE {
            return Err("El MRPACK supera el límite de extracción.".into());
        }
        if file.is_dir() {
            continue;
        }
        if name == "modrinth.index.json" {
            if file.size() > 8 * 1024 * 1024 {
                return Err("El índice MRPACK es demasiado grande.".into());
            }
            let mut b = vec![];
            let expected = file.size();
            Read::by_ref(&mut file)
                .take(expected + 1)
                .read_to_end(&mut b)
                .map_err(|e| e.to_string())?;
            if b.len() as u64 != expected {
                return Err("Una entrada del MRPACK tiene un tamaño inválido.".into());
            }
            index = Some(serde_json::from_slice(&b).map_err(|_| "El índice MRPACK es inválido.")?);
        } else {
            for (layer, prefix) in ["overrides/", "client-overrides/"].iter().enumerate() {
                if let Some(path) = name.strip_prefix(prefix) {
                    ruta(path)?;
                    let mut b = vec![];
                    let expected = file.size();
                    Read::by_ref(&mut file)
                        .take(expected + 1)
                        .read_to_end(&mut b)
                        .map_err(|e| e.to_string())?;
                    if b.len() as u64 != expected {
                        return Err("Una entrada del MRPACK tiene un tamaño inválido.".into());
                    }
                    layers[layer].insert(path.to_owned(), b);
                    break;
                }
            }
        }
    }
    let index: Indice = index.ok_or("El MRPACK no incluye modrinth.index.json.")?;
    if index.format_version != 1
        || index.game != "minecraft"
        || index.files.len() > 10_000
        || index.name.trim().is_empty()
        || index.name.len() > 256
        || index.version_id.len() > 128
    {
        return Err("El formato de este modpack no está admitido.".into());
    }
    let mut paths = BTreeSet::new();
    let mut downloads = 0u64;
    for file in &index.files {
        ruta(&file.path)?;
        if !paths.insert(file.path.to_ascii_lowercase())
            || !file.hashes.get("sha512").is_some_and(|s| hex(s, 128))
            || !file.hashes.get("sha1").is_some_and(|s| hex(s, 40))
            || file.file_size > LIMITE
            || file.downloads.is_empty()
            || file.downloads.len() > 8
            || file
                .env
                .values()
                .any(|v| !matches!(v.as_str(), "required" | "optional" | "unsupported"))
        {
            return Err("El modpack contiene archivos o hashes inválidos.".into());
        }
        for u in &file.downloads {
            url(u, false)?;
        }
        downloads = downloads
            .checked_add(file.file_size)
            .ok_or("Modpack demasiado grande.")?;
    }
    if downloads > 8 * 1024 * 1024 * 1024 {
        return Err("El modpack supera el límite de descarga.".into());
    }
    let mut overrides = std::mem::take(&mut layers[0]);
    overrides.append(&mut layers[1]);
    let mut paths: BTreeMap<String, String> = BTreeMap::new();
    for path in index.files.iter().map(|f| &f.path).chain(overrides.keys()) {
        if paths
            .insert(path.to_ascii_lowercase(), path.clone())
            .is_some_and(|prev| prev != *path)
        {
            return Err("El modpack contiene rutas que solo difieren en mayúsculas.".into());
        }
    }
    for path in paths.keys() {
        for (n, b) in path.bytes().enumerate() {
            if b == b'/' && paths.contains_key(&path[..n]) {
                return Err("Un archivo del modpack ocupa una carpeta.".into());
            }
        }
    }
    Ok((index, overrides))
}
impl Plan {
    pub fn formato(&self) -> &str {
        self.local.as_ref().map_or("Modrinth", |l| l.formato())
    }
    pub fn bytes_archivo(&self) -> u64 {
        self.fuente.size
    }
    pub fn archivos_por_descargar(&self) -> usize {
        self.indice
            .files
            .iter()
            .filter(|f| {
                f.env
                    .get("client")
                    .is_none_or(|v| v != "unsupported" && (self.opcionales || v != "optional"))
                    && !self.local.as_ref().is_some_and(|l| l.contiene(&f.path))
            })
            .count()
    }
    /// Tamaños y carpetas reales de la selección de cliente, sin contar dos veces overrides.
    pub fn resumen(&self) -> (u64, u64, Vec<String>, usize) {
        let (incluidos, carpetas, mut mods) = self
            .local
            .as_ref()
            .map(|l| l.resumen(self))
            .unwrap_or_default();
        let mut descarga = 0;
        for f in &self.indice.files {
            if f.env
                .get("client")
                .is_some_and(|v| v == "unsupported" || (!self.opcionales && v == "optional"))
            {
                continue;
            }
            if !self.local.as_ref().is_some_and(|l| l.contiene(&f.path)) {
                descarga += f.file_size;
                if f.path.starts_with("mods/") && f.path.ends_with(".jar") {
                    mods.push(f.path.clone());
                }
            }
        }
        (descarga, incluidos, carpetas, mods.len())
    }
    pub fn archivos_incluidos(&self) -> usize {
        self.local.as_ref().map_or(0, |l| l.cantidad(self))
    }
    pub fn archivos_cliente(&self) -> usize {
        self.indice
            .files
            .iter()
            .filter(|f| {
                f.env
                    .get("client")
                    .is_none_or(|v| v != "unsupported" && (self.opcionales || v != "optional"))
            })
            .count()
    }
    pub fn archivos_opcionales(&self) -> usize {
        self.indice
            .files
            .iter()
            .filter(|f| f.env.get("client").is_some_and(|v| v == "optional"))
            .count()
    }
}
pub async fn preparar(
    root: &Path,
    version: &Version,
    c: CancellationToken,
) -> Result<Plan, String> {
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let artifact = version
        .files
        .iter()
        .find(|f| f.primary && f.filename.ends_with(".mrpack"))
        .or_else(|| {
            version
                .files
                .iter()
                .find(|f| f.filename.ends_with(".mrpack"))
        })
        .ok_or("Esta versión no contiene un archivo MRPACK.")?;
    let fuente = Fuente {
        url: artifact.url.clone(),
        sha512: artifact.hashes.sha512.to_lowercase(),
        size: artifact.size,
    };
    let bytes = descargar(&cliente()?, &fuente, root, &c, || {}).await?;
    cancelado(&c)?;
    let (index, _) = zip(&bytes)?;
    let minecraft = index
        .dependencies
        .get("minecraft")
        .filter(|v| crate::minecraft::version_minecraft_valida(v))
        .cloned()
        .ok_or("El modpack no declara una versión válida de Minecraft.")?;
    let mut loader = "vanilla".to_owned();
    let mut loader_version = None;
    for (key, version) in &index.dependencies {
        if key == "minecraft" {
            continue;
        }
        let family = match key.as_str() {
            "fabric-loader" => "fabric",
            "quilt-loader" => "quilt",
            "forge" => "forge",
            "neoforge" => "neoforge",
            _ => return Err(format!("Dependencia de modpack no admitida: {key}.")),
        };
        if loader_version.is_some() || !crate::minecraft::version_minecraft_valida(version) {
            return Err("El modpack declara loaders incompatibles o versiones inválidas.".into());
        }
        loader = family.into();
        loader_version = Some(version.clone());
    }
    if !matches!(loader.as_str(), "vanilla" | "fabric" | "forge" | "neoforge") {
        return Err(format!(
            "Este modpack necesita {loader}; su instalador todavía está pendiente en Ícaro."
        ));
    }
    if !version.game_versions.contains(&minecraft)
        || (!version.loaders.contains(&loader) && loader != "vanilla")
    {
        return Err("El MRPACK no coincide con la versión publicada en Modrinth.".into());
    }
    Ok(Plan {
        nombre: index.name.clone(),
        version: index.version_id.clone(),
        minecraft,
        loader,
        loader_version,
        opcionales: true,
        fuente,
        indice: index,
        local: None,
    })
}
pub fn crear(store: &Almacen, plan: &Plan) -> Result<Instancia, String> {
    let base: String = plan
        .nombre
        .chars()
        .filter(|c| !c.is_control() && !"<>:\"/\\|?*".contains(*c))
        .take(60)
        .collect();
    let base = base.trim().trim_end_matches('.');
    let base = if base.is_empty() { "Modpack" } else { base };
    let existing = store.cargar().map_err(|e| e.to_string())?.instancias;
    let mut nombre = base.to_owned();
    for n in 2..1000 {
        if !existing
            .iter()
            .any(|i| i.nombre.eq_ignore_ascii_case(&nombre))
        {
            break;
        }
        nombre = format!("{base} ({n})");
    }
    let instance = store
        .crear_con_loader(&nombre, &plan.minecraft, &plan.loader, 0)
        .map_err(|e| e.to_string())?;
    let result = (|| {
        if let Some(version) = &plan.loader_version {
            crate::versiones_instancia::guardar_loader(store, &instance, version)?;
        }
        let root = store.carpeta(&instance.id).map_err(|e| e.to_string())?;
        escribir(
            &root,
            &root.join(".icaro").join(ESTADO),
            &serde_json::to_vec(&Guardado {
                schema: 1,
                completo: false,
                plan: plan.clone(),
            })
            .map_err(|e| e.to_string())?,
            false,
        )
    })();
    if let Err(error) = result {
        let _ = store.eliminar(&instance.id);
        return Err(error);
    }
    Ok(instance)
}
/// La instancia no está lista mientras su MRPACK conserve una instalación pendiente.
pub fn completo(store: &Almacen, instance: &Instancia) -> bool {
    let Ok(folder) = store.carpeta(&instance.id) else {
        return false;
    };
    let path = folder.join(".icaro").join(ESTADO);
    if segura(&folder, &path, false).is_err() {
        return false;
    }
    if !path.exists() {
        return true;
    }
    if !fs::metadata(&path).is_ok_and(|m| m.len() <= 16 * 1024 * 1024) {
        return false;
    }
    fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice::<Guardado>(&b).ok())
        .is_some_and(|s| s.schema == 1 && s.completo)
}

pub async fn instalar(
    store: &Almacen,
    instance: &Instancia,
    c: CancellationToken,
    progress: impl Fn(crate::minecraft::Progreso) + Send + Sync,
) -> Result<(), String> {
    let folder = store.carpeta(&instance.id).map_err(|e| e.to_string())?;
    let path = folder.join(".icaro").join(ESTADO);
    segura(&folder, &path, false)?;
    if !path.exists() {
        return Ok(());
    }
    if fs::metadata(&path).map_err(|e| e.to_string())?.len() > 16 * 1024 * 1024 {
        return Err("Los metadatos del modpack superan el límite.".into());
    }
    let mut saved: Guardado = serde_json::from_slice(&fs::read(&path).map_err(|e| e.to_string())?)
        .map_err(|_| "Los metadatos del modpack son inválidos.")?;
    if saved.schema != 1
        || saved.plan.minecraft != instance.minecraft
        || saved.plan.loader != instance.loader
        || saved.plan.loader_version != crate::versiones_instancia::leer_loader(store, instance)
    {
        if saved.completo {
            return Ok(());
        }
        return Err("La versión de la instancia cambió durante la instalación del modpack.".into());
    }
    if saved.completo {
        return Ok(());
    }
    let root = store.datos().map_err(|e| e.to_string())?;
    let client = cliente()?;
    let report = |etapa: &str, done, total| {
        progress(crate::minecraft::Progreso {
            etapa: etapa.into(),
            completados: done,
            total,
        })
    };
    let (index, mut overrides) = if let Some(local) = saved
        .plan
        .local
        .as_ref()
        .map(|l| l.para_cliente(&saved.plan))
    {
        let store = store.clone();
        let instance = instance.clone();
        let token = c.clone();
        // Extracción y SHA-512 fuera del ejecutor; memoria acotada para paquetes grandes.
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let worker = tokio::task::spawn_blocking(move || {
            local::extraer(&store, &instance, &local, &token, |done, total| {
                let _ = tx.send((done, total));
            })
        });
        tokio::pin!(worker);
        loop {
            tokio::select! {
                result = &mut worker => {
                    result.map_err(|_| "No se pudo extraer el modpack.".to_owned())??;
                    break;
                },
                Some((done, total)) = rx.recv() => report("Extrayendo archivos del modpack", done, total),
            }
        }
        (saved.plan.indice.clone(), BTreeMap::new())
    } else {
        let bytes = descargar(&client, &saved.plan.fuente, &root, &c, || {
            report("Descargando modpack", 0, 0)
        })
        .await?;
        zip(&bytes)?
    };
    if serde_json::to_value(&index).map_err(|e| e.to_string())?
        != serde_json::to_value(&saved.plan.indice).map_err(|e| e.to_string())?
    {
        return Err("El índice del modpack cambió; sus archivos se conservan.".into());
    }
    overrides.retain(|path, _| {
        index
            .files
            .iter()
            .find(|f| &f.path == path)
            .is_none_or(|f| {
                f.env.get("client").is_none_or(|v| {
                    v != "unsupported" && (saved.plan.opcionales || v != "optional")
                })
            })
    });
    let files: Vec<_> = index
        .files
        .iter()
        .filter(|f| {
            !overrides.contains_key(&f.path)
                && !saved
                    .plan
                    .local
                    .as_ref()
                    .is_some_and(|l| l.contiene(&f.path))
                && f.env.get("client").is_none_or(|v| {
                    v != "unsupported" && (saved.plan.opcionales || v != "optional")
                })
        })
        .collect();
    let total = (files.len() + overrides.len()) as u64;
    let mut done = 0;
    let game = store.juego(&instance.id).map_err(|e| e.to_string())?;
    for file in files {
        cancelado(&c)?;
        report("Verificando archivos del modpack", done, total);
        let mut result = Err("No se pudo descargar un archivo del modpack.".into());
        for u in &file.downloads {
            let source = Fuente {
                url: u.clone(),
                sha512: file.hashes["sha512"].to_lowercase(),
                size: file.file_size,
            };
            result = descargar(&client, &source, &root, &c, || {
                report("Descargando archivos del modpack", done, total)
            })
            .await;
            if result.is_ok() {
                break;
            }
            cancelado(&c)?;
        }
        let bytes = result?;
        if format!("{:x}", sha1::Sha1::digest(&bytes)) != file.hashes["sha1"].to_lowercase() {
            return Err("Un archivo del modpack no pasó la verificación SHA-1.".into());
        }
        cancelado(&c)?;
        escribir(&game, &game.join(ruta(&file.path)?), &bytes, false)?;
        done += 1;
    }
    for (name, bytes) in overrides {
        cancelado(&c)?;
        report("Aplicando archivos incluidos en el modpack", done, total);
        cancelado(&c)?;
        escribir(&game, &game.join(ruta(&name)?), &bytes, false)?;
        done += 1;
    }
    cancelado(&c)?;
    saved.completo = true;
    escribir(
        &folder,
        &path,
        &serde_json::to_vec(&saved).map_err(|e| e.to_string())?,
        true,
    )?;
    report("Modpack preparado", done, total);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    struct Temporal(PathBuf);
    impl Temporal {
        fn nuevo() -> Self {
            let path = std::env::temp_dir().join(format!(
                "icaro-mrpack-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Temporal {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn fixture(extra: Vec<(&str, &[u8])>) -> (Vec<u8>, Version) {
        let jar = b"jar verificado";
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        let opt = zip::write::SimpleFileOptions::default();
        let index = json!({"formatVersion":1,"game":"minecraft","name":"Pack de prueba","versionId":"1.0","dependencies":{"minecraft":"26.3","fabric-loader":"0.19.5"},"files":[
            {"path":"mods/test.jar","hashes":{"sha1":format!("{:x}",sha1::Sha1::digest(jar)),"sha512":hash(jar)},"fileSize":jar.len(),"downloads":["https://cdn.modrinth.com/data/fixture/mod.jar"]},
            {"path":"mods/server.jar","hashes":{"sha1":"b".repeat(40),"sha512":"b".repeat(128)},"fileSize":1,"downloads":["https://cdn.modrinth.com/data/fixture/server.jar"],"env":{"client":"unsupported","server":"required"}},
            {"path":"mods/optional.jar","hashes":{"sha1":"c".repeat(40),"sha512":"c".repeat(128)},"fileSize":1,"downloads":["https://cdn.modrinth.com/data/fixture/optional.jar"],"env":{"client":"optional","server":"unsupported"}}
        ]});
        writer.start_file("modrinth.index.json", opt).unwrap();
        writer
            .write_all(&serde_json::to_vec(&index).unwrap())
            .unwrap();
        for (path, bytes) in [
            ("overrides/config/test.json", b"comun".as_slice()),
            ("client-overrides/config/test.json", b"cliente".as_slice()),
            (
                "server-overrides/config/server.json",
                b"servidor".as_slice(),
            ),
        ]
        .into_iter()
        .chain(extra)
        {
            writer.start_file(path, opt).unwrap();
            writer.write_all(bytes).unwrap();
        }
        let bytes = writer.finish().unwrap().into_inner();
        let version:Version=serde_json::from_value(json!({"id":"fixture","project_id":"pack","name":"1.0","version_number":"1.0","version_type":"release","game_versions":["26.3"],"loaders":["fabric"],"files":[{"filename":"pack.mrpack","url":"https://cdn.modrinth.com/data/fixture/pack.mrpack","size":bytes.len(),"primary":true,"hashes":{"sha512":hash(&bytes)}}],"dependencies":[],"changelog":null})).unwrap();
        (bytes, version)
    }
    fn cache(root: &Path, bytes: &[u8]) {
        let path = root.join("cache/modpacks/archivos").join(hash(bytes));
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
    #[tokio::test]
    async fn mrpack_cancelacion_reintento_lados_overrides_y_configuracion_preservada() {
        let temp = Temporal::nuevo();
        let (bytes, version) = fixture(vec![]);
        cache(&temp.0, &bytes);
        cache(&temp.0, b"jar verificado");
        let mut plan = preparar(&temp.0, &version, CancellationToken::new())
            .await
            .unwrap();
        plan.opcionales = false;
        assert_eq!(plan.archivos_cliente(), 1);
        assert_eq!(plan.archivos_opcionales(), 1);
        let store = Almacen::nuevo(temp.0.join("instances"));
        let original = store.crear("Original", "1.21.1", 0).unwrap();
        fs::write(
            store.juego(&original.id).unwrap().join("options.txt"),
            b"usuario",
        )
        .unwrap();
        let instance = crear(&store, &plan).unwrap();
        let game = store.juego(&instance.id).unwrap();
        assert!(!completo(&store, &instance));
        let cancel = CancellationToken::new();
        let observer = cancel.clone();
        assert!(instalar(&store, &instance, cancel, |p| {
            assert!(!p.etapa.starts_with("Descargando"));
            if p.etapa.starts_with("Aplicando") {
                observer.cancel();
            }
        })
        .await
        .is_err());
        assert_eq!(
            fs::read(game.join("mods/test.jar")).unwrap(),
            b"jar verificado"
        );
        assert!(!game.join("config/test.json").exists());
        instalar(&store, &instance, CancellationToken::new(), |p| {
            assert!(!p.etapa.starts_with("Descargando"))
        })
        .await
        .unwrap();
        assert_eq!(fs::read(game.join("config/test.json")).unwrap(), b"cliente");
        assert!(completo(&store, &instance));
        assert!(
            !game.join("mods/server.jar").exists()
                && !game.join("mods/optional.jar").exists()
                && !game.join("config/server.json").exists()
        );
        assert_eq!(
            crate::versiones_instancia::leer_loader(&store, &instance).as_deref(),
            Some("0.19.5")
        );
        fs::write(game.join("config/test.json"), b"personalizado").unwrap();
        instalar(&store, &instance, CancellationToken::new(), |_| {
            panic!("El pack completo no se reinstala")
        })
        .await
        .unwrap();
        assert_eq!(
            fs::read(game.join("config/test.json")).unwrap(),
            b"personalizado"
        );
        assert_eq!(
            fs::read(store.juego(&original.id).unwrap().join("options.txt")).unwrap(),
            b"usuario"
        );
        let copy = crear(&store, &plan).unwrap();
        assert_ne!(copy.id, instance.id);
        assert_ne!(copy.nombre, instance.nombre);
    }
    #[tokio::test]
    async fn archivo_local_modificado_no_se_sobrescribe() {
        let temp = Temporal::nuevo();
        let (bytes, version) = fixture(vec![]);
        cache(&temp.0, &bytes);
        cache(&temp.0, b"jar verificado");
        let mut plan = preparar(&temp.0, &version, CancellationToken::new())
            .await
            .unwrap();
        plan.opcionales = false;
        let store = Almacen::nuevo(temp.0.join("instances"));
        let instance = crear(&store, &plan).unwrap();
        let game = store.juego(&instance.id).unwrap();
        fs::create_dir_all(game.join("mods")).unwrap();
        fs::write(game.join("mods/test.jar"), b"personal").unwrap();
        assert!(
            instalar(&store, &instance, CancellationToken::new(), |_| {})
                .await
                .is_err()
        );
        assert_eq!(fs::read(game.join("mods/test.jar")).unwrap(), b"personal");
    }
    #[test]
    fn mrpack_rechaza_escape_colisiones_y_origenes_no_permitidos() {
        for path in [
            "../secreto",
            "overrides/../../secreto",
            "overrides/C:/secreto",
            "client-overrides/config/NUL.txt",
        ] {
            let (bytes, _) = fixture(vec![(path, b"fuera")]);
            assert!(zip(&bytes).is_err(), "{path}");
        }
        let (bytes, _) = fixture(vec![("overrides/config/TEST.json", b"colision")]);
        assert!(zip(&bytes).is_err());
        for source in [
            "http://cdn.modrinth.com/a",
            "https://localhost/a",
            "https://github.com:8443/a",
            "https://github.com@127.0.0.1/a",
        ] {
            assert!(url(source, false).is_err());
        }
        assert!(url(
            "https://github.com/org/repo/releases/download/v1/mod.jar",
            false
        )
        .is_ok());
    }
    #[tokio::test]
    #[ignore = "Descarga un MRPACK oficial y Minecraft/Fabric en una carpeta temporal; no usa cuentas ni inicia el juego"]
    async fn modpack_oficial_crea_instancia_instala_y_reutiliza() {
        let temp = Temporal::nuevo();
        let catalog = crate::modrinth::Catalog::new().unwrap();
        let mut versions = catalog
            .versions("fabulously-optimized", None, None)
            .await
            .unwrap();
        versions.retain(|v| v.version_type == "release");
        versions.sort_by(|a, b| b.date_published.cmp(&a.date_published));
        let selected = versions.first().unwrap();
        let mut plan = preparar(&temp.0, selected, CancellationToken::new())
            .await
            .unwrap();
        plan.opcionales = false;
        println!(
            "Modpack: {} {} / Minecraft {} / {} {:?} / {} archivos",
            plan.nombre,
            plan.version,
            plan.minecraft,
            plan.loader,
            plan.loader_version,
            plan.archivos_cliente()
        );
        let store = Almacen::nuevo(temp.0.join("instances"));
        let instance = crear(&store, &plan).unwrap();
        instalar(&store, &instance, CancellationToken::new(), |p| {
            if p.completados % 10 == 0 {
                println!("{} {}/{}", p.etapa, p.completados, p.total)
            }
        })
        .await
        .unwrap();
        let game = store.juego(&instance.id).unwrap();
        assert!(game.join("mods").is_dir());
        assert!(fs::read_dir(game.join("mods")).unwrap().count() > 5);
        let ready = crate::minecraft::instalar(&store, &instance, CancellationToken::new(), |_| {})
            .await
            .unwrap();
        assert!(crate::minecraft::listo(&store, &instance));
        assert!(ready.main_class.contains("KnotClient"));
        instalar(&store, &instance, CancellationToken::new(), |_| {
            panic!("El modpack ya está completo")
        })
        .await
        .unwrap();
        crate::minecraft::instalar(&store, &instance, CancellationToken::new(), |p| {
            assert!(!p.etapa.starts_with("Descargando"))
        })
        .await
        .unwrap();
    }
}
