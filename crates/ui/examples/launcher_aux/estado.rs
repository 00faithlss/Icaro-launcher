// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Persistencia en disco de lo que el usuario organiza: grupos, a qué grupo
//! pertenece cada instancia y la configuración de Java de cada una.

use std::fs;
use std::path::PathBuf;

use icaro_ui::editor::ConfigJava;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
struct JavaGuardado {
    java: Option<String>,
    #[serde(default)]
    heredar: bool,
    #[serde(default)]
    heredar_memoria: bool,
    #[serde(default)]
    java_requiere_seleccion: bool,
    #[serde(default = "verdadero")]
    descarga: bool,
    memoria_min: u32,
    memoria_max: u32,
    metaspace: u32,
    argumentos: String,
}

impl Default for JavaGuardado {
    fn default()->Self {let c=crate::config_por_defecto();Self{java:c.java,heredar:c.heredar,heredar_memoria:c.heredar_memoria,java_requiere_seleccion:c.java_requiere_seleccion,descarga:c.descarga,memoria_min:c.memoria_min,memoria_max:c.memoria_max,metaspace:c.metaspace,argumentos:c.argumentos}}
}

#[derive(Serialize, Deserialize)]
pub struct Guardado {
    #[serde(default)]
    pub apariencia: AparienciaGuardada,
    #[serde(default)]
    pub java_global: crate::java_real::Preferencias,
    #[serde(default)]
    pub ids: Vec<String>,
    pub lista_grupos: Vec<String>,
    pub grupos: Vec<Option<String>>,
    pub cerrados: Vec<Option<String>>,
    /// Modpacks instalados como instancias, por nombre.
    #[serde(default)]
    pub modpacks: Vec<String>,
    #[serde(default)]
    configs: Vec<JavaGuardado>,
    /// Canal elegido en Ajustes; sin valor se usa el de la compilación.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canal: Option<icaro_core::actualizacion::Canal>,
}

/// Preferencias de Apariencia compatibles con estados anteriores.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AparienciaGuardada {
    pub tema: usize,
    pub metal: usize,
    pub escala: i32,
    pub reducir: bool,
}

impl Default for AparienciaGuardada {
    fn default() -> Self { Self { tema: 0, metal: 0, escala: 100, reducir: false } }
}

impl AparienciaGuardada {
    pub fn normalizar(&mut self) {
        self.tema = self.tema.min(2);
        self.metal = self.metal.min(2);
        self.escala = self.escala.clamp(80, 150);
    }
}

pub fn directorio(demo: bool) -> PathBuf {
    if demo {
        return std::env::temp_dir().join("icaro-demo");
    }
    #[cfg(windows)]
    let base = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(not(windows))]
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")));
    base.unwrap_or_else(std::env::temp_dir).join("Icaro")
}

fn ruta(demo: bool) -> PathBuf {
    directorio(demo).join("estado.json")
}

impl Guardado {
    pub fn nuevo(
        lista_grupos: &[String],
        grupos: &[Option<String>],
        cerrados: &[Option<String>],
        configs: &[ConfigJava],
        modpacks: &[String],
    ) -> Self {
        Self {
            apariencia: AparienciaGuardada::default(),
            java_global: Default::default(),
            ids: Vec::new(),
            lista_grupos: lista_grupos.to_vec(),
            grupos: grupos.to_vec(),
            cerrados: cerrados.to_vec(),
            modpacks: modpacks.to_vec(),
            configs: configs
                .iter()
                .map(|c| JavaGuardado {
                    java: c.java.clone(),
                    heredar: c.heredar, heredar_memoria: c.heredar_memoria, java_requiere_seleccion: c.java_requiere_seleccion,
                    descarga: c.descarga,
                    memoria_min: c.memoria_min,
                    memoria_max: c.memoria_max,
                    metaspace: c.metaspace,
                    argumentos: c.argumentos.clone(),
                })
                .collect(),
            canal: None,
        }
    }

    /// Asocia ajustes a la identidad de la instancia, no a su posición en la lista.
    pub fn asociar(&mut self, ids: Vec<String>) {
        self.ids = ids;
    }

    pub fn restaurar_instancias(&mut self, ids: Vec<String>) {
        if self.ids.is_empty() {
            self.ids = ids.clone();
        }
        let indices: Vec<_> = ids.iter().map(|id| self.ids.iter().position(|i| i == id)).collect();
        self.grupos = indices.iter().map(|i| i.and_then(|i| self.grupos.get(i).cloned()).flatten()).collect();
        self.configs = indices.iter().map(|i| i.and_then(|i| self.configs.get(i).cloned()).unwrap_or_else(|| {
            let c = crate::config_por_defecto();
            JavaGuardado { heredar: c.heredar, heredar_memoria: c.heredar_memoria, java_requiere_seleccion: c.java_requiere_seleccion, descarga: c.descarga, java: c.java, memoria_min: c.memoria_min, memoria_max: c.memoria_max, metaspace: c.metaspace, argumentos: c.argumentos }
        })).collect();
        self.ids = ids;
    }

    pub fn cargar_configs(&mut self, store: &icaro_core::instancias::Almacen) -> Result<(), String> {
        for (n, id) in self.ids.iter().enumerate() {
            match store.configuracion(id).map_err(|e| e.to_string())? {
                Some(value) => self.configs[n] = serde_json::from_value(value).map_err(|_| "Configuración de instancia inválida")?,
                None => store.guardar_configuracion(id, &serde_json::to_value(&self.configs[n]).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?,
            }
        }
        Ok(())
    }

    pub fn configs(&self) -> Vec<ConfigJava> {
        self.configs
            .iter()
            .map(|c| ConfigJava {
                java: c.java.clone(),
                heredar: c.heredar, heredar_memoria: c.heredar_memoria, java_requiere_seleccion: c.java_requiere_seleccion,
                descarga: c.descarga,
                memoria_min: c.memoria_min,
                memoria_max: c.memoria_max,
                metaspace: c.metaspace,
                argumentos: c.argumentos.clone(),
            })
            .collect()
    }

    pub fn cargar(demo: bool) -> Option<Self> {
        serde_json::from_str(&fs::read_to_string(ruta(demo)).ok()?).ok()
    }

    /// Serializa el estado; el llamador decide si escribirlo.
    pub fn texto(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_default()
    }

    pub fn escribir(texto: &str, demo: bool) -> Result<(), String> {
        let r = ruta(demo);
        let mut value: serde_json::Value = serde_json::from_str(texto).map_err(|e| e.to_string())?;
        if !demo {
            let saved: Self = serde_json::from_str(texto).map_err(|e| e.to_string())?;
            let store = icaro_core::instancias::Almacen::nuevo(directorio(false).join("instances"));
            for (id, config) in saved.ids.iter().zip(&saved.configs) {
                store.guardar_configuracion(id, &serde_json::to_value(config).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
            }
            value.as_object_mut().ok_or("Estado inválido")?.remove("configs");
            value["schema"] = 2.into();
        }
        if let Ok(bytes) = fs::read(&r) {
            if let Ok(mut previous) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                if let Some(object) = previous.as_object_mut() {
                    object.extend(value.as_object().ok_or("Estado inválido")?.clone());
                    if !demo { object.remove("configs"); }
                    value = previous;
                }
            }
        }
        icaro_core::almacenamiento::escribir_json(&r, &value)
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn estados_anteriores_restauran_apariencia_por_defecto() {
        let a: Guardado = serde_json::from_str(r#"{"lista_grupos":[],"grupos":[],"cerrados":[],"configs":[]}"#).unwrap();
        assert_eq!(a.apariencia.escala, 100);
        assert_eq!(a.apariencia.tema, 0);
        assert_eq!(a.apariencia.metal, 0);
        let mut b: AparienciaGuardada = serde_json::from_str(r#"{"tema":44,"escala":300}"#).unwrap();
        b.normalizar();
        assert_eq!((b.tema, b.metal, b.escala), (2, 0, 150));
    }

    #[test]
    fn java_antiguo_se_conserva_y_ajustes_nuevos_sobreviven_reinicio() {
        let old: Guardado = serde_json::from_str(r#"{"lista_grupos":[],"grupos":[],"cerrados":[],"configs":[{"java":"mi-java","memoria_min":2048,"memoria_max":4096,"metaspace":512,"argumentos":""}]}"#).unwrap();
        let configs = old.configs();
        assert!(!configs[0].heredar);
        assert!(!configs[0].heredar_memoria);
        assert!(configs[0].descarga);
        assert_eq!(configs[0].java.as_deref(), Some("mi-java"));
        let mut config = crate::config_por_defecto();
        config.descarga = false;
        let mut saved = Guardado::nuevo(&[], &[], &[], &[config.clone()], &[]);
        saved.java_global = crate::java_real::Preferencias { ruta: Some("global-java".into()), descarga: false, ..Default::default() };
        let restored: Guardado = serde_json::from_str(&saved.texto()).unwrap();
        assert_eq!(restored.configs(), vec![config]);
        assert_eq!(restored.java_global.ruta.as_deref(), Some("global-java"));
        assert!(!restored.java_global.descarga);
    }
}

fn verdadero() -> bool { true }
