// SPDX-License-Identifier: GPL-3.0-only
//! Selección privada y borradores: editar la instancia nunca publica una revisión.
use super::{inventario::Inventario, Artefacto, Error, Manifiesto, Procedencia, RutaCompartida};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Seleccion {
    privadas: BTreeSet<RutaCompartida>,
}
impl Seleccion {
    pub fn mantener_privado(&mut self, ruta: RutaCompartida, privado: bool) {
        if privado {
            self.privadas.insert(ruta);
        } else {
            self.privadas.remove(&ruta);
        }
    }
    pub fn es_privado(&self, ruta: &RutaCompartida) -> bool {
        self.privadas.contains(ruta)
    }
    pub fn cargar(juego: &Path) -> Result<Self, Error> {
        let ruta = juego.join(".icaro-compartir-privado.json");
        match fs::symlink_metadata(&ruta) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(_) => Err(Error::Disco),
            Ok(_) => {
                let meta = crate::almacenamiento::regular(&ruta)
                    .map_err(|_| Error::Datos("selección privada"))?;
                if !meta.is_file() || meta.len() > 4 * 1024 * 1024 {
                    return Err(Error::Datos("selección privada"));
                }
                serde_json::from_reader(
                    fs::File::open(ruta)
                        .map_err(|_| Error::Disco)?
                        .take(4 * 1024 * 1024 + 1),
                )
                .map_err(|_| Error::Datos("selección privada"))
            }
        }
    }
    pub fn guardar(&self, juego: &Path) -> Result<(), Error> {
        if !crate::almacenamiento::regular(juego)
            .map_err(|_| Error::Disco)?
            .is_dir()
        {
            return Err(Error::Datos("carpeta de juego"));
        }
        crate::almacenamiento::escribir_json(
            &juego.join(".icaro-compartir-privado.json"),
            &serde_json::to_value(self).map_err(|_| Error::Datos("selección privada"))?,
        )
        .map_err(|_| Error::Disco)
    }
}
use std::io::Read;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Eleccion {
    Compartir,
    Privado,
    Incluir,
    Posponer,
    Retirar,
}

#[derive(Debug, Clone)]
pub struct Borrador {
    pub(crate) base: Option<Manifiesto>,
    actuales: BTreeMap<RutaCompartida, Artefacto>,
    elecciones: BTreeMap<RutaCompartida, Eleccion>,
    autorizados: BTreeSet<String>,
    packs: Vec<String>,
}
impl Borrador {
    pub fn eleccion(&self, ruta: &RutaCompartida) -> Option<Eleccion> {
        self.elecciones.get(ruta).copied()
    }
    pub fn archivo_actual(&self, ruta: &RutaCompartida) -> Option<&Artefacto> {
        self.actuales.get(ruta)
    }
    pub fn packs_publicados(&self) -> &[String] {
        &self.packs
    }
    pub fn comparar(
        inventario: &Inventario,
        base: Option<&Manifiesto>,
        seleccion: &Seleccion,
    ) -> Result<Self, Error> {
        let mut actuales = BTreeMap::new();
        for archivo in &inventario.archivos {
            let procedencia = base
                .and_then(|m| {
                    m.archivos
                        .iter()
                        .find(|a| a.ruta == archivo.ruta && a.sha512 == archivo.sha512)
                })
                .map_or(Procedencia::Propio, |a| a.procedencia.clone());
            if actuales
                .insert(
                    archivo.ruta.clone(),
                    Artefacto {
                        ruta: archivo.ruta.clone(),
                        bytes: archivo.bytes,
                        sha512: archivo.sha512.clone(),
                        procedencia,
                    },
                )
                .is_some()
            {
                return Err(Error::Datos("archivo duplicado en inventario"));
            }
        }
        let mut elecciones = BTreeMap::new();
        for ruta in actuales.keys().chain(
            base.into_iter()
                .flat_map(|m| m.archivos.iter().map(|a| &a.ruta)),
        ) {
            let anterior = base.is_some_and(|m| m.archivos.iter().any(|a| &a.ruta == ruta));
            let eleccion = if seleccion.es_privado(ruta) {
                Eleccion::Privado
            } else if !anterior {
                Eleccion::Privado
            } else if actuales.contains_key(ruta) {
                Eleccion::Incluir
            } else {
                Eleccion::Retirar
            };
            elecciones.insert(ruta.clone(), eleccion);
        }
        Ok(Self {
            base: base.cloned(),
            actuales,
            elecciones,
            autorizados: BTreeSet::new(),
            packs: base.map_or_else(Vec::new, |m| m.packs.clone()),
        })
    }
    pub fn elegir(&mut self, ruta: &RutaCompartida, eleccion: Eleccion) -> Result<(), Error> {
        if !self.elecciones.contains_key(ruta) {
            return Err(Error::Datos("archivo fuera del borrador"));
        }
        let anterior = self
            .base
            .as_ref()
            .is_some_and(|m| m.archivos.iter().any(|a| &a.ruta == ruta));
        if matches!(eleccion, Eleccion::Posponer | Eleccion::Retirar) && !anterior
            || matches!(eleccion, Eleccion::Compartir | Eleccion::Incluir)
                && !self.actuales.contains_key(ruta)
        {
            return Err(Error::Datos("elección no disponible"));
        }
        self.elecciones.insert(ruta.clone(), eleccion);
        Ok(())
    }
    pub fn archivos_publicados(&self) -> Vec<Artefacto> {
        self.elecciones
            .iter()
            .filter_map(|(ruta, eleccion)| match eleccion {
                Eleccion::Compartir | Eleccion::Incluir => self.actuales.get(ruta).cloned(),
                Eleccion::Posponer => self
                    .base
                    .as_ref()
                    .and_then(|m| m.archivos.iter().find(|a| &a.ruta == ruta))
                    .cloned(),
                Eleccion::Privado | Eleccion::Retirar => None,
            })
            .collect()
    }
    pub fn seleccionar_packs(&mut self, packs: Vec<String>) {
        self.packs = packs;
    }
    /// Solo reconocer la versión devuelta por el catálogo para el hash inventariado.
    /// El nombre del archivo o la última versión del proyecto no acreditan identidad.
    pub fn reconocer_catalogo(
        &mut self,
        ruta: &RutaCompartida,
        version: &crate::modrinth::Version,
    ) -> Result<(), Error> {
        let archivo = self
            .actuales
            .get_mut(ruta)
            .ok_or(Error::Datos("archivo fuera del borrador"))?;
        if !version
            .files
            .iter()
            .any(|f| f.hashes.sha512 == archivo.sha512 && f.size == archivo.bytes)
        {
            return Err(Error::Datos("el catálogo no corresponde al contenido"));
        }
        let valido = |id: &str| {
            !id.is_empty()
                && id.len() <= 128
                && id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        };
        if !valido(&version.id) || !valido(&version.project_id) {
            return Err(Error::Datos("origen de catálogo"));
        }
        archivo.procedencia = Procedencia::Modrinth {
            proyecto: version.project_id.clone(),
            version: version.id.clone(),
        };
        Ok(())
    }
    pub fn autorizacion_pendiente(&self) -> Vec<Artefacto> {
        self.archivos_publicados()
            .into_iter()
            .filter(|a| {
                let requiere = matches!(a.procedencia, Procedencia::Propio)
                    && (a.ruta.as_str().starts_with("mods/")
                        || a.ruta.as_str().starts_with("resourcepacks/")
                        || a.ruta.as_str().starts_with("kubejs/"));
                requiere
                    && !self.autorizados.contains(&a.sha512)
                    && !self.base.as_ref().is_some_and(|m| {
                        m.archivos
                            .iter()
                            .any(|anterior| anterior.sha512 == a.sha512)
                    })
            })
            .collect()
    }
    /// Llamar únicamente después de la confirmación expresa del anfitrión.
    pub fn autorizar_propios(&mut self) {
        for archivo in self.autorizacion_pendiente() {
            self.autorizados.insert(archivo.sha512);
        }
    }
    pub(crate) fn usa_base(&self, archivo: &Artefacto) -> bool {
        self.elecciones.get(&archivo.ruta) == Some(&Eleccion::Posponer)
    }
    pub(crate) fn manifiesto(
        &self,
        origen: super::Origen,
        perfil: super::Perfil,
        revision: u64,
    ) -> Manifiesto {
        Manifiesto {
            esquema: "icaro-compartir/1".into(),
            origen,
            perfil,
            revision,
            archivos: self.archivos_publicados(),
            packs: self.packs.clone(),
        }
    }
}
