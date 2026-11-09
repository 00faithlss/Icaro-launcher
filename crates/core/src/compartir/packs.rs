// SPDX-License-Identifier: GPL-3.0-only
//! Orden compartido y selección local, sin copiar las demás opciones de Minecraft.
use super::Error;
use std::collections::BTreeSet;

/// Elección revisada de selección y orden, ligada a la lectura local original.
#[derive(Debug, Clone)]
pub struct RevisionOrden {
    actual: Vec<String>,
    anterior: Vec<String>,
    nuevo: Vec<String>,
    salida: Option<Vec<String>>,
}
impl RevisionOrden {
    pub fn nueva(actual: &[String], anterior: &[String], nuevo: &[String]) -> Result<Self, Error> {
        if !unicos(actual) || !unicos(anterior) || !unicos(nuevo) {
            return Err(Error::Datos("selección de packs duplicada o inválida"));
        }
        Ok(Self {
            actual: actual.to_vec(),
            anterior: anterior.to_vec(),
            nuevo: nuevo.to_vec(),
            salida: combinar_orden(actual, anterior, nuevo).ok(),
        })
    }
    pub fn pendiente(&self) -> bool {
        self.salida.is_none()
    }
    pub fn actual(&self) -> &[String] {
        &self.actual
    }
    pub fn nuevo(&self) -> &[String] {
        &self.nuevo
    }
    pub(crate) fn corresponde(&self, anterior: &[String], nuevo: &[String]) -> bool {
        self.anterior == anterior && self.nuevo == nuevo
    }
    pub fn resolver(&mut self, resolucion: super::plan::Resolucion) -> Result<(), Error> {
        let antiguos: BTreeSet<_> = self.anterior.iter().map(|p| p.to_lowercase()).collect();
        let nuevos: BTreeSet<_> = self.nuevo.iter().map(|p| p.to_lowercase()).collect();
        let salida = match resolucion {
            super::plan::Resolucion::ConservarMia => self
                .actual
                .iter()
                .filter(|p| {
                    !antiguos.contains(&p.to_lowercase())
                        || nuevos.contains(&p.to_lowercase())
                        || p.as_str() == "vanilla"
                })
                .cloned()
                .collect(),
            super::plan::Resolucion::DelAnfitrion => {
                let primero = self
                    .actual
                    .iter()
                    .position(|p| {
                        p != "vanilla"
                            && (antiguos.contains(&p.to_lowercase())
                                || nuevos.contains(&p.to_lowercase()))
                    })
                    .unwrap_or(self.actual.len());
                let mut salida = Vec::new();
                for (i, p) in self.actual.iter().enumerate() {
                    if i == primero {
                        salida.extend(
                            self.nuevo
                                .iter()
                                .filter(|p| p.as_str() != "vanilla")
                                .cloned(),
                        );
                    }
                    if p != "vanilla"
                        && !antiguos.contains(&p.to_lowercase())
                        && !nuevos.contains(&p.to_lowercase())
                    {
                        salida.push(p.clone());
                    }
                }
                if primero == self.actual.len() {
                    salida.extend(
                        self.nuevo
                            .iter()
                            .filter(|p| p.as_str() != "vanilla")
                            .cloned(),
                    );
                }
                if self
                    .actual
                    .iter()
                    .chain(&self.nuevo)
                    .any(|p| p == "vanilla")
                {
                    salida.insert(0, "vanilla".into());
                }
                salida
            }
        };
        self.salida = Some(salida);
        Ok(())
    }
    pub fn aplicar(&self, actual: &[String]) -> Result<Vec<String>, Error> {
        if actual != self.actual {
            return Err(Error::CambioExterno);
        }
        self.salida
            .clone()
            .ok_or(Error::Datos("conflicto en la selección de resourcepacks"))
    }
}

fn unicos(packs: &[String]) -> bool {
    let mut vistos = BTreeSet::new();
    packs.len() <= 20_000
        && packs.iter().all(|p| {
            !p.is_empty()
                && p.len() <= 1024
                && !p.chars().any(char::is_control)
                && vistos.insert(p.to_lowercase())
        })
}

/// Conserva el orden relativo de lo personal e inserta un bloque compartido.
/// Un cambio local y otro remoto en la misma selección requieren revisión.
pub fn combinar_orden(
    actual: &[String],
    anterior: &[String],
    nuevo: &[String],
) -> Result<Vec<String>, Error> {
    if !unicos(actual) || !unicos(anterior) || !unicos(nuevo) {
        return Err(Error::Datos("selección de packs duplicada o inválida"));
    }
    if anterior == nuevo {
        return Ok(actual.to_vec());
    }
    // Vanilla ya existe en una instalación nueva: no es un ZIP personal que se
    // vaya a sobrescribir. Su presencia base no debe bloquear la primera revisión.
    let sin_vanilla = |packs: &[String]| {
        packs
            .iter()
            .filter(|p| p.as_str() != "vanilla")
            .cloned()
            .collect::<Vec<_>>()
    };
    let locales = sin_vanilla(actual);
    let anteriores = sin_vanilla(anterior);
    let nuevos = sin_vanilla(nuevo);
    let mut resultado = combinar_archivos(&locales, &anteriores, &nuevos)?;
    if actual.iter().chain(nuevo).any(|p| p == "vanilla") {
        resultado.insert(0, "vanilla".into());
    }
    Ok(resultado)
}
fn combinar_archivos(
    actual: &[String],
    anterior: &[String],
    nuevo: &[String],
) -> Result<Vec<String>, Error> {
    let antiguos: BTreeSet<_> = anterior.iter().map(|p| p.to_lowercase()).collect();
    let bloque_local: Vec<_> = actual
        .iter()
        .filter(|p| antiguos.contains(&p.to_lowercase()))
        .cloned()
        .collect();
    if bloque_local != anterior {
        return Err(Error::Datos("conflicto en la selección de resourcepacks"));
    }
    let personales: BTreeSet<_> = actual
        .iter()
        .filter(|p| !antiguos.contains(&p.to_lowercase()))
        .map(|p| p.to_lowercase())
        .collect();
    if nuevo.iter().any(|p| personales.contains(&p.to_lowercase())) {
        return Err(Error::Datos(
            "pack compartido coincide con un pack personal",
        ));
    }
    let primero = actual
        .iter()
        .position(|p| antiguos.contains(&p.to_lowercase()))
        .unwrap_or(actual.len());
    let mut resultado = Vec::with_capacity(personales.len() + nuevo.len());
    let mut insertado = false;
    for (i, pack) in actual.iter().enumerate() {
        if i == primero {
            resultado.extend_from_slice(nuevo);
            insertado = true;
        }
        if !antiguos.contains(&pack.to_lowercase()) {
            resultado.push(pack.clone());
        }
    }
    if !insertado {
        resultado.extend_from_slice(nuevo);
    }
    Ok(resultado)
}
