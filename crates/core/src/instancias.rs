// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Instancias locales: identidad estable, carpeta aislada y metadatos persistentes.

use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_METADATA: u64 = 64 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Instancia {
    pub id: String,
    pub nombre: String,
    pub minecraft: String,
    #[serde(default = "vanilla")]
    pub loader: String,
    pub portada: u8,
    pub creada: u64,
}
fn vanilla() -> String {
    "vanilla".into()
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("No se pudo leer o guardar la instancia en disco.")]
    Disco,
    #[error("Los datos de la instancia no son válidos.")]
    Datos,
    #[error("El nombre debe contener entre 1 y 80 caracteres, sin caracteres de control.")]
    Nombre,
    #[error("{0}")]
    Papelera(String),
    #[error("Ya existe una instancia con ese nombre.")]
    Repetida,
}

#[derive(Debug, Clone)]
pub struct Almacen {
    raiz: PathBuf,
    rutas: Arc<Mutex<BTreeMap<String, PathBuf>>>,
    eliminadas: Arc<Mutex<BTreeMap<String, (crate::papelera::Ticket, Instancia)>>>,
}
pub struct Informe {
    pub instancias: Vec<Instancia>,
    pub omitidas: usize,
}

fn id_valido(id: &str) -> bool {
    id.len() == 32
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn id_nuevo() -> Result<String, Error> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|_| Error::Disco)?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
fn validar(i: &Instancia) -> Result<(), Error> {
    if i.nombre.trim().is_empty()
        || i.nombre.chars().count() > 80
        || i.nombre.chars().any(char::is_control)
    {
        return Err(Error::Nombre);
    }
    if !matches!(
        i.loader.as_str(),
        "vanilla" | "fabric" | "forge" | "neoforge" | "quilt"
    ) || !id_valido(&i.id)
        || i.portada > 3
        || !crate::minecraft::version_minecraft_valida(&i.minecraft)
    {
        return Err(Error::Datos);
    }
    Ok(())
}
fn enlace(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

/// Nombres de carpetas válidos también en Windows; la identidad sigue siendo `id`.
fn nombre_carpeta(nombre: &str) -> String {
    let limpio: String = nombre
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let limpio = limpio.trim().trim_matches('.').trim();
    let mut nombre = if limpio.is_empty() {
        "Instancia".into()
    } else {
        limpio.to_string()
    };
    let base = nombre.split('.').next().unwrap_or("").to_ascii_uppercase();
    if matches!(base.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ((base.starts_with("COM") || base.starts_with("LPT"))
            && base.len() == 4
            && matches!(base.as_bytes()[3], b'1'..=b'9'))
        || nombre.starts_with('.')
    {
        nombre.insert_str(0, "Instancia ");
    }
    nombre
}

fn regular(path: &Path, dir: bool) -> Result<(), Error> {
    let m = fs::symlink_metadata(path).map_err(|_| Error::Disco)?;
    if enlace(&m) || (dir && !m.is_dir()) || (!dir && (!m.is_file() || m.len() > MAX_METADATA)) {
        return Err(Error::Datos);
    }
    Ok(())
}

fn leer_en(path: &Path) -> Result<Instancia, Error> {
    regular(path, true)?;
    let json = path.join("instance.json");
    let i: Instancia = if json.exists() {
        regular(&json, false)?;
        let mut bytes = Vec::new();
        fs::File::open(json)
            .map_err(|_| Error::Disco)?
            .take(MAX_METADATA + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| Error::Disco)?;
        if bytes.len() > MAX_METADATA as usize {
            return Err(Error::Datos);
        }
        serde_json::from_slice(&bytes).map_err(|_| Error::Datos)?
    } else {
        let cfg = path.join("instance.cfg");
        regular(&cfg, false)?;
        let text = fs::read_to_string(cfg).map_err(|_| Error::Disco)?;
        let mut valores = BTreeMap::new();
        for line in text.lines() {
            if let Some((key, value)) = line.split_once('=') {
                if valores.insert(key.trim(), value.trim()).is_some() {
                    return Err(Error::Datos);
                }
            }
        }
        let texto = |key: &str| -> Result<String, Error> {
            serde_json::from_str(valores.get(key).ok_or(Error::Datos)?).map_err(|_| Error::Datos)
        };
        Instancia {
            id: texto("Id")?,
            nombre: texto("Name")?,
            minecraft: texto("Minecraft")?,
            loader: texto("Loader")?,
            portada: valores
                .get("Cover")
                .ok_or(Error::Datos)?
                .parse()
                .map_err(|_| Error::Datos)?,
            creada: valores
                .get("Created")
                .ok_or(Error::Datos)?
                .parse()
                .map_err(|_| Error::Datos)?,
        }
    };
    validar(&i)?;
    regular(&path.join("minecraft"), true)?;
    Ok(i)
}

#[cfg(test)]
fn cfg(i: &Instancia) -> Result<Vec<u8>, Error> {
    let string = |s: &str| serde_json::to_string(s).map_err(|_| Error::Datos);
    Ok(format!(
        "[General]\nId={}\nName={}\nMinecraft={}\nLoader={}\nCover={}\nCreated={}\n",
        string(&i.id)?,
        string(&i.nombre)?,
        string(&i.minecraft)?,
        string(&i.loader)?,
        i.portada,
        i.creada
    )
    .into_bytes())
}

fn escribir(path: &Path, bytes: &[u8]) -> Result<(), Error> {
    let temp = path.with_file_name(format!("{}.tmp", id_nuevo()?));
    let result = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|_| Error::Disco)?;
        file.write_all(bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| Error::Disco)?;
        drop(file);
        fs::rename(&temp, path).map_err(|_| Error::Disco)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temp);
    }
    result
}

fn documento(path: &Path) -> Result<serde_json::Value, Error> {
    let json = path.join("instance.json");
    if json.exists() {
        regular(&json, false)?;
        if fs::metadata(&json).map_err(|_| Error::Disco)?.len() > MAX_METADATA {
            return Err(Error::Datos);
        }
        let value: serde_json::Value =
            serde_json::from_slice(&fs::read(json).map_err(|_| Error::Disco)?)
                .map_err(|_| Error::Datos)?;
        if !value.is_object() {
            return Err(Error::Datos);
        }
        Ok(value)
    } else {
        Ok(serde_json::json!({}))
    }
}

static CONFIG_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn guardar_en(path: &Path, i: &Instancia) -> Result<(), Error> {
    let _guard = CONFIG_LOCK.lock().map_err(|_| Error::Disco)?;
    let mut value = documento(path)?;
    let object = value.as_object_mut().ok_or(Error::Datos)?;
    if object
        .get("schema")
        .and_then(|v| v.as_u64())
        .is_some_and(|s| s > 2)
    {
        return Err(Error::Datos);
    }
    object.extend(
        serde_json::to_value(i)
            .map_err(|_| Error::Datos)?
            .as_object()
            .ok_or(Error::Datos)?
            .clone(),
    );
    object.insert("schema".into(), 2.into());
    let config = path.join("instance.cfg");
    if config.exists() {
        regular(&config, false)?;
        if fs::metadata(&config).map_err(|_| Error::Disco)?.len() > MAX_METADATA {
            return Err(Error::Datos);
        }
        // Conserva todas las claves desconocidas del formato anterior como texto.
        object
            .entry("legacy_cfg")
            .or_insert(serde_json::Value::String(
                fs::read_to_string(&config).map_err(|_| Error::Disco)?,
            ));
    }
    let bytes = serde_json::to_vec_pretty(&value).map_err(|_| Error::Datos)?;
    if bytes.len() as u64 > MAX_METADATA {
        return Err(Error::Datos);
    }
    if fs::read(path.join("instance.json")).ok().as_deref() != Some(bytes.as_slice()) {
        escribir(&path.join("instance.json"), &bytes)?;
    }
    if config.exists() {
        fs::remove_file(config).map_err(|_| Error::Disco)?;
    }
    Ok(())
}

impl Almacen {
    fn destino(&self, nombre: &str, actual: Option<&Path>) -> Result<PathBuf, Error> {
        let base = nombre_carpeta(nombre);
        let ocupados: Vec<PathBuf> = fs::read_dir(&self.raiz)
            .map_err(|_| Error::Disco)?
            .map(|e| e.map(|e| e.path()).map_err(|_| Error::Disco))
            .collect::<Result<_, _>>()?;
        for n in 1..=10_000 {
            let name = if n == 1 {
                base.clone()
            } else {
                format!("{base} ({n})")
            };
            if !ocupados
                .iter()
                .filter(|p| Some(p.as_path()) != actual)
                .any(|p| {
                    p.file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_lowercase()
                        == name.to_lowercase()
                })
            {
                return Ok(self.raiz.join(name));
            }
        }
        Err(Error::Repetida)
    }
    pub fn nuevo(raiz: impl Into<PathBuf>) -> Self {
        Self {
            raiz: raiz.into(),
            rutas: Default::default(),
            eliminadas: Default::default(),
        }
    }
    /// Raíz de datos compartidos, hermana del directorio de instancias.
    pub(crate) fn datos(&self) -> Result<PathBuf, Error> {
        let instances = self.raiz.canonicalize().map_err(|_| Error::Disco)?;
        instances.parent().map(Path::to_owned).ok_or(Error::Datos)
    }
    pub fn configuracion(&self, id: &str) -> Result<Option<serde_json::Value>, Error> {
        Ok(documento(&self.carpeta(id)?)?.get("configuracion").cloned())
    }
    pub fn guardar_configuracion(
        &self,
        id: &str,
        settings: &serde_json::Value,
    ) -> Result<(), Error> {
        let path = self.carpeta(id)?;
        let _guard = CONFIG_LOCK.lock().map_err(|_| Error::Disco)?;
        let mut value = documento(&path)?;
        if value
            .get("schema")
            .and_then(|v| v.as_u64())
            .is_some_and(|n| n > 2)
        {
            return Err(Error::Datos);
        }
        let mut merged = value
            .get("configuracion")
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));
        let fields = merged.as_object_mut().ok_or(Error::Datos)?;
        fields.extend(settings.as_object().ok_or(Error::Datos)?.clone());
        value["configuracion"] = merged;
        let bytes = serde_json::to_vec_pretty(&value).map_err(|_| Error::Datos)?;
        if bytes.len() as u64 > MAX_METADATA {
            return Err(Error::Datos);
        }
        if fs::read(path.join("instance.json")).ok().as_deref()==Some(bytes.as_slice()){return Ok(());}
        escribir(&path.join("instance.json"), &bytes)
    }

    pub fn carpeta(&self, id: &str) -> Result<PathBuf, Error> {
        if !id_valido(id) {
            return Err(Error::Datos);
        }
        if let Some(path) = self
            .rutas
            .lock()
            .map_err(|_| Error::Disco)?
            .get(id)
            .cloned()
        {
            if regular(&path, true).is_ok() && leer_en(&path).is_ok_and(|i| i.id == id) {
                return Ok(path);
            }
        }
        let mut encontrada = None;
        for entry in fs::read_dir(&self.raiz).map_err(|_| Error::Disco)? {
            let entry = entry.map_err(|_| Error::Disco)?;
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            if leer_en(&entry.path()).is_ok_and(|i| i.id == id) {
                if encontrada.is_some() {
                    return Err(Error::Datos);
                }
                encontrada = Some(entry.path());
            }
        }
        let path = encontrada.ok_or(Error::Disco)?;
        self.rutas
            .lock()
            .map_err(|_| Error::Disco)?
            .insert(id.into(), path.clone());
        Ok(path)
    }
    pub fn juego(&self, id: &str) -> Result<PathBuf, Error> {
        let path = self.carpeta(id)?.join("minecraft");
        let metadata = fs::symlink_metadata(&path).map_err(|_| Error::Disco)?;
        if !metadata.is_dir() || enlace(&metadata) {
            return Err(Error::Datos);
        }
        Ok(path)
    }
    pub(crate) fn leer(&self, id: &str) -> Result<Instancia, Error> {
        let i = leer_en(&self.carpeta(id)?)?;
        if i.id != id {
            return Err(Error::Datos);
        }
        Ok(i)
    }
    pub fn cargar(&self) -> Result<Informe, Error> {
        let entries = match fs::read_dir(&self.raiz) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Informe {
                    instancias: vec![],
                    omitidas: 0,
                })
            }
            Err(_) => return Err(Error::Disco),
        };
        let mut informe = Informe {
            instancias: vec![],
            omitidas: 0,
        };
        let mut registros = Vec::new();
        let mut ids = std::collections::BTreeSet::new();
        for entry in entries {
            let entry = entry.map_err(|_| Error::Disco)?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.')
                || (!id_valido(&name)
                    && !entry.path().join("instance.json").exists()
                    && !entry.path().join("instance.cfg").exists())
            {
                continue;
            }
            match leer_en(&entry.path()) {
                Ok(i) if ids.insert(i.id.clone()) => registros.push((entry.path(), i)),
                Ok(_) => informe.omitidas += 1,
                Err(_) => informe.omitidas += 1,
            }
        }
        let mut rutas = BTreeMap::new();
        for (mut path, i) in registros {
            // Una migración es un rename dentro del mismo almacén: no copia ni borra mundos.
            // Si el sistema tiene la carpeta abierta, la ruta antigua sigue siendo utilizable.
            if path.file_name() == Some(std::ffi::OsStr::new(&i.id)) {
                if let Ok(destino) = self.destino(&i.nombre, Some(&path)) {
                    if guardar_en(&path, &i).is_ok() && fs::rename(&path, &destino).is_ok() {
                        path = destino;
                    }
                }
            }
            let _ = guardar_en(&path, &i);
            rutas.insert(i.id.clone(), path);
            informe.instancias.push(i);
        }
        *self.rutas.lock().map_err(|_| Error::Disco)? = rutas;
        informe
            .instancias
            .sort_by(|a, b| a.creada.cmp(&b.creada).then_with(|| a.id.cmp(&b.id)));
        Ok(informe)
    }
    pub fn crear(&self, nombre: &str, minecraft: &str, portada: u8) -> Result<Instancia, Error> {
        self.crear_con_loader(nombre, minecraft, "vanilla", portada)
    }
    pub fn crear_con_loader(
        &self,
        nombre: &str,
        minecraft: &str,
        loader: &str,
        portada: u8,
    ) -> Result<Instancia, Error> {
        let i = Instancia {
            id: id_nuevo()?,
            nombre: nombre.trim().into(),
            minecraft: minecraft.into(),
            loader: loader.to_lowercase(),
            portada,
            creada: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| Error::Datos)?
                .as_nanos()
                .try_into()
                .map_err(|_| Error::Datos)?,
        };
        validar(&i)?;
        if self
            .cargar()?
            .instancias
            .iter()
            .any(|r| r.nombre.to_lowercase() == i.nombre.to_lowercase())
        {
            return Err(Error::Repetida);
        }
        fs::create_dir_all(&self.raiz).map_err(|_| Error::Disco)?;
        let path = self.destino(&i.nombre, None)?;
        fs::create_dir(&path).map_err(|_| Error::Disco)?;
        let result = fs::create_dir(path.join("minecraft"))
            .map_err(|_| Error::Disco)
            .and_then(|_| guardar_en(&path, &i));
        if let Err(error) = result {
            // La carpeta acaba de ser creada por esta operación y aún no contiene datos de juego.
            let _ = fs::remove_file(path.join("instance.cfg"));
            let _ = fs::remove_file(path.join("instance.json"));
            let _ = fs::remove_dir(path.join("minecraft"));
            let _ = fs::remove_dir(&path);
            return Err(error);
        }
        self.rutas
            .lock()
            .map_err(|_| Error::Disco)?
            .insert(i.id.clone(), path);
        Ok(i)
    }
    pub fn guardar(&self, i: &Instancia) -> Result<(), Error> {
        validar(i)?;
        if self
            .cargar()?
            .instancias
            .iter()
            .any(|r| r.id != i.id && r.nombre.to_lowercase() == i.nombre.to_lowercase())
        {
            return Err(Error::Repetida);
        }
        let path = self.carpeta(&i.id)?;
        let anterior = leer_en(&path)?;
        let destino = if anterior.nombre != i.nombre {
            self.destino(&i.nombre, Some(&path))?
        } else {
            path.clone()
        };
        if destino != path {
            fs::rename(&path, &destino).map_err(|_| Error::Disco)?;
        }
        if let Err(error) = guardar_en(&destino, i) {
            if destino != path {
                let _ = fs::rename(&destino, &path);
            }
            return Err(error);
        }
        self.rutas
            .lock()
            .map_err(|_| Error::Disco)?
            .insert(i.id.clone(), destino);
        Ok(())
    }

    fn papelera(&self) -> Result<PathBuf, Error> {
        let root = fs::canonicalize(&self.raiz).map_err(|_| Error::Disco)?;
        let path = root.join(".papelera");
        match fs::create_dir(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(_) => return Err(Error::Disco),
        }
        let metadata = fs::symlink_metadata(&path).map_err(|_| Error::Disco)?;
        if !metadata.is_dir() || enlace(&metadata) {
            return Err(Error::Datos);
        }
        let resolved = fs::canonicalize(&path).map_err(|_| Error::Disco)?;
        if resolved.parent() != Some(root.as_path()) {
            return Err(Error::Datos);
        }
        Ok(resolved)
    }

    /// Envía la instancia a la papelera del sistema; nunca usa borrado permanente.
    pub fn eliminar(&self, id: &str) -> Result<(), Error> {
        let record = self.leer(id)?;
        let root = fs::canonicalize(&self.raiz).map_err(|_| Error::Disco)?;
        let source = fs::canonicalize(self.carpeta(id)?).map_err(|_| Error::Disco)?;
        if source.parent() != Some(root.as_path()) {
            return Err(Error::Datos);
        }
        let ticket = crate::papelera::enviar(&source).map_err(Error::Papelera)?;
        self.eliminadas
            .lock()
            .map_err(|_| Error::Disco)?
            .insert(id.into(), (ticket, record));
        self.rutas.lock().map_err(|_| Error::Disco)?.remove(id);
        Ok(())
    }

    pub fn restaurar(&self, id: &str) -> Result<Instancia, Error> {
        if !id_valido(id) {
            return Err(Error::Datos);
        }
        let ticket = self
            .eliminadas
            .lock()
            .map_err(|_| Error::Disco)?
            .get(id)
            .cloned();
        if let Some((ticket, record)) = ticket {
            if self.cargar()?.instancias.iter().any(|i| i.nombre.to_lowercase() == record.nombre.to_lowercase()) {
                return Err(Error::Repetida);
            }
            crate::papelera::restaurar(&ticket).map_err(Error::Papelera)?;
            let record = leer_en(&ticket.original)?;
            if record.id != id {
                return Err(Error::Datos);
            }
            self.rutas
                .lock()
                .map_err(|_| Error::Disco)?
                .insert(id.into(), ticket.original);
            self.eliminadas.lock().map_err(|_| Error::Disco)?.remove(id);
            return Ok(record);
        }
        let trash = self.papelera()?;
        let record = leer_en(&trash.join(id))?;
        if record.id != id {
            return Err(Error::Datos);
        }
        if self
            .cargar()?
            .instancias
            .iter()
            .any(|i| i.nombre.to_lowercase() == record.nombre.to_lowercase())
        {
            return Err(Error::Repetida);
        }
        let source = fs::canonicalize(trash.join(id)).map_err(|_| Error::Disco)?;
        if source.parent() != Some(trash.as_path()) {
            return Err(Error::Datos);
        }
        let target = self.destino(&record.nombre, None)?;
        if fs::symlink_metadata(&target).is_ok() {
            return Err(Error::Repetida);
        }
        fs::rename(source, &target).map_err(|_| Error::Disco)?;
        self.rutas
            .lock()
            .map_err(|_| Error::Disco)?
            .insert(id.into(), target);
        Ok(record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Prueba {
        raiz: PathBuf,
    }
    impl Prueba {
        fn nueva() -> Self {
            Self {
                raiz: std::env::temp_dir()
                    .join(format!("icaro-instancias-test-{}", id_nuevo().unwrap())),
            }
        }
        fn almacen(&self) -> Almacen {
            Almacen::nuevo(&self.raiz)
        }
    }
    impl Drop for Prueba {
        fn drop(&mut self) {
            if self.raiz.parent() == Some(std::env::temp_dir().as_path()) {
                let _ = fs::remove_dir_all(&self.raiz);
            }
        }
    }
    #[test]
    fn loader_persists_and_older_metadata_defaults_to_vanilla() {
        let p = Prueba::nueva();
        let store = p.almacen();
        let fabric = store
            .crear_con_loader("Fabric", "1.21.1", "fabric", 0)
            .unwrap();
        let vanilla = store.crear("Vanilla", "1.21.1", 1).unwrap();
        let path = store.carpeta(&vanilla.id).unwrap().join("instance.json");
        let mut value = serde_json::to_value(&vanilla).unwrap();
        value.as_object_mut().unwrap().remove("loader");
        fs::write(path, serde_json::to_vec(&value).unwrap()).unwrap();
        let loaded = store.cargar().unwrap().instancias;
        assert_eq!(
            loaded.iter().find(|i| i.id == fabric.id).unwrap().loader,
            "fabric"
        );
        assert_eq!(
            loaded.iter().find(|i| i.id == vanilla.id).unwrap().loader,
            "vanilla"
        );
        assert!(store
            .crear_con_loader("Bad", "1.21.1", "../outside", 0)
            .is_err());
    }
    #[test]
    fn create_reopen_rename_and_game_folders_are_independent() {
        let p = Prueba::nueva();
        let store = p.almacen();
        let mut a = store.crear("Mi mundo", "1.21.4", 2).unwrap();
        let b = store.crear("Otro", "1.20.1", 0).unwrap();
        fs::write(
            store.juego(&a.id).unwrap().join("options.txt"),
            b"my options",
        )
        .unwrap();
        assert!(!store.juego(&b.id).unwrap().join("options.txt").exists());
        assert_eq!(p.almacen().cargar().unwrap().instancias, vec![a.clone(), b]);
        assert!(matches!(
            store.crear("MI MUNDO", "1.21.4", 0),
            Err(Error::Repetida)
        ));
        let game = store.juego(&a.id).unwrap();
        a.nombre = "Nombre nuevo".into();
        store.guardar(&a).unwrap();
        assert_ne!(store.juego(&a.id).unwrap(), game);
        assert_eq!(
            store.carpeta(&a.id).unwrap().file_name().unwrap(),
            "Nombre nuevo"
        );
        assert_eq!(
            fs::read(store.juego(&a.id).unwrap().join("options.txt")).unwrap(),
            b"my options"
        );
        assert_eq!(store.cargar().unwrap().instancias[0], a);
    }
    #[test]
    fn invalid_paths_and_corrupt_metadata_do_not_hide_valid_instances() {
        let p = Prueba::nueva();
        let store = p.almacen();
        assert!(matches!(store.crear("", "1.21.4", 0), Err(Error::Nombre)));
        assert!(store.crear("Mundo", "../../outside", 0).is_err());
        assert!(!p.raiz.exists());
        let good = store.crear("Mundo", "1.21.4", 0).unwrap();
        let bad = store.crear("Otra", "1.21.4", 0).unwrap();
        fs::write(
            store.carpeta(&bad.id).unwrap().join("instance.json"),
            b"broken",
        )
        .unwrap();
        let report = store.cargar().unwrap();
        assert_eq!(report.instancias, vec![good]);
        assert_eq!(report.omitidas, 1);
        assert!(store.carpeta("../outside").is_err());
    }

    #[test]
    fn delete_and_restore_keep_files_and_do_not_touch_other_instances() {
        let p = Prueba::nueva();
        let store = p.almacen();
        let a = store.crear("Mundo", "1.21.4", 0).unwrap();
        let b = store.crear("Otra", "1.20.1", 1).unwrap();
        fs::write(
            store.juego(&a.id).unwrap().join("options.txt"),
            b"saved options",
        )
        .unwrap();
        assert!(store.eliminar("../outside").is_err());
        store.eliminar(&a.id).unwrap();
        assert_eq!(store.cargar().unwrap().instancias, vec![b.clone()]);
        assert!(!p.raiz.join(&a.id).exists());
        assert!(!p.raiz.join(".papelera").exists());
        assert_eq!(store.restaurar(&a.id).unwrap(), a);
        assert_eq!(store.cargar().unwrap().instancias, vec![a.clone(), b]);
        assert_eq!(
            fs::read(store.juego(&a.id).unwrap().join("options.txt")).unwrap(),
            b"saved options"
        );
        store.eliminar(&a.id).unwrap();
        let replacement = store.crear("MUNDO", "1.21.4", 2).unwrap();
        assert!(store.restaurar(&a.id).is_err());
        assert!(store.juego(&replacement.id).unwrap().exists());
        fs::remove_dir_all(store.carpeta(&replacement.id).unwrap()).unwrap();
        store.restaurar(&a.id).unwrap();
    }

    #[test]
    fn invalid_trash_does_not_remove_the_active_instance() {
        let p = Prueba::nueva();
        let store = p.almacen();
        let a = store.crear("Mundo", "1.21.4", 0).unwrap();
        fs::write(p.raiz.join(".papelera"), b"not a directory").unwrap();
        assert!(store.restaurar(&a.id).is_err());
        assert_eq!(store.cargar().unwrap().instancias, vec![a]);
        assert!(store.restaurar("../outside").is_err());
    }

    #[test]
    fn readable_paths_collisions_reserved_names_and_cfg_fallback_keep_identity() {
        let p = Prueba::nueva();
        let store = p.almacen();
        let a = store.crear("a:b", "1.21.4", 0).unwrap();
        let b = store.crear("a?b", "1.21.4", 0).unwrap();
        let con = store.crear("CON", "1.21.4", 0).unwrap();
        assert_eq!(store.carpeta(&a.id).unwrap().file_name().unwrap(), "a_b");
        assert_eq!(
            store.carpeta(&b.id).unwrap().file_name().unwrap(),
            "a_b (2)"
        );
        assert_eq!(
            store.carpeta(&con.id).unwrap().file_name().unwrap(),
            "Instancia CON"
        );
        let path = store.carpeta(&a.id).unwrap();
        fs::write(path.join("instance.cfg"), cfg(&a).unwrap()).unwrap();
        fs::remove_file(path.join("instance.json")).unwrap();
        assert_eq!(p.almacen().leer(&a.id).unwrap(), a);
        store.cargar().unwrap();
        assert!(path.join("instance.json").exists());
        assert!(!path.join("instance.cfg").exists());
    }

    #[test]
    fn migrate_legacy_directory_once_preserves_worlds_and_uses_collision_suffix() {
        let p = Prueba::nueva();
        let store = p.almacen();
        let a = store.crear("Mi mundo", "1.21.4", 0).unwrap();
        let path = store.carpeta(&a.id).unwrap();
        fs::create_dir_all(path.join("minecraft/saves/world")).unwrap();
        fs::write(path.join("minecraft/saves/world/level.dat"), b"world").unwrap();
        let _ = fs::remove_file(path.join("instance.cfg"));
        let mut metadata = serde_json::to_value(&a).unwrap();
        metadata["dato_de_otra_version"] = serde_json::json!({"conservar":true});
        let legacy = serde_json::to_vec_pretty(&metadata).unwrap();
        fs::write(path.join("instance.json"), &legacy).unwrap();
        fs::rename(path, p.raiz.join(&a.id)).unwrap();
        fs::create_dir(p.raiz.join("Mi mundo")).unwrap();
        let fresh = p.almacen();
        assert_eq!(fresh.cargar().unwrap().instancias, vec![a.clone()]);
        let new = fresh.carpeta(&a.id).unwrap();
        assert_eq!(new.file_name().unwrap(), "Mi mundo (2)");
        assert_eq!(
            fs::read(new.join("minecraft/saves/world/level.dat")).unwrap(),
            b"world"
        );
        assert!(!new.join("instance.cfg").exists());
        let updated: serde_json::Value =
            serde_json::from_slice(&fs::read(new.join("instance.json")).unwrap()).unwrap();
        assert_eq!(
            updated["dato_de_otra_version"],
            metadata["dato_de_otra_version"]
        );
        assert_eq!(updated["schema"], 2);
        assert!(!p.raiz.join(&a.id).exists());
        assert_eq!(fresh.cargar().unwrap().instancias, vec![a.clone()]);
        assert_eq!(fresh.carpeta(&a.id).unwrap(), new);
    }

    #[test]
    fn failed_migration_or_rename_leaves_original_metadata_and_files_available() {
        let p = Prueba::nueva();
        let store = p.almacen();
        let a = store.crear("Original", "1.21.4", 0).unwrap();
        let path = store.carpeta(&a.id).unwrap();
        let _ = fs::remove_file(path.join("instance.cfg"));
        fs::create_dir(path.join("instance.cfg")).unwrap();
        fs::write(path.join("minecraft/options.txt"), b"untouched").unwrap();
        let mut renamed = a.clone();
        renamed.nombre = "Renombrada".into();
        assert!(store.guardar(&renamed).is_err());
        assert!(path.is_dir());
        assert!(!p.raiz.join("Renombrada").exists());
        assert_eq!(store.leer(&a.id).unwrap(), a);
        fs::rename(&path, p.raiz.join(&a.id)).unwrap();
        assert_eq!(store.cargar().unwrap().instancias, vec![a.clone()]);
        assert_eq!(store.carpeta(&a.id).unwrap(), p.raiz.join(&a.id));
        assert_eq!(
            fs::read(store.juego(&a.id).unwrap().join("options.txt")).unwrap(),
            b"untouched"
        );
    }
}
