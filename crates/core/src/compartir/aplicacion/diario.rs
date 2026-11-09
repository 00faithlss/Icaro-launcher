// SPDX-License-Identifier: GPL-3.0-only
use super::{estado, rutas, Error};
use crate::compartir::manifiesto::hexadecimal;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, path::Path};

const MAX_DIARIO: u64 = 32 * 1024 * 1024;
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Huella {
    pub bytes: u64,
    pub sha512: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Operacion {
    pub destino: String,
    pub antes: Option<Huella>,
    pub despues: Option<Huella>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Diario {
    pub esquema: u32,
    pub confirmada: bool,
    pub operaciones: Vec<Operacion>,
    #[serde(default, skip_serializing_if = "rutas::estructura::Estructura::vacia")]
    pub estructura: rutas::estructura::Estructura,
}
impl Diario {
    fn validar(&self) -> Result<(), Error> {
        if self.esquema != 1 || self.operaciones.len() > 20_002 {
            return Err(Error::Datos("diario de sincronización"));
        }
        self.estructura.validar(&self.operaciones)?;
        let mut vistos = BTreeSet::new();
        for op in &self.operaciones {
            rutas::validar_destino(&op.destino)?;
            if !vistos.insert(op.destino.to_lowercase()) || op.antes == op.despues {
                return Err(Error::Datos("operación duplicada o vacía"));
            }
            for hash in op.antes.iter().chain(op.despues.iter()) {
                if !hexadecimal(&hash.sha512, 128) {
                    return Err(Error::Datos("hash del diario"));
                }
            }
        }
        Ok(())
    }
    pub fn guardar(&self, tx: &Path) -> Result<(), Error> {
        self.validar()?;
        rutas::directorio(tx)?;
        let valor = serde_json::to_value(self).map_err(|_| Error::Datos("diario"))?;
        if serde_json::to_vec_pretty(&valor)
            .map_err(|_| Error::Datos("diario"))?
            .len() as u64
            > MAX_DIARIO
        {
            return Err(Error::Limite("diario"));
        }
        crate::almacenamiento::escribir_json(&tx.join("diario.json"), &valor)
            .map_err(|_| Error::Disco)
    }
    pub fn cargar(tx: &Path) -> Result<Self, Error> {
        let bytes = estado::leer_acotado(&tx.join("diario.json"), MAX_DIARIO)?
            .ok_or(Error::Datos("diario ausente"))?;
        let diario: Self = serde_json::from_slice(&bytes).map_err(|_| Error::Datos("diario"))?;
        diario.validar()?;
        Ok(diario)
    }
}
