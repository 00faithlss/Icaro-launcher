// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Estadísticas locales de una instancia: tiempo jugado y espacio en disco.

use super::{enlace, escribir, Almacen, Error, MAX_METADATA};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

const ARCHIVO: &str = "estadisticas.json";
/// Límite de entradas al medir: evita recorrer sin fin un árbol anómalo.
const MAX_ENTRADAS: usize = 1_000_000;
static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[derive(Debug, Default, Serialize, Deserialize)]
struct Estadisticas {
    #[serde(default)]
    segundos_jugados: u64,
}

fn leer(carpeta: &Path) -> Result<Estadisticas, Error> {
    let ruta = carpeta.join(ARCHIVO);
    match fs::symlink_metadata(&ruta) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Estadisticas::default()),
        Err(_) => return Err(Error::Disco),
        Ok(m) if !m.is_file() || enlace(&m) || m.len() > MAX_METADATA => return Err(Error::Datos),
        Ok(_) => {}
    }
    serde_json::from_slice(&fs::read(&ruta).map_err(|_| Error::Disco)?).map_err(|_| Error::Datos)
}

impl Almacen {
    /// Segundos jugados acumulados; cero si la instancia nunca se jugó.
    pub fn tiempo_jugado(&self, id: &str) -> Result<u64, Error> {
        Ok(leer(&self.carpeta(id)?)?.segundos_jugados)
    }

    /// Suma una sesión terminada y devuelve el total. Un registro dañado
    /// devuelve error y se conserva, en lugar de reiniciar el tiempo a cero.
    pub fn sumar_tiempo_jugado(&self, id: &str, segundos: u64) -> Result<u64, Error> {
        let carpeta = self.carpeta(id)?;
        let _guard = LOCK.lock().map_err(|_| Error::Disco)?;
        let mut estadisticas = leer(&carpeta)?;
        estadisticas.segundos_jugados = estadisticas.segundos_jugados.saturating_add(segundos);
        let bytes = serde_json::to_vec_pretty(&estadisticas).map_err(|_| Error::Datos)?;
        escribir(&carpeta.join(ARCHIVO), &bytes)?;
        Ok(estadisticas.segundos_jugados)
    }

    /// Bytes de los archivos de la instancia. No sigue enlaces ni uniones, y
    /// omite lo que desaparece o no se puede leer mientras se recorre.
    pub fn tamano(&self, id: &str) -> Result<u64, Error> {
        let raiz = self.carpeta(id)?;
        let mut pendientes = vec![raiz.clone()];
        let (mut total, mut entradas) = (0u64, 0usize);
        while let Some(carpeta) = pendientes.pop() {
            let lista = match fs::read_dir(&carpeta) {
                Ok(lista) => lista,
                Err(_) if carpeta != raiz => continue,
                Err(_) => return Err(Error::Disco),
            };
            for entrada in lista.flatten() {
                entradas += 1;
                if entradas > MAX_ENTRADAS {
                    return Err(Error::Datos);
                }
                // DirEntry::metadata no sigue enlaces simbólicos.
                let Ok(metadata) = entrada.metadata() else {
                    continue;
                };
                if enlace(&metadata) {
                    continue;
                }
                if metadata.is_dir() {
                    pendientes.push(entrada.path());
                } else if metadata.is_file() {
                    total = total.saturating_add(metadata.len());
                }
            }
        }
        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    struct Prueba(PathBuf);
    impl Drop for Prueba {
        fn drop(&mut self) {
            if self.0.parent() == Some(std::env::temp_dir().as_path()) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
    }

    fn almacen() -> (Prueba, Almacen, String) {
        let raiz = std::env::temp_dir().join(format!(
            "icaro-estadisticas-test-{}",
            super::super::id_nuevo().unwrap()
        ));
        let almacen = Almacen::nuevo(&raiz);
        let i = almacen.crear("Prueba", "1.21.1", 0).unwrap();
        (Prueba(raiz), almacen, i.id)
    }

    #[test]
    fn el_tiempo_jugado_se_acumula_y_persiste_entre_almacenes() {
        let (prueba, almacen, id) = almacen();
        assert_eq!(almacen.tiempo_jugado(&id).unwrap(), 0);
        assert_eq!(almacen.sumar_tiempo_jugado(&id, 90).unwrap(), 90);
        assert_eq!(almacen.sumar_tiempo_jugado(&id, 30).unwrap(), 120);
        let otro = Almacen::nuevo(&prueba.0);
        assert_eq!(otro.tiempo_jugado(&id).unwrap(), 120);
    }

    #[test]
    fn un_registro_danado_no_reinicia_el_tiempo() {
        let (_prueba, almacen, id) = almacen();
        let ruta = almacen.carpeta(&id).unwrap().join(ARCHIVO);
        fs::write(&ruta, b"{no es json").unwrap();
        assert!(almacen.sumar_tiempo_jugado(&id, 10).is_err());
        assert_eq!(fs::read(&ruta).unwrap(), b"{no es json");
    }

    #[test]
    fn el_tamano_suma_los_archivos_de_la_instancia() {
        let (_prueba, almacen, id) = almacen();
        let antes = almacen.tamano(&id).unwrap();
        let mods = almacen.juego(&id).unwrap().join("mods");
        fs::create_dir_all(&mods).unwrap();
        fs::write(mods.join("a.jar"), vec![0u8; 1000]).unwrap();
        fs::write(mods.join("b.jar"), vec![0u8; 24]).unwrap();
        assert_eq!(almacen.tamano(&id).unwrap(), antes + 1024);
    }

    #[test]
    fn una_instancia_inexistente_no_tiene_estadisticas() {
        let (_prueba, almacen, _id) = almacen();
        assert!(almacen.tamano("0123456789abcdef0123456789abcdef").is_err());
        assert!(almacen
            .tiempo_jugado("0123456789abcdef0123456789abcdef")
            .is_err());
    }
}
