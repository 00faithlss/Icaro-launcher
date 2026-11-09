// SPDX-License-Identifier: GPL-3.0-only
//! Muestreo acotado, independiente de las operaciones del panel.
use super::{Accion, Estado, Etapa};
use crate::Mensaje;
use icaro_core::compartir::invitaciones::{ClienteInvitaciones, Codigo};
use icaro_ui::compartir as v;
use iced::Task;

#[derive(Clone)]
pub struct Contadores {
    pub transferencias: usize,
    pub accesos: Result<Vec<String>, String>,
    pub invitacion: Option<Result<(u32, u32), String>>,
}
pub fn pedir(s: &mut Estado) -> Option<Task<Mensaje>> {
    if s.etapa != Etapa::Panel
        || s.panel
            .as_ref()
            .is_some_and(|p| matches!(p.estado, v::EstadoPublicacion::Detenida))
    {
        return None;
    }
    let h = s.anfitrion.clone()?;
    let identidad = s.publicacion.as_ref().map(|p| p.identidad.clone());
    let codigo = s
        .panel
        .as_ref()
        .and_then(|p| p.invitacion.as_ref())
        .and_then(|i| Codigo::parse(&i.codigo).ok());
    let servicio = super::servicio::Servicio::desde_entorno();
    let id = s.solicitud;
    s.biblioteca.cancel_contadores.cancel();
    s.biblioteca.cancel_contadores = tokio_util::sync::CancellationToken::new();
    let cancel_muestreo = s.biblioteca.cancel_contadores.clone();
    s.biblioteca.contadores = s.biblioteca.contadores.wrapping_add(1);
    let generacion = s.biblioteca.contadores;
    Some(Task::perform(
        async move {
            tokio::select! {
                _ = cancel_muestreo.cancelled() => Contadores { transferencias: 0, accesos: Err("Muestreo cancelado".into()), invitacion: None },
                resultado = async move {
            // Dos consultas/minuto con el panel abierto: margen para invitados
            // dentro del presupuesto gratuito de 5.000 solicitudes al día.
            tokio::time::sleep(std::time::Duration::from_secs(60)).await;
            let cancel = tokio_util::sync::CancellationToken::new();
            let resultado = icaro_core::diagnostico_contexto::observar("compartir", "contadores_publicacion", None, &cancel, async move {
                let transferencias = h.transferencias();
                let accesos = h.accesos().await.map_err(icaro_core::diagnostico_contexto::texto);
                let invitacion = if let (Some(identidad), Some(codigo), Some(servicio)) = (identidad, codigo, servicio) {
                    Some(match ClienteInvitaciones::nuevo(&servicio.url, identidad) {
                        Ok(cliente) => cliente.resolver(&codigo).await.map(|i| (i.accepted, i.capacity)).map_err(icaro_core::diagnostico_contexto::texto),
                        Err(e) => Err(icaro_core::diagnostico_contexto::texto(e)),
                    })
                } else { None };
                Ok::<_, String>(Contadores { transferencias, accesos, invitacion })
            }).await;
            resultado.unwrap_or_else(|e| Contadores { transferencias: 0, accesos: Err(e), invitacion: None })
                } => resultado,
            }
        },
        move |r| Mensaje::Compartir(Accion::Contadores(id, generacion, r)),
    ))
}
pub fn recibidos(s: &mut Estado, id: u64, generacion: u64, resultado: Contadores) {
    if !s.vigente(id) || generacion != s.biblioteca.contadores || s.etapa != Etapa::Panel {
        return;
    }
    let Some(p) = &mut s.panel else {
        return;
    };
    p.transferencias = if resultado.transferencias == 0 {
        "Sin transferencias activas".into()
    } else {
        format!("{} transferencias activas", resultado.transferencias)
    };
    if let Ok(keys) = resultado.accesos {
        p.accesos.personas = keys
            .into_iter()
            .enumerate()
            .map(|(n, k)| v::Acceso {
                nombre: format!("Equipo {}", n + 1),
                huella: format!("Clave {}", k.chars().take(12).collect::<String>()),
                id: k,
                estado: "Autorizado".into(),
                conectado: false,
                aceptado: String::new(),
            })
            .collect();
        if p.accesos
            .confirmar
            .as_ref()
            .is_some_and(|k| !p.accesos.personas.iter().any(|a| &a.id == k))
        {
            p.accesos.confirmar = None;
        }
    }
    if let (Some(Ok((aceptados, cupo))), Some(i)) = (resultado.invitacion, &mut p.invitacion) {
        i.aceptados = aceptados;
        i.cupo = cupo;
        if aceptados >= cupo {
            i.estado = v::EstadoInvitacion::Completa;
        }
    }
}
