// SPDX-License-Identifier: GPL-3.0-only
//! Índice del anfitrión y sincronización de archivos exactos de Modrinth.
use super::*;
use crate::{instancias::Instancia, modrinth::Catalog};
use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::RwLock,
};
use tokio_util::sync::CancellationToken;
const SYNC: &str = ".icaro-sync.json";
const MAX_INDEX: usize = 1024 * 1024;
pub const RUTA: &str = "/icaro/v1/mods";
pub mod dedicado;
mod inventario;
use inventario::Inventario;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Indice {
    pub esquema: u32,
    pub minecraft: String,
    pub loader: String,
    pub loader_version: Option<String>,
    pub mods: Vec<Entrada>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub archivos: Vec<ArchivoPropio>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub avisos: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArchivoPropio {
    pub id: String,
    pub nombre: String,
    pub version: String,
    pub archivo: String,
    pub bytes: u64,
    pub sha512: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entrada {
    pub proyecto: String,
    pub version: String,
    pub sha512: String,
}
impl Indice {
    pub fn validar(&self) -> Result<(), String> {
        let identifier = |s: &str| {
            !s.is_empty()
                && s.len() <= 128
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        };
        let mut projects = BTreeSet::new();
        let mut filenames = BTreeSet::new();
        if !matches!(self.esquema, 1 | 2)
            || (self.esquema == 1 && !self.archivos.is_empty())
            || self.mods.len() + self.archivos.len() > 512
            || self.avisos.len() > 1024
            || self
                .avisos
                .iter()
                .any(|s| s.len() > 1024 || s.chars().any(char::is_control))
            || self.archivos.iter().any(|a| {
                !identifier(&a.id)
                    || !a.id.starts_with("icaro-host-")
                    || !projects.insert(&a.id)
                    || !nombre_valido(&a.archivo)
                    || !a.archivo.to_ascii_lowercase().ends_with(".jar")
                    || !filenames.insert(a.archivo.to_lowercase())
                    || a.bytes == 0
                    || a.bytes > MAX_FILE
                    || a.nombre.is_empty()
                    || a.nombre.len() > 240
                    || a.nombre.chars().any(char::is_control)
                    || a.version.len() > 128
                    || a.version.chars().any(char::is_control)
                    || a.sha512.len() != 128
                    || !a.sha512.bytes().all(|b| b.is_ascii_hexdigit())
            })
            || !crate::minecraft::version_minecraft_valida(&self.minecraft)
            || !matches!(
                self.loader.as_str(),
                "fabric" | "quilt" | "forge" | "neoforge"
            )
            || self
                .loader_version
                .as_ref()
                .is_none_or(|s| !crate::minecraft::version_minecraft_valida(s))
            || self.mods.len() > 512
            || self.mods.iter().any(|m| {
                !identifier(&m.proyecto)
                    || !identifier(&m.version)
                    || !projects.insert(&m.proyecto)
                    || m.sha512.len() != 128
                    || !m.sha512.bytes().all(|b| b.is_ascii_hexdigit())
            })
        {
            return Err("El índice de mods del anfitrión no es válido.".into());
        }
        Ok(())
    }
    pub fn revision(&self) -> Result<String, String> {
        self.validar()?;
        Ok(hash(
            &serde_json::to_vec(self).map_err(|_| "No se pudo leer el índice.")?,
        ))
    }
}
/// Última sincronización aplicada con éxito, fuera de `mods` y excluida de la
/// exportación. Si el anfitrión y la carpeta siguen idénticos, el plan es vacío
/// y puede confirmarse sin consultar Modrinth.
const VERIFICADO: &str = ".icaro-sync-verificado.json";
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Verificado {
    esquema: u32,
    origen: String,
    revision: String,
    /// Índice ya completado con las dependencias recuperadas por el cliente.
    indice: Indice,
    locales: Vec<ArchivoPropio>,
    /// SHA-512 de la huella completa de `mods` tras aplicar.
    huella: String,
    /// Segundos desde epoch. Pasadas 24 h se vuelve a confirmar con Modrinth,
    /// por ejemplo para notar versiones retiradas o dependencias recuperadas nuevas.
    fecha: u64,
}
const VIGENCIA_VERIFICADO: u64 = 24 * 3600;
fn segundos_ahora() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}
fn resumen_huella(huella: &[(String, String)]) -> String {
    let texto: String = huella.iter().map(|(ruta, sha)| format!("{ruta}\0{sha}\n")).collect();
    hash(texto.as_bytes())
}
fn leer_verificado(game: &Path) -> Option<Verificado> {
    let ruta = game.join(VERIFICADO);
    if regular(&ruta, false).ok()?.len() > 4 * 1024 * 1024 {
        return None;
    }
    serde_json::from_slice::<Verificado>(&fs::read(ruta).ok()?)
        .ok()
        .filter(|v| {
            v.esquema == 1
                && v.huella.len() == 128
                && segundos_ahora().saturating_sub(v.fecha) < VIGENCIA_VERIFICADO
        })
}
fn guardar_verificado(game: &Path, plan: &Plan, huella: &[(String, String)]) -> Result<(), String> {
    let registro = Verificado {
        esquema: 1,
        origen: plan.origen.clone(),
        revision: plan.revision_origen.clone(),
        indice: plan.indice.clone(),
        locales: plan.locales.clone(),
        huella: resumen_huella(huella),
        fecha: segundos_ahora(),
    };
    crate::almacenamiento::escribir_json(&game.join(VERIFICADO), &serde_json::to_value(registro).map_err(err)?)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Gestionados {
    origen: String,
    proyectos: BTreeSet<String>,
    #[serde(default)]
    archivos: Vec<ArchivoPropio>,
}
#[derive(Debug, Clone, PartialEq)]
pub enum Tipo {
    Agregar,
    Actualizar,
    Quitar,
}
#[derive(Debug, Clone)]
pub struct Cambio {
    pub proyecto: String,
    pub tipo: Tipo,
    pub nombre: String,
    pub antes: String,
    pub despues: String,
    pub bytes: u64,
}
#[derive(Debug, Clone)]
pub struct Plan {
    pub indice: Indice,
    pub cambios: Vec<Cambio>,
    pub conservados: Vec<String>,
    pub conflictos: Vec<String>,
    reemplazos: BTreeSet<String>,
    reemplazos_autorizados: bool,
    origen: String,
    huella: Vec<(String, String)>,
    estado: Vec<EstadoArchivo>,
    /// Identidades Fabric leídas en la revisión, por nombre de archivo activo.
    identidades: BTreeMap<String, String>,
    actuales: Vec<Instalado>,
    destinos: Vec<Version>,
    quitar: BTreeSet<String>,
    locales: Vec<ArchivoPropio>,
    quitar_locales: Vec<ArchivoPropio>,
    revision_origen: String,
    recuperadas: BTreeSet<String>,
    /// Plan vacío confirmado con el registro verificado, sin consultar Modrinth.
    sin_red: bool,
}
impl Plan {
    pub fn revision_origen(&self) -> &str {
        &self.revision_origen
    }
    pub fn autorizar_reemplazos(&mut self, autorizar: bool) {
        self.reemplazos_autorizados = autorizar;
    }
    pub fn puede_aplicar(&self) -> bool {
        self.conflictos.is_empty() || self.reemplazos_autorizados
    }
}

// Corrección acotada a la versión cuyos metadatos omiten un requisito del cliente.
// Fuente: https://modrinth.com/mod/litemoretica (Litematica solo es obligatoria en cliente).
fn requiere_litematica(v: &Version) -> bool {
    v.project_id == "OYR5DMCo" && v.id == "nhi2RkZ9"
}

fn dependencias_cliente(v: &Version) -> Vec<Dependency> {
    let mut deps: Vec<_> = v.dependencies.iter()
        .filter(|d| d.dependency_type == "required").cloned().collect();
    if requiere_litematica(v) && !deps.iter().any(|d| d.project_id.as_deref() == Some("bEpr0Arc")) {
        deps.push(Dependency { project_id: Some("bEpr0Arc".into()), version_id: None,
            dependency_type: "required".into(), file_name: None });
    }
    deps
}

// Los índices antiguos pueden transportar un mod como JAR propio y omitir sus
// bibliotecas. Recuperarlas por hash conserva la identidad exacta del mod raíz.
async fn completar_dependencias_propias(source: &impl Fuente, index: &mut Indice) -> Result<BTreeSet<String>, String> {
    let hashes = index.archivos.iter().map(|a| a.sha512.to_lowercase()).collect::<Vec<_>>();
    let conocidas = if hashes.is_empty() { BTreeMap::new() } else {
        source.versiones_por_hash(&hashes).await.map_err(err)?
    };
    let mut cubiertos = index.mods.iter().map(|m| m.proyecto.clone()).collect::<BTreeSet<_>>();
    let mut pendientes = vec![];
    for archivo in &index.archivos {
        if let Some(version) = conocidas.get(&archivo.sha512.to_lowercase()) {
            if !version.files.iter().any(|f| f.hashes.sha512.eq_ignore_ascii_case(&archivo.sha512)) {
                return Err("Modrinth no confirma el hash del JAR del anfitrión.".into());
            }
            cubiertos.insert(version.project_id.clone());
            pendientes.push(version.clone());
        }
    }
    // También reparar índices nuevos que transportan esta versión por catálogo.
    // Los demás proyectos conservan la validación estricta de su índice exacto.
    for entry in index.mods.iter().filter(|m| m.proyecto == "OYR5DMCo" && m.version == "nhi2RkZ9") {
        let v = source.version(&entry.version).await.map_err(err)?;
        if v.id != entry.version || v.project_id != entry.proyecto
            || !v.files.iter().any(|f| f.hashes.sha512.eq_ignore_ascii_case(&entry.sha512)) {
            return Err("Modrinth no confirma el mod que requiere dependencias de cliente.".into());
        }
        pendientes.push(v);
    }
    let mut recuperadas = BTreeSet::new();
    let mut omitidas = BTreeSet::new();
    while let Some(version) = pendientes.pop() {
        for dep in &dependencias_cliente(&version) {
            // Los pins de un mod transportado como propio no sustituyen el
            // conjunto que el anfitrión ya proporciona (por ejemplo Fabric API).
            if let Some(project) = dep.project_id.as_ref().filter(|p| cubiertos.contains(*p)) {
                if recuperadas.contains(project) && dep.version_id.as_ref().is_some_and(|pin| {
                    index.mods.iter().any(|m| &m.proyecto == project && &m.version != pin)
                }) {
                    return Err(format!("Dos mods del anfitrión requieren versiones distintas de la biblioteca {project}."));
                }
                continue;
            }
            let pinned = if let Some(id) = &dep.version_id {
                let v = source.version(id).await.map_err(err)?;
                if v.id != *id || dep.project_id.as_ref().is_some_and(|p| p != &v.project_id) {
                    return Err("Modrinth devolvió otra dependencia del archivo del anfitrión.".into());
                }
                Some(v)
            } else { None };
            let project = dep.project_id.as_ref().or_else(|| pinned.as_ref().map(|v| &v.project_id))
                .ok_or("Una dependencia del anfitrión no tiene identidad de Modrinth.")?.clone();
            if cubiertos.contains(&project) {
                if recuperadas.contains(&project) && pinned.as_ref().is_some_and(|v| {
                    index.mods.iter().any(|m| m.proyecto == project && m.version != v.id)
                }) {
                    return Err(format!("Dos mods del anfitrión requieren versiones distintas de la biblioteca {project}."));
                }
                continue;
            }
            // El anfitrión excluye del cliente las dependencias sin soporte de
            // cliente (por ejemplo WorldgenFeatureFix para Epic Terrain).
            if omitidas.contains(&project) {
                continue;
            }
            if source.solo_servidor(&project).await.map_err(err)? {
                index.avisos.push(format!("{}: Modrinth exige {project} como dependencia, pero la clasifica como exclusiva de servidor. No se instala en el cliente.", version.project_id));
                omitidas.insert(project);
                continue;
            }
            let candidate = if let Some(v) = pinned { v } else {
                let mut versiones = source.versiones(&project, &index.minecraft, &index.loader).await.map_err(err)?;
                versiones.sort_by(|a,b| b.date_published.cmp(&a.date_published).then(a.id.cmp(&b.id)));
                versiones.into_iter().find(|v| v.project_id == project && v.version_type == "release"
                    && compatible(v, &index.minecraft, &index.loader))
                    .ok_or_else(|| format!("Falta una dependencia compatible del anfitrión: {project}."))?
            };
            if !compatible(&candidate, &index.minecraft, &index.loader) {
                return Err("La dependencia del archivo del anfitrión no coincide con Minecraft o loader.".into());
            }
            let file = candidate.files.iter().find(|f| f.primary && f.filename.ends_with(".jar"))
                .or_else(|| candidate.files.iter().find(|f| f.filename.ends_with(".jar")))
                .ok_or("La dependencia del anfitrión no ofrece un JAR.")?;
            index.mods.push(Entrada { proyecto: candidate.project_id.clone(), version: candidate.id.clone(), sha512: file.hashes.sha512.clone() });
            index.validar()?;
            cubiertos.insert(candidate.project_id.clone());
            recuperadas.insert(candidate.project_id.clone());
            pendientes.push(candidate);
        }
    }
    if !recuperadas.is_empty() {
        index.avisos.push("Se completaron dependencias obligatorias omitidas por el índice del anfitrión.".into());
    }
    index.validar()?;
    Ok(recuperadas)
}
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
const CANCELADA: &str = "Revisión de mods cancelada. No se modificó la instancia.";

static BITACORA: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
static ESCRITURA: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Activa `logs/sincronizacion.log` en la carpeta de datos del launcher. Sin
/// activarlo (pruebas, modo demo) la sincronización no escribe registros.
pub fn activar_registro(root: &Path) {
    let _ = BITACORA.set(root.to_path_buf());
}
/// Una línea `evento clave=valor`. Un fallo al escribir nunca interrumpe la sincronización.
fn registrar(mensaje: impl AsRef<str>) {
    if let Some(root) = BITACORA.get() {
        let _guardia = ESCRITURA.lock();
        let _ = crate::almacenamiento::registrar_en(root, "sincronizacion", mensaje.as_ref());
    }
}
/// Evento que solo conoce la interfaz, como un límite de tiempo o un índice
/// del anfitrión que cambió entre la revisión y la aplicación.
pub fn registrar_evento(evento: &str, instancia: &str, mensaje: &str) {
    registrar(format!("{evento} instancia={instancia} mensaje={mensaje}"));
}
/// Los primeros 12 caracteres bastan para distinguir revisiones en el registro.
fn corta(revision: &str) -> &str {
    revision.get(..12).unwrap_or(revision)
}
fn ms(inicio: std::time::Instant) -> u128 {
    inicio.elapsed().as_millis()
}

/// Ejecuta lecturas y hashes fuera del executor asíncrono. Si el futuro se
/// descarta por cancelación o límite de tiempo, el token detiene el trabajo
/// entre archivos en lugar de dejarlo leyendo toda la carpeta.
async fn bloqueante<T: Send + 'static>(
    trabajo: impl FnOnce(CancellationToken) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let token = CancellationToken::new();
    let _guardia = token.clone().drop_guard();
    tokio::task::spawn_blocking(move || trabajo(token))
        .await
        .map_err(|_| "Se interrumpió la revisión local de los mods.".to_string())?
}

/// Metadatos de un archivo de `mods`. Solo detecta cambios ocurridos después
/// de verificar los hashes; nunca sustituye esa verificación de integridad.
#[derive(Debug, Clone, PartialEq, Eq)]
struct EstadoArchivo {
    ruta: String,
    bytes: u64,
    modificado: Option<std::time::SystemTime>,
    creado: Option<std::time::SystemTime>,
    nodo: u64,
}
fn estado_archivo(ruta: String, meta: &fs::Metadata) -> EstadoArchivo {
    #[cfg(unix)]
    let nodo = std::os::unix::fs::MetadataExt::ino(meta);
    #[cfg(not(unix))]
    let nodo = 0;
    EstadoArchivo {
        ruta,
        bytes: meta.len(),
        modificado: meta.modified().ok(),
        creado: meta.created().ok(),
        nodo,
    }
}
fn estado_dir(
    dir: &Path,
    base: &Path,
    depth: usize,
    result: &mut Vec<EstadoArchivo>,
) -> Result<(), String> {
    regular(dir, true).map_err(err)?;
    if depth > 8 {
        return Err("La carpeta de mods contiene demasiados niveles.".into());
    }
    for entry in fs::read_dir(dir).map_err(err)? {
        let path = entry.map_err(err)?.path();
        let meta = fs::symlink_metadata(&path).map_err(err)?;
        if enlace(&meta) {
            return Err("La carpeta de mods contiene enlaces no permitidos.".into());
        }
        if meta.is_dir() {
            estado_dir(&path, base, depth + 1, result)?;
        } else {
            if result.len() >= 2048 {
                return Err("La carpeta de mods supera los límites de sincronización.".into());
            }
            let ruta = path.strip_prefix(base).map_err(err)?.to_string_lossy().into_owned();
            result.push(estado_archivo(ruta, &meta));
        }
    }
    Ok(())
}
/// Comprueba que `mods` sigue como se verificó. Si los metadatos coinciden no
/// relee nada; si difieren (por ejemplo, un archivo restaurado con el mismo
/// contenido), decide comparando de nuevo los hashes completos.
fn sin_cambios(
    dir: &Path,
    estado_esperado: &[EstadoArchivo],
    huella: &[(String, String)],
    cancel: &CancellationToken,
) -> Result<bool, String> {
    Ok(estado(dir)? == estado_esperado || fingerprint_cancelable(dir, cancel)? == huella)
}
/// Recorre solo metadatos: cuesta milisegundos aunque la carpeta ocupe cientos de MiB.
fn estado(dir: &Path) -> Result<Vec<EstadoArchivo>, String> {
    let mut out = vec![];
    if dir.exists() {
        estado_dir(dir, dir, 0, &mut out)?;
    }
    out.sort_by(|a, b| a.ruta.cmp(&b.ruta));
    Ok(out)
}
fn huella_dir(
    dir: &Path,
    base: &Path,
    depth: usize,
    result: &mut Vec<(String, String)>,
    total: &mut u64,
    cancel: &CancellationToken,
) -> Result<(), String> {
    regular(dir, true).map_err(err)?;
    if depth > 8 {
        return Err("La carpeta de mods contiene demasiados niveles.".into());
    }
    for entry in fs::read_dir(dir).map_err(err)? {
        if cancel.is_cancelled() {
            return Err(CANCELADA.into());
        }
        let path = entry.map_err(err)?.path();
        let meta = fs::symlink_metadata(&path).map_err(err)?;
        if enlace(&meta) {
            return Err("La carpeta de mods contiene enlaces no permitidos.".into());
        }
        if meta.is_dir() {
            huella_dir(&path, base, depth + 1, result, total, cancel)?;
        } else {
            regular(&path, false).map_err(err)?;
            *total = total
                .checked_add(meta.len())
                .ok_or("La carpeta de mods es demasiado grande.")?;
            if result.len() >= 2048 || meta.len() > MAX_FILE || *total > 4 * 1024 * 1024 * 1024 {
                return Err("La carpeta de mods supera los límites de sincronización.".into());
            }
            result.push((
                path.strip_prefix(base)
                    .map_err(err)?
                    .to_string_lossy()
                    .into_owned(),
                hash_archivo(&path).map_err(err)?,
            ));
        }
    }
    Ok(())
}
pub fn huella(store: &Almacen, id: &str) -> Result<Vec<(String, String)>, String> {
    let game = store.juego(id).map_err(err)?;
    let _lock = guardia(&game).map_err(err)?;
    recuperar(&game).map_err(err)?;
    fingerprint(&game.join("mods"))
}
pub(super) fn fingerprint(dir: &Path) -> Result<Vec<(String, String)>, String> {
    fingerprint_cancelable(dir, &CancellationToken::new())
}
fn fingerprint_cancelable(
    dir: &Path,
    cancel: &CancellationToken,
) -> Result<Vec<(String, String)>, String> {
    let mut out = vec![];
    if dir.exists() {
        huella_dir(dir, dir, 0, &mut out, &mut 0, cancel)?;
    }
    out.sort();
    Ok(out)
}

/// Resultado de leer la instancia una sola vez: hashes verificados, metadatos
/// para detectar cambios posteriores, registros e identidades Fabric.
#[derive(Debug, Clone)]
pub struct AnalisisLocal {
    instancia: String,
    huella: Vec<(String, String)>,
    estado: Vec<EstadoArchivo>,
    instalados: Vec<Instalado>,
    identidades: BTreeMap<String, String>,
    /// Archivos leídos completos y archivos cuyo hash se tomó del inventario.
    releidos: usize,
    reutilizados: usize,
}

/// Calcula una vez los hashes de `mods` fuera del executor. Separarlo de
/// [`preparar_desde`] permite limitar el tiempo de la red sin contar el disco.
pub async fn analizar(store: &Almacen, instance: &Instancia) -> Result<AnalisisLocal, String> {
    let inicio = std::time::Instant::now();
    let id = instance.id.clone();
    let store = store.clone();
    let instance = instance.clone();
    let resultado = bloqueante(move |cancel| analizar_bloqueante(&store, &instance, &cancel)).await;
    match &resultado {
        Ok(local) => registrar(format!(
            "revision-local instancia={id} archivos={} bytes={} releidos={} reutilizados={} ms={}",
            local.estado.len(),
            local.estado.iter().map(|e| e.bytes).sum::<u64>(),
            local.releidos,
            local.reutilizados,
            ms(inicio)
        )),
        Err(e) => registrar(format!("error fase=revision-local instancia={id} ms={} mensaje={e}", ms(inicio))),
    }
    resultado
}
fn analizar_bloqueante(
    store: &Almacen,
    instance: &Instancia,
    cancel: &CancellationToken,
) -> Result<AnalisisLocal, String> {
    let game = store.juego(&instance.id).map_err(err)?;
    let mut inventario_previo = Inventario::cargar(&game);
    let (lectura, estado) = {
        let _lock = guardia(&game).map_err(err)?;
        recuperar(&game).map_err(err)?;
        let dir = game.join("mods");
        // Los metadatos previos y posteriores descartan un archivo que cambió
        // mientras se calculaba su hash. Solo se leen los archivos nuevos,
        // modificados o con la verificación caducada.
        let antes = estado(&dir)?;
        let lectura = inventario_previo.leer(&dir, &antes, cancel)?;
        if estado(&dir)? != antes {
            return Err("Los mods cambiaron durante la revisión. Comprueba de nuevo.".into());
        }
        (lectura, antes)
    };
    let huella = lectura.huella;
    let mut instalados = inventario(store, &instance.id).map_err(err)?;
    for m in &mut instalados {
        let filename = if m.activo {
            m.archivo.clone()
        } else {
            format!("{}.disabled", m.archivo)
        };
        let sha = huella
            .iter()
            .find(|(n, _)| n == &filename)
            .ok_or("Los mods cambiaron durante la consulta.")?
            .1
            .clone();
        if !m.sha512.is_empty() && m.sha512.to_lowercase() != sha {
            return Err(format!(
                "{} fue modificado. Revísalo antes de sincronizar.",
                m.archivo
            ));
        }
        m.sha512 = sha;
    }
    let mut identidades = BTreeMap::new();
    if matches!(instance.loader.as_str(), "fabric" | "quilt") {
        let dir = game.join("mods");
        for m in &instalados {
            if cancel.is_cancelled() {
                return Err(CANCELADA.into());
            }
            let nombre = nombre_mod(m);
            // El descriptor solo se abre si el inventario no lo tiene para este mismo contenido.
            let id = match inventario_previo.identidad(&nombre, &m.sha512) {
                Some(id) => id,
                None => {
                    let id = identidad_fabric(&ruta_mod(&dir, m))?;
                    inventario_previo.recordar_identidad(&nombre, id.clone());
                    id
                }
            };
            if let Some(id) = id {
                identidades.insert(m.archivo.clone(), id);
            }
        }
    }
    if !instancia_sin_cambios(store, &instance.id, &estado, &huella, cancel)? {
        return Err("Los mods cambiaron durante la revisión. Comprueba de nuevo.".into());
    }
    // Perder el inventario solo obliga a releer; nunca impide sincronizar.
    if let Err(e) = inventario_previo.guardar(&game) {
        registrar(format!("aviso fase=inventario instancia={} mensaje={e}", instance.id));
    }
    Ok(AnalisisLocal {
        instancia: instance.id.clone(),
        huella,
        estado,
        instalados,
        identidades,
        releidos: lectura.releidos,
        reutilizados: lectura.reutilizados,
    })
}
fn instancia_sin_cambios(
    store: &Almacen,
    id: &str,
    estado_esperado: &[EstadoArchivo],
    huella: &[(String, String)],
    cancel: &CancellationToken,
) -> Result<bool, String> {
    let game = store.juego(id).map_err(err)?;
    let _lock = guardia(&game).map_err(err)?;
    recuperar(&game).map_err(err)?;
    sin_cambios(&game.join("mods"), estado_esperado, huella, cancel)
}
/// Publica solo mods comunes a cliente/servidor identificados por contenido.
pub async fn publicar(
    catalog: &Catalog,
    store: &Almacen,
    instance: &Instancia,
) -> Result<Indice, String> {
    let before = huella(store, &instance.id)?;
    let active: Vec<_> = before
        .iter()
        .filter(|(n, _)| n.ends_with(".jar") && !n.contains(['/', '\\']))
        .collect();
    if active.len() > 512 {
        return Err("La instancia supera 512 mods.".into());
    }
    let hashes = active.iter().map(|(_, h)| h.clone()).collect::<Vec<_>>();
    let versions = catalog.versions_from_hashes(&hashes).await.map_err(err)?;
    let mut entries = vec![];
    let mut candidatos = BTreeMap::new();
    let mut necesarios = BTreeSet::new();
    let mut projects = BTreeSet::new();
    for (name, sha) in active {
        let v = versions.get(sha).ok_or_else(|| {
            format!(
                "{name} no está identificado en Modrinth. El índice no se publicará incompleto."
            )
        })?;
        if !compatible(v, &instance.minecraft, &instance.loader) {
            return Err(format!(
                "{name} no corresponde al Minecraft y loader compartidos."
            ));
        }
        let p = catalog.project(&v.project_id).await.map_err(err)?;
        if !projects.insert(v.project_id.clone()) {
            return Err(
                "Hay dos archivos del mismo proyecto en la instancia del anfitrión.".into(),
            );
        }
        if p.client_side != "unsupported" && p.server_side != "unsupported" {
            necesarios.insert(v.project_id.clone());
        }
        candidatos.insert(
            v.project_id.clone(),
            (
                v.clone(),
                Entrada {
                    proyecto: v.project_id.clone(),
                    version: v.id.clone(),
                    sha512: sha.clone(),
                },
            ),
        );
    }
    // Un mod común puede necesitar una dependencia que solo se ejecute en el cliente.
    loop {
        let previous = necesarios.len();
        for project in necesarios.clone() {
            for dep in candidatos[&project]
                .0
                .dependencies
                .iter()
                .filter(|d| d.dependency_type == "required")
            {
                let candidate = candidatos.values().find(|(v, _)| {
                    dep.version_id.as_ref().map_or_else(
                        || dep.project_id.as_ref() == Some(&v.project_id),
                        |id| id == &v.id,
                    )
                });
                let (v, _) = candidate
                    .ok_or("Falta una dependencia obligatoria en la instancia del anfitrión.")?;
                necesarios.insert(v.project_id.clone());
            }
        }
        if previous == necesarios.len() {
            break;
        }
    }
    for project in necesarios {
        entries.push(candidatos.remove(&project).unwrap().1);
    }
    entries.sort_by(|a, b| a.proyecto.cmp(&b.proyecto));
    let index = Indice {
        esquema: 1,
        minecraft: instance.minecraft.clone(),
        loader: instance.loader.clone(),
        loader_version: crate::versiones_instancia::leer_loader(store, instance),
        mods: entries,
        archivos: vec![],
        avisos: vec![],
    };
    index.validar()?;
    if before != huella(store, &instance.id)? {
        return Err("Los mods del anfitrión cambiaron durante la publicación.".into());
    }
    Ok(index)
}
pub async fn preparar(
    source: &impl Fuente,
    store: &Almacen,
    instance: &Instancia,
    index: Indice,
    origen: String,
) -> Result<Plan, String> {
    let local = analizar(store, instance).await?;
    preparar_desde(source, store, instance, index, origen, local).await
}
/// Construye el plan con un análisis local ya calculado. Solo vuelve a leer
/// metadatos al final; los hashes no se recalculan.
pub async fn preparar_desde(
    source: &impl Fuente,
    store: &Almacen,
    instance: &Instancia,
    index: Indice,
    origen: String,
    local: AnalisisLocal,
) -> Result<Plan, String> {
    let inicio = std::time::Instant::now();
    let resultado = construir_plan(source, store, instance, index, origen.clone(), local).await;
    match &resultado {
        Ok(plan) => {
            let cuenta = |tipo: Tipo| plan.cambios.iter().filter(|c| c.tipo == tipo).count();
            registrar(format!(
                "plan instancia={} origen={origen} revision={} sin_red={} agregar={} actualizar={} quitar={} conflictos={} reemplazos={} descarga_bytes={} ms={}",
                instance.id,
                corta(&plan.revision_origen),
                plan.sin_red,
                cuenta(Tipo::Agregar),
                cuenta(Tipo::Actualizar),
                cuenta(Tipo::Quitar),
                plan.conflictos.len(),
                plan.reemplazos.len(),
                plan.cambios.iter().map(|c| c.bytes).sum::<u64>(),
                ms(inicio)
            ));
        }
        Err(e) => registrar(format!("error fase=plan instancia={} origen={origen} ms={} mensaje={e}", instance.id, ms(inicio))),
    }
    resultado
}
async fn construir_plan(
    source: &impl Fuente,
    store: &Almacen,
    instance: &Instancia,
    mut index: Indice,
    origen: String,
    local: AnalisisLocal,
) -> Result<Plan, String> {
    if local.instancia != instance.id {
        return Err("El análisis local corresponde a otra instancia.".into());
    }
    index.validar()?;
    let revision_origen = index.revision()?;
    if index.minecraft != instance.minecraft
        || index.loader != instance.loader
        || index.loader_version != crate::versiones_instancia::leer_loader(store, instance)
    {
        return Err("La instancia debe usar el mismo Minecraft, loader y versión del loader que el anfitrión. Cámbialos antes de sincronizar.".into());
    }
    // Mismo anfitrión, misma revisión y `mods` idéntica a la última sincronización
    // correcta: el resultado sería un plan vacío, así que no se consulta Modrinth.
    let game = store.juego(&instance.id).map_err(err)?;
    if let Some(verificado) = leer_verificado(&game).filter(|v| {
        v.origen == origen && v.revision == revision_origen && v.huella == resumen_huella(&local.huella)
    }) {
        let wanted: BTreeSet<_> = verificado.indice.mods.iter().map(|m| m.proyecto.clone()).collect();
        let conservados = local
            .instalados
            .iter()
            .filter(|m| {
                !wanted.contains(&m.project_id)
                    && !verificado.locales.iter().any(|a| a.archivo.eq_ignore_ascii_case(&m.archivo))
            })
            .map(|m| m.nombre.clone())
            .collect();
        return Ok(Plan {
            indice: verificado.indice,
            cambios: vec![],
            conservados,
            conflictos: vec![],
            reemplazos: BTreeSet::new(),
            reemplazos_autorizados: false,
            origen,
            huella: local.huella,
            estado: local.estado,
            identidades: local.identidades,
            actuales: local.instalados,
            destinos: vec![],
            quitar: BTreeSet::new(),
            locales: verificado.locales,
            quitar_locales: vec![],
            revision_origen,
            recuperadas: BTreeSet::new(),
            sin_red: true,
        });
    }
    let recuperadas = completar_dependencias_propias(source, &mut index).await?;
    let AnalisisLocal {
        huella: fingerprint,
        estado: estado_inicial,
        instalados: mut installed,
        identidades,
        ..
    } = local;
    let dir = store.juego(&instance.id).map_err(err)?.join("mods");
    // Los JAR importados pueden no tener identidad de catálogo todavía.
    // Una consulta por lote permite reconocer versiones anteriores sin adivinar por nombre.
    if index.mods.iter().any(|e| !installed.iter().any(|m| m.project_id == e.proyecto)) {
        let hashes: Vec<_> = installed.iter().filter(|m| m.activo && m.project_id.is_empty())
            .map(|m| m.sha512.clone()).collect();
        if !hashes.is_empty() {
            let reconocidas = source.versiones_por_hash(&hashes).await.map_err(err)?;
            for m in &mut installed {
                if m.project_id.is_empty() {
                    if let Some(v) = reconocidas.get(&m.sha512).filter(|v| v.files.iter().any(|f| f.hashes.sha512.eq_ignore_ascii_case(&m.sha512))) {
                        m.project_id = v.project_id.clone();
                        m.version_id = v.id.clone();
                        m.version = v.version_number.clone();
                    }
                }
            }
        }
    }
    let mut locales_previos = vec![];
    let managed = if dir.join(SYNC).exists() {
        if regular(&dir.join(SYNC), false).map_err(err)?.len() > MAX_INDEX as u64 {
            return Err("El registro de sincronización no es válido.".into());
        }
        let saved: Gestionados = serde_json::from_slice(&fs::read(dir.join(SYNC)).map_err(err)?)
            .map_err(|_| "El registro de sincronización no es válido.")?;
        if saved.origen == origen {
            let mut check = index.clone();
            check.mods.clear();
            check.avisos.clear();
            check.esquema = 2;
            check.archivos = saved.archivos.clone();
            check.validar()?;
            locales_previos = saved.archivos;
            saved.proyectos
        } else {
            BTreeSet::new()
        }
    } else {
        BTreeSet::new()
    };
    let wanted = index
        .mods
        .iter()
        .map(|m| m.proyecto.clone())
        .collect::<BTreeSet<_>>();
    let mut quitar = managed
        .difference(&wanted)
        .cloned()
        .collect::<BTreeSet<_>>();
    let por_quitar: Vec<_> = quitar.iter().cloned().collect();
    for project in source.solo_cliente_lote(&por_quitar).await.map_err(err)? {
        quitar.remove(&project);
    }
    let mut changes = vec![];
    let mut versions = vec![];
    let mut conflictos = vec![];
    let (locales, mut quitar_locales) = preparar_locales(
        &index,
        &installed,
        &locales_previos,
        &quitar,
        &mut changes,
        &mut conflictos,
        &identidades,
    )?;
    if !locales.is_empty() {
        direccion_archivos(&origen)?;
    }
    quitar.retain(|id| {
        !installed.iter().any(|m| {
            &m.project_id == id
                && locales
                    .iter()
                    .any(|a| a.sha512.eq_ignore_ascii_case(&m.sha512))
        })
    });
    let ids = index.mods.iter().map(|m| m.version.clone()).collect::<Vec<_>>();
    let verificadas = source.versiones_exactas(&ids).await.map_err(err)?;
    // Índices de cambios de Modrinth cuyo nombre se consulta en un solo lote.
    let mut sin_nombre = vec![];
    for entry in &index.mods {
        let mut v = verificadas.get(&entry.version).cloned().ok_or("Modrinth no confirmó una versión del anfitrión.")?;
        if v.id != entry.version
            || v.project_id != entry.proyecto
            || !compatible(&v, &index.minecraft, &index.loader)
        {
            return Err("Modrinth no confirma la versión solicitada por el anfitrión.".into());
        }
        let f = v
            .files
            .iter()
            .find(|f| f.hashes.sha512.eq_ignore_ascii_case(&entry.sha512))
            .cloned()
            .ok_or("Modrinth no confirma el archivo solicitado por el anfitrión.")?;
        if !nombre_valido(&f.filename) || !f.filename.ends_with(".jar") || f.size > MAX_FILE {
            return Err("El archivo solicitado no es válido.".into());
        }
        v.files = vec![f.clone()];
        if locales
            .iter()
            .any(|a| a.archivo.eq_ignore_ascii_case(&f.filename))
        {
            return Err("Un archivo propio y uno de Modrinth usan el mismo nombre.".into());
        }
        let old = installed
            .iter()
            .find(|m| m.project_id == entry.proyecto)
            .or_else(|| {
                installed
                    .iter()
                    .find(|m| m.sha512.eq_ignore_ascii_case(&entry.sha512))
            });
        if old.is_none_or(|m| !m.activo || !m.sha512.eq_ignore_ascii_case(&entry.sha512)) {
            sin_nombre.push(changes.len());
            changes.push(Cambio {
                proyecto: entry.proyecto.clone(),
                tipo: if old.is_some() {
                    Tipo::Actualizar
                } else {
                    Tipo::Agregar
                },
                // Se completa después con una sola consulta por lote.
                nombre: String::new(),
                antes: old.map(|m| m.version.clone()).unwrap_or_default(),
                despues: v.version_number.clone(),
                bytes: if old.is_some_and(|m| m.sha512.eq_ignore_ascii_case(&entry.sha512)) {
                    0
                } else {
                    f.size
                },
            });
        }
        versions.push(v);
    }
    if !sin_nombre.is_empty() {
        let ids: Vec<_> = sin_nombre.iter().map(|&i| changes[i].proyecto.clone()).collect();
        let nombres = source.nombres(&ids).await.map_err(err)?;
        for &i in &sin_nombre {
            let c = &mut changes[i];
            c.nombre = nombres
                .get(&c.proyecto)
                .cloned()
                .ok_or("Modrinth no devolvió el nombre de un proyecto del anfitrión.")?;
        }
    }
    // Dependencias obligatorias también deben estar fijadas en el índice, nunca elegir la última.
    let resolved = resolver(
        source,
        versions.clone(),
        &[],
        &index.minecraft,
        &index.loader,
    )
    .await
    .map_err(err)?;
    if resolved.iter().any(|v| {
        !versions
            .iter()
            .any(|p| p.id == v.id && p.project_id == v.project_id)
    }) {
        return Err("Al índice del anfitrión le falta una dependencia obligatoria exacta.".into());
    }
    for old in installed.iter().filter(|m| quitar.contains(&m.project_id)) {
        changes.push(Cambio {
            proyecto: old.project_id.clone(),
            tipo: Tipo::Quitar,
            nombre: old.nombre.clone(),
            antes: old.version.clone(),
            despues: String::new(),
            bytes: 0,
        });
    }
    let reemplazos = reconciliar_identidades(&installed, &identidades, &locales, &versions, &mut changes, &mut conflictos)?;
    quitar_locales.retain(|a| !reemplazos.contains(&a.archivo));
    let keep = installed
        .iter()
        .filter(|m| {
            !reemplazos.contains(&m.archivo) && !conflictos.contains(&m.archivo) && !locales
                .iter()
                .any(|a| a.archivo.eq_ignore_ascii_case(&m.archivo))
                && !quitar_locales
                    .iter()
                    .any(|a| a.archivo.eq_ignore_ascii_case(&m.archivo))
                && !quitar.contains(&m.project_id)
                && !wanted.contains(&m.project_id)
                && !index
                    .mods
                    .iter()
                    .any(|e| e.sha512.eq_ignore_ascii_case(&m.sha512))
        })
        .map(|m| m.nombre.clone())
        .collect();
    let (store_actual, id) = (store.clone(), instance.id.clone());
    let (esperado, verificada) = (estado_inicial.clone(), fingerprint.clone());
    if !bloqueante(move |token| {
        instancia_sin_cambios(&store_actual, &id, &esperado, &verificada, &token)
    })
    .await?
    {
        return Err("Los mods cambiaron durante la consulta. Comprueba de nuevo.".into());
    }
    Ok(Plan {
        indice: index,
        cambios: changes,
        conservados: keep,
        conflictos,
        reemplazos,
        reemplazos_autorizados: false,
        origen,
        huella: fingerprint,
        estado: estado_inicial,
        identidades,
        actuales: installed,
        destinos: versions,
        quitar,
        locales,
        quitar_locales,
        revision_origen,
        recuperadas,
        sin_red: false,
    })
}
fn identidad_fabric(path: &Path) -> Result<Option<String>, String> {
    Ok(
        dedicado::descriptor_fabric(path, &mut vec![])?.and_then(|d| {
            d.get("id")
                .and_then(|v| v.as_str())
                .filter(|id| !id.is_empty())
                .map(str::to_owned)
        }),
    )
}

// La identidad del descriptor prevalece sobre el nombre y la versión declarada:
// dos JAR pueden anunciar la misma versión y contener registros diferentes.
fn reconciliar_identidades(
    installed: &[Instalado],
    identidades: &BTreeMap<String, String>,
    locales: &[ArchivoPropio],
    versiones: &[Version],
    cambios: &mut Vec<Cambio>,
    conflictos: &mut Vec<String>,
) -> Result<BTreeSet<String>, String> {
    let mut destinos = Vec::new();
    for local in locales {
        let id = installed
            .iter()
            .find(|m| m.sha512.eq_ignore_ascii_case(&local.sha512))
            .and_then(|m| identidades.get(&m.archivo))
            .cloned()
            .or_else(|| local.id.strip_prefix("icaro-host-").map(str::to_owned));
        if let Some(id) = id {
            destinos.push((
                local.id.clone(),
                local.archivo.clone(),
                local.sha512.clone(),
                id,
            ));
        }
    }
    for version in versiones {
        let file = &version.files[0];
        let exacto = installed
            .iter()
            .find(|m| m.sha512.eq_ignore_ascii_case(&file.hashes.sha512));
        let anterior = exacto.or_else(|| {
            installed
                .iter()
                .find(|m| m.project_id == version.project_id)
        });
        if let Some(id) = anterior.and_then(|m| identidades.get(&m.archivo)) {
            destinos.push((
                version.project_id.clone(),
                exacto
                    .map(|m| m.archivo.clone())
                    .unwrap_or_else(|| file.filename.clone()),
                file.hashes.sha512.clone(),
                id.clone(),
            ));
        }
    }
    let mut requeridos = BTreeMap::new();
    let mut reemplazos = BTreeSet::new();
    for (proyecto, archivo, sha, id) in destinos {
        if requeridos
            .insert(id.clone(), sha.clone())
            .is_some_and(|otra| otra != sha)
        {
            return Err(format!(
                "El anfitrión publica dos JAR distintos del mismo mod {id}. Revisa el servidor."
            ));
        }
        for actual in installed.iter().filter(|m| {
            (m.activo || m.archivo.eq_ignore_ascii_case(&archivo))
                && identidades.get(&m.archivo) == Some(&id)
                && !(m.archivo.eq_ignore_ascii_case(&archivo)
                    && m.sha512.eq_ignore_ascii_case(&sha))
        }) {
            if !reemplazos.insert(actual.archivo.clone()) {
                continue;
            }
            conflictos.retain(|n| n != &actual.archivo);
            if let Some(cambio) = cambios.iter_mut().find(|c| c.proyecto == proyecto) {
                if cambio.tipo == Tipo::Agregar {
                    cambio.tipo = Tipo::Actualizar;
                }
                if cambio.antes.is_empty() {
                    cambio.antes = actual.version.clone();
                }
            } else {
                cambios.push(Cambio {
                    proyecto: format!("icaro-reemplazo-{}", hash(actual.archivo.as_bytes())),
                    tipo: Tipo::Quitar,
                    nombre: format!("{id} · copia anterior: {}", actual.archivo),
                    antes: actual.version.clone(),
                    despues: String::new(),
                    bytes: 0,
                });
            }
        }
    }
    Ok(reemplazos)
}

fn preparar_locales(
    index: &Indice,
    installed: &[Instalado],
    previos: &[ArchivoPropio],
    quitar: &BTreeSet<String>,
    changes: &mut Vec<Cambio>,
    conflictos: &mut Vec<String>,
    identidades: &BTreeMap<String, String>,
) -> Result<(Vec<ArchivoPropio>, Vec<ArchivoPropio>), String> {
    let mut wanted = vec![];
    for entry in &index.archivos {
        let previous = previos.iter().find(|a| a.id == entry.id);
        let current = installed.iter().find(|m| m.sha512.eq_ignore_ascii_case(&entry.sha512)).or_else(|| previous
            .and_then(|p| {
                installed
                    .iter()
                    .find(|m| m.archivo.eq_ignore_ascii_case(&p.archivo))
            }))
            .or_else(|| {
                installed
                    .iter()
                    .find(|m| m.sha512.eq_ignore_ascii_case(&entry.sha512))
            })
            .or_else(|| {
                installed
                    .iter()
                    .find(|m| m.archivo.eq_ignore_ascii_case(&entry.archivo))
            })
            .or_else(|| {
                installed.iter().find(|m| m.activo && identidades.get(&m.archivo)
                    .is_some_and(|id| entry.id.strip_prefix("icaro-host-") == Some(id.as_str())))
            });
        if let Some(m) = current {
            let same = m.sha512.eq_ignore_ascii_case(&entry.sha512);
            let owned = previous.is_some_and(|p| {
                p.archivo.eq_ignore_ascii_case(&m.archivo)
                    && p.sha512.eq_ignore_ascii_case(&m.sha512)
            });
            if !same && !owned && !quitar.contains(&m.project_id) {
                conflictos.push(m.archivo.clone());
            }
        }
        let mut local = entry.clone();
        if installed.iter().any(|m| {
            m.archivo.eq_ignore_ascii_case(&entry.archivo)
                && current.is_none_or(|c| c.archivo != m.archivo)
                && !identidades.get(&m.archivo).is_some_and(|id| entry.id.strip_prefix("icaro-host-") == Some(id.as_str()))
        }) {
            return Err(format!(
                "El nombre {} está ocupado por otro archivo local.",
                entry.archivo
            ));
        }
        if current.is_some_and(|m| m.sha512.eq_ignore_ascii_case(&entry.sha512)) {
            local.archivo = current.unwrap().archivo.clone();
        }
        if current.is_none_or(|m| !m.activo || !m.sha512.eq_ignore_ascii_case(&entry.sha512)) {
            changes.push(Cambio {
                proyecto: entry.id.clone(),
                tipo: if current.is_some() {
                    Tipo::Actualizar
                } else {
                    Tipo::Agregar
                },
                nombre: format!("{} · archivo del anfitrión", entry.nombre),
                antes: previous.map(|p| p.version.clone()).unwrap_or_default(),
                despues: entry.version.clone(),
                bytes: if current.is_some_and(|m| m.sha512.eq_ignore_ascii_case(&entry.sha512)) {
                    0
                } else {
                    entry.bytes
                },
            });
        }
        wanted.push(local);
    }
    let mut removed = vec![];
    for previous in previos {
        if wanted
            .iter()
            .any(|a| a.id == previous.id && a.archivo.eq_ignore_ascii_case(&previous.archivo))
            || index
                .mods
                .iter()
                .any(|m| m.sha512.eq_ignore_ascii_case(&previous.sha512))
        {
            continue;
        }
        if let Some(current) = installed
            .iter()
            .find(|m| m.archivo.eq_ignore_ascii_case(&previous.archivo))
        {
            if !current.sha512.eq_ignore_ascii_case(&previous.sha512) {
                return Err(format!(
                    "{} fue modificado. No se eliminará.",
                    previous.archivo
                ));
            }
            if !wanted
                .iter()
                .any(|a| a.archivo.eq_ignore_ascii_case(&previous.archivo))
            {
                removed.push(previous.clone());
                if !wanted.iter().any(|a| a.id == previous.id) {
                    changes.push(Cambio {
                        proyecto: previous.id.clone(),
                        tipo: Tipo::Quitar,
                        nombre: previous.nombre.clone(),
                        antes: previous.version.clone(),
                        despues: String::new(),
                        bytes: 0,
                    });
                }
            }
        }
    }
    if wanted.iter().any(|a| {
        index
            .mods
            .iter()
            .any(|e| e.sha512.eq_ignore_ascii_case(&a.sha512))
    }) {
        return Err("El índice repite un archivo propio y uno de Modrinth.".into());
    }
    Ok((wanted, removed))
}
fn direccion_archivos(origen: &str) -> Result<SocketAddr, String> {
    let addr: SocketAddr = origen
        .parse()
        .map_err(|_| "Origen de archivos propios inválido.".to_string())?;
    if addr.port() < 2
        || endpoint(&SocketAddr::new(addr.ip(), addr.port() - 1).to_string())? != addr
    {
        return Err("Origen de archivos propios inválido.".into());
    }
    Ok(addr)
}
/// Cliente compartido con el anfitrión: reutiliza conexiones entre el índice y
/// las descargas. Cierra las inactivas antes que el servicio (15 s).
fn cliente_anfitrion() -> Result<&'static reqwest::Client, String> {
    static CLIENTE: std::sync::OnceLock<reqwest::Client> = std::sync::OnceLock::new();
    if let Some(cliente) = CLIENTE.get() {
        return Ok(cliente);
    }
    let cliente = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(5))
        .pool_idle_timeout(Duration::from_secs(10))
        .build()
        .map_err(err)?;
    Ok(CLIENTE.get_or_init(|| cliente))
}

/// Descarga un archivo propio del anfitrión directamente a `destino`,
/// verificando tamaño y SHA-512 mientras llegan los bloques. Si la conexión se
/// corta, reanuda hasta dos veces desde lo recibido; si el anfitrión no admite
/// rangos o el archivo cambió, vuelve a empezar sin mezclar contenidos.
async fn descargar_propio_a(origen: &str, file: &ArchivoPropio, destino: &Path) -> Result<(), String> {
    use tokio::io::{AsyncSeekExt, AsyncWriteExt};
    let addr = direccion_archivos(origen)?;
    let cliente = cliente_anfitrion()?;
    let sha = file.sha512.to_lowercase();
    let url = format!("http://{addr}/icaro/v1/files/{sha}");
    let fallo_red = || format!("No se pudo descargar {} del anfitrión.", file.archivo);
    let mut salida = tokio::fs::File::create(destino).await.map_err(err)?;
    let (mut hasher, mut recibidos, mut reintentos) = (Sha512::new(), 0u64, 0);
    loop {
        let mut solicitud = cliente.get(&url).timeout(Duration::from_secs(120));
        if recibidos > 0 {
            solicitud = solicitud
                .header(reqwest::header::RANGE, format!("bytes={recibidos}-"))
                .header(reqwest::header::IF_RANGE, format!("\"{sha}\""));
        }
        let mut response = match solicitud.send().await {
            Ok(r) => r,
            Err(_) if reintentos < 2 && recibidos > 0 => {
                reintentos += 1;
                continue;
            }
            Err(_) => return Err(fallo_red()),
        };
        let estado = response.status().as_u16();
        if recibidos > 0 && estado == 206 {
            // Solo se continúa si el tramo empieza donde quedó y el total coincide.
            let esperado = format!("bytes {recibidos}-");
            let rango = response
                .headers()
                .get(reqwest::header::CONTENT_RANGE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            if !rango.starts_with(&esperado) || !rango.ends_with(&format!("/{}", file.bytes)) {
                return Err("El anfitrión respondió un tramo inesperado del archivo.".into());
            }
        } else if estado == 200 {
            if recibidos > 0 {
                salida.set_len(0).await.map_err(err)?;
                salida.seek(std::io::SeekFrom::Start(0)).await.map_err(err)?;
                (hasher, recibidos) = (Sha512::new(), 0);
            }
            if response.content_length().is_some_and(|n| n != file.bytes || n > MAX_FILE) {
                return Err("Tamaño de archivo del anfitrión inválido.".into());
            }
        } else {
            return Err("El archivo del anfitrión cambió o no está disponible. Comprueba de nuevo.".into());
        }
        let cortada = loop {
            match response.chunk().await {
                Ok(Some(bloque)) => {
                    recibidos = recibidos.saturating_add(bloque.len() as u64);
                    if recibidos > file.bytes {
                        return Err("El archivo supera el tamaño publicado.".into());
                    }
                    hasher.update(&bloque);
                    salida.write_all(&bloque).await.map_err(err)?;
                }
                Ok(None) => break false,
                Err(_) => break true,
            }
        };
        if !cortada && recibidos == file.bytes {
            break;
        }
        if reintentos >= 2 {
            return Err(fallo_red());
        }
        reintentos += 1;
    }
    salida.sync_all().await.map_err(err)?;
    let calculado: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if calculado != sha {
        return Err("El archivo del anfitrión no coincide con su tamaño o SHA-512.".into());
    }
    Ok(())
}

type Trabajo<'a> = std::pin::Pin<Box<dyn std::future::Future<Output = Result<String, String>> + Send + 'a>>;

/// Ejecuta hasta `limite` descargas a la vez. Avisa el inicio y el final de cada
/// una por su identificador y se detiene con el primer error, descartando el resto.
async fn en_paralelo<'a>(
    trabajos: Vec<(String, Trabajo<'a>)>,
    limite: usize,
    progress: &(impl Fn(&str, bool) + Send),
) -> Result<(), String> {
    let mut pendientes = trabajos.into_iter();
    let mut activos: Vec<Trabajo<'a>> = vec![];
    loop {
        while activos.len() < limite.max(1) {
            let Some((id, trabajo)) = pendientes.next() else { break };
            progress(&id, false);
            activos.push(trabajo);
        }
        if activos.is_empty() {
            return Ok(());
        }
        let terminado = std::future::poll_fn(|cx| {
            for i in 0..activos.len() {
                if let std::task::Poll::Ready(resultado) = activos[i].as_mut().poll(cx) {
                    // El trabajo terminó: se descarta su futuro ya completado.
                    drop(activos.swap_remove(i));
                    return std::task::Poll::Ready(resultado);
                }
            }
            std::task::Poll::Pending
        })
        .await?;
        progress(&terminado, true);
    }
}

pub async fn aplicar(
    source: &impl Fuente,
    store: &Almacen,
    instance: &Instancia,
    plan: &Plan,
    cancel: CancellationToken,
    progress: impl Fn(&str, bool) + Send,
) -> Result<(), String> {
    let inicio = std::time::Instant::now();
    registrar(format!(
        "aplicar-inicio instancia={} origen={} revision={} cambios={}",
        instance.id, plan.origen, corta(&plan.revision_origen), plan.cambios.len()
    ));
    let resultado = aplicar_transaccion(source, store, instance, plan, cancel.clone(), progress).await;
    match &resultado {
        Ok(()) => registrar(format!("aplicar-fin instancia={} resultado=ok ms={}", instance.id, ms(inicio))),
        Err(e) => registrar(format!(
            "error fase=aplicar instancia={} cancelada={} ms={} mensaje={e}",
            instance.id,
            cancel.is_cancelled(),
            ms(inicio)
        )),
    }
    resultado
}
async fn aplicar_transaccion(
    source: &impl Fuente,
    store: &Almacen,
    instance: &Instancia,
    plan: &Plan,
    cancel: CancellationToken,
    progress: impl Fn(&str, bool) + Send,
) -> Result<(), String> {
    let game = store.juego(&instance.id).map_err(err)?;
    if !plan.puede_aplicar() {
        return Err("Hay mods locales distintos de los del anfitrión. Revisa los conflictos y autoriza su respaldo antes de sincronizar.".into());
    }
    let _lock = guardia(&game).map_err(err)?;
    recuperar(&game).map_err(err)?;
    let dir = game.join("mods");
    // Los hashes del plan se verificaron al revisarlo; aquí basta detectar
    // cambios posteriores sin volver a leer cientos de MiB.
    let (carpeta, esperado, verificada) = (dir.clone(), plan.estado.clone(), plan.huella.clone());
    if cancel.is_cancelled()
        || !bloqueante(move |token| sin_cambios(&carpeta, &esperado, &verificada, &token)).await?
    {
        return Err("El plan fue cancelado o los mods cambiaron. Comprueba de nuevo.".into());
    }
    let current = store
        .cargar()
        .map_err(err)?
        .instancias
        .into_iter()
        .find(|i| i.id == instance.id)
        .ok_or("La instancia ya no existe.")?;
    if current.minecraft != plan.indice.minecraft
        || current.loader != plan.indice.loader
        || crate::versiones_instancia::leer_loader(store, &current) != plan.indice.loader_version
    {
        return Err("El perfil de la instancia cambió. Comprueba de nuevo.".into());
    }
    // Si este origen ya gestiona exactamente el conjunto verificado, no hay que
    // copiar ni publicar de nuevo toda la carpeta antes de cada partida.
    if plan.cambios.is_empty() && dir.join(SYNC).exists() {
        let saved: Gestionados = serde_json::from_slice(&fs::read(dir.join(SYNC)).map_err(err)?)
            .map_err(|_| "El registro de sincronización no es válido.")?;
        let wanted = plan.indice.mods.iter()
            .map(|m| m.proyecto.clone()).collect::<BTreeSet<_>>();
        if saved.origen == plan.origen
            && saved.proyectos == wanted
            && saved.archivos == plan.locales
        {
            if cancel.is_cancelled() {
                return Err("Sincronización cancelada. Los mods anteriores se conservan.".into());
            }
            registrar(format!("al-dia instancia={} sin_movimientos=true", instance.id));
            // La carpeta coincide con `plan.huella`, verificada al revisar. Un plan
            // sin red no renueva la fecha: la vigencia cuenta desde la última consulta.
            if plan.sin_red {
                return Ok(());
            }
            if let Err(e) = guardar_verificado(&game, plan, &plan.huella) {
                registrar(format!("aviso fase=verificado instancia={} mensaje={e}", instance.id));
            }
            return Ok(());
        }
    }
    // Un plan sin red no contiene destinos: nunca debe ejecutar una transacción.
    if plan.sin_red {
        return Err("La carpeta de mods ya no coincide con la última sincronización. Comprueba de nuevo.".into());
    }
    let mut random = [0u8; 16];
    getrandom::fill(&mut random).map_err(err)?;
    let id_tx = random.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let tx_nombre = format!("{PREFIJO_TX}{id_tx}");
    let tx = game.join(&tx_nombre);
    fs::create_dir(&tx).map_err(err)?;
    fs::create_dir(tx.join("nuevos")).map_err(err)?;
    let result = async {
        // Estado virtual de `mods`: se planifican los movimientos sin tocar la carpeta.
        let mut virtual_ = Virtual::nuevo(&dir, &plan.huella);
        let mut ops: Vec<Op> = vec![];
        let mut descargas: Vec<(usize, Descarga)> = vec![];
        let mut solo_servidor = vec![];
        let respaldos: BTreeSet<_> = plan.conflictos.iter().chain(plan.reemplazos.iter()).cloned().collect();
        for nombre in &respaldos {
            let anterior = plan.actuales.iter().find(|m| &m.archivo == nombre).ok_or("El conflicto ya no existe.")?;
            let actual = nombre_mod(anterior);
            virtual_.retirar(&actual)?;
            ops.push(Op::Respaldar(actual));
        }
        let actuales: Vec<_> = plan.actuales.iter().filter(|m| !respaldos.contains(&m.archivo)).cloned().collect();
        let wanted = plan.indice.mods.iter().map(|m| m.proyecto.clone()).collect::<BTreeSet<_>>();
        let mut records = actuales.iter().filter(|m| !plan.quitar.contains(&m.project_id) && !wanted.contains(&m.project_id)).cloned().collect::<Vec<_>>();
        for old in actuales.iter().filter(|m| plan.quitar.contains(&m.project_id)) {
            let actual = nombre_mod(old);
            virtual_.retirar(&actual)?;
            ops.push(Op::Retirar(actual));
        }
        for v in &plan.destinos {
            let file = &v.files[0];
            let old = actuales.iter().find(|m| m.sha512.eq_ignore_ascii_case(&file.hashes.sha512)).or_else(|| actuales.iter().find(|m| m.project_id == v.project_id));
            let same = old.is_some_and(|m| m.sha512.eq_ignore_ascii_case(&file.hashes.sha512));
            let filename = if same { old.unwrap().archivo.clone() } else { file.filename.clone() };
            if actuales.iter().any(|m| !plan.quitar.contains(&m.project_id) && m.archivo.eq_ignore_ascii_case(&filename) && old.is_none_or(|o| o.archivo != m.archivo)) {
                return Err(format!("El nombre {filename} está ocupado por un archivo local."));
            }
            if (virtual_.existe(&filename) || virtual_.existe(&format!("{filename}.disabled"))) && old.is_none_or(|m| m.archivo.to_lowercase() != filename.to_lowercase()) {
                return Err(format!("El nombre {filename} está ocupado por un archivo local."));
            }
            if same {
                let old = old.unwrap();
                if !old.activo {
                    let desde = nombre_mod(old);
                    virtual_.activar(&desde, &filename)?;
                    ops.push(Op::Activar { desde, hasta: filename.clone() });
                }
            } else {
                let indice = descargas.len();
                descargas.push((indice, Descarga::Modrinth(v.project_id.clone(), file.clone())));
                if let Some(old) = old {
                    let actual = nombre_mod(old);
                    virtual_.retirar(&actual)?;
                    ops.push(Op::Retirar(actual));
                }
                virtual_.colocar(&filename, tx.join("nuevos").join(indice.to_string()))?;
                ops.push(Op::Colocar { indice, nombre: filename.clone() });
            }
            if plan.recuperadas.contains(&v.project_id) && matches!(instance.loader.as_str(), "fabric" | "quilt") {
                solo_servidor.push((v.project_id.clone(), filename.clone()));
            }
            records.retain(|m| m.archivo != filename && m.project_id != v.project_id);
            let nombre = plan.cambios.iter().find(|c| c.proyecto == v.project_id).map(|c| c.nombre.clone()).or_else(|| old.map(|m| m.nombre.clone())).unwrap_or_else(|| v.name.clone());
            records.push(Instalado { project_id: v.project_id.clone(), nombre, version_id: v.id.clone(), version: v.version_number.clone(), publicada: v.date_published.clone(), archivo: filename, sha512: file.hashes.sha512.clone(), activo: true, dependencias: v.dependencies.clone() });
        }
        for old in &plan.quitar_locales {
            let disabled = format!("{}.disabled", old.archivo);
            if virtual_.existe(&old.archivo) {
                virtual_.retirar(&old.archivo)?;
                ops.push(Op::Retirar(old.archivo.clone()));
            } else if virtual_.existe(&disabled) {
                virtual_.retirar(&disabled)?;
                ops.push(Op::Retirar(disabled));
            }
            records.retain(|m| !m.archivo.eq_ignore_ascii_case(&old.archivo));
        }
        for local in &plan.locales {
            let disabled = format!("{}.disabled", local.archivo);
            // Los hashes de la revisión siguen vigentes: la carpeta no cambió.
            let verificado = |nombre: &str| plan.huella.iter().any(|(n, h)| n == nombre && h.eq_ignore_ascii_case(&local.sha512));
            let same = if virtual_.existe(&local.archivo) { verificado(&local.archivo) } else if virtual_.existe(&disabled) { verificado(&disabled) } else { false };
            if same {
                if !virtual_.existe(&local.archivo) {
                    virtual_.activar(&disabled, &local.archivo)?;
                    ops.push(Op::Activar { desde: disabled, hasta: local.archivo.clone() });
                }
            } else {
                let indice = descargas.len();
                descargas.push((indice, Descarga::Propio(local.clone())));
                if virtual_.existe(&local.archivo) {
                    virtual_.retirar(&local.archivo)?;
                    ops.push(Op::Retirar(local.archivo.clone()));
                }
                if virtual_.existe(&disabled) {
                    virtual_.retirar(&disabled)?;
                    ops.push(Op::Retirar(disabled));
                }
                virtual_.colocar(&local.archivo, tx.join("nuevos").join(indice.to_string()))?;
                ops.push(Op::Colocar { indice, nombre: local.archivo.clone() });
            }
            records.retain(|m| !m.archivo.eq_ignore_ascii_case(&local.archivo));
        }
        // Solo se descarga lo que falta; la carpeta activa no cambia hasta el commit.
        // Cada archivo va directo a disco con su hash calculado por bloques: hasta
        // cuatro a la vez desde Modrinth y dos desde el anfitrión.
        if cancel.is_cancelled() {
            return Err("Sincronización cancelada. Los mods anteriores se conservan.".into());
        }
        let mut de_modrinth: Vec<(String, Trabajo<'_>)> = vec![];
        let mut del_anfitrion: Vec<(String, Trabajo<'_>)> = vec![];
        for (indice, descarga) in &descargas {
            let (id, destino, instancia) = (descarga.id(), tx.join("nuevos").join(indice.to_string()), instance.id.clone());
            match descarga {
                Descarga::Modrinth(_, file) => {
                    let id_registro = id.clone();
                    de_modrinth.push((id, Box::pin(async move {
                        let inicio = std::time::Instant::now();
                        source.descargar_a(file, &destino).await.map_err(err)?;
                        registrar(format!("descarga instancia={instancia} id={id_registro} archivo={} fuente=modrinth bytes={} ms={}", file.filename, file.size, ms(inicio)));
                        Ok(id_registro)
                    })));
                }
                Descarga::Propio(local) => {
                    let (id_registro, origen) = (id.clone(), plan.origen.clone());
                    del_anfitrion.push((id, Box::pin(async move {
                        let inicio = std::time::Instant::now();
                        descargar_propio_a(&origen, local, &destino).await?;
                        registrar(format!("descarga instancia={instancia} id={id_registro} archivo={} fuente=anfitrion bytes={} ms={}", local.archivo, local.bytes, ms(inicio)));
                        Ok(id_registro)
                    })));
                }
            }
        }
        let descargar = async {
            en_paralelo(de_modrinth, 4, &progress).await?;
            en_paralelo(del_anfitrion, 2, &progress).await
        };
        tokio::select! {
            _ = cancel.cancelled() => return Err("Sincronización cancelada. Los mods anteriores se conservan.".into()),
            r = descargar => r?,
        }
        records.retain(|m| !m.project_id.is_empty());
        let meta = serde_json::to_vec_pretty(&records).map_err(err)?;
        let sync = serde_json::to_vec(&Gestionados { origen: plan.origen.clone(), proyectos: wanted.clone(), archivos: plan.locales.clone() }).map_err(err)?;
        for (nombre, bytes) in [(META, meta), (SYNC, sync)] {
            let indice = descargas.len() + ops.len();
            guardar_archivo(&tx.join("nuevos").join(indice.to_string()), &bytes).map_err(err)?;
            if virtual_.existe(nombre) {
                virtual_.retirar(nombre)?;
                ops.push(Op::Retirar(nombre.into()));
            }
            virtual_.colocar(nombre, tx.join("nuevos").join(indice.to_string()))?;
            ops.push(Op::Colocar { indice, nombre: nombre.into() });
        }
        // Las comprobaciones de identidad usan el estado final, no la carpeta actual.
        let fabric = matches!(instance.loader.as_str(), "fabric" | "quilt");
        let requeridos_archivos: Vec<_> = plan.locales.iter().map(|l| l.archivo.clone())
            .chain(records.iter().filter(|r| wanted.contains(&r.project_id)).map(|r| r.archivo.clone()))
            .collect();
        let final_ = virtual_.clone();
        let cancelado = cancel.clone();
        let (carpeta, conocidas) = (dir.clone(), plan.identidades.clone());
        bloqueante(move |token| {
            // Los archivos que siguen en `mods` no cambiaron desde la revisión:
            // se reutiliza su identidad y solo se abren los JAR descargados.
            let identidad = |ruta: &Path| -> Result<Option<String>, String> {
                if ruta.parent() == Some(carpeta.as_path()) {
                    let nombre = ruta.file_name().unwrap_or_default().to_string_lossy();
                    let base = nombre.strip_suffix(".disabled").unwrap_or(&nombre);
                    return Ok(conocidas.get(base).cloned());
                }
                identidad_fabric(ruta)
            };
            for (proyecto, archivo) in &solo_servidor {
                if dedicado::descriptor_fabric(&final_.ruta(archivo)?, &mut vec![])?
                    .is_some_and(|d| d.get("environment").and_then(|v| v.as_str()) == Some("server"))
                {
                    return Err(format!("La dependencia {proyecto} declara que solo admite servidor. El conjunto anterior se conserva."));
                }
            }
            if !fabric {
                return Ok(());
            }
            let mut requeridos = BTreeSet::new();
            for archivo in &requeridos_archivos {
                if let Some(id) = identidad(&final_.ruta(archivo)?)? { requeridos.insert(id); }
            }
            let mut vistos = BTreeSet::new();
            for (nombre, ruta) in final_.archivos() {
                if token.is_cancelled() || cancelado.is_cancelled() {
                    return Err("Sincronización cancelada. Los mods anteriores se conservan.".into());
                }
                if nombre.ends_with(".jar") {
                    if let Some(id) = identidad(ruta)? {
                        if requeridos.contains(&id) && !vistos.insert(id.clone()) {
                            return Err(format!("Quedaron dos versiones activas del mod {id}. No se publicó la sincronización."));
                        }
                    }
                }
            }
            Ok(())
        })
        .await?;
        if cancel.is_cancelled() {
            return Err("Sincronización cancelada. Los mods anteriores se conservan.".into());
        }
        // Commit: el diario se escribe antes de mover el primer archivo y
        // permite deshacer la transacción tras un cierre abrupto.
        let (juego, carpeta, esperado, verificada) = (game.clone(), dir.clone(), plan.estado.clone(), plan.huella.clone());
        let tx_commit = tx_nombre.clone();
        let respaldo = format!(".icaro-mods-conflictos-{id_tx}");
        let (instancia, inicio_commit, total_ops) = (instance.id.clone(), std::time::Instant::now(), ops.len());
        bloqueante(move |token| {
            if !sin_cambios(&carpeta, &esperado, &verificada, &token)? {
                return Err("Los mods cambiaron o se canceló la sincronización. El conjunto anterior se conserva.".into());
            }
            let movimientos = movimientos(&ops, &tx_commit, &respaldo);
            if !carpeta.exists() {
                // Instalación desde cero: una carpeta vacía es inocua si se deshace.
                fs::create_dir(&carpeta).map_err(err)?;
            }
            if ops.iter().any(|op| matches!(op, Op::Respaldar(_))) {
                // El respaldo de conflictos permanece después de confirmar.
                fs::create_dir(juego.join(&respaldo)).map_err(err)?;
            }
            fs::create_dir(juego.join(&tx_commit).join("retirados")).map_err(err)?;
            ejecutar(&juego, &tx_commit, &movimientos)?;
            registrar(format!("commit instancia={instancia} tx={tx_commit} movimientos={total_ops} ms={}", ms(inicio_commit)));
            Ok(())
        })
        .await?;
        // La sincronización ya está publicada: un fallo aquí solo pierde el atajo sin red.
        let (juego, carpeta, publicado) = (game.clone(), dir.clone(), plan.clone());
        let registro = bloqueante(move |token| {
            let mut inventario = Inventario::cargar(&juego);
            let lectura = inventario.leer(&carpeta, &estado(&carpeta)?, &token)?;
            let _ = inventario.guardar(&juego);
            guardar_verificado(&juego, &publicado, &lectura.huella)
        })
        .await;
        if let Err(e) = registro {
            registrar(format!("aviso fase=verificado instancia={} mensaje={e}", instance.id));
        }
        // Los cambios sin descarga (retiradas, activaciones) terminan con el commit.
        for c in &plan.cambios {
            if !descargas.iter().any(|(_, d)| d.id() == c.proyecto) {
                progress(&c.proyecto, true);
            }
        }
        Ok(())
    }
    .await;
    if tx.exists() {
        // Si quedó un diario sin confirmar, `recuperar` lo resolverá; nunca se borra a ciegas.
        if !tx.join(DIARIO).exists() || leer_diario(&tx).is_ok_and(|d| d.confirmada) {
            let _ = borrar_preparacion(&game, &tx);
        }
    }
    result
}

const PREFIJO_TX: &str = ".icaro-mods-tx-";
const DIARIO: &str = "diario.json";

#[derive(Debug, Clone)]
enum Descarga {
    Modrinth(String, ModFile),
    Propio(ArchivoPropio),
}
impl Descarga {
    fn id(&self) -> String {
        match self {
            Descarga::Modrinth(proyecto, _) => proyecto.clone(),
            Descarga::Propio(local) => local.id.clone(),
        }
    }
}

/// Paso del plan sobre la carpeta `mods`; todos se ejecutan como renombrados.
#[derive(Debug, Clone)]
enum Op {
    Retirar(String),
    Respaldar(String),
    Activar { desde: String, hasta: String },
    Colocar { indice: usize, nombre: String },
}

fn nombre_mod(m: &Instalado) -> String {
    if m.activo {
        m.archivo.clone()
    } else {
        format!("{}.disabled", m.archivo)
    }
}

/// Contenido previsto de `mods` (nombres de primer nivel) y la ruta de la
/// que procede cada archivo al confirmar.
#[derive(Debug, Clone)]
struct Virtual {
    archivos: BTreeMap<String, (String, PathBuf)>,
}
impl Virtual {
    fn nuevo(dir: &Path, huella: &[(String, String)]) -> Self {
        Self {
            archivos: huella
                .iter()
                .filter(|(n, _)| !n.contains(['/', '\\']))
                .map(|(n, _)| (n.to_lowercase(), (n.clone(), dir.join(n))))
                .collect(),
        }
    }
    fn existe(&self, nombre: &str) -> bool {
        self.archivos.contains_key(&nombre.to_lowercase())
    }
    fn retirar(&mut self, nombre: &str) -> Result<(), String> {
        self.archivos
            .remove(&nombre.to_lowercase())
            .map(|_| ())
            .ok_or_else(|| format!("{nombre} ya no está en la carpeta de mods. Comprueba de nuevo."))
    }
    fn colocar(&mut self, nombre: &str, origen: PathBuf) -> Result<(), String> {
        if !nombre_valido(nombre.strip_suffix(".disabled").unwrap_or(nombre)) && nombre != META && nombre != SYNC {
            return Err("Nombre de mod inválido.".into());
        }
        if self.existe(nombre) {
            return Err(format!("El nombre {nombre} está ocupado por un archivo local."));
        }
        self.archivos.insert(nombre.to_lowercase(), (nombre.into(), origen));
        Ok(())
    }
    fn activar(&mut self, desde: &str, hasta: &str) -> Result<(), String> {
        let (_, origen) = self
            .archivos
            .remove(&desde.to_lowercase())
            .ok_or_else(|| format!("{desde} ya no está en la carpeta de mods. Comprueba de nuevo."))?;
        self.colocar(hasta, origen)
    }
    fn ruta(&self, nombre: &str) -> Result<PathBuf, String> {
        self.archivos
            .get(&nombre.to_lowercase())
            .map(|(_, ruta)| ruta.clone())
            .ok_or_else(|| format!("Falta {nombre} en el conjunto preparado."))
    }
    fn archivos(&self) -> impl Iterator<Item = (&String, &PathBuf)> {
        self.archivos.values().map(|(n, r)| (n, r))
    }
}

/// Renombrado registrado en el diario, con rutas relativas a la carpeta de juego.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Movimiento {
    origen: String,
    destino: String,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Diario {
    esquema: u32,
    confirmada: bool,
    movimientos: Vec<Movimiento>,
}

fn movimientos(ops: &[Op], tx: &str, respaldo: &str) -> Vec<Movimiento> {
    let mod_ = |n: &str| format!("mods/{n}");
    ops.iter()
        .enumerate()
        .map(|(i, op)| match op {
            Op::Retirar(n) => Movimiento { origen: mod_(n), destino: format!("{tx}/retirados/{i}") },
            Op::Respaldar(n) => Movimiento { origen: mod_(n), destino: format!("{respaldo}/{n}") },
            Op::Activar { desde, hasta } => Movimiento { origen: mod_(desde), destino: mod_(hasta) },
            Op::Colocar { indice, nombre } => Movimiento { origen: format!("{tx}/nuevos/{indice}"), destino: mod_(nombre) },
        })
        .collect()
}

/// Solo acepta rutas creadas por la transacción: `mods/<archivo>` o un archivo
/// dentro de una carpeta `.icaro-mods-*` de la carpeta de juego.
fn ruta_diario(game: &Path, relativa: &str) -> Result<PathBuf, Error> {
    let partes: Vec<_> = relativa.split('/').collect();
    let valida = |p: &&str| !p.is_empty() && *p != "." && *p != ".." && !p.contains(['\\', ':']);
    let permitida = match partes.as_slice() {
        ["mods", archivo] => {
            *archivo == META
                || *archivo == SYNC
                || nombre_valido(archivo.strip_suffix(".disabled").unwrap_or(archivo))
        }
        [carpeta, archivo] => carpeta.starts_with(".icaro-mods-") && valida(archivo),
        [carpeta, sub, archivo] => {
            carpeta.starts_with(PREFIJO_TX)
                && matches!(*sub, "nuevos" | "retirados")
                && archivo.parse::<usize>().is_ok()
        }
        _ => false,
    };
    if !permitida || !partes.iter().all(valida) {
        return Err(Error::Datos);
    }
    Ok(partes.iter().fold(game.to_path_buf(), |r, p| r.join(p)))
}

fn escribir_diario(tx: &Path, diario: &Diario) -> Result<(), String> {
    crate::almacenamiento::escribir_json(
        &tx.join(DIARIO),
        &serde_json::to_value(diario).map_err(err)?,
    )
}
fn leer_diario(tx: &Path) -> Result<Diario, Error> {
    let ruta = tx.join(DIARIO);
    if regular(&ruta, false)?.len() > 4 * 1024 * 1024 {
        return Err(Error::Datos);
    }
    let diario: Diario = serde_json::from_slice(&fs::read(ruta).map_err(|_| Error::Disco)?)
        .map_err(|_| Error::Datos)?;
    if diario.esquema != 1 || diario.movimientos.len() > 8192 {
        return Err(Error::Datos);
    }
    Ok(diario)
}

/// Mueve solo los archivos afectados. Ante un fallo deshace lo realizado; si
/// tampoco puede deshacerse, el diario queda para la recuperación posterior.
fn ejecutar(game: &Path, tx_nombre: &str, movimientos: &[Movimiento]) -> Result<(), String> {
    let tx = game.join(tx_nombre);
    let mut diario = Diario { esquema: 1, confirmada: false, movimientos: movimientos.to_vec() };
    escribir_diario(&tx, &diario)?;
    let mut hechos = 0;
    let resultado = (|| {
        for m in movimientos {
            let origen = ruta_diario(game, &m.origen).map_err(err)?;
            let destino = ruta_diario(game, &m.destino).map_err(err)?;
            regular(&origen, false).map_err(err)?;
            if fs::symlink_metadata(&destino).is_ok() {
                return Err(format!("{} ya existe. No se sobrescribirá.", m.destino));
            }
            fs::rename(&origen, &destino).map_err(err)?;
            hechos += 1;
        }
        Ok(())
    })();
    if let Err(e) = resultado {
        // Solo se deshace lo que este proceso movió; lo pendiente sigue intacto.
        let deshecho = deshacer(game, &movimientos[..hechos]);
        registrar(format!(
            "rollback tx={tx_nombre} hechos={hechos} total={} resultado={} causa={e}",
            movimientos.len(),
            if deshecho.is_ok() { "deshecho" } else { "pendiente-recuperacion" }
        ));
        return match deshecho {
            Ok(()) => {
                let _ = fs::remove_file(tx.join(DIARIO));
                Err(format!("No se pudo publicar la sincronización. El conjunto anterior se conserva. {e}"))
            }
            Err(_) => Err(format!("No se pudo publicar la sincronización ni deshacerla por completo; se reintentará al volver a abrir la instancia. {e}")),
        };
    }
    diario.confirmada = true;
    escribir_diario(&tx, &diario)
}

/// Deshace en orden inverso los movimientos completados. Un archivo presente en
/// ambos extremos es un cambio ajeno: se detiene sin sobrescribirlo.
fn deshacer(game: &Path, movimientos: &[Movimiento]) -> Result<(), Error> {
    for m in movimientos.iter().rev() {
        let origen = ruta_diario(game, &m.origen)?;
        let destino = ruta_diario(game, &m.destino)?;
        let en_origen = fs::symlink_metadata(&origen).is_ok();
        let en_destino = fs::symlink_metadata(&destino).is_ok();
        match (en_origen, en_destino) {
            (false, true) => {
                regular(&destino, false)?;
                fs::rename(&destino, &origen).map_err(|_| Error::Disco)?;
            }
            (true, true) => return Err(Error::Archivo(m.origen.clone())),
            _ => {}
        }
    }
    Ok(())
}

/// Resuelve transacciones de sincronización interrumpidas. Se ejecuta con el
/// bloqueo de la instancia tomado, antes de leer o modificar `mods`.
pub(super) fn recuperar_transacciones(game: &Path) -> Result<(), Error> {
    for entrada in fs::read_dir(game).map_err(|_| Error::Disco)? {
        let entrada = entrada.map_err(|_| Error::Disco)?;
        let nombre = entrada.file_name().to_string_lossy().into_owned();
        if !nombre.starts_with(PREFIJO_TX) {
            continue;
        }
        let tx = entrada.path();
        regular(&tx, true)?;
        let instancia = game
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if tx.join(DIARIO).exists() {
            let diario = leer_diario(&tx)?;
            if !diario.confirmada {
                if let Err(e) = deshacer(game, &diario.movimientos) {
                    registrar(format!("error fase=recuperacion carpeta={instancia} tx={nombre} mensaje={e}"));
                    return Err(e);
                }
                registrar(format!(
                    "recuperacion carpeta={instancia} tx={nombre} accion=deshecha movimientos={}",
                    diario.movimientos.len()
                ));
            } else {
                registrar(format!("recuperacion carpeta={instancia} tx={nombre} accion=limpieza-confirmada"));
            }
        } else {
            registrar(format!("recuperacion carpeta={instancia} tx={nombre} accion=descartar-descargas"));
        }
        borrar_preparacion(game, &tx)?;
    }
    for entrada in fs::read_dir(game).map_err(|_| Error::Disco)? {
        // Un respaldo de conflictos vacío solo puede venir de un commit deshecho.
        let ruta = entrada.map_err(|_| Error::Disco)?.path();
        if ruta.file_name().is_some_and(|n| n.to_string_lossy().starts_with(".icaro-mods-conflictos-"))
            && fs::read_dir(&ruta).is_ok_and(|mut d| d.next().is_none())
        {
            let _ = fs::remove_dir(&ruta);
        }
    }
    Ok(())
}
pub fn endpoint(address: &str) -> Result<SocketAddr, String> {
    let d = crate::servidores::direccion(address)?;
    let ip = d
        .host
        .parse::<IpAddr>()
        .map_err(|_| "Usa la IP privada de ZeroTier para consultar la sincronización.")?;
    let private = match ip {
        IpAddr::V4(ip) => ip.is_private() || ip.is_loopback(),
        IpAddr::V6(ip) => ip.is_loopback() || ip.segments()[0] & 0xfe00 == 0xfc00,
    };
    if !private {
        return Err("La sincronización del anfitrión requiere una IP privada de ZeroTier.".into());
    }
    Ok(SocketAddr::new(
        ip,
        d.puerto
            .checked_add(1)
            .ok_or("El puerto de sincronización no es válido.")?,
    ))
}
pub async fn consultar(addr: SocketAddr, cancel: CancellationToken) -> Result<Indice, String> {
    let inicio = std::time::Instant::now();
    let resultado = consultar_indice(addr, cancel).await;
    match &resultado {
        Ok(indice) => registrar(format!(
            "indice anfitrion={addr} mods={} archivos_propios={} avisos={} ms={}",
            indice.mods.len(),
            indice.archivos.len(),
            indice.avisos.len(),
            ms(inicio)
        )),
        Err(e) => registrar(format!("error fase=indice anfitrion={addr} ms={} mensaje={e}", ms(inicio))),
    }
    resultado
}
async fn consultar_indice(addr: SocketAddr, cancel: CancellationToken) -> Result<Indice, String> {
    // Último índice recibido de cada anfitrión con su ETag: si no cambió, el
    // servicio responde 304 y no se vuelve a descargar ni a decodificar.
    static RECIBIDOS: std::sync::Mutex<BTreeMap<SocketAddr, (String, Indice)>> =
        std::sync::Mutex::new(BTreeMap::new());
    if addr.port() < 2 {
        return Err("Puerto de sincronización inválido.".into());
    }
    endpoint(&SocketAddr::new(addr.ip(), addr.port() - 1).to_string())?;
    let client = cliente_anfitrion()?;
    let previo = RECIBIDOS.lock().ok().and_then(|r| r.get(&addr).cloned());
    let work = async {
        let mut solicitud = client.get(format!("http://{addr}{RUTA}")).timeout(Duration::from_secs(5));
        if let Some((etiqueta, _)) = &previo {
            solicitud = solicitud.header(reqwest::header::IF_NONE_MATCH, etiqueta);
        }
        let mut response=solicitud.send().await.map_err(|_|format!("No responde el servicio de mods en {addr}. El anfitrión debe ejecutar icaro-sync-host y permitir ese puerto en su cortafuegos. Estar conectado a ZeroTier no inicia este servicio."))?;
        if response.status() == reqwest::StatusCode::NOT_MODIFIED {
            if let Some((_, indice)) = previo {
                indice.validar()?;
                return Ok(indice);
            }
        }
        if !response.status().is_success() {
            return Err("El anfitrión todavía no tiene un índice de mods disponible.".into());
        }
        let etiqueta = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|v| v.to_str().ok())
            .filter(|v| v.len() <= 256)
            .map(str::to_owned);
        if response
            .content_length()
            .is_some_and(|n| n > MAX_INDEX as u64)
        {
            return Err("El índice supera el tamaño permitido.".into());
        }
        let mut bytes = vec![];
        while let Some(chunk) = response.chunk().await.map_err(err)? {
            if bytes.len() + chunk.len() > MAX_INDEX {
                return Err("El índice supera el tamaño permitido.".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        let index: Indice = serde_json::from_slice(&bytes)
            .map_err(|_| "El servicio no entregó un índice de mods válido.")?;
        index.validar()?;
        // Un anfitrión antiguo no envía ETag y siempre entrega el índice completo.
        if let (Some(etiqueta), Ok(mut recibidos)) = (etiqueta, RECIBIDOS.lock()) {
            recibidos.insert(addr, (etiqueta, index.clone()));
        }
        Ok(index)
    };
    tokio::select! {_=cancel.cancelled()=>Err("Consulta cancelada.".into()), r=work=>r}
}
pub type Publicacion = Arc<RwLock<Option<Indice>>>;
pub async fn servir(
    listener: TcpListener,
    index: Publicacion,
    cancel: CancellationToken,
) -> Result<(), String> {
    servir_archivos(listener, index, cancel, None).await
}
pub async fn servir_archivos(
    listener: TcpListener,
    index: Publicacion,
    cancel: CancellationToken,
    mods: Option<PathBuf>,
) -> Result<(), String> {
    // Las descargas esperan turno en orden de llegada en lugar de cortar la
    // conexión; el índice no ocupa estos turnos y nunca espera detrás de un JAR.
    let descargas = Arc::new(tokio::sync::Semaphore::new(8));
    let conexiones = Arc::new(tokio::sync::Semaphore::new(64));
    loop {
        let (socket, _) = tokio::select! {_=cancel.cancelled()=>return Ok(()), r=listener.accept()=>r.map_err(err)?};
        let Ok(permiso) = conexiones.clone().try_acquire_owned() else {
            continue;
        };
        let (index, mods, c, descargas) = (index.clone(), mods.clone(), cancel.clone(), descargas.clone());
        tokio::spawn(async move {
            let _permiso = permiso;
            tokio::select! { _ = c.cancelled() => {}, _ = atender(socket, index, mods, descargas) => {} }
        });
    }
}

/// Solicitud HTTP mínima: solo se interpretan las cabeceras que usa el servicio.
struct Solicitud {
    linea: String,
    si_no_coincide: Option<String>,
    rango: Option<String>,
    si_rango: Option<String>,
    cerrar: bool,
}
fn leer_solicitud(datos: &[u8]) -> Result<Solicitud, String> {
    let texto = std::str::from_utf8(datos).map_err(err)?;
    let mut lineas = texto.split("\r\n");
    let linea = lineas.next().unwrap_or("").to_owned();
    let mut solicitud = Solicitud {
        cerrar: linea.ends_with(" HTTP/1.0"),
        linea,
        si_no_coincide: None,
        rango: None,
        si_rango: None,
    };
    for cabecera in lineas {
        let Some((nombre, valor)) = cabecera.split_once(':') else { continue };
        let valor = valor.trim().to_owned();
        match nombre.trim().to_ascii_lowercase().as_str() {
            "if-none-match" => solicitud.si_no_coincide = Some(valor),
            "range" => solicitud.rango = Some(valor),
            "if-range" => solicitud.si_rango = Some(valor),
            "connection" => solicitud.cerrar = valor.eq_ignore_ascii_case("close"),
            _ => {}
        }
    }
    Ok(solicitud)
}
/// `bytes=N-` o `bytes=N-M` sobre un archivo de `total` bytes; otro formato se ignora.
fn rango_pedido(valor: &str, total: u64) -> Option<Result<(u64, u64), ()>> {
    let (inicio, fin) = valor.strip_prefix("bytes=")?.split_once('-')?;
    let inicio = inicio.trim().parse::<u64>().ok()?;
    let fin = match fin.trim() {
        "" => total.checked_sub(1)?,
        f => f.parse::<u64>().ok()?.min(total.checked_sub(1)?),
    };
    Some(if inicio <= fin && inicio < total { Ok((inicio, fin)) } else { Err(()) })
}

/// Atiende varias solicitudes por conexión. Solo las respuestas correctas la
/// mantienen abierta; un error o una solicitud inválida la cierran.
async fn atender(
    mut socket: tokio::net::TcpStream,
    index: Publicacion,
    mods: Option<PathBuf>,
    descargas: Arc<tokio::sync::Semaphore>,
) -> Result<(), String> {
    let mut pendiente = Vec::new();
    for _ in 0..100 {
        let fin_cabeceras = loop {
            if let Some(fin) = pendiente.windows(4).position(|w| w == b"\r\n\r\n") {
                break fin + 4;
            }
            let mut buf = [0u8; 1024];
            let leido = tokio::time::timeout(Duration::from_secs(15), socket.read(&mut buf)).await;
            let n = match leido {
                Ok(Ok(n)) if n > 0 => n,
                _ => return Ok(()),
            };
            if pendiente.len() + n > 4096 {
                return Ok(());
            }
            pendiente.extend_from_slice(&buf[..n]);
        };
        let datos: Vec<u8> = pendiente.drain(..fin_cabeceras).collect();
        let Ok(solicitud) = leer_solicitud(&datos) else {
            responder_y_cerrar(&mut socket, "400 Bad Request").await;
            return Ok(());
        };
        let respuesta = tokio::time::timeout(
            Duration::from_secs(120),
            responder(&mut socket, &solicitud, &index, mods.as_deref(), &descargas),
        )
        .await;
        if !matches!(respuesta, Ok(Ok(true))) || solicitud.cerrar {
            return Ok(());
        }
    }
    Ok(())
}
async fn responder_y_cerrar(socket: &mut tokio::net::TcpStream, estado: &str) {
    let cabecera = format!("HTTP/1.1 {estado}\r\nContent-Length: 0\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n");
    let _ = socket.write_all(cabecera.as_bytes()).await;
}
/// Devuelve si la conexión puede seguir abierta.
async fn responder(
    socket: &mut tokio::net::TcpStream,
    solicitud: &Solicitud,
    index: &Publicacion,
    mods: Option<&Path>,
    descargas: &tokio::sync::Semaphore,
) -> Result<bool, String> {
    let linea = solicitud.linea.as_str();
    let es_indice = linea == format!("GET {RUTA} HTTP/1.1") || linea == format!("GET {RUTA} HTTP/1.0");
    let file_hash = linea
        .strip_prefix("GET /icaro/v1/files/")
        .and_then(|s| s.strip_suffix(" HTTP/1.1").or_else(|| s.strip_suffix(" HTTP/1.0")));
    let snapshot = index.read().await.clone();
    let conexion = if solicitud.cerrar { "close" } else { "keep-alive" };
    if es_indice {
        let Some(indice) = snapshot else {
            responder_y_cerrar(socket, "503 Service Unavailable").await;
            return Ok(false);
        };
        // La revisión identifica el contenido: el cliente puede evitar descargarlo otra vez.
        let etiqueta = format!("\"{}\"", indice.revision()?);
        if solicitud.si_no_coincide.as_deref() == Some(etiqueta.as_str()) {
            let cabecera = format!("HTTP/1.1 304 Not Modified\r\nETag: {etiqueta}\r\nCache-Control: no-cache\r\nConnection: {conexion}\r\n\r\n");
            socket.write_all(cabecera.as_bytes()).await.map_err(err)?;
            return Ok(true);
        }
        let cuerpo = serde_json::to_vec(&indice).map_err(err)?;
        let cabecera = format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nETag: {etiqueta}\r\nCache-Control: no-cache\r\nConnection: {conexion}\r\n\r\n", cuerpo.len());
        socket.write_all(cabecera.as_bytes()).await.map_err(err)?;
        socket.write_all(&cuerpo).await.map_err(err)?;
        return Ok(true);
    }
    let (Some(sha), Some(root), Some(snapshot)) = (file_hash, mods, snapshot) else {
        responder_y_cerrar(socket, "404 Not Found").await;
        return Ok(false);
    };
    let sha = sha.to_ascii_lowercase();
    let Some(archivo) = (sha.len() == 128 && sha.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| snapshot.archivos.iter().find(|a| a.sha512.eq_ignore_ascii_case(&sha)))
        .flatten()
        .cloned()
    else {
        responder_y_cerrar(socket, "404 Not Found").await;
        return Ok(false);
    };
    let Ok(_turno) = tokio::time::timeout(Duration::from_secs(60), descargas.acquire()).await else {
        responder_y_cerrar(socket, "503 Service Unavailable").await;
        return Ok(false);
    };
    let ruta = root.join(&archivo.archivo);
    let valido = nombre_valido(&archivo.archivo)
        && regular(root, true).is_ok()
        && regular(&ruta, false).is_ok_and(|m| m.len() == archivo.bytes && m.len() <= MAX_FILE);
    // Con los metadatos ya verificados se envía por bloques sin releerlo entero;
    // si cambiaron, se comprueba el contenido antes de servirlo.
    let (raiz, nombre, esperado) = (root.to_path_buf(), archivo.archivo.clone(), sha.clone());
    let vigente = valido
        && tokio::task::spawn_blocking(move || {
            dedicado::sha_vigente(&raiz, &nombre).as_deref() == Some(esperado.as_str())
                || hash_archivo(&raiz.join(&nombre)).is_ok_and(|h| h == esperado)
        })
        .await
        .unwrap_or(false);
    if !vigente {
        responder_y_cerrar(socket, "410 Gone").await;
        return Ok(false);
    }
    let total = archivo.bytes;
    let etiqueta = format!("\"{sha}\"");
    // If-Range distinto de esta etiqueta: se entrega el archivo completo.
    let rango = solicitud
        .rango
        .as_deref()
        .filter(|_| solicitud.si_rango.as_deref().is_none_or(|v| v == etiqueta))
        .and_then(|r| rango_pedido(r, total));
    let (estado, inicio, fin) = match rango {
        Some(Ok((inicio, fin))) => ("206 Partial Content", inicio, fin),
        Some(Err(())) => {
            let cabecera = format!("HTTP/1.1 416 Range Not Satisfiable\r\nContent-Range: bytes */{total}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
            let _ = socket.write_all(cabecera.as_bytes()).await;
            return Ok(false);
        }
        None => ("200 OK", 0, total.saturating_sub(1)),
    };
    let longitud = if total == 0 { 0 } else { fin - inicio + 1 };
    let rango_cabecera = if estado.starts_with("206") { format!("Content-Range: bytes {inicio}-{fin}/{total}\r\n") } else { String::new() };
    let cabecera = format!("HTTP/1.1 {estado}\r\nContent-Type: application/octet-stream\r\nContent-Length: {longitud}\r\nETag: {etiqueta}\r\nAccept-Ranges: bytes\r\n{rango_cabecera}Cache-Control: no-store\r\nConnection: {conexion}\r\n\r\n");
    let mut origen = tokio::fs::File::open(&ruta).await.map_err(err)?;
    tokio::io::AsyncSeekExt::seek(&mut origen, std::io::SeekFrom::Start(inicio)).await.map_err(err)?;
    socket.write_all(cabecera.as_bytes()).await.map_err(err)?;
    let enviados = tokio::io::copy(&mut tokio::io::AsyncReadExt::take(origen, longitud), socket).await.map_err(err)?;
    Ok(enviados == longitud)
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fake {
        versions: BTreeMap<String, Version>,
        hashes: BTreeMap<String, Version>,
        bytes: BTreeMap<String, Vec<u8>>,
        bad: bool,
    }
    impl Fuente for Fake {
        async fn versiones_por_hash(&self, hashes: &[String]) -> Result<BTreeMap<String, Version>, Error> {
            Ok(self.hashes.iter().filter(|(h, _)| hashes.contains(h)).map(|(h,v)| (h.clone(),v.clone())).collect())
        }
        async fn version(&self, id: &str) -> Result<Version, Error> {
            self.versions.get(id).cloned().ok_or(Error::Datos)
        }
        async fn versiones(&self, project: &str, mc: &str, loader: &str) -> Result<Vec<Version>, Error> {
            Ok(self.versions.values().filter(|v| v.project_id == project && compatible(v,mc,loader)).cloned().collect())
        }
        async fn nombre(&self, id: &str) -> Result<String, Error> {
            Ok(id.into())
        }
        async fn archivo(&self, f: &ModFile) -> Result<Vec<u8>, Error> {
            if self.bad {
                Ok(b"corrupto".to_vec())
            } else {
                self.bytes.get(&f.filename).cloned().ok_or(Error::Datos)
            }
        }
    }
    fn fake() -> Fake {
        let mut f = Fake {
            versions: BTreeMap::new(),
            hashes: BTreeMap::new(),
            bytes: BTreeMap::new(),
            bad: false,
        };
        for (id, filename, body) in [
            ("v1", "example-1.jar", b"primero".as_slice()),
            ("v2", "example-2.jar", b"segundo".as_slice()),
        ] {
            f.versions.insert(
                id.into(),
                Version {
                    id: id.into(),
                    project_id: "example".into(),
                    name: "Example".into(),
                    version_number: id.into(),
                    version_type: "release".into(),
                    game_versions: vec!["1.20.1".into()],
                    loaders: vec!["fabric".into()],
                    files: vec![ModFile {
                        filename: filename.into(),
                        url: format!(
                            "https://cdn.modrinth.com/data/example/versions/{id}/{filename}"
                        ),
                        size: body.len() as u64,
                        primary: true,
                        hashes: crate::modrinth::Hashes { sha512: hash(body) },
                    }],
                    dependencies: vec![],
                    changelog: None,
                    date_published: String::new(),
                    downloads: 0,
                },
            );
            f.bytes.insert(filename.into(), body.to_vec());
        }
        f
    }
    struct Temp(PathBuf);
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn store() -> (Temp, Almacen, Instancia) {
        let mut bytes = [0u8; 16];
        getrandom::fill(&mut bytes).unwrap();
        let temp = Temp(std::env::temp_dir().join(format!("icaro-sync-test-{}", hash(&bytes))));
        let store = Almacen::nuevo(temp.0.join("instances"));
        let i = store
            .crear_con_loader("Amigos", "1.20.1", "fabric", 0)
            .unwrap();
        crate::versiones_instancia::guardar_loader(&store, &i, "0.16.14").unwrap();
        (temp, store, i)
    }
    fn index(f: &Fake, id: &str) -> Indice {
        let v = &f.versions[id];
        Indice {
            esquema: 1,
            minecraft: "1.20.1".into(),
            loader: "fabric".into(),
            loader_version: Some("0.16.14".into()),
            mods: vec![Entrada {
                proyecto: v.project_id.clone(),
                version: v.id.clone(),
                sha512: v.files[0].hashes.sha512.clone(),
            }],
            archivos: vec![],
            avisos: vec![],
        }
    }
    #[tokio::test]
    async fn actualiza_retira_solo_gestionados_y_conserva_archivos_locales() {
        let (_temp, store, i) = store();
        let f = fake();
        super::super::instalar(
            &f,
            &store,
            &i.id,
            vec![f.versions["v1"].clone()],
            &i.minecraft,
            &i.loader,
            |_| {},
        )
        .await
        .unwrap();
        let dir = store.juego(&i.id).unwrap().join("mods");
        fs::write(dir.join("local.jar"), b"personal").unwrap();
        let plan = preparar(&f, &store, &i, index(&f, "v2"), "127.0.0.1:25566".into())
            .await
            .unwrap();
        assert_eq!(plan.cambios.len(), 1);
        assert_eq!(plan.cambios[0].tipo, Tipo::Actualizar);
        assert_eq!(plan.conservados, vec!["local.jar"]);
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {})
            .await
            .unwrap();
        assert_eq!(fs::read(dir.join("example-2.jar")).unwrap(), b"segundo");
        assert!(!dir.join("example-1.jar").exists());
        let mut empty = index(&f, "v2");
        empty.mods.clear();
        let plan = preparar(&f, &store, &i, empty, "127.0.0.1:25566".into())
            .await
            .unwrap();
        assert_eq!(plan.cambios[0].tipo, Tipo::Quitar);
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {})
            .await
            .unwrap();
        assert!(!dir.join("example-2.jar").exists());
        assert_eq!(fs::read(dir.join("local.jar")).unwrap(), b"personal");
    }
    #[tokio::test]
    async fn sincroniza_litemoretica_sin_dependencias_declaradas_desde_cero() {
        for propio in [false, true] {
            let (_temp, store, i) = store();
            let mut f = fake();
            for (project, id, name) in [
                ("OYR5DMCo", "nhi2RkZ9", "litemoretica"),
                ("bEpr0Arc", "litematica1", "litematica"),
                ("GcWjdA9I", "malilib1", "malilib"),
            ] {
                let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
                writer.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default()).unwrap();
                write!(writer, "{{\"id\":\"{name}\",\"version\":\"1\",\"environment\":\"*\"}}").unwrap();
                let bytes = writer.finish().unwrap().into_inner();
                let mut v = f.versions["v1"].clone();
                v.project_id = project.into(); v.id = id.into();
                v.files[0].filename = format!("{name}.jar");
                v.files[0].size = bytes.len() as u64; v.files[0].hashes.sha512 = hash(&bytes);
                if name == "litematica" {
                    v.dependencies = serde_json::from_value(serde_json::json!([
                        {"project_id":"GcWjdA9I","dependency_type":"required"}
                    ])).unwrap();
                }
                f.hashes.insert(hash(&bytes), v.clone());
                f.bytes.insert(v.files[0].filename.clone(), bytes);
                f.versions.insert(id.into(), v);
            }
            let mut indice = index(&f, "nhi2RkZ9");
            let host = store.datos().unwrap().join("host");
            fs::create_dir_all(&host).unwrap();
            if propio {
                let v = &f.versions["nhi2RkZ9"];
                fs::write(host.join("litemoretica.jar"), &f.bytes["litemoretica.jar"]).unwrap();
                indice.esquema = 2; indice.mods.clear();
                indice.archivos.push(ArchivoPropio { id:"icaro-host-litemoretica".into(),
                    nombre:"Litemoretica".into(), version:"1.2.5".into(), archivo:"litemoretica.jar".into(),
                    bytes:v.files[0].size, sha512:v.files[0].hashes.sha512.clone() });
            }
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap().to_string();
            let cancel = CancellationToken::new();
            let servicio = tokio::spawn(servir_archivos(listener, Arc::new(RwLock::new(Some(indice.clone()))), cancel.clone(), Some(host)));
            let dir = store.juego(&i.id).unwrap().join("mods");
            assert!(!dir.exists());
            let plan = preparar(&f, &store, &i, indice.clone(), addr.clone()).await.unwrap();
            for project in ["bEpr0Arc", "GcWjdA9I"] {
                assert!(plan.cambios.iter().any(|c| c.proyecto == project && c.tipo == Tipo::Agregar));
            }
            aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_,_| {}).await.unwrap();
            for name in ["litemoretica.jar", "litematica.jar", "malilib.jar"] {
                assert!(dir.join(name).is_file());
            }
            let plan = preparar(&f, &store, &i, indice, addr).await.unwrap();
            assert!(plan.cambios.is_empty());
            cancel.cancel(); servicio.await.unwrap().unwrap();
        }
        // No extender la excepción a otros proyectos o versiones desconocidas.
        let mut v = fake().versions["v1"].clone();
        assert!(dependencias_cliente(&v).is_empty());
        v.project_id = "OYR5DMCo".into();
        assert!(dependencias_cliente(&v).is_empty());
    }

    #[tokio::test]
    #[ignore = "Descarga Litemoretica y dependencias reales en una instancia temporal; no inicia Minecraft"]
    async fn litemoretica_real_sincroniza_dependencias_cliente() {
        let (_temp, store, _) = store();
        let i = store.crear_con_loader("Cliente real", "1.21.1", "fabric", 0).unwrap();
        crate::versiones_instancia::guardar_loader(&store, &i, "0.19.5").unwrap();
        let source = Catalog::new().unwrap();
        let v = source.version("nhi2RkZ9").await.unwrap();
        assert_eq!(v.project_id, "OYR5DMCo");
        let f = v.files.iter().find(|f| f.primary).unwrap();
        let indice = Indice { esquema:1, minecraft:i.minecraft.clone(), loader:i.loader.clone(),
            loader_version:Some("0.19.5".into()), mods:vec![Entrada {
                proyecto:v.project_id.clone(), version:v.id.clone(), sha512:f.hashes.sha512.clone()
            }], archivos:vec![], avisos:vec![] };
        let plan = preparar(&source, &store, &i, indice.clone(), "127.0.0.1:25566".into()).await.unwrap();
        assert_eq!(plan.cambios.len(), 3);
        aplicar(&source, &store, &i, &plan, CancellationToken::new(), |_,_| {}).await.unwrap();
        let installed = inventario(&store, &i.id).unwrap();
        for project in ["OYR5DMCo", "bEpr0Arc", "GcWjdA9I"] {
            let m = installed.iter().find(|m| m.project_id == project).unwrap();
            let path = store.juego(&i.id).unwrap().join("mods").join(&m.archivo);
            assert_eq!(hash(&fs::read(&path).unwrap()), m.sha512);
            if project == "GcWjdA9I" {
                let mut jar = zip::ZipArchive::new(fs::File::open(path).unwrap()).unwrap();
                assert!(jar.by_name("fi/dy/masa/malilib/event/InitializationHandler.class").is_ok());
            }
        }
        assert!(preparar(&source, &store, &i, indice, "127.0.0.1:25566".into()).await.unwrap().cambios.is_empty());
    }

    #[tokio::test]
    async fn indice_antiguo_recupera_bibliotecas_desde_cero_sin_cambiar_pins_del_anfitrion() {
        fn jar(id: &str, entorno: &str) -> Vec<u8> {
            let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
            writer.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default()).unwrap();
            write!(writer, "{{\"id\":\"{id}\",\"version\":\"1\",\"environment\":\"{entorno}\"}}").unwrap();
            writer.finish().unwrap().into_inner()
        }
        let (_temp, store, i) = store();
        let mut f = fake();
        let body = jar("raiz", "*");
        let mut root = f.versions["v1"].clone();
        root.id = "raiz1".into(); root.project_id = "raiz".into();
        root.files[0].filename = "raiz.jar".into();
        root.files[0].hashes.sha512 = hash(&body); root.files[0].size = body.len() as u64;
        root.dependencies = serde_json::from_value(serde_json::json!([
            {"project_id":"biblioteca","dependency_type":"required"},
            {"version_id":"otra1","dependency_type":"required"},
            {"project_id":"example","version_id":"v2","dependency_type":"required"}
        ])).unwrap();
        f.hashes.insert(hash(&body), root);
        for project in ["biblioteca", "otra"] {
            let bytes = jar(project, "*");
            let mut v = f.versions["v1"].clone();
            v.id = format!("{project}1"); v.project_id = project.into();
            v.files[0].filename = format!("{project}.jar");
            v.files[0].size = bytes.len() as u64; v.files[0].hashes.sha512 = hash(&bytes);
            f.bytes.insert(v.files[0].filename.clone(), bytes);
            f.versions.insert(v.id.clone(), v);
        }
        let host = store.datos().unwrap().join("host-mods");
        fs::create_dir_all(&host).unwrap(); fs::write(host.join("raiz.jar"), &body).unwrap();
        let mut indice = index(&f, "v1"); indice.esquema = 2;
        indice.archivos.push(ArchivoPropio { id:"icaro-host-raiz".into(), nombre:"Raíz".into(), version:"1".into(), archivo:"raiz.jar".into(), bytes:body.len() as u64, sha512:hash(&body) });
        let revision = indice.revision().unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let cancel = CancellationToken::new();
        let servicio = tokio::spawn(servir_archivos(listener, Arc::new(RwLock::new(Some(indice.clone()))), cancel.clone(), Some(host)));
        let dir = store.juego(&i.id).unwrap().join("mods");
        assert!(!dir.exists());
        let plan = preparar(&f, &store, &i, indice.clone(), addr.to_string()).await.unwrap();
        assert_eq!(plan.revision_origen(), revision);
        assert_eq!(plan.indice.mods.len(), 3);
        assert!(plan.indice.mods.iter().any(|m| m.proyecto == "example" && m.version == "v1"));
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_,_| {}).await.unwrap();
        for name in ["raiz.jar", "biblioteca.jar", "otra.jar", "example-1.jar"] { assert!(dir.join(name).is_file()); }
        assert!(!dir.join("example-2.jar").exists());
        let plan = preparar(&f, &store, &i, indice.clone(), addr.to_string()).await.unwrap();
        assert!(plan.cambios.is_empty());
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_,_| panic!("No copiar un conjunto ya completo")).await.unwrap();
        let anterior = huella(&store, &i.id).unwrap();
        // El registro verificado caducó: la revisión vuelve a consultar Modrinth.
        fs::remove_file(store.juego(&i.id).unwrap().join(VERIFICADO)).unwrap();
        let server = jar("biblioteca", "server");
        let v = f.versions.get_mut("biblioteca1").unwrap();
        v.files[0].hashes.sha512 = hash(&server); v.files[0].size = server.len() as u64;
        f.bytes.insert("biblioteca.jar".into(), server);
        let plan = preparar(&f, &store, &i, indice, addr.to_string()).await.unwrap();
        assert!(aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_,_| {}).await.unwrap_err().contains("solo admite servidor"));
        assert_eq!(anterior, huella(&store, &i.id).unwrap());
        cancel.cancel(); servicio.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn sincronizar_conserva_mods_personales_de_catalogo_manuales_y_desactivados() {
        let (_temp, store, i) = store();
        let mut f = fake();
        let mut cliente = f.versions["v1"].clone();
        cliente.id = "cliente1".into();
        cliente.project_id = "cliente".into();
        cliente.files[0].filename = "cliente.jar".into();
        cliente.files[0].size = 7;
        cliente.files[0].hashes.sha512 = hash(b"cliente");
        f.bytes.insert("cliente.jar".into(), b"cliente".to_vec());
        f.versions.insert(cliente.id.clone(), cliente.clone());
        super::super::instalar(
            &f, &store, &i.id, vec![f.versions["v1"].clone(), cliente],
            &i.minecraft, &i.loader, |_| {},
        ).await.unwrap();
        let dir = store.juego(&i.id).unwrap().join("mods");
        fs::write(dir.join("manual.jar"), b"manual").unwrap();
        fs::write(dir.join("desactivado.jar.disabled"), b"desactivado").unwrap();
        // Primero registra el conjunto, después lo actualiza y finalmente retira
        // todos los mods del anfitrión. Los personales no pasan a ser gestionados.
        for version in [Some("v1"), Some("v2"), None] {
            let mut indice = index(&f, version.unwrap_or("v2"));
            if version.is_none() { indice.mods.clear(); }
            let plan = preparar(&f, &store, &i, indice, "127.0.0.1:25566".into()).await.unwrap();
            assert!(!plan.cambios.iter().any(|c| c.proyecto == "cliente"));
            aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {}).await.unwrap();
            assert_eq!(fs::read(dir.join("cliente.jar")).unwrap(), b"cliente");
            assert_eq!(fs::read(dir.join("manual.jar")).unwrap(), b"manual");
            assert_eq!(fs::read(dir.join("desactivado.jar.disabled")).unwrap(), b"desactivado");
            assert!(!dir.join("desactivado.jar").exists());
            assert!(inventario(&store, &i.id).unwrap().iter().any(|m| m.project_id == "cliente"));
            let gestionados: Gestionados = serde_json::from_slice(&fs::read(dir.join(SYNC)).unwrap()).unwrap();
            assert!(!gestionados.proyectos.contains("cliente"));
        }
        assert!(!dir.join("example-1.jar").exists());
        assert!(!dir.join("example-2.jar").exists());
    }

    #[tokio::test]
    async fn conjunto_al_dia_registra_el_origen_una_vez_y_no_vuelve_a_publicar() {
        let (_temp, store, i) = store();
        let f = fake();
        super::super::instalar(
            &f, &store, &i.id, vec![f.versions["v1"].clone()],
            &i.minecraft, &i.loader, |_| {},
        ).await.unwrap();
        let dir = store.juego(&i.id).unwrap().join("mods");
        fs::write(dir.join("personal.jar"), b"personal").unwrap();
        let mut indice = index(&f, "v1");
        indice.esquema = 2;
        indice.archivos.push(ArchivoPropio {
            id: "icaro-host-personal".into(), nombre: "Personal".into(), version: "1".into(),
            archivo: "personal.jar".into(), bytes: 8, sha512: hash(b"personal"),
        });
        let origen = "127.0.0.1:25566";
        let plan = preparar(&f, &store, &i, indice.clone(), origen.into()).await.unwrap();
        assert!(plan.cambios.is_empty());
        assert!(!dir.join(SYNC).exists());
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {}).await.unwrap();
        assert!(dir.join(SYNC).exists());
        let plan = preparar(&f, &store, &i, indice, origen.into()).await.unwrap();
        assert!(plan.cambios.is_empty());
        let registro = fs::metadata(dir.join(SYNC)).unwrap().modified().unwrap();
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {
            panic!("Un conjunto al día no debe volver a preparar archivos");
        }).await.unwrap();
        assert_eq!(registro, fs::metadata(dir.join(SYNC)).unwrap().modified().unwrap());
        fs::write(dir.join("personal.jar"), b"modificado").unwrap();
        assert!(aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {}).await.is_err());
    }

    #[tokio::test]
    async fn fallo_cancelacion_y_plan_obsoleto_no_sustituyen_el_conjunto() {
        let (_temp, store, i) = store();
        let mut f = fake();
        super::super::instalar(
            &f,
            &store,
            &i.id,
            vec![f.versions["v1"].clone()],
            &i.minecraft,
            &i.loader,
            |_| {},
        )
        .await
        .unwrap();
        let plan = preparar(&f, &store, &i, index(&f, "v2"), "host".into())
            .await
            .unwrap();
        let before = huella(&store, &i.id).unwrap();
        f.bad = true;
        assert!(
            aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {})
                .await
                .is_err()
        );
        assert_eq!(huella(&store, &i.id).unwrap(), before);
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert!(aplicar(&f, &store, &i, &plan, cancel, |_, _| {})
            .await
            .is_err());
        assert_eq!(huella(&store, &i.id).unwrap(), before);
        let dir = store.juego(&i.id).unwrap().join("mods");
        fs::write(dir.join("nuevo-local.jar"), b"nuevo").unwrap();
        assert!(
            aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {})
                .await
                .is_err()
        );
        assert_eq!(fs::read(dir.join("example-1.jar")).unwrap(), b"primero");
    }
    #[tokio::test]
    async fn modificar_un_mod_despues_de_revisar_invalida_el_plan_sin_tocarlo() {
        let (_temp, store, i) = store();
        let f = fake();
        super::super::instalar(&f, &store, &i.id, vec![f.versions["v1"].clone()], &i.minecraft, &i.loader, |_| {})
            .await
            .unwrap();
        let plan = preparar(&f, &store, &i, index(&f, "v2"), "host".into()).await.unwrap();
        let dir = store.juego(&i.id).unwrap().join("mods");
        fs::write(dir.join("example-1.jar"), b"editado por otra herramienta").unwrap();
        let error = aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {})
            .await
            .unwrap_err();
        assert!(error.contains("cambiaron"), "{error}");
        assert_eq!(fs::read(dir.join("example-1.jar")).unwrap(), b"editado por otra herramienta");
        assert!(!dir.join("example-2.jar").exists());
    }
    #[tokio::test]
    async fn analisis_local_previo_detecta_cambios_durante_la_consulta() {
        let (_temp, store, i) = store();
        let f = fake();
        super::super::instalar(&f, &store, &i.id, vec![f.versions["v1"].clone()], &i.minecraft, &i.loader, |_| {})
            .await
            .unwrap();
        let local = analizar(&store, &i).await.unwrap();
        let dir = store.juego(&i.id).unwrap().join("mods");
        fs::write(dir.join("nuevo-local.jar"), b"nuevo").unwrap();
        let error = preparar_desde(&f, &store, &i, index(&f, "v2"), "host".into(), local.clone())
            .await
            .unwrap_err();
        assert!(error.contains("cambiaron"), "{error}");
        let otra = store.crear_con_loader("Otra", "1.20.1", "fabric", 0).unwrap();
        assert!(preparar_desde(&f, &store, &otra, index(&f, "v2"), "host".into(), local).await.is_err());
    }
    #[tokio::test]
    async fn actualizar_no_copia_ni_recrea_los_archivos_no_afectados() {
        let (_temp, store, i) = store();
        let f = fake();
        let dir = store.juego(&i.id).unwrap().join("mods");
        let plan = preparar(&f, &store, &i, index(&f, "v1"), "host".into()).await.unwrap();
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {}).await.unwrap();
        fs::write(dir.join("personal.jar"), vec![7u8; 256 * 1024]).unwrap();
        let identidad = |nombre: &str| {
            let meta = fs::metadata(dir.join(nombre)).unwrap();
            estado_archivo(nombre.into(), &meta)
        };
        let personal = identidad("personal.jar");
        let plan = preparar(&f, &store, &i, index(&f, "v2"), "host".into()).await.unwrap();
        let avisos = std::sync::Mutex::new(vec![]);
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |id, hecho| avisos.lock().unwrap().push((id.to_owned(), hecho)))
            .await
            .unwrap();
        // El mismo archivo, no una copia: conserva creación, fecha e identidad.
        assert_eq!(identidad("personal.jar"), personal);
        assert!(dir.join("example-2.jar").exists() && !dir.join("example-1.jar").exists());
        assert_eq!(*avisos.lock().unwrap(), vec![("example".to_owned(), false), ("example".to_owned(), true)]);
        let juego = store.juego(&i.id).unwrap();
        assert!(!fs::read_dir(&juego).unwrap().any(|e| e.unwrap().file_name().to_string_lossy().starts_with(".icaro-mods-tx-")));
    }
    #[tokio::test]
    #[ignore = "Mide revisión y aplicación con 500 MiB de mods personales; escribe archivos temporales grandes"]
    async fn medir_actualizacion_con_carpeta_grande() {
        let (_temp, store, i) = store();
        let f = fake();
        let plan = preparar(&f, &store, &i, index(&f, "v1"), "host".into()).await.unwrap();
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {}).await.unwrap();
        let dir = store.juego(&i.id).unwrap().join("mods");
        let bloque = (0..2 * 1024 * 1024u32).map(|n| (n % 251) as u8).collect::<Vec<_>>();
        for n in 0..250 {
            let mut contenido = bloque.clone();
            contenido[..4].copy_from_slice(&(n as u32).to_le_bytes());
            fs::write(dir.join(format!("personal-{n:03}.jar")), contenido).unwrap();
            // Una instancia real tiene mods antiguos; los recién escritos no entran en el inventario.
            let hace = std::time::SystemTime::now() - Duration::from_secs(3600);
            fs::File::options().write(true).open(dir.join(format!("personal-{n:03}.jar"))).unwrap().set_modified(hace).unwrap();
        }
        let inicio = std::time::Instant::now();
        let plan = preparar(&f, &store, &i, index(&f, "v2"), "host".into()).await.unwrap();
        let revision = inicio.elapsed();
        let inicio = std::time::Instant::now();
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {}).await.unwrap();
        let aplicacion = inicio.elapsed();
        let inicio = std::time::Instant::now();
        let plan = preparar(&f, &store, &i, index(&f, "v2"), "host".into()).await.unwrap();
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {}).await.unwrap();
        let sin_cambios = inicio.elapsed();
        let inicio = std::time::Instant::now();
        let plan = preparar(&f, &store, &i, index(&f, "v2"), "host".into()).await.unwrap();
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {}).await.unwrap();
        let sin_cambios_2 = inicio.elapsed();
        println!("MEDICION revision={revision:?} aplicacion={aplicacion:?} jugar_sin_cambios={sin_cambios:?} jugar_sin_cambios_2={sin_cambios_2:?}");
    }
    fn transaccion(juego: &Path, confirmada: bool, movimientos: &[(&str, &str)]) -> PathBuf {
        let tx = juego.join(format!("{PREFIJO_TX}prueba"));
        fs::create_dir_all(tx.join("nuevos")).unwrap();
        fs::create_dir_all(tx.join("retirados")).unwrap();
        let movimientos = movimientos.iter().map(|(o, d)| Movimiento { origen: (*o).into(), destino: (*d).into() }).collect();
        escribir_diario(&tx, &Diario { esquema: 1, confirmada, movimientos }).unwrap();
        tx
    }
    #[test]
    fn cierre_abrupto_deshace_el_commit_parcial_al_abrir_la_instancia() {
        let (_temp, store, i) = store();
        let juego = store.juego(&i.id).unwrap();
        let mods = juego.join("mods");
        fs::create_dir_all(&mods).unwrap();
        fs::write(mods.join("personal.jar"), b"personal").unwrap();
        // Se retiró a.jar y se colocó b.jar; faltaba colocar c.jar.
        let tx = transaccion(&juego, false, &[
            ("mods/a.jar", ".icaro-mods-tx-prueba/retirados/0"),
            (".icaro-mods-tx-prueba/nuevos/1", "mods/b.jar"),
            (".icaro-mods-tx-prueba/nuevos/2", "mods/c.jar"),
        ]);
        fs::write(tx.join("retirados/0"), b"anterior").unwrap();
        fs::write(mods.join("b.jar"), b"nuevo").unwrap();
        fs::write(tx.join("nuevos/2"), b"pendiente").unwrap();
        let inventario = inventario(&store, &i.id).unwrap();
        assert_eq!(fs::read(mods.join("a.jar")).unwrap(), b"anterior");
        assert!(!mods.join("b.jar").exists() && !mods.join("c.jar").exists());
        assert_eq!(fs::read(mods.join("personal.jar")).unwrap(), b"personal");
        assert!(!tx.exists());
        assert_eq!(inventario.len(), 2);
    }
    #[test]
    fn commit_confirmado_no_se_deshace_y_un_archivo_ajeno_no_se_sobrescribe() {
        let (_temp, store, i) = store();
        let juego = store.juego(&i.id).unwrap();
        let mods = juego.join("mods");
        fs::create_dir_all(&mods).unwrap();
        let tx = transaccion(&juego, true, &[(".icaro-mods-tx-prueba/nuevos/0", "mods/b.jar")]);
        fs::write(mods.join("b.jar"), b"nuevo").unwrap();
        inventario(&store, &i.id).unwrap();
        assert_eq!(fs::read(mods.join("b.jar")).unwrap(), b"nuevo");
        assert!(!tx.exists());
        // Si alguien volvió a crear el original, la recuperación se detiene sin pisarlo.
        let tx = transaccion(&juego, false, &[("mods/a.jar", ".icaro-mods-tx-prueba/retirados/0")]);
        fs::write(tx.join("retirados/0"), b"anterior").unwrap();
        fs::write(mods.join("a.jar"), b"del jugador").unwrap();
        assert!(inventario(&store, &i.id).is_err());
        assert_eq!(fs::read(mods.join("a.jar")).unwrap(), b"del jugador");
        assert_eq!(fs::read(tx.join("retirados/0")).unwrap(), b"anterior");
    }
    #[test]
    fn fallo_durante_el_commit_restaura_lo_movido() {
        let (_temp, _store, _i) = store();
        let juego = _store.juego(&_i.id).unwrap();
        let mods = juego.join("mods");
        fs::create_dir_all(&mods).unwrap();
        let tx = transaccion(&juego, false, &[]);
        fs::write(mods.join("a.jar"), b"anterior").unwrap();
        fs::write(mods.join("ocupado.jar"), b"ajeno").unwrap();
        fs::write(tx.join("nuevos/1"), b"nuevo").unwrap();
        let movimientos = [
            Movimiento { origen: "mods/a.jar".into(), destino: ".icaro-mods-tx-prueba/retirados/0".into() },
            Movimiento { origen: ".icaro-mods-tx-prueba/nuevos/1".into(), destino: "mods/ocupado.jar".into() },
        ];
        let error = ejecutar(&juego, ".icaro-mods-tx-prueba", &movimientos).unwrap_err();
        assert!(error.contains("conserva"), "{error}");
        assert_eq!(fs::read(mods.join("a.jar")).unwrap(), b"anterior");
        assert_eq!(fs::read(mods.join("ocupado.jar")).unwrap(), b"ajeno");
        assert!(!tx.join(DIARIO).exists());
    }
    #[tokio::test]
    async fn el_registro_describe_plan_descargas_commit_errores_y_recuperacion() {
        // El registro es global: las demás pruebas pueden escribir aquí, por eso
        // se filtran las líneas por la instancia de esta prueba.
        let raiz = std::env::temp_dir().join(format!("icaro-sync-registro-{}", std::process::id()));
        activar_registro(&raiz);
        let (_temp, store, i) = store();
        let mut f = fake();
        let plan = preparar(&f, &store, &i, index(&f, "v1"), "host".into()).await.unwrap();
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {}).await.unwrap();
        let plan = preparar(&f, &store, &i, index(&f, "v2"), "host".into()).await.unwrap();
        f.bad = true;
        assert!(aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {}).await.is_err());
        let juego = store.juego(&i.id).unwrap();
        let tx = transaccion(&juego, false, &[("mods/perdido.jar", ".icaro-mods-tx-prueba/retirados/0")]);
        fs::write(tx.join("retirados/0"), b"anterior").unwrap();
        inventario(&store, &i.id).unwrap();
        let carpeta = juego.parent().unwrap().file_name().unwrap().to_string_lossy().into_owned();
        let registro = fs::read_to_string(raiz.join("logs/sincronizacion.log")).unwrap();
        let propias: Vec<_> = registro
            .lines()
            .filter(|l| l.contains(&format!("instancia={} ", i.id)) || l.contains(&format!("carpeta={carpeta} ")))
            .collect();
        for esperado in [
            "revision-local instancia=",
            "plan instancia=",
            "agregar=1",
            "aplicar-inicio instancia=",
            "descarga instancia=",
            "fuente=modrinth",
            "commit instancia=",
            "aplicar-fin instancia=",
            "error fase=aplicar instancia=",
            "accion=deshecha movimientos=1",
        ] {
            assert!(propias.iter().any(|l| l.contains(esperado)), "falta {esperado}:\n{}", propias.join("\n"));
        }
        assert!(propias.iter().all(|l| !l.contains(&*raiz.to_string_lossy())), "el registro no debe incluir rutas absolutas");
    }
    #[test]
    fn el_diario_solo_acepta_rutas_de_la_transaccion() {
        let juego = Path::new("juego");
        for valida in ["mods/a.jar", "mods/a.jar.disabled", "mods/.icaro-mods.json", ".icaro-mods-tx-x/nuevos/3", ".icaro-mods-conflictos-x/a.jar"] {
            assert!(ruta_diario(juego, valida).is_ok(), "{valida}");
        }
        for invalida in ["../a.jar", "mods/../a.jar", "mods/sub/a.jar", "C:/a.jar", "mods/a\\b.jar", "options.txt", "saves/mundo/level.dat", ".icaro-mods-tx-x/nuevos/a", ".icaro-mods-tx-x/otros/1", "/mods/a.jar", ""] {
            assert!(ruta_diario(juego, invalida).is_err(), "{invalida}");
        }
    }
    #[tokio::test]
    async fn la_segunda_revision_reutiliza_el_inventario_y_detecta_mods_modificados() {
        let (_temp, store, i) = store();
        let f = fake();
        let plan = preparar(&f, &store, &i, index(&f, "v1"), "host".into()).await.unwrap();
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {}).await.unwrap();
        let dir = store.juego(&i.id).unwrap().join("mods");
        fs::write(dir.join("personal.jar"), b"mod personal").unwrap();
        let envejecer = |nombre: &std::ffi::OsStr| {
            let hace = std::time::SystemTime::now() - Duration::from_secs(3600);
            fs::File::options().write(true).open(dir.join(nombre)).unwrap().set_modified(hace).unwrap();
        };
        for entrada in fs::read_dir(&dir).unwrap() {
            envejecer(&entrada.unwrap().file_name());
        }
        let primera = analizar(&store, &i).await.unwrap();
        assert_eq!(primera.reutilizados, 0);
        let segunda = analizar(&store, &i).await.unwrap();
        assert_eq!((segunda.releidos, segunda.reutilizados), (0, primera.releidos));
        assert_eq!(segunda.huella, primera.huella);
        assert_eq!(segunda.identidades, primera.identidades);
        // Un mod gestionado editado se relee y la revisión lo rechaza como antes.
        fs::write(dir.join("example-1.jar"), b"editado").unwrap();
        envejecer(std::ffi::OsStr::new("example-1.jar"));
        let error = analizar(&store, &i).await.unwrap_err();
        assert!(error.contains("fue modificado"), "{error}");
    }
    #[tokio::test]
    async fn las_descargas_respetan_el_limite_y_se_detienen_con_el_primer_error() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let (activas, maximo) = (AtomicUsize::new(0), AtomicUsize::new(0));
        let trabajo = |n: usize, falla: bool| -> (String, Trabajo<'_>) {
            let (activas, maximo) = (&activas, &maximo);
            (n.to_string(), Box::pin(async move {
                let ahora = activas.fetch_add(1, Ordering::SeqCst) + 1;
                maximo.fetch_max(ahora, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(20)).await;
                activas.fetch_sub(1, Ordering::SeqCst);
                if falla { Err(format!("falló {n}")) } else { Ok(n.to_string()) }
            }))
        };
        let avisos = std::sync::Mutex::new(vec![]);
        let aviso = |id: &str, hecho: bool| avisos.lock().unwrap().push((id.to_owned(), hecho));
        en_paralelo((0..10).map(|n| trabajo(n, false)).collect(), 4, &aviso).await.unwrap();
        assert_eq!(maximo.load(Ordering::SeqCst), 4);
        let terminados = avisos.lock().unwrap().iter().filter(|(_, hecho)| *hecho).count();
        assert_eq!(terminados, 10);

        // Con un error, el resto no llega a empezar.
        avisos.lock().unwrap().clear();
        let trabajos = (0..10).map(|n| trabajo(n, n == 1)).collect();
        assert_eq!(en_paralelo(trabajos, 2, &aviso).await.unwrap_err(), "falló 1");
        assert!(avisos.lock().unwrap().iter().all(|(id, _)| id.parse::<usize>().unwrap() < 3));
    }
    /// Envía una solicitud por una conexión ya abierta y lee cabeceras y cuerpo
    /// según Content-Length, sin esperar a que el servicio cierre la conexión.
    async fn pedir(conexion: &mut tokio::net::TcpStream, solicitud: &str) -> (String, Vec<u8>) {
        conexion.write_all(solicitud.as_bytes()).await.unwrap();
        let mut datos = vec![];
        let fin = loop {
            if let Some(fin) = datos.windows(4).position(|w| w == b"\r\n\r\n") { break fin + 4; }
            let mut buf = [0u8; 4096];
            let n = conexion.read(&mut buf).await.unwrap();
            assert!(n > 0, "conexión cerrada antes de las cabeceras");
            datos.extend_from_slice(&buf[..n]);
        };
        let cabeceras = String::from_utf8_lossy(&datos[..fin]).into_owned();
        let largo = cabeceras.lines().find_map(|l| l.strip_prefix("Content-Length: ")).map_or(0, |v| v.trim().parse::<usize>().unwrap());
        let mut cuerpo = datos[fin..].to_vec();
        while cuerpo.len() < largo {
            let mut buf = [0u8; 4096];
            let n = conexion.read(&mut buf).await.unwrap();
            assert!(n > 0);
            cuerpo.extend_from_slice(&buf[..n]);
        }
        (cabeceras, cuerpo)
    }
    #[tokio::test]
    async fn el_indice_responde_304_y_varias_solicitudes_comparten_conexion() {
        let f = fake();
        let indice = index(&f, "v2");
        let etiqueta = format!("\"{}\"", indice.revision().unwrap());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let cancel = CancellationToken::new();
        let servicio = tokio::spawn(servir(listener, Arc::new(RwLock::new(Some(indice.clone()))), cancel.clone()));
        let mut conexion = tokio::net::TcpStream::connect(addr).await.unwrap();
        let (cabeceras, cuerpo) = pedir(&mut conexion, &format!("GET {RUTA} HTTP/1.1\r\nHost: x\r\n\r\n")).await;
        assert!(cabeceras.starts_with("HTTP/1.1 200") && cabeceras.contains(&format!("ETag: {etiqueta}")), "{cabeceras}");
        assert_eq!(serde_json::from_slice::<Indice>(&cuerpo).unwrap().revision().unwrap(), indice.revision().unwrap());
        // Misma conexión, índice sin cambios: 304 sin cuerpo.
        let (cabeceras, cuerpo) = pedir(&mut conexion, &format!("GET {RUTA} HTTP/1.1\r\nHost: x\r\nIf-None-Match: {etiqueta}\r\n\r\n")).await;
        assert!(cabeceras.starts_with("HTTP/1.1 304") && cuerpo.is_empty(), "{cabeceras}");
        // El cliente guarda la etiqueta y reutiliza su copia en la siguiente consulta.
        assert_eq!(consultar(addr, CancellationToken::new()).await.unwrap().revision().unwrap(), indice.revision().unwrap());
        assert_eq!(consultar(addr, CancellationToken::new()).await.unwrap().revision().unwrap(), indice.revision().unwrap());
        cancel.cancel();
        servicio.await.unwrap().unwrap();
    }
    #[tokio::test]
    async fn los_archivos_propios_admiten_rangos_validados_y_rechazan_rangos_imposibles() {
        let (_temp, store, _i) = store();
        let mods = store.datos().unwrap().join("servidor-mods");
        fs::create_dir_all(&mods).unwrap();
        let contenido = b"0123456789abcdef".to_vec();
        fs::write(mods.join("propio.jar"), &contenido).unwrap();
        let sha = hash(&contenido);
        let mut indice = index(&fake(), "v1");
        indice.esquema = 2;
        indice.archivos.push(ArchivoPropio { id: "icaro-host-propio".into(), nombre: "Propio".into(), version: "1".into(), archivo: "propio.jar".into(), bytes: contenido.len() as u64, sha512: sha.clone() });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let cancel = CancellationToken::new();
        let servicio = tokio::spawn(servir_archivos(listener, Arc::new(RwLock::new(Some(indice))), cancel.clone(), Some(mods.clone())));
        let ruta = format!("/icaro/v1/files/{sha}");
        let mut conexion = tokio::net::TcpStream::connect(addr).await.unwrap();
        let (cabeceras, cuerpo) = pedir(&mut conexion, &format!("GET {ruta} HTTP/1.1\r\nHost: x\r\nRange: bytes=10-\r\nIf-Range: \"{sha}\"\r\n\r\n")).await;
        assert!(cabeceras.starts_with("HTTP/1.1 206") && cabeceras.contains("Content-Range: bytes 10-15/16"), "{cabeceras}");
        assert_eq!(cuerpo, b"abcdef");
        // Un validador distinto entrega el archivo completo, nunca un tramo de otro contenido.
        let (cabeceras, cuerpo) = pedir(&mut conexion, &format!("GET {ruta} HTTP/1.1\r\nHost: x\r\nRange: bytes=10-\r\nIf-Range: \"otro\"\r\n\r\n")).await;
        assert!(cabeceras.starts_with("HTTP/1.1 200"), "{cabeceras}");
        assert_eq!(cuerpo, contenido);
        let (cabeceras, _) = pedir(&mut conexion, &format!("GET {ruta} HTTP/1.1\r\nHost: x\r\nRange: bytes=99-\r\n\r\n")).await;
        assert!(cabeceras.starts_with("HTTP/1.1 416") && cabeceras.contains("Content-Range: bytes */16"), "{cabeceras}");
        // Un archivo cambiado tras publicar no se entrega con el hash anterior.
        fs::write(mods.join("propio.jar"), b"0123456789ABCDEF").unwrap();
        let mut conexion = tokio::net::TcpStream::connect(addr).await.unwrap();
        let (cabeceras, _) = pedir(&mut conexion, &format!("GET {ruta} HTTP/1.1\r\nHost: x\r\n\r\n")).await;
        assert!(cabeceras.starts_with("HTTP/1.1 410"), "{cabeceras}");
        cancel.cancel();
        servicio.await.unwrap().unwrap();
    }
    /// Anfitrión de prueba que corta la primera descarga a la mitad. Con `rangos`
    /// atiende la reanudación con 206; sin ellos responde 200, como un servicio antiguo.
    async fn anfitrion_que_corta(contenido: Vec<u8>, rangos: bool) -> (SocketAddr, Arc<std::sync::Mutex<Vec<String>>>, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let solicitudes = Arc::new(std::sync::Mutex::new(vec![]));
        let registro = solicitudes.clone();
        let tarea = tokio::spawn(async move {
            for intento in 0.. {
                let (mut s, _) = listener.accept().await.unwrap();
                let mut datos = vec![];
                while !datos.windows(4).any(|w| w == b"\r\n\r\n") {
                    let mut buf = [0u8; 1024];
                    let n = s.read(&mut buf).await.unwrap();
                    if n == 0 { break; }
                    datos.extend_from_slice(&buf[..n]);
                }
                let texto = String::from_utf8_lossy(&datos).into_owned();
                registro.lock().unwrap().push(texto.clone());
                let total = contenido.len();
                let desde = texto.lines().find_map(|l| l.strip_prefix("range: bytes=").or_else(|| l.strip_prefix("Range: bytes=")))
                    .and_then(|r| r.trim_end_matches('-').parse::<usize>().ok());
                let (cabecera, cuerpo) = match desde.filter(|_| rangos) {
                    Some(d) => (format!("HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {d}-{}/{total}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", total - 1, total - d), contenido[d..].to_vec()),
                    None => (format!("HTTP/1.1 200 OK\r\nContent-Length: {total}\r\nConnection: close\r\n\r\n"), contenido.clone()),
                };
                s.write_all(cabecera.as_bytes()).await.unwrap();
                // La primera respuesta se corta tras la mitad del cuerpo.
                let enviar = if intento == 0 { &cuerpo[..cuerpo.len() / 2] } else { &cuerpo[..] };
                s.write_all(enviar).await.unwrap();
            }
        });
        (addr, solicitudes, tarea)
    }
    #[tokio::test]
    async fn una_descarga_cortada_del_anfitrion_se_reanuda_o_reinicia_sin_mezclar() {
        let contenido: Vec<u8> = (0..200_000u32).map(|n| (n % 251) as u8).collect();
        let archivo = ArchivoPropio { id: "icaro-host-grande".into(), nombre: "Grande".into(), version: "1".into(), archivo: "grande.jar".into(), bytes: contenido.len() as u64, sha512: hash(&contenido) };
        let carpeta = std::env::temp_dir().join(format!("icaro-reanudar-{}", hash(&contenido)[..16].to_owned()));
        fs::create_dir_all(&carpeta).unwrap();
        for rangos in [true, false] {
            let (addr, solicitudes, tarea) = anfitrion_que_corta(contenido.clone(), rangos).await;
            // El origen es el puerto de Minecraft + 1: el del servicio de prueba.
            let origen = addr.to_string();
            let destino = carpeta.join(format!("rangos-{rangos}"));
            descargar_propio_a(&origen, &archivo, &destino).await.unwrap();
            assert_eq!(fs::read(&destino).unwrap(), contenido);
            let pedidas = solicitudes.lock().unwrap().clone();
            assert_eq!(pedidas.len(), 2, "{pedidas:?}");
            let segunda = pedidas[1].to_ascii_lowercase();
            assert!(segunda.contains("range: bytes=") && segunda.contains(&format!("if-range: \"{}\"", archivo.sha512)), "{segunda}");
            tarea.abort();
        }
        fs::remove_dir_all(carpeta).unwrap();
    }
    #[test]
    fn el_host_reutiliza_hashes_de_mods_sin_cambios() {
        let (_temp, store, _i) = store();
        let carpeta = store.datos().unwrap().join("servidor");
        let mods = carpeta.join("mods");
        fs::create_dir_all(&mods).unwrap();
        fs::write(mods.join("a.jar"), b"uno").unwrap();
        let hace = std::time::SystemTime::now() - Duration::from_secs(3600);
        fs::File::options().write(true).open(mods.join("a.jar")).unwrap().set_modified(hace).unwrap();
        let servidor = dedicado::ServidorDedicado { carpeta, minecraft: "1.20.1".into(), loader: "fabric".into(), loader_version: "0.16.14".into(), excluir: BTreeSet::new(), incluir: BTreeSet::new(), incluir_servidor: false };
        assert_eq!(servidor.huella().unwrap(), fingerprint(&mods).unwrap());
        assert_eq!(dedicado::sha_vigente(&mods, "a.jar"), Some(hash(b"uno")));
        // Un cambio invalida el hash guardado y la siguiente huella lo relee.
        fs::write(mods.join("a.jar"), b"dos").unwrap();
        assert_eq!(dedicado::sha_vigente(&mods, "a.jar"), None);
        assert_eq!(servidor.huella().unwrap(), fingerprint(&mods).unwrap());
    }
    /// Fuente que falla ante cualquier consulta: prueba que no se usó la red.
    struct SinRed;
    impl Fuente for SinRed {
        async fn version(&self, _: &str) -> Result<Version, Error> { Err(Error::Datos) }
        async fn versiones(&self, _: &str, _: &str, _: &str) -> Result<Vec<Version>, Error> { Err(Error::Datos) }
        async fn nombre(&self, _: &str) -> Result<String, Error> { Err(Error::Datos) }
        async fn archivo(&self, _: &ModFile) -> Result<Vec<u8>, Error> { Err(Error::Datos) }
        async fn versiones_por_hash(&self, _: &[String]) -> Result<BTreeMap<String, Version>, Error> { Err(Error::Datos) }
        async fn solo_cliente(&self, _: &str) -> Result<bool, Error> { Err(Error::Datos) }
    }
    #[tokio::test]
    async fn sincronizacion_verificada_se_confirma_sin_red_hasta_que_cambia_la_carpeta() {
        let (_temp, store, i) = store();
        let f = fake();
        let plan = preparar(&f, &store, &i, index(&f, "v1"), "host".into()).await.unwrap();
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {}).await.unwrap();
        let juego = store.juego(&i.id).unwrap();
        assert!(juego.join(VERIFICADO).exists());
        let antes = huella(&store, &i.id).unwrap();

        // Mismo anfitrión y misma carpeta: plan vacío sin una sola consulta.
        let plan = preparar(&SinRed, &store, &i, index(&f, "v1"), "host".into()).await.unwrap();
        assert!(plan.sin_red && plan.cambios.is_empty());
        aplicar(&SinRed, &store, &i, &plan, CancellationToken::new(), |_, _| {}).await.unwrap();
        assert_eq!(huella(&store, &i.id).unwrap(), antes);

        // Otro origen o una revisión distinta vuelven al camino normal.
        assert!(preparar(&SinRed, &store, &i, index(&f, "v1"), "otro".into()).await.is_err());
        assert!(preparar(&SinRed, &store, &i, index(&f, "v2"), "host".into()).await.is_err());

        // Un mod personal nuevo invalida el atajo y un plan sin red obsoleto se rechaza.
        let dir = juego.join("mods");
        fs::write(dir.join("personal.jar"), b"nuevo").unwrap();
        assert!(preparar(&SinRed, &store, &i, index(&f, "v1"), "host".into()).await.is_err());
        let error = aplicar(&SinRed, &store, &i, &plan, CancellationToken::new(), |_, _| {}).await.unwrap_err();
        assert!(error.contains("cambiaron") || error.contains("Comprueba de nuevo"), "{error}");
        assert!(dir.join("personal.jar").exists() && dir.join("example-1.jar").exists());

        // Con la red, el plan normal conserva el mod personal y renueva el registro.
        let plan = preparar(&f, &store, &i, index(&f, "v1"), "host".into()).await.unwrap();
        assert!(!plan.sin_red && plan.cambios.is_empty());
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {}).await.unwrap();
        assert!(preparar(&SinRed, &store, &i, index(&f, "v1"), "host".into()).await.unwrap().sin_red);

        // Pasadas 24 h el registro caduca y se vuelve a confirmar con Modrinth.
        let mut registro = leer_verificado(&juego).unwrap();
        registro.fecha -= VIGENCIA_VERIFICADO;
        fs::write(juego.join(VERIFICADO), serde_json::to_vec(&registro).unwrap()).unwrap();
        assert!(preparar(&SinRed, &store, &i, index(&f, "v1"), "host".into()).await.is_err());
    }
    #[tokio::test]
    async fn descartar_la_revision_local_detiene_el_trabajo_bloqueante() {
        let (aviso, detenido) = std::sync::mpsc::channel();
        let espera = tokio::time::timeout(
            Duration::from_millis(50),
            bloqueante(move |token| {
                let inicio = std::time::Instant::now();
                while !token.is_cancelled() && inicio.elapsed() < Duration::from_secs(10) {
                    std::thread::sleep(Duration::from_millis(5));
                }
                let _ = aviso.send(token.is_cancelled());
                Ok(())
            }),
        )
        .await;
        assert!(espera.is_err());
        assert!(detenido.recv_timeout(Duration::from_secs(5)).unwrap());
    }
    #[tokio::test]
    async fn apogee_renombrado_y_duplicado_se_actualiza_por_id_y_hash_sin_perder_mods_personales() {
        fn jar(id: &str, contenido: &str) -> Vec<u8> {
            let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
            writer
                .start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
                .unwrap();
            write!(
                writer,
                "{{\"id\":\"{id}\",\"version\":\"1.1.1\",\"environment\":\"*\"}}"
            )
            .unwrap();
            writer
                .start_file("registros.txt", zip::write::SimpleFileOptions::default())
                .unwrap();
            writer.write_all(contenido.as_bytes()).unwrap();
            writer.finish().unwrap().into_inner()
        }
        for exacto_presente in [false, true] {
            let (_temp, store, i) = store();
            let f = fake();
            let game = store.juego(&i.id).unwrap();
            let dir = game.join("mods");
            fs::create_dir(&dir).unwrap();
            let viejo = jar("apogee", "registros anteriores");
            let nuevo = jar("apogee", "23 registros nuevos del servidor");
            let personal = jar("solo_cliente", "personal");
            fs::write(dir.join("apogee-1.1.1.jar"), &viejo).unwrap();
            fs::write(dir.join("apogee-copia.jar"), &viejo).unwrap();
            fs::write(dir.join("apogee-antiguo.jar.disabled"), &viejo).unwrap();
            fs::write(dir.join("personal.jar"), &personal).unwrap();
            if exacto_presente {
                fs::write(dir.join("apogee-1.2.0.jar"), &nuevo).unwrap();
            }
            let host = game.join("host");
            fs::create_dir(&host).unwrap();
            fs::write(host.join("apogee-1.2.0.jar"), &nuevo).unwrap();
            let mut indice = index(&f, "v1");
            indice.esquema = 2;
            indice.mods.clear();
            indice.archivos.push(ArchivoPropio {
                id: "icaro-host-apogee".into(),
                nombre: "Apogee".into(),
                version: "1.1.1".into(),
                archivo: "apogee-1.2.0.jar".into(),
                bytes: nuevo.len() as u64,
                sha512: hash(&nuevo),
            });
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap().to_string();
            let cancel = CancellationToken::new();
            let servicio = tokio::spawn(servir_archivos(
                listener,
                Arc::new(RwLock::new(Some(indice.clone()))),
                cancel.clone(),
                Some(host),
            ));
            let plan = preparar(&f, &store, &i, indice.clone(), addr.clone())
                .await
                .unwrap();
            assert!(plan.puede_aplicar() && plan.conflictos.is_empty());
            assert_eq!(plan.reemplazos.len(), 2);
            if !exacto_presente {
                assert!(plan.cambios.iter().any(|c| c.tipo == Tipo::Actualizar));
            }
            aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {})
                .await
                .unwrap();
            assert_eq!(fs::read(dir.join("apogee-1.2.0.jar")).unwrap(), nuevo);
            assert!(!dir.join("apogee-1.1.1.jar").exists() && !dir.join("apogee-copia.jar").exists());
            assert_eq!(fs::read(dir.join("personal.jar")).unwrap(), personal);
            assert!(dir.join("apogee-antiguo.jar.disabled").exists());
            let respaldo = fs::read_dir(&game)
                .unwrap()
                .flatten()
                .find(|e| {
                    e.file_name()
                        .to_string_lossy()
                        .starts_with(".icaro-mods-conflictos-")
                })
                .unwrap()
                .path();
            assert_eq!(fs::read(respaldo.join("apogee-1.1.1.jar")).unwrap(), viejo);
            let plan = preparar(&f, &store, &i, indice, addr).await.unwrap();
            assert!(plan.cambios.is_empty() && plan.reemplazos.is_empty());
            cancel.cancel();
            servicio.await.unwrap().unwrap();
        }
    }

    #[tokio::test]
    async fn conflicto_local_muestra_el_plan_y_solo_reemplaza_con_autorizacion_y_respaldo() {
        let (_temp, store, i) = store();
        let f = fake();
        let game = store.juego(&i.id).unwrap();
        let dir = game.join("mods");
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("survivor.jar"), b"mi mod personal").unwrap();
        fs::write(dir.join("cliente.jar"), b"solo cliente").unwrap();
        let host = game.join("host");
        fs::create_dir(&host).unwrap();
        fs::write(host.join("survivor.jar"), b"mod del anfitrion").unwrap();
        let mut indice = index(&f, "v2");
        indice.esquema = 2;
        indice.archivos.push(ArchivoPropio {
            id: "icaro-host-survivor".into(),
            nombre: "Survivor".into(),
            version: "2".into(),
            archivo: "survivor.jar".into(),
            bytes: 17,
            sha512: hash(b"mod del anfitrion"),
        });
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let cancel = CancellationToken::new();
        let servicio = tokio::spawn(servir_archivos(
            listener,
            Arc::new(RwLock::new(Some(indice.clone()))),
            cancel.clone(),
            Some(host),
        ));
        let mut plan = preparar(&f, &store, &i, indice.clone(), addr.clone())
            .await
            .unwrap();
        assert_eq!(plan.conflictos, ["survivor.jar"]);
        assert_eq!(plan.cambios.len(), 2);
        assert!(!plan.puede_aplicar());
        assert!(
            aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {})
                .await
                .is_err()
        );
        assert_eq!(
            fs::read(dir.join("survivor.jar")).unwrap(),
            b"mi mod personal"
        );
        plan.autorizar_reemplazos(true);
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {})
            .await
            .unwrap();
        assert_eq!(
            fs::read(dir.join("survivor.jar")).unwrap(),
            b"mod del anfitrion"
        );
        assert_eq!(fs::read(dir.join("cliente.jar")).unwrap(), b"solo cliente");
        let respaldo = fs::read_dir(&game)
            .unwrap()
            .flatten()
            .find(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with(".icaro-mods-conflictos-")
            })
            .unwrap()
            .path();
        assert_eq!(
            fs::read(respaldo.join("survivor.jar")).unwrap(),
            b"mi mod personal"
        );
        let plan = preparar(&f, &store, &i, indice, addr).await.unwrap();
        assert!(plan.conflictos.is_empty() && plan.cambios.is_empty());
        aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {})
            .await
            .unwrap();
        assert!(respaldo.join("survivor.jar").is_file());
        cancel.cancel();
        servicio.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn no_adivina_versiones_ni_sobrescribe_nombres_locales() {
        let (_temp, store, i) = store();
        let f = fake();
        let mut invalid = index(&f, "v2");
        invalid.mods[0].sha512 = "a".repeat(128);
        assert!(preparar(&f, &store, &i, invalid, "host".into())
            .await
            .is_err());
        let mut invalid = index(&f, "v2");
        invalid.loader_version = Some("0.16.13".into());
        assert!(preparar(&f, &store, &i, invalid, "host".into())
            .await
            .is_err());
        let dir = store.juego(&i.id).unwrap().join("mods");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("example-2.jar"), b"local").unwrap();
        let plan = preparar(&f, &store, &i, index(&f, "v2"), "host".into())
            .await
            .unwrap();
        assert!(
            aplicar(&f, &store, &i, &plan, CancellationToken::new(), |_, _| {})
                .await
                .is_err()
        );
        assert_eq!(fs::read(dir.join("example-2.jar")).unwrap(), b"local");
    }
    #[tokio::test]
    async fn servicio_solo_entrega_indice_y_rechaza_rutas_y_metodos() {
        let f = fake();
        let value = index(&f, "v2");
        let publication = Arc::new(RwLock::new(Some(value.clone())));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let cancel = CancellationToken::new();
        let worker = tokio::spawn(servir(listener, publication.clone(), cancel.clone()));
        assert_eq!(
            consultar(addr, CancellationToken::new())
                .await
                .unwrap()
                .revision()
                .unwrap(),
            value.revision().unwrap()
        );
        for request in [
            "POST /icaro/v1/mods HTTP/1.1\r\nHost: localhost\r\n\r\n",
            "GET /../instance.json HTTP/1.1\r\nHost: localhost\r\n\r\n",
        ] {
            let mut c = tokio::net::TcpStream::connect(addr).await.unwrap();
            c.write_all(request.as_bytes()).await.unwrap();
            let mut response = vec![];
            c.read_to_end(&mut response).await.unwrap();
            assert!(response.starts_with(b"HTTP/1.1 404"));
        }
        *publication.write().await = None;
        assert!(consultar(addr, CancellationToken::new()).await.is_err());
        cancel.cancel();
        worker.await.unwrap().unwrap();
        assert!(endpoint("8.8.8.8").is_err());
        assert!(endpoint("0.0.0.0").is_err());
        assert!(endpoint("10.147.17.1:65535").is_err());
        assert_eq!(endpoint("10.147.17.1").unwrap().port(), 25566);
    }
    #[tokio::test]
    #[ignore = "Descarga dos mods reales de Modrinth en instancias temporales aisladas"]
    async fn modrinth_real_publica_indice_http_y_sincroniza_contenido_exacto() {
        let (_host, host, hi) = store();
        let (_client, client, ci) = store();
        let catalog = Catalog::new().unwrap();
        let api = catalog
            .versions("fabric-api", Some("1.20.1"), Some("fabric"))
            .await
            .unwrap()
            .into_iter()
            .find(|v| v.version_type == "release")
            .unwrap();
        let sodium = catalog
            .versions("sodium", Some("1.20.1"), Some("fabric"))
            .await
            .unwrap()
            .into_iter()
            .find(|v| v.version_type == "release")
            .unwrap();
        super::super::instalar(
            &catalog,
            &host,
            &hi.id,
            vec![api.clone(), sodium.clone()],
            &hi.minecraft,
            &hi.loader,
            |_| {},
        )
        .await
        .unwrap();
        let value = publicar(&catalog, &host, &hi).await.unwrap();
        assert!(value.mods.iter().any(|m| m.proyecto == api.project_id));
        assert!(!value.mods.iter().any(|m| m.proyecto == sodium.project_id));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let token = CancellationToken::new();
        let worker = tokio::spawn(servir(
            listener,
            Arc::new(RwLock::new(Some(value))),
            token.clone(),
        ));
        let received = consultar(addr, CancellationToken::new()).await.unwrap();
        let plan = preparar(&catalog, &client, &ci, received, addr.to_string())
            .await
            .unwrap();
        aplicar(
            &catalog,
            &client,
            &ci,
            &plan,
            CancellationToken::new(),
            |_, _| {},
        )
        .await
        .unwrap();
        let installed = inventario(&client, &ci.id).unwrap();
        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].version_id, api.id);
        let host_file = host
            .juego(&hi.id)
            .unwrap()
            .join("mods")
            .join(&installed[0].archivo);
        let client_file = client
            .juego(&ci.id)
            .unwrap()
            .join("mods")
            .join(&installed[0].archivo);
        assert_eq!(
            hash_archivo(&host_file).unwrap(),
            hash_archivo(&client_file).unwrap()
        );
        token.cancel();
        worker.await.unwrap().unwrap();
    }
}
