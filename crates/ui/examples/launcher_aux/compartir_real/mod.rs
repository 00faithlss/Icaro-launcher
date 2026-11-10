// SPDX-License-Identifier: GPL-3.0-only
//! Controlador experimental del publicador; no amplía el update del launcher.
mod acciones;
mod avance;
mod biblioteca;
mod biblioteca_tareas;
mod compartidas;
mod contadores;
mod estado;
mod invitaciones;
mod invitado;
mod invitado_diseno;
mod invitado_instalacion;
mod invitado_tareas;
mod invitado_vistas;
mod listo;
mod modelos;
#[cfg(test)]
mod pruebas;
#[cfg(test)]
mod pruebas_avance;
#[cfg(test)]
mod pruebas_biblioteca;
#[cfg(test)]
mod pruebas_invitado;
#[cfg(test)]
mod pruebas_worker;
mod resultados;
mod selector;
mod servicio;
mod sincronizar;
#[cfg(test)]
mod sync_pruebas;
mod sync_tareas;
mod sync_vistas;
#[cfg(test)]
mod sync_worker;
mod tareas;
mod vistas;
pub use acciones::{actualizar, Accion, Operacion};
pub use biblioteca::cargar as cargar_biblioteca;
pub use compartidas::pagina as pagina_compartidas;
pub use estado::{Estado, Etapa};
pub use invitado_diseno::preparar as preparar_invitado_diseno;
pub use modelos::tamano;
pub use sincronizar::antes_de_jugar;
pub use vistas::{accesos, clave as clave_fase, vista};
pub use vistas::transferencias;
