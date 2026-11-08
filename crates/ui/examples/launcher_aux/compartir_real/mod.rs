// SPDX-License-Identifier: GPL-3.0-only
//! Controlador experimental del publicador; no amplía el update del launcher.
mod acciones;
mod estado;
mod invitaciones;
mod invitado;
mod invitado_diseno;
mod invitado_instalacion;
mod invitado_tareas;
mod invitado_vistas;
mod modelos;
#[cfg(test)]
mod pruebas;
#[cfg(test)]
mod pruebas_invitado;
#[cfg(test)]
mod pruebas_worker;
mod resultados;
mod servicio;
mod tareas;
mod vistas;
pub use acciones::{actualizar, Accion, Operacion};
pub use estado::{Estado, Etapa};
pub use invitado_diseno::preparar as preparar_invitado_diseno;
pub use vistas::{accesos, vista};
