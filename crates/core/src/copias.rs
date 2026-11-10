// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Copias de los datos del launcher (ajustes, cuentas sin conexión y grupos)
//! para cambiar de canal sin quedar atrapado en la beta. Cada copia guarda
//! la versión y el esquema de datos con que se hizo: bajar de versión solo
//! se permite con una copia que la versión de destino sepa leer. Nunca toca
//! instancias, mundos ni mods. Ejecutar fuera del hilo de iced.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Esquema de datos de esta versión. Subirlo cuando el formato de los datos
/// guardados cambie de forma que una versión anterior no pueda leerlo.
pub const ESQUEMA: u32 = 1;
/// Lo que se copia, relativo a la carpeta de datos de Ícaro.
pub const ARCHIVOS: [&str; 2] = ["estado.json", "accounts/locales.json"];
/// Copias que se conservan por defecto.
pub const CONSERVAR: usize = 3;
const MANIFIESTO: &str = "copia.json";
const MAX_ARCHIVO: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Copia {
    /// Nombre de la carpeta de la copia.
    #[serde(skip)]
    pub id: String,
    /// Segundos Unix.
    pub fecha: u64,
    pub version: String,
    pub beta: bool,
    pub esquema: u32,
    pub bytes: u64,
    pub archivos: Vec<String>,
}

impl Copia {
    /// Una versión de destino puede leer la copia si no viene de una beta,
    /// no es más nueva que ella y su esquema no es mayor.
    pub fn compatible(&self, destino: &str, esquema_destino: u32) -> bool {
        !self.beta
            && numeros(&self.version).zip(numeros(destino)).is_some_and(|(c, d)| c <= d)
            && self.esquema <= esquema_destino
    }
}

fn numeros(v: &str) -> Option<[u64; 3]> {
    let mut p = v.trim_start_matches('v').split(['.', '-']);
    Some([p.next()?.parse().ok()?, p.next()?.parse().ok()?, p.next()?.parse().ok()?])
}

fn carpeta(raiz: &Path) -> PathBuf {
    raiz.join("copias")
}

fn ahora() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Archivo regular sin enlaces; `None` si no existe.
fn regular(ruta: &Path) -> Result<Option<fs::Metadata>, String> {
    match fs::symlink_metadata(ruta) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(_) => Err("No se pudo leer un archivo de datos.".into()),
        Ok(_) => {
            let m = crate::almacenamiento::regular(ruta)?;
            if !m.is_file() || m.len() > MAX_ARCHIVO {
                return Err("Un archivo de datos no es válido para copiarlo.".into());
            }
            Ok(Some(m))
        }
    }
}

/// Escribe `bytes` en `destino` de forma atómica (temporal y renombrado).
fn escribir(destino: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(padre) = destino.parent() {
        fs::create_dir_all(padre).map_err(|_| "No se pudo crear la carpeta de datos.")?;
    }
    let temporal = destino.with_extension("icaro-copia.tmp");
    let resultado = (|| {
        use std::io::Write;
        let mut f = fs::File::create(&temporal).map_err(|_| "No se pudo escribir la copia.")?;
        f.write_all(bytes).map_err(|_| "No se pudo escribir la copia.")?;
        f.sync_all().map_err(|_| "No se pudo escribir la copia.")?;
        fs::rename(&temporal, destino).map_err(|_| "No se pudo reemplazar un archivo de datos.")?;
        Ok::<(), &str>(())
    })();
    let _ = fs::remove_file(&temporal);
    resultado.map_err(Into::into)
}

/// Guarda una copia de los datos actuales. La carpeta solo cuenta como copia
/// cuando su manifiesto se escribió al final.
pub fn crear(raiz: &Path, version: &str, beta: bool, esquema: u32) -> Result<Copia, String> {
    let fecha = ahora();
    let id = format!("{fecha}-{}", version.replace(|c: char| !c.is_ascii_alphanumeric() && c != '.', "_"));
    let dir = carpeta(raiz).join(&id);
    if dir.exists() {
        return Err("Ya hay una copia con esa fecha; vuelve a intentarlo.".into());
    }
    let mut copia = Copia { id: id.clone(), fecha, version: version.into(), beta, esquema, bytes: 0, archivos: Vec::new() };
    for rel in ARCHIVOS {
        let origen = raiz.join(rel);
        if regular(&origen)?.is_none() {
            continue;
        }
        let bytes = fs::read(&origen).map_err(|_| "No se pudo leer un archivo de datos.")?;
        escribir(&dir.join(rel), &bytes)?;
        copia.bytes += bytes.len() as u64;
        copia.archivos.push(rel.into());
    }
    let manifiesto = serde_json::to_vec_pretty(&copia).map_err(|_| "No se pudo preparar la copia.")?;
    escribir(&dir.join(MANIFIESTO), &manifiesto)?;
    Ok(copia)
}

/// Copias completas, de la más nueva a la más vieja. Las dañadas se omiten.
pub fn listar(raiz: &Path) -> Vec<Copia> {
    let Ok(lista) = fs::read_dir(carpeta(raiz)) else {
        return Vec::new();
    };
    let mut copias: Vec<Copia> = lista
        .flatten()
        .filter_map(|e| {
            let id = e.file_name().to_string_lossy().into_owned();
            let ruta = e.path().join(MANIFIESTO);
            regular(&ruta).ok()??;
            let mut c: Copia = serde_json::from_slice(&fs::read(&ruta).ok()?).ok()?;
            c.archivos.retain(|a| ARCHIVOS.contains(&a.as_str()));
            c.id = id;
            Some(c)
        })
        .collect();
    copias.sort_by(|a, b| b.fecha.cmp(&a.fecha).then(b.id.cmp(&a.id)));
    copias
}

fn ruta_copia(raiz: &Path, id: &str) -> Result<PathBuf, String> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c)) {
        return Err("Copia no válida.".into());
    }
    Ok(carpeta(raiz).join(id))
}

/// Borra las copias más viejas y deja `conservar`. La copia estable más
/// reciente se conserva siempre: es la única salida de la beta.
pub fn podar(raiz: &Path, conservar: usize) -> Result<(), String> {
    let copias = listar(raiz);
    let estable = copias.iter().find(|c| !c.beta).map(|c| c.id.clone());
    for c in copias.into_iter().skip(conservar) {
        if estable.as_deref() == Some(c.id.as_str()) {
            continue;
        }
        let dir = ruta_copia(raiz, &c.id)?;
        match crate::almacenamiento::regular(&dir) {
            Ok(m) if m.is_dir() => fs::remove_dir_all(&dir).map_err(|_| "No se pudo borrar una copia vieja.")?,
            _ => return Err("Una copia vieja no es una carpeta normal.".into()),
        }
    }
    Ok(())
}

/// Devuelve los datos a como estaban en la copia. Lo que no estaba en ella
/// se quita (quien restaura guarda antes una copia de lo actual).
pub fn restaurar(raiz: &Path, id: &str) -> Result<(), String> {
    let dir = ruta_copia(raiz, id)?;
    let copia = listar(raiz)
        .into_iter()
        .find(|c| c.id == id)
        .ok_or("La copia no está completa.")?;
    // Se leen todos los archivos antes de tocar nada.
    let mut contenido = Vec::new();
    for rel in &copia.archivos {
        let ruta = dir.join(rel);
        regular(&ruta)?.ok_or("Falta un archivo en la copia.")?;
        contenido.push((rel.clone(), fs::read(&ruta).map_err(|_| "No se pudo leer la copia.")?));
    }
    for rel in ARCHIVOS {
        match contenido.iter().find(|(r, _)| r == rel) {
            Some((_, bytes)) => escribir(&raiz.join(rel), bytes)?,
            None => {
                if regular(&raiz.join(rel))?.is_some() {
                    fs::remove_file(raiz.join(rel)).map_err(|_| "No se pudo quitar un archivo de datos.")?;
                }
            }
        }
    }
    Ok(())
}

/// Empezar de cero: guarda una copia de lo actual y quita ajustes y cuentas
/// sin conexión. Las instancias se vuelven a detectar desde su carpeta.
pub fn empezar_de_cero(raiz: &Path, version: &str, beta: bool) -> Result<Copia, String> {
    let copia = crear(raiz, version, beta, ESQUEMA)?;
    for rel in ARCHIVOS {
        if regular(&raiz.join(rel))?.is_some() {
            fs::remove_file(raiz.join(rel)).map_err(|_| "No se pudo quitar un archivo de datos.")?;
        }
    }
    Ok(copia)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raiz() -> PathBuf {
        let r = std::env::temp_dir().join(format!("icaro-copias-{}", ahora() as u128 * 1000 + rand_nanos()));
        fs::create_dir_all(r.join("accounts")).unwrap();
        r
    }
    fn rand_nanos() -> u128 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos() as u128
    }

    #[test]
    fn crear_listar_restaurar_y_podar_conservan_solo_los_datos_del_launcher() {
        let r = raiz();
        fs::write(r.join("estado.json"), b"{\"v\":1}").unwrap();
        fs::write(r.join("accounts/locales.json"), b"[\"Ana\"]").unwrap();
        fs::create_dir_all(r.join("instances/x")).unwrap();
        fs::write(r.join("instances/x/mundo.dat"), b"mundo").unwrap();
        let estable = crear(&r, "0.3.4", false, 1).unwrap();
        assert_eq!(estable.archivos.len(), 2);
        // Cambios en la beta.
        fs::write(r.join("estado.json"), b"{\"v\":2}").unwrap();
        fs::remove_file(r.join("accounts/locales.json")).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        let beta = crear(&r, "0.4.0", true, 2).unwrap();
        let lista = listar(&r);
        assert_eq!(lista[0].id, beta.id);
        assert!(!lista[0].compatible("0.3.4", 1));
        assert!(lista[1].compatible("0.3.4", 1));
        restaurar(&r, &estable.id).unwrap();
        assert_eq!(fs::read(r.join("estado.json")).unwrap(), b"{\"v\":1}");
        assert_eq!(fs::read(r.join("accounts/locales.json")).unwrap(), b"[\"Ana\"]");
        assert_eq!(fs::read(r.join("instances/x/mundo.dat")).unwrap(), b"mundo");
        restaurar(&r, &beta.id).unwrap();
        assert!(!r.join("accounts/locales.json").exists());
        // Podar deja la más nueva y, además, la última estable.
        podar(&r, 1).unwrap();
        let quedan = listar(&r);
        assert_eq!(quedan.len(), 2);
        assert!(quedan.iter().any(|c| c.id == estable.id));
        assert!(restaurar(&r, "../fuera").is_err());
        fs::remove_dir_all(r).unwrap();
    }

    #[test]
    fn la_compatibilidad_mira_canal_version_y_esquema() {
        let c = |version: &str, beta, esquema| Copia { id: "x".into(), fecha: 0, version: version.into(), beta, esquema, bytes: 0, archivos: vec![] };
        assert!(c("0.3.3", false, 1).compatible("0.3.4", 1));
        assert!(!c("0.3.3", false, 2).compatible("0.3.4", 1));
        assert!(!c("0.4.0", false, 1).compatible("0.3.4", 1));
        assert!(!c("0.3.4", true, 1).compatible("0.3.4", 1));
    }

    #[test]
    fn empezar_de_cero_guarda_una_copia_y_quita_los_datos() {
        let r = raiz();
        fs::write(r.join("estado.json"), b"{}").unwrap();
        let c = empezar_de_cero(&r, "0.4.0", true).unwrap();
        assert!(!r.join("estado.json").exists());
        assert_eq!(listar(&r)[0].id, c.id);
        fs::remove_dir_all(r).unwrap();
    }
}
