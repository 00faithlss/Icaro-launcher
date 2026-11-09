// SPDX-License-Identifier: GPL-3.0-only
use super::{Error, Limites};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Ruta relativa portable validada también al deserializar datos remotos.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct RutaCompartida(String);

impl RutaCompartida {
    pub fn parse(ruta: &str) -> Result<Self, Error> {
        let partes: Vec<_> = ruta.split('/').collect();
        if ruta.len() > 1024
            || partes.len() < 2
            || partes.len() > 32
            || !matches!(partes[0], "mods" | "config" | "kubejs" | "resourcepacks")
            || partes.iter().any(|p| !componente_valido(p))
        {
            return Err(Error::Datos("ruta no compartible"));
        }
        if partes[0] == "mods"
            && (partes.len() != 2 || !partes[1].to_ascii_lowercase().ends_with(".jar"))
        {
            return Err(Error::Datos(
                "mod fuera de la carpeta o con extensión inválida",
            ));
        }
        Ok(Self(ruta.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub(crate) fn clave(&self) -> String {
        self.0.to_lowercase()
    }
}
impl TryFrom<String> for RutaCompartida {
    type Error = Error;
    fn try_from(value: String) -> Result<Self, Error> {
        Self::parse(&value)
    }
}
impl From<RutaCompartida> for String {
    fn from(value: RutaCompartida) -> Self {
        value.0
    }
}

fn componente_valido(parte: &str) -> bool {
    if parte.is_empty()
        || parte.len() > 240
        || parte.starts_with('.')
        || parte.ends_with(['.', ' '])
        || parte
            .chars()
            .any(|c| c.is_control() || "<>:\"\\|?*".contains(c))
    {
        return false;
    }
    let minuscula = parte.to_lowercase();
    let base = minuscula.split('.').next().unwrap_or_default();
    if matches!(base, "con" | "prn" | "aux" | "nul" | "conin$" | "conout$")
        || ((base.starts_with("com") || base.starts_with("lpt"))
            && base.chars().count() == 4
            && base
                .chars()
                .last()
                .is_some_and(|c| "123456789¹²³".contains(c)))
    {
        return false;
    }
    !matches!(
        minuscula.as_str(),
        "accounts" | "accounts.json" | "credentials.json" | "tokens.json" | "secrets.json"
    ) && ![".pem", ".key", ".p12", ".pfx"]
        .iter()
        .any(|e| minuscula.ends_with(e))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Origen {
    pub anfitrion: String,
    pub publicacion: String,
}
impl Origen {
    pub fn validar(&self) -> Result<(), Error> {
        if !hexadecimal(&self.anfitrion, 64) || !hexadecimal(&self.publicacion, 32) {
            return Err(Error::Datos("identidad de publicación"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Perfil {
    pub minecraft: String,
    pub loader: String,
    pub loader_version: Option<String>,
}
impl Perfil {
    pub fn validar(&self) -> Result<(), Error> {
        if self.minecraft.len() > 128
            || !crate::minecraft::version_minecraft_valida(&self.minecraft)
            || !crate::minecraft::loader_instalable(&self.loader)
            || (self.loader == "vanilla" && self.loader_version.is_some())
            || (self.loader != "vanilla"
                && self.loader_version.as_ref().is_none_or(|v| {
                    v.len() > 128 || !crate::minecraft::version_minecraft_valida(v)
                }))
        {
            return Err(Error::Datos("perfil de Minecraft o loader"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "tipo", rename_all = "snake_case", deny_unknown_fields)]
pub enum Procedencia {
    Propio,
    Modrinth { proyecto: String, version: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Artefacto {
    pub ruta: RutaCompartida,
    pub bytes: u64,
    pub sha512: String,
    pub procedencia: Procedencia,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifiesto {
    pub esquema: String,
    pub origen: Origen,
    pub revision: u64,
    pub perfil: Perfil,
    pub archivos: Vec<Artefacto>,
    /// Selección activa en el orden que usa Minecraft, incluidos packs integrados.
    pub packs: Vec<String>,
}

pub(crate) fn hexadecimal(s: &str, len: usize) -> bool {
    s.len() == len
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn identificador(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

impl Manifiesto {
    /// Limita el cuerpo antes de deserializar colecciones o cadenas controladas por la red.
    pub fn leer(bytes: &[u8], limites: &Limites) -> Result<Self, Error> {
        if bytes.len() > limites.manifiesto_bytes {
            return Err(Error::Limite("tamaño del manifiesto"));
        }
        let value: Self =
            serde_json::from_slice(bytes).map_err(|_| Error::Datos("formato del manifiesto"))?;
        value.validar(limites)?;
        Ok(value)
    }
    pub fn validar(&self, limites: &Limites) -> Result<(), Error> {
        if self.esquema != "icaro-compartir/1" || self.revision == 0 {
            return Err(Error::Datos("versión del manifiesto"));
        }
        self.origen.validar()?;
        self.perfil.validar()?;
        if self.archivos.len() > limites.archivos || self.packs.len() > limites.archivos {
            return Err(Error::Limite("número de archivos"));
        }
        let mut rutas = BTreeSet::new();
        let mut total = 0u64;
        for archivo in &self.archivos {
            RutaCompartida::parse(archivo.ruta.as_str())?;
            if !rutas.insert(archivo.ruta.clave()) || !hexadecimal(&archivo.sha512, 128) {
                return Err(Error::Datos("rutas duplicadas o hash"));
            }
            if let Procedencia::Modrinth { proyecto, version } = &archivo.procedencia {
                if !identificador(proyecto) || !identificador(version) {
                    return Err(Error::Datos("origen de catálogo"));
                }
            }
            if archivo.bytes > limites.archivo_bytes {
                return Err(Error::Limite("tamaño por archivo"));
            }
            total = total
                .checked_add(archivo.bytes)
                .ok_or(Error::Limite("tamaño total"))?;
            if total > limites.revision_bytes {
                return Err(Error::Limite("tamaño total"));
            }
        }
        for ruta in &rutas {
            if ruta
                .match_indices('/')
                .any(|(i, _)| rutas.contains(&ruta[..i]))
            {
                return Err(Error::Datos("un destino es archivo y carpeta"));
            }
        }
        let mut packs = BTreeSet::new();
        for pack in &self.packs {
            if pack.len() > 1024 || !packs.insert(pack.to_lowercase()) {
                return Err(Error::Datos("selección de resourcepacks"));
            }
            if let Some(archivo) = pack.strip_prefix("file/") {
                let ruta = RutaCompartida::parse(&format!("resourcepacks/{archivo}"))?;
                let prefijo = format!("{}/", ruta.as_str());
                if !self
                    .archivos
                    .iter()
                    .any(|a| a.ruta == ruta || a.ruta.as_str().starts_with(&prefijo))
                {
                    return Err(Error::Datos("resourcepack activo ausente de la revisión"));
                }
            } else if pack != "vanilla"
                && (pack.is_empty()
                    || !pack
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"_:-./".contains(&b))
                    || pack.contains(".."))
            {
                return Err(Error::Datos("identificador de pack integrado"));
            }
        }
        if serde_json::to_vec(self)
            .map_err(|_| Error::Datos("serialización"))?
            .len()
            > limites.manifiesto_bytes
        {
            return Err(Error::Limite("tamaño del manifiesto"));
        }
        Ok(())
    }
}
