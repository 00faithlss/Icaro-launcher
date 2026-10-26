// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Avisos transitorios del sistema de diseño: cinco segundos, pausa al
//! interactuar y hasta tres avisos; el cuarto retira el más antiguo.
use std::time::{Duration, Instant};

pub const DURACION: Duration = Duration::from_secs(5);
pub const SALIDA: Duration = Duration::from_millis(200);
/// Reloj de un aviso quieto: basta para detectar su vencimiento.
pub const TIC_QUIETO: Duration = Duration::from_millis(100);

pub struct Aviso<M> {
    pub texto: String,
    pub accion: Option<M>,
    pub desde: Instant,
    pub error: bool,
    pub etiqueta_accion: String,
    retirada: Option<Instant>,
    duracion: Duration,
    pausa: Option<Instant>,
}

impl<M> Aviso<M> {
    pub fn nuevo(texto: String, accion: Option<M>, desde: Instant) -> Self {
        Self {
            texto,
            accion,
            desde,
            error: false,
            etiqueta_accion: "Deshacer".into(),
            retirada: None,
            duracion: DURACION,
            pausa: None,
        }
    }

    pub fn con_duracion(mut self, duracion: Duration) -> Self {
        self.duracion = duracion;
        self
    }

    pub fn de_error(mut self) -> Self {
        self.error = true;
        self.etiqueta_accion = "Ver detalle".into();
        self
    }

    pub fn pausar(&mut self, pausado: bool, ahora: Instant) {
        if pausado { self.pausa.get_or_insert(ahora); }
        else if let Some(desde) = self.pausa.take() {
            self.desde += ahora.saturating_duration_since(desde);
        }
    }

    fn reloj(&self, ahora: Instant) -> Instant { self.pausa.unwrap_or(ahora) }

    /// Desplazamiento desde abajo durante dur-base, sin recortar el contenido.
    pub fn entrada(&self, ahora: Instant, reducir: bool) -> f32 {
        if reducir { return 0.; }
        let t = (self.reloj(ahora).saturating_duration_since(self.desde).as_secs_f32() / 0.2).clamp(0., 1.);
        20. * (1. - crate::movimiento::Curva::Salida.aplicar(t))
    }

    pub fn vencido(&self, ahora: Instant) -> bool {
        self.reloj(ahora).saturating_duration_since(self.desde) >= self.duracion
            || self
                .retirada
                .is_some_and(|inicio| ahora.saturating_duration_since(inicio) >= SALIDA)
    }

    pub fn cerrar(&mut self, ahora: Instant) {
        self.retirada.get_or_insert(ahora);
    }

    /// El fundido ocupa los últimos dur-base del tiempo activo del aviso.
    pub fn opacidad(&self, ahora: Instant, reducir: bool) -> f32 {
        if reducir {
            return 1.0;
        }
        let inicio = self.retirada.unwrap_or(self.desde + self.duracion - SALIDA);
        let reloj = if self.retirada.is_some() { ahora } else { self.reloj(ahora) };
        let t = (reloj.saturating_duration_since(inicio).as_secs_f32() / SALIDA.as_secs_f32())
            .clamp(0.0, 1.0);
        1.0 - t * t * (3.0 - 2.0 * t)
    }

    pub fn saliendo(&self, ahora: Instant) -> bool {
        self.retirada.is_some()
            || self.reloj(ahora).saturating_duration_since(self.desde) >= self.duracion - SALIDA
    }

    /// La entrada y el fundido son los únicos tramos que cambian cada cuadro.
    /// El fundido se anticipa un `TIC_QUIETO` para no empezar tarde.
    pub fn animando(&self, ahora: Instant, reducir: bool) -> bool {
        let vivo = self.reloj(ahora).saturating_duration_since(self.desde);
        !reducir
            && (self.retirada.is_some()
                || vivo < SALIDA
                || vivo + TIC_QUIETO >= self.duracion.saturating_sub(SALIDA))
    }
}

pub struct Entrada<M> { pub id: u64, pub aviso: Aviso<M> }
pub struct Cola<M> { entradas: std::collections::VecDeque<Entrada<M>>, siguiente: u64 }
impl<M> Default for Cola<M> {
    fn default() -> Self { Self { entradas: std::collections::VecDeque::new(), siguiente: 0 } }
}
impl<M> Cola<M> {
    pub fn agregar(&mut self, aviso: Aviso<M>) -> u64 {
        self.siguiente = self.siguiente.wrapping_add(1);
        if self.entradas.len() == 3 { self.entradas.pop_front(); }
        self.entradas.push_back(Entrada { id: self.siguiente, aviso });
        self.siguiente
    }
    pub fn entradas(&self) -> impl Iterator<Item = &Entrada<M>> { self.entradas.iter() }
    pub fn esta_vacia(&self) -> bool { self.entradas.is_empty() }
    pub fn animando(&self, ahora: Instant, reducir: bool) -> bool {
        self.entradas.iter().any(|e| e.aviso.animando(ahora, reducir))
    }
    pub fn actualizar(&mut self, ahora: Instant) { self.entradas.retain(|e| !e.aviso.vencido(ahora)); }
    pub fn cerrar(&mut self, id: u64, ahora: Instant) {
        if let Some(e) = self.entradas.iter_mut().find(|e| e.id == id) { e.aviso.cerrar(ahora); }
    }
    pub fn quitar(&mut self, id: u64) { self.entradas.retain(|e| e.id != id); }
    pub fn pausar(&mut self, id: u64, pausa: bool, ahora: Instant) {
        if let Some(e) = self.entradas.iter_mut().find(|e| e.id == id) { e.aviso.pausar(pausa, ahora); }
    }
    pub fn ultimo(&self) -> Option<u64> { self.entradas.back().map(|e| e.id) }
    pub fn limpiar(&mut self) { self.entradas.clear(); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_aviso_caduca_a_los_cinco_segundos_y_el_siguiente_reinicia_el_plazo() {
        let inicio = Instant::now();
        let mut aviso = Aviso::<()>::nuevo("Primero".into(), None, inicio);
        assert!(!aviso.vencido(inicio + Duration::from_millis(4999)));
        assert!(aviso.vencido(inicio + DURACION));
        aviso = Aviso::nuevo("Segundo".into(), None, inicio + Duration::from_secs(2));
        assert!(!aviso.vencido(inicio + Duration::from_secs(3)));
        assert!(aviso.vencido(inicio + Duration::from_secs(7)));
        assert_eq!(aviso.texto, "Segundo");
    }

    #[test]
    fn el_hover_o_foco_no_consume_el_plazo_y_la_cola_solo_conserva_tres() {
        let inicio = Instant::now();
        let mut c = Cola::default();
        for i in 0..4 { c.agregar(Aviso::<()>::nuevo(i.to_string(), None, inicio)); }
        assert_eq!(c.entradas().map(|e| e.aviso.texto.as_str()).collect::<Vec<_>>(), ["1", "2", "3"]);
        let id = c.ultimo().unwrap();
        c.pausar(id, true, inicio + Duration::from_secs(2));
        c.actualizar(inicio + Duration::from_secs(9));
        assert_eq!(c.entradas().count(), 1);
        c.pausar(id, false, inicio + Duration::from_secs(9));
        c.actualizar(inicio + Duration::from_secs(11));
        assert!(!c.esta_vacia());
        c.actualizar(inicio + Duration::from_secs(12));
        assert!(c.esta_vacia());
    }

    #[test]
    fn la_salida_es_gradual_y_cerrar_no_reinicia_el_fundido() {
        let inicio = Instant::now();
        let mut aviso = Aviso::<()>::nuevo("Aviso".into(), None, inicio);
        assert_eq!(aviso.opacidad(inicio + DURACION - SALIDA, false), 1.0);
        assert!((aviso.opacidad(inicio + DURACION - SALIDA / 2, false) - 0.5).abs() < 0.001);
        assert_eq!(aviso.opacidad(inicio + DURACION, false), 0.0);
        assert_eq!(aviso.opacidad(inicio + DURACION - SALIDA / 2, true), 1.0);
        aviso.cerrar(inicio);
        aviso.cerrar(inicio + SALIDA / 2);
        assert!((aviso.opacidad(inicio + SALIDA / 2, false) - 0.5).abs() < 0.001);
        assert!(aviso.vencido(inicio + SALIDA));
        let siguiente = Aviso::<()>::nuevo("Siguiente".into(), None, inicio + SALIDA);
        assert_eq!(siguiente.opacidad(inicio + SALIDA, false), 1.0);
    }
    #[test]
    fn solo_la_entrada_y_la_salida_piden_cuadros() {
        let inicio = Instant::now();
        let mut c = Cola::default();
        c.agregar(Aviso::<()>::nuevo("Aviso".into(), None, inicio));
        assert!(c.animando(inicio + SALIDA / 2, false));
        assert!(!c.animando(inicio + Duration::from_secs(2), false));
        assert!(c.animando(inicio + DURACION - SALIDA - TIC_QUIETO, false));
        assert!(c.animando(inicio + DURACION - SALIDA / 2, false));
        assert!(!c.animando(inicio + SALIDA / 2, true));
        let id = c.ultimo().unwrap();
        c.cerrar(id, inicio + Duration::from_secs(2));
        assert!(c.animando(inicio + Duration::from_secs(2), false));
    }

    #[test]
    fn deshacer_tiene_diez_segundos_y_conserva_el_fundido() {
        let inicio = Instant::now();
        let a = Aviso::nuevo("Retirados".into(), Some(()), inicio)
            .con_duracion(Duration::from_secs(10));
        assert!(!a.vencido(inicio + Duration::from_secs(9)));
        assert_eq!(a.opacidad(inicio + Duration::from_secs(9), false), 1.);
        assert!(a.opacidad(inicio + Duration::from_secs(10) - SALIDA / 2, false) < 1.);
        assert!(a.vencido(inicio + Duration::from_secs(10)));
    }
}
