// SPDX-License-Identifier: GPL-3.0-only
//! Importación local con lectura y extracción por bloques, sin cargar el ZIP en RAM.
use super::*;
use std::io::{Seek, SeekFrom};

const MAX_ARCHIVO: u64 = 8 * 1024 * 1024 * 1024;
const MAX_TOTAL: u64 = 32 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct ArchivoLocal {
    sha512: String,
    bytes: u64,
    entradas: Vec<Entrada>,
    #[serde(default)]
    formato: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Entrada {
    origen: String,
    destino: String,
    bytes: u64,
    crc: u32,
}
impl ArchivoLocal {
    pub(super) fn formato(&self) -> &str {
        if self.formato.is_empty() {
            "ZIP completo"
        } else {
            &self.formato
        }
    }
    pub(super) fn resumen(&self, plan: &Plan) -> (u64, Vec<String>, Vec<String>) {
        let omitidos: BTreeSet<_> = plan
            .indice
            .files
            .iter()
            .filter(|f| {
                f.env
                    .get("client")
                    .is_some_and(|v| v == "unsupported" || (!plan.opcionales && v == "optional"))
            })
            .map(|f| f.path.as_str())
            .collect();
        let selected: Vec<_> = self
            .entradas
            .iter()
            .filter(|e| !omitidos.contains(e.destino.as_str()))
            .collect();
        let bytes = selected.iter().map(|e| e.bytes).sum();
        let carpetas = selected
            .iter()
            .map(|e| e.destino.split('/').next().unwrap_or("").to_owned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let mods = selected
            .iter()
            .filter(|e| e.destino.starts_with("mods/") && e.destino.ends_with(".jar"))
            .map(|e| e.destino.clone())
            .collect();
        (bytes, carpetas, mods)
    }
    pub(super) fn cantidad(&self, plan: &Plan) -> usize {
        self.para_cliente(plan).entradas.len()
    }
    pub(super) fn para_cliente(&self, plan: &Plan) -> Self {
        let mut selected = self.clone();
        selected.entradas.retain(|e| {
            plan.indice
                .files
                .iter()
                .find(|f| f.path == e.destino)
                .is_none_or(|f| {
                    f.env
                        .get("client")
                        .is_none_or(|v| v != "unsupported" && (plan.opcionales || v != "optional"))
                })
        });
        selected
    }
    pub(super) fn contiene(&self, path: &str) -> bool {
        self.entradas.iter().any(|e| e.destino == path)
    }
}
fn copiar(
    reader: &mut impl Read,
    writer: &mut impl Write,
    limite: u64,
    c: &CancellationToken,
) -> Result<(u64, String), String> {
    let mut buffer = vec![0; 1024 * 1024];
    let mut hash = Sha512::new();
    let mut total = 0u64;
    loop {
        cancelado(c)?;
        let n = reader.read(&mut buffer).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        total = total
            .checked_add(n as u64)
            .ok_or("Archivo demasiado grande.")?;
        if total > limite {
            return Err("El archivo supera el tamaño permitido.".into());
        }
        hash.update(&buffer[..n]);
        writer.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
    }
    Ok((total, format!("{:x}", hash.finalize())))
}
fn cache(root: &Path, sha: &str) -> Result<PathBuf, String> {
    if !hex(sha, 128) {
        return Err("Identidad de archivo local inválida.".into());
    }
    Ok(root
        .join("cache/modpacks/locales")
        .join(format!("{sha}.zip")))
}
fn leer_json(
    archive: &mut zip::ZipArchive<fs::File>,
    name: &str,
) -> Result<Option<serde_json::Value>, String> {
    let Ok(mut file) = archive.by_name(name) else {
        return Ok(None);
    };
    if file.size() > 8 * 1024 * 1024 {
        return Err("El manifiesto supera 8 MiB.".into());
    }
    let mut bytes = vec![];
    file.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| format!("El manifiesto {name} no es válido."))
}

/// Crea una copia verificada en caché y revisa su contenido en un trabajador.
/// Los datos indicados manualmente se usan solo si el ZIP no declara un perfil.
pub fn preparar(
    root: &Path,
    archivo: &Path,
    minecraft: &str,
    loader: &str,
    loader_version: &str,
    c: &CancellationToken,
) -> Result<Plan, String> {
    preparar_con_progreso(
        root,
        archivo,
        minecraft,
        loader,
        loader_version,
        c,
        |_, _| {},
    )
}

/// Informa fases reales sin realizar peticiones a catálogos para archivos incluidos.
pub fn preparar_con_progreso(
    root: &Path,
    archivo: &Path,
    minecraft: &str,
    loader: &str,
    loader_version: &str,
    c: &CancellationToken,
    progreso: impl Fn(usize, String),
) -> Result<Plan, String> {
    progreso(0, "Abriendo el archivo".into());
    let extension = archivo
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !matches!(extension.as_str(), "mrpack" | "zip") {
        return Err("Selecciona un archivo .mrpack o .zip.".into());
    }
    let meta = crate::almacenamiento::regular(archivo)?;
    if !meta.is_file() || meta.len() > MAX_ARCHIVO {
        return Err("El paquete debe ser un archivo de hasta 8 GiB.".into());
    }
    cancelado(c)?;
    progreso(
        1,
        format!("{} bytes · {}", meta.len(), extension.to_uppercase()),
    );
    // Pedir el perfil antes de copiar varios GiB de un ZIP sin manifiesto.
    if extension == "zip" {
        let mut zip = zip::ZipArchive::new(fs::File::open(archivo).map_err(|e| e.to_string())?)
            .map_err(|_| "El paquete no es un ZIP válido.")?;
        if zip.len() > 10_000 {
            return Err("El paquete supera 10.000 entradas.".into());
        }
        let tiene_perfil = (0..zip.len()).any(|n| {
            zip.by_index(n).ok().is_some_and(|f| {
                f.name().ok().is_some_and(|n| {
                    n == "modrinth.index.json"
                        || n == "manifest.json"
                        || n == "mmc-pack.json"
                        || n.ends_with("/mmc-pack.json")
                })
            })
        });
        if !tiene_perfil {
            if !crate::minecraft::version_minecraft_valida(minecraft.trim()) {
                return Err(
                    "Este ZIP no indica Minecraft. Escribe su versión y vuelve a revisar.".into(),
                );
            }
            if loader != "vanilla"
                && !crate::minecraft::version_minecraft_valida(loader_version.trim())
            {
                return Err("Indica la versión del loader que utiliza el paquete.".into());
            }
        }
    }
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let temp = root.join(format!(
        "cache/modpacks/locales/.importar-{}-{nonce}.tmp",
        std::process::id()
    ));
    segura(root, &temp, true)?;
    let resultado = (|| {
        let mut source = fs::File::open(archivo).map_err(|e| e.to_string())?;
        let mut output = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temp)
            .map_err(|e| e.to_string())?;
        let (bytes, sha) = copiar(&mut source, &mut output, MAX_ARCHIVO, c)?;
        if bytes != meta.len() {
            return Err("El paquete cambió mientras se leía; vuelve a seleccionarlo.".into());
        }
        output.sync_all().map_err(|e| e.to_string())?;
        drop(output);
        // Revisar la copia que se instalará, aunque el original cambie o se mueva.
        let mut archive = zip::ZipArchive::new(fs::File::open(&temp).map_err(|e| e.to_string())?)
            .map_err(|_| "El paquete no es un ZIP válido.")?;
        if archive.len() > 10_000 {
            return Err("El paquete supera 10.000 entradas.".into());
        }
        let mut names = BTreeSet::new();
        progreso(2, format!("{} entradas", archive.len()));
        let mut entries = vec![];
        let mut total = 0u64;
        for n in 0..archive.len() {
            cancelado(c)?;
            let file = archive.by_index(n).map_err(|e| e.to_string())?;
            let name = file
                .name()
                .map_err(|_| "Nombre inválido en el ZIP.")?
                .trim_end_matches('/')
                .to_owned();
            ruta(&name)?;
            if !names.insert(name.to_ascii_lowercase())
                || file
                    .unix_mode()
                    .is_some_and(|m| !matches!(m & 0o170000, 0 | 0o100000 | 0o040000))
            {
                return Err("El paquete contiene enlaces o rutas duplicadas.".into());
            }
            total = total
                .checked_add(file.size())
                .ok_or("Paquete demasiado grande.")?;
            if total > MAX_TOTAL || file.size() > MAX_ARCHIVO {
                return Err("El paquete supera 32 GiB extraídos u 8 GiB por archivo.".into());
            }
            if !file.is_dir() {
                entries.push(Entrada {
                    origen: name,
                    destino: String::new(),
                    bytes: file.size(),
                    crc: file.crc32(),
                });
            }
        }
        let mr = leer_json(&mut archive, "modrinth.index.json")?;
        if extension == "mrpack" && mr.is_none() {
            return Err("El MRPACK no contiene modrinth.index.json.".into());
        }
        let cf = leer_json(&mut archive, "manifest.json")?;
        let formato = if mr.is_some() {
            "Modrinth"
        } else if cf.is_some() {
            "CurseForge"
        } else {
            "ZIP completo"
        }
        .to_owned();
        let mut index: Indice;
        let mut family = loader.trim().to_lowercase();
        let mut mc = minecraft.trim().to_owned();
        let mut lv = (!loader_version.trim().is_empty()).then(|| loader_version.trim().to_owned());
        if let Some(value) = mr {
            // Reutilizar todas las comprobaciones del contrato MRPACK sin sus overrides en memoria.
            let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
            writer
                .start_file(
                    "modrinth.index.json",
                    zip::write::SimpleFileOptions::default(),
                )
                .map_err(|e| e.to_string())?;
            writer
                .write_all(&serde_json::to_vec(&value).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            index = super::zip(&writer.finish().map_err(|e| e.to_string())?.into_inner())?.0;
            mc = index
                .dependencies
                .get("minecraft")
                .cloned()
                .unwrap_or_default();
            family = "vanilla".into();
            lv = None;
            for (key, version) in &index.dependencies {
                if key == "minecraft" {
                    continue;
                }
                let selected = match key.as_str() {
                    "fabric-loader" => "fabric",
                    "forge" => "forge",
                    "neoforge" => "neoforge",
                    _ => {
                        return Err(format!(
                            "El instalador de {key} todavía no está disponible."
                        ))
                    }
                };
                if lv.is_some() {
                    return Err(format!(
                        "El instalador de {key} todavía no está disponible."
                    ));
                }
                family = selected.into();
                lv = Some(version.clone());
            }
            let mut mapped = BTreeMap::new();
            for prefix in ["overrides/", "client-overrides/"] {
                for entry in &entries {
                    if let Some(dest) = entry.origen.strip_prefix(prefix) {
                        if index
                            .files
                            .iter()
                            .find(|f| f.path == dest)
                            .is_some_and(|f| {
                                f.env.get("client").is_some_and(|e| e == "unsupported")
                            })
                        {
                            continue;
                        }
                        let mut entry = entry.clone();
                        entry.destino = dest.into();
                        mapped.insert(dest.to_owned(), entry);
                    }
                }
            }
            entries = mapped.into_values().collect();
        } else {
            let mut prefix = String::new();
            if let Some(value) = cf {
                if value["files"].as_array().is_some_and(|f| !f.is_empty()) {
                    return Err("Este ZIP referencia archivos de CurseForge sin incluirlos. Importa un ZIP con los archivos completos o un MRPACK.".into());
                }
                if let Some(v) = value["minecraft"]["version"].as_str() {
                    mc = v.into();
                }
                if let Some(id) = value["minecraft"]["modLoaders"]
                    .as_array()
                    .and_then(|a| a.iter().find(|v| v["primary"] == true))
                    .and_then(|v| v["id"].as_str())
                {
                    if let Some(v) = id.strip_prefix("fabric-") {
                        family = "fabric".into();
                        lv = Some(v.into());
                    } else if let Some(v) = id.strip_prefix("forge-") {
                        family = "forge".into();
                        lv = Some(v.into());
                    } else if let Some(v) = id.strip_prefix("neoforge-") {
                        family = "neoforge".into();
                        lv = Some(v.into());
                    } else {
                        return Err(
                            "Este ZIP requiere un loader cuyo instalador no está disponible."
                                .into(),
                        );
                    }
                }
                prefix = format!("{}/", value["overrides"].as_str().unwrap_or("overrides"));
                ruta(prefix.trim_end_matches('/'))?;
            } else {
                // Exportaciones Ícaro/Prism y ZIP de una carpeta de Minecraft.
                for candidate in ["minecraft/", ".minecraft/"] {
                    if entries.iter().any(|e| e.origen.starts_with(candidate)) {
                        prefix = candidate.into();
                        break;
                    }
                }
                if prefix.is_empty() && !entries.iter().any(|e| contenido(&e.origen)) {
                    let roots: BTreeSet<_> = entries
                        .iter()
                        .filter_map(|e| e.origen.split_once('/').map(|(p, _)| p))
                        .collect();
                    if roots.len() == 1 {
                        prefix = format!("{}/", roots.first().unwrap());
                    }
                }
                for candidate in [format!("{prefix}mmc-pack.json"), "mmc-pack.json".into()] {
                    if let Some(value) = leer_json(&mut archive, &candidate)? {
                        if let Some(components) = value["components"].as_array() {
                            family = "vanilla".into();
                            lv = None;
                            for component in components {
                                match component["uid"].as_str() {
                                    Some("net.minecraft") => {
                                        mc = component["version"].as_str().unwrap_or("").into()
                                    }
                                    Some("net.fabricmc.fabric-loader") => {
                                        family = "fabric".into();
                                        lv = component["version"].as_str().map(str::to_owned);
                                    }
                                    Some("net.minecraftforge") => {
                                        family = "forge".into();
                                        lv = component["version"].as_str().map(str::to_owned);
                                    }
                                    Some("net.neoforged") => {
                                        family = "neoforge".into();
                                        lv = component["version"].as_str().map(str::to_owned);
                                    }
                                    Some("org.quiltmc.quilt-loader") => {
                                        return Err(
                                            "El loader del paquete todavía no está disponible."
                                                .into(),
                                        )
                                    }
                                    _ => {}
                                }
                            }
                        }
                        break;
                    }
                }
                for candidate in [
                    format!("{prefix}.minecraft/"),
                    format!("{prefix}minecraft/"),
                ] {
                    if entries.iter().any(|e| e.origen.starts_with(&candidate)) {
                        prefix = candidate;
                        break;
                    }
                }
            }
            entries = entries
                .into_iter()
                .filter_map(|mut e| {
                    let dest = e.origen.strip_prefix(&prefix)?;
                    if !contenido(dest) {
                        return None;
                    }
                    e.destino = dest.into();
                    Some(e)
                })
                .collect();
            if entries.is_empty() {
                return Err("El ZIP no contiene carpetas de juego importables (mods, config, resourcepacks…).".into());
            }
            index = Indice {
                format_version: 1,
                game: "minecraft".into(),
                name: archivo
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Modpack")
                    .chars()
                    .take(80)
                    .collect(),
                version_id: "local".into(),
                files: vec![],
                dependencies: BTreeMap::new(),
            };
            index.dependencies.insert("minecraft".into(), mc.clone());
            if let Some(v) = &lv {
                let key = if family == "fabric" {
                    "fabric-loader"
                } else {
                    family.as_str()
                };
                index.dependencies.insert(key.into(), v.clone());
            }
        }
        progreso(
            3,
            format!(
                "{} incluidos · {} referenciados",
                entries.len(),
                index.files.len()
            ),
        );
        if !crate::minecraft::version_minecraft_valida(&mc) {
            return Err(
                "Este ZIP no indica Minecraft. Escribe su versión y vuelve a revisar.".into(),
            );
        }
        if !matches!(family.as_str(), "vanilla" | "fabric" | "forge" | "neoforge") {
            return Err(
                "Selecciona Vanilla, Fabric, Forge o NeoForge; Quilt sigue pendiente.".into(),
            );
        }
        if family != "vanilla"
            && lv
                .as_ref()
                .is_none_or(|v| !crate::minecraft::version_minecraft_valida(v))
        {
            return Err("Indica la versión del loader que utiliza el paquete.".into());
        }
        let mut paths = BTreeSet::new();
        for entry in &entries {
            ruta(&entry.destino)?;
            if !paths.insert(entry.destino.to_ascii_lowercase()) {
                return Err("Hay archivos con el mismo destino.".into());
            }
        }
        for path in &paths {
            for (n, _) in path.match_indices('/') {
                if paths.contains(&path[..n]) {
                    return Err("Un archivo ocupa una carpeta del paquete.".into());
                }
            }
        }
        for file in &index.files {
            for path in &paths {
                let name = file.path.to_ascii_lowercase();
                if path != &name
                    && (path.starts_with(&(name.clone() + "/"))
                        || name.starts_with(&(path.clone() + "/")))
                {
                    return Err("Un archivo referenciado ocupa una carpeta incluida.".into());
                }
                if path == &name
                    && entries
                        .iter()
                        .any(|e| e.destino.to_ascii_lowercase() == name && e.destino != file.path)
                {
                    return Err("Rutas que solo difieren en mayúsculas.".into());
                }
            }
        }
        progreso(4, format!("Minecraft {mc} · {family}"));
        cancelado(c)?;
        let target = cache(root, &sha)?;
        segura(root, &target, false)?;
        if target.exists() {
            let mut existing = fs::File::open(&target).map_err(|e| e.to_string())?;
            if copiar(&mut existing, &mut std::io::sink(), MAX_ARCHIVO, c)? != (bytes, sha.clone())
            {
                return Err("La copia local en caché fue modificada.".into());
            }
        } else {
            fs::hard_link(&temp, &target).map_err(|e| e.to_string())?;
        }
        Ok(Plan {
            nombre: index.name.clone(),
            version: index.version_id.clone(),
            minecraft: mc,
            loader: family,
            loader_version: lv,
            opcionales: true,
            fuente: Fuente {
                url: String::new(),
                sha512: sha.clone(),
                size: bytes,
            },
            indice: index,
            local: Some(ArchivoLocal {
                sha512: sha,
                bytes,
                entradas: entries,
                formato,
            }),
        })
    })();
    let _ = fs::remove_file(&temp);
    resultado
}
fn contenido(path: &str) -> bool {
    matches!(
        path.split('/').next().unwrap_or(""),
        "mods"
            | "config"
            | "resourcepacks"
            | "shaderpacks"
            | "defaultconfigs"
            | "kubejs"
            | "scripts"
            | "saves"
            | "screenshots"
            | "journeymap"
            | "xaero"
            | "XaeroWaypoints"
            | "XaeroWorldMap"
            | "options.txt"
            | "servers.dat"
            | "icon.png"
            | "optionsof.txt"
            | "schematics"
            | "patchouli_books"
            | "fancymenu"
            | "custom"
            | "datapacks"
    )
}

pub(super) fn extraer(
    store: &Almacen,
    instance: &Instancia,
    local: &ArchivoLocal,
    c: &CancellationToken,
    progress: impl Fn(u64, u64),
) -> Result<(), String> {
    if local.bytes > MAX_ARCHIVO || local.entradas.len() > 10_000 {
        return Err("Los metadatos del paquete superan los límites.".into());
    }
    let total = local
        .entradas
        .iter()
        .try_fold(0u64, |total, entry| {
            let total = total.checked_add(entry.bytes)?;
            (entry.bytes <= MAX_ARCHIVO && total <= MAX_TOTAL).then_some(total)
        })
        .ok_or("Los metadatos del paquete superan los límites.")?;
    progress(0, total);
    let root = store.datos().map_err(|e| e.to_string())?;
    let path = cache(&root, &local.sha512)?;
    segura(&root, &path, false)?;
    let mut file = fs::File::open(path)
        .map_err(|_| "No se encuentra la copia local del paquete; vuelve a importarlo.")?;
    if copiar(&mut file, &mut std::io::sink(), MAX_ARCHIVO, c)?
        != (local.bytes, local.sha512.clone())
    {
        return Err("La copia del paquete no pasó SHA-512.".into());
    }
    file.seek(SeekFrom::Start(0)).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let game = store.juego(&instance.id).map_err(|e| e.to_string())?;
    let mut done = 0;
    for entry in &local.entradas {
        cancelado(c)?;
        let target = game.join(ruta(&entry.destino)?);
        segura(&game, &target, true)?;
        let mut source = archive.by_name(&entry.origen).map_err(|e| e.to_string())?;
        if source.size() != entry.bytes || source.crc32() != entry.crc {
            return Err("El archivo incluido cambió.".into());
        }
        // No reemplazar archivos modificados durante una instalación cancelada.
        if target.exists() {
            let actual = copiar(
                &mut fs::File::open(&target).map_err(|e| e.to_string())?,
                &mut std::io::sink(),
                MAX_ARCHIVO,
                c,
            )?;
            let expected = copiar(&mut source, &mut std::io::sink(), entry.bytes, c)?;
            if actual != expected || expected.0 != entry.bytes {
                return Err(
                    "Un archivo local cambió; se conserva. Importa en una nueva instancia.".into(),
                );
            }
            done += entry.bytes;
            progress(done, total);
            continue;
        }
        let temp = target.with_file_name(format!(".icaro-import-{}.tmp", std::process::id()));
        let result: Result<(), String> = (|| {
            let mut output = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temp)
                .map_err(|e| e.to_string())?;
            // zip verifica CRC al terminar de leer; el destino solo se publica después.
            let mut buffer = vec![0; 1024 * 1024];
            let mut count = 0;
            loop {
                cancelado(c)?;
                let n = source.read(&mut buffer).map_err(|e| e.to_string())?;
                if n == 0 {
                    break;
                }
                count += n as u64;
                if count > entry.bytes {
                    return Err("Entrada con tamaño inválido.".into());
                }
                output.write_all(&buffer[..n]).map_err(|e| e.to_string())?;
                progress(done + count, total);
            }
            if count != entry.bytes {
                return Err("Entrada truncada.".into());
            }
            output.sync_all().map_err(|e| e.to_string())?;
            drop(output);
            cancelado(c)?;
            segura(&game, &target, false)?;
            fs::hard_link(&temp, &target)
                .map_err(|_| "El destino ya existe; no se sobrescribió.".to_owned())?;
            Ok(())
        })();
        let _ = fs::remove_file(&temp);
        result?;
        done += entry.bytes;
    }
    progress(done, total);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Temp(PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            if let Ok(path) = self.0.canonicalize() {
                assert_eq!(
                    path.parent(),
                    Some(std::env::temp_dir().canonicalize().unwrap().as_path())
                );
                assert!(path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("icaro-import-"));
                let _ = fs::remove_dir_all(path);
            }
        }
    }
    fn temp() -> Temp {
        let mut nonce = [0u8; 16];
        getrandom::fill(&mut nonce).unwrap();
        let t = Temp(std::env::temp_dir().join(format!("icaro-import-{}", hash(&nonce))));
        fs::create_dir_all(&t.0).unwrap();
        t
    }
    fn paquete(path: &Path, files: &[(&str, &[u8])]) {
        let mut writer = zip::ZipWriter::new(fs::File::create(path).unwrap());
        for (name, bytes) in files {
            writer
                .start_file(*name, zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
    }
    #[tokio::test]
    async fn zip_local_instala_contenido_conserva_original_y_no_sobrescribe_cambios() {
        let t = temp();
        let root = t.0.join("data");
        let path = t.0.join("Amigos.zip");
        paquete(
            &path,
            &[
                ("Pack/minecraft/mods/ejemplo.jar", b"mod"),
                ("Pack/minecraft/resourcepacks/arte.zip", b"pack"),
                ("Pack/minecraft/config/ajustes.json", b"config"),
                ("Pack/accounts.json", b"privado"),
            ],
        );
        let c = CancellationToken::new();
        let plan = preparar(&root, &path, "1.21.1", "fabric", "0.19.5", &c).unwrap();
        assert_eq!(plan.archivos_incluidos(), 3);
        let store = Almacen::nuevo(root.join("instances"));
        let instance = crear(&store, &plan).unwrap();
        // La instalación usa la copia verificada, no depende del original.
        fs::rename(&path, t.0.join("movido.zip")).unwrap();
        let game = store.juego(&instance.id).unwrap();
        fs::create_dir_all(game.join("config")).unwrap();
        fs::write(game.join("config/ajustes.json"), b"personal").unwrap();
        assert!(instalar(&store, &instance, c.clone(), |_| {})
            .await
            .unwrap_err()
            .contains("se conserva"));
        assert_eq!(
            fs::read(game.join("config/ajustes.json")).unwrap(),
            b"personal"
        );
        assert!(!completo(&store, &instance));
        fs::remove_file(game.join("config/ajustes.json")).unwrap();
        instalar(&store, &instance, c.clone(), |_| {})
            .await
            .unwrap();
        assert!(completo(&store, &instance));
        for (name, bytes) in [
            ("mods/ejemplo.jar", b"mod".as_slice()),
            ("resourcepacks/arte.zip", b"pack"),
            ("config/ajustes.json", b"config"),
        ] {
            assert_eq!(fs::read(game.join(name)).unwrap(), bytes);
        }
        assert!(!game.join("accounts.json").exists());
        fs::write(game.join("config/ajustes.json"), b"editado").unwrap();
        instalar(&store, &instance, c, |_| {
            panic!("No reinstalar un paquete completo")
        })
        .await
        .unwrap();
        assert_eq!(
            fs::read(game.join("config/ajustes.json")).unwrap(),
            b"editado"
        );
    }
    #[tokio::test]
    async fn mrpack_local_respeta_manifiesto_capas_y_opcionales() {
        let t = temp();
        let path = t.0.join("ejemplo.mrpack");
        let root = t.0.join("data");
        let indice=serde_json::json!({"formatVersion":1,"game":"minecraft","name":"Pack","versionId":"1","dependencies":{"minecraft":"1.21.1","fabric-loader":"0.19.5"},"files":[
            {"path":"mods/optional.jar","hashes":{"sha512":"a".repeat(128),"sha1":"b".repeat(40)},"downloads":["https://cdn.modrinth.com/data/optional.jar"],"fileSize":3,"env":{"client":"optional"}}
        ]}).to_string();
        paquete(
            &path,
            &[
                ("modrinth.index.json", indice.as_bytes()),
                ("overrides/config/a.json", b"base"),
                ("client-overrides/config/a.json", b"cliente"),
                ("overrides/mods/optional.jar", b"opt"),
                ("server-overrides/config/a.json", b"server"),
            ],
        );
        let c = CancellationToken::new();
        let mut plan = preparar(&root, &path, "incorrecto", "forge", "incorrecto", &c).unwrap();
        assert_eq!(plan.minecraft, "1.21.1");
        assert_eq!(plan.loader, "fabric");
        plan.opcionales = false;
        assert_eq!(plan.archivos_incluidos(), 1);
        let store = Almacen::nuevo(root.join("instances"));
        let instance = crear(&store, &plan).unwrap();
        instalar(&store, &instance, c, |_| {}).await.unwrap();
        let game = store.juego(&instance.id).unwrap();
        assert_eq!(fs::read(game.join("config/a.json")).unwrap(), b"cliente");
        assert!(!game.join("mods/optional.jar").exists());
    }
    #[test]
    fn rutas_inseguras_colisiones_y_cancelacion_no_crean_instancias() {
        let t = temp();
        let root = t.0.join("data");
        let path = t.0.join("mal.zip");
        for names in [
            vec!["../fuera"],
            vec!["mods/a.jar", "mods/A.jar"],
            vec!["mods/a", "mods/a/b"],
            vec!["C:/mods/a.jar"],
            vec!["mods/CON.jar"],
        ] {
            let files: Vec<_> = names.iter().map(|n| (*n, b"x".as_slice())).collect();
            paquete(&path, &files);
            assert!(preparar(
                &root,
                &path,
                "1.21.1",
                "vanilla",
                "",
                &CancellationToken::new()
            )
            .is_err());
        }
        paquete(&path, &[("mods/a.jar", b"x")]);
        assert!(
            preparar(&root, &path, "", "vanilla", "", &CancellationToken::new())
                .unwrap_err()
                .contains("Escribe su versión")
        );
        let c = CancellationToken::new();
        c.cancel();
        assert!(preparar(&root, &path, "1.21.1", "vanilla", "", &c).is_err());
        assert!(!root.join("instances").exists());
        assert!(!t.0.join("fuera").exists());
    }
    #[test]
    fn revision_informa_fases_y_resumen_real_sin_crear_instancia() {
        let t = temp();
        let root = t.0.join("data");
        let path = t.0.join("pack.zip");
        paquete(
            &path,
            &[
                ("mods/a.jar", b"jar"),
                ("config/a.json", b"config"),
                ("resourcepacks/a.zip", b"pack"),
            ],
        );
        let fases = std::sync::Mutex::new(vec![]);
        let plan = preparar_con_progreso(
            &root,
            &path,
            "1.21.1",
            "fabric",
            "0.19.5",
            &CancellationToken::new(),
            |fase, _| fases.lock().unwrap().push(fase),
        )
        .unwrap();
        assert_eq!(*fases.lock().unwrap(), vec![0, 1, 2, 3, 4]);
        assert_eq!(plan.formato(), "ZIP completo");
        assert_eq!(plan.archivos_por_descargar(), 0);
        assert_eq!(
            plan.resumen(),
            (
                0,
                13,
                vec!["config".into(), "mods".into(), "resourcepacks".into()],
                1
            )
        );
        assert!(!root.join("instances").exists());
    }
    #[test]
    fn perfiles_prism_y_curseforge_y_formatos_no_admitidos() {
        let t = temp();
        let root = t.0.join("data");
        let path = t.0.join("pack.zip");
        let c = CancellationToken::new();
        let prism=serde_json::json!({"components":[{"uid":"net.minecraft","version":"1.21.1"},{"uid":"net.fabricmc.fabric-loader","version":"0.19.5"}]}).to_string();
        paquete(
            &path,
            &[
                ("Amigos/mmc-pack.json", prism.as_bytes()),
                ("Amigos/.minecraft/mods/uno.jar", b"mod"),
            ],
        );
        let p = preparar(&root, &path, "", "vanilla", "", &c).unwrap();
        assert_eq!(p.minecraft, "1.21.1");
        assert_eq!(p.loader, "fabric");
        assert_eq!(p.archivos_incluidos(), 1);
        let cf=serde_json::json!({"minecraft":{"version":"1.21.1","modLoaders":[{"id":"fabric-0.19.5","primary":true}]},"files":[],"overrides":"overrides"}).to_string();
        paquete(
            &path,
            &[
                ("manifest.json", cf.as_bytes()),
                ("overrides/config/uno.json", b"config"),
            ],
        );
        let p = preparar(&root, &path, "", "vanilla", "", &c).unwrap();
        assert_eq!(p.loader_version.as_deref(), Some("0.19.5"));
        assert_eq!(p.archivos_incluidos(), 1);
        let mut value: serde_json::Value = serde_json::from_str(&cf).unwrap();
        value["files"] = serde_json::json!([{"projectID":1,"fileID":2}]);
        paquete(
            &path,
            &[
                ("manifest.json", value.to_string().as_bytes()),
                ("overrides/config/uno", b"config"),
            ],
        );
        assert!(preparar(&root, &path, "", "vanilla", "", &c)
            .unwrap_err()
            .contains("CurseForge"));
    }
    #[test]
    fn importa_forge_y_neoforge_con_version_exacta_en_prism_curseforge_y_mrpack() {
        let t = temp();
        let root = t.0.join("data");
        let c = CancellationToken::new();
        for (family, uid, version) in [
            ("forge", "net.minecraftforge", "47.4.10"),
            ("neoforge", "net.neoforged", "21.1.219"),
        ] {
            let path = t.0.join(format!("{family}.zip"));
            let prism = serde_json::json!({"components":[
                {"uid":"net.minecraft","version":"1.21.1"},{"uid":uid,"version":version}
            ]})
            .to_string();
            paquete(
                &path,
                &[
                    ("mmc-pack.json", prism.as_bytes()),
                    ("minecraft/mods/personal.jar", b"personal"),
                ],
            );
            let plan = preparar(&root, &path, "", "vanilla", "", &c).unwrap();
            assert_eq!(plan.loader, family);
            assert_eq!(plan.loader_version.as_deref(), Some(version));
            let cf=serde_json::json!({"minecraft":{"version":"1.21.1","modLoaders":[{"id":format!("{family}-{version}"),"primary":true}]},"files":[],"overrides":"overrides"}).to_string();
            paquete(
                &path,
                &[
                    ("manifest.json", cf.as_bytes()),
                    ("overrides/mods/personal.jar", b"personal"),
                ],
            );
            let plan = preparar(&root, &path, "", "vanilla", "", &c).unwrap();
            assert_eq!(plan.loader, family);
            assert_eq!(plan.loader_version.as_deref(), Some(version));
            let mr=serde_json::json!({"formatVersion":1,"game":"minecraft","name":"Pack","versionId":"1",
                "dependencies":{"minecraft":"1.21.1",family:version},"files":[]}).to_string();
            let path = t.0.join(format!("{family}.mrpack"));
            paquete(
                &path,
                &[
                    ("modrinth.index.json", mr.as_bytes()),
                    ("overrides/mods/personal.jar", b"personal"),
                ],
            );
            let plan = preparar(&root, &path, "", "vanilla", "", &c).unwrap();
            assert_eq!(plan.loader, family);
            assert_eq!(plan.loader_version.as_deref(), Some(version));
        }
    }
    #[tokio::test]
    async fn cache_alterada_y_cancelacion_conservan_instancia_incompleta() {
        let t = temp();
        let path = t.0.join("pack.zip");
        let root = t.0.join("data");
        paquete(&path, &[("config/a", b"abc")]);
        let c = CancellationToken::new();
        let plan = preparar(&root, &path, "1.21.1", "vanilla", "", &c).unwrap();
        let store = Almacen::nuevo(root.join("instances"));
        let instance = crear(&store, &plan).unwrap();
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert!(instalar(&store, &instance, cancel, |_| {}).await.is_err());
        assert!(!completo(&store, &instance));
        fs::write(
            cache(&root, &plan.local.unwrap().sha512).unwrap(),
            b"alterado",
        )
        .unwrap();
        assert!(instalar(&store, &instance, c, |_| {})
            .await
            .unwrap_err()
            .contains("SHA-512"));
        assert!(!store.juego(&instance.id).unwrap().join("config/a").exists());
    }
    #[tokio::test]
    #[ignore = "Crea, copia y extrae un ZIP64 de más de 2 GiB en carpetas temporales"]
    async fn zip64_mayor_de_dos_gib_se_importa_por_bloques() {
        let t = temp();
        let path = t.0.join("grande.zip");
        let root = t.0.join("data");
        let mut writer = zip::ZipWriter::new(fs::File::create(&path).unwrap());
        writer
            .start_file(
                "resourcepacks/grande.zip",
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored)
                    .large_file(true),
            )
            .unwrap();
        let block = vec![0u8; 1024 * 1024];
        for _ in 0..2049 {
            writer.write_all(&block).unwrap();
        }
        writer.finish().unwrap();
        assert!(fs::metadata(&path).unwrap().len() > 2 * 1024 * 1024 * 1024);
        let c = CancellationToken::new();
        let plan = preparar(&root, &path, "1.21.1", "vanilla", "", &c).unwrap();
        let store = Almacen::nuevo(root.join("instances"));
        let instance = crear(&store, &plan).unwrap();
        instalar(&store, &instance, c, |_| {}).await.unwrap();
        assert_eq!(
            fs::metadata(
                store
                    .juego(&instance.id)
                    .unwrap()
                    .join("resourcepacks/grande.zip")
            )
            .unwrap()
            .len(),
            2049 * 1024 * 1024
        );
        assert!(completo(&store, &instance));
    }
}
