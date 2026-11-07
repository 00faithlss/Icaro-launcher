// SPDX-License-Identifier: GPL-3.0-only
//! Comparación de tres vías: todavía no aplica archivos ni modifica una instancia.
use super::{
    inventario::{ArchivoInventario, Inventario},
    Artefacto, Error, Limites, Manifiesto, RutaCompartida,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BaseAplicada {
    pub manifiesto: Option<Manifiesto>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolucion {
    DelAnfitrion,
    ConservarMia,
}

#[derive(Debug, Clone)]
pub struct CambioCompartido {
    pub ruta: RutaCompartida,
    pub antes: Option<String>,
    pub antes_bytes: Option<u64>,
    pub despues: Option<Artefacto>,
}
#[derive(Debug, Clone)]
pub struct Conflicto {
    pub ruta: RutaCompartida,
    pub anterior: Option<Artefacto>,
    pub local: Option<ArchivoInventario>,
    pub remoto: Option<Artefacto>,
}
#[derive(Debug, Clone)]
pub struct PlanCompartido {
    pub(crate) base: BaseAplicada,
    pub revision: Manifiesto,
    pub cambios: Vec<CambioCompartido>,
    pub conservados: Vec<RutaCompartida>,
    pub conflictos: Vec<Conflicto>,
}
impl PlanCompartido {
    pub fn puede_aplicar(&self) -> bool {
        self.conflictos.is_empty()
    }
    pub fn resolver(&mut self, ruta: &RutaCompartida, resolucion: Resolucion) -> Result<(), Error> {
        let indice = self
            .conflictos
            .iter()
            .position(|c| &c.ruta == ruta)
            .ok_or(Error::Datos("conflicto no pendiente"))?;
        let conflicto = self.conflictos.remove(indice);
        match resolucion {
            Resolucion::ConservarMia => self.conservados.push(conflicto.ruta),
            Resolucion::DelAnfitrion => self.cambios.push(CambioCompartido {
                ruta: conflicto.ruta,
                antes: conflicto.local.as_ref().map(|a| a.sha512.clone()),
                antes_bytes: conflicto.local.as_ref().map(|a| a.bytes),
                despues: conflicto.remoto,
            }),
        }
        self.cambios.sort_by(|a, b| a.ruta.cmp(&b.ruta));
        self.conservados.sort();
        Ok(())
    }
}

pub fn planificar(
    remoto: &Manifiesto,
    aplicada: &BaseAplicada,
    inventario: &Inventario,
) -> Result<PlanCompartido, Error> {
    remoto.validar(&Limites::default())?;
    if let Some(base) = &aplicada.manifiesto {
        base.validar(&Limites::default())?;
        if base.origen != remoto.origen || base.perfil != remoto.perfil {
            return Err(Error::Datos("origen o perfil distintos del vínculo"));
        }
        if remoto.revision < base.revision || (remoto.revision == base.revision && remoto != base) {
            return Err(Error::Datos("revisión anterior o modificada sin publicar"));
        }
    }
    let anteriores: BTreeMap<_, _> = aplicada
        .manifiesto
        .iter()
        .flat_map(|m| &m.archivos)
        .map(|a| (a.ruta.clone(), a))
        .collect();
    let siguientes: BTreeMap<_, _> = remoto
        .archivos
        .iter()
        .map(|a| (a.ruta.clone(), a))
        .collect();
    let locales: BTreeMap<_, _> = inventario
        .archivos
        .iter()
        .map(|a| (a.ruta.clone(), a))
        .collect();
    if locales.len() != inventario.archivos.len() {
        return Err(Error::Datos("inventario duplicado"));
    }
    let rutas: BTreeSet<_> = anteriores
        .keys()
        .chain(siguientes.keys())
        .chain(locales.keys())
        .cloned()
        .collect();
    let mut claves = BTreeSet::new();
    if rutas.iter().any(|r| !claves.insert(r.clave())) {
        return Err(Error::Datos("colisión de nombres entre equipos"));
    }
    let mut plan = PlanCompartido {
        base: aplicada.clone(),
        revision: remoto.clone(),
        cambios: vec![],
        conservados: vec![],
        conflictos: vec![],
    };
    for ruta in rutas {
        let anterior = anteriores.get(&ruta).copied();
        let siguiente = siguientes.get(&ruta).copied();
        let local = locales.get(&ruta).copied();
        let hash_antes = anterior.map(|a| (a.sha512.as_str(), a.bytes));
        let hash_remoto = siguiente.map(|a| (a.sha512.as_str(), a.bytes));
        let hash_local = local.map(|a| (a.sha512.as_str(), a.bytes));
        if hash_local == hash_remoto {
            continue;
        } else if hash_antes == hash_remoto {
            plan.conservados.push(ruta);
        } else if hash_local == hash_antes {
            plan.cambios.push(CambioCompartido {
                ruta,
                antes: local.map(|a| a.sha512.clone()),
                antes_bytes: local.map(|a| a.bytes),
                despues: siguiente.cloned(),
            });
        } else {
            plan.conflictos.push(Conflicto {
                ruta,
                anterior: anterior.cloned(),
                local: local.cloned(),
                remoto: siguiente.cloned(),
            });
        }
    }
    Ok(plan)
}
