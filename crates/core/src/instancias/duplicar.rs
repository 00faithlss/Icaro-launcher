// SPDX-License-Identifier: GPL-3.0-only
//! Duplicar una instancia: una copia independiente con otro id y otro nombre.
//!
//! Se copia todo lo que es de quien juega (mods, configuración, mundos,
//! recursos, capturas y ajustes de la instancia) y se deja fuera lo que no
//! debe heredarse: el estado privado de instalación (`.icaro`, con rutas de la
//! instancia original; se rehace verificado al jugar), el tiempo jugado, los
//! vínculos de Compartir (la copia no es la publicación ni la recibida) y los
//! archivos internos de sincronización. Los enlaces simbólicos no se siguen.
//! Todo va a una carpeta temporal y solo al final toma su nombre.
use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

/// Nombres que nunca se copian, en cualquier nivel.
fn excluido(nombre: &str, en_raiz: bool) -> bool {
    if en_raiz && matches!(nombre, ".icaro" | ".icaro-minecraft" | "estadisticas.json") {
        return true;
    }
    // Vínculos de Compartir, inventarios, planes y diarios de sincronización.
    nombre.starts_with(".icaro-")
}

impl Almacen {
    /// Copia la instancia `id` como `nombre`. Devuelve la nueva instancia.
    pub fn duplicar(&self, id: &str, nombre: &str) -> Result<Instancia, Error> {
        let original = self.leer(id)?;
        let origen = self.carpeta(id)?;
        let copia = Instancia {
            id: id_nuevo()?,
            nombre: nombre.trim().into(),
            creada: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| Error::Datos)?
                .as_nanos()
                .try_into()
                .map_err(|_| Error::Datos)?,
            ..original
        };
        validar(&copia)?;
        if self
            .cargar()?
            .instancias
            .iter()
            .any(|r| r.nombre.to_lowercase() == copia.nombre.to_lowercase())
        {
            return Err(Error::Repetida);
        }
        let destino = self.destino(&copia.nombre, None)?;
        let temporal = self.raiz.join(format!(".icaro-copia-{}", copia.id));
        let resultado = (|| {
            fs::create_dir(&temporal).map_err(|_| Error::Disco)?;
            copiar_carpeta(&origen, &temporal, true)?;
            if !temporal.join("minecraft").is_dir() {
                fs::create_dir(temporal.join("minecraft")).map_err(|_| Error::Disco)?;
            }
            // Mismo documento (con su configuración) y otra identidad.
            guardar_en(&temporal, &copia)?;
            fs::rename(&temporal, &destino).map_err(|_| Error::Disco)
        })();
        if let Err(e) = resultado {
            let _ = fs::remove_dir_all(&temporal);
            return Err(e);
        }
        self.rutas
            .lock()
            .map_err(|_| Error::Disco)?
            .insert(copia.id.clone(), destino);
        Ok(copia)
    }
}

fn copiar_carpeta(origen: &Path, destino: &Path, en_raiz: bool) -> Result<(), Error> {
    for entrada in fs::read_dir(origen).map_err(|_| Error::Disco)? {
        let entrada = entrada.map_err(|_| Error::Disco)?;
        let nombre = entrada.file_name();
        if excluido(&nombre.to_string_lossy(), en_raiz) {
            continue;
        }
        let meta = fs::symlink_metadata(entrada.path()).map_err(|_| Error::Disco)?;
        if meta.file_type().is_symlink() || enlace(&meta) {
            continue;
        }
        let hacia = destino.join(&nombre);
        if meta.is_dir() {
            fs::create_dir(&hacia).map_err(|_| Error::Disco)?;
            copiar_carpeta(&entrada.path(), &hacia, false)?;
        } else if meta.is_file() {
            fs::copy(entrada.path(), &hacia).map_err(|_| Error::Disco)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_copia_conserva_lo_del_jugador_y_no_hereda_vinculos_ni_tiempo() {
        let raiz = std::env::temp_dir().join(format!("icaro-duplicar-{}", id_nuevo().unwrap()));
        fs::create_dir_all(&raiz).unwrap();
        let store = Almacen::nuevo(raiz.join("instances"));
        fs::create_dir_all(raiz.join("instances")).unwrap();
        let i = store
            .crear_con_loader("Amigos", "1.21.1", "fabric", 2)
            .unwrap();
        store
            .guardar_configuracion(&i.id, &serde_json::json!({"memoria_max": 6144}))
            .unwrap();
        let base = store.carpeta(&i.id).unwrap();
        let juego = store.juego(&i.id).unwrap();
        fs::create_dir_all(juego.join("mods")).unwrap();
        fs::write(juego.join("mods/sodium.jar"), b"mod").unwrap();
        fs::create_dir_all(juego.join("saves/Mundo")).unwrap();
        fs::write(juego.join("saves/Mundo/level.dat"), b"mundo").unwrap();
        fs::write(juego.join(".icaro-compartir-vinculo.json"), b"{}").unwrap();
        fs::write(juego.join("mods/.icaro-mods-inventario.json"), b"{}").unwrap();
        fs::create_dir_all(base.join(".icaro")).unwrap();
        fs::write(base.join(".icaro/instalado.json"), b"{}").unwrap();
        fs::write(base.join("estadisticas.json"), b"{}").unwrap();
        fs::write(base.join("loader.json"), b"{\"x\":1}").unwrap();

        let copia = store.duplicar(&i.id, "Amigos (copia)").unwrap();
        assert_ne!(copia.id, i.id);
        assert_eq!(
            (
                copia.minecraft.as_str(),
                copia.loader.as_str(),
                copia.portada
            ),
            ("1.21.1", "fabric", 2)
        );
        let cbase = store.carpeta(&copia.id).unwrap();
        let cjuego = store.juego(&copia.id).unwrap();
        assert_eq!(fs::read(cjuego.join("mods/sodium.jar")).unwrap(), b"mod");
        assert_eq!(
            fs::read(cjuego.join("saves/Mundo/level.dat")).unwrap(),
            b"mundo"
        );
        assert!(cbase.join("loader.json").exists());
        assert!(!cjuego.join(".icaro-compartir-vinculo.json").exists());
        assert!(!cjuego.join("mods/.icaro-mods-inventario.json").exists());
        assert!(!cbase.join(".icaro").exists());
        assert!(!cbase.join("estadisticas.json").exists());
        assert_eq!(
            store.configuracion(&copia.id).unwrap().unwrap()["memoria_max"],
            6144
        );
        // El original queda intacto y la lista tiene las dos.
        assert!(juego.join(".icaro-compartir-vinculo.json").exists());
        assert_eq!(store.cargar().unwrap().instancias.len(), 2);
        assert!(matches!(
            store.duplicar(&i.id, "amigos (COPIA)"),
            Err(Error::Repetida)
        ));
        let _ = fs::remove_dir_all(raiz);
    }
}
