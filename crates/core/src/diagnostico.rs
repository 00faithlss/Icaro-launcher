// SPDX-License-Identifier: GPL-3.0-only
//! Registro de diagnóstico local en `logs/diagnostico.jsonl`.
//!
//! Los emisores ([`Registro`], [`Medicion`]) solo encolan con `try_send` en una
//! cola acotada, bajo un mutex que solo cubre esa operación en memoria: nunca
//! esperan disco ni cola llena. Desde la primera intención de cerrar dejan de
//! aceptarse eventos y lo ya aceptado se escribe o se cuenta como perdido.
//! Un hilo escritor numera, serializa,
//! rota (1 MiB, tres respaldos) y escribe bajo un bloqueo del sistema operativo.
//! Solo se registran etiquetas estáticas, números y causas tipadas; las
//! instancias se identifican por un alias salado de la sesión.
//!
//! No se garantiza persistir lo que siga en memoria ante `kill`, corte de luz,
//! disco lleno o fallo del sistema de archivos. El panic fatal tiene su propia
//! ruta de emergencia en `logs/panico.jsonl` (ver [`Sesion::instalar_panico`]).
mod archivos;
mod escritor;
mod global;
mod legible;
mod medicion;
mod modelos;
mod panico;
#[cfg(test)]
mod pruebas;
#[cfg(test)]
mod pruebas_escritura;

pub use legible::{fecha_local, usar_desfase_local, ARCHIVO as ARCHIVO_LEGIBLE};
pub use medicion::Medicion;
pub use modelos::{Campo, Causa, Resultado, ESQUEMA};

use escritor::{Acuse, Config, Escritor, Mensaje, Salud};
use modelos::{Cabecera, Evento, Ubicacion};
use sha2::{Digest, Sha256};
use std::{
    fmt,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc::{self, RecvTimeoutError, SyncSender, TrySendError},
        Arc,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

/// Nombre del registro dentro de `logs/`.
pub const ARCHIVO: &str = "diagnostico.jsonl";
const ARCHIVO_BLOQUEO: &str = "diagnostico.lock";

/// Fallo del propio registro. No contiene rutas ni mensajes del sistema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorRegistro {
    /// La carpeta o el archivo de registro no son utilizables.
    Archivo(Causa),
    /// No se pudo generar el identificador aleatorio de la sesión.
    Aleatorio,
    /// No se pudo crear el hilo escritor.
    Hilo,
    /// El escritor no respondió dentro del plazo.
    Plazo,
    /// El escritor ya terminó.
    Cerrado,
    /// Se perdieron eventos al escribir desde el último vaciado.
    Escritura(Causa),
}

impl fmt::Display for ErrorRegistro {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ErrorRegistro::Archivo(_) => "el archivo de diagnóstico no es utilizable",
            ErrorRegistro::Aleatorio => "no se pudo generar la sesión de diagnóstico",
            ErrorRegistro::Hilo => "no se pudo iniciar el escritor de diagnóstico",
            ErrorRegistro::Plazo => "el escritor de diagnóstico no respondió a tiempo",
            ErrorRegistro::Cerrado => "el registro de diagnóstico está cerrado",
            ErrorRegistro::Escritura(_) => "no se pudieron escribir eventos de diagnóstico",
        })
    }
}

impl std::error::Error for ErrorRegistro {}

/// Salud del registro. `enviados` son los eventos aceptados por la cola;
/// `descartados`, los rechazados por cola llena o cerrada; `perdidos`, los
/// aceptados que no pudieron escribirse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Estado {
    pub enviados: u64,
    pub escritos: u64,
    pub descartados: u64,
    pub perdidos: u64,
    pub errores_escritura: u64,
    pub ultimo_error: Option<Causa>,
    pub cerrado: bool,
}

impl Estado {
    /// Eventos aceptados que todavía no se escribieron ni se perdieron.
    pub fn pendientes(&self) -> u64 {
        self.enviados.saturating_sub(self.escritos + self.perdidos)
    }
}

/// Inicia el registro en `raiz/logs`. Valida la carpeta y el archivo antes de
/// crear el hilo escritor. No emite eventos por sí mismo.
pub fn iniciar(
    raiz: &Path,
    version: &'static str,
    compilacion: &'static str,
) -> Result<Sesion, ErrorRegistro> {
    iniciar_con(raiz, version, compilacion, Config::default())
}

pub(crate) fn iniciar_con(
    raiz: &Path,
    version: &'static str,
    compilacion: &'static str,
    config: Config,
) -> Result<Sesion, ErrorRegistro> {
    let logs = archivos::carpeta_logs(raiz).map_err(ErrorRegistro::Archivo)?;
    let base = logs.join(ARCHIVO);
    archivos::tamano_regular(&base).map_err(ErrorRegistro::Archivo)?;
    let bloqueo =
        archivos::abrir_bloqueo(&logs.join(ARCHIVO_BLOQUEO)).map_err(ErrorRegistro::Archivo)?;
    let mut aleatorio = [0u8; 40];
    getrandom::fill(&mut aleatorio).map_err(|_| ErrorRegistro::Aleatorio)?;
    let mut sal = [0u8; 32];
    sal.copy_from_slice(&aleatorio[8..]);
    let cab = Cabecera {
        sesion: hex(&aleatorio[..8]),
        version,
        compilacion,
    };
    let salud = Arc::new(Salud::default());
    let (tx, rx) = mpsc::sync_channel(config.capacidad);
    // El registro legible es un espejo: si no se puede preparar, se sigue sin él.
    let legible = legible::preparar(&logs, &cab);
    let escritor = Escritor::nuevo(
        rx,
        cab.clone(),
        salud.clone(),
        config.clone(),
        base,
        bloqueo,
    )
    .con_legible(legible);
    let hilo = thread::Builder::new()
        .name("icaro-diagnostico".into())
        .spawn(move || escritor.ejecutar())
        .map_err(|_| ErrorRegistro::Hilo)?;
    let compartido = Compartido {
        tx,
        salud,
        sal,
        siguiente: AtomicU64::new(1),
        logs,
        cab,
    };
    Ok(Sesion {
        registro: Registro {
            c: Arc::new(compartido),
        },
        hilo: Some(hilo),
        cerrada: false,
        plazo_drop: config.plazo_drop,
    })
}

/// Gravedad de una línea libre del registro legible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gravedad {
    Info,
    Aviso,
    Error,
}

/// Escribe en `logs/latest.log` una línea con texto libre, por ejemplo el
/// mensaje completo de un error con su ruta. Es un registro local para quien
/// usa Ícaro: no va a `diagnostico.jsonl`. No espera ni hace IO; sin registro
/// global no hace nada.
#[track_caller]
pub fn anotar(gravedad: Gravedad, componente: &str, texto: &str) {
    let Some(registro) = global::actual() else {
        return;
    };
    let lugar = std::panic::Location::caller();
    let nivel = match gravedad {
        Gravedad::Info => "INFO",
        Gravedad::Aviso => "WARN",
        Gravedad::Error => "ERROR",
    };
    let donde = if gravedad == Gravedad::Info {
        String::new()
    } else {
        format!(" · {}:{}", modelos::nombre_base(lugar.file()), lugar.line())
    };
    let texto: String = texto.chars().take(4000).collect::<String>().replace(['\r', '\n'], " ");
    registro.anotar(format!(
        "[{}] [{componente}/{nivel}]: {texto}{donde}\n",
        legible::fecha_local(std::time::SystemTime::now())
    ));
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Propietario del hilo escritor. Al descartarse cierra con plazo acotado.
pub struct Sesion {
    registro: Registro,
    hilo: Option<JoinHandle<()>>,
    cerrada: bool,
    plazo_drop: Duration,
}

impl fmt::Debug for Sesion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sesion")
            .field("cerrada", &self.cerrada)
            .finish_non_exhaustive()
    }
}

impl Sesion {
    pub fn registro(&self) -> Registro {
        self.registro.clone()
    }

    /// Vacía la cola y detiene el escritor esperando como mucho `plazo`. Si el
    /// escritor no responde, se deja el hilo desacoplado y se devuelve `Plazo`:
    /// lo que siguiera en memoria puede perderse. Llamadas posteriores no hacen nada.
    pub fn cerrar(&mut self, plazo: Duration) -> Result<(), ErrorRegistro> {
        if self.cerrada {
            return Ok(());
        }
        self.cerrada = true;
        global::retirar(&self.registro);
        let fin = Instant::now() + plazo;
        let resultado = self.registro.control(Mensaje::Cerrar, true, plazo);
        if let Some(hilo) = self.hilo.take() {
            while !hilo.is_finished() && Instant::now() < fin {
                thread::sleep(Duration::from_millis(1));
            }
            if hilo.is_finished() {
                let _ = hilo.join();
            }
        }
        resultado
    }

    /// Instala este registro como global para [`Medicion::global`]. Devuelve
    /// `false` si ya hay otro registro global abierto.
    pub fn instalar_global(&self) -> bool {
        global::instalar(&self.registro)
    }

    /// Encadena el hook de panic de emergencia antes del hook actual. Escribe en
    /// `logs/panico.jsonl` sin pasar por la cola; ver el módulo `panico`.
    pub fn instalar_panico(&self) {
        let c = &self.registro.c;
        panico::instalar(panico::Contexto {
            logs: c.logs.clone(),
            cab: c.cab.clone(),
        });
    }
}

impl Drop for Sesion {
    fn drop(&mut self) {
        let plazo = self.plazo_drop;
        let _ = self.cerrar(plazo);
    }
}

/// Emisor clonable y barato. Nunca bloquea ni hace IO.
#[derive(Clone)]
pub struct Registro {
    c: Arc<Compartido>,
}

struct Compartido {
    tx: SyncSender<Mensaje>,
    salud: Arc<Salud>,
    sal: [u8; 32],
    siguiente: AtomicU64,
    logs: PathBuf,
    cab: Cabecera,
}

impl fmt::Debug for Registro {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Registro").finish_non_exhaustive()
    }
}

impl Registro {
    /// Empieza a medir una operación; emite su línea de inicio.
    #[track_caller]
    pub fn medir(&self, componente: &'static str, accion: &'static str) -> Medicion {
        Medicion::nueva(self.clone(), componente, accion, Ubicacion::llamador())
    }

    /// Espera como mucho `plazo` a que el escritor procese lo encolado hasta
    /// ahora. Devuelve `Escritura` si se perdieron eventos desde el último vaciado.
    /// Puede esperar: no llamarlo desde `update`/`view`.
    pub fn vaciar(&self, plazo: Duration) -> Result<(), ErrorRegistro> {
        self.control(Mensaje::Vaciar, false, plazo)
    }

    pub fn estado(&self) -> Estado {
        let s = &self.c.salud;
        Estado {
            enviados: s.enviados.load(Ordering::Acquire),
            escritos: s.escritos.load(Ordering::Acquire),
            descartados: s.descartados.load(Ordering::Acquire),
            perdidos: s.perdidos.load(Ordering::Acquire),
            errores_escritura: s.errores_escritura.load(Ordering::Acquire),
            ultimo_error: s.ultimo_error(),
            cerrado: s.cerrado.load(Ordering::Acquire),
        }
    }

    /// Encola sin esperar. La comprobación de cierre, el `try_send` y los
    /// contadores van bajo el mutex corto de la cola, para que ningún evento se
    /// acepte después de que el escritor drene lo que quedó tras `Cerrar`.
    pub(crate) fn emitir(&self, evento: Evento) {
        let salud = &self.c.salud;
        let _cola = salud.cola();
        if salud.cerrando.load(Ordering::Acquire) {
            salud.descartados.fetch_add(1, Ordering::AcqRel);
            return;
        }
        salud.enviados.fetch_add(1, Ordering::AcqRel);
        if self.c.tx.try_send(Mensaje::Evento(evento)).is_err() {
            salud.enviados.fetch_sub(1, Ordering::AcqRel);
            salud.descartados.fetch_add(1, Ordering::AcqRel);
        }
    }

    /// Línea libre del registro legible; no pasa por `diagnostico.jsonl`.
    fn anotar(&self, texto: String) {
        let salud = &self.c.salud;
        let _cola = salud.cola();
        if !salud.cerrando.load(Ordering::Acquire) {
            let _ = self.c.tx.try_send(Mensaje::Texto(texto));
        }
    }

    pub(crate) fn nueva_operacion(&self) -> u64 {
        self.c.siguiente.fetch_add(1, Ordering::Relaxed)
    }

    /// Alias de 16 caracteres derivado del ID y la sal de la sesión; `None` si
    /// el ID no tiene exactamente 32 caracteres hexadecimales.
    pub(crate) fn alias_instancia(&self, id: &str) -> Option<Arc<str>> {
        if id.len() != 32 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let mut resumen = Sha256::new();
        resumen.update(b"icaro-diagnostico-instancia\0");
        resumen.update(self.c.sal);
        resumen.update(id.to_ascii_lowercase().as_bytes());
        let resumen = resumen.finalize();
        Some(Arc::from(hex(&resumen[..8]).as_str()))
    }

    pub(crate) fn mismo(&self, otro: &Registro) -> bool {
        Arc::ptr_eq(&self.c, &otro.c)
    }

    pub(crate) fn cerrado(&self) -> bool {
        self.c.salud.cerrado.load(Ordering::Acquire)
    }

    /// Envía un mensaje de control con reintentos no bloqueantes y espera el
    /// acuse. Con `cierre`, marca `cerrando` en el mismo paso que el envío, de
    /// modo que desde la primera intención de cerrar no entran eventos nuevos y
    /// la cola llena solo puede vaciarse. Cada intento retiene el mutex corto de
    /// la cola; las pausas entre intentos y la espera del acuse van fuera de él.
    fn control(
        &self,
        crear: fn(Acuse) -> Mensaje,
        cierre: bool,
        plazo: Duration,
    ) -> Result<(), ErrorRegistro> {
        let fin = Instant::now() + plazo;
        let (acuse, respuesta) = mpsc::sync_channel(1);
        let mut mensaje = crear(acuse);
        loop {
            let intento = {
                let salud = &self.c.salud;
                let _cola = salud.cola();
                if cierre {
                    salud.cerrando.store(true, Ordering::Release);
                }
                self.c.tx.try_send(mensaje)
            };
            match intento {
                Ok(()) => break,
                Err(TrySendError::Full(devuelto)) => {
                    if Instant::now() >= fin {
                        return Err(ErrorRegistro::Plazo);
                    }
                    mensaje = devuelto;
                    thread::sleep(Duration::from_millis(2));
                }
                Err(TrySendError::Disconnected(_)) => return Err(ErrorRegistro::Cerrado),
            }
        }
        match respuesta.recv_timeout(fin.saturating_duration_since(Instant::now())) {
            Ok(resultado) => resultado.map_err(ErrorRegistro::Escritura),
            Err(RecvTimeoutError::Timeout) => Err(ErrorRegistro::Plazo),
            Err(RecvTimeoutError::Disconnected) => Err(ErrorRegistro::Cerrado),
        }
    }
}
