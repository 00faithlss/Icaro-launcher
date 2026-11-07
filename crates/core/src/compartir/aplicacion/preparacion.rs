// SPDX-License-Identifier: GPL-3.0-only
use super::{
    comprobar_cancelacion,
    diario::{Diario, Huella, Operacion},
    estado, leer_base, opciones, rutas, ContextoDestino, Error, FuenteArchivos, PlanPreparado,
    BASE, PREFIJO,
};
use crate::compartir::{
    plan::{BaseAplicada, PlanCompartido},
    Artefacto,
};
use std::{collections::BTreeSet, fs, io::Cursor};
use tokio_util::sync::CancellationToken;

enum Contenido {
    Archivo(Artefacto),
    Bytes(Vec<u8>),
    Ausente,
}

pub fn preparar(
    ctx: &ContextoDestino,
    plan: PlanCompartido,
    fuente: &impl FuenteArchivos,
    cancel: &CancellationToken,
) -> Result<PlanPreparado, Error> {
    comprobar_cancelacion(cancel)?;
    if !plan.puede_aplicar() {
        return Err(Error::Datos("hay conflictos sin resolver"));
    }
    plan.revision.validar(&ctx.limites)?;
    rutas::directorio(&ctx.juego)?;
    let bloqueo =
        crate::mods::guardia(&ctx.juego).map_err(|_| Error::Datos("instancia ocupada"))?;
    crate::mods::recuperar(&ctx.juego).map_err(|e| {
        Error::Datos(match e {
            crate::mods::Error::Disco => "no se pudo recuperar la instancia",
            _ => "recuperación pendiente de la instancia",
        })
    })?;
    let (aplicada, huella_base) = leer_base(ctx)?;
    if aplicada != plan.base {
        return Err(Error::CambioExterno);
    }
    let mut operaciones = Vec::new();
    let mut contenidos = Vec::new();
    let mut vistos = BTreeSet::new();
    for cambio in &plan.cambios {
        let destino = cambio.ruta.as_str();
        if !vistos.insert(destino.to_lowercase()) {
            return Err(Error::Datos("cambio duplicado"));
        }
        if let Some(archivo) = &cambio.despues {
            if archivo.ruta != cambio.ruta || !plan.revision.archivos.contains(archivo) {
                return Err(Error::Datos("archivo ajeno a la revisión"));
            }
        } else if plan.revision.archivos.iter().any(|a| a.ruta == cambio.ruta) {
            return Err(Error::Datos("retirada contraria a la revisión"));
        }
        let antes = match (&cambio.antes, cambio.antes_bytes) {
            (Some(hash), Some(bytes)) => Some(Huella {
                sha512: hash.clone(),
                bytes,
            }),
            (None, None) => None,
            _ => return Err(Error::Datos("estado anterior del cambio")),
        };
        let despues = cambio.despues.as_ref().map(|a| Huella {
            bytes: a.bytes,
            sha512: a.sha512.clone(),
        });
        if antes == despues {
            continue;
        }
        operaciones.push(Operacion {
            destino: destino.into(),
            antes,
            despues,
        });
        contenidos.push(
            cambio
                .despues
                .clone()
                .map_or(Contenido::Ausente, Contenido::Archivo),
        );
    }
    let anteriores = plan
        .base
        .manifiesto
        .as_ref()
        .map_or(&[][..], |m| m.packs.as_slice());
    if let Some((antes, bytes)) = opciones::preparar(&ctx.juego, anteriores, &plan.revision.packs)?
    {
        agregar_bytes(
            "options.txt",
            bytes,
            antes,
            &mut operaciones,
            &mut contenidos,
        );
    }
    let base = BaseAplicada {
        manifiesto: Some(plan.revision.clone()),
    };
    let bytes = serde_json::to_vec(&base).map_err(|_| Error::Datos("base aplicada"))?;
    if bytes.len() > ctx.limites.manifiesto_bytes.saturating_add(128) {
        return Err(Error::Limite("base aplicada"));
    }
    agregar_bytes(BASE, bytes, huella_base, &mut operaciones, &mut contenidos);
    let estructura = rutas::estructura::preparar(&ctx.juego, &operaciones, cancel)?;
    comprobar_cancelacion(cancel)?;
    let mut nonce = [0u8; 16];
    getrandom::fill(&mut nonce).map_err(|_| Error::Disco)?;
    let id: String = nonce.iter().map(|b| format!("{b:02x}")).collect();
    let nombre = format!("{PREFIJO}{id}");
    let tx = ctx.juego.join(&nombre);
    fs::create_dir(&tx).map_err(|_| Error::Disco)?;
    rutas::crear_directorio(&tx.join("nuevos"))?;
    rutas::crear_directorio(&tx.join("anteriores"))?;
    let diario = Diario {
        esquema: 1,
        confirmada: false,
        operaciones,
        estructura,
    };
    diario.guardar(&tx)?;
    for (i, contenido) in contenidos.into_iter().enumerate() {
        comprobar_cancelacion(cancel)?;
        let Some(esperada) = &diario.operaciones[i].despues else {
            continue;
        };
        let path = rutas::slot(&tx, "nuevos", i)?;
        let entrada = match contenido {
            Contenido::Archivo(archivo) => fuente.abrir(&archivo)?,
            Contenido::Bytes(bytes) => Box::new(Cursor::new(bytes)),
            Contenido::Ausente => return Err(Error::Datos("contenido preparado ausente")),
        };
        estado::copiar(entrada, &path, esperada, cancel)?;
    }
    comprobar_cancelacion(cancel)?;
    Ok(PlanPreparado {
        contexto: ctx.clone(),
        nombre,
        diario,
        plan,
        _bloqueo: bloqueo,
    })
}

fn agregar_bytes(
    destino: &str,
    bytes: Vec<u8>,
    antes: Option<Huella>,
    operaciones: &mut Vec<Operacion>,
    contenidos: &mut Vec<Contenido>,
) {
    let despues = estado::de_bytes(&bytes);
    if antes.as_ref() == Some(&despues) {
        return;
    }
    operaciones.push(Operacion {
        destino: destino.into(),
        antes,
        despues: Some(despues),
    });
    contenidos.push(Contenido::Bytes(bytes));
}
