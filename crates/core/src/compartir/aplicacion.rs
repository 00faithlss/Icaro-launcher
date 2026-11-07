// SPDX-License-Identifier: GPL-3.0-only
//! Preparación verificada y aplicación recuperable de una revisión completa.
mod diario;
mod ejecucion;
mod estado;
mod estructura;
mod opciones;
mod preparacion;
mod recuperacion;
mod rutas;

use super::{
    plan::{BaseAplicada, PlanCompartido},
    Artefacto, Error, Limites,
};
pub use ejecucion::aplicar;
pub use preparacion::preparar;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};
use tokio_util::sync::CancellationToken;

pub(crate) const BASE: &str = ".icaro-compartir-base.json";
pub(crate) const PREFIJO: &str = ".icaro-compartir-tx-";

#[derive(Debug, Clone)]
pub struct ContextoDestino {
    pub juego: PathBuf,
    pub limites: Limites,
}

/// Origen local ya descargado. Se consume por bloques dentro de spawn_blocking.
pub trait FuenteArchivos: Sync {
    fn abrir(&self, archivo: &Artefacto) -> Result<Box<dyn Read + Send>, Error>;
}
impl FuenteArchivos for super::revisiones::Revision {
    fn abrir(&self, archivo: &Artefacto) -> Result<Box<dyn Read + Send>, Error> {
        Ok(Box::new(
            fs::File::open(self.archivo(&archivo.ruta)?).map_err(|_| Error::Disco)?,
        ))
    }
}

/// Conserva el bloqueo hasta aplicar o descartar la preparación.
pub struct PlanPreparado {
    contexto: ContextoDestino,
    nombre: String,
    diario: diario::Diario,
    plan: PlanCompartido,
    _bloqueo: fs::File,
}

#[derive(Debug, Clone)]
pub enum EventoAplicacion {
    /// Se emite después de cada movimiento durable previsto por el diario.
    Movimiento {
        hechos: usize,
        total: usize,
    },
    Completada,
}

pub fn cargar_base(ctx: &ContextoDestino) -> Result<BaseAplicada, Error> {
    leer_base(ctx).map(|(base, _)| base)
}
fn leer_base(ctx: &ContextoDestino) -> Result<(BaseAplicada, Option<diario::Huella>), Error> {
    let path = rutas::resolver(&ctx.juego, BASE, false)?;
    let Some(bytes) = estado::leer_acotado(
        &path,
        ctx.limites.manifiesto_bytes.saturating_add(128) as u64,
    )?
    else {
        return Ok((BaseAplicada::default(), None));
    };
    let base: BaseAplicada =
        serde_json::from_slice(&bytes).map_err(|_| Error::Datos("base de sincronización"))?;
    if let Some(manifiesto) = &base.manifiesto {
        manifiesto.validar(&ctx.limites)?;
    }
    Ok((base, Some(estado::de_bytes(&bytes))))
}

/// La aplicación debe llamarlo al abrir la instancia antes de leer su contenido.
pub fn recuperar(juego: &Path) -> Result<(), Error> {
    let _bloqueo = crate::mods::guardia(juego).map_err(|_| Error::Datos("instancia ocupada"))?;
    recuperar_bajo_bloqueo(juego)
}
pub(crate) fn recuperar_bajo_bloqueo(juego: &Path) -> Result<(), Error> {
    recuperacion::recuperar(juego)
}

fn comprobar_cancelacion(cancel: &CancellationToken) -> Result<(), Error> {
    super::comprobar_cancelacion(cancel)
}
