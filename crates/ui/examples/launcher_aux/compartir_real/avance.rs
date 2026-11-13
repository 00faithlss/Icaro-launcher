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
/// Tiempo mínimo entre dos avances mostrados.
const INTERVALO_VISTA: std::time::Duration = std::time::Duration::from_millis(250);
pub fn recibido(s: &mut Estado, id: u64, avance: Avance) {
    if s.vigente(id)
        && matches!(s.etapa, Etapa::InstalandoInvitado | Etapa::AplicandoVinculo)
        && !s.cerrar_al_terminar
    {
        // Con cientos de archivos pequeños el nombre y el contador cambiarían
        // cada pocos milisegundos y el texto parpadea: solo se muestra una
        // muestra cada 250 ms, además de cada cambio de fase y el final.
        let ahora = std::time::Instant::now();
        let fin = avance.total > 0 && avance.hechos >= avance.total;
        let cambia = avance.fase != s.invitado.avance.fase || avance.detalle != s.invitado.avance.detalle;
        let paso = s.invitado.avance_t.is_none_or(|t| ahora.duration_since(t) >= INTERVALO_VISTA);
        if fin || cambia || paso {
            s.invitado.avance = avance;
            s.invitado.avance_t = Some(ahora);
        }
    }
}

/// `watch` conserva solo el avance más reciente aunque el disco emita muchos eventos.
pub fn tarea<F, Fut>(id: u64, trabajo: F) -> iced::Task<crate::Mensaje>
where
    F: FnOnce(Emitir) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = super::invitado_instalacion::Resultado> + Send + 'static,
{
    tarea_con_resultado(id, trabajo, super::Accion::Instalada)
}
pub fn tarea_con_resultado<F, Fut, R, C>(
    id: u64,
    trabajo: F,
    terminar: C,
) -> iced::Task<crate::Mensaje>
where
    F: FnOnce(Emitir) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = R> + Send + 'static,
    R: Send + 'static,
    C: FnOnce(u64, R) -> super::Accion + Send + 'static,
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
                        let _=salida.send(terminar(id,r)).await;
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

#[cfg(test)]
mod pruebas_vista {
    use super::*;

    fn estado() -> Estado {
        let mut s = Estado::default();
        s.solicitud = 1;
        s.etapa = Etapa::InstalandoInvitado;
        s
    }

    #[test]
    fn el_avance_mostrado_no_cambia_mas_de_cuatro_veces_por_segundo() {
        let mut s = estado();
        let a = |hechos, archivo: &str| Avance::nuevo(v::Fase::Descarga, hechos, 100, archivo, "Descargando");
        recibido(&mut s, 1, a(1, "uno.jar"));
        recibido(&mut s, 1, a(2, "dos.jar"));
        recibido(&mut s, 1, a(3, "tres.jar"));
        // Solo se mostró la primera muestra: el resto llegó dentro de 250 ms.
        assert_eq!(s.invitado.avance.archivo, "uno.jar");
        // El final y un cambio de fase pasan siempre.
        recibido(&mut s, 1, a(100, "ultimo.jar"));
        assert_eq!(s.invitado.avance.archivo, "ultimo.jar");
        recibido(&mut s, 1, Avance::nuevo(v::Fase::Verificacion, 0, 5, "", "Verificando"));
        assert_eq!(s.invitado.avance.detalle, "Verificando");
    }
}
