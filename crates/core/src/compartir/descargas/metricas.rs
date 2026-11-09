// SPDX-License-Identifier: GPL-3.0-only
use serde::Serialize;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
macro_rules! contadores {
    ($($campo:ident),* $(,)?) => {
        #[derive(Default)]
        pub(super) struct Contadores { $(pub(super) $campo: AtomicU64,)* }
        #[derive(Debug, Clone, Default, Serialize)]
        pub struct EstadisticasDescarga { $(pub $campo: u64,)* }
        impl MetricasDescarga {
            pub fn lectura(&self) -> EstadisticasDescarga {
                EstadisticasDescarga { $($campo: self.0.$campo.load(Ordering::Relaxed),)* }
            }
        }
    }
}
contadores!(
    archivos,
    objetos,
    duplicados_evitados,
    aciertos_cache,
    fallos_cache,
    bytes_cache_reutilizados,
    payload_red_verificado,
    descargas_propias,
    descargas_cdn,
    lotes_catalogo,
    pico_propias,
    pico_cdn,
    activas_propias,
    activas_cdn
);
#[derive(Clone, Default)]
pub struct MetricasDescarga(pub(super) Arc<Contadores>);
pub(super) struct EnCurso {
    metricas: MetricasDescarga,
    propio: bool,
}
impl MetricasDescarga {
    pub(super) fn iniciar(&self, propio: bool) -> EnCurso {
        let (actual, pico) = if propio {
            (&self.0.activas_propias, &self.0.pico_propias)
        } else {
            (&self.0.activas_cdn, &self.0.pico_cdn)
        };
        pico.fetch_max(
            actual.fetch_add(1, Ordering::Relaxed) + 1,
            Ordering::Relaxed,
        );
        EnCurso {
            metricas: self.clone(),
            propio,
        }
    }
    pub(super) fn registrar(&self) {
        use crate::{diagnostico::Campo, diagnostico_contexto as d};
        let e = self.lectura();
        for (nombre, valor) in [
            ("cache_aciertos", e.aciertos_cache),
            ("cache_fallos", e.fallos_cache),
            ("bytes_cache", e.bytes_cache_reutilizados),
            ("payload_red_verificado", e.payload_red_verificado),
            ("duplicados_evitados", e.duplicados_evitados),
            ("lotes_catalogo", e.lotes_catalogo),
            ("pico_propias", e.pico_propias),
            ("pico_cdn", e.pico_cdn),
        ] {
            d::dato(Campo::Numero(nombre, valor));
        }
    }
}
impl Drop for EnCurso {
    fn drop(&mut self) {
        let contador = if self.propio {
            &self.metricas.0.activas_propias
        } else {
            &self.metricas.0.activas_cdn
        };
        contador.fetch_sub(1, Ordering::Relaxed);
    }
}
