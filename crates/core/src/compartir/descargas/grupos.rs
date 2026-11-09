// SPDX-License-Identifier: GPL-3.0-only
use crate::{
    compartir::{Artefacto, Error, Procedencia},
    modrinth,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};
#[derive(Clone)]
pub(super) struct Grupo {
    pub archivo: Artefacto,
    pub destinos: Vec<Artefacto>,
    pub confirmar: Vec<Procedencia>,
    pub cache: Option<PathBuf>,
    pub catalogo: Option<modrinth::File>,
}
impl Grupo {
    pub fn propio(&self) -> bool {
        self.destinos
            .iter()
            .all(|a| matches!(a.procedencia, Procedencia::Propio))
    }
}
pub(super) fn agrupar(archivos: Vec<Artefacto>) -> Result<Vec<Grupo>, Error> {
    let mut grupos: BTreeMap<String, Grupo> = BTreeMap::new();
    for a in archivos {
        let g = grupos.entry(a.sha512.clone()).or_insert_with(|| Grupo {
            archivo: a.clone(),
            destinos: vec![],
            confirmar: vec![],
            cache: None,
            catalogo: None,
        });
        if g.archivo.bytes != a.bytes {
            return Err(Error::Datos(
                "tamaño contradictorio para el mismo contenido",
            ));
        }
        g.destinos.push(a);
    }
    Ok(grupos.into_values().collect())
}
pub(super) fn versiones_necesarias(grupos: &[Grupo]) -> Vec<String> {
    grupos
        .iter()
        .flat_map(|g| &g.confirmar)
        .filter_map(|p| match p {
            Procedencia::Modrinth { version, .. } => Some(version.clone()),
            _ => None,
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
pub(super) fn validar_catalogo(
    grupos: &mut [Grupo],
    versiones: &BTreeMap<String, modrinth::Version>,
) -> Result<(), Error> {
    for g in grupos {
        for p in &g.confirmar {
            if let Procedencia::Modrinth { proyecto, version } = p {
                let v = versiones
                    .get(version)
                    .filter(|v| v.id == *version && v.project_id == *proyecto)
                    .ok_or(Error::Datos("versión de catálogo distinta"))?;
                let f = v
                    .files
                    .iter()
                    .find(|f| f.hashes.sha512 == g.archivo.sha512 && f.size == g.archivo.bytes)
                    .ok_or(Error::Datos("archivo de catálogo distinto"))?;
                if g.catalogo.is_none() {
                    g.catalogo = Some(f.clone());
                }
            }
        }
    }
    Ok(())
}
