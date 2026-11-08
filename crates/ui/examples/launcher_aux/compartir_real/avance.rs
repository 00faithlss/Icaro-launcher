// SPDX-License-Identifier: GPL-3.0-only
//! Contadores reales de la etapa actual, sin porcentajes globales estimados.
use super::{Estado, Etapa};
use icaro_ui::compartir as v;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct Avance {
    pub fase: v::Fase,
    pub hechos: u64,
    pub total: u64,
    pub archivo: String,
    pub detalle: String,
}
pub type Emitir = Arc<dyn Fn(Avance) + Send + Sync>;
impl Default for Avance {
    fn default() -> Self {
        Self::nuevo(v::Fase::Preparacion, 0, 0, "", "Preparando la instalación")
    }
}
impl Avance {
    pub fn nuevo(fase: v::Fase, hechos: u64, total: u64, archivo: &str, detalle: &str) -> Self {
        Self {
            fase,
            hechos,
            total,
            archivo: archivo.into(),
            detalle: detalle.into(),
        }
    }
    pub fn porcentaje(&self) -> f32 {
        if self.total == 0 {
            0.
        } else {
            (self.hechos.min(self.total) as f64 / self.total as f64 * 100.) as f32
        }
    }
    pub fn descripcion(&self) -> String {
        if self.total == 0 {
            self.detalle.clone()
        } else {
            format!(
                "{} · {} de {} en esta etapa",
                self.detalle, self.hechos, self.total
            )
        }
    }
}
pub fn recibido(s: &mut Estado, id: u64, avance: Avance) {
    if s.vigente(id) && s.etapa == Etapa::InstalandoInvitado && !s.cerrar_al_terminar {
        s.invitado.avance = avance;
    }
}

/// `watch` conserva solo el avance más reciente aunque el disco emita muchos eventos.
pub fn tarea<F, Fut>(id: u64, trabajo: F) -> iced::Task<crate::Mensaje>
where
    F: FnOnce(Emitir) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = super::invitado_instalacion::Resultado> + Send + 'static,
{
    use iced::futures::SinkExt;
    iced::Task::run(
        iced::stream::channel(16, move |mut salida| async move {
            let (tx, mut rx) = tokio::sync::watch::channel(Avance::default());
            let emitir: Emitir = Arc::new(move |avance| {
                tx.send_replace(avance);
            });
            let operacion = trabajo(emitir);
            tokio::pin!(operacion);
            let mut abierto = true;
            loop {
                tokio::select! {
                    r=&mut operacion=> {
                        let _=salida.send(super::Accion::Instalada(id,r)).await;
                        break;
                    }
                    r=rx.changed(),if abierto=> {
                        if r.is_err() {abierto=false;continue;}
                        let avance=rx.borrow_and_update().clone();
                        let _=salida.send(super::Accion::Avance(id,avance)).await;
                    }
                }
            }
        }),
        crate::Mensaje::Compartir,
    )
}
