// SPDX-License-Identifier: GPL-3.0-only
//! Tipos públicos del diagnóstico y su serialización JSON.
//!
//! Solo se aceptan etiquetas `&'static str` escritas en el código, números y
//! causas tipadas: ningún tipo de este módulo guarda texto dinámico, `Display`
//! ni `Debug` de errores externos.
use serde_json::{Map, Value};
use std::{
    io,
    panic::Location,
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

/// Versión del formato de cada línea JSON.
pub const ESQUEMA: u64 = 1;

/// Desenlace de una operación medida.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resultado {
    Exito,
    Cancelado,
    Error(Causa),
}

impl Resultado {
    pub(crate) fn etiqueta(&self) -> &'static str {
        match self {
            Resultado::Exito => "exito",
            Resultado::Cancelado => "cancelado",
            Resultado::Error(_) => "error",
        }
    }
}

/// Dato estructurado de una operación; nombres y valores de texto son estáticos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Campo {
    Numero(&'static str, u64),
    Etiqueta(&'static str, &'static str),
    /// Código de salida de un proceso; `None` si terminó sin código (señal).
    CodigoSalida(Option<i32>),
}

/// Causa tipada de un fallo, sin mensajes, rutas ni URL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Causa {
    Disco {
        tipo: &'static str,
        codigo_os: Option<i32>,
    },
    Red {
        timeout: bool,
        conexion: bool,
        estado: Option<u16>,
    },
    Servicio {
        estado: u16,
        codigo: &'static str,
    },
    Validacion(&'static str),
    Integridad,
    Conflicto,
    Credencial,
    Interno,
    Panico,
    Proceso {
        codigo: Option<i32>,
    },
}

/// Códigos de error conocidos del servicio de invitaciones. Cualquier otro
/// código remoto se registra como `desconocido`, nunca copiado.
const CODIGOS_SERVICIO: &[&str] = &[
    "not_found",
    "wrong_host",
    "host_invitation_limit",
    "code_collision",
    "sharing_stopped",
    "access_revoked",
    "invitation_expired",
    "expiry_in_past",
    "capacity_below_accepted",
    "capacity_full",
    "claim_conflict",
    "temporarily_unavailable",
    "invalid_request",
    "body_too_large",
    "invalid_json",
    "invalid_signature",
    "expired_signature",
    "invalid_payload",
    "replayed_nonce",
    "identity_rate_limited",
    "invalid_meeting_signature",
    "invalid_meeting",
    "rate_limited",
    "daily_budget_exhausted",
];

impl Causa {
    /// Categoría y código del sistema operativo; nunca el mensaje ni la ruta.
    pub fn de_io(error: &io::Error) -> Self {
        Causa::Disco {
            tipo: tipo_io(error.kind()),
            codigo_os: error.raw_os_error(),
        }
    }

    /// Indicadores y estado HTTP de un error de reqwest; nunca su URL ni texto.
    pub fn de_red(error: &reqwest::Error) -> Self {
        if error.is_decode() {
            return Causa::Validacion("respuesta_json");
        }
        Causa::Red {
            timeout: error.is_timeout(),
            conexion: error.is_connect(),
            estado: error.status().map(|estado| estado.as_u16()),
        }
    }

    /// Respuesta de error del servicio: el código remoto pasa por la lista de
    /// etiquetas conocidas y se sustituye por `desconocido` si no figura.
    pub fn servicio(estado: u16, codigo_remoto: &str) -> Self {
        let codigo = CODIGOS_SERVICIO
            .iter()
            .copied()
            .find(|conocido| *conocido == codigo_remoto)
            .unwrap_or("desconocido");
        Causa::Servicio { estado, codigo }
    }

    pub(crate) fn categoria(&self) -> &'static str {
        match self {
            Causa::Disco { .. } => "disco",
            Causa::Red { .. } => "red",
            Causa::Servicio { .. } => "servicio",
            Causa::Validacion(_) => "validacion",
            Causa::Integridad => "integridad",
            Causa::Conflicto => "conflicto",
            Causa::Credencial => "credencial",
            Causa::Interno => "interno",
            Causa::Panico => "panico",
            Causa::Proceso { .. } => "proceso",
        }
    }

    pub(crate) fn json(&self) -> Value {
        let mut m = Map::new();
        m.insert("categoria".into(), self.categoria().into());
        match *self {
            Causa::Disco { tipo, codigo_os } => {
                m.insert("tipo".into(), tipo.into());
                m.insert("codigo_os".into(), codigo_os.into());
            }
            Causa::Red {
                timeout,
                conexion,
                estado,
            } => {
                m.insert("timeout".into(), timeout.into());
                m.insert("conexion".into(), conexion.into());
                m.insert("estado".into(), estado.into());
            }
            Causa::Servicio { estado, codigo } => {
                m.insert("estado".into(), estado.into());
                m.insert("codigo".into(), codigo.into());
            }
            Causa::Validacion(codigo) => {
                m.insert("codigo".into(), codigo.into());
            }
            Causa::Proceso { codigo } => {
                m.insert("codigo_salida".into(), codigo.into());
            }
            _ => {}
        }
        Value::Object(m)
    }
}

fn tipo_io(tipo: io::ErrorKind) -> &'static str {
    use io::ErrorKind as K;
    match tipo {
        K::NotFound => "no_encontrado",
        K::PermissionDenied => "permiso",
        K::AlreadyExists => "ya_existe",
        K::WouldBlock | K::ResourceBusy => "ocupado",
        K::StorageFull => "disco_lleno",
        K::ReadOnlyFilesystem => "solo_lectura",
        K::IsADirectory => "es_carpeta",
        K::NotADirectory => "no_es_carpeta",
        K::DirectoryNotEmpty => "carpeta_no_vacia",
        K::TimedOut => "timeout",
        K::Interrupted => "interrumpido",
        K::InvalidInput => "entrada_invalida",
        K::InvalidData => "datos_invalidos",
        K::UnexpectedEof => "fin_inesperado",
        K::WriteZero => "escritura_cero",
        K::Unsupported => "no_soportado",
        K::OutOfMemory => "sin_memoria",
        K::ConnectionRefused
        | K::ConnectionReset
        | K::ConnectionAborted
        | K::NotConnected
        | K::BrokenPipe => "conexion",
        _ => "otro",
    }
}

/// Gravedad de una línea.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Nivel {
    Info,
    Aviso,
    Error,
}

/// Clase de línea emitida.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TipoEvento {
    Inicio,
    Etapa,
    Dato,
    Fin,
    Abandono,
    Descartes,
    Panico,
}

impl TipoEvento {
    fn etiqueta(self) -> &'static str {
        match self {
            TipoEvento::Inicio => "inicio",
            TipoEvento::Etapa => "etapa",
            TipoEvento::Dato => "dato",
            TipoEvento::Fin => "fin",
            TipoEvento::Abandono => "abandono",
            TipoEvento::Descartes => "descartes",
            TipoEvento::Panico => "panico",
        }
    }
}

/// Ubicación del código: solo el nombre del archivo Rust y la línea.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Ubicacion {
    archivo: &'static str,
    linea: u32,
}

impl Ubicacion {
    #[track_caller]
    pub(crate) fn llamador() -> Self {
        let lugar = Location::caller();
        Self {
            archivo: nombre_base(lugar.file()),
            linea: lugar.line(),
        }
    }
}

/// Último componente de una ruta de código fuente, sin carpetas.
pub(crate) fn nombre_base(ruta: &str) -> &str {
    ruta.rsplit(|c| c == '/' || c == '\\').next().unwrap_or("")
}

pub(crate) fn ubicacion_json(archivo: &str, linea: u32) -> Value {
    let mut m = Map::new();
    m.insert("archivo".into(), nombre_base(archivo).into());
    m.insert("linea".into(), linea.into());
    Value::Object(m)
}

/// Datos fijos de la sesión que acompañan a cada línea.
#[derive(Debug, Clone)]
pub(crate) struct Cabecera {
    pub sesion: String,
    pub version: &'static str,
    pub compilacion: &'static str,
}

/// Evento ya capturado por un emisor; el escritor le asigna la secuencia.
#[derive(Debug, Clone)]
pub(crate) struct Evento {
    pub fecha: SystemTime,
    pub nivel: Nivel,
    pub componente: &'static str,
    pub accion: &'static str,
    pub operacion: Option<u64>,
    pub etapa: Option<&'static str>,
    pub tipo: TipoEvento,
    pub resultado: Option<&'static str>,
    pub causa: Option<Causa>,
    pub duracion_ms: Option<u64>,
    pub datos: Vec<Campo>,
    pub instancia: Option<Arc<str>>,
    pub ubicacion: Option<Ubicacion>,
}

impl Evento {
    /// Evento con la fecha de este instante y el resto de campos vacíos.
    pub(crate) fn nuevo(
        tipo: TipoEvento,
        nivel: Nivel,
        componente: &'static str,
        accion: &'static str,
    ) -> Self {
        Self {
            fecha: SystemTime::now(),
            nivel,
            componente,
            accion,
            operacion: None,
            etapa: None,
            tipo,
            resultado: None,
            causa: None,
            duracion_ms: None,
            datos: Vec::new(),
            instancia: None,
            ubicacion: None,
        }
    }

    /// Objeto JSON de la línea; `seq` es `None` en la ruta de emergencia.
    pub(crate) fn objeto(&self, cab: &Cabecera, seq: Option<u64>) -> Map<String, Value> {
        let mut m = Map::new();
        m.insert("esquema".into(), ESQUEMA.into());
        m.insert("fecha".into(), fecha_iso(self.fecha).into());
        m.insert("nivel".into(), nivel(self.nivel).into());
        m.insert("sesion".into(), cab.sesion.as_str().into());
        m.insert("seq".into(), seq.into());
        m.insert("version".into(), cab.version.into());
        m.insert("compilacion".into(), cab.compilacion.into());
        m.insert("componente".into(), self.componente.into());
        m.insert("accion".into(), self.accion.into());
        m.insert("operacion".into(), self.operacion.into());
        m.insert("etapa".into(), self.etapa.into());
        m.insert("evento".into(), self.tipo.etiqueta().into());
        m.insert("resultado".into(), self.resultado.into());
        m.insert(
            "causa".into(),
            self.causa.map(|c| c.json()).unwrap_or(Value::Null),
        );
        m.insert("duracion_ms".into(), self.duracion_ms.into());
        if let Some(alias) = &self.instancia {
            m.insert("instancia".into(), alias.as_ref().into());
        }
        if !self.datos.is_empty() {
            m.insert("datos".into(), datos_json(&self.datos));
        }
        if let Some(u) = self.ubicacion {
            m.insert("ubicacion".into(), ubicacion_json(u.archivo, u.linea));
        }
        m
    }

    pub(crate) fn linea(&self, cab: &Cabecera, seq: u64) -> Vec<u8> {
        linea_json(self.objeto(cab, Some(seq)))
    }
}

/// Serializa un objeto como una línea JSON terminada en `\n`.
pub(crate) fn linea_json(objeto: Map<String, Value>) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(&Value::Object(objeto)).unwrap_or_default();
    bytes.push(b'\n');
    bytes
}

fn nivel(nivel: Nivel) -> &'static str {
    match nivel {
        Nivel::Info => "info",
        Nivel::Aviso => "aviso",
        Nivel::Error => "error",
    }
}

fn datos_json(datos: &[Campo]) -> Value {
    let mut m = Map::new();
    for campo in datos {
        match *campo {
            Campo::Numero(nombre, valor) => m.insert(nombre.into(), valor.into()),
            Campo::Etiqueta(nombre, valor) => m.insert(nombre.into(), valor.into()),
            Campo::CodigoSalida(codigo) => m.insert("codigo_salida".into(), codigo.into()),
        };
    }
    Value::Object(m)
}

/// Fecha UTC ISO 8601 con milisegundos, sin depender del reloj de chrono.
pub(crate) fn fecha_iso(fecha: SystemTime) -> String {
    let desde = fecha.duration_since(UNIX_EPOCH).unwrap_or_default();
    let segundos = desde.as_secs();
    let (anio, mes, dia) = civil((segundos / 86_400) as i64);
    let resto = segundos % 86_400;
    format!(
        "{anio:04}-{mes:02}-{dia:02}T{:02}:{:02}:{:02}.{:03}Z",
        resto / 3_600,
        resto % 3_600 / 60,
        resto % 60,
        desde.subsec_millis()
    )
}

/// Días desde 1970-01-01 a fecha civil (algoritmo de H. Hinnant).
pub(crate) fn civil(dias: i64) -> (i64, u32, u32) {
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let dia = doy - (153 * mp + 2) / 5 + 1;
    let mes = if mp < 10 { mp + 3 } else { mp - 9 };
    let anio = yoe + era * 400 + i64::from(mes <= 2);
    (anio, mes as u32, dia as u32)
}
