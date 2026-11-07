// SPDX-License-Identifier: GPL-3.0-only
//! Orden compartido y selección local, sin copiar las demás opciones de Minecraft.
use super::Error;
use std::collections::BTreeSet;

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
