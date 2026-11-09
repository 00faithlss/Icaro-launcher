// SPDX-License-Identifier: GPL-3.0-only
//! Nombre legible, versión y entorno de los mods para revisar una publicación.
//!
//! Solo lee descriptores acotados dentro de cada JAR (`fabric.mod.json`,
//! `quilt.mod.json` o `META-INF/neoforge.mods.toml`/`mods.toml`). Nunca modifica
//! archivos ni decide qué se publica: la revisión solo los usa para mostrar
//! nombres y proponer atajos.

use super::{inventario::Inventario, revisiones::Revision, Error, RutaCompartida};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Seek},
    path::Path,
};
use tokio_util::sync::CancellationToken;

const LIMITE: u64 = 128 * 1024;
const LARGO: usize = 120;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MetadatosMod {
    pub id: String,
    pub nombre: String,
    pub version: String,
    /// Declarado solo para el cliente. Es información, no una prohibición.
    pub solo_cliente: bool,
}

/// Metadatos de los JAR actuales y de los que publicó la revisión anterior.
#[derive(Debug, Clone, Default)]
pub struct MetadatosPublicacion {
    pub actuales: BTreeMap<RutaCompartida, MetadatosMod>,
    pub anteriores: BTreeMap<RutaCompartida, MetadatosMod>,
}

/// Lee los descriptores de `mods/*.jar` del inventario y, para comparar
/// versiones, los de la revisión publicada que cambiaron o se retiraron.
pub fn analizar(
    juego: &Path,
    inventario: &Inventario,
    anterior: Option<&Revision>,
    cancel: &CancellationToken,
) -> Result<MetadatosPublicacion, Error> {
    let mut m = MetadatosPublicacion::default();
    for a in &inventario.archivos {
        if cancel.is_cancelled() {
            return Err(Error::Cancelada);
        }
        if es_jar(&a.ruta) {
            if let Some(d) = leer_jar(&juego.join(a.ruta.as_str())) {
                m.actuales.insert(a.ruta.clone(), d);
            }
        }
    }
    if let Some(r) = anterior {
        for a in &r.manifiesto.archivos {
            if cancel.is_cancelled() {
                return Err(Error::Cancelada);
            }
            let igual = inventario
                .archivos
                .binary_search_by(|f| f.ruta.cmp(&a.ruta))
                .ok()
                .and_then(|i| inventario.archivos.get(i))
                .is_some_and(|f| f.sha512 == a.sha512);
            if !es_jar(&a.ruta) || igual {
                continue;
            }
            if let Some(d) = r.archivo(&a.ruta).ok().and_then(|p| leer_jar(&p)) {
                m.anteriores.insert(a.ruta.clone(), d);
            }
        }
    }
    Ok(m)
}

fn es_jar(ruta: &RutaCompartida) -> bool {
    let r = ruta.as_str();
    r.starts_with("mods/") && r.to_ascii_lowercase().ends_with(".jar")
}

/// Descriptor del mod, o `None` si el JAR no declara uno legible.
pub fn leer_jar(path: &Path) -> Option<MetadatosMod> {
    let meta = crate::almacenamiento::regular(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    let mut jar = zip::ZipArchive::new(fs::File::open(path).ok()?).ok()?;
    if let Some(b) = entrada(&mut jar, "fabric.mod.json") {
        return fabric(&b);
    }
    if let Some(b) = entrada(&mut jar, "quilt.mod.json") {
        return quilt(&b);
    }
    for n in ["META-INF/neoforge.mods.toml", "META-INF/mods.toml"] {
        if let Some(b) = entrada(&mut jar, n) {
            return toml(&String::from_utf8_lossy(&b));
        }
    }
    None
}

fn entrada<R: Read + Seek>(jar: &mut zip::ZipArchive<R>, nombre: &str) -> Option<Vec<u8>> {
    let mut e = jar.by_name(nombre).ok()?;
    if e.size() > LIMITE {
        return None;
    }
    let mut b = Vec::new();
    e.by_ref().take(LIMITE + 1).read_to_end(&mut b).ok()?;
    (b.len() as u64 <= LIMITE).then_some(b)
}

fn json(b: &[u8]) -> Option<serde_json::Value> {
    serde_json::from_slice(b).ok().or_else(|| {
        // Algunos descriptores llevan saltos de línea sin escapar dentro de cadenas.
        let b: Vec<u8> = b
            .iter()
            .map(|c| if *c < 0x20 { b' ' } else { *c })
            .collect();
        serde_json::from_slice(&b).ok()
    })
}

/// Texto visible: sin controles, sin plantillas sin resolver y con largo acotado.
fn limpio(s: &str) -> String {
    let s = s.trim();
    if s.contains("${") {
        return String::new();
    }
    s.chars().filter(|c| !c.is_control()).take(LARGO).collect()
}

fn cadena(v: Option<&serde_json::Value>) -> String {
    v.and_then(|v| v.as_str()).map(limpio).unwrap_or_default()
}

fn armar(id: String, nombre: String, version: String, solo_cliente: bool) -> Option<MetadatosMod> {
    if id.is_empty() {
        return None;
    }
    Some(MetadatosMod {
        nombre: if nombre.is_empty() {
            id.clone()
        } else {
            nombre
        },
        id,
        version,
        solo_cliente,
    })
}

fn fabric(b: &[u8]) -> Option<MetadatosMod> {
    let d = json(b)?;
    armar(
        cadena(d.get("id")),
        cadena(d.get("name")),
        cadena(d.get("version")),
        d.get("environment").and_then(|v| v.as_str()) == Some("client"),
    )
}

fn quilt(b: &[u8]) -> Option<MetadatosMod> {
    let d = json(b)?;
    let l = d.get("quilt_loader")?;
    armar(
        cadena(l.get("id")),
        cadena(l.get("metadata").and_then(|m| m.get("name"))),
        cadena(l.get("version")),
        d.get("minecraft")
            .and_then(|m| m.get("environment"))
            .and_then(|v| v.as_str())
            == Some("client"),
    )
}

/// Lectura mínima del primer `[[mods]]`; no es un intérprete TOML completo.
fn toml(s: &str) -> Option<MetadatosMod> {
    let (mut en_mods, mut visto) = (false, false);
    let (mut id, mut nombre, mut version) = (String::new(), String::new(), String::new());
    let mut solo_cliente = false;
    for linea in s.lines() {
        let l = linea.trim();
        if l.starts_with('[') {
            if visto {
                break;
            }
            en_mods = l == "[[mods]]";
            visto = en_mods;
            continue;
        }
        let Some((k, v)) = l.split_once('=') else {
            continue;
        };
        let (k, v) = (k.trim(), valor(v));
        if !en_mods {
            if k == "clientSideOnly" {
                solo_cliente = v == "true";
            }
            continue;
        }
        match k {
            "modId" => id = limpio(&v),
            "displayName" => nombre = limpio(&v),
            "version" => version = limpio(&v),
            _ => {}
        }
    }
    armar(id, nombre, version, solo_cliente)
}

fn valor(v: &str) -> String {
    let v = v.trim();
    for q in ['"', '\''] {
        if let Some(r) = v.strip_prefix(q) {
            return r.split(q).next().unwrap_or_default().to_owned();
        }
    }
    v.split('#').next().unwrap_or_default().trim().to_owned()
}

/// Mod al que pertenece una configuración, por su carpeta o el prefijo del
/// archivo: `config/entityculling.json` y `config/jade/jade.json` → su modid.
pub fn mod_de_configuracion<'a>(
    ruta: &str,
    ids: impl IntoIterator<Item = &'a str>,
) -> Option<&'a str> {
    let resto = ruta.strip_prefix("config/")?;
    let primero = resto.split('/').next()?;
    let clave = |s: &str| s.to_ascii_lowercase().replace('-', "_");
    let raiz = clave(primero.split(['.', '-']).next().filter(|s| !s.is_empty())?);
    let carpeta = resto.contains('/').then(|| clave(primero));
    ids.into_iter().find(|id| {
        let id = clave(id);
        id == raiz || carpeta.as_deref() == Some(id.as_str())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn jar(dir: &Path, nombre: &str, entradas: &[(&str, &str)]) -> std::path::PathBuf {
        let path = dir.join(nombre);
        let mut z = zip::ZipWriter::new(fs::File::create(&path).unwrap());
        for (n, c) in entradas {
            z.start_file(*n, zip::write::SimpleFileOptions::default())
                .unwrap();
            z.write_all(c.as_bytes()).unwrap();
        }
        z.finish().unwrap();
        path
    }

    /// Carpeta temporal propia que se borra al terminar la prueba.
    struct Temporal(std::path::PathBuf);
    impl Temporal {
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for Temporal {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn temporal() -> Temporal {
        let mut nonce = [0; 16];
        getrandom::fill(&mut nonce).unwrap();
        let root =
            std::env::temp_dir().join(format!("icaro-metadatos-{:x}", u128::from_le_bytes(nonce)));
        fs::create_dir(&root).unwrap();
        Temporal(root)
    }

    #[test]
    fn lee_nombre_version_y_entorno_de_cada_loader() {
        let dir = temporal();
        let f = jar(
            dir.path(),
            "sodium.jar",
            &[(
                "fabric.mod.json",
                r#"{"id":"sodium","name":"Sodium","version":"0.6.5","environment":"client"}"#,
            )],
        );
        assert_eq!(
            leer_jar(&f),
            Some(MetadatosMod {
                id: "sodium".into(),
                nombre: "Sodium".into(),
                version: "0.6.5".into(),
                solo_cliente: true
            })
        );
        let q = jar(
            dir.path(),
            "q.jar",
            &[(
                "quilt.mod.json",
                r#"{"quilt_loader":{"id":"qmod","version":"1.0","metadata":{"name":"Q Mod"}},"minecraft":{"environment":"*"}}"#,
            )],
        );
        let q = leer_jar(&q).unwrap();
        assert_eq!((q.nombre.as_str(), q.solo_cliente), ("Q Mod", false));
        let n = jar(
            dir.path(),
            "n.jar",
            &[(
                "META-INF/neoforge.mods.toml",
                "modLoader=\"javafml\"\n[[mods]]\nmodId=\"jade\" # id\nversion=\"${file.jarVersion}\"\ndisplayName='Jade'\n[[dependencies.jade]]\nmodId=\"neoforge\"\n",
            )],
        );
        let n = leer_jar(&n).unwrap();
        assert_eq!(
            (n.id.as_str(), n.nombre.as_str(), n.version.as_str()),
            ("jade", "Jade", "")
        );
    }

    #[test]
    fn jar_sin_descriptor_o_enorme_no_inventa_datos() {
        let dir = temporal();
        let vacio = jar(dir.path(), "a.jar", &[("a.txt", "x")]);
        assert_eq!(leer_jar(&vacio), None);
        let grande = format!(r#"{{"id":"x","name":"{}"}}"#, "a".repeat(LIMITE as usize));
        let g = jar(dir.path(), "g.jar", &[("fabric.mod.json", &grande)]);
        assert_eq!(leer_jar(&g), None);
        assert_eq!(leer_jar(&dir.path().join("no-existe.jar")), None);
    }

    #[test]
    fn configuraciones_siguen_a_su_mod_por_prefijo_o_carpeta() {
        let ids = [
            "entityculling",
            "sodium",
            "jade",
            "sound_physics_remastered",
            "dynamic_fps",
        ];
        let de = |r| mod_de_configuracion(r, ids);
        assert_eq!(de("config/entityculling.json"), Some("entityculling"));
        assert_eq!(de("config/sodium-options.json"), Some("sodium"));
        assert_eq!(de("config/jade/jade.json"), Some("jade"));
        assert_eq!(
            de("config/sound_physics_remastered/soundphysics.properties"),
            Some("sound_physics_remastered")
        );
        assert_eq!(de("config/dynamic_fps.json"), Some("dynamic_fps"));
        assert_eq!(de("config/defaultoptions/options.txt"), None);
        assert_eq!(de("kubejs/server_scripts/a.js"), None);
    }
}
