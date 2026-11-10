// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Vistas puras de compartir instancias. El launcher aporta datos comprobados
//! y procesa las acciones; ninguna vista consulta disco, red o el núcleo.

mod anfitrion;
mod compartidas;
mod comun;
mod contenido;
mod invitado;
mod listo;
mod modelos;
mod pagina;
mod publicacion;
mod seguimiento;
mod selector;

pub use anfitrion::*;
pub use compartidas::instancias_compartidas;
pub use contenido::*;
pub use invitado::*;
pub use listo::{compartido_listo, CompartidoListo, EstadoCodigo, PersonaUnida};
pub use modelos::*;
pub use pagina::{migas, pagina, Miga};
pub use publicacion::{
    elecciones_de_atajo, frase_publicacion, llevar_a_tipo, mensaje_atajo, revision_publicacion,
};
pub use seguimiento::*;
pub use selector::{selector_instancia, EstadoOpcion, OpcionInstancia, SelectorInstancia};
