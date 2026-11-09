// SPDX-License-Identifier: GPL-3.0-only
//! Regresiones de compartir instancias; cada módulo cubre una responsabilidad.
use icaro_core::compartir::{inventario, Limites, Manifiesto, RutaCompartida};
use sha2::{Digest, Sha512};
use std::fs;
use tokio_util::sync::CancellationToken;
#[path = "compartir/comun.rs"]
mod comun;
use comun::{manifiesto, origen_y_perfil, Carpeta};
#[path = "compartir/contratos.rs"]
mod contratos;

#[path = "compartir/inventario.rs"]
mod pruebas_inventario;

#[path = "compartir/packs.rs"]
mod packs;

#[path = "compartir/plan.rs"]
mod plan;

#[path = "compartir/revisiones.rs"]
mod revisiones;

#[path = "compartir/seleccion.rs"]
mod seleccion;

#[path = "compartir/aplicacion.rs"]
mod aplicacion;
#[path = "compartir/identidad.rs"]
mod identidad;
#[path = "compartir/recuperacion.rs"]
mod recuperacion;
