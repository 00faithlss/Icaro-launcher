// SPDX-License-Identifier: GPL-3.0-only
//! Medición de una operación: inicio, etapas, datos y un único desenlace.
use super::{
    global,
    modelos::{Campo, Evento, Nivel, Resultado, TipoEvento, Ubicacion},
    Registro,
};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};

/// Operación en curso. Todas sus líneas comparten `operacion`. Si se descarta
/// sin `terminar`, emite una sola vez `abandonado`. Es `Send + Sync`, de modo
/// que puede compartirse con `Arc` entre tareas.
pub struct Medicion {
    interior: Option<Interior>,
}

struct Interior {
    registro: Registro,
    componente: &'static str,
    accion: &'static str,
    operacion: u64,
    inicio: Instant,
    estado: Mutex<EstadoOperacion>,
    terminada: AtomicBool,
}

#[derive(Default)]
struct EstadoOperacion {
    etapa: Option<&'static str>,
    instancia: Option<Arc<str>>,
}

impl Medicion {
    pub(crate) fn nueva(
        registro: Registro,
        componente: &'static str,
        accion: &'static str,
        ubicacion: Ubicacion,
    ) -> Self {
        let interior = Interior {
            operacion: registro.nueva_operacion(),
            registro,
            componente,
            accion,
            inicio: Instant::now(),
            estado: Mutex::new(EstadoOperacion::default()),
            terminada: AtomicBool::new(false),
        };
        let mut evento = interior.evento(TipoEvento::Inicio, Nivel::Info);
        evento.ubicacion = Some(ubicacion);
        interior.registro.emitir(evento);
        Self {
            interior: Some(interior),
        }
    }

    /// Mide con el registro global; sin registro global no hace nada.
    #[track_caller]
    pub fn global(componente: &'static str, accion: &'static str) -> Medicion {
        let ubicacion = Ubicacion::llamador();
        match global::actual() {
            Some(registro) => Medicion::nueva(registro, componente, accion, ubicacion),
            None => Medicion { interior: None },
        }
    }

    fn activa(&self) -> Option<&Interior> {
        self.interior
            .as_ref()
            .filter(|i| !i.terminada.load(Ordering::Acquire))
    }

    /// Cambia la etapa actual; las líneas siguientes y el desenlace la incluyen.
    pub fn etapa(&self, etapa: &'static str) {
        if let Some(i) = self.activa() {
            i.estado().etapa = Some(etapa);
            i.registro.emitir(i.evento(TipoEvento::Etapa, Nivel::Info));
        }
    }

    /// Registra un dato real de la operación (cantidades, etiquetas, salida).
    pub fn dato(&self, campo: Campo) {
        if let Some(i) = self.activa() {
            let mut evento = i.evento(TipoEvento::Dato, Nivel::Info);
            evento.datos.push(campo);
            i.registro.emitir(evento);
        }
    }

    /// Asocia la instancia por un alias salado de la sesión. Solo acepta IDs
    /// de 32 caracteres hexadecimales; el ID y cualquier otro texto no se registran.
    pub fn instancia(&self, id: &str) {
        let Some(i) = self.activa() else { return };
        match i.registro.alias_instancia(id) {
            Some(alias) => i.estado().instancia = Some(alias),
            None => {
                let mut evento = i.evento(TipoEvento::Dato, Nivel::Aviso);
                evento.datos.push(Campo::Etiqueta("instancia", "invalida"));
                i.registro.emitir(evento);
            }
        }
    }

    /// Cierra la operación con su desenlace. Las llamadas posteriores no tienen efecto.
    #[track_caller]
    pub fn terminar(&self, resultado: Resultado) {
        let ubicacion = Ubicacion::llamador();
        let Some(i) = self.interior.as_ref() else {
            return;
        };
        if i.terminada.swap(true, Ordering::AcqRel) {
            return;
        }
        let nivel = match resultado {
            Resultado::Error(_) => Nivel::Error,
            Resultado::Exito | Resultado::Cancelado => Nivel::Info,
        };
        let mut evento = i.evento(TipoEvento::Fin, nivel);
        evento.resultado = Some(resultado.etiqueta());
        if let Resultado::Error(causa) = resultado {
            evento.causa = Some(causa);
        }
        evento.ubicacion = Some(ubicacion);
        i.registro.emitir(evento);
    }
    /// El dueño del futuro puede abandonarlo aunque un hilo aún conserve el contexto.
    /// El progreso posterior no modifica una operación cuyo desenlace ya se emitió.
    pub fn abandonar(&self) {
        let Some(i) = self.interior.as_ref() else {
            return;
        };
        if !i.terminada.swap(true, Ordering::AcqRel) {
            let mut evento = i.evento(TipoEvento::Abandono, Nivel::Aviso);
            evento.resultado = Some("abandonado");
            i.registro.emitir(evento);
        }
    }
}

impl Drop for Medicion {
    fn drop(&mut self) {
        self.abandonar();
    }
}

impl Interior {
    fn estado(&self) -> std::sync::MutexGuard<'_, EstadoOperacion> {
        self.estado.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Evento de esta operación con fecha y duración tomadas ahora.
    fn evento(&self, tipo: TipoEvento, nivel: Nivel) -> Evento {
        let (etapa, instancia) = {
            let estado = self.estado();
            (estado.etapa, estado.instancia.clone())
        };
        let mut evento = Evento::nuevo(tipo, nivel, self.componente, self.accion);
        evento.operacion = Some(self.operacion);
        evento.etapa = etapa;
        evento.instancia = instancia;
        evento.duracion_ms =
            Some(u64::try_from(self.inicio.elapsed().as_millis()).unwrap_or(u64::MAX));
        evento
    }
}
