// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Actualizaciones publicadas: metadatos de GitHub, descarga acotada y SHA-256.
//! Admite el portable o el instalador declarado en latest.json de una release.
//!
//! Hay dos canales: las estables (`v<versión>-build.<n>`, publicadas desde
//! `main`) y las betas (`v<versión>-beta.<n>`, prereleases publicadas desde
//! `develop`). El canal estable nunca ofrece una beta.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::io::AsyncWriteExt;

const RELEASE: &str = "https://api.github.com/repos/00faithlss/Icaro-launcher/releases/latest";
/// Las releases más recientes, incluidas las prereleases del canal beta.
const RELEASES: &str = "https://api.github.com/repos/00faithlss/Icaro-launcher/releases?per_page=30";

/// Canal de actualizaciones elegido en Ajustes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Canal {
    #[default]
    Estable,
    /// Recibe también las betas de `develop`, además de las estables.
    Beta,
}
const MAX_ARCHIVO: u64 = 1024 * 1024 * 1024;
const PLATAFORMA: &str = "windows-x86_64";

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Nota {
    pub tipo: String,
    pub texto: String,
}
#[derive(Debug, Clone, Deserialize)]
struct Manifest {
    version: String,
    #[serde(default)]
    compilacion: u64,
    #[serde(default)]
    notas: Vec<Nota>,
    plataformas: BTreeMap<String, Artefacto>,
}
#[derive(Debug, Clone, Deserialize)]
struct Artefacto {
    archivo: String,
    sha256: String,
}
#[derive(Debug, Clone, Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}
#[derive(Debug, Clone, Deserialize)]
struct Asset {
    name: String,
    size: u64,
    browser_download_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Disponible {
    pub version: String,
    #[serde(default)]
    pub compilacion: u64,
    #[serde(default)]
    pub beta: bool,
    #[serde(default)]
    instalador: bool,
    pub notas: Vec<Nota>,
    pub bytes: u64,
    url: String,
    sha256: String,
}
#[derive(Debug, Clone)]
pub struct Preparada {
    pub version: String,
    pub compilacion: u64,
    archivo: PathBuf,
    sha256: String,
    bytes: u64,
    instalador: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resultado {
    esquema: u8,
    pub version: String,
    pub compilacion: u64,
    pub correcto: bool,
    #[serde(default)]
    pub detalle: Option<String>,
}

fn ruta_resultado(exe: &Path) -> Result<PathBuf, Error> {
    let exe = exe.canonicalize().map_err(|_| Error::Disco)?;
    let identidad = exe.to_string_lossy().to_lowercase();
    Ok(carpeta().join(format!("resultado-{:x}.json", Sha256::digest(identidad.as_bytes()))))
}

/// Resultado de esta instalación: vuelve a la misma pantalla al reabrir Ícaro.
pub fn tomar_resultado(version: &str, compilacion: u64) -> Option<Resultado> {
    let exe = std::env::current_exe().ok()?;
    let path = ruta_resultado(&exe).ok()?;
    leer_resultado(&path, version, compilacion).ok()
}
fn leer_resultado(path: &Path, version: &str, compilacion: u64) -> Result<Resultado, Error> {
    use std::io::Read;
    rechazar_enlaces(path)?;
    let mut bytes = vec![];
    std::fs::File::open(path).map_err(|_| Error::Disco)?.take(4097).read_to_end(&mut bytes).map_err(|_| Error::Disco)?;
    if bytes.len() > 4096 { return Err(Error::Datos); }
    let mut resultado: Resultado = serde_json::from_slice(&bytes).map_err(|_| Error::Datos)?;
    if resultado.esquema != 1 || numeros(&resultado.version).is_none() { return Err(Error::Datos); }
    resultado.correcto &= resultado.version == version && resultado.compilacion == compilacion;
    std::fs::remove_file(path).map_err(|_| Error::Disco)?;
    Ok(resultado)
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("No se pudo comprobar la actualización. Revisa tu conexión e inténtalo de nuevo.")]
    Red,
    #[error("La versión publicada aún no incluye una actualización para este equipo.")]
    SinArtefacto,
    #[error("La información de la actualización no es válida.")]
    Datos,
    #[error("La descarga no pasó la verificación. No se ha cambiado Ícaro.")]
    Integridad,
    #[error("No se pudo guardar o aplicar la actualización.")]
    Disco,
    #[error("La actualización automática todavía no está disponible para este sistema.")]
    Plataforma,
}
fn cliente() -> Result<reqwest::Client, Error> {
    reqwest::Client::builder()
        .https_only(true)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(600))
        .redirect(reqwest::redirect::Policy::limited(5))
        .user_agent(crate::http::USER_AGENT)
        .build()
        .map_err(|_| Error::Red)
}
fn numeros(v: &str) -> Option<[u64; 3]> {
    let partes: Vec<_> = v.strip_prefix('v').unwrap_or(v).split('.').collect();
    if partes.len() != 3 {
        return None;
    }
    Some([
        partes[0].parse().ok()?,
        partes[1].parse().ok()?,
        partes[2].parse().ok()?,
    ])
}
fn tag(version: &str, compilacion: u64, beta: bool) -> String {
    if beta {
        format!("v{version}-beta.{compilacion}")
    } else if compilacion == 0 {
        format!("v{version}")
    } else {
        format!("v{version}-build.{compilacion}")
    }
}

/// Versión, compilación y canal de un tag de release descargable.
fn leer_tag(tag: &str) -> Option<([u64; 3], u64, bool)> {
    let (version, resto) = tag.strip_prefix('v')?.split_once('-')?;
    let (beta, compilacion) = match resto.split_once('.')? {
        ("build", n) => (false, n),
        ("beta", n) => (true, n),
        _ => return None,
    };
    let compilacion: u64 = compilacion.parse().ok().filter(|n| *n > 0)?;
    Some((numeros(version)?, compilacion, beta))
}

/// La release más nueva que puede recibir el canal. Una beta solo se acepta
/// marcada como prerelease y una estable solo sin marcar.
fn elegir(releases: Vec<Release>, canal: Canal) -> Option<Release> {
    releases
        .into_iter()
        .filter(|r| !r.draft)
        .filter_map(|r| {
            let (version, compilacion, beta) = leer_tag(&r.tag_name)?;
            (beta == r.prerelease && (!beta || canal == Canal::Beta))
                .then_some(((version, compilacion), r))
        })
        .max_by_key(|(orden, _)| *orden)
        .map(|(_, r)| r)
}

/// Una instalación registrada solo se actualiza con su instalador completo.
pub fn instalada() -> bool {
    #[cfg(windows)]
    {
        let Ok(exe) = std::env::current_exe() else { return false; };
        let registry = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER);
        let Ok(key) = registry.open_subkey("Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{A758C875-3CA1-42F0-A1E8-85F1D58F7E42}_is1") else { return false; };
        let Ok(location) = key.get_value::<String, _>("InstallLocation") else { return false; };
        return matches!((exe.parent().and_then(|p| p.canonicalize().ok()), Path::new(&location).canonicalize().ok()), (Some(executable), Some(installed)) if executable == installed);
    }
    #[cfg(not(windows))]
    false
}
fn asset_valido(url: &str, tag: &str, nombre: &str) -> bool {
    if nombre.is_empty()
        || !nombre
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    {
        return false;
    }
    url == format!("https://github.com/00faithlss/Icaro-launcher/releases/download/{tag}/{nombre}")
}
#[cfg(test)]
fn resolver(
    release: Release,
    manifest: Manifest,
    actual: &str,
) -> Result<Option<Disponible>, Error> {
    resolver_compilacion(release, manifest, actual, 0, false, Canal::Estable)
}
fn resolver_compilacion(
    release: Release,
    manifest: Manifest,
    actual: &str,
    compilacion_actual: u64,
    instalador: bool,
    canal: Canal,
) -> Result<Option<Disponible>, Error> {
    let beta = release.prerelease;
    if release.draft || (beta && canal != Canal::Beta) {
        return Ok(None);
    }
    let actual = numeros(actual).ok_or(Error::Datos)?;
    let version = numeros(&manifest.version).ok_or(Error::Datos)?;
    if (beta && manifest.compilacion == 0)
        || release.tag_name != tag(&manifest.version, manifest.compilacion, beta)
    {
        return Err(Error::Datos);
    }
    if version < actual || (version == actual && manifest.compilacion <= compilacion_actual) {
        return Ok(None);
    }
    let artefacto = manifest
        .plataformas
        .get(if instalador { "windows-x86_64-setup" } else { PLATAFORMA })
        .ok_or(Error::SinArtefacto)?;
    if !artefacto.archivo.ends_with(".exe")
        || artefacto.sha256.len() != 64
        || !artefacto.sha256.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(Error::Datos);
    }
    let asset = release
        .assets
        .iter()
        .find(|a| a.name == artefacto.archivo)
        .ok_or(Error::SinArtefacto)?;
    if asset.size == 0
        || asset.size > MAX_ARCHIVO
        || !asset_valido(&asset.browser_download_url, &release.tag_name, &asset.name)
        || manifest.notas.len() > 200
        || manifest.notas.iter().any(|n| {
            n.texto.len() > 4096 || !matches!(n.tipo.as_str(), "nuevo" | "mejora" | "correccion")
        })
    {
        return Err(Error::Datos);
    }
    Ok(Some(Disponible {
        version: manifest.version.trim_start_matches('v').into(),
        compilacion: manifest.compilacion,
        beta,
        instalador,
        notas: manifest.notas,
        bytes: asset.size,
        url: asset.browser_download_url.clone(),
        sha256: artefacto.sha256.to_ascii_lowercase(),
    }))
}

/// Sin releases publicadas (404) es un resultado normal, no una actualización ficticia.
pub async fn comprobar(actual: &str) -> Result<Option<Disponible>, Error> {
    comprobar_compilacion(actual, 0).await
}
pub async fn comprobar_compilacion(actual: &str, compilacion_actual: u64) -> Result<Option<Disponible>, Error> {
    comprobar_canal(actual, compilacion_actual, Canal::Estable).await
}
/// El canal estable consulta la release marcada como más reciente; el beta
/// elige entre las últimas releases, incluidas las prereleases.
pub async fn comprobar_canal(
    actual: &str,
    compilacion_actual: u64,
    canal: Canal,
) -> Result<Option<Disponible>, Error> {
    let client = cliente()?;
    let respuesta = client
        .get(if canal == Canal::Beta { RELEASES } else { RELEASE })
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|_| Error::Red)?;
    if respuesta.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !respuesta.status().is_success() {
        return Err(Error::Red);
    }
    let release = if canal == Canal::Beta {
        let releases: Vec<Release> = crate::http::body_json(respuesta, "GitHub", 8 * 1024 * 1024)
            .await
            .map_err(|_| Error::Datos)?;
        match elegir(releases, canal) {
            Some(release) => release,
            None => return Ok(None),
        }
    } else {
        crate::http::body_json(respuesta, "GitHub", 2 * 1024 * 1024)
            .await
            .map_err(|_| Error::Datos)?
    };
    if release.draft || (release.prerelease && canal != Canal::Beta) {
        return Ok(None);
    }
    let asset = release
        .assets
        .iter()
        .find(|a| a.name == "latest.json")
        .ok_or(Error::SinArtefacto)?;
    if !asset_valido(&asset.browser_download_url, &release.tag_name, &asset.name)
        || asset.size > 1024 * 1024
    {
        return Err(Error::Datos);
    }
    let respuesta = client
        .get(&asset.browser_download_url)
        .send()
        .await
        .map_err(|_| Error::Red)?;
    if !respuesta.status().is_success() {
        return Err(Error::Red);
    }
    let manifest = crate::http::body_json(respuesta, "GitHub", 1024 * 1024)
        .await
        .map_err(|_| Error::Datos)?;
    resolver_compilacion(release, manifest, actual, compilacion_actual, instalada(), canal)
}

pub fn carpeta() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Icaro")
        .join("updates")
}

fn rechazar_enlaces(ruta: &Path) -> Result<(), Error> {
    for parte in ruta.ancestors() {
        match std::fs::symlink_metadata(parte) {
            Ok(m) => {
                #[cfg(windows)]
                {
                    use std::os::windows::fs::MetadataExt;
                    if m.file_attributes() & 0x400 != 0 {
                        return Err(Error::Datos);
                    }
                }
                if m.file_type().is_symlink() {
                    return Err(Error::Datos);
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(Error::Disco),
        }
    }
    Ok(())
}
fn staging() -> Result<PathBuf, Error> {
    #[cfg(windows)]
    if std::env::var_os("LOCALAPPDATA").is_none() {
        return Err(Error::Disco);
    }
    let raiz = carpeta();
    rechazar_enlaces(&raiz)?;
    std::fs::create_dir_all(&raiz).map_err(|_| Error::Disco)?;
    rechazar_enlaces(&raiz)?;
    for _ in 0..4 {
        let mut aleatorio = [0u8; 16];
        getrandom::fill(&mut aleatorio).map_err(|_| Error::Disco)?;
        let id: String = aleatorio.iter().map(|b| format!("{b:02x}")).collect();
        let ruta = raiz.join(id);
        match std::fs::create_dir(&ruta) {
            Ok(()) => {
                rechazar_enlaces(&ruta)?;
                return Ok(ruta);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(_) => return Err(Error::Disco),
        }
    }
    Err(Error::Disco)
}
// Reutiliza únicamente archivos completos que coincidan con el manifiesto actual.
// Los .part y los binarios modificados nunca se consideran preparados.
fn recuperar_descarga(raiz: &Path, disponible: &Disponible) -> Option<Preparada> {
    rechazar_enlaces(raiz).ok()?;
    let mut candidatos: Vec<_> = std::fs::read_dir(raiz)
        .ok()?
        .take(256)
        .filter_map(Result::ok)
        .filter(|e| {
            let nombre = e.file_name();
            let nombre = nombre.to_string_lossy();
            nombre.len() == 32 && nombre.bytes().all(|b| b.is_ascii_hexdigit())
        })
        .collect();
    candidatos
        .sort_by_key(|e| std::cmp::Reverse(e.metadata().ok().and_then(|m| m.modified().ok())));
    for candidato in candidatos.into_iter().take(64) {
        let archivo = candidato.path().join(format!("{}.exe", disponible.sha256));
        if rechazar_enlaces(&archivo).is_err() {
            continue;
        }
        let Ok(mut entrada) = std::fs::File::open(&archivo) else {
            continue;
        };
        if !entrada
            .metadata()
            .is_ok_and(|m| m.is_file() && m.len() == disponible.bytes)
        {
            continue;
        }
        let mut hash = Sha256::new();
        if std::io::copy(
            &mut std::io::Read::take(&mut entrada, disponible.bytes + 1),
            &mut hash,
        )
        .is_err()
            || format!("{:x}", hash.finalize()) != disponible.sha256
        {
            continue;
        }
        return Some(Preparada {
            version: disponible.version.clone(),
            compilacion: disponible.compilacion,
            archivo,
            sha256: disponible.sha256.clone(),
            bytes: disponible.bytes,
            instalador: disponible.instalador,
        });
    }
    None
}

/// Reutiliza una descarga íntegra o descarga a un temporal antes de preparar el reemplazo.
pub async fn descargar<F: Fn(u64, u64)>(d: Disponible, progreso: F) -> Result<Preparada, Error> {
    validar_descarga(&d)?;
    if !cfg!(all(windows, target_arch = "x86_64")) {
        return Err(Error::Plataforma);
    }
    let disponible = d.clone();
    if let Some(preparada) =
        tokio::task::spawn_blocking(move || recuperar_descarga(&carpeta(), &disponible))
            .await
            .map_err(|_| Error::Disco)?
    {
        progreso(d.bytes, d.bytes);
        return Ok(preparada);
    }
    let ruta = staging()?;
    let temporal = ruta.join(format!("{}.part", d.sha256));
    let destino = ruta.join(format!("{}.exe", d.sha256));
    let resultado = async {
        let mut respuesta = cliente()?
            .get(&d.url)
            .send()
            .await
            .map_err(|_| Error::Red)?;
        if !respuesta.status().is_success() {
            return Err(Error::Red);
        }
        if respuesta.content_length().is_some_and(|n| n != d.bytes) {
            return Err(Error::Integridad);
        }
        rechazar_enlaces(&temporal)?;
        let mut archivo = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporal)
            .await
            .map_err(|_| Error::Disco)?;
        let mut hash = Sha256::new();
        let mut recibidos = 0u64;
        while let Some(chunk) = respuesta.chunk().await.map_err(|_| Error::Red)? {
            recibidos += chunk.len() as u64;
            if recibidos > d.bytes || recibidos > MAX_ARCHIVO {
                return Err(Error::Integridad);
            }
            hash.update(&chunk);
            archivo.write_all(&chunk).await.map_err(|_| Error::Disco)?;
            progreso(recibidos, d.bytes);
        }
        if recibidos != d.bytes || format!("{:x}", hash.finalize()) != d.sha256 {
            return Err(Error::Integridad);
        }
        archivo.sync_all().await.map_err(|_| Error::Disco)?;
        drop(archivo);
        tokio::fs::rename(&temporal, &destino)
            .await
            .map_err(|_| Error::Disco)?;
        Ok(Preparada {
            version: d.version,
            compilacion: d.compilacion,
            archivo: destino,
            sha256: d.sha256,
            bytes: d.bytes,
            instalador: d.instalador,
        })
    }
    .await;
    if resultado.is_err() {
        let _ = tokio::fs::remove_file(&temporal).await;
    }
    resultado
}

fn validar_descarga(d: &Disponible) -> Result<(), Error> {
    let url = reqwest::Url::parse(&d.url).map_err(|_| Error::Datos)?;
    let partes: Vec<_> = url.path_segments().ok_or(Error::Datos)?.collect();
    if url.scheme() != "https"
        || url.host_str() != Some("github.com")
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || partes.len() != 6
        || partes[..4] != ["00faithlss", "Icaro-launcher", "releases", "download"]
        || partes[4] != tag(&d.version, d.compilacion, d.beta)
        || numeros(&d.version).is_none()
        || !partes[5].ends_with(".exe")
        || !asset_valido(&d.url, partes[4], partes[5])
        || d.bytes == 0
        || d.bytes > MAX_ARCHIVO
        || d.sha256.len() != 64
        || !d.sha256.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(Error::Datos);
    }
    Ok(())
}

/// Revalida el ejecutable preparado antes de delegar el reemplazo al salir del proceso.
pub fn aplicar(preparada: &Preparada, reiniciar: bool) -> Result<(), Error> {
    if !cfg!(windows) {
        return Err(Error::Plataforma);
    }
    rechazar_enlaces(&preparada.archivo)?;
    rechazar_enlaces(&carpeta())?;
    let origen = preparada.archivo.canonicalize().map_err(|_| Error::Disco)?;
    let raiz = carpeta().canonicalize().map_err(|_| Error::Disco)?;
    if !origen.starts_with(&raiz)
        || origen.parent().and_then(|p| p.parent()) != Some(raiz.as_path())
    {
        return Err(Error::Datos);
    }
    let esperado = &preparada.sha256;
    if esperado.len() != 64 || !esperado.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(Error::Datos);
    }
    let mut archivo = std::fs::File::open(&origen).map_err(|_| Error::Disco)?;
    if !archivo
        .metadata()
        .is_ok_and(|m| m.is_file() && m.len() == preparada.bytes)
    {
        return Err(Error::Integridad);
    }
    let mut hash = Sha256::new();
    std::io::copy(&mut archivo, &mut hash).map_err(|_| Error::Disco)?;
    if format!("{:x}", hash.finalize()) != *esperado {
        return Err(Error::Integridad);
    }
    let destino = std::env::current_exe().map_err(|_| Error::Disco)?;
    rechazar_enlaces(&destino)?;
    if preparada.instalador != instalada() { return Err(Error::Datos); }
    lanzar(&origen, &destino, esperado, reiniciar, preparada)
}

#[cfg(windows)]
fn lanzar(origen: &Path, destino: &Path, hash: &str, reiniciar: bool, preparada: &Preparada) -> Result<(), Error> {
    use std::io::Write;
    use std::os::windows::process::CommandExt;
    let mut aleatorio = [0u8; 16];
    getrandom::fill(&mut aleatorio).map_err(|_| Error::Disco)?;
    let id: String = aleatorio.iter().map(|b| format!("{b:02x}")).collect();
    let script = origen
        .parent()
        .ok_or(Error::Datos)?
        .join(format!("aplicar-{id}.ps1"));
    rechazar_enlaces(&script)?;
    let mut archivo = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&script)
        .map_err(|_| Error::Disco)?;
    archivo
        .write_all(SCRIPT.as_bytes())
        .map_err(|_| Error::Disco)?;
    archivo.sync_all().map_err(|_| Error::Disco)?;
    drop(archivo);
    // PowerShell e Inno Setup reciben rutas Win32, sin el prefijo de canonicalize.
    let mut cmd = std::process::Command::new("powershell.exe");
    cmd.args([
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
    ])
    .arg(ruta_ayudante(&script))
    .arg("-ProcessId")
    .arg(std::process::id().to_string())
    .arg("-Source")
    .arg(ruta_ayudante(origen))
    .arg("-Target")
    .arg(ruta_ayudante(destino))
    .arg("-Hash")
    .arg(hash)
    .arg("-Outcome")
    .arg(ruta_ayudante(&ruta_resultado(destino)?))
    .arg("-Version")
    .arg(&preparada.version)
    .arg("-BuildNumber")
    .arg(preparada.compilacion.to_string());
    if reiniciar {
        cmd.arg("-Restart");
    }
    if preparada.instalador { cmd.arg("-Installer"); }
    cmd.creation_flags(0x08000000)
        .spawn()
        .map_err(|_| Error::Disco)?;
    Ok(())
}

fn ruta_ayudante(path: &Path) -> PathBuf {
    let texto = path.to_string_lossy();
    if let Some(unc) = texto.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{unc}"));
    }
    if let Some(disco) = texto.strip_prefix(r"\\?\") {
        return PathBuf::from(disco);
    }
    path.to_path_buf()
}
#[cfg(not(windows))]
fn lanzar(_: &Path, _: &Path, _: &str, _: bool, _: &Preparada) -> Result<(), Error> {
    Err(Error::Plataforma)
}

const SCRIPT: &str = r#"param([int]$ProcessId, [string]$Source, [string]$Target, [string]$Hash, [switch]$Restart, [switch]$Installer, [string]$Outcome, [string]$Version, [UInt64]$BuildNumber)
$ErrorActionPreference = 'Stop'
# Solo un ayudante puede actualizar una misma copia del launcher.
$taskIdentity = [Security.Cryptography.SHA256]::Create()
$taskName = [BitConverter]::ToString($taskIdentity.ComputeHash([Text.Encoding]::UTF8.GetBytes($Target.ToLowerInvariant()))).Replace('-', '')
$taskIdentity.Dispose()
$taskMutex = [Threading.Mutex]::new($false, ('Local\IcaroUpdate-' + $taskName))
try { $taskOwnsMutex = $taskMutex.WaitOne(0) } catch [Threading.AbandonedMutexException] { $taskOwnsMutex = $true }
if (-not $taskOwnsMutex) { $taskMutex.Dispose(); exit 0 }
$taskBackup = $Target + '.icaro-anterior-' + [Guid]::NewGuid().ToString('N')
$taskNew = $Target + '.icaro-nuevo-' + [Guid]::NewGuid().ToString('N')
$taskSourceStream = $null
$taskOutputStream = $null
$taskReplaced = $false
$taskSuccess = $false
$taskFailure = $null
$taskRestartAllowed = $true
function Assert-RegularPath([string]$taskPath) {
    $taskCheck = [IO.Path]::GetFullPath($taskPath)
    while ($taskCheck) {
        if (Test-Path -LiteralPath $taskCheck) {
            $taskItem = Get-Item -LiteralPath $taskCheck -Force
            if ($taskItem.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Ruta no segura' }
        }
        $taskParent = [IO.Directory]::GetParent($taskCheck)
        if (-not $taskParent) { break }
        $taskCheck = $taskParent.FullName
    }
}
try {
    $taskProcess = Get-Process -Id $ProcessId -ErrorAction SilentlyContinue
    if ($taskProcess) { $taskProcess.WaitForExit() }
    $taskOther = Get-Process -Name ([IO.Path]::GetFileNameWithoutExtension($Target)) -ErrorAction SilentlyContinue | Where-Object { $_.Path -and $_.Path -ieq $Target }
    if ($taskOther) {
        $taskRestartAllowed = $false
        throw 'Cierra las otras ventanas de Icaro Launcher y vuelve a intentar la actualización.'
    }
    Assert-RegularPath $Source
    Assert-RegularPath $Target
    $taskSourceStream = [IO.File]::Open($Source, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
    $taskAlgorithm = [Security.Cryptography.SHA256]::Create()
    $taskActualHash = [BitConverter]::ToString($taskAlgorithm.ComputeHash($taskSourceStream)).Replace('-', '')
    $taskAlgorithm.Dispose()
    if ($taskActualHash -ne $Hash) { throw 'Integridad' }
    if ($Installer) {
        $taskInstallDir = [IO.Path]::GetDirectoryName([IO.Path]::GetFullPath($Target))
        Assert-RegularPath $taskInstallDir
        $taskLog = $Source + '.install.log'
        $taskSetup = Start-Process -FilePath $Source -ArgumentList @('/VERYSILENT','/SUPPRESSMSGBOXES','/SP-','/NORESTART','/NOCLOSEAPPLICATIONS','/NORESTARTAPPLICATIONS',('/DIR="' + $taskInstallDir + '"'),('/LOG="' + $taskLog + '"')) -WindowStyle Hidden -Wait -PassThru
        if ($taskSetup.ExitCode -ne 0) { throw ('Instalador: ' + $taskSetup.ExitCode) }
        if ((Get-Item -LiteralPath $Target).VersionInfo.ProductVersion -ne $Version) { throw 'El instalador terminó sin actualizar esta copia de Icaro Launcher.' }
        $taskSuccess = $true
        return
    }
    $taskSourceStream.Position = 0
    $taskOutputStream = [IO.File]::Open($taskNew, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
    $taskSourceStream.CopyTo($taskOutputStream)
    $taskOutputStream.Flush($true)
    $taskOutputStream.Dispose()
    $taskOutputStream = $null
    $taskSourceStream.Dispose()
    $taskSourceStream = $null
    for ($taskAttempt = 0; $taskAttempt -lt 40; $taskAttempt++) {
        try {
            Assert-RegularPath $Target
            [IO.File]::Replace($taskNew, $Target, $taskBackup)
            $taskReplaced = $true
            break
        } catch { Start-Sleep -Milliseconds 250 }
    }
    if (-not $taskReplaced) { throw 'No se pudo reemplazar el launcher' }
    $taskSuccess = $true
} catch {
    $taskFailure = $_.Exception.Message
    if ($taskReplaced -and (Test-Path -LiteralPath $taskBackup)) {
        try {
            Assert-RegularPath $taskBackup
            Assert-RegularPath $Target
            [IO.File]::Replace($taskBackup, $Target, [NullString]::Value)
        } catch { }
    }
    try {
        $taskErrorFile = $Source + '.' + [Guid]::NewGuid().ToString('N') + '.error.txt'
        Assert-RegularPath $taskErrorFile
        $taskErrorStream = [IO.File]::Open($taskErrorFile, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
        $taskErrorWriter = [IO.StreamWriter]::new($taskErrorStream, [Text.UTF8Encoding]::new($false))
        $taskErrorWriter.WriteLine($taskFailure)
        $taskErrorWriter.Dispose()
    } catch { }
} finally {
    if ($taskOutputStream) { $taskOutputStream.Dispose() }
    if ($taskSourceStream) { $taskSourceStream.Dispose() }
    if ($Outcome) {
        $taskReceiptTemp = $Outcome + '.' + [Guid]::NewGuid().ToString('N') + '.tmp'
        try {
            Assert-RegularPath $Outcome
            Assert-RegularPath $taskReceiptTemp
            $taskReceipt = @{esquema=1;version=$Version;compilacion=$BuildNumber;correcto=$taskSuccess;detalle=$taskFailure} | ConvertTo-Json -Compress
            [IO.File]::WriteAllText($taskReceiptTemp, $taskReceipt, [Text.UTF8Encoding]::new($false))
            if (Test-Path -LiteralPath $Outcome) { [IO.File]::Replace($taskReceiptTemp, $Outcome, [NullString]::Value) }
            else { [IO.File]::Move($taskReceiptTemp, $Outcome) }
        } catch { }
    }
    # Tanto éxito como fallo vuelven al launcher; no aparece otro asistente.
    if ($Restart -and $taskRestartAllowed -and (Test-Path -LiteralPath $Target)) { Start-Process -FilePath $Target -WindowStyle Normal }
    $taskMutex.ReleaseMutex()
    $taskMutex.Dispose()
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn el_ayudante_recibe_rutas_win32_y_conserva_espacios_y_unc() {
        assert_eq!(
            ruta_ayudante(Path::new(r"\\?\C:\Icaro Launcher\archivo.exe")),
            PathBuf::from(r"C:\Icaro Launcher\archivo.exe")
        );
        assert_eq!(
            ruta_ayudante(Path::new(r"\\?\UNC\equipo\Icaro Launcher\archivo.exe")),
            PathBuf::from(r"\\equipo\Icaro Launcher\archivo.exe")
        );
    }
    struct ArchivoResultado(PathBuf);
    impl ArchivoResultado {
        fn nuevo(bytes: &[u8]) -> Self {
            let mut id = [0u8; 16];
            getrandom::fill(&mut id).unwrap();
            let nombre: String = id.iter().map(|b| format!("{b:02x}")).collect();
            let ruta = std::env::temp_dir().join(format!("icaro-resultado-{nombre}.json"));
            use std::io::Write;
            let mut archivo = std::fs::OpenOptions::new().create_new(true).write(true).open(&ruta).unwrap();
            archivo.write_all(bytes).unwrap();
            Self(ruta)
        }
    }
    impl Drop for ArchivoResultado {
        fn drop(&mut self) { let _ = std::fs::remove_file(&self.0); }
    }
    #[test]
    fn el_resultado_se_consume_una_sola_vez_y_solo_confirma_el_binario_esperado() {
        let bytes = br#"{"esquema":1,"version":"0.2.0","compilacion":42,"correcto":true}"#;
        let archivo = ArchivoResultado::nuevo(bytes);
        assert!(leer_resultado(&archivo.0, "0.2.0", 42).unwrap().correcto);
        assert!(!archivo.0.exists());
        assert!(leer_resultado(&archivo.0, "0.2.0", 42).is_err());
        for (version, build) in [("0.1.0", 42), ("0.2.0", 41)] {
            let archivo = ArchivoResultado::nuevo(bytes);
            assert!(!leer_resultado(&archivo.0, version, build).unwrap().correcto);
        }
        let archivo = ArchivoResultado::nuevo(br#"{"esquema":1,"version":"0.2.0","compilacion":42,"correcto":false}"#);
        assert!(!leer_resultado(&archivo.0, "0.2.0", 42).unwrap().correcto);
    }
    #[test]
    fn no_acepta_resultados_corruptos_ni_excesivos() {
        for bytes in [br#"{"esquema":2,"version":"0.2.0","compilacion":42,"correcto":true}"#.as_slice(),
            br#"{"esquema":1,"version":"invalid","compilacion":42,"correcto":true}"#,
            b"{broken", &[b' '; 4097]] {
            let archivo = ArchivoResultado::nuevo(bytes);
            assert!(matches!(leer_resultado(&archivo.0,"0.2.0",42),Err(Error::Datos)));
        }
    }
    fn datos() -> (Release, Manifest) {
        let release = Release { tag_name: "v0.2.0".into(), draft: false, prerelease: false,
            assets: vec![Asset { name: "icaro-windows-x86_64.exe".into(), size: 200,
                browser_download_url: "https://github.com/00faithlss/Icaro-launcher/releases/download/v0.2.0/icaro-windows-x86_64.exe".into() }] };
        let manifest = Manifest {
            version: "0.2.0".into(),
            compilacion: 0,
            notas: vec![Nota {
                tipo: "mejora".into(),
                texto: "Consola más rápida.".into(),
            }],
            plataformas: BTreeMap::from([(
                PLATAFORMA.into(),
                Artefacto {
                    archivo: "icaro-windows-x86_64.exe".into(),
                    sha256: "ab".repeat(32),
                },
            )]),
        };
        (release, manifest)
    }
    #[test]
    fn reintentar_tras_reabrir_reutiliza_solo_la_descarga_completa_y_verificada() {
        let marca = ArchivoResultado::nuevo(b"prueba de cache");
        let raiz = marca.0.with_extension("cache");
        let carpeta = raiz.join("0123456789abcdef0123456789abcdef");
        std::fs::create_dir(&raiz).unwrap();
        std::fs::create_dir(&carpeta).unwrap();
        let (release, manifest) = datos();
        let mut disponible = resolver(release, manifest, "0.1.0").unwrap().unwrap();
        let contenido = b"ejecutable ya descargado";
        disponible.bytes = contenido.len() as u64;
        disponible.sha256 = format!("{:x}", Sha256::digest(contenido));
        let archivo = carpeta.join(format!("{}.exe", disponible.sha256));
        let parcial = archivo.with_extension("part");
        std::fs::write(&parcial, contenido).unwrap();
        assert!(recuperar_descarga(&raiz, &disponible).is_none());
        std::fs::rename(&parcial, &archivo).unwrap();
        let preparada = recuperar_descarga(&raiz, &disponible).unwrap();
        assert_eq!(preparada.archivo, archivo);
        assert_eq!(preparada.version, disponible.version);
        let mut otro = disponible.clone();
        otro.sha256 = "f".repeat(64);
        assert!(recuperar_descarga(&raiz, &otro).is_none());
        std::fs::write(&archivo, vec![b'x'; contenido.len()]).unwrap();
        assert!(recuperar_descarga(&raiz, &disponible).is_none());
        std::fs::write(&archivo, b"incompleto").unwrap();
        assert!(recuperar_descarga(&raiz, &disponible).is_none());
        std::fs::remove_file(&archivo).unwrap();
        std::fs::remove_dir(&carpeta).unwrap();
        std::fs::remove_dir(&raiz).unwrap();
    }

    #[test]
    fn compara_versiones_numericamente() {
        assert!(numeros("0.10.0") > numeros("0.9.9"));
        assert_eq!(numeros("v0.1.0"), Some([0, 1, 0]));
        assert_eq!(numeros("0.2.0-beta"), None);
    }
    #[test]
    fn solo_ofrece_release_mas_nueva() {
        let (r, m) = datos();
        assert!(resolver(r, m, "0.1.0").unwrap().is_some());
        let (r, m) = datos();
        assert!(resolver(r, m, "0.2.0").unwrap().is_none());
    }
    #[test]
    fn rechaza_artefacto_externo_o_tag_distinto() {
        let (mut r, m) = datos();
        r.assets[0].browser_download_url = "https://otro.example/launcher.exe".into();
        assert!(resolver(r, m, "0.1.0").is_err());
        let (mut r, m) = datos();
        r.tag_name = "v0.3.0".into();
        assert!(resolver(r, m, "0.1.0").is_err());
    }
    #[test]
    fn requiere_hash_y_plataforma() {
        let (r, mut m) = datos();
        m.plataformas.get_mut(PLATAFORMA).unwrap().sha256 = "ab".into();
        assert!(resolver(r, m, "0.1.0").is_err());
        let (r, mut m) = datos();
        m.plataformas.clear();
        assert!(matches!(resolver(r, m, "0.1.0"), Err(Error::SinArtefacto)));
    }
    #[test]
    fn revalida_destino_antes_de_descargar() {
        let (r, m) = datos();
        let mut d = resolver(r, m, "0.1.0").unwrap().unwrap();
        assert!(validar_descarga(&d).is_ok());
        d.url = "https://github.com/otro/Icaro/releases/download/v0.2.0/icaro.exe".into();
        assert!(validar_descarga(&d).is_err());
        d.url = "https://github.com/00faithlss/Icaro-launcher/releases/download/v0.3.0/icaro.exe".into();
        assert!(validar_descarga(&d).is_err());
    }
    #[test]
    fn una_compilacion_nueva_de_la_misma_version_se_actualiza_sin_downgrade() {
        let (mut r, mut m) = datos();
        m.compilacion = 42;
        r.tag_name = tag(&m.version, m.compilacion, false);
        r.assets[0].browser_download_url = format!("https://github.com/00faithlss/Icaro-launcher/releases/download/{}/{}", r.tag_name, r.assets[0].name);
        let d = resolver_compilacion(r.clone(), m.clone(), "0.2.0", 41, false, Canal::Estable).unwrap().unwrap();
        assert_eq!(d.compilacion, 42);
        assert!(validar_descarga(&d).is_ok());
        assert!(resolver_compilacion(r.clone(), m.clone(), "0.2.0", 42, false, Canal::Estable).unwrap().is_none());
        assert!(resolver_compilacion(r, m, "0.3.0", 0, false, Canal::Estable).unwrap().is_none());
    }
    fn beta(version: &str, compilacion: u64) -> (Release, Manifest) {
        let (mut r, mut m) = datos();
        m.version = version.into();
        m.compilacion = compilacion;
        r.prerelease = true;
        r.tag_name = tag(version, compilacion, true);
        r.assets[0].browser_download_url = format!("https://github.com/00faithlss/Icaro-launcher/releases/download/{}/{}", r.tag_name, r.assets[0].name);
        (r, m)
    }
    #[test]
    fn las_betas_solo_llegan_al_canal_beta() {
        let (r, m) = beta("0.3.0", 50);
        assert!(resolver_compilacion(r.clone(), m.clone(), "0.2.5", 40, false, Canal::Estable).unwrap().is_none());
        let d = resolver_compilacion(r.clone(), m.clone(), "0.2.5", 40, false, Canal::Beta).unwrap().unwrap();
        assert!(d.beta);
        assert!(validar_descarga(&d).is_ok());
        // Una beta con el tag de una estable, o sin compilación, se rechaza.
        let (mut r2, m2) = (r.clone(), m.clone());
        r2.tag_name = tag("0.3.0", 50, false);
        assert!(resolver_compilacion(r2, m2, "0.2.5", 40, false, Canal::Beta).is_err());
        let (r3, mut m3) = (r, m);
        m3.compilacion = 0;
        assert!(resolver_compilacion(r3, m3, "0.2.5", 40, false, Canal::Beta).is_err());
    }
    #[test]
    fn el_canal_beta_elige_la_release_mas_nueva_sin_mezclar_marcas() {
        let release = |tag: &str, prerelease: bool, draft: bool| Release { tag_name: tag.into(), draft, prerelease, assets: vec![] };
        let lista = || vec![
            release("v0.2.5-build.100", false, false),
            release("v0.3.0-beta.120", true, false),
            release("v0.3.0-beta.130", true, true),
            release("v0.3.0-beta.140", false, false),
            release("v0.3.0", false, false),
            release("v0.2.6-build.150", false, false),
        ];
        assert_eq!(elegir(lista(), Canal::Beta).unwrap().tag_name, "v0.3.0-beta.120");
        assert_eq!(elegir(lista(), Canal::Estable).unwrap().tag_name, "v0.2.6-build.150");
        // La estable de la misma versión publicada después sustituye a la beta.
        let mut con_estable = lista();
        con_estable.push(release("v0.3.0-build.160", false, false));
        assert_eq!(elegir(con_estable, Canal::Beta).unwrap().tag_name, "v0.3.0-build.160");
        assert_eq!(leer_tag("v0.3.0-beta.0"), None);
        assert_eq!(leer_tag("v0.3.0-rc.5"), None);
    }
    #[test]
    fn la_instalacion_exige_el_setup_y_no_sustituye_solo_el_portable() {
        let (r, m) = datos();
        assert!(matches!(resolver_compilacion(r, m, "0.1.0", 0, true, Canal::Estable), Err(Error::SinArtefacto)));
        let (mut r, mut m) = datos();
        let mut setup = m.plataformas[PLATAFORMA].clone();
        setup.archivo = "icaro-setup.exe".into();
        m.plataformas.insert("windows-x86_64-setup".into(), setup.clone());
        r.assets.push(Asset {name:setup.archivo.clone(),size:120,browser_download_url:format!("https://github.com/00faithlss/Icaro-launcher/releases/download/v0.2.0/{}",setup.archivo)});
        let d = resolver_compilacion(r, m, "0.1.0", 0, true, Canal::Estable).unwrap().unwrap();
        assert!(d.instalador);
        assert_eq!(d.bytes,120);
        assert!(validar_descarga(&d).is_ok());
    }
}
