// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Catálogo público: cliente independiente de las credenciales de Minecraft.

use crate::http;
pub use crate::http::Error;
use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use std::path::PathBuf;
use std::time::{Duration, Instant};

mod cache;
#[cfg(test)]
mod cache_tests;
pub(crate) mod presupuesto;

const API: &str = "https://api.modrinth.com/v2/";
const BODY_LIMIT: usize = 4 * 1024 * 1024;
pub const PAGE_SIZE: u32 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Order {
    #[default]
    Relevance,
    Downloads,
    Updated,
    Newest,
}
impl Order {
    fn as_str(self) -> &'static str {
        match self {
            Self::Relevance => "relevance",
            Self::Downloads => "downloads",
            Self::Updated => "updated",
            Self::Newest => "newest",
        }
    }
}
impl std::fmt::Display for Order {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Relevance => "Relevancia",
            Self::Downloads => "Más descargados",
            Self::Updated => "Actualizados recientemente",
            Self::Newest => "Más recientes",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Mod,
    Modpack,
    Resourcepack,
    Shader,
}
impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Mod => "mod",
            Self::Modpack => "modpack",
            Self::Resourcepack => "resourcepack",
            Self::Shader => "shader",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Search {
    pub query: String,
    pub kind: Kind,
    pub minecraft: Option<String>,
    pub loader: Option<String>,
    pub offset: u32,
    /// Resultados por página; la API admite hasta 100.
    pub limit: u32,
    pub order: Order,
}

fn search_url(search: &Search) -> Url {
    search_url_filters(search, &[], None)
}

fn search_url_filters(search: &Search, categories: &[String], environment: Option<&str>) -> Url {
    let mut facets = vec![vec![format!("project_type:{}", search.kind.as_str())]];
    if let Some(version) = &search.minecraft {
        facets.push(vec![format!("versions:{version}")]);
    }
    if let Some(loader) = &search.loader {
        facets.push(vec![format!("categories:{}", loader.to_lowercase())]);
    }
    if !categories.is_empty() {
        facets.push(
            categories
                .iter()
                .map(|c| format!("categories:{c}"))
                .collect(),
        );
    }
    if let Some(side) = environment {
        facets.push(vec![format!("environment:{side}")]);
    }
    let mut url = Url::parse(API)
        .expect("URL constante")
        .join("search")
        .expect("Ruta constante");
    url.query_pairs_mut().extend_pairs([
        ("query", search.query.as_str()),
        (
            "facets",
            &serde_json::to_string(&facets).expect("Facetas de texto"),
        ),
        ("offset", &search.offset.to_string()),
        ("limit", &search.limit.clamp(1, 100).to_string()),
        ("index", search.order.as_str()),
    ]);
    url
}

#[derive(Debug, Clone, Deserialize)]
pub struct Results {
    pub hits: Vec<Hit>,
    pub offset: u32,
    pub limit: u32,
    pub total_hits: u32,
}
#[derive(Debug, Clone, Deserialize)]
pub struct Hit {
    pub project_id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub author: String,
    pub downloads: u64,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub versions: Vec<String>,
}
#[derive(Debug, Clone, Deserialize)]
pub struct Project {
    #[serde(default)]
    pub project_type: String,
    pub id: String,
    pub slug: String,
    pub title: String,
    pub description: String,
    pub body: String,
    pub license: License,
    pub downloads: u64,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub gallery: Vec<GalleryImage>,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub loaders: Vec<String>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub published: String,
    #[serde(default)]
    pub updated: String,
    #[serde(default)]
    pub followers: u64,
    #[serde(default)]
    pub client_side: String,
    #[serde(default)]
    pub server_side: String,
    pub issues_url: Option<String>,
    pub source_url: Option<String>,
    pub wiki_url: Option<String>,
    pub discord_url: Option<String>,
    #[serde(default)]
    pub donation_urls: Option<Vec<Donation>>,
    pub organization: Option<String>,
}
#[derive(Debug, Clone, Deserialize)]
pub struct GalleryImage {
    pub url: String,
    #[serde(default)]
    pub raw_url: Option<String>,
    pub featured: bool,
    pub title: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub ordering: i64,
}
impl GalleryImage {
    /// El CDN puede publicar `url` como vista previa y `raw_url` como original.
    pub fn original_url(&self) -> &str {
        self.raw_url
            .as_deref()
            .filter(|url| supported_image(url))
            .unwrap_or(&self.url)
    }
}
#[derive(Debug, Clone, Deserialize)]
pub struct Donation {
    pub platform: String,
    pub url: String,
}
#[derive(Debug, Clone, Deserialize)]
pub struct Member {
    pub user: User,
    pub role: String,
    #[serde(default)]
    pub is_owner: bool,
}
#[derive(Debug, Clone, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub name: Option<String>,
    #[serde(default)]
    pub avatar_url: Option<String>,
}
#[derive(Debug, Clone, Deserialize)]
pub struct Organization {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub icon_url: Option<String>,
    #[serde(default)]
    pub members: Vec<Member>,
}
#[derive(Debug, Clone, Deserialize)]
pub struct RelatedProject {
    pub id: String,
    #[serde(alias="name")]
    pub title: String,
    pub downloads: u64,
    pub icon_url: Option<String>,
    #[serde(default)]
    pub project_type: String,
    #[serde(default)]
    pub project_types: Vec<String>,
}
#[derive(Debug, Clone, Deserialize)]
pub struct GameVersion {
    pub version: String,
    pub version_type: String,
}
#[derive(Debug, Clone, Deserialize)]
pub struct Loader {
    pub name: String,
    pub supported_project_types: Vec<String>,
}
#[derive(Debug, Clone)]
pub struct Tags {
    pub versions: Vec<GameVersion>,
    pub loaders: Vec<Loader>,
}

/// Imágenes públicas del catálogo y avatares que Modrinth publica desde GitHub.
/// El cliente no sigue redirecciones ni admite credenciales o destinos locales.
fn image_url(value: &str) -> Result<Url, Error> {
    let url = Url::parse(value).map_err(|_| Error::Response("la imagen"))?;
    let modrinth = url.host_str() == Some("cdn.modrinth.com")
        && url.query().is_none()
        && (url.path().starts_with("/data/") || url.path().starts_with("/user/"));
    let github_avatar = matches!(url.host_str(),
        Some("avatars.githubusercontent.com" | "avatars0.githubusercontent.com"
            | "avatars1.githubusercontent.com" | "avatars2.githubusercontent.com"
            | "avatars3.githubusercontent.com"))
        && url.path().strip_prefix("/u/").is_some_and(|id|
            !id.is_empty() && id.bytes().all(|c| c.is_ascii_digit()))
        && url.query().is_none_or(|query| query == "v=4");
    if url.scheme() != "https"
        || !(modrinth || github_avatar)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.fragment().is_some()
    {
        return Err(Error::Response("la imagen"));
    }
    Ok(url)
}
pub fn supported_image(value: &str) -> bool {
    image_url(value).is_ok()
}
#[derive(Debug, Clone, Deserialize)]
pub struct License {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub url: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub version_number: String,
    pub version_type: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
    pub files: Vec<File>,
    pub dependencies: Vec<Dependency>,
    pub changelog: Option<String>,
    #[serde(default)]
    pub date_published: String,
    #[serde(default)]
    pub downloads: u64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct File {
    pub filename: String,
    pub url: String,
    pub size: u64,
    pub primary: bool,
    pub hashes: Hashes,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hashes {
    pub sha512: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub dependency_type: String,
    pub file_name: Option<String>,
}

fn project_url(id: &str) -> Result<Url, Error> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    {
        return Err(Error::Response("Modrinth"));
    }
    Url::parse(API)
        .expect("URL constante")
        .join(&format!("project/{id}"))
        .map_err(|_| Error::Response("Modrinth"))
}

#[derive(Clone)]
pub struct Catalog {
    client: Client,
    cooldown: Arc<Mutex<Cooldown>>,
    metadata: Option<Arc<tokio::sync::Mutex<cache::Cache>>>,
    /// Un turno por recurso: evita pedir dos veces lo mismo sin bloquear
    /// recursos distintos ni los aciertos de caché durante la red.
    en_curso: Arc<Mutex<std::collections::BTreeMap<String, Arc<tokio::sync::Mutex<()>>>>>,
    /// Compartido por tienda, sincronización e instalación: todas consumen la misma cuota.
    presupuesto: Arc<Mutex<presupuesto::Presupuesto>>,
    api: String,
    metadata_path: Option<PathBuf>,
}
#[derive(Default)]
struct Cooldown {
    until: Option<Instant>,
    failures: u32,
}
impl Catalog {
    /// Comparte metadatos durante los reintentos del anfitrión, con límites de tamaño y edad.
    pub fn with_metadata_cache(mut self) -> Self {
        self.metadata = Some(Arc::new(tokio::sync::Mutex::new(cache::Cache::default())));
        self
    }
    /// Reutiliza respuestas vigentes entre aperturas; la caducidad no se reinicia.
    pub fn with_persistent_metadata_cache(mut self, path: PathBuf) -> Self {
        self.metadata = Some(Arc::new(tokio::sync::Mutex::new(cache::Cache::cargar(&path))));
        self.metadata_path = Some(path);
        self
    }
    pub async fn guardar_metadata_cache(&self) -> Result<(), Error> {
        let (Some(path), Some(cache)) = (&self.metadata_path, &self.metadata) else { return Ok(()); };
        let snapshot = cache.lock().await.snapshot();
        let path = path.clone();
        tokio::task::spawn_blocking(move || crate::almacenamiento::escribir_json(&path, &snapshot))
            .await.map_err(|_| Error::Response("la caché de Modrinth"))?
            .map_err(|_| Error::Response("la caché de Modrinth"))
    }
    fn version_url(&self, id: &str) -> Result<Url, Error> {
        let mut url = self.project_url(id)?;
        url.set_path(&format!("/v2/version/{id}"));
        Ok(url)
    }
    /// Consulta solo IDs ausentes de caché, en lotes acotados de 50 versiones.
    pub async fn versions_by_ids(&self, ids: &[String]) -> Result<std::collections::BTreeMap<String, Version>, Error> {
        if ids.len() > 512 { return Err(Error::Response("las versiones de Modrinth")); }
        let mut result = std::collections::BTreeMap::new();
        let mut missing = std::collections::BTreeSet::new();
        {
            let mut cache = if let Some(cache) = &self.metadata { Some(cache.lock().await) } else { None };
            for id in ids {
                let key = format!("GET {}", self.version_url(id)?);
                if let Some(v) = cache.as_mut().and_then(|c| c.get::<Version>(&key)).filter(|v| &v.id == id) {
                    result.insert(id.clone(), v);
                } else { missing.insert(id.clone()); }
            }
        }
        let missing: Vec<_> = missing.into_iter().collect();
        for batch in missing.chunks(50) {
            let mut url = Url::parse(&format!("{}versions", self.api)).map_err(|_| Error::Response("Modrinth"))?;
            url.query_pairs_mut().append_pair("ids", &serde_json::to_string(batch).map_err(|_| Error::Response("Modrinth"))?);
            let versions: Vec<Version> = self.json(self.client.get(url)).await?;
            let fetched: std::collections::BTreeMap<_, _> = versions.iter().map(|v| (v.id.clone(), v.clone())).collect();
            if versions.len() != batch.len() || fetched.len() != batch.len() || batch.iter().any(|id| !fetched.contains_key(id)) {
                return Err(Error::Response("las versiones de Modrinth"));
            }
            let mut cache = if let Some(cache) = &self.metadata { Some(cache.lock().await) } else { None };
            for (id, version) in fetched {
                if let Some(cache) = &mut cache {
                    cache.insert(format!("GET {}", self.version_url(&id)?), &version, cache::TTL)?;
                }
                result.insert(id, version);
            }
        }
        Ok(result)
    }
    /// Espera restante del proveedor; los aciertos de caché siguen disponibles.
    pub fn retry_after(&self) -> Duration {
        self.cooldown.lock().ok().and_then(|state| state.until)
            .and_then(|until| until.checked_duration_since(Instant::now())).unwrap_or_default()
    }
    fn project_url(&self, id: &str) -> Result<Url, Error> {
        let original = project_url(id)?;
        let mut url = Url::parse(&self.api).map_err(|_| Error::Response("Modrinth"))?;
        url.set_path(original.path());
        Ok(url)
    }
    async fn turno(&self, clave: &str) -> tokio::sync::OwnedMutexGuard<()> {
        let candado = {
            let mut mapa = self.en_curso.lock().unwrap_or_else(|e| e.into_inner());
            // Un turno que solo conserva el mapa ya no lo usa nadie.
            mapa.retain(|_, c| Arc::strong_count(c) > 1);
            mapa.entry(clave.to_owned()).or_default().clone()
        };
        candado.lock_owned().await
    }
    async fn metadata_json<T: serde::de::DeserializeOwned>(&self, url: Url) -> Result<T, Error> {
        let Some(cache) = &self.metadata else { return self.json(self.client.get(url)).await; };
        let key = format!("GET {url}");
        // Peticiones simultáneas del mismo recurso esperan la primera respuesta;
        // la caché solo se bloquea para leer y escribir, nunca durante la red.
        let _turno = self.turno(&key).await;
        if let Some(value) = cache.lock().await.get(&key) { return Ok(value); }
        let value: serde_json::Value = self.json(self.client.get(url)).await?;
        let decoded = serde_json::from_value(value.clone()).map_err(|_| Error::Response("Modrinth"))?;
        cache.lock().await.insert(key, &value, cache::TTL)?;
        Ok(decoded)
    }

    pub async fn creator_projects(&self, id: &str, organization: bool) -> Result<Vec<RelatedProject>, Error> {
        let mut url=self.project_url(id)?;
        url.set_path(&if organization {format!("/v3/organization/{id}/projects")}else{format!("/v2/user/{id}/projects")});
        let mut projects:Vec<RelatedProject>=self.json(self.client.get(url)).await?;
        projects.sort_by(|a,b|b.downloads.cmp(&a.downloads));
        Ok(projects)
    }
    pub async fn versions_from_hashes(&self, hashes: &[String]) -> Result<std::collections::BTreeMap<String, Version>, Error> {
        self.versions_hashes(hashes, None).await
    }
    pub async fn latest_from_hashes(&self, hashes: &[String], minecraft: &str, loader: &str) -> Result<std::collections::BTreeMap<String, Version>, Error> {
        self.versions_hashes(hashes, Some((minecraft, loader))).await
    }
    async fn versions_hashes(&self, hashes: &[String], target: Option<(&str, &str)>) -> Result<std::collections::BTreeMap<String, Version>, Error> {
        if hashes.len() > 512 || hashes.iter().any(|hash| hash.len() != 128 || !hash.bytes().all(|c| c.is_ascii_hexdigit())) {
            return Err(Error::Response("los hashes de Modrinth"));
        }
        if hashes.is_empty() { return Ok(Default::default()); }
        {
            if let Some(cache) = &self.metadata {
                let mut result = std::collections::BTreeMap::new();
                let mut missing = std::collections::BTreeSet::new();
                let key = |hash: &str| match target {
                    Some((mc,loader)) => format!("update:{mc}:{loader}:{hash}"),
                    None => format!("hash:{hash}"),
                };
                {
                    let mut cache = cache.lock().await;
                    for hash in hashes {
                        match cache.get::<Option<Version>>(&key(hash)) {
                            Some(Some(v)) => { result.insert(hash.clone(), v); }
                            Some(None) => {}
                            None => { missing.insert(hash.clone()); }
                        }
                    }
                }
                if !missing.is_empty() {
                    let missing: Vec<_> = missing.into_iter().collect();
                    let fetched = self.request_hashes(&missing, target).await?;
                    let mut cache = cache.lock().await;
                    for hash in missing {
                        let value = fetched.get(&hash);
                        let ttl = if target.is_some() { Duration::from_secs(300) }
                            else if value.is_some() { cache::TTL } else { cache::NEGATIVE_TTL };
                        cache.insert(key(&hash), &value, ttl)?;
                        if let Some(v)=value {cache.insert(format!("GET {}",self.version_url(&v.id)?),v,cache::TTL)?;}
                    }
                    result.extend(fetched);
                }
                return Ok(result);
            }
        }
        self.request_hashes(hashes, target).await
    }
    async fn request_hashes(&self, hashes: &[String], target: Option<(&str, &str)>) -> Result<std::collections::BTreeMap<String, Version>, Error> {
        let mut body = serde_json::json!({"hashes":hashes, "algorithm":"sha512"});
        let endpoint = if let Some((minecraft, loader)) = target {
            body["game_versions"] = serde_json::json!([minecraft]);
            body["loaders"] = serde_json::json!([loader]);
            body["version_types"] = serde_json::json!(["release"]);
            "version_files/update"
        } else { "version_files" };
        let result: std::collections::BTreeMap<String, Version> = self.json(self.client.post(format!("{}{endpoint}", self.api)).json(&body)).await?;
        if result.iter().any(|(hash, v)| !hashes.contains(hash) || (target.is_none() && !v.files.iter().any(|f| f.hashes.sha512.eq_ignore_ascii_case(hash)))) { return Err(Error::Response("los hashes de Modrinth")); }
        Ok(result)
    }
    pub async fn download_mod(&self, file: &File) -> Result<Vec<u8>, Error> {
        let url = image_url(&file.url)?;
        if !url.path().starts_with("/data/") {
            return Err(Error::Response("el archivo del mod"));
        }
        if file.size == 0 || file.size > 128 * 1024 * 1024 {
            return Err(Error::Response("el archivo del mod"));
        }
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|_| Error::Network("el archivo del mod"))?;
        if !response.status().is_success() {
            return Err(Error::Status {
                service: "el archivo del mod",
                status: response.status().as_u16(),
            });
        }
        http::body_bytes(response, "el archivo del mod", file.size as usize).await
    }
    /// Descarga a `destino` por bloques, sin cargar el JAR en memoria. Devuelve
    /// el SHA-512 calculado mientras llegaban los datos; el llamador lo compara.
    pub async fn download_mod_to(&self, file: &File, destino: &std::path::Path) -> Result<String, Error> {
        use sha2::{Digest, Sha512};
        use tokio::io::AsyncWriteExt;
        let url = image_url(&file.url)?;
        if !url.path().starts_with("/data/") || file.size == 0 || file.size > 128 * 1024 * 1024 {
            return Err(Error::Response("el archivo del mod"));
        }
        let mut response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|_| Error::Network("el archivo del mod"))?;
        if !response.status().is_success() {
            return Err(Error::Status { service: "el archivo del mod", status: response.status().as_u16() });
        }
        if response.content_length().is_some_and(|n| n != file.size) {
            return Err(Error::Response("el archivo del mod"));
        }
        let mut salida = tokio::fs::File::create(destino).await.map_err(|_| Error::Response("el archivo del mod"))?;
        let (mut hasher, mut total) = (Sha512::new(), 0u64);
        while let Some(bloque) = response.chunk().await.map_err(|_| Error::Network("el archivo del mod"))? {
            total = total.saturating_add(bloque.len() as u64);
            if total > file.size {
                return Err(Error::Response("el archivo del mod"));
            }
            hasher.update(&bloque);
            salida.write_all(&bloque).await.map_err(|_| Error::Response("el archivo del mod"))?;
        }
        salida.sync_all().await.map_err(|_| Error::Response("el archivo del mod"))?;
        if total != file.size {
            return Err(Error::Response("el archivo del mod"));
        }
        Ok(hasher.finalize().iter().map(|b| format!("{b:02x}")).collect())
    }
    pub fn new() -> Result<Self, Error> {
        Ok(Self {
            client: http::client()?,
            cooldown: Arc::new(Mutex::new(Cooldown::default())),
            metadata: None,
            en_curso: Arc::default(),
            presupuesto: Arc::new(Mutex::new(presupuesto::Presupuesto::nuevo(Instant::now()))),
            api: API.into(),
            metadata_path: None,
        })
    }
    async fn json<T: serde::de::DeserializeOwned>(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<T, Error> {
        let remaining = self.retry_after();
        if !remaining.is_zero() { return Err(Error::RateLimited(remaining.as_secs() + 1)); }
        // El turno se reserva con el candado tomado y se espera sin él.
        let espera = self.presupuesto.lock().map_err(|_| Error::Response("Modrinth"))?.reservar(Instant::now());
        if !espera.is_zero() { tokio::time::sleep(espera).await; }
        let result = http::json_con_cuota(request, "Modrinth", BODY_LIMIT).await;
        if let Ok((_, Some(cuota))) = &result {
            if let Ok(mut presupuesto) = self.presupuesto.lock() { presupuesto.observar(*cuota, Instant::now()); }
        }
        let result = result.map(|(valor, _)| valor);
        let mut cooldown = self.cooldown.lock().map_err(|_| Error::Response("Modrinth"))?;
        if let Err(Error::RateLimited(seconds)) = &result {
            let exponential = 60u64.saturating_mul(1u64 << cooldown.failures.min(4)).min(900);
            // El jitter solo alarga la espera propia; nunca acorta la que pide Modrinth.
            let wait = Duration::from_secs((*seconds).clamp(1, 86400)).max(presupuesto::con_jitter(Duration::from_secs(exponential)));
            let until = Instant::now() + wait;
            cooldown.until = Some(cooldown.until.map_or(until, |old| old.max(until)));
            cooldown.failures = cooldown.failures.saturating_add(1);
            return Err(Error::RateLimited(wait.as_secs()));
        } else if result.is_ok() && cooldown.until.is_none_or(|until| until <= Instant::now()) {
            cooldown.failures = 0;
            cooldown.until = None;
        }
        result
    }
    pub async fn search(&self, search: &Search) -> Result<Results, Error> {
        if search.query.len() > 4096 {
            return Err(Error::Response("Modrinth"));
        }
        self.json(self.client.get(search_url(search))).await
    }

    /// Filtros de TiendaMods; se aplican antes de paginar los resultados.
    pub async fn search_filtered(
        &self,
        search: &Search,
        categories: &[String],
        environment: Option<&str>,
    ) -> Result<Results, Error> {
        if search.query.len() > 4096 || categories.len() > 32 {
            return Err(Error::Response("Modrinth"));
        }
        self.json(
            self.client
                .get(search_url_filters(search, categories, environment)),
        )
        .await
    }
    pub async fn tags(&self) -> Result<Tags, Error> {
        let base = Url::parse(API).expect("URL constante");
        let versions = self
            .json(self.client.get(base.join("tag/game_version").unwrap()))
            .await?;
        let loaders = self
            .json(self.client.get(base.join("tag/loader").unwrap()))
            .await?;
        Ok(Tags { versions, loaders })
    }
    pub async fn image(&self, value: &str) -> Result<Vec<u8>, Error> {
        self.image_with_limit(value, 4 * 1024 * 1024).await
    }
    /// Imagen original abierta expresamente en el visor, fuera de la caché de miniaturas.
    pub async fn gallery_image(&self, value: &str) -> Result<Vec<u8>, Error> {
        self.image_with_limit(value, 16 * 1024 * 1024).await
    }
    async fn image_with_limit(&self, value: &str, limit: usize) -> Result<Vec<u8>, Error> {
        let response = self
            .client
            .get(image_url(value)?)
            .send()
            .await
            .map_err(|_| Error::Network("la imagen"))?;
        if !response.status().is_success() {
            return Err(Error::Status {
                service: "la imagen",
                status: response.status().as_u16(),
            });
        }
        http::body_bytes(response, "la imagen", limit).await
    }
    pub async fn project(&self, id: &str) -> Result<Project, Error> {
        let mut project: Project = self.metadata_json(self.project_url(id)?).await?;
        project.gallery.sort_by(|a, b| {
            a.ordering
                .cmp(&b.ordering)
                .then_with(|| a.title.cmp(&b.title))
        });
        Ok(project)
    }
    /// Reutiliza fichas vigentes y consulta hasta 50 proyectos por petición.
    pub async fn projects_by_ids(&self, ids: &[String]) -> Result<std::collections::BTreeMap<String, Project>, Error> {
        if ids.len()>512 {return Err(Error::Response("los proyectos de Modrinth"));}
        let mut result=std::collections::BTreeMap::new();let mut missing=std::collections::BTreeSet::new();
        {
            let mut cache=if let Some(c)=&self.metadata {Some(c.lock().await)} else {None};
            for id in ids {
                let key=format!("GET {}",self.project_url(id)?);
                if let Some(p)=cache.as_mut().and_then(|c|c.get::<Project>(&key)).filter(|p|p.id==*id) {result.insert(id.clone(),p);} else {missing.insert(id.clone());}
            }
        }
        let missing:Vec<_>=missing.into_iter().collect();
        for batch in missing.chunks(50) {
            let mut url=Url::parse(&format!("{}projects",self.api)).map_err(|_|Error::Response("Modrinth"))?;
            url.query_pairs_mut().append_pair("ids",&serde_json::to_string(batch).map_err(|_|Error::Response("Modrinth"))?);
            let values:Vec<serde_json::Value>=self.json(self.client.get(url)).await?;
            let mut fetched=std::collections::BTreeMap::new();
            for value in values {
                let p:Project=serde_json::from_value(value.clone()).map_err(|_|Error::Response("los proyectos de Modrinth"))?;
                if !batch.contains(&p.id)||fetched.insert(p.id.clone(),(p,value)).is_some() {return Err(Error::Response("los proyectos de Modrinth"));}
            }
            let mut cache=if let Some(c)=&self.metadata {Some(c.lock().await)} else {None};
            for (id,(p,value)) in fetched {
                if let Some(c)=&mut cache {c.insert(format!("GET {}",self.project_url(&id)?),&value,cache::TTL)?;}
                result.insert(id,p);
            }
        }
        Ok(result)
    }
    pub async fn members(&self, id: &str) -> Result<Vec<Member>, Error> {
        let mut url = self.project_url(id)?;
        url.set_path(&format!("{}/members", url.path()));
        let mut members: Vec<Member> = self.metadata_json(url).await?;
        // v2 identifica al propietario mediante el rol; otras respuestas incluyen is_owner.
        for member in &mut members {
            member.is_owner |= member.role.eq_ignore_ascii_case("owner");
        }
        members.sort_by_key(|m| !m.is_owner);
        Ok(members)
    }
    pub async fn version(&self, id: &str) -> Result<Version, Error> {
        self.metadata_json(self.version_url(id)?).await
    }
    pub async fn organization(&self, id: &str) -> Result<Organization, Error> {
        let mut url = self.project_url(id)?;
        // Las organizaciones se consultan en v3; la ficha v2 conserva su ID.
        url.set_path(&format!("/v3/organization/{id}"));
        self.json(self.client.get(url)).await
    }
    pub async fn versions(
        &self,
        id: &str,
        minecraft: Option<&str>,
        loader: Option<&str>,
    ) -> Result<Vec<Version>, Error> {
        let mut url = self.project_url(id)?;
        url.set_path(&format!("{}/version", url.path()));
        url.query_pairs_mut()
            .append_pair("include_changelog", "false");
        if let Some(mc) = minecraft {
            url.query_pairs_mut()
                .append_pair("game_versions", &serde_json::json!([mc]).to_string());
        }
        if let Some(loader) = loader {
            url.query_pairs_mut().append_pair(
                "loaders",
                &serde_json::json!([loader.to_lowercase()]).to_string(),
            );
        }
        let versions: Vec<Version> = self.metadata_json(url).await?;
        let mut versions: Vec<_> = versions
            .into_iter()
            .filter(|v| {
                minecraft.is_none_or(|mc| v.game_versions.iter().any(|x| x == mc))
                    && loader.is_none_or(|loader| {
                        v.loaders.iter().any(|x| x.eq_ignore_ascii_case(loader))
                    })
            })
            .collect();
        versions.sort_by(|a, b| b.date_published.cmp(&a.date_published));
        if let Some(cache)=&self.metadata {
            let mut cache=cache.lock().await;
            for v in &versions {cache.insert(format!("GET {}",self.version_url(&v.id)?),v,cache::TTL)?;}
        }
        Ok(versions)
    }
}

pub async fn open_link(value: &str) -> Result<(), Error> {
    let url = Url::parse(value).map_err(|_| Error::Response("el enlace"))?;
    if !matches!(url.scheme(), "https" | "http")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(Error::Response("el enlace"));
    }
    tokio::task::spawn_blocking(move || {
        webbrowser::open(url.as_str()).map_err(|_| Error::Network("el navegador"))
    })
    .await
    .map_err(|_| Error::Network("el navegador"))?
}

pub async fn open_project(id: &str) -> Result<(), Error> {
    project_url(id)?;
    let url = format!("https://modrinth.com/project/{id}");
    tokio::task::spawn_blocking(move || {
        webbrowser::open(&url).map_err(|_| Error::Network("el navegador"))
    })
    .await
    .map_err(|_| Error::Network("el navegador"))?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gallery_uses_the_raw_image_and_supports_responses_without_it() {
        let mut image: GalleryImage = serde_json::from_value(serde_json::json!({
            "url":"https://cdn.modrinth.com/data/pack/images/preview_350.webp",
            "raw_url":"https://cdn.modrinth.com/data/pack/images/original.png",
            "featured":false
        }))
        .unwrap();
        assert_eq!(image.original_url(), image.raw_url.as_deref().unwrap());
        for raw in [
            None,
            Some(""),
            Some("https://localhost/private.png"),
            Some("https://foreign.example/image.png"),
            Some("file:///C:/secret.png"),
        ] {
            image.raw_url = raw.map(str::to_owned);
            assert_eq!(image.original_url(), image.url);
        }
        let legacy: GalleryImage = serde_json::from_value(serde_json::json!({
            "url":"https://cdn.modrinth.com/data/pack/images/legacy.png", "featured":false
        }))
        .unwrap();
        assert_eq!(legacy.original_url(), legacy.url);
    }
    #[test]
    fn project_media_cannot_fetch_local_or_foreign_resources() {
        assert!(supported_image(
            "https://cdn.modrinth.com/user/creator/avatar.png"
        ));
        assert!(supported_image(
            "https://cdn.modrinth.com/data/abc/icon.png"
        ));
        for url in [
            "http://cdn.modrinth.com/data/icon.png",
            "https://localhost/data/icon.png",
            "https://127.0.0.1/data/icon.png",
            "https://cdn.modrinth.com.attacker.example/data/icon.png",
            "https://cdn.modrinth.com@attacker.example/data/icon.png",
            "https://name:secret@cdn.modrinth.com/data/icon.png",
            "https://cdn.modrinth.com:8443/data/icon.png",
            "https://cdn.modrinth.com/data/icon.png?token=PRIVATE",
            "file:///C:/secret.png",
        ] {
            assert!(!supported_image(url));
        }
    }
    #[test]
    fn creator_avatars_accept_modrinth_and_exact_github_endpoints() {
        for url in [
            "https://avatars3.githubusercontent.com/u/31803019?v=4",
            "https://avatars.githubusercontent.com/u/17120022?v=4",
            "https://avatars.githubusercontent.com/u/17120022",
        ] {
            assert!(supported_image(url), "El avatar publicado por Modrinth debe cargar: {url}");
        }
        for url in [
            "https://avatars.githubusercontent.com.attacker.example/u/17120022?v=4",
            "https://avatars.githubusercontent.com/u/17120022?token=PRIVATE",
            "https://avatars.githubusercontent.com/u/17120022?v=4&redirect=http://localhost",
            "https://avatars.githubusercontent.com/private.png?v=4",
            "https://avatars.githubusercontent.com/u/not-a-user?v=4",
            "http://avatars.githubusercontent.com/u/17120022?v=4",
            "https://avatars.githubusercontent.com:8443/u/17120022?v=4",
        ] {
            assert!(!supported_image(url), "No es un endpoint de avatar permitido: {url}");
        }
        let organization: Organization = serde_json::from_value(serde_json::json!({
            "id":"LjcZDkRW", "name":"CaffeineMC",
            "icon_url":"https://cdn.modrinth.com/data/LjcZDkRW/icon.webp"
        })).unwrap();
        assert!(supported_image(organization.icon_url.as_deref().unwrap()));
        assert!(serde_json::from_value::<Organization>(serde_json::json!({
            "id":"old", "name":"Sin imagen"
        })).unwrap().icon_url.is_none());
    }
    #[test]
    fn shaders_search_uses_own_project_type_and_no_loader() {
        let url = search_url(&Search {
            query: "complementary".into(), kind: Kind::Shader,
            minecraft: Some("1.21.4".into()), loader: None,
            offset: 0, limit: PAGE_SIZE, order: Order::Relevance,
        });
        let query: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        let facets: Vec<Vec<String>> = serde_json::from_str(&query["facets"]).unwrap();
        assert!(facets.contains(&vec!["project_type:shader".into()]));
        assert!(facets.contains(&vec!["versions:1.21.4".into()]));
        assert!(!facets.iter().flatten().any(|f| f.contains("fabric") || f.contains("iris")));
    }
    #[tokio::test]
    async fn cooldown_blocks_queries_before_a_network_request() {
        let catalog = Catalog::new().unwrap();
        catalog.cooldown.lock().unwrap().until = Some(Instant::now() + Duration::from_secs(30));
        let search = Search {
            query: "sodium".into(),
            kind: Kind::Mod,
            minecraft: None,
            loader: None,
            offset: 0,
            limit: PAGE_SIZE,
            order: Order::Relevance,
        };
        assert!(
            matches!(catalog.clone().search(&search).await, Err(Error::RateLimited(seconds)) if (1..=31).contains(&seconds))
        );
    }
    #[test]
    fn facets_are_encoded_and_filters_are_conjoined() {
        let search = Search {
            query: "sodium & iris".into(),
            kind: Kind::Mod,
            minecraft: Some("1.21.4".into()),
            loader: Some("Fabric".into()),
            offset: 20,
            limit: PAGE_SIZE,
            order: Order::Downloads,
        };
        let url = search_url(&search);
        let pairs: std::collections::HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(pairs["query"], "sodium & iris");
        assert_eq!(pairs["offset"], "20");
        assert_eq!(pairs["index"], "downloads");
        let facets: serde_json::Value = serde_json::from_str(&pairs["facets"]).unwrap();
        assert_eq!(
            facets,
            serde_json::json!([
                ["project_type:mod"],
                ["versions:1.21.4"],
                ["categories:fabric"]
            ])
        );
        assert!(!url.as_str().contains("token"));
        let filtered = search_url_filters(
            &search,
            &["optimization".into(), "utility".into()],
            Some("client_only"),
        );
        let pairs: std::collections::HashMap<_, _> = filtered.query_pairs().into_owned().collect();
        let facets: serde_json::Value = serde_json::from_str(&pairs["facets"]).unwrap();
        assert_eq!(
            facets,
            serde_json::json!([
                ["project_type:mod"],
                ["versions:1.21.4"],
                ["categories:fabric"],
                ["categories:optimization", "categories:utility"],
                ["environment:client_only"]
            ])
        );
        assert_eq!(pairs["offset"], "20");
    }
    #[test]
    fn rejects_paths_hosts_and_query_injection_in_ids() {
        for id in [
            "",
            "../version",
            "https://attacker.example",
            "abc?token=secret",
            "abc#fragment",
            "a/b",
        ] {
            assert!(project_url(id).is_err());
        }
        assert_eq!(
            project_url("fabric-api").unwrap().host_str(),
            Some("api.modrinth.com")
        );
    }
}
