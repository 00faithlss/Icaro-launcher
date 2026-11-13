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
/// Primeras muestras tras abrir la publicación: 5 s entre una y otra, durante 3 minutos.
pub const MUESTRAS_RAPIDAS: u32 = 36;
const ESPERA_RAPIDA: std::time::Duration = std::time::Duration::from_secs(5);
/// Después: una por minuto, dentro del presupuesto gratuito de 5.000 solicitudes al día.
const ESPERA_LENTA: std::time::Duration = std::time::Duration::from_secs(60);
/// Cuánto esperar antes de la próxima muestra de la publicación `solicitud`.
fn espera_siguiente(b: &mut super::biblioteca::Biblioteca, solicitud: u64) -> std::time::Duration {
    if b.muestras_de != solicitud {
        b.muestras_de = solicitud;
        b.muestras_rapidas = MUESTRAS_RAPIDAS;
    }
    if b.muestras_rapidas > 0 {
        b.muestras_rapidas -= 1;
        ESPERA_RAPIDA
    } else {
        ESPERA_LENTA
    }
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
    // Al abrir una publicación (o generar un código) se mira cada pocos
    // segundos durante unos minutos, para ver enseguida quién se une; después
    // baja al ritmo lento que cuida el presupuesto del servicio gratuito.
    let espera = espera_siguiente(&mut s.biblioteca, s.solicitud);
    s.biblioteca.contadores = s.biblioteca.contadores.wrapping_add(1);
    let generacion = s.biblioteca.contadores;
    Some(Task::perform(
        async move {
            tokio::select! {
                _ = cancel_muestreo.cancelled() => Contadores { transferencias: 0, accesos: Err("Muestreo cancelado".into()), invitacion: None },
                resultado = async move {
            tokio::time::sleep(espera).await;
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

#[cfg(test)]
mod pruebas_ritmo {
    use super::*;

    #[test]
    fn al_abrir_una_publicacion_se_mira_rapido_y_luego_despacio() {
        let mut b = super::super::biblioteca::Biblioteca::default();
        for _ in 0..MUESTRAS_RAPIDAS {
            assert_eq!(espera_siguiente(&mut b, 7), ESPERA_RAPIDA);
        }
        assert_eq!(espera_siguiente(&mut b, 7), ESPERA_LENTA);
        // Otra publicación abierta vuelve a empezar rápido.
        assert_eq!(espera_siguiente(&mut b, 8), ESPERA_RAPIDA);
    }
}
