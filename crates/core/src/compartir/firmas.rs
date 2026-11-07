// SPDX-License-Identifier: GPL-3.0-only
//! Autenticidad del manifiesto fuera del canal; el hash comprueba el contenido.
use super::{identidad::Identidad, Error, Limites, Manifiesto};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RevisionFirmada {
    pub manifiesto: String,
    pub firma: String,
}
impl RevisionFirmada {
    pub fn crear(
        identidad: &Identidad,
        manifiesto: &Manifiesto,
        limites: &Limites,
    ) -> Result<Self, Error> {
        manifiesto.validar(limites)?;
        if identidad.clave_publica() != manifiesto.origen.anfitrion {
            return Err(Error::Datos("identidad del publicador"));
        }
        let manifiesto =
            serde_json::to_string(manifiesto).map_err(|_| Error::Datos("manifiesto"))?;
        let firma = identidad.firmar(format!("icaro-revision/1\n{manifiesto}").as_bytes());
        Ok(Self { manifiesto, firma })
    }
    pub fn verificar(&self, anfitrion: &str, limites: &Limites) -> Result<Manifiesto, Error> {
        if self.manifiesto.len() > limites.manifiesto_bytes {
            return Err(Error::Limite("manifiesto firmado"));
        }
        Identidad::verificar(
            anfitrion,
            format!("icaro-revision/1\n{}", self.manifiesto).as_bytes(),
            &self.firma,
        )?;
        let manifiesto = Manifiesto::leer(self.manifiesto.as_bytes(), limites)?;
        if manifiesto.origen.anfitrion != anfitrion {
            return Err(Error::Datos("origen distinto del firmante"));
        }
        Ok(manifiesto)
    }
}
