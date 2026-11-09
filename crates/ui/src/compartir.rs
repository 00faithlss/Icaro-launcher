// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Vistas puras de compartir instancias. El launcher aporta datos comprobados
//! y procesa las acciones; ninguna vista consulta disco, red o el núcleo.

mod anfitrion;
mod comun;
mod contenido;
mod invitado;
mod modelos;
mod publicacion;
mod seguimiento;

pub use anfitrion::*;
pub use contenido::*;
pub use invitado::*;
pub use modelos::*;
pub use publicacion::{
    elecciones_de_atajo, frase_publicacion, llevar_a_tipo, mensaje_atajo, revision_publicacion,
};
pub use seguimiento::*;
