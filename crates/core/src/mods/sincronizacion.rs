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
            // Dos JAR del mismo proyecto (por ejemplo dos versiones de Sodium)
            // no pueden compartir identidad: el segundo queda como archivo local.
            let mut tomados: BTreeSet<String> = installed.iter().filter(|m| !m.project_id.is_empty()).map(|m| m.project_id.clone()).collect();
            for m in &mut installed {
                if m.project_id.is_empty() {
                    if let Some(v) = reconocidas.get(&m.sha512).filter(|v| v.files.iter().any(|f| f.hashes.sha512.eq_ignore_ascii_case(&m.sha512)) && tomados.insert(v.project_id.clone())) {
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
        // El inventario admite un registro por proyecto; los demás JAR quedan locales.
        let mut proyectos = BTreeSet::new();
        records.retain(|m| !m.project_id.is_empty() && proyectos.insert(m.project_id.clone()));
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
mod tests;
