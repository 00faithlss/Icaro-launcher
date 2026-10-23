// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Versiones publicadas por los loaders y cambios de instancia con respaldo previo.
use crate::{
    http,
    instancias::{Almacen, Instancia},
    manifest::VersionEntry,
    modrinth::{Catalog, Version},
    mods,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Debug, Clone, Default)]
pub struct Loaders {
    pub versiones: [Vec<String>; 5],
    pub errores: [Option<String>; 5],
}

fn version_segura(v: &str) -> bool {
    crate::minecraft::version_minecraft_valida(v)
}

/// Las versiones finales van antes que las preliminares (`0.30.1` antes que
/// `0.31.0-beta.4`) para que la primera de la lista sea siempre una estable.
fn ordenar(versiones: &mut Vec<String>) {
    versiones.sort_by(|a, b| clave(b).cmp(&clave(a)).then_with(|| b.cmp(a)));
    versiones.dedup();
}
fn clave(v: &str) -> (bool, Vec<u64>, Vec<u64>) {
    let (base, pre) = v.split_once('-').map_or((v, None), |(b, p)| (b, Some(p)));
    (pre.is_none(), partes(base), pre.map(partes).unwrap_or_default())
}
fn partes(v: &str) -> Vec<u64> {
    v.split(|c: char| !c.is_ascii_digit())
        .filter_map(|p| p.parse().ok())
        .collect()
}

/// Vanilla, Fabric, Quilt, NeoForge y Forge, sin versiones de muestra.
pub async fn loaders(minecraft: &str) -> Result<Loaders, String> {
    loaders_para(minecraft, None).await
}
async fn loaders_para(minecraft: &str, solo: Option<usize>) -> Result<Loaders, String> {
    if !version_segura(minecraft) {
        return Err("La versión de Minecraft no es válida.".into());
    }
    let client = http::client().map_err(|e| e.to_string())?;
    let simple = |base: &'static str, servicio: &'static str, indice: usize| {
        let client = client.clone();
        let mc = minecraft.to_owned();
        async move {
            if solo.is_some_and(|i| i != indice) {
                return Ok(Vec::new());
            }
            let value: Vec<serde_json::Value> = http::json(
                client.get(format!("{base}/{mc}")),
                servicio,
                8 * 1024 * 1024,
            )
            .await
            .map_err(|e| e.to_string())?;
            let mut versions: Vec<String> = value
                .into_iter()
                .filter_map(|v| v.get("loader")?.get("version")?.as_str().map(str::to_owned))
                .filter(|v| version_segura(v))
                .collect();
            ordenar(&mut versions);
            Ok::<_, String>(versions)
        }
    };
    let fabric = simple("https://meta.fabricmc.net/v2/versions/loader", "Fabric", 1);
    let quilt = simple("https://meta.quiltmc.org/v3/versions/loader", "Quilt", 2);
    let neo = async {
        if solo.is_some_and(|i| i != 3) {
            return Ok(Vec::new());
        }
        #[derive(Deserialize)]
        struct Maven {
            versions: Vec<String>,
        }
        let value: Maven = http::json(
            client.get(
                "https://maven.neoforged.net/api/maven/versions/releases/net/neoforged/neoforge",
            ),
            "NeoForge",
            2 * 1024 * 1024,
        )
        .await
        .map_err(|e| e.to_string())?;
        let nums = partes(minecraft);
        let prefix = if minecraft.starts_with("1.") && nums.len() >= 2 {
            format!("{}.{}.", nums[1], nums.get(2).copied().unwrap_or(0))
        } else {
            format!("{minecraft}.")
        };
        let mut versions = value
            .versions
            .into_iter()
            .filter(|v| v.starts_with(&prefix) && version_segura(v))
            .collect();
        ordenar(&mut versions);
        Ok::<_, String>(versions)
    };
    let forge = async {
        if solo.is_some_and(|i| i != 4) {
            return Ok(Vec::new());
        }
        #[derive(Deserialize)]
        struct Promos {
            promos: BTreeMap<String, String>,
        }
        let value: Promos = http::json(
            client.get(
                "https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json",
            ),
            "Forge",
            2 * 1024 * 1024,
        )
        .await
        .map_err(|e| e.to_string())?;
        let mut versions = ["recommended", "latest"]
            .iter()
            .filter_map(|kind| value.promos.get(&format!("{minecraft}-{kind}")).cloned())
            .filter(|v| version_segura(v))
            .collect();
        ordenar(&mut versions);
        Ok::<_, String>(versions)
    };
    let (a, b, c, d) = tokio::join!(fabric, quilt, neo, forge);
    let mut result = Loaders::default();
    for (i, value) in [a, b, c, d].into_iter().enumerate() {
        match value {
            Ok(v) => result.versiones[i + 1] = v,
            Err(e) => result.errores[i + 1] = Some(e),
        }
    }
    Ok(result)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SeleccionLoader {
    minecraft: String,
    loader: String,
    version: String,
}
pub fn guardar_loader(store: &Almacen, i: &Instancia, version: &str) -> Result<(), String> {
    if i.loader == "vanilla" {
        return Ok(());
    }
    if !version_segura(version) {
        return Err("Selecciona una versión publicada del loader.".into());
    }
    let root = store.carpeta(&i.id).map_err(|e| e.to_string())?;
    let dst = root.join("loader.json");
    if dst.exists() {
        regular(&dst, false)?;
    }
    let temp = root.join(format!("loader-{}.next", nonce()?));
    let bytes = serde_json::to_vec_pretty(&SeleccionLoader {
        minecraft: i.minecraft.clone(),
        loader: i.loader.clone(),
        version: version.into(),
    })
    .map_err(|_| "No se pudo guardar el loader.")?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|_| "No se pudo guardar el loader.")?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|_| "No se pudo guardar el loader.")?;
    drop(file);
    fs::rename(temp, dst).map_err(|_| "No se pudo guardar el loader.".into())
}

pub fn leer_loader(store: &Almacen, i: &Instancia) -> Option<String> {
    let path = store.carpeta(&i.id).ok()?.join("loader.json");
    regular(&path, false).ok()?;
    if fs::metadata(&path).ok()?.len() > 64 * 1024 {
        return None;
    }
    let config: SeleccionLoader = serde_json::from_slice(&fs::read(path).ok()?).ok()?;
    (config.minecraft == i.minecraft
        && config.loader == i.loader
        && version_segura(&config.version))
    .then_some(config.version)
}

/// El manifiesto ordena versiones nuevas, snapshots y antiguas por su publicación.
pub fn baja(versiones: &[VersionEntry], actual: &str, destino: &str) -> bool {
    match (
        versiones.iter().find(|v| v.id == actual),
        versiones.iter().find(|v| v.id == destino),
    ) {
        (Some(a), Some(b)) => b.release_time < a.release_time,
        _ => partes(destino) < partes(actual),
    }
}

#[derive(Debug, Clone, Default)]
pub struct Compatibilidad {
    pub total: usize,
    pub compatibles: usize,
    pub no_cambian: usize,
    pub actualizar: Vec<Version>,
    pub identificados: Vec<mods::Instalado>,
    pub desactivar: Vec<String>,
    pub nombres: Vec<String>,
    pub desactivar_sin_actualizar: Vec<String>,
    pub nombres_sin_actualizar: Vec<String>,
}
#[derive(Debug, Clone)]
pub struct Revision {
    pub mundos: usize,
    pub desactivar: Vec<String>,
    pub nombres: Vec<String>,
    pub baja: bool,
    pub loader_version: Option<String>,
    pub loader: String,
    pub compatibilidad: Compatibilidad,
}
/// Se ejecuta una vez en segundo plano al abrir el selector, nunca al dibujarlo.
pub fn mods_para_compatibilidad(
    store: &Almacen,
    i: &Instancia,
) -> Result<Vec<mods::Instalado>, String> {
    let game = store.juego(&i.id).map_err(|e| e.to_string())?;
    let mut installed = mods::inventario(store, &i.id).map_err(|e| e.to_string())?;
    for item in &mut installed {
        let filename = if item.activo {
            item.archivo.clone()
        } else {
            format!("{}.disabled", item.archivo)
        };
        let path = game.join("mods").join(filename);
        regular(&path, false)?;
        if fs::metadata(&path)
            .map_err(|_| "No se pudo revisar el mod.")?
            .len()
            > 128 * 1024 * 1024
        {
            return Err("Un mod supera el tamaño máximo para revisarlo.".into());
        }
        let bytes = fs::read(path).map_err(|_| "No se pudo revisar el mod.")?;
        item.sha512 = Sha512::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
    }
    Ok(installed)
}
pub trait FuenteCambio: mods::Fuente {
    fn actuales_hash(
        &self,
        hashes: &[String],
    ) -> impl std::future::Future<Output = Result<BTreeMap<String, Version>, String>> + Send;
    fn actualizaciones_hash(
        &self,
        hashes: &[String],
        mc: &str,
        loader: &str,
    ) -> impl std::future::Future<Output = Result<BTreeMap<String, Version>, String>> + Send;
}
impl FuenteCambio for Catalog {
    async fn actuales_hash(&self, hashes: &[String]) -> Result<BTreeMap<String, Version>, String> {
        self.versions_from_hashes(hashes)
            .await
            .map_err(|e| e.to_string())
    }
    async fn actualizaciones_hash(
        &self,
        hashes: &[String],
        mc: &str,
        loader: &str,
    ) -> Result<BTreeMap<String, Version>, String> {
        self.latest_from_hashes(hashes, mc, loader)
            .await
            .map_err(|e| e.to_string())
    }
}
pub async fn compatibilidad(
    cat: &Catalog,
    installed: &[mods::Instalado],
    destino: &str,
    loader: &str,
) -> Result<Compatibilidad, String> {
    compatibilidad_con_fuente(cat, installed, destino, loader).await
}
pub async fn compatibilidad_con_fuente(
    source: &impl FuenteCambio,
    installed: &[mods::Instalado],
    destino: &str,
    loader: &str,
) -> Result<Compatibilidad, String> {
    if !version_segura(destino)
        || !matches!(
            loader,
            "vanilla" | "fabric" | "quilt" | "forge" | "neoforge"
        )
    {
        return Err("El destino de la revisión no es válido.".into());
    }
    let active: Vec<_> = installed.iter().collect();
    let mut result = Compatibilidad {
        total: active.len(),
        ..Default::default()
    };
    if loader == "vanilla" {
        for m in active {
            if m.activo {
                result.desactivar.push(m.archivo.clone());
                result.nombres.push(m.nombre.clone());
            } else {
                result.no_cambian += 1;
            }
        }
        result.desactivar_sin_actualizar = result.desactivar.clone();
        result.nombres_sin_actualizar = result.nombres.clone();
        return Ok(result);
    }
    let hashes = active
        .iter()
        .map(|m| m.sha512.to_lowercase())
        .collect::<Vec<_>>();
    if hashes
        .iter()
        .any(|h| h.len() != 128 || !h.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Err("Primero prepara los hashes reales de los mods instalados.".into());
    }
    // Un lote por petición, no una petición por mod. El caller limita destinos simultáneos.
    let (mut actuales, mut updates) = (BTreeMap::new(), BTreeMap::new());
    for chunk in hashes.chunks(256) {
        let (a, u) = tokio::try_join!(
            source.actuales_hash(chunk),
            source.actualizaciones_hash(chunk, destino, loader)
        )?;
        actuales.extend(a);
        updates.extend(u);
    }
    for m in active {
        let hash = m.sha512.to_lowercase();
        let actual = actuales.get(&hash);
        if actual.is_some_and(|v| !m.project_id.is_empty() && m.project_id != v.project_id) {
            return Err(format!("El hash de {} no coincide con sus metadatos; revisa el archivo antes de continuar.",m.nombre));
        }
        if let Some(v) = actual {
            let mut item = m.clone();
            item.project_id = v.project_id.clone();
            item.version_id = v.id.clone();
            item.version = v.version_number.clone();
            item.publicada = v.date_published.clone();
            item.dependencias = v.dependencies.clone();
            result.identificados.push(item);
        }
        let current_compatible = actual.is_some_and(|v| mods::compatible(v, destino, loader));
        let candidate = updates.get(&hash).filter(|v| {
            mods::compatible(v, destino, loader)
                && actual.is_some_and(|a| a.project_id == v.project_id)
        });
        if !current_compatible && m.activo {
            result.desactivar_sin_actualizar.push(m.archivo.clone());
            result.nombres_sin_actualizar.push(m.nombre.clone());
        }
        if let Some(next) = candidate {
            result.compatibles += 1;
            if actual.is_some_and(|a| a.id == next.id) {
                result.no_cambian += 1;
            } else {
                result.actualizar.push(next.clone());
            }
        } else if current_compatible {
            result.compatibles += 1;
            result.no_cambian += 1;
        } else if m.activo {
            result.desactivar.push(m.archivo.clone());
            result.nombres.push(m.nombre.clone());
        } else {
            result.no_cambian += 1;
        }
    }
    Ok(result)
}
pub async fn revisar(store: &Almacen, i: &Instancia, destino: &str) -> Result<Revision, String> {
    revisar_con_loader(store, i, destino, &i.loader).await
}
pub async fn revisar_con_loader(
    store: &Almacen,
    i: &Instancia,
    destino: &str,
    loader: &str,
) -> Result<Revision, String> {
    if !version_segura(destino) {
        return Err("La versión de destino no es válida.".into());
    }
    let (store_copy, record) = (store.clone(), i.clone());
    let (installed, mundos) = tokio::task::spawn_blocking(move || {
        let game = store_copy.juego(&record.id).map_err(|e| e.to_string())?;
        let saves = game.join("saves");
        let mundos = if saves.exists() {
            regular(&saves, true)?;
            fs::read_dir(saves)
                .map_err(|_| "No se pudieron revisar los mundos.")?
                .filter_map(Result::ok)
                .filter(|e| e.path().join("level.dat").is_file())
                .count()
        } else {
            0
        };
        Ok::<_, String>((mods_para_compatibilidad(&store_copy, &record)?, mundos))
    })
    .await
    .map_err(|_| "No se pudo revisar la instancia.")??;
    let catalog = Catalog::new().map_err(|e| e.to_string())?;
    let compatibilidad = compatibilidad(&catalog, &installed, destino, loader).await?;
    let mut result = Revision {
        mundos,
        desactivar: compatibilidad.desactivar.clone(),
        nombres: compatibilidad.nombres.clone(),
        baja: baja(&[], &i.minecraft, destino),
        loader_version: None,
        loader: loader.into(),
        compatibilidad,
    };
    if loader != "vanilla" {
        let index = match loader {
            "fabric" => 1,
            "quilt" => 2,
            "neoforge" => 3,
            "forge" => 4,
            _ => return Err("El loader no es válido.".into()),
        };
        let available = loaders_para(destino, Some(index)).await?;
        if let Some(e) = &available.errores[index] {
            return Err(e.clone());
        }
        result.loader_version = leer_loader(store, i)
            .filter(|v| i.loader == loader && available.versiones[index].contains(v))
            .or_else(|| available.versiones[index].first().cloned());
        if result.loader_version.is_none() {
            return Err(format!(
                "{loader} no tiene una versión publicada para Minecraft {destino}."
            ));
        }
    }
    Ok(result)
}

fn regular(path: &Path, dir: bool) -> Result<(), String> {
    let m =
        fs::symlink_metadata(path).map_err(|_| "No se pudo leer el archivo de la instancia.")?;
    #[cfg(windows)]
    let link = {
        use std::os::windows::fs::MetadataExt;
        m.file_attributes() & 0x400 != 0
    };
    #[cfg(not(windows))]
    let link = m.file_type().is_symlink();
    if link || (dir && !m.is_dir()) || (!dir && !m.is_file()) {
        return Err(
            "La instancia contiene un enlace o archivo no seguro. Se conservó sin cambios.".into(),
        );
    }
    Ok(())
}
fn nonce() -> Result<String, String> {
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "No se pudo crear el respaldo.")?
        .as_nanos();
    Ok(time.to_string())
}
fn copiar_arbol(
    source: &Path,
    target: &Path,
    depth: usize,
    files: &mut usize,
) -> Result<(), String> {
    if depth > 64 || *files > 100_000 {
        return Err("La instancia supera el límite de archivos para crear su respaldo.".into());
    }
    regular(source, true)?;
    regular(target, true)?;
    for entry in
        fs::read_dir(source).map_err(|_| "No se pudo leer la instancia para respaldarla.")?
    {
        if *files >= 100_000 {
            return Err("La instancia supera el límite de archivos para crear su respaldo.".into());
        }
        let entry = entry.map_err(|_| "No se pudo leer la instancia para respaldarla.")?;
        // El bloqueo protege la transacción actual; nunca forma parte de una copia de datos.
        if entry.file_name() == ".icaro-mods.lock" {
            continue;
        }
        let src = entry.path();
        let dst = target.join(entry.file_name());
        let meta = fs::symlink_metadata(&src).map_err(|_| "No se pudo leer la instancia.")?;
        if meta.is_dir() {
            regular(&src, true)?;
            fs::create_dir(&dst).map_err(|_| "No se pudo crear el respaldo de la instancia.")?;
            copiar_arbol(&src, &dst, depth + 1, files)?;
        } else {
            regular(&src, false)?;
            *files += 1;
            let mut out = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(dst)
                .map_err(|_| "No se pudo crear el respaldo.")?;
            let mut input =
                fs::File::open(src).map_err(|_| "No se pudo leer el archivo para respaldarlo.")?;
            std::io::copy(&mut input, &mut out)
                .map_err(|_| "No se pudo completar el respaldo. No se cambió la versión.")?;
            out.sync_all()
                .map_err(|_| "No se pudo completar el respaldo.")?;
        }
    }
    Ok(())
}
#[derive(Debug, Clone)]
pub struct Resultado {
    pub instancia: Instancia,
    pub copia: bool,
    pub respaldo: Option<PathBuf>,
}

/// Cambia únicamente la versión del loader y conserva Minecraft, familia y mods.
pub fn cambiar_solo_loader(store:&Almacen,i:&Instancia,version:&str,respaldo:bool)->Result<Resultado,String> {
    if i.loader=="vanilla" || !version_segura(version) {return Err("Selecciona una versión válida del loader de esta instancia.".into());}
    if store.leer(&i.id).map_err(|e|e.to_string())? != *i {return Err("La instancia cambió; vuelve a abrir el selector.".into());}
    let revision=Revision{mundos:0,desactivar:vec![],nombres:vec![],baja:false,loader_version:Some(version.into()),loader:i.loader.clone(),compatibilidad:Compatibilidad::default()};
    aplicar(store,i,&i.minecraft,&revision,false,respaldo)
}

/// Nunca borra archivos: respalda primero; retiene los mods sin compatibilidad como `.disabled`.
pub fn aplicar(
    store: &Almacen,
    i: &Instancia,
    destino: &str,
    revision: &Revision,
    copia: bool,
    respaldo_antes: bool,
) -> Result<Resultado, String> {
    let cambio_solo_loader = destino == i.minecraft && revision.loader == i.loader
        && i.loader != "vanilla" && revision.desactivar.is_empty()
        && revision.loader_version.is_some()
        && revision.loader_version != leer_loader(store, i);
    if !version_segura(destino) || (destino == i.minecraft && !cambio_solo_loader) {
        return Err("Selecciona una versión diferente de Minecraft.".into());
    }
    if !copia && !respaldo_antes && (revision.baja || baja(&[], &i.minecraft, destino)) {
        return Err(
            "Bajar de versión exige completar un respaldo antes de modificar la instancia.".into(),
        );
    }
    let destino_loader = if revision.loader.is_empty() {
        i.loader.as_str()
    } else {
        revision.loader.as_str()
    };
    if !matches!(
        destino_loader,
        "vanilla" | "fabric" | "quilt" | "forge" | "neoforge"
    ) {
        return Err("El loader de destino no es válido.".into());
    }
    if destino_loader != "vanilla"
        && revision
            .loader_version
            .as_deref()
            .is_none_or(|v| !version_segura(v))
    {
        return Err("Primero verifica una versión compatible del loader.".into());
    }
    let game = store.juego(&i.id).map_err(|e| e.to_string())?;
    let (mut target, backup) = if copia {
        let existentes = store.cargar().map_err(|e| e.to_string())?.instancias;
        let mut nombre = format!("{} (copia)", i.nombre);
        let mut n = 2;
        while existentes
            .iter()
            .any(|r| r.nombre.eq_ignore_ascii_case(&nombre))
        {
            nombre = format!("{} (copia {n})", i.nombre);
            n += 1;
        }
        // Crear con la versión original hasta que todos los archivos estén copiados.
        let new = store
            .crear_con_loader(&nombre, &i.minecraft, &i.loader, i.portada)
            .map_err(|e| e.to_string())?;
        if let Err(e) = copiar_arbol(
            &game,
            &store.juego(&new.id).map_err(|e| e.to_string())?,
            0,
            &mut 0,
        ) {
            let _ = store.eliminar(&new.id);
            return Err(e);
        }
        (new, None)
    } else if respaldo_antes {
        let root = store.carpeta(&i.id).map_err(|e| e.to_string())?;
        let base = root.join(".icaro-backups");
        if !base.exists() {
            fs::create_dir(&base).map_err(|_| "No se pudo crear el respaldo.")?;
        }
        regular(&base, true)?;
        let backup = base.join(nonce()?);
        fs::create_dir(&backup).map_err(|_| "No se pudo crear el respaldo.")?;
        copiar_arbol(&game, &backup, 0, &mut 0)?;
        (i.clone(), Some(backup))
    } else {
        (i.clone(), None)
    };
    let target_game = store.juego(&target.id).map_err(|e| e.to_string())?;
    let loader_path = store
        .carpeta(&target.id)
        .map_err(|e| e.to_string())?
        .join("loader.json");
    let anterior_loader = if loader_path.exists() {
        regular(&loader_path, false)?;
        if fs::metadata(&loader_path)
            .map_err(|_| "No se pudo leer el loader.")?
            .len()
            > 64 * 1024
        {
            return Err("Los metadatos del loader no son válidos.".into());
        }
        Some(fs::read(&loader_path).map_err(|_| "No se pudo leer el loader.")?)
    } else {
        None
    };
    let mut moved: Vec<(PathBuf, PathBuf)> = vec![];
    let mut loader_cambiado = false;
    let result = (|| {
        for filename in &revision.desactivar {
            if Path::new(filename).components().count() != 1 || !filename.ends_with(".jar") {
                return Err(
                    "No se pudo validar un archivo de mod; la versión original se conserva.".into(),
                );
            }
            let src = target_game.join("mods").join(filename);
            let dst = target_game
                .join("mods")
                .join(format!("{filename}.disabled"));
            regular(&src, false)?;
            if dst.exists() {
                return Err(
                    "Ya existe una copia desactivada del mod. La versión original se conserva."
                        .into(),
                );
            }
            fs::rename(&src, &dst)
                .map_err(|_| "No se pudo desactivar el mod antes de cambiar la versión.")?;
            moved.push((src, dst));
        }
        target.minecraft = destino.into();
        target.loader = destino_loader.into();
        if let Some(version) = &revision.loader_version {
            guardar_loader(store, &target, version)?;
            loader_cambiado = true;
        }
        store.guardar(&target).map_err(|e| e.to_string())
    })();
    if let Err(e) = result {
        if loader_cambiado {
            if let Some(bytes) = anterior_loader {
                let rollback = loader_path.with_file_name(format!("loader-{}.rollback", nonce()?));
                fs::write(&rollback,bytes).map_err(|_|"No se pudo recuperar la elección previa del loader; el respaldo conserva tus archivos.")?;
                fs::rename(rollback,&loader_path).map_err(|_|"No se pudo recuperar la elección previa del loader; el respaldo conserva tus archivos.")?;
            } else {
                regular(&loader_path, false)?;
                fs::remove_file(&loader_path)
                    .map_err(|_| "No se pudo retirar la elección de loader incompleta.")?;
            }
        }
        for (src, dst) in moved.into_iter().rev() {
            let _ = fs::rename(dst, src);
        }
        if copia {
            let _ = store.eliminar(&target.id);
        }
        return Err(e);
    }
    Ok(Resultado {
        instancia: target,
        copia,
        respaldo: backup,
    })
}

struct Preparacion {
    root: PathBuf,
    base: PathBuf,
}
impl Drop for Preparacion {
    fn drop(&mut self) {
        if let (Ok(root), Ok(base)) = (self.root.canonicalize(), self.base.canonicalize()) {
            if root.parent() == Some(base.as_path())
                && root
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with(".icaro-version-next-"))
            {
                let _ = fs::remove_dir_all(root);
            }
        }
    }
}
fn verificar_snapshot(game: &Path, installed: &[mods::Instalado]) -> Result<(), String> {
    let mods_dir = game.join("mods");
    let expected = installed
        .iter()
        .map(|m| {
            if m.activo {
                m.archivo.clone()
            } else {
                format!("{}.disabled", m.archivo)
            }
        })
        .collect::<std::collections::BTreeSet<_>>();
    let actual = if mods_dir.exists() {
        fs::read_dir(&mods_dir)
            .map_err(|_| "No se pudo revisar la carpeta de mods.")?
            .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
            .filter(|n| n.ends_with(".jar") || n.ends_with(".jar.disabled"))
            .collect::<std::collections::BTreeSet<_>>()
    } else {
        Default::default()
    };
    if expected != actual {
        return Err(
            "La lista de mods cambió durante la operación. Revisa de nuevo la compatibilidad."
                .into(),
        );
    }
    for m in installed {
        let name = if m.activo {
            m.archivo.clone()
        } else {
            format!("{}.disabled", m.archivo)
        };
        let path = game.join("mods").join(name);
        regular(&path, false)?;
        if fs::metadata(&path)
            .map_err(|_| "No se pudo verificar el mod.")?
            .len()
            > 128 * 1024 * 1024
        {
            return Err("El mod cambió de tamaño durante la operación.".into());
        }
        let bytes = fs::read(path).map_err(|_| "No se pudo volver a comprobar un mod.")?;
        let hash: String = Sha512::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        if hash != m.sha512 {
            return Err("Los mods cambiaron durante la revisión. Vuelve a comprobar antes de cambiar de versión.".into());
        }
    }
    Ok(())
}
/// Descarga y verifica reemplazos en una instancia temporal antes de tocar la original.
pub async fn aplicar_con_mods(
    source: &impl mods::Fuente,
    store: &Almacen,
    i: &Instancia,
    destino: &str,
    revision: &Revision,
    copia: bool,
    respaldo: bool,
    actualizar_mods: bool,
) -> Result<Resultado, String> {
    let mut effective = revision.clone();
    if effective.compatibilidad.total > 0 {
        effective.desactivar = if actualizar_mods {
            effective.compatibilidad.desactivar.clone()
        } else {
            effective.compatibilidad.desactivar_sin_actualizar.clone()
        };
        effective.nombres = if actualizar_mods {
            effective.compatibilidad.nombres.clone()
        } else {
            effective.compatibilidad.nombres_sin_actualizar.clone()
        };
    }
    if !actualizar_mods || effective.compatibilidad.actualizar.is_empty() {
        let (s, record, destino, r) = (store.clone(), i.clone(), destino.to_owned(), effective);
        return tokio::task::spawn_blocking(move || {
            aplicar(&s, &record, &destino, &r, copia, respaldo)
        })
        .await
        .map_err(|_| "No se pudo completar el cambio de versión.")?;
    }
    if !version_segura(destino)
        || (!copia && !respaldo && (effective.baja || baja(&[], &i.minecraft, destino)))
    {
        return Err(
            "El cambio requiere una versión válida y un respaldo antes de bajar de versión.".into(),
        );
    }
    let loader = if effective.loader.is_empty() {
        i.loader.clone()
    } else {
        effective.loader.clone()
    };
    if loader == "vanilla" {
        return Err(
            "Minecraft Vanilla no permite instalar mods; desactiva su actualización.".into(),
        );
    }
    let (s, record, r, loader_c, destino_c) = (
        store.clone(),
        i.clone(),
        effective.clone(),
        loader.clone(),
        destino.to_owned(),
    );
    let(preparacion,temp,temp_i,installed)=tokio::task::spawn_blocking(move||{
        let installed=mods_para_compatibilidad(&s,&record)?;
        let base=s.carpeta(&record.id).map_err(|e|e.to_string())?;regular(&base,true)?;
        let root=base.join(format!(".icaro-version-next-{}",nonce()?));fs::create_dir(&root).map_err(|_|"No se pudo preparar el cambio de mods.")?;
        let preparation=Preparacion{root:root.clone(),base};let temp=Almacen::nuevo(&root);let temp_i=temp.crear_con_loader("Preparación de cambio",&destino_c,&loader_c,record.portada).map_err(|e|e.to_string())?;
        let target_game=temp.juego(&temp_i.id).map_err(|e|e.to_string())?;let oldmods=s.juego(&record.id).map_err(|e|e.to_string())?.join("mods");let nextmods=target_game.join("mods");
        fs::create_dir(&nextmods).map_err(|_|"No se pudo preparar la carpeta de mods.")?;if oldmods.exists(){copiar_arbol(&oldmods,&nextmods,0,&mut 0)?;}
        let mut identified=installed.clone();
        for item in &mut identified {
            if let Some(known)=r.compatibilidad.identificados.iter().find(|m|m.archivo==item.archivo){if known.sha512!=item.sha512{return Err("Los mods cambiaron después de calcular su compatibilidad.".into())}*item=known.clone();}
            if r.desactivar.contains(&item.archivo)&&item.activo{
                let from=nextmods.join(&item.archivo);let to=nextmods.join(format!("{}.disabled",item.archivo));if to.exists(){return Err("Ya existe una copia desactivada del mod; conserva tus archivos y revisa la instancia.".into())}fs::rename(from,to).map_err(|_|"No se pudo preparar la desactivación de un mod.")?;item.activo=false;
            }
        }
        mods::registrar_inventario(&temp,&temp_i.id,&identified).map_err(|e|e.to_string())?;
        Ok::<_,String>((preparation,temp,temp_i,installed))
    }).await.map_err(|_|"No se pudo preparar el cambio de mods.")??;
    mods::instalar(
        source,
        &temp,
        &temp_i.id,
        effective.compatibilidad.actualizar.clone(),
        destino,
        &loader,
        |_| {},
    )
    .await
    .map_err(|e| e.to_string())?;
    let source_game = temp.juego(&temp_i.id).map_err(|e| e.to_string())?;
    let (s, record, destino, r) = (store.clone(), i.clone(), destino.to_owned(), effective);
    let result = tokio::task::spawn_blocking(move || {
        let game = s.juego(&record.id).map_err(|e| e.to_string())?;
        let _lock = mods::guardia(&game).map_err(|e| e.to_string())?;
        verificar_snapshot(&game, &installed)?;
        let original_root = s.carpeta(&record.id).map_err(|e| e.to_string())?;
        let original_loader = original_root.join("loader.json");
        let loader_before = if original_loader.exists() {
            regular(&original_loader, false)?;
            Some(
                fs::read(&original_loader)
                    .map_err(|_| "No se pudo guardar el estado previo del loader.")?,
            )
        } else {
            None
        };
        let mut commit_revision = r;
        commit_revision.desactivar.clear();
        commit_revision.nombres.clear();
        let result = aplicar(&s, &record, &destino, &commit_revision, copia, respaldo)?;
        let target_game = s.juego(&result.instancia.id).map_err(|e| e.to_string())?;
        let mods_dir = target_game.join("mods");
        let previous = target_game.join(".icaro-mods-version-backup");
        let had_mods = mods_dir.exists();
        let change = (|| {
            if previous.exists() {
                mods::borrar_preparacion(&target_game, &previous).map_err(|e| e.to_string())?;
            }
            if had_mods {
                regular(&mods_dir, true)?;
                fs::rename(&mods_dir, &previous)
                    .map_err(|_| "No se pudo guardar la carpeta de mods anterior.")?;
            }
            if fs::rename(source_game.join("mods"), &mods_dir).is_err() {
                if had_mods {
                    let _ = fs::rename(&previous, &mods_dir);
                }
                return Err("No se pudo publicar el nuevo conjunto de mods.".to_owned());
            }
            Ok(())
        })();
        if let Err(error) = change {
            if copia {
                let _ = s.eliminar(&result.instancia.id);
            } else {
                s.guardar(&record).map_err(|_| {
                    "No se pudo recuperar la versión original; el respaldo conserva tus archivos."
                })?;
                if let Some(bytes) = loader_before {
                    let next = original_root.join(format!("loader-{}.rollback", nonce()?));
                    fs::write(&next, bytes)
                        .map_err(|_| "No se pudo recuperar el loader anterior.")?;
                    fs::rename(next, &original_loader)
                        .map_err(|_| "No se pudo recuperar el loader anterior.")?;
                } else if original_loader.exists() {
                    regular(&original_loader, false)?;
                    fs::remove_file(original_loader)
                        .map_err(|_| "No se pudo retirar el loader incompleto.")?;
                }
            }
            return Err(error);
        }
        Ok(result)
    })
    .await
    .map_err(|_| "No se pudo completar el cambio de mods.")?;
    drop(preparacion);
    result
}

#[cfg(test)]
mod tests {
    #[test]
    fn las_versiones_estables_van_antes_que_las_preliminares() {
        let mut versiones: Vec<String> = ["0.31.0-beta.4", "0.24.0-beta.2", "0.24.0", "0.30.1", "0.30.1-beta.1", "0.24.0-beta.10", "0.30.1"]
            .map(String::from)
            .to_vec();
        ordenar(&mut versiones);
        assert_eq!(
            versiones,
            ["0.30.1", "0.24.0", "0.31.0-beta.4", "0.30.1-beta.1", "0.24.0-beta.10", "0.24.0-beta.2"]
        );
    }

    use super::*;
    fn store() -> (Almacen, PathBuf) {
        let root = std::env::temp_dir().join(format!("icaro-version-test-{}", nonce().unwrap()));
        (Almacen::nuevo(&root), root)
    }
    #[test]
    fn cambiar_loader_conserva_minecraft_familia_mods_y_mundos() {
        let root=std::env::temp_dir().join(format!("icaro-loader-pin-{}",nonce().unwrap()));
        let store=Almacen::nuevo(root.join("instances"));
        let i=store.crear_con_loader("Fabric","26.3","fabric",0).unwrap();
        guardar_loader(&store,&i,"0.19.4").unwrap();
        let game=store.juego(&i.id).unwrap();
        fs::create_dir_all(game.join("saves/world")).unwrap();fs::create_dir_all(game.join("mods")).unwrap();
        fs::write(game.join("saves/world/level.dat"),b"mundo").unwrap();fs::write(game.join("mods/mod.jar"),b"mod").unwrap();
        let original=fs::read(store.carpeta(&i.id).unwrap().join("instance.json")).unwrap();
        let result=cambiar_solo_loader(&store,&i,"0.19.5",true).unwrap();
        assert_eq!(result.instancia.minecraft,"26.3");assert_eq!(result.instancia.loader,"fabric");
        assert_eq!(leer_loader(&store,&i).as_deref(),Some("0.19.5"));
        assert_eq!(fs::read(game.join("mods/mod.jar")).unwrap(),b"mod");
        assert_eq!(fs::read(game.join("saves/world/level.dat")).unwrap(),b"mundo");
        assert_eq!(fs::read(result.respaldo.unwrap().join("saves/world/level.dat")).unwrap(),b"mundo");
        assert_eq!(fs::read(store.carpeta(&i.id).unwrap().join("instance.json")).unwrap(),original);
        assert!(cambiar_solo_loader(&store,&i,"0.19.5",false).is_err());
        assert!(cambiar_solo_loader(&store,&i,"../escape",false).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn respaldo_precede_cambio_y_copia_conserva_original() {
        let (s, root) = store();
        let i = s.crear("Prueba", "1.21.4", 0).unwrap();
        let game = s.juego(&i.id).unwrap();
        fs::create_dir(game.join("saves")).unwrap();
        fs::write(game.join("saves/level.dat"), b"mundo").unwrap();
        let r = Revision {
            mundos: 1,
            desactivar: vec![],
            nombres: vec![],
            baja: true,
            loader_version: None,
            loader: String::new(),
            compatibilidad: Compatibilidad::default(),
        };
        let changed = aplicar(&s, &i, "1.21.1", &r, false, true).unwrap();
        assert_eq!(
            fs::read(changed.respaldo.unwrap().join("saves/level.dat")).unwrap(),
            b"mundo"
        );
        assert_eq!(s.cargar().unwrap().instancias[0].minecraft, "1.21.1");
        let copy = aplicar(&s, &changed.instancia, "1.20.1", &r, true, true).unwrap();
        assert_ne!(copy.instancia.id, i.id);
        assert_eq!(
            fs::read(s.juego(&copy.instancia.id).unwrap().join("saves/level.dat")).unwrap(),
            b"mundo"
        );
        assert_eq!(
            s.cargar()
                .unwrap()
                .instancias
                .iter()
                .find(|v| v.id == i.id)
                .unwrap()
                .minecraft,
            "1.21.1"
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn fallo_no_cambia_metadata_ni_mods_anteriores() {
        let (s, root) = store();
        let i = s.crear("Prueba", "1.21.4", 0).unwrap();
        let game = s.juego(&i.id).unwrap();
        fs::create_dir(game.join("mods")).unwrap();
        fs::write(game.join("mods/uno.jar"), b"jar").unwrap();
        let r = Revision {
            mundos: 0,
            desactivar: vec!["uno.jar".into(), "../fuera.jar".into()],
            nombres: vec![],
            baja: true,
            loader_version: None,
            loader: String::new(),
            compatibilidad: Compatibilidad::default(),
        };
        assert!(aplicar(&s, &i, "1.21.1", &r, false, true).is_err());
        assert!(game.join("mods/uno.jar").exists());
        assert_eq!(s.cargar().unwrap().instancias[0].minecraft, "1.21.4");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn bajar_sin_respaldo_se_rechaza_en_el_nucleo_y_pin_cambia_con_minecraft() {
        let (s, root) = store();
        let i = s.crear_con_loader("Prueba", "1.21.4", "fabric", 0).unwrap();
        guardar_loader(&s, &i, "0.16.9").unwrap();
        let r = Revision {
            mundos: 0,
            desactivar: vec![],
            nombres: vec![],
            baja: true,
            loader_version: Some("0.16.10".into()),
            loader: String::new(),
            compatibilidad: Compatibilidad::default(),
        };
        assert!(aplicar(&s, &i, "1.21.1", &r, false, false).is_err());
        assert_eq!(s.cargar().unwrap().instancias[0].minecraft, "1.21.4");
        assert_eq!(leer_loader(&s, &i).as_deref(), Some("0.16.9"));
        let changed = aplicar(&s, &i, "1.21.1", &r, false, true).unwrap();
        assert_eq!(
            leer_loader(&s, &changed.instancia).as_deref(),
            Some("0.16.10")
        );
        assert!(leer_loader(&s, &i).is_none());
        let copy = aplicar(&s, &changed.instancia, "1.20.1", &r, true, true).unwrap();
        assert_eq!(leer_loader(&s, &copy.instancia).as_deref(), Some("0.16.10"));
        assert_eq!(
            leer_loader(&s, &changed.instancia).as_deref(),
            Some("0.16.10")
        );
        fs::remove_dir_all(root).unwrap();
    }
    struct CambioFake {
        versions: BTreeMap<String, Version>,
        bytes: BTreeMap<String, Vec<u8>>,
        corrupt: bool,
    }
    impl mods::Fuente for CambioFake {
        async fn version(&self, id: &str) -> Result<Version, mods::Error> {
            self.versions.get(id).cloned().ok_or(mods::Error::Datos)
        }
        async fn versiones(
            &self,
            project: &str,
            mc: &str,
            loader: &str,
        ) -> Result<Vec<Version>, mods::Error> {
            Ok(self
                .versions
                .values()
                .filter(|v| v.project_id == project && mods::compatible(v, mc, loader))
                .cloned()
                .collect())
        }
        async fn nombre(&self, project: &str) -> Result<String, mods::Error> {
            Ok(project.into())
        }
        async fn archivo(&self, file: &crate::modrinth::File) -> Result<Vec<u8>, mods::Error> {
            if self.corrupt {
                Ok(b"corrupto".to_vec())
            } else {
                self.bytes
                    .get(&file.filename)
                    .cloned()
                    .ok_or(mods::Error::Datos)
            }
        }
    }
    impl FuenteCambio for CambioFake {
        async fn actuales_hash(
            &self,
            hashes: &[String],
        ) -> Result<BTreeMap<String, Version>, String> {
            Ok(hashes
                .iter()
                .filter_map(|h| {
                    self.versions
                        .values()
                        .find(|v| v.files.iter().any(|f| f.hashes.sha512 == *h))
                        .map(|v| (h.clone(), v.clone()))
                })
                .collect())
        }
        async fn actualizaciones_hash(
            &self,
            hashes: &[String],
            mc: &str,
            loader: &str,
        ) -> Result<BTreeMap<String, Version>, String> {
            let actuales = self.actuales_hash(hashes).await?;
            Ok(actuales
                .into_iter()
                .filter_map(|(hash, old)| {
                    self.versions
                        .values()
                        .filter(|v| {
                            v.project_id == old.project_id && mods::compatible(v, mc, loader)
                        })
                        .max_by_key(|v| &v.date_published)
                        .map(|v| (hash, v.clone()))
                })
                .collect())
        }
    }
    fn cambio_fake() -> CambioFake {
        let mut source = CambioFake {
            versions: BTreeMap::new(),
            bytes: BTreeMap::new(),
            corrupt: false,
        };
        for (project, id, mcs, day) in [
            ("cambia", "cambia01", vec!["1.21.4"], "01"),
            ("cambia", "cambia02", vec!["1.21.5"], "02"),
            ("igual", "igual01", vec!["1.21.4", "1.21.5"], "01"),
            ("sin_destino", "sin01", vec!["1.21.4"], "01"),
            ("lib", "lib01", vec!["1.21.5"], "02"),
        ] {
            let bytes = id.as_bytes().to_vec();
            let hash: String = Sha512::digest(&bytes)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            let filename = format!("{id}.jar");
            let v:Version=serde_json::from_value(serde_json::json!({"id":id,"project_id":project,"name":id,"version_number":id,"version_type":"release","game_versions":mcs,"loaders":["fabric"],"files":[{"filename":filename,"url":format!("https://cdn.modrinth.com/data/{project}/versions/{id}/{filename}"),"size":bytes.len(),"primary":true,"hashes":{"sha512":hash}}],"dependencies":[],"changelog":null,"date_published":format!("2026-01-{day}T00:00:00Z")})).unwrap();
            source.bytes.insert(filename, bytes);
            source.versions.insert(id.into(), v);
        }
        source
    }
    async fn instancia_para_cambio(source: &CambioFake) -> (Almacen, PathBuf, Instancia) {
        let (s, root) = store();
        let i = s.crear_con_loader("Cambio", "1.21.4", "fabric", 0).unwrap();
        guardar_loader(&s, &i, "0.16.9").unwrap();
        let initial = ["cambia01", "igual01", "sin01"]
            .iter()
            .map(|id| source.versions[*id].clone())
            .collect();
        mods::instalar(source, &s, &i.id, initial, "1.21.4", "fabric", |_| {})
            .await
            .unwrap();
        let game = s.juego(&i.id).unwrap();
        fs::create_dir_all(game.join("saves/Mundo")).unwrap();
        fs::write(game.join("saves/Mundo/level.dat"), b"mundo-original").unwrap();
        (s, root, i)
    }
    async fn revision_fake(source: &CambioFake, s: &Almacen, i: &Instancia) -> Revision {
        let installed = mods_para_compatibilidad(s, i).unwrap();
        let c = compatibilidad_con_fuente(source, &installed, "1.21.5", "fabric")
            .await
            .unwrap();
        Revision {
            mundos: 1,
            desactivar: c.desactivar.clone(),
            nombres: c.nombres.clone(),
            baja: false,
            loader_version: Some("0.16.10".into()),
            loader: "fabric".into(),
            compatibilidad: c,
        }
    }
    #[tokio::test]
    async fn compatibilidad_batch_distingue_actualizaciones_actuales_y_sin_destino() {
        let source = cambio_fake();
        let (s, root, i) = instancia_para_cambio(&source).await;
        let r = revision_fake(&source, &s, &i).await;
        assert_eq!(r.compatibilidad.total, 3);
        assert_eq!(r.compatibilidad.compatibles, 2);
        assert_eq!(r.compatibilidad.no_cambian, 1);
        assert_eq!(r.compatibilidad.actualizar[0].id, "cambia02");
        assert_eq!(r.desactivar, ["sin01.jar"]);
        assert_eq!(r.compatibilidad.desactivar_sin_actualizar.len(), 2);
        let mut manual = mods_para_compatibilidad(&s, &i).unwrap();
        for m in &mut manual {
            m.project_id.clear();
            m.version_id.clear();
        }
        let c = compatibilidad_con_fuente(&source, &manual, "1.21.5", "fabric")
            .await
            .unwrap();
        assert_eq!(c.compatibles, 2);
        assert!(c.identificados.iter().all(|m| !m.project_id.is_empty()));
        let none = compatibilidad_con_fuente(&source, &manual, "1.21.5", "vanilla")
            .await
            .unwrap();
        assert_eq!(none.compatibles, 0);
        assert_eq!(none.desactivar.len(), 3);
        fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn migracion_descarga_fallida_conserva_version_loader_mods_y_mundos() {
        let mut source = cambio_fake();
        let (s, root, i) = instancia_para_cambio(&source).await;
        let r = revision_fake(&source, &s, &i).await;
        source.corrupt = true;
        assert!(
            aplicar_con_mods(&source, &s, &i, "1.21.5", &r, false, true, true)
                .await
                .is_err()
        );
        let game = s.juego(&i.id).unwrap();
        assert_eq!(s.cargar().unwrap().instancias[0].minecraft, "1.21.4");
        assert_eq!(leer_loader(&s, &i).as_deref(), Some("0.16.9"));
        assert_eq!(
            fs::read(game.join("mods/cambia01.jar")).unwrap(),
            b"cambia01"
        );
        assert!(game.join("mods/sin01.jar").exists());
        assert_eq!(
            fs::read(game.join("saves/Mundo/level.dat")).unwrap(),
            b"mundo-original"
        );
        assert!(!fs::read_dir(s.carpeta(&i.id).unwrap()).unwrap().any(|e| e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".icaro-version-next-")));
        fs::remove_dir_all(root).unwrap();
    }
    #[tokio::test]
    async fn migracion_actualiza_con_dependencias_respalda_y_opcion_apagada_desactiva() {
        let mut source = cambio_fake();
        source
            .versions
            .get_mut("cambia02")
            .unwrap()
            .dependencies
            .push(crate::modrinth::Dependency {
                project_id: Some("lib".into()),
                version_id: None,
                dependency_type: "required".into(),
                file_name: None,
            });
        let (s, root, i) = instancia_para_cambio(&source).await;
        let r = revision_fake(&source, &s, &i).await;
        let changed = aplicar_con_mods(&source, &s, &i, "1.21.5", &r, false, true, true)
            .await
            .unwrap();
        assert_eq!(changed.instancia.minecraft, "1.21.5");
        assert_eq!(
            leer_loader(&s, &changed.instancia).as_deref(),
            Some("0.16.10")
        );
        let game = s.juego(&i.id).unwrap();
        assert_eq!(
            fs::read(game.join("mods/cambia02.jar")).unwrap(),
            b"cambia02"
        );
        assert!(game.join("mods/lib01.jar").exists());
        assert!(game.join("mods/sin01.jar.disabled").exists());
        assert_eq!(
            fs::read(changed.respaldo.unwrap().join("mods/cambia01.jar")).unwrap(),
            b"cambia01"
        );
        assert_eq!(
            fs::read(game.join("saves/Mundo/level.dat")).unwrap(),
            b"mundo-original"
        );
        let (s2, root2, i2) = instancia_para_cambio(&source).await;
        let r2 = revision_fake(&source, &s2, &i2).await;
        let copy = aplicar_con_mods(&source, &s2, &i2, "1.21.5", &r2, true, true, false)
            .await
            .unwrap();
        assert_ne!(copy.instancia.id, i2.id);
        let copygame = s2.juego(&copy.instancia.id).unwrap();
        assert!(copygame.join("mods/cambia01.jar.disabled").exists());
        assert!(!copygame.join("mods/cambia02.jar").exists());
        assert_eq!(
            s2.cargar()
                .unwrap()
                .instancias
                .iter()
                .find(|m| m.id == i2.id)
                .unwrap()
                .minecraft,
            "1.21.4"
        );
        assert!(s2.juego(&i2.id).unwrap().join("mods/cambia01.jar").exists());
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(root2).unwrap();
    }
    #[tokio::test]
    async fn migracion_cuenta_inactivos_sin_reactivarlos_y_rollback_commit_recupera_loader() {
        let source = cambio_fake();
        let (s, root, i) = instancia_para_cambio(&source).await;
        mods::alternar(&s, &i.id, "cambia01.jar").unwrap();
        let r = revision_fake(&source, &s, &i).await;
        assert_eq!(r.compatibilidad.total, 3);
        assert_eq!(r.compatibilidad.compatibles, 2);
        assert_eq!(r.compatibilidad.actualizar.len(), 1);
        let changed = aplicar_con_mods(&source, &s, &i, "1.21.5", &r, false, true, true)
            .await
            .unwrap();
        assert!(s
            .juego(&i.id)
            .unwrap()
            .join("mods/cambia02.jar.disabled")
            .exists());
        assert!(!s.juego(&i.id).unwrap().join("mods/cambia02.jar").exists());
        assert_eq!(changed.instancia.minecraft, "1.21.5");
        let (s2, root2, i2) = instancia_para_cambio(&source).await;
        let r2 = revision_fake(&source, &s2, &i2).await;
        let game = s2.juego(&i2.id).unwrap();
        fs::write(game.join(".icaro-mods-version-backup"), b"preservar").unwrap();
        assert!(
            aplicar_con_mods(&source, &s2, &i2, "1.21.5", &r2, false, true, true)
                .await
                .is_err()
        );
        assert_eq!(s2.cargar().unwrap().instancias[0].minecraft, "1.21.4");
        assert_eq!(leer_loader(&s2, &i2).as_deref(), Some("0.16.9"));
        assert_eq!(
            fs::read(game.join("mods/cambia01.jar")).unwrap(),
            b"cambia01"
        );
        assert_eq!(
            fs::read(game.join(".icaro-mods-version-backup")).unwrap(),
            b"preservar"
        );
        assert!(game.join("mods/sin01.jar").exists());
        fs::remove_dir_all(root).unwrap();
        fs::remove_dir_all(root2).unwrap();
    }
}
