// SPDX-License-Identifier: GPL-3.0-only
use super::{
    comprobar_cancelacion, estado, recuperacion, rutas, Error, EventoAplicacion, PlanPreparado,
};
use crate::compartir::plan::BaseAplicada;
use std::fs;
use tokio_util::sync::CancellationToken;

pub fn aplicar(
    mut preparada: PlanPreparado,
    cancel: &CancellationToken,
    progreso: impl Fn(EventoAplicacion),
) -> Result<BaseAplicada, Error> {
    comprobar_cancelacion(cancel)?;
    let juego = &preparada.contexto.juego;
    let tx = rutas::transaccion(juego, &preparada.nombre)?;
    // Se verifican los estados anteriores antes del primer movimiento.
    for op in &preparada.diario.operaciones {
        comprobar_cancelacion(cancel)?;
        if rutas::estructura::huella_anterior(juego, op, &preparada.diario.operaciones, cancel)?
            != op.antes
        {
            return Err(Error::CambioExterno);
        }
    }
    let total = preparada
        .diario
        .operaciones
        .iter()
        .map(|op| usize::from(op.antes.is_some()) + usize::from(op.despues.is_some()))
        .sum();
    let mut hechos = 0;
    let resultado = (|| {
        if !preparada.diario.estructura.vacia() {
            super::estructura::ejecutar(juego, &tx, &preparada.diario, &progreso, total)?;
            preparada.diario.confirmada = true;
            return preparada.diario.guardar(&tx);
        }
        for (i, op) in preparada.diario.operaciones.iter().enumerate() {
            let destino = rutas::resolver(juego, &op.destino, true)?;
            // La fase final termina o se revierte; cancelar no interrumpe sus movimientos.
            if estado::huella(&destino, &CancellationToken::new())? != op.antes {
                return Err(Error::CambioExterno);
            }
            if op.antes.is_some() {
                fs::rename(&destino, rutas::slot(&tx, "anteriores", i)?)
                    .map_err(|e| crate::compartir::diagnostico::disco(e))?;
                hechos += 1;
                progreso(EventoAplicacion::Movimiento { hechos, total });
            }
            if op.despues.is_some() {
                if estado::huella(&rutas::slot(&tx, "nuevos", i)?, &CancellationToken::new())?
                    != op.despues
                {
                    return Err(Error::CambioExterno);
                }
                if fs::symlink_metadata(&destino).is_ok() {
                    return Err(Error::CambioExterno);
                }
                fs::rename(rutas::slot(&tx, "nuevos", i)?, &destino).map_err(|e| crate::compartir::diagnostico::disco(e))?;
                hechos += 1;
                progreso(EventoAplicacion::Movimiento { hechos, total });
            }
        }
        preparada.diario.confirmada = true;
        preparada.diario.guardar(&tx)
    })();
    if let Err(error) = resultado {
        return match recuperacion::deshacer(juego, &preparada.nombre, &preparada.diario) {
            Ok(()) => Err(error),
            Err(_) => Err(Error::Recuperacion),
        };
    }
    // Una limpieza diferida no convierte una revisión confirmada en un fallo de aplicación.
    let _ = recuperacion::conservar_respaldo(juego, &preparada.nombre);
    progreso(EventoAplicacion::Completada);
    Ok(BaseAplicada {
        manifiesto: Some(preparada.plan.revision.clone()),
    })
}
