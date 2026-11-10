// SPDX-License-Identifier: GPL-3.0-only
//! Hilo escritor único de `logs/diagnostico.jsonl`.
//!
//! Los emisores solo hacen `try_send` a una cola acotada. Este hilo asigna la
//! secuencia en orden estricto de recepción, serializa, rota bajo un bloqueo del
//! sistema operativo en un archivo separado y lleva la contabilidad de salud.
//! No mantiene abierto el archivo de destino entre lotes, para que otro proceso
//! pueda rotarlo.
//!
//! Cierre: los emisores encolan bajo `Salud::cola`, un mutex que solo protege la
//! comprobación de `cerrando`, el `try_send` y los contadores (nunca disco ni
//! esperas). Desde la primera intención de cerrar (`Sesion::cerrar` o la
//! recepción de `Cerrar`) no se aceptan eventos nuevos; al recibir `Cerrar` el
//! escritor drena lo que quedara detrás, de modo que todo evento aceptado acaba
//! escrito o contado como perdido, y todo control pendiente recibe acuse.
//!
//! Una línea mayor que la cota se cuenta como perdida y se notifica sin impedir
//! que se escriban las válidas que la siguen en el mismo lote.
use super::{
    archivos,
    modelos::{Cabecera, Campo, Causa, Evento, Nivel, TipoEvento},
};
use std::{
    fs::File,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{Receiver, RecvTimeoutError, SyncSender},
        Arc, Mutex,
    },
    time::Duration,
};

/// Máximo de líneas acumuladas antes de escribir aunque la cola siga con datos.
const LOTE_MAXIMO: usize = 256;

pub(crate) type Acuse = SyncSender<Result<(), Causa>>;

pub(crate) enum Mensaje {
    Evento(Evento),
    /// Línea libre solo para el registro legible (mensajes de error con rutas).
    Texto(String),
    Vaciar(Acuse),
    Cerrar(Acuse),
}

/// Parámetros del registro; las pruebas colocadas usan límites pequeños.
#[derive(Clone)]
pub(crate) struct Config {
    pub capacidad: usize,
    pub limite: u64,
    pub respaldos: usize,
    pub plazo_bloqueo: Duration,
    pub intervalo: Duration,
    pub plazo_drop: Duration,
    #[cfg(test)]
    pub compuerta: Option<Arc<Compuerta>>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            capacidad: 512,
            limite: 1024 * 1024,
            respaldos: 3,
            plazo_bloqueo: Duration::from_secs(2),
            intervalo: Duration::from_secs(1),
            plazo_drop: Duration::from_secs(2),
            #[cfg(test)]
            compuerta: None,
        }
    }
}

/// Gancho determinista de pruebas: el escritor no recibe hasta que se abre.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct Compuerta {
    abierta: Mutex<bool>,
    aviso: std::sync::Condvar,
}

#[cfg(test)]
impl Compuerta {
    pub(crate) fn abrir(&self) {
        *self.abierta.lock().unwrap_or_else(|e| e.into_inner()) = true;
        self.aviso.notify_all();
    }

    fn esperar(&self) {
        let mut abierta = self.abierta.lock().unwrap_or_else(|e| e.into_inner());
        while !*abierta {
            abierta = self.aviso.wait(abierta).unwrap_or_else(|e| e.into_inner());
        }
    }
}

/// Contadores compartidos entre emisores, escritor y consultas de estado.
#[derive(Default)]
pub(crate) struct Salud {
    pub enviados: AtomicU64,
    pub escritos: AtomicU64,
    pub descartados: AtomicU64,
    pub perdidos: AtomicU64,
    pub errores_escritura: AtomicU64,
    pub ultimo_error: Mutex<Option<Causa>>,
    /// El hilo escritor terminó (o ya respondió a `Cerrar`).
    pub cerrado: AtomicBool,
    /// Se pidió cerrar: no se aceptan eventos nuevos. Se escribe bajo `cola`.
    pub cerrando: AtomicBool,
    /// Serializa la aceptación de eventos y controles con el inicio del cierre.
    /// Solo se retiene durante operaciones de memoria no bloqueantes.
    pub cola: Mutex<()>,
}

impl Salud {
    pub(crate) fn ultimo_error(&self) -> Option<Causa> {
        *self.ultimo_error.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Retiene el mutex corto de la cola. Nunca esperar disco ni colas con él.
    pub(crate) fn cola(&self) -> std::sync::MutexGuard<'_, ()> {
        self.cola.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Deja de aceptar eventos; tras volver, ningún emisor puede encolar más.
    pub(crate) fn marcar_cerrando(&self) {
        let _cola = self.cola();
        self.cerrando.store(true, Ordering::Release);
    }
}

/// Marca el registro como cerrado al terminar el hilo, también si termina mal.
struct MarcaCierre(Arc<Salud>);

impl Drop for MarcaCierre {
    fn drop(&mut self) {
        // Sin el mutex: si el hilo terminó por un panic no debe esperar a nadie;
        // los envíos posteriores fallan igualmente porque el receptor ya no existe.
        self.0.cerrando.store(true, Ordering::Release);
        self.0.cerrado.store(true, Ordering::Release);
    }
}

struct Linea {
    bytes: Vec<u8>,
    /// `true` si proviene de un emisor (cuenta en `escritos`/`perdidos`).
    externa: bool,
}

pub(crate) struct Escritor {
    rx: Receiver<Mensaje>,
    cab: Cabecera,
    salud: Arc<Salud>,
    config: Config,
    base: PathBuf,
    bloqueo: File,
    seq: u64,
    lote: Vec<Linea>,
    descartes_informados: u64,
    perdidos_informados: u64,
    fallo_sin_acuse: Option<Causa>,
    /// `logs/latest.log` y las líneas legibles pendientes de escribir.
    legible: Option<PathBuf>,
    texto: String,
}

impl Escritor {
    /// Escribe además cada evento en el registro legible `ruta`.
    pub(crate) fn con_legible(mut self, ruta: Option<PathBuf>) -> Self {
        self.legible = ruta;
        self
    }

    pub(crate) fn nuevo(
        rx: Receiver<Mensaje>,
        cab: Cabecera,
        salud: Arc<Salud>,
        config: Config,
        base: PathBuf,
        bloqueo: File,
    ) -> Self {
        Self {
            rx,
            cab,
            salud,
            config,
            base,
            bloqueo,
            seq: 0,
            lote: Vec::new(),
            descartes_informados: 0,
            perdidos_informados: 0,
            fallo_sin_acuse: None,
            legible: None,
            texto: String::new(),
        }
    }

    /// Bucle del hilo: recibe, drena lo disponible, escribe y responde acuses.
    pub(crate) fn ejecutar(mut self) {
        let _marca = MarcaCierre(self.salud.clone());
        loop {
            #[cfg(test)]
            if let Some(compuerta) = &self.config.compuerta {
                compuerta.esperar();
            }
            let primero = match self.rx.recv_timeout(self.config.intervalo) {
                Ok(mensaje) => mensaje,
                Err(RecvTimeoutError::Timeout) => {
                    self.volcar();
                    continue;
                }
                Err(RecvTimeoutError::Disconnected) => {
                    self.volcar();
                    return;
                }
            };
            let mut sigue = self.procesar(primero);
            while sigue {
                match self.rx.try_recv() {
                    Ok(mensaje) => sigue = self.procesar(mensaje),
                    Err(_) => break,
                }
            }
            if !sigue {
                return;
            }
            self.volcar();
        }
    }

    /// Devuelve `false` si se pidió cerrar.
    fn procesar(&mut self, mensaje: Mensaje) -> bool {
        match mensaje {
            Mensaje::Evento(evento) => {
                self.anotar(&evento, true);
                if self.lote.len() >= LOTE_MAXIMO {
                    self.volcar();
                }
                true
            }
            Mensaje::Vaciar(acuse) => {
                self.volcar();
                self.responder(acuse);
                true
            }
            Mensaje::Texto(t) => {
                self.texto_libre(t);
                true
            }
            Mensaje::Cerrar(acuse) => {
                self.cerrar(acuse);
                false
            }
        }
    }

    /// Deja de aceptar eventos y drena lo que quedó detrás de `Cerrar` (eventos
    /// aceptados antes de marcar `cerrando`, vaciados y otros cierres). Como los
    /// emisores comprueban `cerrando` bajo el mismo mutex que usan para encolar,
    /// cuando `try_recv` queda vacío ya no puede llegar ningún evento.
    fn cerrar(&mut self, acuse: Acuse) {
        self.salud.marcar_cerrando();
        let mut cierres = vec![acuse];
        while let Ok(mensaje) = self.rx.try_recv() {
            match mensaje {
                Mensaje::Evento(evento) => {
                    self.anotar(&evento, true);
                    if self.lote.len() >= LOTE_MAXIMO {
                        self.volcar();
                    }
                }
                Mensaje::Vaciar(vaciado) => {
                    self.volcar();
                    self.responder(vaciado);
                }
                Mensaje::Texto(t) => self.texto_libre(t),
                Mensaje::Cerrar(otro) => cierres.push(otro),
            }
        }
        self.volcar();
        self.salud.cerrado.store(true, Ordering::Release);
        let resultado = match self.fallo_sin_acuse.take() {
            Some(causa) => Err(causa),
            None => Ok(()),
        };
        for cierre in cierres {
            let _ = cierre.try_send(resultado);
        }
    }

    fn texto_libre(&mut self, t: String) {
        if self.legible.is_some() {
            self.texto.push_str(&t);
        }
    }

    fn responder(&mut self, acuse: Acuse) {
        let resultado = match self.fallo_sin_acuse.take() {
            Some(causa) => Err(causa),
            None => Ok(()),
        };
        let _ = acuse.try_send(resultado);
    }

    fn anotar(&mut self, evento: &Evento, externa: bool) {
        let bytes = evento.linea(&self.cab, self.seq);
        self.seq += 1;
        if self.legible.is_some() {
            self.texto.push_str(&super::legible::linea(evento));
        }
        self.lote.push(Linea { bytes, externa });
    }

    /// Deja constancia de descartes de la cola y de pérdidas por escritura
    /// ocurridas desde el último informe escrito. Devuelve los totales que se
    /// darán por informados si la línea del informe llega al archivo.
    fn informar_perdidas(&mut self) -> Option<(u64, u64)> {
        let descartados = self.salud.descartados.load(Ordering::Acquire);
        let perdidos = self.salud.perdidos.load(Ordering::Acquire);
        if descartados <= self.descartes_informados && perdidos <= self.perdidos_informados {
            return None;
        }
        let mut evento = Evento::nuevo(
            TipoEvento::Descartes,
            Nivel::Aviso,
            "diagnostico",
            "registro",
        );
        evento.datos = vec![
            Campo::Numero("descartados", descartados - self.descartes_informados),
            Campo::Numero("descartados_total", descartados),
            Campo::Numero("perdidos", perdidos - self.perdidos_informados),
            Campo::Numero("perdidos_total", perdidos),
        ];
        self.anotar(&evento, false);
        Some((descartados, perdidos))
    }

    fn volcar(&mut self) {
        let informe = self.informar_perdidas();
        if let (Some(ruta), false) = (&self.legible, self.texto.is_empty()) {
            super::legible::anexar(ruta, &self.texto);
            self.texto.clear();
        }
        if self.lote.is_empty() {
            return;
        }
        let lote = std::mem::take(&mut self.lote);
        let mut escritas = vec![false; lote.len()];
        let fallo = self.escribir(&lote, &mut escritas);
        let (mut hechas, mut perdidas) = (0, 0);
        for (linea, &escrita) in lote.iter().zip(&escritas) {
            match (linea.externa, escrita) {
                (true, true) => hechas += 1,
                (true, false) => perdidas += 1,
                (false, _) => {}
            }
        }
        self.salud.escritos.fetch_add(hechas, Ordering::AcqRel);
        // El informe siempre es la última línea anotada; si llegó al archivo, sus
        // totales quedan informados. Las pérdidas de este lote se informan después.
        if let Some((descartados, perdidos)) = informe {
            if escritas.last() == Some(&true) {
                self.descartes_informados = descartados;
                self.perdidos_informados = perdidos;
            }
        }
        if let Some(causa) = fallo {
            self.salud.perdidos.fetch_add(perdidas, Ordering::AcqRel);
            self.salud.errores_escritura.fetch_add(1, Ordering::AcqRel);
            *self
                .salud
                .ultimo_error
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = Some(causa);
            // El próximo acuse informa el fallo solo si se perdieron eventos de
            // emisores; un informe interno no escrito se reintenta en el siguiente lote.
            if perdidas > 0 {
                self.fallo_sin_acuse = Some(causa);
            }
        }
    }

    /// Escribe bajo el bloqueo entre procesos; marca en `escritas` cada línea que
    /// llegó al archivo y devuelve la causa del último fallo, si lo hubo.
    fn escribir(&self, lote: &[Linea], escritas: &mut [bool]) -> Option<Causa> {
        let pausa = Duration::from_millis(5);
        if let Err(causa) = archivos::bloquear(&self.bloqueo, self.config.plazo_bloqueo, pausa) {
            return Some(causa);
        }
        let resultado = self.escribir_bloqueado(lote, escritas);
        let _ = self.bloqueo.unlock();
        resultado
    }

    /// Nunca deja crecer `base` por encima del límite. Una línea mayor que la
    /// cota se salta (queda perdida y se notifica) y se sigue con las demás; si
    /// la rotación o el anexado fallan, las líneas restantes se pierden.
    fn escribir_bloqueado(&self, lote: &[Linea], escritas: &mut [bool]) -> Option<Causa> {
        let limite = self.config.limite;
        let mut tamano = match archivos::tamano_regular(&self.base) {
            Ok(tamano) => tamano.unwrap_or(0),
            Err(causa) => return Some(causa),
        };
        let mut fallo = None;
        let mut hechas = 0;
        while hechas < lote.len() {
            let primera = lote[hechas].bytes.len() as u64;
            if primera > limite {
                fallo = Some(Causa::Validacion("linea_demasiado_grande"));
                hechas += 1;
                continue;
            }
            if tamano > 0 && tamano + primera > limite {
                if let Err(causa) = archivos::rotar(&self.base, self.config.respaldos) {
                    return Some(causa);
                }
                tamano = 0;
            }
            let mut fin = hechas;
            let mut bloque = Vec::new();
            while fin < lote.len() {
                let largo = lote[fin].bytes.len() as u64;
                if fin > hechas && (largo > limite || tamano + bloque.len() as u64 + largo > limite)
                {
                    break;
                }
                bloque.extend_from_slice(&lote[fin].bytes);
                fin += 1;
            }
            if let Err(causa) = archivos::anexar(&self.base, tamano, &bloque) {
                return Some(causa);
            }
            escritas[hechas..fin].fill(true);
            tamano += bloque.len() as u64;
            hechas = fin;
        }
        fallo
    }
}
