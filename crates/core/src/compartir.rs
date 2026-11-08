// SPDX-License-Identifier: GPL-3.0-only
//! Publicaciones seleccionadas de una instancia, independientes del protocolo HTTP antiguo.
pub mod accesos;
pub mod anfitrion;
pub mod aplicacion;
pub mod confianza;
pub mod descargas;
pub mod firmas;
pub mod identidad;
pub mod inventario;
pub mod invitaciones;
mod manifiesto;
pub mod packs;
pub mod plan;
pub mod protocolo;
pub mod publicacion;
pub mod revisiones;
pub mod transporte;
pub mod vinculos;

pub use manifiesto::{Artefacto, Manifiesto, Origen, Perfil, Procedencia, RutaCompartida};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Los datos de la publicación no son válidos: {0}.")]
    Datos(&'static str),
    #[error("La publicación supera el límite de {0}.")]
    Limite(&'static str),
    #[error("No se pudo leer o guardar el contenido compartido.")]
    Disco,
    #[error("La operación fue cancelada.")]
    Cancelada,
    #[error("El contenido cambió durante la revisión. Vuelve a revisarlo.")]
    CambioExterno,
    #[error("La recuperación encontró archivos modificados fuera de Ícaro. Se conservaron los archivos y respaldos para revisarlos.")]
    Recuperacion,
    #[error("No se pudo acceder a la identidad segura del equipo. No se guardará una clave sin protección.")]
    Credencial,
    #[error("No se pudo conectar con el anfitrión o el servicio de invitaciones.")]
    Conexion,
    #[error("El servicio de invitaciones respondió {estado}: {codigo}.")]
    Servicio {
        estado: u16,
        codigo: String,
        espera_segundos: Option<u64>,
    },
}

/// Límites del nuevo protocolo; no modifican los del índice HTTP existente.
#[derive(Debug, Clone)]
pub struct Limites {
    pub manifiesto_bytes: usize,
    pub archivos: usize,
    pub archivo_bytes: u64,
    pub revision_bytes: u64,
}
impl Default for Limites {
    fn default() -> Self {
        Self {
            manifiesto_bytes: 4 * 1024 * 1024,
            archivos: 20_000,
            archivo_bytes: 512 * 1024 * 1024,
            revision_bytes: 4 * 1024 * 1024 * 1024,
        }
    }
}

pub(crate) fn comprobar_cancelacion(
    cancel: &tokio_util::sync::CancellationToken,
) -> Result<(), Error> {
    if cancel.is_cancelled() {
        Err(Error::Cancelada)
    } else {
        Ok(())
    }
}
