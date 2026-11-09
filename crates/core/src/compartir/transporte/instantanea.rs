// SPDX-License-Identifier: GPL-3.0-only
//! Índice de una revisión inmutable; reemplazarla invalida la instantánea.
use super::{Error, Estado, Procedencia, Revision};
use std::{collections::HashMap, sync::Arc};

pub(super) struct Instantanea {
    pub revision: Arc<Revision>,
    pub archivos: HashMap<String, usize>,
}
impl Estado {
    pub(super) fn instantanea(&self) -> Result<Arc<Instantanea>, Error> {
        let vigente = self.revision.read().map_err(|_| Error::Disco)?;
        let mut cache = self.instantanea.lock().map_err(|_| Error::Disco)?;
        Ok(actualizar(&mut cache, &vigente))
    }
}
fn actualizar(cache: &mut Option<Arc<Instantanea>>, vigente: &Revision) -> Arc<Instantanea> {
    if let Some(actual) = cache.as_ref() {
        let m = &actual.revision.manifiesto;
        let v = &vigente.manifiesto;
        // El número identifica contenido inmutable dentro de origen/perfil.
        if m.origen == v.origen && m.revision == v.revision && m.perfil == v.perfil {
            return actual.clone();
        }
    }
    let archivos = vigente
        .manifiesto
        .archivos
        .iter()
        .enumerate()
        .filter(|(_, a)| matches!(a.procedencia, Procedencia::Propio))
        .map(|(i, a)| (a.sha512.clone(), i))
        .collect();
    let nueva = Arc::new(Instantanea {
        revision: Arc::new(vigente.clone()),
        archivos,
    });
    *cache = Some(nueva.clone());
    nueva
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compartir::{
        revisiones::{self, ContextoPublicacion},
        Limites, Manifiesto,
    };

    #[test]
    fn reutiliza_inmutable_e_invalida_revision_origen_y_perfil() {
        let mut nonce = [0; 16];
        getrandom::fill(&mut nonce).unwrap();
        let root = std::env::temp_dir().join(format!(
            "icaro-instantanea-{:x}",
            u128::from_le_bytes(nonce)
        ));
        std::fs::create_dir(&root).unwrap();
        let m: Manifiesto = serde_json::from_value(serde_json::json!({
            "esquema":"icaro-compartir/1", "origen":{"anfitrion":"ab".repeat(32),"publicacion":"ab".repeat(16)},
            "revision":1,"perfil":{"minecraft":"1.21.1","loader":"vanilla","loader_version":null},
            "archivos":[{"ruta":"config/a.toml","bytes":3,"sha512":"ab".repeat(64),"procedencia":{"tipo":"propio"}}],"packs":[]
        })).unwrap();
        std::fs::write(root.join("vigente.json"), serde_json::to_vec(&m).unwrap()).unwrap();
        let ctx = ContextoPublicacion {
            juego: root.join("juego"),
            almacen: root.clone(),
            origen: m.origen.clone(),
            perfil: m.perfil.clone(),
            limites: Limites::default(),
        };
        let mut revision = revisiones::vigente(&ctx).unwrap().unwrap();
        let mut cache = None;
        let primera = actualizar(&mut cache, &revision);
        assert!(Arc::ptr_eq(&primera, &actualizar(&mut cache, &revision)));
        revision.manifiesto.revision += 1;
        revision.manifiesto.archivos[0].sha512 = "cd".repeat(64);
        let segunda = actualizar(&mut cache, &revision);
        assert!(!Arc::ptr_eq(&primera, &segunda));
        assert!(segunda.archivos.contains_key(&"cd".repeat(64)));
        assert!(!segunda.archivos.contains_key(&"ab".repeat(64)));
        assert!(primera.archivos.contains_key(&"ab".repeat(64)));
        revision.manifiesto.origen.publicacion = "cd".repeat(16);
        let tercero = actualizar(&mut cache, &revision);
        assert!(!Arc::ptr_eq(&segunda, &tercero));
        revision.manifiesto.perfil.minecraft = "1.21.2".into();
        assert!(!Arc::ptr_eq(&tercero, &actualizar(&mut cache, &revision)));
        assert!(revision.archivo_en(usize::MAX).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
