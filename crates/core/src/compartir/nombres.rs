// SPDX-License-Identifier: GPL-3.0-only
//! Nombre de jugador con que se presenta cada equipo invitado.
//!
//! Es solo una etiqueta para que el anfitrión reconozca a sus amigos: la
//! autorización sigue dependiendo de la identidad del equipo (`accesos`), nunca
//! de este nombre. Se guarda aparte de `accesos.json` para que una versión
//! anterior pueda seguir leyendo sus autorizaciones.
use super::Error;
use std::{collections::BTreeMap, path::PathBuf, sync::Mutex};

/// Caracteres admitidos; los nombres de Minecraft usan 3–16, se deja margen.
const MAXIMO: usize = 32;
const MAXIMO_EQUIPOS: usize = 256;

pub struct Nombres {
    path: PathBuf,
    mapa: Mutex<BTreeMap<String, String>>,
}

/// Recorta y valida un nombre recibido por la red o puesto por el usuario.
pub fn limpiar(nombre: &str) -> Option<String> {
    let n = nombre.trim();
    let valido = !n.is_empty()
        && n.chars().count() <= MAXIMO
        && n.chars().all(|c| !c.is_control());
    valido.then(|| n.to_owned())
}

impl Nombres {
    /// Un archivo dañado o ausente empieza vacío: perder una etiqueta no debe
    /// impedir compartir.
    pub fn abrir(path: PathBuf) -> Self {
        let mapa = std::fs::read(&path)
            .ok()
            .filter(|b| b.len() <= 256 * 1024)
            .and_then(|b| serde_json::from_slice::<BTreeMap<String, String>>(&b).ok())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(k, v)| Some((k, limpiar(&v)?)))
            .collect();
        Self {
            path,
            mapa: Mutex::new(mapa),
        }
    }
    pub fn todos(&self) -> BTreeMap<String, String> {
        self.mapa.lock().map(|m| m.clone()).unwrap_or_default()
    }
    pub fn fijar(&self, equipo: &str, nombre: &str) -> Result<(), Error> {
        let nombre = limpiar(nombre).ok_or(Error::Datos("nombre de jugador"))?;
        let mut actual = self.mapa.lock().map_err(|_| Error::Disco)?;
        if actual.get(equipo) == Some(&nombre) {
            return Ok(());
        }
        if !actual.contains_key(equipo) && actual.len() >= MAXIMO_EQUIPOS {
            return Err(Error::Limite("nombres"));
        }
        let mut nuevo = actual.clone();
        nuevo.insert(equipo.to_owned(), nombre);
        crate::almacenamiento::escribir_json(
            &self.path,
            &serde_json::to_value(&nuevo).map_err(|_| Error::Disco)?,
        )
        .map_err(|_| Error::Disco)?;
        *actual = nuevo;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Temporal(PathBuf);
    impl Temporal {
        fn nueva() -> Self {
            use std::sync::atomic::{AtomicU32, Ordering};
            static N: AtomicU32 = AtomicU32::new(0);
            let p = std::env::temp_dir().join(format!(
                "icaro-nombres-{}-{}",
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for Temporal {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn guarda_el_nombre_y_rechaza_los_invalidos() {
        let dir = Temporal::nueva();
        let path = dir.0.join("nombres.json");
        let n = Nombres::abrir(path.clone());
        n.fijar("a", "  Forjador ").unwrap();
        assert!(n.fijar("b", "").is_err());
        assert!(n.fijar("b", "uno\ndos").is_err());
        assert!(n.fijar("b", &"x".repeat(40)).is_err());
        assert_eq!(Nombres::abrir(path).todos().get("a").map(String::as_str), Some("Forjador"));
    }

    #[test]
    fn un_archivo_danado_empieza_vacio() {
        let dir = Temporal::nueva();
        let path = dir.0.join("nombres.json");
        std::fs::write(&path, b"{no es json").unwrap();
        assert!(Nombres::abrir(path).todos().is_empty());
    }
}
