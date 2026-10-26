// SPDX-License-Identifier: GPL-3.0-only
//! Origen de solo lectura: mods de un servidor dedicado, sin Almacen ni instancia.
use super::*;
use std::io::Read;

/// Inventario de cada carpeta de mods servida. El servicio es de larga duración:
/// tras la primera lectura, el sondeo de diez segundos solo relee lo que cambia.
static INVENTARIOS: std::sync::Mutex<BTreeMap<PathBuf, super::inventario::Inventario>> =
    std::sync::Mutex::new(BTreeMap::new());

/// SHA-512 de `mods/<archivo>` si sus metadatos siguen siendo los ya verificados.
/// Permite servirlo por bloques sin releerlo entero en cada petición.
pub(super) fn sha_vigente(mods: &Path, archivo: &str) -> Option<String> {
    let meta = fs::symlink_metadata(mods.join(archivo)).ok()?;
    if enlace(&meta) || !meta.is_file() {
        return None;
    }
    let estado = estado_archivo(archivo.to_owned(), &meta);
    let inventarios = INVENTARIOS.lock().ok()?;
    inventarios.get(mods)?.sha_vigente(&estado).map(str::to_owned)
}

#[derive(Debug, Clone)]
pub struct ServidorDedicado {
    pub carpeta: PathBuf,
    pub minecraft: String,
    pub loader: String,
    pub loader_version: String,
    pub excluir: BTreeSet<String>,
    /// JAR que se publican aunque Modrinth o su descriptor los declaren
    /// exclusivos de servidor.
    pub incluir: BTreeSet<String>,
    /// Publica todos los JAR exclusivos de servidor para reproducir `mods` completa.
    pub incluir_servidor: bool,
}
impl ServidorDedicado {
    pub fn validar(&self) -> Result<(), String> {
        let jar = |n: &String| nombre_valido(n) && n.to_ascii_lowercase().ends_with(".jar");
        if !self.excluir.iter().all(jar) {
            return Err("Las exclusiones deben ser nombres de JAR, sin rutas.".into());
        }
        if !self.incluir.iter().all(jar) {
            return Err("Las inclusiones deben ser nombres de JAR, sin rutas.".into());
        }
        if let Some(n) = self.incluir.intersection(&self.excluir).next() {
            return Err(format!("{n} no puede incluirse y excluirse a la vez."));
        }
        if !self.carpeta.is_absolute() {
            return Err("La carpeta del servidor debe ser absoluta.".into());
        }
        self.indice(vec![]).validar()?;
        regular(&self.carpeta, true).map_err(err)?;
        regular(&self.carpeta.join("mods"), true)
            .map_err(|_| "El servidor debe tener una carpeta mods legible.".to_string())?;
        Ok(())
    }
    /// El anfitrión pidió publicar este JAR aunque sea exclusivo de servidor.
    fn incluye(&self, nombre: &str) -> bool {
        self.incluir_servidor || self.incluir.contains(nombre)
    }
    fn indice(&self, mods: Vec<Entrada>) -> Indice {
        Indice {
            esquema: 1,
            minecraft: self.minecraft.clone(),
            loader: self.loader.clone(),
            loader_version: Some(self.loader_version.clone()),
            mods,
            archivos: vec![],
            avisos: vec![],
        }
    }
    pub fn huella(&self) -> Result<Vec<(String, String)>, String> {
        self.validar()?;
        let dir = self.carpeta.join("mods");
        let estado = estado(&dir)?;
        let archivos = {
            let mut inventarios = INVENTARIOS.lock().map_err(|_| "No se pudo leer el inventario de mods.")?;
            let inventario = inventarios.entry(dir.clone()).or_default();
            inventario.leer(&dir, &estado, &CancellationToken::new())?.huella
        };
        if archivos
            .iter()
            .any(|(n, _)| n.to_ascii_lowercase().ends_with(".jar") && n.contains(['/', '\\']))
        {
            return Err("Hay JAR en subcarpetas de mods. Coloca los mods activos directamente en mods para publicarlos.".into());
        }
        Ok(archivos)
    }
}

trait Catalogo {
    async fn hashes(&self, hashes: &[String]) -> Result<BTreeMap<String, Version>, String>;
    async fn cliente(&self, version: &Version) -> Result<bool, String>;
    async fn version(&self, id: &str) -> Result<Version, String>;
    async fn versiones(
        &self,
        id: &str,
        minecraft: &str,
        loader: &str,
    ) -> Result<Vec<Version>, String>;
}
impl Catalogo for Catalog {
    async fn hashes(&self, hashes: &[String]) -> Result<BTreeMap<String, Version>, String> {
        self.versions_from_hashes(hashes).await.map_err(err)
    }
    async fn cliente(&self, version: &Version) -> Result<bool, String> {
        let project = self.project(&version.project_id).await.map_err(err)?;
        if project.id != version.project_id { return Err("Modrinth devolvió otro proyecto.".into()); }
        match project.client_side.as_str() {
            "unsupported" => Ok(false),
            "required" | "optional" => Ok(true),
            _ => Err(format!(
                "Modrinth no confirma si {} puede ejecutarse en el cliente.",
                version.project_id
            )),
        }
    }
    async fn version(&self, id: &str) -> Result<Version, String> {
        self.version(id).await.map_err(err)
    }
    async fn versiones(
        &self,
        id: &str,
        minecraft: &str,
        loader: &str,
    ) -> Result<Vec<Version>, String> {
        self.versions(id, Some(minecraft), Some(loader))
            .await
            .map_err(err)
    }
}

// Un Fabric exclusivo de servidor puede ser privado y no estar en Modrinth.
// Solo se excluye si su propio descriptor lo declara explícitamente.
pub(super) fn descriptor_fabric(
    path: &Path,
    avisos: &mut Vec<String>,
) -> Result<Option<serde_json::Value>, String> {
    let nombre = path.file_name().unwrap_or_default().to_string_lossy();
    let aviso = |detalle: &str, avisos: &mut Vec<String>| {
        let mensaje = format!("{nombre}: {detalle}");
        if !avisos.contains(&mensaje) {
            avisos.push(mensaje);
        }
    };
    let file =
        fs::File::open(path).map_err(|e| format!("{nombre}: no se pudo leer el JAR: {e}"))?;
    let Ok(mut jar) = zip::ZipArchive::new(file) else {
        return Ok(None);
    };
    let Ok(mut descriptor) = jar.by_name("fabric.mod.json") else {
        return Ok(None);
    };
    if descriptor.size() > 128 * 1024 {
        aviso(
            "fabric.mod.json supera el límite; no se pudo confirmar su entorno o identidad.",
            avisos,
        );
        return Ok(None);
    }
    let mut bytes = vec![];
    if descriptor
        .by_ref()
        .take(128 * 1024 + 1)
        .read_to_end(&mut bytes)
        .is_err()
        || bytes.len() > 128 * 1024
    {
        aviso("fabric.mod.json no se pudo leer dentro del límite; revisa si el mod es necesario en el cliente.",avisos);
        return Ok(None);
    }
    let strict = serde_json::from_slice::<serde_json::Value>(&bytes);
    let data = match strict {
        Ok(data) => data,
        Err(_) => {
            // Solo la copia en memoria se normaliza. Los escapes válidos y el JAR quedan intactos.
            let normalizada: Vec<u8> = bytes
                .iter()
                .map(|b| if *b < 0x20 { b' ' } else { *b })
                .collect();
            match serde_json::from_slice::<serde_json::Value>(&normalizada) {
                Ok(data) => {
                    aviso("se toleraron caracteres de control sin escapar en fabric.mod.json; el JAR original se conserva.",avisos);
                    data
                }
                Err(_) => {
                    aviso("fabric.mod.json ilegible; no se pudo confirmar su entorno o identidad. Revisa si el mod es necesario en el cliente.",avisos);
                    return Ok(None);
                }
            }
        }
    };
    if !data.is_object() {
        aviso(
            "fabric.mod.json no contiene un objeto; no se pudo confirmar su entorno o identidad.",
            avisos,
        );
        return Ok(None);
    }
    Ok(Some(data))
}
fn exclusivo_servidor(path: &Path, loader: &str, avisos: &mut Vec<String>) -> Result<bool, String> {
    if !matches!(loader, "fabric" | "quilt") {
        return Ok(false);
    }
    Ok(descriptor_fabric(path, avisos)?
        .is_some_and(|data| data.get("environment").and_then(|v| v.as_str()) == Some("server")))
}

pub async fn publicar(catalog: &Catalog, servidor: &ServidorDedicado) -> Result<Indice, String> {
    publicar_con(catalog, servidor).await
}
async fn publicar_con(
    catalog: &impl Catalogo,
    servidor: &ServidorDedicado,
) -> Result<Indice, String> {
    let before = servidor.huella()?;
    let mut active = vec![];
    let mut avisos = vec![];
    for (name, sha) in &before {
        if !name.to_ascii_lowercase().ends_with(".jar") || servidor.excluir.contains(name) {
            continue;
        }
        if exclusivo_servidor(&servidor.carpeta.join("mods").join(name), &servidor.loader, &mut avisos)? {
            if !servidor.incluye(name) {
                continue;
            }
            avisos.push(format!("{name}: declara environment: server, pero se publica por indicación del anfitrión. Fabric no lo cargará en el cliente."));
        }
        active.push((name, sha));
    }
    for name in servidor.incluir.iter().filter(|n| !before.iter().any(|(b, _)| b == *n)) {
        avisos.push(format!("{name}: se pidió incluirlo, pero no está en mods."));
    }
    if active.len() > 512 {
        return Err("El servidor supera 512 mods.".into());
    }
    let hashes: Vec<_> = active.iter().map(|(_, h)| (*h).clone()).collect();
    let by_hash = if hashes.is_empty() {
        BTreeMap::new()
    } else {
        catalog.hashes(&hashes).await?
    };
    let mut encontrados = BTreeMap::new();
    let mut jars = BTreeMap::new();
    let mut propios = BTreeSet::new();
    let mut archivos = vec![];
    let mut elegidos: BTreeMap<String, (Version, Entrada)> = BTreeMap::new();
    for (name, sha) in active {
        let Some(v) = by_hash.get(sha) else {
            archivos.push(archivo_propio(
                &servidor.carpeta.join("mods").join(name), name, sha, &mut avisos,
            )?);
            avisos.push(format!("{name}: no está en Modrinth; se descargará directamente del anfitrión. Revisa que sea necesario en el cliente."));
            continue;
        };
        if !v.files.iter().any(|f| f.hashes.sha512.eq_ignore_ascii_case(sha)) {
            return Err(format!("Modrinth no confirma el archivo por SHA-512 de {name}."));
        }
        if encontrados.insert(v.project_id.clone(), v.clone()).is_some() {
            return Err(format!("Hay dos archivos del proyecto {} en mods.", v.project_id));
        }
        // Conservar la ruta incluso si la clasificación del catálogo descarta
        // el proyecto: otro mod puede exigir esta biblioteca en el cliente.
        jars.insert(v.project_id.clone(), (name.clone(), sha.clone()));
        if !catalog.cliente(v).await? {
            if !servidor.incluye(name) {
                continue;
            }
            avisos.push(format!("{name}: Modrinth lo clasifica como exclusivo de servidor, pero se publica por indicación del anfitrión."));
        }
        if !compatible(v, &servidor.minecraft, &servidor.loader) {
            propios.insert(v.project_id.clone());
            avisos.push(format!("{name}: Modrinth reconoce el hash, pero no declara Minecraft {} y loader {}. Se publicará el JAR instalado como archivo propio; revisa su compatibilidad en el cliente.",servidor.minecraft,servidor.loader));
        }
        elegidos.insert(v.project_id.clone(), (v.clone(), Entrada {
            proyecto: v.project_id.clone(), version: v.id.clone(), sha512: sha.clone(),
        }));
    }
    // Resolver todo el grafo antes de decidir su transporte. También los JAR propios
    // identificados pueden necesitar dependencias de cliente ausentes en el servidor.
    let mut aristas: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut revisados = BTreeSet::new();
    loop {
        let Some(v) = elegidos.values().map(|(v, _)| v)
            .find(|v| !revisados.contains(&v.id)).cloned() else { break; };
        revisados.insert(v.id.clone());
        for dep in &super::dependencias_cliente(&v) {
            // Un pin sin proyecto debe resolverse incluso si la versión instalada es otra.
            let pinned = if dep.project_id.is_none() {
                let id = dep.version_id.as_ref().ok_or_else(||
                    format!("{}: una dependencia obligatoria no tiene proyecto ni versión de Modrinth.", v.project_id))?;
                let value = catalog.version(id).await?;
                if value.id != *id { return Err("Modrinth devolvió otra versión de dependencia.".into()); }
                Some(value)
            } else { None };
            let project = dep.project_id.as_ref()
                .or_else(|| pinned.as_ref().map(|p| &p.project_id)).unwrap().clone();
            let existente = encontrados.get(&project);
            let candidate = if let Some(value) = existente {
                value.clone()
            } else if let Some(value) = pinned {
                value
            } else if let Some(id) = &dep.version_id {
                let value = catalog.version(id).await?;
                if value.id != *id { return Err("Modrinth devolvió otra versión de dependencia.".into()); }
                value
            } else if let Some((value, _)) = elegidos.get(&project) {
                value.clone()
            } else {
                let mut versions = catalog.versiones(&project, &servidor.minecraft, &servidor.loader).await?;
                versions.sort_by(|a, b| b.date_published.cmp(&a.date_published).then(a.id.cmp(&b.id)));
                versions.into_iter().find(|value| value.project_id == project
                    && value.version_type == "release"
                    && compatible(value, &servidor.minecraft, &servidor.loader))
                    .ok_or_else(|| format!("No hay una dependencia estable compatible para {project}."))?
            };
            if candidate.project_id != project {
                return Err(format!("Conflicto en una dependencia de {}: el catálogo devolvió otro proyecto.", v.project_id));
            }
            let incluida = jars.get(&candidate.project_id).is_some_and(|(n, _)| servidor.incluye(n));
            if !incluida && !catalog.cliente(&candidate).await? {
                let descriptor = if matches!(servidor.loader.as_str(), "fabric" | "quilt") {
                    if let Some((name, _)) = jars.get(&candidate.project_id) {
                        descriptor_fabric(&servidor.carpeta.join("mods").join(name), &mut avisos)?
                    } else { None }
                } else { None };
                let admite_cliente = descriptor.as_ref().is_some_and(|d| {
                    // Fabric usa ambos entornos cuando no se declara environment.
                    d.get("environment").is_none()
                        || matches!(d.get("environment").and_then(|v| v.as_str()), Some("*" | "client"))
                });
                if admite_cliente {
                    avisos.push(format!("{}: se incluye la dependencia {} porque el JAR instalado admite cliente, aunque Modrinth la clasifica como exclusiva de servidor.", v.project_id, candidate.project_id));
                } else {
                    let (name, _) = jars.get(&v.project_id).ok_or_else(|| format!(
                        "{} requiere una dependencia exclusiva de servidor y no tiene un JAR instalado para publicarse como propio.",v.project_id))?;
                    propios.insert(v.project_id.clone());
                    avisos.push(format!("{name}: Modrinth exige {} como dependencia, pero la clasifica como exclusiva de servidor. Se conserva el JAR instalado como archivo propio y se excluye esa dependencia del cliente.",candidate.project_id));
                    continue;
                }
            }
            if dep.version_id.as_ref().is_some_and(|id| id != &candidate.id) {
                if dep.project_id.is_some() {
                    let id = dep.version_id.as_deref().unwrap();
                    let declared = catalog.version(id).await?;
                    if declared.id != id || declared.project_id != candidate.project_id {
                        return Err(format!("Conflicto en una dependencia de {}: el pin no pertenece al proyecto declarado.",v.project_id));
                    }
                }
                // El servidor aporta ambos archivos exactos. El pin del catálogo no es
                // un rango de Fabric: conservar el conjunto y advertir, sin certificarlo.
                if let (Some((name, _)), Some((dependency, _))) =
                    (jars.get(&v.project_id), jars.get(&candidate.project_id)) {
                    propios.insert(v.project_id.clone());
                    avisos.push(format!("{name}: Modrinth fija {} para {}, pero el servidor tiene {} ({dependency}). Se conserva el conjunto instalado y se publica {name} como archivo propio; revisa su compatibilidad en el cliente.",dep.version_id.as_deref().unwrap(),candidate.project_id,candidate.id));
                } else {
                    return Err(format!("Conflicto en una dependencia de {}: {} requiere {}, disponible {}. No hay ambos JAR instalados para conservar el conjunto del servidor.",v.project_id,candidate.project_id,dep.version_id.as_deref().unwrap(),candidate.id));
                }
            }
            aristas.entry(v.project_id.clone()).or_default().insert(candidate.project_id.clone());
            if let Some((prev, _)) = elegidos.get(&candidate.project_id) {
                if prev.id != candidate.id {
                    return Err(format!("Dependencias incompatibles para {}: {} y {}.",candidate.project_id,prev.id,candidate.id));
                }
                continue;
            }
            if !compatible(&candidate, &servidor.minecraft, &servidor.loader) {
                return Err(format!("Una dependencia de {} no confirma Minecraft o loader y no hay un JAR instalado que pueda publicarse como propio.",v.project_id));
            }
            let file = candidate.files.iter().find(|f| f.primary && f.filename.ends_with(".jar"))
                .or_else(|| candidate.files.iter().find(|f| f.filename.ends_with(".jar")))
                .ok_or("La dependencia no tiene un JAR de cliente.")?;
            if !nombre_valido(&file.filename) || file.size > MAX_FILE {
                return Err("Archivo de dependencia inválido.".into());
            }
            let entry = Entrada { proyecto: candidate.project_id.clone(), version: candidate.id.clone(), sha512: file.hashes.sha512.clone() };
            elegidos.insert(candidate.project_id.clone(), (candidate, entry));
            if elegidos.len() + archivos.len() > 512 {
                return Err("Las dependencias superan 512 mods.".into());
            }
        }
    }
    // El cliente valida los pins de cada entrada de Modrinth. Propagar hasta un punto
    // fijo evita publicar un índice que el anfitrión acepta pero el cliente rechaza.
    loop {
        let afectados: Vec<_> = aristas.iter().filter(|(id, deps)|
            !propios.contains(*id) && deps.iter().any(|d| propios.contains(d)))
            .map(|(id, _)| id.clone()).collect();
        if afectados.is_empty() { break; }
        for id in afectados {
            let (name, _) = jars.get(&id).ok_or_else(|| format!(
                "La dependencia de cliente {id} necesita un archivo propio, pero su JAR no está instalado en el servidor."))?;
            avisos.push(format!("{name}: depende de un JAR con metadatos incompletos o pins distintos; se publicará el archivo instalado del anfitrión."));
            propios.insert(id);
        }
    }
    for id in propios {
        let (name, sha) = &jars[&id];
        archivos.push(archivo_propio(&servidor.carpeta.join("mods").join(name),name,sha,&mut avisos)?);
        elegidos.remove(&id);
    }
    // No entregar al launcher un conflicto explícito entre las entradas de catálogo.
    for (v, _) in elegidos.values() {
        for dep in v.dependencies.iter().filter(|d| d.dependency_type == "incompatible") {
            if (dep.project_id.is_some() || dep.version_id.is_some()) && elegidos.values().any(|(other, _)|
                dep.project_id.as_ref().is_none_or(|id| id == &other.project_id)
                && dep.version_id.as_ref().is_none_or(|id| id == &other.id)) {
                return Err(format!("{} declara una incompatibilidad con otro mod del índice.",v.project_id));
            }
        }
    }
    let mut index = servidor.indice(elegidos.into_values().map(|(_, entry)| entry).collect());
    index.archivos = archivos;
    index.avisos = avisos;
    if !index.archivos.is_empty() {
        index.esquema = 2;
    }
    index.validar()?;
    if before != servidor.huella()? {
        return Err(
            "Los mods del servidor cambiaron durante la publicación. Se volverán a consultar."
                .into(),
        );
    }
    Ok(index)
}

fn archivo_propio(
    path: &Path,
    name: &str,
    sha: &str,
    avisos: &mut Vec<String>,
) -> Result<ArchivoPropio, String> {
    let bytes = regular(path, false).map_err(err)?.len();
    let _jar = zip::ZipArchive::new(fs::File::open(path).map_err(err)?)
        .map_err(|_| format!("{name} no es un JAR válido."))?;
    let mut id = hash(name.to_lowercase().as_bytes())[..24].to_string();
    let mut nombre = name.to_string();
    let mut version = "Archivo del anfitrión".to_string();
    if let Some(data) = descriptor_fabric(path, avisos)? {
        if let Some(value) = data.get("id").and_then(|v| v.as_str()).filter(|v| {
            !v.is_empty()
                && v.len() <= 64
                && v.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        }) {
            id = value.to_string();
        }
        if let Some(value) = data.get("name").and_then(|v| v.as_str()) {
            nombre = value.to_string();
        }
        if let Some(value) = data.get("version").and_then(|v| v.as_str()) {
            version = value.to_string();
        }
    }
    Ok(ArchivoPropio {
        id: format!("icaro-host-{id}"),
        nombre,
        version,
        archivo: name.into(),
        bytes,
        sha512: sha.into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn jar_descriptor(descriptor: &[u8]) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        writer
            .start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        writer.write_all(descriptor).unwrap();
        writer.finish().unwrap().into_inner()
    }
    #[tokio::test]
    async fn descripciones_multilinea_y_descriptor_roto_no_bloquean_el_indice_ni_alteran_jars() {
        let (_temp, s) = servidor();
        let mut catalog = Fake::default();
        let debugify=jar_descriptor(b"{\"id\":\"debugify\",\"version\":\"1\",\"description\":\"linea uno\nlinea dos\tcon tab\",\"environment\":\"*\"}");
        let weapon=jar_descriptor(b"{\"id\":\"weaponmaster\",\"version\":\"4.4.2\",\"description\":\"uno\r\ndos\",\"environment\":\"*\"}");
        let roto = jar_descriptor(b"{\"id\":\"roto\",descripcion truncada");
        let server = jar_descriptor(
            b"{\"id\":\"backend\",\"description\":\"uno\ndos\",\"environment\":\"server\"}",
        );
        for (name, body) in [
            ("Debugify-1.21.1+1.0.jar", &debugify),
            ("weaponmaster_ydm-fabric-1.21.1-4.4.2.jar", &weapon),
            ("roto.jar", &roto),
            ("backend.jar", &server),
        ] {
            fs::write(s.carpeta.join("mods").join(name), body).unwrap();
        }
        // Un mod identificado y otro propio recorren la misma lectura tolerante.
        catalog.hashes.insert(
            hash(&debugify),
            version("debugify", "dbg1", &debugify, serde_json::json!([])),
        );
        let before = s.huella().unwrap();
        let index = publicar_con(&catalog, &s).await.unwrap();
        assert_eq!(index.mods.len(), 1);
        assert_eq!(index.archivos.len(), 2);
        let weapon_entry = index
            .archivos
            .iter()
            .find(|a| a.archivo.starts_with("weaponmaster"))
            .unwrap();
        assert_eq!(weapon_entry.id, "icaro-host-weaponmaster");
        assert_eq!(weapon_entry.sha512, hash(&weapon));
        assert!(index
            .avisos
            .iter()
            .any(|a| a.contains("Debugify-1.21.1+1.0.jar") && a.contains("se toleraron")));
        assert!(index
            .avisos
            .iter()
            .any(|a| a.starts_with("roto.jar:") && a.contains("ilegible")));
        assert_eq!(
            index
                .avisos
                .iter()
                .filter(|a| a.starts_with("roto.jar:") && a.contains("ilegible"))
                .count(),
            1
        );
        assert_eq!(before, s.huella().unwrap());
    }
    #[test]
    fn descriptor_valido_con_escapes_no_se_normaliza_y_tamano_excesivo_es_aviso() {
        let (_temp, s) = servidor();
        let path = s.carpeta.join("mods/prueba.jar");
        let mut avisos = vec![];
        fs::write(&path,jar_descriptor(br#"{"id":"test","description":"linea\nsegunda\t\"citada\"","environment":"server"}"#)).unwrap();
        let data = descriptor_fabric(&path, &mut avisos).unwrap().unwrap();
        assert_eq!(
            data["description"].as_str(),
            Some("linea\nsegunda\t\"citada\"")
        );
        assert!(avisos.is_empty());
        fs::write(&path, jar_descriptor(&vec![b' '; 128 * 1024 + 1])).unwrap();
        assert!(descriptor_fabric(&path, &mut avisos).unwrap().is_none());
        assert!(avisos[0].starts_with("prueba.jar:"));
    }
    #[tokio::test]
    #[ignore = "Descarga Fabric API real de Modrinth en un servidor temporal aislado"]
    async fn modrinth_real_publica_desde_carpeta_dedicada_y_el_cliente_verifica_el_indice() {
        let (_temp, s) = servidor();
        let catalog = Catalog::new().unwrap();
        let version = catalog
            .versions("fabric-api", Some(&s.minecraft), Some(&s.loader))
            .await
            .unwrap()
            .into_iter()
            .find(|v| v.version_type == "release")
            .unwrap();
        let file = version.files.iter().find(|f| f.primary).unwrap();
        let bytes = <Catalog as Fuente>::archivo(&catalog, file).await.unwrap();
        assert_eq!(hash(&bytes), file.hashes.sha512);
        fs::write(s.carpeta.join("mods").join(&file.filename), bytes).unwrap();
        let index = publicar(&catalog, &s).await.unwrap();
        assert!(index
            .mods
            .iter()
            .any(|e| e.version == version.id && e.sha512 == file.hashes.sha512));
        let store = Almacen::nuevo(s.carpeta.join("cliente/instances"));
        let instance = store
            .crear_con_loader("Cliente", &s.minecraft, &s.loader, 0)
            .unwrap();
        crate::versiones_instancia::guardar_loader(&store, &instance, &s.loader_version).unwrap();
        let plan =
            super::super::preparar(&catalog, &store, &instance, index, "127.0.0.1:25566".into())
                .await
                .unwrap();
        assert!(plan
            .cambios
            .iter()
            .any(|c| c.proyecto == version.project_id && c.tipo == Tipo::Agregar));
        super::super::aplicar(
            &catalog,
            &store,
            &instance,
            &plan,
            CancellationToken::new(),
            |_, _| {},
        )
        .await
        .unwrap();
        assert_eq!(
            hash_archivo(
                &store
                    .juego(&instance.id)
                    .unwrap()
                    .join("mods")
                    .join(&file.filename)
            )
            .unwrap(),
            file.hashes.sha512
        );
    }
    struct Temp(PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn servidor() -> (Temp, ServidorDedicado) {
        let mut random = [0; 16];
        getrandom::fill(&mut random).unwrap();
        let dir = std::env::temp_dir().join(format!("icaro-dedicado-{}", hash(&random)));
        fs::create_dir_all(dir.join("mods")).unwrap();
        let s = ServidorDedicado {
            carpeta: dir.clone(),
            minecraft: "1.21.1".into(),
            loader: "fabric".into(),
            loader_version: "0.16.14".into(),
            excluir: BTreeSet::new(),
            incluir: BTreeSet::new(),
            incluir_servidor: false,
        };
        (Temp(dir), s)
    }
    fn jar(environment: &str) -> Vec<u8> {
        jar_id(environment, "test")
    }
    fn jar_id(environment: &str, id: &str) -> Vec<u8> {
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        writer
            .start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        write!(writer, "{{\"schemaVersion\":1,\"id\":\"{id}\",\"version\":\"1\",\"environment\":\"{environment}\"}}").unwrap();
        writer.finish().unwrap().into_inner()
    }
    fn version(project: &str, id: &str, bytes: &[u8], dependencies: serde_json::Value) -> Version {
        serde_json::from_value(serde_json::json!({
            "id":id,"project_id":project,"name":project,"version_number":"1","version_type":"release",
            "game_versions":["1.21.1"],"loaders":["fabric"],"date_published":"2026-10-14T00:00:00Z",
            "files":[{"filename":format!("{project}.jar"),"url":format!("https://cdn.modrinth.com/data/{project}/versions/{id}/{project}.jar"),"size":bytes.len(),"primary":true,"hashes":{"sha512":hash(bytes)}}],
            "dependencies":dependencies
        })).unwrap()
    }
    #[tokio::test]
    #[ignore = "Descarga Surveyor 1.2.4, Fabric API 0.116.17 y sus dependencias reales; no inicia Minecraft"]
    async fn surveyor_real_con_fabric_instalada_publica_y_sincroniza_sin_reemplazarla() {
        let (_temp,mut s) = servidor();
        s.loader_version = "0.19.5".into();
        let catalog = Catalog::new().unwrap().with_metadata_cache();
        let surveyor = catalog.version("egJBsDTn").await.unwrap();
        assert_eq!(surveyor.project_id,"4KjqhPc9");
        assert_eq!(surveyor.version_number,"1.2.4+1.21");
        let api = catalog.versions("fabric-api",Some("1.21.1"),Some("fabric")).await.unwrap()
            .into_iter().find(|v| v.version_number == "0.116.17+1.21.1").expect("Fabric API de la reproducción");
        let mut files = vec![];
        for v in [&surveyor,&api] {
            let file = v.files.iter().find(|f| f.primary).unwrap().clone();
            let bytes = catalog.download_mod(&file).await.unwrap();
            assert_eq!(hash(&bytes),file.hashes.sha512);
            fs::write(s.carpeta.join("mods").join(&file.filename),bytes).unwrap();
            files.push(file);
        }
        let before = s.huella().unwrap();
        let index = publicar(&catalog,&s).await.unwrap();
        assert!(index.mods.iter().any(|v| v.version == api.id));
        assert!(index.archivos.iter().any(|a| a.sha512 == files[0].hashes.sha512));
        assert!(index.avisos.iter().any(|a| a.contains("Modrinth fija")));
        let store = Almacen::nuevo(s.carpeta.join("cliente/instances"));
        let instance = store.crear_con_loader("Cliente","1.21.1","fabric",0).unwrap();
        crate::versiones_instancia::guardar_loader(&store,&instance,&s.loader_version).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let cancel = CancellationToken::new();
        let service = tokio::spawn(super::super::servir_archivos(listener,Arc::new(RwLock::new(Some(index))),cancel.clone(),Some(s.carpeta.join("mods"))));
        let index = super::super::consultar(addr,CancellationToken::new()).await.unwrap();
        let plan = super::super::preparar(&catalog,&store,&instance,index,addr.to_string()).await.unwrap();
        super::super::aplicar(&catalog,&store,&instance,&plan,CancellationToken::new(),|_,_|{}).await.unwrap();
        for file in files {
            assert_eq!(hash_archivo(&store.juego(&instance.id).unwrap().join("mods").join(&file.filename)).unwrap(),file.hashes.sha512);
        }
        assert_eq!(before,s.huella().unwrap());
        cancel.cancel();service.await.unwrap().unwrap();
    }
    #[tokio::test]
    #[ignore = "Descarga Ksyxis 1.4.4 real de Modrinth en una carpeta temporal"]
    async fn ksyxis_real_no_bloquea_el_indice_con_minecraft_1211() {
        let (_temp, mut s) = servidor();
        s.loader_version = "0.19.5".into();
        let catalog = Catalog::new().unwrap();
        let v = catalog
            .versions("ksyxis", None, None)
            .await
            .unwrap()
            .into_iter()
            .find(|v| v.version_number == "1.4.4")
            .expect("Ksyxis 1.4.4 publicado");
        let file = v.files.iter().find(|f| f.primary).unwrap();
        let bytes = <Catalog as Fuente>::archivo(&catalog, file).await.unwrap();
        assert_eq!(hash(&bytes), file.hashes.sha512);
        fs::write(s.carpeta.join("mods").join(&file.filename), &bytes).unwrap();
        let before = s.huella().unwrap();
        let index = publicar(&catalog, &s).await.unwrap();
        if Catalogo::cliente(&catalog, &v).await.unwrap() {
            if !compatible(&v, &s.minecraft, &s.loader) {
                assert!(index
                    .archivos
                    .iter()
                    .any(|a| a.sha512 == file.hashes.sha512));
                assert!(index.avisos.iter().any(|a| a.contains("no declara")));
            } else {
                assert!(index.mods.iter().any(|m| m.sha512 == file.hashes.sha512));
            }
        } else {
            assert!(index.mods.is_empty() && index.archivos.is_empty());
        }
        assert_eq!(before, s.huella().unwrap());
    }
    #[tokio::test]
    async fn universales_con_versiones_extremas_o_loader_no_listado_son_propios_con_aviso() {
        let (_temp, s) = servidor();
        let mut catalog = Fake::default();
        for (name, project, versions, loaders, server_only) in [
            (
                "Ksyxis-1.4.4.jar",
                "ksyxis",
                vec!["1.8", "26.3"],
                vec!["fabric", "forge"],
                false,
            ),
            (
                "multi-loader.jar",
                "multiloader",
                vec!["1.21.1"],
                vec!["forge"],
                false,
            ),
            (
                "server-only.jar",
                "backend",
                vec!["1.8", "26.3"],
                vec!["forge"],
                true,
            ),
        ] {
            let body = jar_id("*", project);
            fs::write(s.carpeta.join("mods").join(name), &body).unwrap();
            let mut v = version(project, project, &body, serde_json::json!([]));
            v.game_versions = versions.into_iter().map(String::from).collect();
            v.loaders = loaders.into_iter().map(String::from).collect();
            catalog.hashes.insert(hash(&body), v);
            if server_only {
                catalog.server_only.insert(project.into());
            }
        }
        for (project, dependency) in [("dependiente", "ksyxis"), ("dependiente2", "dependiente")] {
            let body = jar_id("*", project);
            fs::write(s.carpeta.join("mods").join(format!("{project}.jar")), &body).unwrap();
            catalog.hashes.insert(hash(&body),version(project,project,&body,serde_json::json!([{"project_id":dependency,"version_id":dependency,"dependency_type":"required"}])));
        }
        let before = s.huella().unwrap();
        let index = publicar_con(&catalog, &s).await.unwrap();
        assert!(index.mods.is_empty());
        assert_eq!(index.archivos.len(), 4);
        assert_eq!(index.avisos.len(), 4);
        assert!(index
            .avisos
            .iter()
            .any(|a| a.starts_with("Ksyxis-1.4.4.jar:") && a.contains("no declara")));
        assert!(index
            .archivos
            .iter()
            .all(|a| a.archivo != "server-only.jar"));
        assert_eq!(before, s.huella().unwrap());
        // Si el catálogo devuelve otro hash, eso sí invalida la publicación.
        catalog
            .hashes
            .values_mut()
            .for_each(|v| v.files[0].hashes.sha512 = "0".repeat(128));
        assert!(publicar_con(&catalog, &s)
            .await
            .unwrap_err()
            .contains("SHA-512"));
    }
    #[derive(Default)]
    struct Fake {
        hashes: BTreeMap<String, Version>,
        versions: BTreeMap<String, Version>,
        server_only: BTreeSet<String>,
        modificar: Option<PathBuf>,
        files: BTreeMap<String, Vec<u8>>,
    }
    impl Catalogo for Fake {
        async fn hashes(&self, _: &[String]) -> Result<BTreeMap<String, Version>, String> {
            if let Some(path) = &self.modificar {
                fs::write(path, b"mod nuevo").unwrap();
            }
            Ok(self.hashes.clone())
        }
        async fn cliente(&self, version: &Version) -> Result<bool, String> {
            Ok(!self.server_only.contains(&version.project_id))
        }
        async fn version(&self, id: &str) -> Result<Version, String> {
            self.versions.get(id).or_else(|| self.hashes.values().find(|v| v.id == id))
                .cloned().ok_or("Versión ausente".into())
        }
        async fn versiones(&self, id: &str, _: &str, _: &str) -> Result<Vec<Version>, String> {
            Ok(self
                .versions
                .values()
                .filter(|v| v.project_id == id)
                .cloned()
                .collect())
        }
    }
    impl Fuente for Fake {
        async fn versiones_por_hash(&self, hashes: &[String]) -> Result<BTreeMap<String, Version>, Error> {
            Ok(self.hashes.iter().filter(|(h, _)| hashes.contains(h)).map(|(h, v)| (h.clone(), v.clone())).collect())
        }
        async fn solo_servidor(&self, project: &str) -> Result<bool, Error> {
            Ok(self.server_only.contains(project))
        }
        async fn version(&self, id: &str) -> Result<Version, Error> {
            Catalogo::version(self,id).await.map_err(|_| Error::Datos)
        }
        async fn versiones(&self, id: &str, mc: &str, loader: &str) -> Result<Vec<Version>, Error> {
            Catalogo::versiones(self,id,mc,loader).await.map_err(|_| Error::Datos)
        }
        async fn nombre(&self, id: &str) -> Result<String, Error> { Ok(id.into()) }
        async fn archivo(&self, file: &ModFile) -> Result<Vec<u8>, Error> {
            self.files.get(&file.hashes.sha512).cloned().ok_or(Error::Datos)
        }
    }
    #[tokio::test]
    async fn carpeta_dedicada_publica_hashes_excluye_servidor_y_agrega_dependencia_cliente_sin_escribir(
    ) {
        let (_temp, s) = servidor();
        let mut catalog = Fake::default();
        let common = jar("*");
        let private = jar("server");
        let dedicated = b"servidor conocido";
        fs::write(s.carpeta.join("mods/common.jar"), &common).unwrap();
        fs::write(s.carpeta.join("mods/private.jar"), private).unwrap();
        fs::write(s.carpeta.join("mods/backend.jar"), dedicated).unwrap();
        fs::write(s.carpeta.join("server.properties"), b"server-port=25565").unwrap();
        fs::create_dir(s.carpeta.join("world")).unwrap();
        fs::write(s.carpeta.join("world/level.dat"), b"mundo").unwrap();
        catalog.hashes.insert(hash(&common), version("common", "v1", &common, serde_json::json!([{"version_id":"dep1","project_id":"clientdep","dependency_type":"required"}])));
        catalog.hashes.insert(
            hash(dedicated),
            version("backend", "back1", dedicated, serde_json::json!([])),
        );
        catalog.server_only.insert("backend".into());
        catalog.versions.insert(
            "dep1".into(),
            version("clientdep", "dep1", b"cliente", serde_json::json!([])),
        );
        let before = s.huella().unwrap();
        let index = publicar_con(&catalog, &s).await.unwrap();
        assert_eq!(
            index
                .mods
                .iter()
                .map(|e| e.proyecto.as_str())
                .collect::<Vec<_>>(),
            vec!["clientdep", "common"]
        );
        assert_eq!(index.mods[1].sha512, hash(&common));
        assert_eq!(before, s.huella().unwrap());
        assert_eq!(
            fs::read(s.carpeta.join("world/level.dat")).unwrap(),
            b"mundo"
        );
        assert_eq!(
            fs::read(s.carpeta.join("server.properties")).unwrap(),
            b"server-port=25565"
        );
        // El mismo contrato HTTP que consulta el launcher sigue siendo compatible.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let published = Arc::new(RwLock::new(Some(index)));
        let cancel = CancellationToken::new();
        let service = tokio::spawn(super::super::servir(
            listener,
            published.clone(),
            cancel.clone(),
        ));
        let fetched = super::super::consultar(addr, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(fetched.mods.len(), 2);
        fs::remove_file(s.carpeta.join("mods/common.jar")).unwrap();
        let next = publicar_con(&catalog, &s).await.unwrap();
        assert!(next.mods.is_empty());
        *published.write().await = Some(next);
        assert!(super::super::consultar(addr, CancellationToken::new())
            .await
            .unwrap()
            .mods
            .is_empty());
        cancel.cancel();
        service.await.unwrap().unwrap();
    }
    #[tokio::test]
    async fn archivo_no_identificado_es_aviso_y_cambio_durante_consulta_impide_publicar() {
        let (_temp, s) = servidor();
        let body = jar("*");
        fs::write(s.carpeta.join("mods/common.jar"), &body).unwrap();
        let mut catalog = Fake::default();
        let index = publicar_con(&catalog, &s).await.unwrap();
        assert_eq!(index.archivos.len(), 1);
        assert_eq!(index.avisos.len(), 1);
        catalog.hashes.insert(
            hash(&body),
            version("common", "v1", &body, serde_json::json!([])),
        );
        catalog.modificar = Some(s.carpeta.join("mods/new.jar"));
        assert!(publicar_con(&catalog, &s)
            .await
            .unwrap_err()
            .contains("cambiaron"));
    }
    #[tokio::test]
    async fn ciento_sesenta_y_tres_mods_con_diecinueve_propios_publican_lista_completa() {
        let (_temp, mut s) = servidor();
        s.loader_version = "0.19.5".into();
        let mut catalog = Fake::default();
        for n in 0..163 {
            let project = format!("mod{n}");
            let body = jar_id("*", &project);
            fs::write(s.carpeta.join("mods").join(format!("{project}.jar")), &body).unwrap();
            if n < 144 {
                catalog.hashes.insert(
                    hash(&body),
                    version(&project, &format!("ver{n}"), &body, serde_json::json!([])),
                );
            }
        }
        let index = publicar_con(&catalog, &s).await.unwrap();
        assert_eq!(index.mods.len(), 144);
        assert_eq!(index.archivos.len(), 19);
        assert_eq!(index.avisos.len(), 19);
        assert_eq!(index.loader_version.as_deref(), Some("0.19.5"));
        s.excluir.insert("mod162.jar".into());
        assert_eq!(publicar_con(&catalog, &s).await.unwrap().archivos.len(), 18);
    }
    #[tokio::test]
    async fn privados_http_se_agregan_actualizan_y_retiran_sin_tocar_locales_y_rechazan_cambios() {
        let (_temp, mut s) = servidor();
        s.loader_version = "0.19.5".into();
        let catalog = Fake::default();
        let source = Catalog::new().unwrap();
        let file = s.carpeta.join("mods/casinocraft.jar");
        let original = jar("*");
        fs::write(&file, &original).unwrap();
        let index = publicar_con(&catalog, &s).await.unwrap();
        let store = Almacen::nuevo(s.carpeta.join("clientes/instances"));
        let instance = store
            .crear_con_loader("Cliente", "1.21.1", "fabric", 0)
            .unwrap();
        crate::versiones_instancia::guardar_loader(&store, &instance, "0.19.5").unwrap();
        let dir = store.juego(&instance.id).unwrap().join("mods");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("manual.jar"), b"mod personal").unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let published = Arc::new(RwLock::new(Some(index.clone())));
        let cancel = CancellationToken::new();
        let service = tokio::spawn(super::super::servir_archivos(
            listener,
            published.clone(),
            cancel.clone(),
            Some(s.carpeta.join("mods")),
        ));
        let downloaded = super::super::consultar(addr, CancellationToken::new())
            .await
            .unwrap();
        let plan = super::super::preparar(&source, &store, &instance, downloaded, addr.to_string())
            .await
            .unwrap();
        assert_eq!(plan.cambios.len(), 1);
        // El archivo cambia después de preparar: no se entregará el contenido nuevo bajo un hash antiguo.
        fs::write(&file, jar("client")).unwrap();
        assert!(super::super::aplicar(
            &source,
            &store,
            &instance,
            &plan,
            CancellationToken::new(),
            |_, _| {}
        )
        .await
        .is_err());
        assert!(!dir.join("casinocraft.jar").exists());
        fs::write(&file, &original).unwrap();
        super::super::aplicar(
            &source,
            &store,
            &instance,
            &plan,
            CancellationToken::new(),
            |_, _| {},
        )
        .await
        .unwrap();
        assert_eq!(fs::read(dir.join("casinocraft.jar")).unwrap(), original);
        let updated = jar("client");
        fs::write(&file, &updated).unwrap();
        let next = publicar_con(&catalog, &s).await.unwrap();
        *published.write().await = Some(next.clone());
        let plan = super::super::preparar(&source, &store, &instance, next, addr.to_string())
            .await
            .unwrap();
        assert_eq!(plan.cambios[0].tipo, Tipo::Actualizar);
        super::super::aplicar(
            &source,
            &store,
            &instance,
            &plan,
            CancellationToken::new(),
            |_, _| {},
        )
        .await
        .unwrap();
        assert_eq!(fs::read(dir.join("casinocraft.jar")).unwrap(), updated);
        let client = reqwest::Client::builder().no_proxy().build().unwrap();
        for path in [
            "/icaro/v1/files/../server.properties",
            "/icaro/v1/files/nohash",
        ] {
            assert_eq!(
                client
                    .get(format!("http://{addr}{path}"))
                    .send()
                    .await
                    .unwrap()
                    .status(),
                reqwest::StatusCode::NOT_FOUND
            );
        }
        fs::remove_file(&file).unwrap();
        let next = publicar_con(&catalog, &s).await.unwrap();
        let plan =
            super::super::preparar(&source, &store, &instance, next.clone(), addr.to_string())
                .await
                .unwrap();
        assert_eq!(plan.cambios[0].tipo, Tipo::Quitar);
        fs::write(dir.join("casinocraft.jar"), b"modificado por el jugador").unwrap();
        assert!(
            super::super::preparar(&source, &store, &instance, next, addr.to_string())
                .await
                .unwrap_err()
                .contains("modificado")
        );
        fs::write(dir.join("casinocraft.jar"), &updated).unwrap();
        super::super::aplicar(
            &source,
            &store,
            &instance,
            &plan,
            CancellationToken::new(),
            |_, _| {},
        )
        .await
        .unwrap();
        assert!(!dir.join("casinocraft.jar").exists());
        assert_eq!(fs::read(dir.join("manual.jar")).unwrap(), b"mod personal");
        cancel.cancel();
        service.await.unwrap().unwrap();
    }
    #[tokio::test]
    async fn dependencias_ausentes_con_pins_incompatibles_siguen_bloqueando() {
        let (_temp, s) = servidor();
        let mut catalog = Fake::default();
        for (project,pin) in [("primero","dep1"),("segundo","dep2")] {
            let bytes = jar_id("*",project);
            fs::write(s.carpeta.join("mods").join(format!("{project}.jar")),&bytes).unwrap();
            catalog.hashes.insert(hash(&bytes),version(project,project,&bytes,
                serde_json::json!([{"version_id":pin,"project_id":"ausente","dependency_type":"required"}])));
            catalog.versions.insert(pin.into(),version("ausente",pin,b"dependencia",serde_json::json!([])));
        }
        let before = s.huella().unwrap();
        assert!(publicar_con(&catalog,&s).await.unwrap_err().contains("Dependencias incompatibles"));
        assert_eq!(before,s.huella().unwrap());
    }
    #[tokio::test]
    async fn pin_distinto_conserva_instalados_propaga_y_sincroniza_el_cliente_por_http() {
        // Cubrir tanto project_id como los pins que solo identifican una versión antigua.
        for version_only in [false,true] {
            let (_temp,s) = servidor();
            let mut catalog = Fake::default();
            let dep = serde_json::json!({"version_id":"api-antigua","project_id":if version_only {None} else {Some("fabric-api")},"dependency_type":"required"});
            let mut originales = BTreeMap::new();
            for (project,id,depends) in [
                ("fabric-api","api-instalada",serde_json::json!([])),
                ("surveyor","surveyor124",serde_json::json!([dep,{"project_id":"solo-cliente","version_id":"cliente1","dependency_type":"required"}])),
                ("mapa","mapa1",serde_json::json!([{"project_id":"surveyor","version_id":"surveyor124","dependency_type":"required"}])),
                ("otro-mapa","mapa2",serde_json::json!([{"version_id":"mapa1","dependency_type":"required"}])),
            ] {
                let bytes = jar_descriptor(serde_json::json!({"id":project,"version":if project == "fabric-api" {"0.116.17+1.21.1"} else {"1.2.4"},"depends":if project == "surveyor" {serde_json::json!({"fabric-api":">=0.104.0+1.21.1"})} else {serde_json::json!({})}}).to_string().as_bytes());
                fs::write(s.carpeta.join("mods").join(format!("{project}.jar")),&bytes).unwrap();
                catalog.files.insert(hash(&bytes),bytes.clone());
                catalog.hashes.insert(hash(&bytes),version(project,id,&bytes,depends));
                originales.insert(format!("{project}.jar"),bytes);
            }
            catalog.versions.insert("api-antigua".into(),version("fabric-api","api-antigua",b"antigua",serde_json::json!([])));
            let extra = jar_id("client","solo-cliente");
            catalog.versions.insert("cliente1".into(),version("solo-cliente","cliente1",&extra,serde_json::json!([])));
            catalog.files.insert(hash(&extra),extra.clone());
            originales.insert("solo-cliente.jar".into(),extra);
            let before = s.huella().unwrap();
            let index = publicar_con(&catalog,&s).await.unwrap();
            assert_eq!(index.mods.len(),2);
            assert_eq!(index.archivos.len(),3);
            assert!(index.mods.iter().any(|v| v.version == "api-instalada"));
            assert!(!index.mods.iter().any(|v| v.version == "api-antigua"));
            assert!(index.avisos.iter().any(|a| a.contains("api-antigua") && a.contains("api-instalada")));
            let store = Almacen::nuevo(s.carpeta.join("cliente/instances"));
            let instance = store.crear_con_loader("Cliente","1.21.1","fabric",0).unwrap();
            crate::versiones_instancia::guardar_loader(&store,&instance,&s.loader_version).unwrap();
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let cancel = CancellationToken::new();
            let service = tokio::spawn(super::super::servir_archivos(listener,Arc::new(RwLock::new(Some(index))),cancel.clone(),Some(s.carpeta.join("mods"))));
            let index = super::super::consultar(addr,CancellationToken::new()).await.unwrap();
            let plan = super::super::preparar(&catalog,&store,&instance,index,addr.to_string()).await.unwrap();
            assert_eq!(plan.cambios.len(),5);
            super::super::aplicar(&catalog,&store,&instance,&plan,CancellationToken::new(),|_,_|{}).await.unwrap();
            for (name,bytes) in originales {
                assert_eq!(fs::read(store.juego(&instance.id).unwrap().join("mods").join(name)).unwrap(),bytes);
            }
            assert_eq!(before,s.huella().unwrap());
            cancel.cancel();service.await.unwrap().unwrap();
        }
    }
    #[tokio::test]
    async fn ciclos_se_publican_una_vez_y_un_pin_de_otro_proyecto_se_rechaza() {
        let (_temp,s) = servidor();
        let mut catalog = Fake::default();
        for (project,deps) in [
            ("api",serde_json::json!([])),
            ("a",serde_json::json!([{"project_id":"api","version_id":"old","dependency_type":"required"},{"project_id":"b","dependency_type":"required"}])),
            ("b",serde_json::json!([{"project_id":"a","dependency_type":"required"}])),
        ] {
            let bytes = jar_id("*",project);
            fs::write(s.carpeta.join("mods").join(format!("{project}.jar")),&bytes).unwrap();
            catalog.hashes.insert(hash(&bytes),version(project,project,&bytes,deps));
        }
        catalog.versions.insert("old".into(),version("api","old",b"old",serde_json::json!([])));
        let index = publicar_con(&catalog,&s).await.unwrap();
        assert_eq!(index.mods.len(),1);
        assert_eq!(index.archivos.len(),2);
        assert_eq!(index.avisos.len(),2);
        catalog.versions.get_mut("old").unwrap().project_id = "otro".into();
        assert!(publicar_con(&catalog,&s).await.unwrap_err().contains("pin no pertenece"));
    }
    #[tokio::test]
    async fn dependencia_fabric_comun_se_incluye_aunque_el_catalogo_diga_solo_servidor() {
        for entorno in [Some("*"), None] {
            let (_temp, s) = servidor();
            let mut catalog = Fake::default();
            let root = jar_id("*", "comun");
            let mut descriptor = serde_json::json!({"schemaVersion":1,"id":"biblioteca","version":"1"});
            if let Some(entorno) = entorno { descriptor["environment"] = entorno.into(); }
            let library = jar_descriptor(descriptor.to_string().as_bytes());
            fs::write(s.carpeta.join("mods/comun.jar"), &root).unwrap();
            fs::write(s.carpeta.join("mods/biblioteca.jar"), &library).unwrap();
            catalog.hashes.insert(hash(&root), version("comun", "comun1", &root,
                serde_json::json!([{"project_id":"biblioteca","version_id":"biblioteca1","dependency_type":"required"}])));
            catalog.hashes.insert(hash(&library), version("biblioteca", "biblioteca1", &library, serde_json::json!([])));
            catalog.server_only.insert("biblioteca".into());
            let index = publicar_con(&catalog, &s).await.unwrap();
            assert_eq!(index.mods.len(), 2);
            assert!(index.mods.iter().any(|m| m.proyecto == "biblioteca"));
            assert!(index.archivos.is_empty());
            assert!(index.avisos.iter().any(|a| a.contains("JAR instalado admite cliente")));
            let store = Almacen::nuevo(s.carpeta.join("cliente/instances"));
            let instance = store.crear_con_loader("Cliente", "1.21.1", "fabric", 0).unwrap();
            crate::versiones_instancia::guardar_loader(&store, &instance, &s.loader_version).unwrap();
            let plan = super::super::preparar(&catalog, &store, &instance, index, "127.0.0.1:25566".into()).await.unwrap();
            assert_eq!(plan.cambios.len(), 2);
        }
    }

    #[tokio::test]
    async fn dependencia_de_servidor_no_reaparece_en_la_resolucion_del_cliente() {
        let (_temp,s) = servidor();
        let mut catalog = Fake::default();
        let root = jar_id("*","comun");
        fs::write(s.carpeta.join("mods/comun.jar"),&root).unwrap();
        catalog.hashes.insert(hash(&root),version("comun","comun1",&root,serde_json::json!([{"project_id":"backend","version_id":"backend1","dependency_type":"required"}])));
        catalog.versions.insert("backend1".into(),version("backend","backend1",b"servidor",serde_json::json!([])));
        catalog.server_only.insert("backend".into());
        let index = publicar_con(&catalog,&s).await.unwrap();
        assert!(index.mods.is_empty());
        assert_eq!(index.archivos.len(),1);
        assert!(index.avisos[0].contains("exclusiva de servidor"));
        let store = Almacen::nuevo(s.carpeta.join("cliente/instances"));
        let instance = store.crear_con_loader("Cliente","1.21.1","fabric",0).unwrap();
        crate::versiones_instancia::guardar_loader(&store,&instance,&s.loader_version).unwrap();
        let plan = super::super::preparar(&catalog,&store,&instance,index,"127.0.0.1:25566".into()).await.unwrap();
        assert_eq!(plan.cambios.len(),1);
    }

    /// Epic Terrain exige WorldgenFeatureFix (`w2O2In9c`), que Modrinth declara
    /// sin soporte de cliente y sin versión compatible publicada para el cliente.
    fn epic_terrain(s: &ServidorDedicado, catalog: &mut Fake) {
        let epic = jar_id("*", "epicterrain");
        let worldgen = b"worldgenfeaturefix".to_vec();
        let privado = jar_id("server", "privado");
        fs::write(s.carpeta.join("mods/epicterrain-0.1.4.jar"), &epic).unwrap();
        fs::write(s.carpeta.join("mods/worldgenfeaturefix.jar"), &worldgen).unwrap();
        fs::write(s.carpeta.join("mods/privado.jar"), &privado).unwrap();
        catalog.hashes.insert(hash(&epic), version("epic", "epic1", &epic,
            serde_json::json!([{"project_id":"w2O2In9c","dependency_type":"required"}])));
        catalog.hashes.insert(hash(&worldgen), version("w2O2In9c", "wff1", &worldgen, serde_json::json!([])));
        catalog.server_only.insert("w2O2In9c".into());
    }
    async fn plan_cliente(s: &ServidorDedicado, catalog: &Fake, index: Indice) -> Result<super::super::Plan, String> {
        let store = Almacen::nuevo(s.carpeta.join("cliente/instances"));
        let instance = store.crear_con_loader("Cliente", "1.21.1", "fabric", 0).unwrap();
        crate::versiones_instancia::guardar_loader(&store, &instance, &s.loader_version).unwrap();
        super::super::preparar(catalog, &store, &instance, index, "127.0.0.1:25566".into()).await
    }

    #[tokio::test]
    async fn cliente_omite_dependencia_exclusiva_de_servidor_que_el_anfitrion_excluyo() {
        let (_temp, s) = servidor();
        let mut catalog = Fake::default();
        epic_terrain(&s, &mut catalog);
        let index = publicar_con(&catalog, &s).await.unwrap();
        assert!(index.mods.is_empty());
        assert_eq!(index.archivos.len(), 1);
        assert_eq!(index.archivos[0].archivo, "epicterrain-0.1.4.jar");
        assert!(index.avisos.iter().any(|a| a.contains("exige w2O2In9c") && a.contains("exclusiva de servidor")));
        let plan = plan_cliente(&s, &catalog, index).await.unwrap();
        assert_eq!(plan.cambios.len(), 1);
        assert!(plan.indice.mods.iter().all(|m| m.proyecto != "w2O2In9c"));
        assert!(plan.indice.avisos.iter().any(|a| a.contains("w2O2In9c") && a.contains("No se instala en el cliente")));
    }

    #[tokio::test]
    async fn incluir_servidor_publica_todos_los_jar_y_el_cliente_los_instala() {
        let (_temp, mut s) = servidor();
        let mut catalog = Fake::default();
        epic_terrain(&s, &mut catalog);
        s.incluir_servidor = true;
        let index = publicar_con(&catalog, &s).await.unwrap();
        let proyectos: BTreeSet<_> = index.mods.iter().map(|m| m.proyecto.as_str()).collect();
        assert_eq!(proyectos, ["epic", "w2O2In9c"].into());
        assert_eq!(index.archivos.len(), 1);
        assert_eq!(index.archivos[0].archivo, "privado.jar");
        assert!(index.avisos.iter().any(|a| a.contains("worldgenfeaturefix.jar") && a.contains("indicación del anfitrión")));
        let plan = plan_cliente(&s, &catalog, index).await.unwrap();
        assert_eq!(plan.cambios.len(), 3);
    }

    #[tokio::test]
    async fn incluir_archivo_publica_solo_los_jar_indicados() {
        let (_temp, mut s) = servidor();
        let mut catalog = Fake::default();
        epic_terrain(&s, &mut catalog);
        s.incluir.insert("worldgenfeaturefix.jar".into());
        s.incluir.insert("ausente.jar".into());
        s.validar().unwrap();
        let index = publicar_con(&catalog, &s).await.unwrap();
        let proyectos: BTreeSet<_> = index.mods.iter().map(|m| m.proyecto.as_str()).collect();
        assert_eq!(proyectos, ["epic", "w2O2In9c"].into());
        assert!(index.archivos.is_empty());
        assert!(index.avisos.iter().any(|a| a.contains("ausente.jar") && a.contains("no está en mods")));
        s.excluir.insert("worldgenfeaturefix.jar".into());
        assert!(s.validar().unwrap_err().contains("incluirse y excluirse"));
        s.excluir.clear();
        s.incluir.insert("../fuera.jar".into());
        assert!(s.validar().unwrap_err().contains("inclusiones"));
    }
    #[tokio::test]
    async fn cliente_acepta_163_mods_y_rechaza_un_indice_incompleto() {
        let (_temp,s) = servidor();
        let mut catalog = Fake::default();
        for n in 0..163 {
            let project = format!("mod{n}");
            let body = jar_id("*",&project);
            fs::write(s.carpeta.join("mods").join(format!("{project}.jar")),&body).unwrap();
            catalog.hashes.insert(hash(&body),version(&project,&project,&body,serde_json::json!([])));
        }
        let index = publicar_con(&catalog,&s).await.unwrap();
        let store = Almacen::nuevo(s.carpeta.join("cliente/instances"));
        let instance = store.crear_con_loader("Cliente","1.21.1","fabric",0).unwrap();
        crate::versiones_instancia::guardar_loader(&store,&instance,&s.loader_version).unwrap();
        let plan = super::super::preparar(&catalog,&store,&instance,index.clone(),"127.0.0.1:25566".into()).await.unwrap();
        assert_eq!(plan.cambios.len(),163);
        let root = catalog.hashes.values_mut().find(|v| v.project_id == "mod0").unwrap();
        root.dependencies = serde_json::from_value(serde_json::json!([{"project_id":"mod162","dependency_type":"required"}])).unwrap();
        let mut incomplete = index;
        incomplete.mods.retain(|e| e.proyecto != "mod162");
        let required = catalog.hashes.values().find(|v| v.project_id == "mod162").unwrap().clone();
        catalog.versions.insert(required.id.clone(),required);
        assert!(super::super::preparar(&catalog,&store,&instance,incomplete,"127.0.0.1:25566".into()).await.unwrap_err().contains("falta una dependencia"));
    }
    #[test]
    fn origen_invalido_o_jars_anidados_no_generan_un_indice_vacio() {
        let (_temp, mut s) = servidor();
        s.loader_version.clear();
        assert!(s.validar().is_err());
        s.loader_version = "0.16.14".into();
        fs::create_dir(s.carpeta.join("mods/otra")).unwrap();
        fs::write(s.carpeta.join("mods/otra/active.jar"), b"mod").unwrap();
        assert!(s.huella().is_err());
        fs::remove_dir_all(s.carpeta.join("mods")).unwrap();
        assert!(s.huella().is_err());
    }
}
