// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Mod sugerido únicamente durante la primera instalación de Fabric.
use super::*;
use crate::instancias::Instancia;
use tokio_util::sync::CancellationToken;

const PROJECT: &str = "P7dR8mSH";

fn presente(store: &Almacen, instance: &Instancia) -> Result<bool, Error> {
    let game = store.juego(&instance.id).map_err(|_| Error::Disco)?;
    let mods = inventario(store, &instance.id)?;
    let _lock = guardia(&game)?;
    for item in mods {
        if item.project_id == PROJECT || item.project_id == "fabric-api" {
            return Ok(true);
        }
        let path = ruta_mod(&game.join("mods"), &item);
        regular(&path, false)?;
        let Ok(mut jar) = zip::ZipArchive::new(fs::File::open(path).map_err(|_| Error::Disco)?)
        else {
            continue;
        };
        let Ok(mut descriptor) = jar.by_name("fabric.mod.json") else {
            continue;
        };
        if descriptor.size() > 128 * 1024 {
            return Err(Error::Datos);
        }
        let mut bytes = Vec::new();
        descriptor
            .by_ref()
            .take(128 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| Error::Disco)?;
        if bytes.len() > 128 * 1024 {
            return Err(Error::Datos);
        }
        if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
            if value["id"].as_str() == Some("fabric-api")
                || value["provides"]
                    .as_array()
                    .is_some_and(|values| values.iter().any(|id| id.as_str() == Some("fabric-api")))
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

pub async fn instalar_por_defecto(
    store: &Almacen,
    instance: &Instancia,
    cancel: CancellationToken,
    progress: impl Fn(&str) + Send,
) -> Result<(), String> {
    if instance.loader != "fabric" {
        return Ok(());
    }
    if cancel.is_cancelled() {
        return Err("Instalación cancelada.".into());
    }
    let source = Catalog::new()
        .map_err(|e| e.to_string())?
        .with_metadata_cache();
    instalar_con_fuente(&source, store, instance, cancel, progress)
        .await
        .map_err(|e| e.to_string())
}

async fn instalar_con_fuente(
    source: &impl Fuente,
    store: &Almacen,
    instance: &Instancia,
    cancel: CancellationToken,
    progress: impl Fn(&str) + Send,
) -> Result<(), Error> {
    if presente(store, instance)? {
        return Ok(());
    }
    progress("Buscando Fabric API compatible");
    let versions = tokio::select! {
        biased;
        _=cancel.cancelled()=>return Err(Error::Datos),
        result=source.versiones(PROJECT,&instance.minecraft,"fabric")=>result?,
    };
    let Some(version) = ultima(
        versions
            .into_iter()
            .filter(|v| v.project_id == PROJECT && compatible(v, &instance.minecraft, "fabric"))
            .collect(),
    ) else {
        progress("Fabric API aún no tiene una versión compatible; Fabric se conserva instalado");
        return Ok(());
    };
    // El instalador verifica SHA-512 y publica la transacción completa. Una
    // cancelación durante la red deja los originales y nunca publica a medias.
    tokio::select! {
        biased;
        _=cancel.cancelled()=>Err(Error::Datos),
        result=instalar(source,store,&instance.id,vec![version],&instance.minecraft,"fabric",progress)=>result.map(|_|()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct FuentePrueba {
        versiones: Vec<Version>,
        fallo: bool,
        descargas: AtomicUsize,
    }
    impl Fuente for FuentePrueba {
        async fn versiones(&self, _: &str, _: &str, _: &str) -> Result<Vec<Version>, Error> {
            Ok(self.versiones.clone())
        }
        async fn version(&self, id: &str) -> Result<Version, Error> {
            self.versiones
                .iter()
                .find(|v| v.id == id)
                .cloned()
                .ok_or(Error::Datos)
        }
        async fn nombre(&self, _: &str) -> Result<String, Error> {
            Ok("Fabric API".into())
        }
        async fn archivo(&self, file: &ModFile) -> Result<Vec<u8>, Error> {
            self.descargas.fetch_add(1, Ordering::Relaxed);
            Ok(if self.fallo {
                b"alterado".to_vec()
            } else {
                file.filename.as_bytes().to_vec()
            })
        }
    }
    fn fuente(fallo: bool, mc: &str) -> FuentePrueba {
        let filename = "fabric-api-prueba.jar";
        let version=serde_json::from_value(serde_json::json!({
            "id":"prueba","project_id":PROJECT,"name":"Fabric API","version_number":"1","version_type":"release",
            "game_versions":[mc],"loaders":["fabric"],"files":[{"filename":filename,
                "url":"https://cdn.modrinth.com/data/P7dR8mSH/versions/prueba/fabric-api-prueba.jar",
                "size":filename.len(),"primary":true,"hashes":{"sha512":hash(filename.as_bytes())}}],
            "dependencies":[],"date_published":"2026-10-01T00:00:00Z"
        })).unwrap();
        FuentePrueba {
            versiones: vec![version],
            fallo,
            descargas: AtomicUsize::new(0),
        }
    }

    fn instancia() -> (PathBuf, Almacen, Instancia) {
        let root = std::env::temp_dir().join("icaro-fabric-api");
        // Cada caso usa una ruta exclusiva, incluso con pruebas paralelas.
        let mut nonce = [0u8; 16];
        getrandom::fill(&mut nonce).unwrap();
        let root = root.join(hash(&nonce));
        let store = Almacen::nuevo(&root);
        let i = store
            .crear_con_loader("Fabric", "1.21.1", "fabric", 0)
            .unwrap();
        (root, store, i)
    }

    #[test]
    fn reconoce_api_local_renombrada_y_desactivada() {
        let (root, store, i) = instancia();
        let dir = store.juego(&i.id).unwrap().join("mods");
        fs::create_dir_all(&dir).unwrap();
        let file = fs::File::create(dir.join("personal.jar.disabled")).unwrap();
        let mut jar = zip::ZipWriter::new(file);
        jar.start_file("fabric.mod.json", zip::write::SimpleFileOptions::default())
            .unwrap();
        jar.write_all(br#"{"id":"fabric-api","version":"1"}"#)
            .unwrap();
        jar.finish().unwrap();
        assert!(presente(&store, &i).unwrap());
        assert!(dir.join("personal.jar.disabled").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn instala_compatible_conserva_mods_y_no_reactiva_la_api() {
        let (root, store, i) = instancia();
        let dir = store.juego(&i.id).unwrap().join("mods");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("personal.jar"), b"personal").unwrap();
        let source = fuente(false, "1.21.1");
        instalar_con_fuente(&source, &store, &i, CancellationToken::new(), |_| {})
            .await
            .unwrap();
        assert_eq!(fs::read(dir.join("personal.jar")).unwrap(), b"personal");
        assert_eq!(source.descargas.load(Ordering::Relaxed), 1);
        alternar(&store, &i.id, "fabric-api-prueba.jar").unwrap();
        instalar_con_fuente(&source, &store, &i, CancellationToken::new(), |_| {})
            .await
            .unwrap();
        assert_eq!(source.descargas.load(Ordering::Relaxed), 1);
        assert!(dir.join("fabric-api-prueba.jar.disabled").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn sin_version_compatible_no_descarga_y_un_hash_incorrecto_conserva_originales() {
        let (root, store, i) = instancia();
        let dir = store.juego(&i.id).unwrap().join("mods");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("personal.jar"), b"personal").unwrap();
        let incompatible = fuente(false, "1.20.1");
        instalar_con_fuente(&incompatible, &store, &i, CancellationToken::new(), |_| {})
            .await
            .unwrap();
        assert_eq!(incompatible.descargas.load(Ordering::Relaxed), 0);
        let corrupta = fuente(true, "1.21.1");
        assert!(
            instalar_con_fuente(&corrupta, &store, &i, CancellationToken::new(), |_| {})
                .await
                .is_err()
        );
        assert_eq!(fs::read(dir.join("personal.jar")).unwrap(), b"personal");
        assert!(!dir.join("fabric-api-prueba.jar").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
