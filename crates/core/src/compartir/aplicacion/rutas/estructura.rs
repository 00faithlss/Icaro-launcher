// SPDX-License-Identifier: GPL-3.0-only
//! Estructura prevista: solo se retiran carpetas vacías y archivos seleccionados.
use super::super::{
    diario::{Huella, Operacion},
    estado, Error,
};
use super::{directorio, resolver};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fs, path::Path};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Estructura {
    pub creados: Vec<String>,
    pub retirados: Vec<String>,
}
impl Estructura {
    pub fn vacia(&self) -> bool {
        self.creados.is_empty() && self.retirados.is_empty()
    }
    pub fn validar(&self, ops: &[Operacion]) -> Result<(), Error> {
        let mut vistos = BTreeSet::new();
        if self.creados.len() + self.retirados.len() > 640_064 {
            return Err(Error::Datos("estructura excesiva"));
        }
        for (rutas, nuevos) in [(&self.creados, true), (&self.retirados, false)] {
            for ruta in rutas {
                // Una carpeta siempre es prefijo estricto de un destino autorizado.
                super::validar_destino(&format!("{ruta}/icaro"))?;
                if !vistos.insert(ruta.to_lowercase())
                    || !ops.iter().any(|op| {
                        if nuevos {
                            op.despues.is_some() && debajo(&op.destino, ruta)
                        } else {
                            op.despues.is_some()
                                && (op.destino == *ruta || debajo(ruta, &op.destino))
                        }
                    })
                {
                    return Err(Error::Datos("directorio ajeno al diario"));
                }
            }
        }
        Ok(())
    }
}
fn debajo(ruta: &str, padre: &str) -> bool {
    ruta.strip_prefix(padre)
        .is_some_and(|resto| resto.starts_with('/'))
}
/// Inspecciona sin atravesar archivos ni enlaces. Un padre archivo equivale a ausencia.
pub(crate) fn tipo(juego: &Path, ruta: &str) -> Result<Option<bool>, Error> {
    directorio(juego)?;
    let mut path = juego.to_path_buf();
    let partes: Vec<_> = ruta.split('/').collect();
    for (i, parte) in partes.iter().enumerate() {
        path.push(parte);
        match fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(_) => return Err(Error::Disco),
            Ok(_) => (),
        }
        let meta = crate::almacenamiento::regular(&path).map_err(|_| Error::CambioExterno)?;
        if i + 1 == partes.len() {
            return Ok(Some(meta.is_dir()));
        }
        if !meta.is_dir() {
            return Ok(None);
        }
    }
    Err(Error::CambioExterno)
}
pub(crate) fn huella_anterior(
    juego: &Path,
    op: &Operacion,
    ops: &[Operacion],
    cancel: &CancellationToken,
) -> Result<Option<Huella>, Error> {
    // Comprobar cada padre: solo una retirada gestionada puede bloquearlo.
    let partes: Vec<_> = op.destino.split('/').collect();
    for n in 1..partes.len() {
        let padre = partes[..n].join("/");
        if tipo(juego, &padre)? == Some(false) {
            let retirada = ops
                .iter()
                .find(|o| o.destino == padre && o.antes.is_some() && o.despues.is_none())
                .ok_or(Error::CambioExterno)?;
            if estado::huella(&resolver(juego, &padre, false)?, cancel)? != retirada.antes {
                return Err(Error::CambioExterno);
            }
            return Ok(None);
        }
    }
    if tipo(juego, &op.destino)? == Some(true) {
        if op.antes.is_some() || op.despues.is_none() {
            return Err(Error::CambioExterno);
        }
        let mut carpetas = Vec::new();
        inspeccionar(juego, &op.destino, ops, cancel, &mut carpetas)?;
        return Ok(None);
    }
    estado::huella(&resolver(juego, &op.destino, false)?, cancel)
}
fn inspeccionar(
    juego: &Path,
    ruta: &str,
    ops: &[Operacion],
    cancel: &CancellationToken,
    carpetas: &mut Vec<String>,
) -> Result<(), Error> {
    super::super::comprobar_cancelacion(cancel)?;
    directorio(&juego.join(ruta))?;
    for entrada in fs::read_dir(juego.join(ruta)).map_err(|_| Error::Disco)? {
        let entrada = entrada.map_err(|_| Error::Disco)?;
        let nombre = entrada
            .file_name()
            .into_string()
            .map_err(|_| Error::CambioExterno)?;
        let hijo = format!("{ruta}/{nombre}");
        super::validar_destino(&hijo)?;
        match tipo(juego, &hijo)? {
            Some(true) => inspeccionar(juego, &hijo, ops, cancel, carpetas)?,
            Some(false) => {
                let op = ops
                    .iter()
                    .find(|o| o.destino == hijo && o.antes.is_some() && o.despues.is_none())
                    .ok_or(Error::CambioExterno)?;
                if estado::huella(&resolver(juego, &hijo, false)?, cancel)? != op.antes {
                    return Err(Error::CambioExterno);
                }
            }
            None => return Err(Error::CambioExterno),
        }
    }
    carpetas.push(ruta.into());
    Ok(())
}
pub(crate) fn preparar(
    juego: &Path,
    ops: &[Operacion],
    cancel: &CancellationToken,
) -> Result<Estructura, Error> {
    let mut estructura = Estructura::default();
    for op in ops {
        if huella_anterior(juego, op, ops, cancel)? != op.antes {
            return Err(Error::CambioExterno);
        }
        if op.despues.is_some() && tipo(juego, &op.destino)? == Some(true) {
            inspeccionar(juego, &op.destino, ops, cancel, &mut estructura.retirados)?;
        }
    }
    // Solo activar el nuevo protocolo si existe un cambio de tipo real.
    let transicion = !estructura.retirados.is_empty()
        || ops.iter().any(|op| {
            op.antes.is_some()
                && op.despues.is_none()
                && ops
                    .iter()
                    .any(|n| n.despues.is_some() && debajo(&n.destino, &op.destino))
        });
    if !transicion {
        return Ok(Estructura::default());
    }
    for op in ops.iter().filter(|o| o.despues.is_some()) {
        let partes: Vec<_> = op.destino.split('/').collect();
        for n in 1..partes.len() {
            let padre = partes[..n].join("/");
            if tipo(juego, &padre)? != Some(true) {
                estructura.creados.push(padre);
            }
        }
    }
    for rutas in [&mut estructura.creados, &mut estructura.retirados] {
        rutas.sort_by_key(|r| (r.split('/').count(), r.clone()));
        rutas.dedup();
    }
    estructura.validar(ops)?;
    Ok(estructura)
}
pub(crate) fn quitar_vacio(juego: &Path, ruta: &str) -> Result<(), Error> {
    if tipo(juego, ruta)? != Some(true) {
        return Err(Error::CambioExterno);
    }
    let path = juego.join(ruta);
    if fs::read_dir(&path)
        .map_err(|_| Error::Disco)?
        .next()
        .is_some()
    {
        return Err(Error::CambioExterno);
    }
    fs::remove_dir(path).map_err(|_| Error::CambioExterno)
}
pub(crate) fn crear(juego: &Path, ruta: &str) -> Result<(), Error> {
    let path = juego.join(ruta);
    if let Some(padre) = path.parent() {
        directorio(padre)?;
    }
    super::crear_directorio(&path)
}
