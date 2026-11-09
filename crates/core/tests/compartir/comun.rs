// SPDX-License-Identifier: GPL-3.0-only
use super::*;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static SECUENCIA: AtomicU64 = AtomicU64::new(0);
pub(super) struct Carpeta(pub(super) PathBuf);
impl Carpeta {
    pub(super) fn nueva() -> Self {
        let path = std::env::temp_dir().join(format!(
            "icaro-compartir-test-{}-{}",
            std::process::id(),
            SECUENCIA.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    pub(super) fn escribir(&self, ruta: &str, bytes: &[u8]) {
        let destino = self.0.join(ruta);
        fs::create_dir_all(destino.parent().unwrap()).unwrap();
        fs::write(destino, bytes).unwrap();
    }
}
impl Drop for Carpeta {
    fn drop(&mut self) {
        // El único destino recursivo es el sandbox creado por esta prueba.
        if self.0.parent() == Some(std::env::temp_dir().as_path())
            && self
                .0
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("icaro-compartir-test-")
        {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}

pub(super) fn manifiesto() -> serde_json::Value {
    serde_json::json!({
        "esquema": "icaro-compartir/1",
        "origen": { "anfitrion": "ab".repeat(32), "publicacion": "cd".repeat(16) },
        "revision": 1,
        "perfil": { "minecraft": "1.21.1", "loader": "fabric", "loader_version": "0.16.10" },
        "archivos": [{"ruta":"mods/ejemplo.jar", "bytes":3, "sha512":format!("{:x}",Sha512::digest(b"jar")), "procedencia":{"tipo":"propio"}}],
        "packs": []
    })
}

pub(super) fn origen_y_perfil() -> (icaro_core::compartir::Origen, icaro_core::compartir::Perfil) {
    let m = Manifiesto::leer(
        &serde_json::to_vec(&manifiesto()).unwrap(),
        &Limites::default(),
    )
    .unwrap();
    (m.origen, m.perfil)
}
