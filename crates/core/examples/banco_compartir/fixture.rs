// SPDX-License-Identifier: GPL-3.0-only
use super::Resultado;
use icaro_core::compartir::{
    accesos::{Accesos, Autoridad},
    identidad::{AlmacenClave, Identidad},
    revisiones::{self, ContextoPublicacion, Revision},
    Error, Limites, Manifiesto,
};
use sha2::{Digest, Sha512};
use std::{
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    sync::{Arc, Mutex, RwLock},
};
use zeroize::Zeroizing;

struct Memoria(Mutex<Option<Zeroizing<Vec<u8>>>>);
impl AlmacenClave for Memoria {
    fn leer(&self) -> Result<Option<Zeroizing<Vec<u8>>>, Error> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn guardar(&self, bytes: &[u8]) -> Result<(), Error> {
        *self.0.lock().unwrap() = Some(Zeroizing::new(bytes.to_vec()));
        Ok(())
    }
}
pub struct Permiso;
impl Autoridad for Permiso {
    fn autorizado(
        &self,
        _: String,
    ) -> Pin<Box<dyn Future<Output = Result<bool, Error>> + Send + '_>> {
        Box::pin(async { Ok(true) })
    }
}
pub struct Fixture {
    pub root: PathBuf,
    pub host: Identidad,
    pub guest: Identidad,
    pub manifiesto: Manifiesto,
    pub revision: Arc<RwLock<Revision>>,
    pub acl: Arc<Accesos>,
}
// Solo esta guardia borra el directorio exclusivo creado con create_dir.
struct Temporal(PathBuf);
impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
impl Fixture {
    pub fn crear(archivos: usize, bytes: usize) -> Resultado<Self> {
        let host = Identidad::desde_almacen(&Memoria(Mutex::new(None)))?;
        let guest = Identidad::desde_almacen(&Memoria(Mutex::new(None)))?;
        let root = std::env::temp_dir().join(format!(
            "icaro-banco-compartir-{}-{}",
            std::process::id(),
            host.clave_publica()
        ));
        std::fs::create_dir(&root)?;
        let temporal = Temporal(root.clone());
        let almacen = root.join("publicacion");
        std::fs::create_dir_all(almacen.join("contenido"))?;
        let mut entradas = Vec::new();
        for indice in 0..archivos {
            let mut contenido: Vec<u8> = (0..bytes)
                .map(|i| ((i * 31 + indice) % 251) as u8)
                .collect();
            contenido[..8].copy_from_slice(&(indice as u64).to_le_bytes());
            let hash = format!("{:x}", Sha512::digest(&contenido));
            std::fs::write(almacen.join("contenido").join(&hash), contenido)?;
            entradas.push(
                serde_json::json!({"ruta": format!("config/archivo-{indice:08}.toml"),
                "bytes": bytes, "sha512": hash, "procedencia": {"tipo": "propio"}}),
            );
        }
        let manifiesto: Manifiesto = serde_json::from_value(serde_json::json!({
            "esquema": "icaro-compartir/1", "origen": {"anfitrion": host.clave_publica(),
                "publicacion": "ab".repeat(16)}, "revision": 1,
            "perfil": {"minecraft": "1.21.1", "loader": "vanilla", "loader_version": null},
            "archivos": entradas, "packs": []}))?;
        manifiesto.validar(&Limites::default())?;
        std::fs::write(
            almacen.join("vigente.json"),
            serde_json::to_vec(&manifiesto)?,
        )?;
        let contexto = ContextoPublicacion {
            juego: root.join("juego"),
            almacen,
            origen: manifiesto.origen.clone(),
            perfil: manifiesto.perfil.clone(),
            limites: Limites::default(),
        };
        let revision = revisiones::vigente(&contexto)?.ok_or("fixture sin revisión")?;
        let acl = Arc::new(Accesos::abrir(root.join("accesos.json"))?);
        acl.autorizar(&guest.clave_publica())?;
        std::mem::forget(temporal);
        Ok(Self {
            root,
            host,
            guest,
            manifiesto,
            revision: Arc::new(RwLock::new(revision)),
            acl,
        })
    }
    pub fn verificar(&self, destino: &Path) -> Resultado<()> {
        for archivo in &self.manifiesto.archivos {
            let bytes = std::fs::read(destino.join(archivo.ruta.as_str()))?;
            if bytes.len() as u64 != archivo.bytes
                || format!("{:x}", Sha512::digest(&bytes)) != archivo.sha512
            {
                return Err("integridad final incorrecta".into());
            }
        }
        Ok(())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
