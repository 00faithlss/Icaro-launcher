// SPDX-License-Identifier: GPL-3.0-only
//! Archivos oficiales para alojar una instancia como servidor dedicado: el JAR
//! de servidor de Mojang (SHA-1 del `version.json` ya verificado de la
//! instancia) y, en Fabric, el lanzador de servidor de meta.fabricmc.net.
use super::*;

#[derive(Debug, Deserialize)]
struct VersionServidor {
    id: String,
    downloads: BTreeMap<String, Descarga>,
}

#[derive(Debug, Deserialize)]
struct Instalador {
    version: String,
    #[serde(default)]
    stable: bool,
}

fn cliente() -> Result<reqwest::Client, Error> {
    reqwest::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(180))
        .user_agent(crate::http::USER_AGENT)
        .build()
        .map_err(|_| Error::Red)
}

/// Segmento de versión seguro para rutas y URL (`0.16.9`, `1.21.1`, `1.0.1`).
fn segmento(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= 64
        && v != "."
        && v != ".."
        && v.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_' | b'+'))
}

/// `version.json` de la instancia ya instalada: la carpeta lleva el SHA-1 del
/// archivo y se vuelve a comprobar antes de confiar en sus descargas.
fn version_instalada(
    root: &Path,
    minecraft: &str,
    cancel: &CancellationToken,
) -> Result<VersionServidor, Error> {
    let carpeta = root.join("meta/minecraft/versions").join(minecraft);
    let lectura = fs::read_dir(&carpeta).map_err(|_| {
        Error::NoSoportada("instala y abre la instancia una vez antes de alojarla".into())
    })?;
    for entrada in lectura.flatten().take(32) {
        let nombre = entrada.file_name().to_string_lossy().into_owned();
        if !hash_valido(&nombre) {
            continue;
        }
        let ruta = entrada.path().join("version.json");
        if segura(root, &ruta, false).is_err() || !ruta.is_file() {
            continue;
        }
        if sha1_archivo(&ruta, cancel)?.1 != nombre {
            continue;
        }
        let version: VersionServidor = leer_json(root, &ruta)?;
        if version.id == minecraft {
            return Ok(version);
        }
    }
    Err(Error::NoSoportada(
        "instala y abre la instancia una vez antes de alojarla".into(),
    ))
}

/// Descarga (o reutiliza) el `server.jar` oficial verificado por SHA-1.
pub async fn jar_vanilla(
    store: &Almacen,
    minecraft: &str,
    cancel: &CancellationToken,
) -> Result<PathBuf, Error> {
    if !version_minecraft_valida(minecraft) {
        return Err(Error::Datos);
    }
    let root = store.datos().map_err(|_| Error::Disco)?;
    let version = version_instalada(&root, minecraft, cancel)?;
    let descarga = version
        .downloads
        .get("server")
        .ok_or_else(|| Error::NoSoportada("esta versión no tiene servidor oficial".into()))?;
    let archivo = nuevo_archivo(
        descarga,
        root.join("meta/minecraft/servers")
            .join(&descarga.sha1)
            .join("server.jar"),
        "Descargando el servidor",
    )?;
    descargar(&cliente()?, &root, &archivo, cancel).await?;
    Ok(archivo.ruta)
}

/// Lanzador oficial de servidor Fabric para esa versión y loader. Fabric no
/// publica su hash: se descarga solo de meta.fabricmc.net por HTTPS, se
/// comprueba que sea un JAR y se reutiliza desde la caché compartida.
pub async fn jar_fabric(
    store: &Almacen,
    minecraft: &str,
    loader: &str,
    cancel: &CancellationToken,
) -> Result<PathBuf, Error> {
    if !version_minecraft_valida(minecraft) || !segmento(loader) {
        return Err(Error::Datos);
    }
    let root = store.datos().map_err(|_| Error::Disco)?;
    let base = root
        .join("meta/loaders/fabric-servidor")
        .join(minecraft)
        .join(loader);
    if let Ok(lectura) = fs::read_dir(&base) {
        for entrada in lectura.flatten() {
            let ruta = entrada.path().join("fabric-server-launch.jar");
            if segura(&root, &ruta, false).is_ok() && es_jar(&ruta) {
                return Ok(ruta);
            }
        }
    }
    let client = cliente()?;
    let lista = bytes_oficiales(
        &client,
        "https://meta.fabricmc.net/v2/versions/installer",
        METADATA_LIMIT as usize,
        cancel,
    )
    .await?;
    let instaladores: Vec<Instalador> = serde_json::from_slice(&lista).map_err(|_| Error::Datos)?;
    let instalador = instaladores
        .iter()
        .find(|i| i.stable && segmento(&i.version))
        .ok_or(Error::Datos)?;
    let url = format!(
        "https://meta.fabricmc.net/v2/versions/loader/{minecraft}/{loader}/{}/server/jar",
        instalador.version
    );
    let bytes = bytes_oficiales(&client, &url, 16 * 1024 * 1024, cancel).await?;
    if bytes.len() < 4 || &bytes[..4] != b"PK\x03\x04" {
        return Err(Error::Integridad);
    }
    let ruta = base
        .join(&instalador.version)
        .join("fabric-server-launch.jar");
    segura(&root, &ruta, true)?;
    let (temporal, mut archivo) = Temporal::crear(
        ruta.parent()
            .ok_or(Error::Datos)?
            .join(format!(".icaro-{}.tmp", random()?)),
    )?;
    archivo
        .write_all(&bytes)
        .and_then(|_| archivo.sync_all())
        .map_err(|_| Error::Disco)?;
    drop(archivo);
    cancelado(cancel)?;
    fs::rename(&temporal.0, &ruta).map_err(|_| Error::Disco)?;
    Ok(ruta)
}

fn es_jar(ruta: &Path) -> bool {
    let mut cabecera = [0u8; 4];
    fs::symlink_metadata(ruta).is_ok_and(|m| m.is_file() && !enlace(&m) && m.len() > 4)
        && File::open(ruta)
            .and_then(|mut f| f.read_exact(&mut cabecera))
            .is_ok()
        && &cabecera == b"PK\x03\x04"
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn los_segmentos_no_escapan_de_la_carpeta() {
        assert!(segmento("0.16.9"));
        assert!(segmento("1.0.1+build.2"));
        assert!(!segmento(".."));
        assert!(!segmento("0.16/../x"));
        assert!(!segmento(""));
    }
}
