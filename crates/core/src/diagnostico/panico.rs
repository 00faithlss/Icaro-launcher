// SPDX-License-Identifier: GPL-3.0-only
//! Escritura de emergencia para un panic fatal.
//!
//! Release usa `panic = "abort"`: tras el hook el proceso termina y la cola del
//! escritor no llega a vaciarse. Por eso el hook escribe de forma síncrona en
//! `logs/panico.jsonl`, por una ruta independiente del canal y del bloqueo del
//! escritor, y solo después llama al hook anterior.
//!
//! Garantías y límites:
//! - Registra ubicación (archivo y línea) y la clase de la carga, nunca su texto,
//!   backtrace ni nombre de hilo.
//! - No espera bloqueos en memoria ni del sistema operativo más allá de unos
//!   pocos reintentos de `try_lock`; un panic simultáneo o anidado se omite.
//!   La escritura física en disco sí puede tardar: no se promete un tiempo.
//! - No captura abortos externos, `kill`, falta de memoria ni fallos del sistema
//!   de archivos. Lo que imprima el hook anterior en stderr no se copia al log.
use super::{
    archivos,
    modelos::{linea_json, ubicacion_json, Cabecera, Causa, Evento, Nivel, TipoEvento},
};
use std::{
    any::Any,
    panic::{self, Location},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, TryLockError,
    },
    time::Duration,
};

/// Tamaño máximo de `panico.jsonl` antes de pasar a su único respaldo.
const LIMITE: u64 = 256 * 1024;
const REINTENTOS_BLOQUEO: Duration = Duration::from_millis(100);

pub(crate) struct Contexto {
    pub logs: PathBuf,
    pub cab: Cabecera,
}

static CONTEXTO: Mutex<Option<Arc<Contexto>>> = Mutex::new(None);
static INSTALADO: AtomicBool = AtomicBool::new(false);
static EN_CURSO: AtomicBool = AtomicBool::new(false);

/// Actualiza el contexto e instala una vez el hook, encadenado antes del anterior.
pub(crate) fn instalar(contexto: Contexto) {
    *CONTEXTO.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(contexto));
    if INSTALADO.swap(true, Ordering::AcqRel) {
        return;
    }
    let anterior = panic::take_hook();
    panic::set_hook(Box::new(move |info| {
        emergencia(info.location(), clase_carga(info.payload()));
        anterior(info);
    }));
}

/// Clase de la carga del panic, sin leer su contenido.
fn clase_carga(carga: &(dyn Any + Send)) -> &'static str {
    if carga.is::<&'static str>() {
        "texto_estatico"
    } else if carga.is::<String>() {
        "texto_dinamico"
    } else {
        "otra"
    }
}

fn emergencia(ubicacion: Option<&Location<'_>>, carga: &'static str) {
    // Un panic anidado o simultáneo no espera: se omite.
    if EN_CURSO.swap(true, Ordering::AcqRel) {
        return;
    }
    let contexto = match CONTEXTO.try_lock() {
        Ok(guarda) => guarda.clone(),
        Err(TryLockError::Poisoned(guarda)) => guarda.into_inner().clone(),
        Err(TryLockError::WouldBlock) => None,
    };
    if let Some(contexto) = contexto {
        let _ = escribir(&contexto, ubicacion, carga);
        // También en el registro legible, para verlo junto al resto.
        let donde = ubicacion
            .map(|l| format!(" en {}:{}", super::modelos::nombre_base(l.file()), l.line()))
            .unwrap_or_default();
        super::legible::anexar(
            &contexto.logs.join(super::legible::ARCHIVO),
            &format!(
                "[{}] [proceso/ERROR]: cierre inesperado{donde}\n",
                super::legible::fecha_local(std::time::SystemTime::now())
            ),
        );
    }
    EN_CURSO.store(false, Ordering::Release);
}

fn escribir(
    contexto: &Contexto,
    ubicacion: Option<&Location<'_>>,
    carga: &'static str,
) -> Result<(), Causa> {
    archivos::carpeta_valida(&contexto.logs)?;
    let cerrojo = archivos::abrir_bloqueo(&contexto.logs.join("panico.lock"))?;
    archivos::bloquear(&cerrojo, REINTENTOS_BLOQUEO, Duration::from_millis(5))?;
    let resultado = escribir_bloqueado(contexto, ubicacion, carga);
    let _ = cerrojo.unlock();
    resultado
}

fn escribir_bloqueado(
    contexto: &Contexto,
    ubicacion: Option<&Location<'_>>,
    carga: &'static str,
) -> Result<(), Causa> {
    let base = contexto.logs.join("panico.jsonl");
    let linea = linea_panico(&contexto.cab, ubicacion, carga);
    if linea.len() as u64 > LIMITE {
        return Err(Causa::Validacion("linea_demasiado_grande"));
    }
    let mut tamano = archivos::tamano_regular(&base)?.unwrap_or(0);
    if tamano > 0 && tamano + linea.len() as u64 > LIMITE {
        archivos::rotar(&base, 1)?;
        tamano = 0;
    }
    archivos::anexar(&base, tamano, &linea)
}

pub(crate) fn linea_panico(
    cab: &Cabecera,
    ubicacion: Option<&Location<'_>>,
    carga: &'static str,
) -> Vec<u8> {
    let mut evento = Evento::nuevo(TipoEvento::Panico, Nivel::Error, "proceso", "panico");
    evento.resultado = Some("error");
    evento.causa = Some(Causa::Panico);
    let mut objeto = evento.objeto(cab, None);
    objeto.insert("carga".into(), carga.into());
    if let Some(lugar) = ubicacion {
        objeto.insert(
            "ubicacion".into(),
            ubicacion_json(lugar.file(), lugar.line()),
        );
    }
    linea_json(objeto)
}
