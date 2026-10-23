// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Mods de Modrinth: dependencias, hashes y sustitución del conjunto preparado.
use crate::{
    instancias::Almacen,
    modrinth::{Catalog, Dependency, File as ModFile, Version},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

const META: &str = ".icaro-mods.json";
pub mod fabric_api;
pub mod locales;
pub mod sincronizacion;
const MAX_FILE: u64 = 128 * 1024 * 1024;

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Papelera(String),
    #[error(transparent)]
    Red(#[from] crate::modrinth::Error),
    #[error("No se pudieron leer o guardar los mods. Los archivos actuales se conservan.")]
    Disco,
    #[error("Los archivos o metadatos de mods no son válidos.")]
    Datos,
    #[error("Otra operación de mods está en curso. Inténtalo de nuevo al terminar.")]
    Ocupada,
    #[error("No hay una versión compatible de {0} para esta instancia.")]
    Compatible(String),
    #[error("Hay requisitos incompatibles entre los mods: {0}.")]
    Conflicto(String),
    #[error("El archivo descargado no coincide con su tamaño o SHA-512.")]
    Hash,
    #[error("Hay un archivo modificado o un nombre ocupado: {0}. Se conservó el archivo actual.")]
    Archivo(String),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instalado {
    pub project_id: String,
    pub nombre: String,
    pub version_id: String,
    pub version: String,
    #[serde(default)]
    pub publicada: String,
    pub archivo: String,
    pub sha512: String,
    pub activo: bool,
    pub dependencias: Vec<Dependency>,
}
fn nombre_valido(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 240
        && !s.starts_with('.')
        && !s.ends_with('.')
        && !s.ends_with(' ')
        && !s
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
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
fn enlace(m: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        m.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        m.file_type().is_symlink()
    }
}
fn regular(path: &Path, directory: bool) -> Result<fs::Metadata, Error> {
    let m = fs::symlink_metadata(path).map_err(|_| Error::Disco)?;
    if enlace(&m) || (directory && !m.is_dir()) || (!directory && !m.is_file()) {
        return Err(Error::Datos);
    }
    Ok(m)
}
fn hash(bytes: &[u8]) -> String {
    Sha512::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn ruta_mod(dir: &Path, m: &Instalado) -> PathBuf {
    dir.join(if m.activo {
        m.archivo.clone()
    } else {
        format!("{}.disabled", m.archivo)
    })
}
pub(crate) fn guardia(game: &Path) -> Result<fs::File, Error> {
    regular(game, true)?;
    let path = game.join(".icaro-mods.lock");
    if path.exists() {
        regular(&path, false)?;
    }
    let file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|_| Error::Disco)?;
    file.try_lock().map_err(|_| Error::Ocupada)?;
    Ok(file)
}
fn recuperar(game: &Path) -> Result<(), Error> {
    let mods = game.join("mods");
    let backup = game.join(".icaro-mods-backup");
    if !mods.exists() && backup.exists() {
        regular(&backup, true)?;
        fs::rename(backup, mods).map_err(|_| Error::Disco)?;
    }
    sincronizacion::recuperar_transacciones(game)
}
fn registros(game: &Path) -> Result<Vec<Instalado>, Error> {
    let dir = game.join("mods");
    if !dir.exists() {
        return Ok(vec![]);
    }
    regular(&dir, true)?;
    let path = dir.join(META);
    if !path.exists() {
        return Ok(vec![]);
    }
    if regular(&path, false)?.len() > 2 * 1024 * 1024 {
        return Err(Error::Datos);
    }
    let data: Vec<Instalado> = serde_json::from_slice(&fs::read(path).map_err(|_| Error::Disco)?)
        .map_err(|_| Error::Datos)?;
    let mut projects = BTreeSet::new();
    let mut names = BTreeSet::new();
    if data.len() > 512
        || data.iter().any(|m| {
            !nombre_valido(&m.archivo)
                || !m.archivo.ends_with(".jar")
                || !projects.insert(m.project_id.clone())
                || !names.insert(m.archivo.to_lowercase())
        })
    {
        return Err(Error::Datos);
    }
    Ok(data)
}
pub fn inventario(store: &Almacen, id: &str) -> Result<Vec<Instalado>, Error> {
    let game = store.juego(id).map_err(|_| Error::Disco)?;
    let _lock = guardia(&game)?;
    recuperar(&game)?;
    let dir = game.join("mods");
    let mut mods = registros(&game)?;
    let mut known = BTreeSet::new();
    for m in &mut mods {
        let active = dir.join(&m.archivo);
        let disabled = dir.join(format!("{}.disabled", m.archivo));
        if active.exists() {
            regular(&active, false)?;
            m.activo = true;
        } else if disabled.exists() {
            regular(&disabled, false)?;
            m.activo = false;
        } else {
            return Err(Error::Archivo(m.archivo.clone()));
        }
        known.insert(m.archivo.clone());
    }
    if dir.exists() {
        for e in fs::read_dir(dir).map_err(|_| Error::Disco)? {
            let e = e.map_err(|_| Error::Disco)?;
            let name = e.file_name().to_string_lossy().into_owned();
            let base = name.strip_suffix(".disabled").unwrap_or(&name);
            if base.ends_with(".jar") && !known.contains(base) {
                regular(&e.path(), false)?;
                if !nombre_valido(base) {
                    return Err(Error::Datos);
                }
                mods.push(Instalado {
                    project_id: String::new(),
                    nombre: base.into(),
                    version_id: String::new(),
                    version: "Archivo local".into(),
                    publicada: String::new(),
                    archivo: base.into(),
                    sha512: String::new(),
                    activo: name == base,
                    dependencias: vec![],
                });
            }
        }
    }
    Ok(mods)
}
pub trait Fuente: Sync {
    fn actualizaciones_por_hash(&self, _hashes: &[String], _mc: &str, _loader: &str)
        -> impl std::future::Future<Output = Result<Option<BTreeMap<String, Version>>, Error>> + Send {
        async { Ok(None) }
    }
    fn versiones_exactas(&self, ids: &[String]) -> impl std::future::Future<Output = Result<BTreeMap<String, Version>, Error>> + Send {
        async move {
            let mut versions = BTreeMap::new();
            for id in ids { versions.insert(id.clone(), self.version(id).await?); }
            Ok(versions)
        }
    }
    fn versiones_por_hash(&self, _hashes: &[String]) -> impl std::future::Future<Output = Result<BTreeMap<String, Version>, Error>> + Send {
        async { Ok(BTreeMap::new()) }
    }
    fn solo_cliente(&self, _project: &str) -> impl std::future::Future<Output = Result<bool, Error>> + Send {
        async { Ok(false) }
    }
    /// Proyectos exclusivos de cliente entre `projects`. Las fuentes con
    /// consultas por lote deben sobrescribirlo para no pedir uno a uno.
    fn solo_cliente_lote(&self, projects: &[String]) -> impl std::future::Future<Output = Result<BTreeSet<String>, Error>> + Send {
        async move {
            let mut result = BTreeSet::new();
            for p in projects {
                if self.solo_cliente(p).await? {
                    result.insert(p.clone());
                }
            }
            Ok(result)
        }
    }
    /// Nombre visible de cada proyecto, consultado por lotes cuando la fuente lo permite.
    fn nombres(&self, projects: &[String]) -> impl std::future::Future<Output = Result<BTreeMap<String, String>, Error>> + Send {
        async move {
            let mut result = BTreeMap::new();
            for p in projects {
                result.insert(p.clone(), self.nombre(p).await?);
            }
            Ok(result)
        }
    }
    fn version(&self, id: &str)
        -> impl std::future::Future<Output = Result<Version, Error>> + Send;
    fn versiones(
        &self,
        project: &str,
        mc: &str,
        loader: &str,
    ) -> impl std::future::Future<Output = Result<Vec<Version>, Error>> + Send;
    fn nombre(
        &self,
        project: &str,
    ) -> impl std::future::Future<Output = Result<String, Error>> + Send;
    fn archivo(
        &self,
        file: &ModFile,
    ) -> impl std::future::Future<Output = Result<Vec<u8>, Error>> + Send;
    /// Descarga y verifica tamaño y SHA-512 directamente en `destino`. Por
    /// defecto usa [`Fuente::archivo`]; las fuentes de red lo hacen por bloques.
    fn descargar_a(
        &self,
        file: &ModFile,
        destino: &Path,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send {
        async move {
            let bytes = self.archivo(file).await?;
            if bytes.len() as u64 != file.size || hash(&bytes) != file.hashes.sha512.to_lowercase() {
                return Err(Error::Hash);
            }
            guardar_archivo(destino, &bytes)
        }
    }
}
impl Fuente for Catalog {
    async fn actualizaciones_por_hash(&self, hashes:&[String], mc:&str, loader:&str) -> Result<Option<BTreeMap<String, Version>>, Error> {
        let mut result=BTreeMap::new();
        for batch in hashes.chunks(512) {result.extend(self.latest_from_hashes(batch,mc,loader).await?);}
        Ok(Some(result))
    }
    async fn versiones_exactas(&self, ids: &[String]) -> Result<BTreeMap<String, Version>, Error> {
        Ok(self.versions_by_ids(ids).await?)
    }
    async fn versiones_por_hash(&self, hashes: &[String]) -> Result<BTreeMap<String, Version>, Error> {
        Ok(self.versions_from_hashes(hashes).await?)
    }
    async fn solo_cliente(&self, project: &str) -> Result<bool, Error> {
        let p = self.project(project).await?;
        Ok(p.server_side == "unsupported" && p.client_side != "unsupported")
    }
    async fn solo_cliente_lote(&self, projects: &[String]) -> Result<BTreeSet<String>, Error> {
        let mut result = BTreeSet::new();
        for lote in projects.chunks(512) {
            let fichas = self.projects_by_ids(lote).await?;
            for id in lote {
                let p = fichas.get(id).ok_or(Error::Datos)?;
                if p.server_side == "unsupported" && p.client_side != "unsupported" {
                    result.insert(id.clone());
                }
            }
        }
        Ok(result)
    }
    async fn nombres(&self, projects: &[String]) -> Result<BTreeMap<String, String>, Error> {
        let mut result = BTreeMap::new();
        for lote in projects.chunks(512) {
            let fichas = self.projects_by_ids(lote).await?;
            for id in lote {
                result.insert(id.clone(), fichas.get(id).ok_or(Error::Datos)?.title.clone());
            }
        }
        Ok(result)
    }
    async fn version(&self, id: &str) -> Result<Version, Error> {
        Ok(self.version(id).await?)
    }
    async fn versiones(&self, p: &str, mc: &str, l: &str) -> Result<Vec<Version>, Error> {
        Ok(self.versions(p, Some(mc), Some(l)).await?)
    }
    async fn nombre(&self, p: &str) -> Result<String, Error> {
        Ok(self.project(p).await?.title)
    }
    async fn archivo(&self, f: &ModFile) -> Result<Vec<u8>, Error> {
        Ok(self.download_mod(f).await?)
    }
    async fn descargar_a(&self, f: &ModFile, destino: &Path) -> Result<(), Error> {
        if self.download_mod_to(f, destino).await? != f.hashes.sha512.to_lowercase() {
            let _ = fs::remove_file(destino);
            return Err(Error::Hash);
        }
        Ok(())
    }
}
pub fn compatible(v: &Version, mc: &str, loader: &str) -> bool {
    loader != "vanilla"
        && v.game_versions.iter().any(|x| x == mc)
        && v.loaders.iter().any(|x| x.eq_ignore_ascii_case(loader))
}
/// Los resourcepacks no dependen del loader. Su versión declarada es orientativa.
pub fn recurso_compatible(v: &Version, mc: &str) -> bool {
    v.game_versions.iter().any(|x| x == mc)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TipoPack {
    Recursos,
    Shaders,
}
impl TipoPack {
    pub fn carpeta(self) -> &'static str {
        match self {
            Self::Recursos => "resourcepacks",
            Self::Shaders => "shaderpacks",
        }
    }
    fn meta(self) -> &'static str {
        match self {
            Self::Recursos => ".icaro-resourcepacks.json",
            Self::Shaders => ".icaro-shaderpacks.json",
        }
    }
    pub fn etiqueta(self) -> &'static str {
        match self {
            Self::Recursos => "resourcepacks",
            Self::Shaders => "shaders",
        }
    }
}

fn recuperar_packs(game: &Path, tipo: TipoPack) -> Result<(), Error> {
    let dir = game.join(tipo.carpeta());
    let backup = game.join(format!(".icaro-mods-{}-backup", tipo.carpeta()));
    if !dir.exists() && backup.exists() {
        regular(&backup, true)?;
        fs::rename(backup, dir).map_err(|_| Error::Disco)?;
    }
    Ok(())
}

fn registros_packs(game: &Path, tipo: TipoPack) -> Result<Vec<Instalado>, Error> {
    let dir = game.join(tipo.carpeta());
    if !dir.exists() {
        return Ok(vec![]);
    }
    regular(&dir, true)?;
    let path = dir.join(tipo.meta());
    if !path.exists() {
        return Ok(vec![]);
    }
    if regular(&path, false)?.len() > 2 * 1024 * 1024 {
        return Err(Error::Datos);
    }
    let list: Vec<Instalado> = serde_json::from_slice(&fs::read(path).map_err(|_| Error::Disco)?)
        .map_err(|_| Error::Datos)?;
    let mut names = BTreeSet::new();
    let mut projects = BTreeSet::new();
    if list.len() > 512
        || list.iter().any(|r| {
            !nombre_valido(&r.archivo)
                || !r.archivo.ends_with(".zip")
                || !names.insert(r.archivo.to_lowercase())
                || !projects.insert(r.project_id.clone())
        })
    {
        return Err(Error::Datos);
    }
    Ok(list)
}

/// Incluye los ZIP locales sin confundirlos con los JAR de mods.
pub fn inventario_packs(
    store: &Almacen,
    id: &str,
    tipo: TipoPack,
) -> Result<Vec<Instalado>, Error> {
    let game = store.juego(id).map_err(|_| Error::Disco)?;
    let _lock = guardia(&game)?;
    recuperar_packs(&game, tipo)?;
    let dir = game.join(tipo.carpeta());
    let mut list = registros_packs(&game, tipo)?;
    for item in &list {
        regular(&dir.join(&item.archivo), false)?;
    }
    if dir.exists() {
        for entry in fs::read_dir(&dir).map_err(|_| Error::Disco)? {
            let entry = entry.map_err(|_| Error::Disco)?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.ends_with(".zip") && !list.iter().any(|r| r.archivo == name) {
                regular(&entry.path(), false)?;
                if !nombre_valido(&name) {
                    return Err(Error::Datos);
                }
                list.push(Instalado {
                    project_id: String::new(),
                    nombre: name.clone(),
                    version_id: String::new(),
                    version: "Archivo local".into(),
                    publicada: String::new(),
                    archivo: name,
                    sha512: String::new(),
                    activo: true,
                    dependencias: vec![],
                });
            }
        }
    }
    let active = packs_activos(store, id, tipo)?;
    for item in &mut list {
        item.activo = active.contains(&item.archivo);
    }
    Ok(list)
}

pub fn inventario_recursos(store: &Almacen, id: &str) -> Result<Vec<Instalado>, Error> {
    inventario_packs(store, id, TipoPack::Recursos)
}
pub async fn instalar_recursos(
    source: &impl Fuente,
    store: &Almacen,
    id: &str,
    roots: Vec<Version>,
    progress: impl Fn(&str) + Send,
) -> Result<Vec<Instalado>, Error> {
    instalar_packs(source, store, id, roots, TipoPack::Recursos, progress).await
}

pub fn recursos_activos(store: &Almacen, id: &str) -> Result<Vec<String>, Error> {
    let game = store.juego(id).map_err(|_| Error::Disco)?;
    let path = game.join("options.txt");
    if !path.exists() {
        return Ok(vec![]);
    }
    if regular(&path, false)?.len() > 4 * 1024 * 1024 {
        return Err(Error::Datos);
    }
    let text = fs::read_to_string(path).map_err(|_| Error::Disco)?;
    let value = text
        .lines()
        .find_map(|line| line.strip_prefix("resourcePacks:"));
    value.map_or(Ok(vec![]), |json| {
        serde_json::from_str(json).map_err(|_| Error::Datos)
    })
}

/// Cambia únicamente resourcePacks y conserva las demás opciones del juego.
pub fn alternar_recurso(store: &Almacen, id: &str, filename: &str) -> Result<(), Error> {
    let tipo = TipoPack::Recursos;
    if !nombre_valido(filename) || !filename.ends_with(".zip") {
        return Err(Error::Datos);
    }
    let game = store.juego(id).map_err(|_| Error::Disco)?;
    let _lock = guardia(&game)?;
    regular(&game.join(tipo.carpeta()), true)?;
    regular(&game.join(tipo.carpeta()).join(filename), false)?;
    let path = game.join("options.txt");
    if path.exists() && regular(&path, false)?.len() > 4 * 1024 * 1024 {
        return Err(Error::Datos);
    }
    let text = if path.exists() {
        fs::read_to_string(&path).map_err(|_| Error::Disco)?
    } else {
        String::new()
    };
    let mut active = recursos_activos(store, id)?;
    let key = format!("file/{filename}");
    if active.contains(&key) {
        active.retain(|s| s != &key);
    } else {
        active.push(key);
    }
    let replacement = format!(
        "resourcePacks:{}",
        serde_json::to_string(&active).map_err(|_| Error::Datos)?
    );
    let mut replaced = false;
    let mut lines: Vec<String> = text
        .lines()
        .map(|line| {
            if line.starts_with("resourcePacks:") {
                replaced = true;
                replacement.clone()
            } else {
                line.to_owned()
            }
        })
        .collect();
    if !replaced {
        lines.push(replacement);
    }
    let next = game.join(".icaro-resource-options-next");
    if next.exists() {
        regular(&next, false)?;
    }
    guardar_archivo(&next, format!("{}\n", lines.join("\n")).as_bytes())?;
    let backup = game.join(".icaro-resource-options-backup");
    if backup.exists() {
        regular(&backup, false)?;
        fs::remove_file(&backup).map_err(|_| Error::Disco)?;
    }
    if path.exists() {
        fs::rename(&path, &backup).map_err(|_| Error::Disco)?;
    }
    if fs::rename(&next, &path).is_err() {
        if backup.exists() {
            let _ = fs::rename(&backup, &path);
        }
        return Err(Error::Disco);
    }
    if backup.exists() {
        let _ = fs::remove_file(backup);
    }
    Ok(())
}

/// Conserva el ZIP original: Minecraft lo lee directamente desde resourcepacks.
/// No exige loader ni bloquea versiones de Minecraft fuera de las declaradas.
pub async fn instalar_packs(
    source: &impl Fuente,
    store: &Almacen,
    id: &str,
    roots: Vec<Version>,
    tipo: TipoPack,
    progress: impl Fn(&str) + Send,
) -> Result<Vec<Instalado>, Error> {
    let game = store.juego(id).map_err(|_| Error::Disco)?;
    let _lock = guardia(&game)?;
    recuperar_packs(&game, tipo)?;
    let dir = game.join(tipo.carpeta());
    let mut installed = registros_packs(&game, tipo)?;
    let mut renombres = vec![];
    for item in &installed {
        comprobar_actual(&dir, item)?;
    }
    let mut random = [0u8; 16];
    getrandom::fill(&mut random).map_err(|_| Error::Disco)?;
    let stage = game.join(format!(
        ".icaro-mods-{}-next-{}",
        tipo.carpeta(),
        hash(&random)
    ));
    fs::create_dir(&stage).map_err(|_| Error::Disco)?;
    let result = async {
        if dir.exists() {
            copiar(&dir, &stage, 0, &mut 0, &mut 0)?;
        }
        for version in roots {
            if installed
                .iter()
                .any(|r| r.project_id == version.project_id && r.version_id == version.id)
            {
                continue;
            }
            let file = version
                .files
                .iter()
                .find(|f| f.primary)
                .or(version.files.first())
                .ok_or(Error::Datos)?;
            if !nombre_valido(&file.filename)
                || !file.filename.ends_with(".zip")
                || file.size == 0
                || file.size > MAX_FILE
                || file.hashes.sha512.len() != 128
                || !file.hashes.sha512.bytes().all(|b| b.is_ascii_hexdigit())
                || !crate::modrinth::supported_image(&file.url)
            {
                return Err(Error::Datos);
            }
            let old = installed
                .iter()
                .position(|r| r.project_id == version.project_id);
            let target = stage.join(&file.filename);
            if target.exists() && old.is_none_or(|n| installed[n].archivo != file.filename) {
                return Err(Error::Archivo(file.filename.clone()));
            }
            progress(&format!("Descargando {}…", version.name));
            let bytes = source.archivo(file).await?;
            if bytes.len() as u64 != file.size || hash(&bytes) != file.hashes.sha512.to_lowercase()
            {
                return Err(Error::Hash);
            }
            // Validar el contenedor y la metadata sin extraer ni ejecutar su contenido.
            let mut archive =
                zip::ZipArchive::new(std::io::Cursor::new(&bytes)).map_err(|_| Error::Datos)?;
            if tipo == TipoPack::Recursos {
                let mut metadata = archive.by_name("pack.mcmeta").map_err(|_| Error::Datos)?;
                if metadata.size() > 1024 * 1024 {
                    return Err(Error::Datos);
                }
                let mut data = Vec::new();
                std::io::Read::read_to_end(&mut metadata, &mut data).map_err(|_| Error::Datos)?;
                let value: serde_json::Value =
                    serde_json::from_slice(&data).map_err(|_| Error::Datos)?;
                if !value.get("pack").is_some_and(|p| p.is_object()) {
                    return Err(Error::Datos);
                }
            } else if !archive.file_names().any(|name| {
                name.is_ok_and(|name| name.starts_with("shaders/") && !name.ends_with('/'))
            }) {
                return Err(Error::Datos);
            }
            let name = source.nombre(&version.project_id).await?;
            if let Some(n) = old {
                renombres.push((installed[n].archivo.clone(), file.filename.clone()));
                fs::remove_file(stage.join(&installed[n].archivo)).map_err(|_| Error::Disco)?;
                installed.remove(n);
            }
            guardar_archivo(&target, &bytes)?;
            installed.push(Instalado {
                project_id: version.project_id,
                nombre: name,
                version_id: version.id,
                version: version.version_number,
                publicada: version.date_published,
                archivo: file.filename.clone(),
                sha512: file.hashes.sha512.clone(),
                activo: true,
                dependencias: version.dependencies,
            });
        }
        guardar_archivo(
            &stage.join(tipo.meta()),
            &serde_json::to_vec_pretty(&installed).map_err(|_| Error::Datos)?,
        )?;
        let backup = game.join(format!(".icaro-mods-{}-backup", tipo.carpeta()));
        if backup.exists() {
            borrar_preparacion(&game, &backup)?;
        }
        if dir.exists() {
            regular(&dir, true)?;
            fs::rename(&dir, &backup).map_err(|_| Error::Disco)?;
        }
        if fs::rename(&stage, &dir).is_err() {
            if backup.exists() {
                let _ = fs::rename(&backup, &dir);
            }
            return Err(Error::Disco);
        }
        // Si una actualización cambia el nombre del ZIP, preservar activación y prioridad.
        let config_result = (|| -> Result<(), Error> {
            if renombres.is_empty() {
                Ok(())
            } else if tipo == TipoPack::Recursos {
                let mut active = recursos_activos(store, id)?;
                for name in &mut active {
                    for (old, new) in &renombres {
                        if *name == format!("file/{old}") {
                            *name = format!("file/{new}");
                            break;
                        }
                    }
                }
                escribir_config(
                    &game,
                    &game.join("options.txt"),
                    &[(
                        "resourcePacks:",
                        serde_json::to_string(&active).map_err(|_| Error::Datos)?,
                    )],
                )
            } else {
                let active = packs_activos(store, id, tipo)?;
                if let Some((_, new)) = renombres.iter().find(|(old, _)| active.contains(old)) {
                    escribir_config(
                        &game,
                        &game.join("config/iris.properties"),
                        &[("shaderPack=", new.clone())],
                    )
                } else {
                    Ok(())
                }
            }
        })();
        if let Err(e) = config_result {
            let _ = fs::rename(&dir, &stage);
            if backup.exists() {
                let _ = fs::rename(&backup, &dir);
            }
            return Err(e);
        }
        if backup.exists() {
            let _ = borrar_preparacion(&game, &backup);
        }
        Ok(installed)
    }
    .await;
    if stage.exists() {
        let _ = borrar_preparacion(&game, &stage);
    }
    result
}
/// Iris usa un solo shader activo; los resourcepacks conservan su orden de prioridad.
pub fn packs_activos(store: &Almacen, id: &str, tipo: TipoPack) -> Result<Vec<String>, Error> {
    if tipo == TipoPack::Recursos {
        return Ok(recursos_activos(store, id)?
            .into_iter()
            .filter_map(|x| x.strip_prefix("file/").map(str::to_owned))
            .collect());
    }
    let game = store.juego(id).map_err(|_| Error::Disco)?;
    let path = game.join("config/iris.properties");
    if !path.exists() {
        return Ok(vec![]);
    }
    let text = leer_config(&path)?;
    if text.lines().any(|l| l.trim() == "enableShaders=false") {
        return Ok(vec![]);
    }
    Ok(text
        .lines()
        .find_map(|l| l.strip_prefix("shaderPack="))
        .filter(|s| nombre_valido(s))
        .map(str::to_owned)
        .into_iter()
        .collect())
}
fn leer_config(path: &Path) -> Result<String, Error> {
    if !path.exists() {
        return Ok(String::new());
    }
    if regular(path, false)?.len() > 4 * 1024 * 1024 {
        return Err(Error::Datos);
    }
    fs::read_to_string(path).map_err(|_| Error::Disco)
}
fn escribir_config(game: &Path, path: &Path, values: &[(&str, String)]) -> Result<(), Error> {
    let text = leer_config(path)?;
    let mut lines: Vec<String> = text
        .lines()
        .filter(|line| !values.iter().any(|(key, _)| line.starts_with(key)))
        .map(str::to_owned)
        .collect();
    lines.extend(values.iter().map(|(k, v)| format!("{k}{v}")));
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir(parent).map_err(|_| Error::Disco)?;
        }
        regular(parent, true)?;
    }
    let next = game.join(".icaro-pack-options-next");
    if next.exists() {
        regular(&next, false)?;
    }
    guardar_archivo(&next, format!("{}\n", lines.join("\n")).as_bytes())?;
    let backup = game.join(".icaro-pack-options-backup");
    if backup.exists() {
        regular(&backup, false)?;
        fs::remove_file(&backup).map_err(|_| Error::Disco)?;
    }
    if path.exists() {
        fs::rename(path, &backup).map_err(|_| Error::Disco)?;
    }
    if fs::rename(&next, path).is_err() {
        if backup.exists() {
            let _ = fs::rename(&backup, path);
        }
        return Err(Error::Disco);
    }
    Ok(())
}
pub fn activar_packs(
    store: &Almacen,
    id: &str,
    tipo: TipoPack,
    files: &BTreeSet<String>,
    activo: bool,
) -> Result<(), Error> {
    let game = store.juego(id).map_err(|_| Error::Disco)?;
    let _lock = guardia(&game)?;
    regular(&game.join(tipo.carpeta()), true)?;
    for name in files {
        if !nombre_valido(name) || !name.ends_with(".zip") {
            return Err(Error::Datos);
        }
        regular(&game.join(tipo.carpeta()).join(name), false)?;
    }
    if tipo == TipoPack::Recursos {
        let mut names = recursos_activos(store, id)?;
        for name in files {
            let key = format!("file/{name}");
            names.retain(|n| n != &key);
            if activo {
                names.push(key);
            }
        }
        escribir_config(
            &game,
            &game.join("options.txt"),
            &[(
                "resourcePacks:",
                serde_json::to_string(&names).map_err(|_| Error::Datos)?,
            )],
        )
    } else {
        let path = game.join("config/iris.properties");
        // No inventar soporte de shaders: el archivo lo crea Iris cuando está instalado.
        if !path.exists() {
            return Err(Error::Compatible(
                "shaders: instala e inicia Iris antes de activarlos".into(),
            ));
        }
        if activo && files.len() != 1 {
            return Err(Error::Compatible(
                "shaders: solo puede estar activo un paquete a la vez".into(),
            ));
        }
        if let Some(name) = files.first().filter(|_| activo) {
            escribir_config(
                &game,
                &path,
                &[
                    ("shaderPack=", name.clone()),
                    ("enableShaders=", "true".into()),
                ],
            )
        } else if packs_activos(store, id, tipo)?
            .iter()
            .any(|n| files.contains(n))
        {
            escribir_config(&game, &path, &[("enableShaders=", "false".into())])
        } else {
            Ok(())
        }
    }
}
/// Retira solo ZIP seleccionados; el conjunto previo queda recuperable en backup.
pub fn quitar_packs(
    store: &Almacen,
    id: &str,
    tipo: TipoPack,
    files: &BTreeSet<String>,
) -> Result<(), Error> {
    let game = store.juego(id).map_err(|_| Error::Disco)?;
    let _lock = guardia(&game)?;
    recuperar_packs(&game, tipo)?;
    let dir = game.join(tipo.carpeta());
    regular(&dir, true)?;
    let mut records = registros_packs(&game, tipo)?;
    for name in files {
        if !nombre_valido(name) || !name.ends_with(".zip") {
            return Err(Error::Datos);
        }
        regular(&dir.join(name), false)?;
    }
    let config_path = game.join(if tipo == TipoPack::Recursos {
        "options.txt"
    } else {
        "config/iris.properties"
    });
    let previous_config = if config_path.exists() {
        Some(leer_config(&config_path)?)
    } else {
        None
    };
    let mut nonce = [0; 16];
    getrandom::fill(&mut nonce).map_err(|_| Error::Disco)?;
    let stage = game.join(format!(
        ".icaro-mods-{}-next-{}",
        tipo.carpeta(),
        hash(&nonce)
    ));
    fs::create_dir(&stage).map_err(|_| Error::Disco)?;
    let result = (|| {
        copiar(&dir, &stage, 0, &mut 0, &mut 0)?;
        for name in files {
            fs::remove_file(stage.join(name)).map_err(|_| Error::Disco)?;
        }
        records.retain(|r| !files.contains(&r.archivo));
        guardar_archivo(
            &stage.join(tipo.meta()),
            &serde_json::to_vec_pretty(&records).map_err(|_| Error::Datos)?,
        )?;
        let backup = game.join(format!(".icaro-mods-{}-backup", tipo.carpeta()));
        if backup.exists() {
            borrar_preparacion(&game, &backup)?;
        }
        fs::rename(&dir, &backup).map_err(|_| Error::Disco)?;
        if fs::rename(&stage, &dir).is_err() {
            let _ = fs::rename(&backup, &dir);
            return Err(Error::Disco);
        }
        let cfg = (|| -> Result<(), Error> {
            if tipo == TipoPack::Recursos {
                let mut active = recursos_activos(store, id)?;
                active.retain(|x| !files.iter().any(|n| x == &format!("file/{n}")));
                escribir_config(
                    &game,
                    &game.join("options.txt"),
                    &[(
                        "resourcePacks:",
                        serde_json::to_string(&active).map_err(|_| Error::Datos)?,
                    )],
                )
            } else if packs_activos(store, id, tipo)?
                .iter()
                .any(|n| files.contains(n))
            {
                escribir_config(
                    &game,
                    &game.join("config/iris.properties"),
                    &[("enableShaders=", "false".into())],
                )
            } else {
                Ok(())
            }
        })();
        if let Err(e) = cfg {
            let _ = fs::rename(&dir, &stage);
            let _ = fs::rename(&backup, &dir);
            return Err(e);
        }
        if let Err(error) = crate::papelera::enviar(&backup) {
            if backup.exists() {
                let _ = fs::rename(&dir, &stage);
                let _ = fs::rename(&backup, &dir);
                if let Some(text) = &previous_config {
                    let _ = guardar_archivo(&config_path, text.as_bytes());
                } else if config_path.exists() {
                    let _ = fs::remove_file(&config_path);
                }
            }
            return Err(Error::Papelera(error));
        }
        Ok(())
    })();
    if stage.exists() {
        let _ = borrar_preparacion(&game, &stage);
    }
    result
}

fn principal(v: &Version) -> Result<&ModFile, Error> {
    let f = v
        .files
        .iter()
        .find(|f| f.primary)
        .or(v.files.first())
        .ok_or(Error::Datos)?;
    if !nombre_valido(&f.filename)
        || !f.filename.ends_with(".jar")
        || f.size == 0
        || f.size > MAX_FILE
        || f.hashes.sha512.len() != 128
        || !f.hashes.sha512.bytes().all(|b| b.is_ascii_hexdigit())
        || !crate::modrinth::supported_image(&f.url)
    {
        return Err(Error::Datos);
    }
    Ok(f)
}
fn ultima(mut versions: Vec<Version>) -> Option<Version> {
    versions.sort_by(|a, b| b.date_published.cmp(&a.date_published));
    versions
        .iter()
        .find(|v| v.version_type == "release")
        .cloned()
        .or_else(|| versions.into_iter().next())
}
pub async fn actualizaciones(
    source: &impl Fuente,
    installed: &[Instalado],
    mc: &str,
    loader: &str,
) -> Result<Vec<Version>, Error> {
    let identificados: Vec<_> = installed.iter().filter(|m| !m.project_id.is_empty()
        && m.sha512.len()==128 && m.sha512.bytes().all(|b|b.is_ascii_hexdigit())).collect();
    let hashes: Vec<_> = identificados.iter().map(|m|m.sha512.to_lowercase()).collect::<BTreeSet<_>>().into_iter().collect();
    let batch = source.actualizaciones_por_hash(&hashes,mc,loader).await?;
    if let Some(batch) = &batch {
        let mut result=vec![];
        for m in &identificados {
            if let Some(v)=batch.get(&m.sha512.to_lowercase()).filter(|v|v.project_id==m.project_id
                && v.id!=m.version_id && v.version_type=="release" && compatible(v,mc,loader)) {
                let published=if m.publicada.is_empty(){source.version(&m.version_id).await?.date_published}else{m.publicada.clone()};
                if v.date_published>published {result.push(v.clone());}
            }
        }
        let fallback:Vec<_>=installed.iter().filter(|m|!identificados.iter().any(|i|i.project_id==m.project_id)).cloned().collect();
        // Los inventarios antiguos sin hash mantienen su comprobación por proyecto.
        result.extend(actualizaciones_proyectos(source,&fallback,mc,loader).await?);
        return Ok(result);
    }
    actualizaciones_proyectos(source,installed,mc,loader).await
}

async fn actualizaciones_proyectos(source:&impl Fuente, installed:&[Instalado], mc:&str, loader:&str) -> Result<Vec<Version>, Error> {
    let mut result = vec![];
    for m in installed.iter().filter(|m| !m.project_id.is_empty()) {
        let versions = source.versiones(&m.project_id, mc, loader).await?;
        if let Some(v) = ultima(
            versions
                .into_iter()
                .filter(|v| {
                    v.project_id == m.project_id
                        && compatible(v, mc, loader)
                        && v.version_type == "release"
                })
                .collect(),
        ) {
            let published = if m.publicada.is_empty() {
                source.version(&m.version_id).await?.date_published
            } else {
                m.publicada.clone()
            };
            if v.id != m.version_id && v.date_published > published {
                result.push(v);
            }
        }
    }
    Ok(result)
}
pub async fn resolver(
    source: &impl Fuente,
    roots: Vec<Version>,
    installed: &[Instalado],
    mc: &str,
    loader: &str,
) -> Result<Vec<Version>, Error> {
    let mut selected: BTreeMap<String, Version> = BTreeMap::new();
    let mut queue = VecDeque::new();
    for v in roots {
        if !compatible(&v, mc, loader) {
            return Err(Error::Compatible(v.name));
        }
        if let Some(prior) = selected.insert(v.project_id.clone(), v.clone()) {
            if prior.id != v.id {
                return Err(Error::Conflicto(v.name));
            }
        }
        queue.push_back(v);
    }
    let mut visited = BTreeSet::new();
    while let Some(v) = queue.pop_front() {
        if !visited.insert(v.id.clone()) {
            continue;
        }
        if visited.len() > 512 {
            return Err(Error::Datos);
        }
        for dep in v
            .dependencies
            .iter()
            .filter(|d| d.dependency_type == "required")
        {
            let candidate = if let Some(id) = &dep.version_id {
                let pinned = source.version(id).await?;
                if pinned.id != *id
                    || dep
                        .project_id
                        .as_ref()
                        .is_some_and(|p| *p != pinned.project_id)
                {
                    return Err(Error::Datos);
                }
                pinned
            } else if let Some(p) = &dep.project_id {
                if let Some(existing) = selected.get(p) {
                    existing.clone()
                } else if let Some(existing) = installed.iter().find(|m| m.project_id == *p) {
                    let version = source.version(&existing.version_id).await?;
                    if version.id != existing.version_id {
                        return Err(Error::Datos);
                    }
                    version
                } else {
                    ultima(
                        source
                            .versiones(p, mc, loader)
                            .await?
                            .into_iter()
                            .filter(|v| v.project_id == *p && compatible(v, mc, loader))
                            .collect(),
                    )
                    .ok_or_else(|| Error::Compatible(p.clone()))?
                }
            } else {
                return Err(Error::Compatible(
                    dep.file_name
                        .clone()
                        .unwrap_or("Dependencia externa".into()),
                ));
            };
            if !compatible(&candidate, mc, loader) {
                return Err(Error::Compatible(candidate.name));
            }
            if dep
                .project_id
                .as_ref()
                .is_some_and(|p| *p != candidate.project_id)
            {
                return Err(Error::Datos);
            }
            if let Some(prior) = selected.get(&candidate.project_id) {
                if prior.id != candidate.id {
                    return Err(Error::Conflicto(candidate.name));
                }
            } else {
                selected.insert(candidate.project_id.clone(), candidate.clone());
                queue.push_back(candidate);
            }
        }
    }
    for v in selected.values() {
        for dep in v
            .dependencies
            .iter()
            .filter(|d| d.dependency_type == "incompatible")
        {
            let matches = |project: &str, version: &str| {
                (dep.project_id.is_some() || dep.version_id.is_some())
                    && dep.project_id.as_deref().is_none_or(|p| p == project)
                    && dep.version_id.as_deref().is_none_or(|id| id == version)
            };
            let conflict = selected
                .values()
                .any(|other| matches(&other.project_id, &other.id))
                || installed.iter().any(|other| {
                    other.activo
                        && !selected.contains_key(&other.project_id)
                        && matches(&other.project_id, &other.version_id)
                });
            if conflict {
                return Err(Error::Conflicto(v.name.clone()));
            }
        }
    }
    for old in installed
        .iter()
        .filter(|m| m.activo && !selected.contains_key(&m.project_id))
    {
        for dep in &old.dependencias {
            let project = if dep.dependency_type == "required" && dep.project_id.is_none() {
                if let Some(id) = &dep.version_id {
                    let version = source.version(id).await?;
                    if version.id != *id {
                        return Err(Error::Datos);
                    }
                    Some(version.project_id)
                } else {
                    None
                }
            } else {
                dep.project_id.clone()
            };
            if selected.values().any(|v| {
                let same = project.as_deref().is_some_and(|p| p == v.project_id)
                    || dep.version_id.as_deref().is_some_and(|id| id == v.id);
                (dep.dependency_type == "required"
                    && same
                    && dep.version_id.as_deref().is_some_and(|id| id != v.id))
                    || (dep.dependency_type == "incompatible"
                        && same
                        && dep.version_id.as_deref().is_none_or(|id| id == v.id))
            }) {
                return Err(Error::Conflicto(old.nombre.clone()));
            }
        }
    }
    Ok(selected.into_values().collect())
}
pub(crate) fn borrar_preparacion(game: &Path, path: &Path) -> Result<(), Error> {
    regular(path, true)?;
    let root = fs::canonicalize(game).map_err(|_| Error::Disco)?;
    let resolved = fs::canonicalize(path).map_err(|_| Error::Disco)?;
    if resolved.parent() != Some(root.as_path())
        || !resolved
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .starts_with(".icaro-mods-")
    {
        return Err(Error::Datos);
    }
    fs::remove_dir_all(resolved).map_err(|_| Error::Disco)
}
fn copiar(
    source: &Path,
    target: &Path,
    depth: usize,
    total: &mut u64,
    count: &mut usize,
) -> Result<(), Error> {
    if depth > 16 {
        return Err(Error::Datos);
    }
    regular(source, true)?;
    for entry in fs::read_dir(source).map_err(|_| Error::Disco)? {
        let entry = entry.map_err(|_| Error::Disco)?;
        *count += 1;
        if *count > 4096 {
            return Err(Error::Datos);
        }
        let meta = fs::symlink_metadata(entry.path()).map_err(|_| Error::Disco)?;
        if enlace(&meta) {
            return Err(Error::Datos);
        }
        let dest = target.join(entry.file_name());
        if meta.is_dir() {
            fs::create_dir(&dest).map_err(|_| Error::Disco)?;
            copiar(&entry.path(), &dest, depth + 1, total, count)?;
        } else if meta.is_file() {
            *total = total.saturating_add(meta.len());
            if *total > 2 * 1024 * 1024 * 1024 {
                return Err(Error::Datos);
            }
            fs::copy(entry.path(), dest).map_err(|_| Error::Disco)?;
        } else {
            return Err(Error::Datos);
        }
    }
    Ok(())
}
fn comprobar_actual(dir: &Path, m: &Instalado) -> Result<(), Error> {
    let path = ruta_mod(dir, m);
    if regular(&path, false)?.len() > MAX_FILE {
        return Err(Error::Datos);
    }
    let bytes = fs::read(path).map_err(|_| Error::Disco)?;
    if hash(&bytes) != m.sha512.to_lowercase() {
        return Err(Error::Archivo(m.archivo.clone()));
    }
    Ok(())
}
fn guardar_archivo(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    let mut f = fs::File::create(path).map_err(|_| Error::Disco)?;
    f.write_all(bytes)
        .and_then(|_| f.sync_all())
        .map_err(|_| Error::Disco)
}
pub async fn instalar(
    source: &impl Fuente,
    store: &Almacen,
    id: &str,
    roots: Vec<Version>,
    mc: &str,
    loader: &str,
    progress: impl Fn(&str) + Send,
) -> Result<Vec<Instalado>, Error> {
    let game = store.juego(id).map_err(|_| Error::Disco)?;
    let _lock = guardia(&game)?;
    recuperar(&game)?;
    let dir = game.join("mods");
    let mut installed = registros(&game)?;
    for m in &mut installed {
        m.activo = dir.join(&m.archivo).exists();
        comprobar_actual(&dir, m)?;
    }
    progress("Resolviendo dependencias obligatorias…");
    let mut active: BTreeSet<String> = roots
        .iter()
        .filter(|v| {
            installed
                .iter()
                .find(|m| m.project_id == v.project_id)
                .is_none_or(|m| m.activo)
        })
        .map(|v| v.project_id.clone())
        .collect();
    let plan = resolver(source, roots, &installed, mc, loader).await?;
    loop {
        let before = active.len();
        for v in plan
            .iter()
            .filter(|v| active.contains(&v.project_id))
            .cloned()
            .collect::<Vec<_>>()
        {
            for dep in v
                .dependencies
                .iter()
                .filter(|d| d.dependency_type == "required")
            {
                if let Some(target) = plan.iter().find(|p| {
                    dep.project_id
                        .as_deref()
                        .is_some_and(|id| id == p.project_id)
                        || dep.version_id.as_deref().is_some_and(|id| id == p.id)
                }) {
                    active.insert(target.project_id.clone());
                }
            }
        }
        if active.len() == before {
            break;
        }
    }
    let mut random = [0u8; 16];
    getrandom::fill(&mut random).map_err(|_| Error::Disco)?;
    let stage = game.join(format!(
        ".icaro-mods-next-{}",
        random
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    ));
    fs::create_dir(&stage).map_err(|_| Error::Disco)?;
    let result = async {
        if dir.exists() {
            copiar(&dir, &stage, 0, &mut 0, &mut 0)?;
        }
        let mut occupied: BTreeSet<String> =
            installed.iter().map(|m| m.archivo.to_lowercase()).collect();
        // Una consulta por lote en lugar de una ficha por cada mod descargado.
        let pendientes: Vec<_> = plan
            .iter()
            .filter(|v| !installed.iter().any(|m| m.project_id == v.project_id && m.version_id == v.id))
            .map(|v| v.project_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let nombres = source.nombres(&pendientes).await?;
        for v in plan {
            if let Some(m) = installed
                .iter_mut()
                .find(|m| m.project_id == v.project_id && m.version_id == v.id)
            {
                if !m.activo && active.contains(&v.project_id) {
                    let source = ruta_mod(&stage, m);
                    let target = stage.join(&m.archivo);
                    if target.exists() {
                        return Err(Error::Archivo(m.archivo.clone()));
                    }
                    fs::rename(source, target).map_err(|_| Error::Disco)?;
                    m.activo = true;
                }
                continue;
            }
            let file = principal(&v)?.clone();
            let old = installed.iter().position(|m| m.project_id == v.project_id);
            if occupied.contains(&file.filename.to_lowercase())
                && old.is_none_or(|i| {
                    installed[i].archivo.to_lowercase() != file.filename.to_lowercase()
                })
            {
                return Err(Error::Archivo(file.filename.clone()));
            }
            let activo =
                active.contains(&v.project_id) || old.map_or(true, |i| installed[i].activo);
            let destination = stage.join(if activo {
                file.filename.clone()
            } else {
                format!("{}.disabled", file.filename)
            });
            if destination.exists()
                && old.is_none_or(|i| ruta_mod(&stage, &installed[i]) != destination)
            {
                return Err(Error::Archivo(file.filename.clone()));
            }
            progress(&format!("Descargando {}…", v.name));
            let bytes = source.archivo(&file).await?;
            if bytes.len() as u64 != file.size || hash(&bytes) != file.hashes.sha512.to_lowercase()
            {
                return Err(Error::Hash);
            }
            let name = nombres.get(&v.project_id).cloned().ok_or(Error::Datos)?;
            if let Some(i) = old {
                occupied.remove(&installed[i].archivo.to_lowercase());
                fs::remove_file(ruta_mod(&stage, &installed[i])).map_err(|_| Error::Disco)?;
                installed.remove(i);
            }
            guardar_archivo(&destination, &bytes)?;
            occupied.insert(file.filename.to_lowercase());
            installed.push(Instalado {
                project_id: v.project_id,
                nombre: name,
                version_id: v.id,
                version: v.version_number,
                publicada: v.date_published,
                archivo: file.filename.clone(),
                sha512: file.hashes.sha512.clone(),
                activo,
                dependencias: v.dependencies,
            });
        }
        guardar_archivo(
            &stage.join(META),
            &serde_json::to_vec_pretty(&installed).map_err(|_| Error::Datos)?,
        )?;
        progress("Guardando los mods de la instancia…");
        let backup = game.join(".icaro-mods-backup");
        if backup.exists() {
            borrar_preparacion(&game, &backup)?;
        }
        if dir.exists() {
            regular(&dir, true)?;
            fs::rename(&dir, &backup).map_err(|_| Error::Disco)?;
        }
        if fs::rename(&stage, &dir).is_err() {
            if backup.exists() {
                let _ = fs::rename(&backup, &dir);
            }
            return Err(Error::Disco);
        }
        if backup.exists() {
            let _ = borrar_preparacion(&game, &backup);
        }
        Ok(installed)
    }
    .await;
    if stage.exists() {
        let _ = borrar_preparacion(&game, &stage);
    }
    result
}

/// Metadata preparada para una migración; se valida cada JAR antes de publicarla.
pub(crate) fn registrar_inventario(
    store: &Almacen,
    id: &str,
    list: &[Instalado],
) -> Result<(), Error> {
    let game = store.juego(id).map_err(|_| Error::Disco)?;
    let _lock = guardia(&game)?;
    let dir = game.join("mods");
    regular(&dir, true)?;
    let mut projects = BTreeSet::new();
    let mut names = BTreeSet::new();
    if list.len() > 512 {
        return Err(Error::Datos);
    }
    let records = list
        .iter()
        .filter(|m| !m.project_id.is_empty())
        .cloned()
        .collect::<Vec<_>>();
    for m in &records {
        if !nombre_valido(&m.archivo)
            || !m.archivo.ends_with(".jar")
            || !projects.insert(m.project_id.clone())
            || !names.insert(m.archivo.to_lowercase())
        {
            return Err(Error::Datos);
        }
        comprobar_actual(&dir, m)?;
    }
    guardar_registros(&game, &records)
}

pub fn alternar(store: &Almacen, id: &str, filename: &str) -> Result<(), Error> {
    if !nombre_valido(filename) {
        return Err(Error::Datos);
    }
    let game = store.juego(id).map_err(|_| Error::Disco)?;
    let _lock = guardia(&game)?;
    recuperar(&game)?;
    let dir = game.join("mods");
    let active = dir.join(filename);
    let disabled = dir.join(format!("{filename}.disabled"));
    let (source, target) = if active.exists() {
        (active, disabled)
    } else {
        (disabled, active)
    };
    regular(&source, false)?;
    if target.exists() {
        return Err(Error::Archivo(filename.into()));
    }
    fs::rename(source, target).map_err(|_| Error::Disco)
}

/// Ticket de una retirada reversible. La ruta no se acepta desde la interfaz.
pub struct Retirados {
    papelera: crate::papelera::Ticket,
    instancia: String,
    carpeta: PathBuf,
    mods: Vec<Instalado>,
    hashes: BTreeMap<String, String>,
}

fn guardar_registros(game: &Path, list: &[Instalado]) -> Result<(), Error> {
    let bytes = serde_json::to_vec_pretty(list).map_err(|_| Error::Datos)?;
    let dir = game.join("mods");
    let temp = dir.join(".icaro-mods.json.next");
    if temp.exists() {
        regular(&temp, false)?;
    }
    fs::write(&temp, bytes).map_err(|_| Error::Disco)?;
    fs::rename(temp, dir.join(".icaro-mods.json")).map_err(|_| Error::Disco)
}

pub fn retirar(store: &Almacen, id: &str, files: &BTreeSet<String>) -> Result<Retirados, Error> {
    retirar_con_hash(store, id, files, &BTreeMap::new())
}

fn hash_archivo(path: &Path) -> Result<String, Error> {
    if regular(path, false)?.len() > MAX_FILE {
        return Err(Error::Datos);
    }
    let mut file = fs::File::open(path)
        .map_err(|_| Error::Disco)?
        .take(MAX_FILE + 1);
    let mut hash = Sha512::new();
    let mut buf = [0; 64 * 1024];
    let mut size = 0;
    loop {
        let n = file.read(&mut buf).map_err(|_| Error::Disco)?;
        if n == 0 {
            break;
        }
        size += n as u64;
        if size > MAX_FILE {
            return Err(Error::Datos);
        }
        hash.update(&buf[..n]);
    }
    Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// Revisión y retirada comparten bloqueo; un resultado visual antiguo nunca
/// puede retirar el nuevo contenido de un archivo con el mismo nombre.
pub(crate) fn retirar_con_hash(
    store: &Almacen,
    id: &str,
    files: &BTreeSet<String>,
    hashes: &BTreeMap<String, String>,
) -> Result<Retirados, Error> {
    if files.is_empty() {
        return Err(Error::Datos);
    }
    let inventory = inventario(store, id)?;
    let game = store.juego(id).map_err(|_| Error::Disco)?;
    let _lock = guardia(&game)?;
    let manifest = registros(&game)?;
    let previous_manifest = manifest.clone();
    let selected: Vec<_> = inventory
        .into_iter()
        .filter(|m| files.contains(&m.archivo))
        .collect();
    if selected.len() != files.len() {
        return Err(Error::Datos);
    }
    let dir = game.join("mods");
    for item in &selected {
        if let Some(expected) = hashes.get(&item.archivo) {
            if hash_archivo(&ruta_mod(&dir, item))? != *expected {
                return Err(Error::Archivo(item.archivo.clone()));
            }
        }
    }
    let root = store
        .carpeta(id)
        .map_err(|_| Error::Disco)?
        .join(".icaro-transactions");
    if !root.exists() {
        fs::create_dir(&root).map_err(|_| Error::Disco)?;
    }
    regular(&root, true)?;
    let mut random = [0u8; 16];
    getrandom::fill(&mut random).map_err(|_| Error::Datos)?;
    let trash = root.join(&hash(&random)[..16]);
    fs::create_dir(&trash).map_err(|_| Error::Disco)?;
    let dir = game.join("mods");
    let exact: BTreeMap<String, String> = selected
        .iter()
        .map(|m| Ok((m.archivo.clone(), hash_archivo(&ruta_mod(&dir, m))?)))
        .collect::<Result<_, Error>>()?;
    crate::almacenamiento::escribir_json(
        &trash.join("recuperacion.json"),
        &serde_json::json!({"schema":1,"mods":selected,"hashes":exact}),
    )
    .map_err(|_| Error::Disco)?;
    let mut moved: Vec<(PathBuf, PathBuf)> = vec![];
    let result = (|| {
        for item in &selected {
            let src = ruta_mod(&dir, item);
            regular(&src, false)?;
            let dst = trash.join(src.file_name().ok_or(Error::Datos)?);
            fs::rename(&src, &dst).map_err(|_| Error::Disco)?;
            moved.push((src, dst));
        }
        let kept: Vec<_> = manifest
            .into_iter()
            .filter(|m| !files.contains(&m.archivo))
            .collect();
        guardar_registros(&game, &kept)
    })();
    if let Err(e) = result {
        for (src, dst) in moved.into_iter().rev() {
            let _ = fs::rename(dst, src);
        }
        return Err(e);
    }
    let papelera = match crate::papelera::enviar(&trash) {
        Ok(ticket) => ticket,
        Err(error) => {
            // Si el sistema no retiró la carpeta, revierte la transacción.
            if trash.exists() {
                for (src, dst) in moved.into_iter().rev() {
                    let _ = fs::rename(dst, src);
                }
                let _ = guardar_registros(&game, &previous_manifest);
            }
            return Err(Error::Papelera(error));
        }
    };
    Ok(Retirados {
        papelera,
        instancia: id.into(),
        carpeta: trash,
        mods: selected,
        hashes: exact,
    })
}

impl Retirados {
    pub(crate) fn carpeta_recuperada(&self) -> Result<PathBuf, Error> {
        if !self.carpeta.exists() {
            crate::papelera::restaurar(&self.papelera).map_err(|_| Error::Disco)?;
        }
        regular(&self.carpeta, true)?;
        Ok(self.carpeta.clone())
    }
}
pub fn restaurar(store: &Almacen, id: &str, ticket: &Retirados) -> Result<(), Error> {
    if ticket.instancia != id {
        return Err(Error::Datos);
    }
    let game = store.juego(id).map_err(|_| Error::Disco)?;
    let _lock = guardia(&game)?;
    ticket.carpeta_recuperada()?;
    let parent = ticket.carpeta.parent().ok_or(Error::Datos)?;
    regular(parent, true)?;
    if parent.canonicalize().map_err(|_| Error::Disco)?
        != store
            .carpeta(id)
            .map_err(|_| Error::Disco)?
            .join(".icaro-transactions")
            .canonicalize()
            .map_err(|_| Error::Disco)?
    {
        return Err(Error::Datos);
    }
    let dir = game.join("mods");
    let mut manifest = registros(&game)?;
    for item in &ticket.mods {
        if dir.join(&item.archivo).exists()
            || dir.join(format!("{}.disabled", item.archivo)).exists()
            || manifest.iter().any(|m| {
                m.archivo == item.archivo
                    || (!item.project_id.is_empty() && m.project_id == item.project_id)
            })
        {
            return Err(Error::Archivo(item.archivo.clone()));
        }
        regular(&ruta_mod(&ticket.carpeta, item), false)?;
        if let Some(expected) = ticket.hashes.get(&item.archivo) {
            if hash_archivo(&ruta_mod(&ticket.carpeta, item))? != *expected {
                return Err(Error::Archivo(item.archivo.clone()));
            }
        }
    }
    let mut moved = vec![];
    for item in &ticket.mods {
        let src = ruta_mod(&ticket.carpeta, item);
        let dst = ruta_mod(&dir, item);
        if fs::rename(&src, &dst).is_err() {
            for (s, d) in moved.into_iter().rev() {
                let _ = fs::rename(d, s);
            }
            return Err(Error::Disco);
        }
        moved.push((src, dst));
        if !item.project_id.is_empty() {
            manifest.push(item.clone());
        }
    }
    if let Err(e) = guardar_registros(&game, &manifest) {
        for (s, d) in moved.into_iter().rev() {
            let _ = fs::rename(d, s);
        }
        return Err(e);
    }
    let _ = fs::remove_file(ticket.carpeta.join("recuperacion.json"));
    let _ = fs::remove_dir(&ticket.carpeta);
    let _ = fs::remove_dir(parent);
    Ok(())
}

/// Recuperación explícita de retiradas interrumpidas; nunca pisa archivos nuevos.
pub fn recuperar_operaciones(store: &Almacen, id: &str) -> Result<usize, Error> {
    let game = store.juego(id).map_err(|_| Error::Disco)?;
    let _lock = guardia(&game)?;
    recuperar(&game)?;
    recuperar_packs(&game, TipoPack::Recursos)?;
    recuperar_packs(&game, TipoPack::Shaders)?;
    let root = store
        .carpeta(id)
        .map_err(|_| Error::Disco)?
        .join(".icaro-transactions");
    if !root.exists() {
        return Ok(0);
    }
    regular(&root, true)?;
    let mut count = 0;
    for entry in fs::read_dir(&root).map_err(|_| Error::Disco)? {
        let stage = entry.map_err(|_| Error::Disco)?.path();
        regular(&stage, true)?;
        let journal = stage.join("recuperacion.json");
        if !journal.exists() {
            continue;
        }
        if regular(&journal, false)?.len() > 4 * 1024 * 1024 {
            return Err(Error::Datos);
        }
        #[derive(Deserialize)]
        struct Recuperacion {
            schema: u32,
            mods: Vec<Instalado>,
            hashes: BTreeMap<String, String>,
        }
        let data: Recuperacion =
            serde_json::from_slice(&fs::read(&journal).map_err(|_| Error::Disco)?)
                .map_err(|_| Error::Datos)?;
        if data.schema != 1 || data.mods.len() > 4096 {
            return Err(Error::Datos);
        }
        let dir = game.join("mods");
        regular(&dir, true)?;
        let mut records = registros(&game)?;
        for item in &data.mods {
            if !nombre_valido(&item.archivo) {
                return Err(Error::Datos);
            }
            let src = ruta_mod(&stage, item);
            let dst = ruta_mod(&dir, item);
            let candidate = if src.exists() {
                if dst.exists() {
                    return Err(Error::Archivo(item.archivo.clone()));
                }
                src
            } else {
                dst
            };
            regular(&candidate, false)?;
            if data.hashes.get(&item.archivo) != Some(&hash_archivo(&candidate)?) {
                return Err(Error::Archivo(item.archivo.clone()));
            }
        }
        // Si se interrumpe otra vez, el diario permite reanudar los mismos movimientos.
        for item in &data.mods {
            let src = ruta_mod(&stage, item);
            if src.exists() {
                fs::rename(src, ruta_mod(&dir, item)).map_err(|_| Error::Disco)?;
            }
            if !item.project_id.is_empty() && !records.iter().any(|m| m.archivo == item.archivo) {
                records.push(item.clone());
            }
        }
        guardar_registros(&game, &records)?;
        fs::remove_file(journal).map_err(|_| Error::Disco)?;
        fs::remove_dir(stage).map_err(|_| Error::Disco)?;
        count += 1;
    }
    let _ = fs::remove_dir(root);
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    pub(super) struct Prueba(pub(super) PathBuf);
    impl Prueba {
        pub(super) fn nueva() -> Self {
            let mut bytes = [0; 16];
            getrandom::fill(&mut bytes).unwrap();
            Self(std::env::temp_dir().join(format!("icaro-mods-test-{}", hash(&bytes))))
        }
        pub(super) fn instancia(&self) -> (Almacen, crate::instancias::Instancia) {
            let store = Almacen::nuevo(&self.0);
            let i = store
                .crear_con_loader("Mundo", "1.21.1", "fabric", 0)
                .unwrap();
            (store, i)
        }
    }
    impl Drop for Prueba {
        fn drop(&mut self) {
            if self.0.parent() == Some(std::env::temp_dir().as_path())
                && self
                    .0
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("icaro-mods-test-")
            {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
    }
    struct Fake {
        versions: Vec<Version>,
        bad: bool,
        downloads: AtomicUsize,
    }
    impl Fuente for Fake {
        async fn version(&self, id: &str) -> Result<Version, Error> {
            self.versions
                .iter()
                .find(|v| v.id == id)
                .cloned()
                .ok_or(Error::Datos)
        }
        async fn versiones(&self, project: &str, _: &str, _: &str) -> Result<Vec<Version>, Error> {
            Ok(self
                .versions
                .iter()
                .filter(|v| v.project_id == project)
                .cloned()
                .collect())
        }
        async fn nombre(&self, project: &str) -> Result<String, Error> {
            Ok(format!("Nombre {project}"))
        }
        async fn archivo(&self, file: &ModFile) -> Result<Vec<u8>, Error> {
            self.downloads.fetch_add(1, Ordering::Relaxed);
            Ok(if self.bad {
                b"bad".to_vec()
            } else {
                file.filename.as_bytes().to_vec()
            })
        }
    }
    fn version(project: &str, id: &str, dependencies: Vec<Dependency>) -> Version {
        let filename = format!("{project}-{id}.jar");
        serde_json::from_value(serde_json::json!({"id":format!("{project}{id}"),"project_id":project,"name":id,"version_number":id,"version_type":"release","game_versions":["1.21.1"],"loaders":["fabric"],"files":[{"filename":filename,"url":format!("https://cdn.modrinth.com/data/{project}/versions/{id}/{filename}"),"size":filename.len(),"primary":true,"hashes":{"sha512":hash(filename.as_bytes())}}],"dependencies":dependencies,"date_published":format!("2026-10-{id}T00:00:00Z")})).unwrap()
    }
    fn required(project: &str, id: Option<&str>) -> Dependency {
        Dependency {
            project_id: Some(project.into()),
            version_id: id.map(|id| format!("{project}{id}")),
            dependency_type: "required".into(),
            file_name: None,
        }
    }
    fn fake(versions: Vec<Version>) -> Fake {
        Fake {
            versions,
            bad: false,
            downloads: AtomicUsize::new(0),
        }
    }
    struct RecursoFake {
        version: Version,
        bytes: Vec<u8>,
        corrupto: bool,
    }
    impl Fuente for RecursoFake {
        async fn version(&self, _: &str) -> Result<Version, Error> {
            Ok(self.version.clone())
        }
        async fn versiones(&self, _: &str, _: &str, _: &str) -> Result<Vec<Version>, Error> {
            Ok(vec![self.version.clone()])
        }
        async fn nombre(&self, _: &str) -> Result<String, Error> {
            Ok("Fresh Animations".into())
        }
        async fn archivo(&self, _: &ModFile) -> Result<Vec<u8>, Error> {
            Ok(if self.corrupto {
                b"incorrecto".to_vec()
            } else {
                self.bytes.clone()
            })
        }
    }
    fn recurso_fake() -> RecursoFake {
        let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        archive
            .start_file("pack.mcmeta", zip::write::SimpleFileOptions::default())
            .unwrap();
        archive
            .write_all(br#"{"pack":{"pack_format":46,"description":"Prueba"}}"#)
            .unwrap();
        let bytes = archive.finish().unwrap().into_inner();
        let mut v = version("resource", "01", vec![]);
        v.loaders = vec!["minecraft".into()];
        v.game_versions = vec!["26.3".into()];
        v.files[0].filename = "fresh-animations.zip".into();
        v.files[0].size = bytes.len() as u64;
        v.files[0].hashes.sha512 = hash(&bytes);
        RecursoFake {
            version: v,
            bytes,
            corrupto: false,
        }
    }
    #[tokio::test]
    async fn resourcepack_zip_no_exige_loader_y_permite_otro_minecraft() {
        let p = Prueba::nueva();
        let (store, i) = p.instancia();
        let source = recurso_fake();
        assert!(recurso_compatible(&source.version, "26.3"));
        assert!(!recurso_compatible(&source.version, "1.21.1"));
        let list = instalar_recursos(&source, &store, &i.id, vec![source.version.clone()], |_| {})
            .await
            .unwrap();
        let game = store.juego(&i.id).unwrap();
        assert_eq!(
            fs::read(game.join("resourcepacks/fresh-animations.zip")).unwrap(),
            source.bytes
        );
        assert!(!game.join("mods/fresh-animations.zip").exists());
        assert!(!game.join("resourcepacks/pack.mcmeta").exists());
        assert_eq!(list[0].nombre, "Fresh Animations");
        assert_eq!(inventario_recursos(&store, &i.id).unwrap().len(), 1);
        fs::write(game.join("resourcepacks/local.zip"), &source.bytes).unwrap();
        assert_eq!(inventario_recursos(&store, &i.id).unwrap().len(), 2);
        fs::write(
            game.join("options.txt"),
            "lang:es_es\nresourcePacks:[\"vanilla\",\"file/local.zip\"]\ngamma:0.8\n",
        )
        .unwrap();
        alternar_recurso(&store, &i.id, "fresh-animations.zip").unwrap();
        assert_eq!(
            recursos_activos(&store, &i.id).unwrap(),
            ["vanilla", "file/local.zip", "file/fresh-animations.zip"]
        );
        let options = fs::read_to_string(game.join("options.txt")).unwrap();
        assert!(options.contains("lang:es_es\n") && options.contains("gamma:0.8\n"));
        alternar_recurso(&store, &i.id, "fresh-animations.zip").unwrap();
        assert_eq!(
            recursos_activos(&store, &i.id).unwrap(),
            ["vanilla", "file/local.zip"]
        );
        assert!(matches!(
            alternar_recurso(&store, &i.id, "../options.txt"),
            Err(Error::Datos)
        ));
    }
    #[tokio::test]
    async fn resourcepack_hash_incorrecto_conserva_zip_anterior() {
        let p = Prueba::nueva();
        let (store, i) = p.instancia();
        let mut source = recurso_fake();
        instalar_recursos(&source, &store, &i.id, vec![source.version.clone()], |_| {})
            .await
            .unwrap();
        source.corrupto = true;
        source.version.id = "nueva".into();
        let result =
            instalar_recursos(&source, &store, &i.id, vec![source.version.clone()], |_| {}).await;
        assert!(matches!(result, Err(Error::Hash)));
        assert_eq!(
            fs::read(
                store
                    .juego(&i.id)
                    .unwrap()
                    .join("resourcepacks/fresh-animations.zip")
            )
            .unwrap(),
            source.bytes
        );
        assert_eq!(
            inventario_recursos(&store, &i.id).unwrap()[0].version_id,
            "resource01"
        );
    }
    #[tokio::test]
    async fn shaders_zip_iris_activation_preserves_configuration_and_rejects_resourcepack() {
        let p = Prueba::nueva();
        let (store, i) = p.instancia();
        let mut source = recurso_fake();
        let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        archive
            .start_file(
                "shaders/shaders.properties",
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        archive.write_all(b"clouds=off").unwrap();
        source.bytes = archive.finish().unwrap().into_inner();
        source.version.files[0].size = source.bytes.len() as u64;
        source.version.files[0].hashes.sha512 = hash(&source.bytes);
        source.version.files[0].filename = "shader.zip".into();
        instalar_packs(
            &source,
            &store,
            &i.id,
            vec![source.version.clone()],
            TipoPack::Shaders,
            |_| {},
        )
        .await
        .unwrap();
        let game = store.juego(&i.id).unwrap();
        let files = BTreeSet::from(["shader.zip".into()]);
        assert!(!game.join("resourcepacks/shader.zip").exists());
        assert!(matches!(
            activar_packs(&store, &i.id, TipoPack::Shaders, &files, true),
            Err(Error::Compatible(_))
        ));
        fs::create_dir_all(game.join("config")).unwrap();
        fs::write(
            game.join("config/iris.properties"),
            "# Iris\nmaxShadowRenderDistance=32\nenableShaders=false\n",
        )
        .unwrap();
        activar_packs(&store, &i.id, TipoPack::Shaders, &files, true).unwrap();
        assert!(inventario_packs(&store, &i.id, TipoPack::Shaders).unwrap()[0].activo);
        assert!(fs::read_to_string(game.join("config/iris.properties"))
            .unwrap()
            .contains("maxShadowRenderDistance=32"));
        source.version.id = "shader02".into();
        source.version.files[0].filename = "shader-new.zip".into();
        instalar_packs(
            &source,
            &store,
            &i.id,
            vec![source.version.clone()],
            TipoPack::Shaders,
            |_| {},
        )
        .await
        .unwrap();
        assert_eq!(
            packs_activos(&store, &i.id, TipoPack::Shaders).unwrap(),
            ["shader-new.zip"]
        );
        quitar_packs(
            &store,
            &i.id,
            TipoPack::Shaders,
            &BTreeSet::from(["shader-new.zip".into()]),
        )
        .unwrap();
        assert!(inventario_packs(&store, &i.id, TipoPack::Shaders)
            .unwrap()
            .is_empty());
        assert!(packs_activos(&store, &i.id, TipoPack::Shaders)
            .unwrap()
            .is_empty());
        let invalid = recurso_fake();
        assert!(matches!(
            instalar_packs(
                &invalid,
                &store,
                &i.id,
                vec![invalid.version.clone()],
                TipoPack::Shaders,
                |_| {}
            )
            .await,
            Err(Error::Datos)
        ));
    }
    #[tokio::test]
    async fn resourcepacks_bulk_activation_removal_preserves_unselected_and_renamed_update() {
        let p = Prueba::nueva();
        let (store, i) = p.instancia();
        let mut source = recurso_fake();
        instalar_recursos(&source, &store, &i.id, vec![source.version.clone()], |_| {})
            .await
            .unwrap();
        let game = store.juego(&i.id).unwrap();
        fs::write(game.join("resourcepacks/local.zip"), &source.bytes).unwrap();
        fs::write(
            game.join("options.txt"),
            "lang:es_es\nresourcePacks:[\"vanilla\"]\n",
        )
        .unwrap();
        let files = BTreeSet::from(["fresh-animations.zip".into(), "local.zip".into()]);
        activar_packs(&store, &i.id, TipoPack::Recursos, &files, true).unwrap();
        assert_eq!(
            inventario_recursos(&store, &i.id)
                .unwrap()
                .iter()
                .filter(|m| m.activo)
                .count(),
            2
        );
        source.version.id = "resource02".into();
        source.version.files[0].filename = "fresh-new.zip".into();
        instalar_recursos(&source, &store, &i.id, vec![source.version.clone()], |_| {})
            .await
            .unwrap();
        assert_eq!(
            recursos_activos(&store, &i.id).unwrap(),
            ["vanilla", "file/fresh-new.zip", "file/local.zip"]
        );
        let options_before = fs::read(game.join("options.txt")).unwrap();
        fs::write(game.join("options.txt"), "resourcePacks:INVALID\n").unwrap();
        source.version.id = "resource03".into();
        source.version.files[0].filename = "fresh-latest.zip".into();
        assert!(matches!(
            instalar_recursos(&source, &store, &i.id, vec![source.version.clone()], |_| {}).await,
            Err(Error::Datos)
        ));
        assert!(game.join("resourcepacks/fresh-new.zip").exists());
        assert!(!game.join("resourcepacks/fresh-latest.zip").exists());
        let manifest: Vec<Instalado> = serde_json::from_slice(
            &fs::read(game.join("resourcepacks/.icaro-resourcepacks.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(manifest[0].version_id, "resource02");
        assert!(matches!(
            quitar_packs(
                &store,
                &i.id,
                TipoPack::Recursos,
                &BTreeSet::from(["fresh-new.zip".into()])
            ),
            Err(Error::Datos)
        ));
        assert!(game.join("resourcepacks/fresh-new.zip").exists());
        fs::write(game.join("options.txt"), options_before).unwrap();
        quitar_packs(
            &store,
            &i.id,
            TipoPack::Recursos,
            &BTreeSet::from(["fresh-new.zip".into()]),
        )
        .unwrap();
        assert_eq!(
            inventario_recursos(&store, &i.id).unwrap()[0].archivo,
            "local.zip"
        );
        assert_eq!(
            recursos_activos(&store, &i.id).unwrap(),
            ["vanilla", "file/local.zip"]
        );
        assert!(fs::read_to_string(game.join("options.txt"))
            .unwrap()
            .contains("lang:es_es"));
        assert!(matches!(
            quitar_packs(
                &store,
                &i.id,
                TipoPack::Recursos,
                &BTreeSet::from(["../options.txt".into()])
            ),
            Err(Error::Datos)
        ));
    }
    #[test]
    fn recuperacion_interrumpida_conserva_colisiones_y_reanuda() {
        let p = Prueba::nueva();
        let (store, i) = p.instancia();
        let game = store.juego(&i.id).unwrap();
        let dir = game.join("mods");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("manual.jar"), b"original").unwrap();
        let list = inventario(&store, &i.id).unwrap();
        let item = list
            .iter()
            .find(|m| m.archivo == "manual.jar")
            .unwrap()
            .clone();
        let hash = hash_archivo(&dir.join("manual.jar")).unwrap();
        let stage = store
            .carpeta(&i.id)
            .unwrap()
            .join(".icaro-transactions/interrumpida");
        fs::create_dir_all(&stage).unwrap();
        fs::rename(dir.join("manual.jar"), stage.join("manual.jar")).unwrap();
        crate::almacenamiento::escribir_json(
            &stage.join("recuperacion.json"),
            &serde_json::json!({"schema":1,"mods":[item],"hashes":{"manual.jar":hash}}),
        )
        .unwrap();
        fs::write(dir.join("manual.jar"), b"nuevo").unwrap();
        assert!(recuperar_operaciones(&store, &i.id).is_err());
        assert_eq!(fs::read(stage.join("manual.jar")).unwrap(), b"original");
        fs::remove_file(dir.join("manual.jar")).unwrap();
        assert_eq!(recuperar_operaciones(&store, &i.id).unwrap(), 1);
        assert_eq!(fs::read(dir.join("manual.jar")).unwrap(), b"original");
        assert!(!stage.exists());
    }

    #[tokio::test]
    async fn retirar_y_deshacer_preserva_mods_manuales_y_no_pisa_archivos_nuevos() {
        let p = Prueba::nueva();
        let (store, i) = p.instancia();
        let v = version("mod", "01", vec![]);
        let source = fake(vec![v.clone()]);
        instalar(&source, &store, &i.id, vec![v], "1.21.1", "fabric", |_| {})
            .await
            .unwrap();
        let game = store.juego(&i.id).unwrap();
        let dir = game.join("mods");
        fs::write(dir.join("manual.jar"), b"manual").unwrap();
        let before = inventario(&store, &i.id).unwrap();
        let files = before.iter().map(|m| m.archivo.clone()).collect();
        let ticket = retirar(&store, &i.id, &files).unwrap();
        assert!(inventario(&store, &i.id).unwrap().is_empty());
        fs::write(dir.join("manual.jar"), b"nuevo").unwrap();
        assert!(restaurar(&store, &i.id, &ticket).is_err());
        assert_eq!(fs::read(dir.join("manual.jar")).unwrap(), b"nuevo");
        assert!(!dir.join("mod-01.jar").exists());
        fs::rename(dir.join("manual.jar"), dir.join("nuevo.jar")).unwrap();
        restaurar(&store, &i.id, &ticket).unwrap();
        let restored = inventario(&store, &i.id).unwrap();
        assert_eq!(restored.len(), 3);
        assert!(restored.iter().any(|m| m.project_id == "mod"));
        assert_eq!(fs::read(dir.join("manual.jar")).unwrap(), b"manual");
    }
    #[tokio::test]
    async fn required_dependencies_install_once_and_disabled_requirements_become_active() {
        let p = Prueba::nueva();
        let (store, i) = p.instancia();
        let dep = version("dep", "01", vec![]);
        let root = version(
            "root",
            "01",
            vec![
                required("dep", None),
                required("dep", Some("01")),
                Dependency {
                    dependency_type: "optional".into(),
                    ..required("optional", None)
                },
            ],
        );
        let source = fake(vec![dep.clone(), root.clone()]);
        instalar(
            &source,
            &store,
            &i.id,
            vec![dep],
            "1.21.1",
            "fabric",
            |_| {},
        )
        .await
        .unwrap();
        alternar(&store, &i.id, "dep-01.jar").unwrap();
        let game = store.juego(&i.id).unwrap();
        fs::write(game.join("mods/manual.jar"), b"local").unwrap();
        instalar(
            &source,
            &store,
            &i.id,
            vec![root],
            "1.21.1",
            "fabric",
            |_| {},
        )
        .await
        .unwrap();
        let reopened = inventario(&Almacen::nuevo(&p.0), &i.id).unwrap();
        assert_eq!(source.downloads.load(Ordering::Relaxed), 2);
        assert_eq!(reopened.len(), 3);
        assert!(
            reopened
                .iter()
                .find(|m| m.project_id == "dep")
                .unwrap()
                .activo
        );
        assert_eq!(
            reopened
                .iter()
                .find(|m| m.project_id == "root")
                .unwrap()
                .nombre,
            "Nombre root"
        );
        assert_eq!(fs::read(game.join("mods/manual.jar")).unwrap(), b"local");
        assert!(!game.join("mods/optional-01.jar").exists());
    }
    #[tokio::test]
    async fn failed_update_keeps_the_entire_old_set_and_modified_files_are_preserved() {
        let p = Prueba::nueva();
        let (store, i) = p.instancia();
        let old = version("root", "01", vec![]);
        let new = version("root", "02", vec![required("dep", None)]);
        let dep = version("dep", "01", vec![]);
        let mut source = fake(vec![old.clone(), new.clone(), dep]);
        instalar(
            &source,
            &store,
            &i.id,
            vec![old],
            "1.21.1",
            "fabric",
            |_| {},
        )
        .await
        .unwrap();
        let dir = store.juego(&i.id).unwrap().join("mods");
        let metadata = fs::read(dir.join(META)).unwrap();
        source.bad = true;
        assert!(matches!(
            instalar(
                &source,
                &store,
                &i.id,
                vec![new.clone()],
                "1.21.1",
                "fabric",
                |_| {}
            )
            .await,
            Err(Error::Hash)
        ));
        assert_eq!(fs::read(dir.join(META)).unwrap(), metadata);
        assert_eq!(fs::read(dir.join("root-01.jar")).unwrap(), b"root-01.jar");
        assert!(!dir.join("root-02.jar").exists());
        source.bad = false;
        fs::write(dir.join("root-01.jar"), b"user change").unwrap();
        assert!(matches!(
            instalar(
                &source,
                &store,
                &i.id,
                vec![new],
                "1.21.1",
                "fabric",
                |_| {}
            )
            .await,
            Err(Error::Archivo(_))
        ));
        assert_eq!(fs::read(dir.join("root-01.jar")).unwrap(), b"user change");
    }
    #[tokio::test]
    async fn incompatible_versions_conflicting_pins_cycles_and_unsafe_names() {
        let mut wrong = version("root", "01", vec![]);
        wrong.loaders = vec!["forge".into()];
        let source = fake(vec![]);
        assert!(matches!(
            resolver(&source, vec![wrong], &[], "1.21.1", "fabric").await,
            Err(Error::Compatible(_))
        ));
        let a = version("a", "01", vec![required("b", Some("01"))]);
        let b = version("b", "01", vec![required("a", Some("01"))]);
        // Version IDs are globally unique in Modrinth.
        let mut a = a;
        a.id = "a01".into();
        a.dependencies[0].version_id = Some("b01".into());
        let mut b = b;
        b.id = "b01".into();
        b.dependencies[0].version_id = Some("a01".into());
        let mut c = version("c", "01", vec![required("b", Some("02"))]);
        c.id = "c01".into();
        let mut b2 = version("b", "02", vec![]);
        b2.id = "b02".into();
        let source = fake(vec![a.clone(), b, c.clone(), b2]);
        assert_eq!(
            resolver(&source, vec![a.clone()], &[], "1.21.1", "fabric")
                .await
                .unwrap()
                .len(),
            2
        );
        assert!(matches!(
            resolver(&source, vec![a, c], &[], "1.21.1", "fabric").await,
            Err(Error::Conflicto(_))
        ));
        for s in [
            "../mod.jar",
            "..\\mod.jar",
            "CON.jar",
            "mods:secret.jar",
            "mod.jar ",
            ".hidden.jar",
        ] {
            assert!(!nombre_valido(s));
        }
        let p = Prueba::nueva();
        let (store, i) = p.instancia();
        let mut bad = version("x", "01", vec![]);
        bad.files[0].filename = "../outside.jar".into();
        assert!(matches!(
            instalar(
                &source,
                &store,
                &i.id,
                vec![bad],
                "1.21.1",
                "fabric",
                |_| {}
            )
            .await,
            Err(Error::Datos)
        ));
        assert!(!store.juego(&i.id).unwrap().join("mods").exists());
    }
    #[tokio::test]
    async fn updating_a_library_respects_a_version_only_pin_of_an_installed_mod() {
        let p = Prueba::nueva();
        let (store, i) = p.instancia();
        let dep = version("dep", "01", vec![]);
        let mut pin = required("dep", Some("01"));
        pin.project_id = None;
        let parent = version("parent", "01", vec![pin]);
        let update = version("dep", "02", vec![]);
        let source = fake(vec![dep, parent.clone(), update.clone()]);
        instalar(
            &source,
            &store,
            &i.id,
            vec![parent],
            "1.21.1",
            "fabric",
            |_| {},
        )
        .await
        .unwrap();
        let installed = inventario(&store, &i.id).unwrap();
        assert!(matches!(
            resolver(&source, vec![update], &installed, "1.21.1", "fabric").await,
            Err(Error::Conflicto(_))
        ));
    }
    #[tokio::test]
    async fn newer_stable_updates_do_not_downgrade_newer_beta_and_lock_serializes_writes() {
        let p = Prueba::nueva();
        let (store, i) = p.instancia();
        let old = version("x", "01", vec![]);
        let mut newest = version("x", "03", vec![]);
        newest.version_type = "beta".into();
        let release = version("x", "02", vec![]);
        let source = fake(vec![old.clone(), newest.clone(), release]);
        instalar(
            &source,
            &store,
            &i.id,
            vec![old],
            "1.21.1",
            "fabric",
            |_| {},
        )
        .await
        .unwrap();
        assert_eq!(
            actualizaciones(
                &source,
                &inventario(&store, &i.id).unwrap(),
                "1.21.1",
                "fabric"
            )
            .await
            .unwrap()[0]
                .id,
            "x02"
        );
        instalar(
            &source,
            &store,
            &i.id,
            vec![newest],
            "1.21.1",
            "fabric",
            |_| {},
        )
        .await
        .unwrap();
        assert!(actualizaciones(
            &source,
            &inventario(&store, &i.id).unwrap(),
            "1.21.1",
            "fabric"
        )
        .await
        .unwrap()
        .is_empty());
        let _lock = guardia(&store.juego(&i.id).unwrap()).unwrap();
        assert!(matches!(
            alternar(&store, &i.id, "x-03.jar"),
            Err(Error::Ocupada)
        ));
    }
    #[tokio::test]
    #[ignore = "Consulta Modrinth y descarga mods a una instancia temporal aislada"]
    async fn modrinth_real_required_dependencies() {
        let p = Prueba::nueva();
        let (store, i) = p.instancia();
        let catalog = Catalog::new().unwrap();
        let root = ultima(
            catalog
                .versions("reeses-sodium-options", Some("1.21.1"), Some("fabric"))
                .await
                .unwrap()
                .into_iter()
                .filter(|v| compatible(v, "1.21.1", "fabric") && v.version_type == "release")
                .collect(),
        )
        .unwrap();
        assert!(root
            .dependencies
            .iter()
            .any(|d| d.dependency_type == "required"));
        let result = instalar(
            &catalog,
            &store,
            &i.id,
            vec![root],
            "1.21.1",
            "fabric",
            |_| {},
        )
        .await
        .unwrap();
        assert!(result.len() >= 2);
        assert!(result.iter().any(|m| m.nombre == "Sodium"));
        for m in result {
            comprobar_actual(&store.juego(&i.id).unwrap().join("mods"), &m).unwrap();
        }
    }
}
