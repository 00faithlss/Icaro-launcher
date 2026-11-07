// SPDX-License-Identifier: GPL-3.0-only
//! Puente entre el servicio de invitaciones y el control de acceso del transporte.
use super::ClienteInvitaciones;
use crate::compartir::{
    accesos::{Accesos, Autoridad},
    manifiesto::hexadecimal,
    Error,
};
use std::{future::Future, pin::Pin, sync::Arc};

pub struct AutoridadServicio {
    cliente: ClienteInvitaciones,
    publicacion: String,
    accesos: Arc<Accesos>,
}
impl AutoridadServicio {
    pub fn nueva(
        cliente: ClienteInvitaciones,
        publicacion: String,
        accesos: Arc<Accesos>,
    ) -> Result<Self, Error> {
        if !hexadecimal(&publicacion, 32) {
            return Err(Error::Datos("publicación"));
        }
        Ok(Self {
            cliente,
            publicacion,
            accesos,
        })
    }
}
impl Autoridad for AutoridadServicio {
    fn autorizado(
        &self,
        invitado: String,
    ) -> Pin<Box<dyn Future<Output = Result<bool, Error>> + Send + '_>> {
        Box::pin(async move {
            if !hexadecimal(&invitado, 64) {
                return Err(Error::Datos("identidad de acceso"));
            }
            // Fallar cerrado: ni el código presentado por el invitado ni una copia local
            // sustituyen la consulta firmada del anfitrión al servicio autoritativo.
            let autorizados = self.cliente.autorizados(&self.publicacion).await?;
            let remoto = autorizados.contains(&invitado);
            let accesos = self.accesos.clone();
            tokio::task::spawn_blocking(move || {
                if !remoto {
                    // No crear una revocación para alguien que todavía no ha aceptado.
                    if accesos.permitir(&invitado) {
                        accesos.revocar(&invitado)?;
                    }
                    return Ok(false);
                }
                match accesos.autorizar(&invitado) {
                    Ok(()) => Ok(accesos.permitir(&invitado)),
                    Err(Error::Datos("acceso revocado" | "publicación detenida")) => Ok(false),
                    Err(e) => Err(e),
                }
            })
            .await
            .map_err(|_| Error::Disco)?
        })
    }
}
