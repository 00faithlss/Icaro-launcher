// SPDX-License-Identifier: GPL-3.0-only
//! Ciclo de vida del registro: fuera de iced::run; avisos solo consultan memoria.
use icaro_core::diagnostico::{self, Campo, ErrorRegistro, Medicion, Registro, Resultado, Sesion};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    OnceLock,
};

static REGISTRO: OnceLock<Registro> = OnceLock::new();
static AVISADO: AtomicBool = AtomicBool::new(false);

pub struct Inicio {
    pub sesion: Option<Sesion>,
    pub fallo: Option<ErrorRegistro>,
}
pub fn iniciar() -> Inicio {
    if std::env::var("ICARO_DEMO").as_deref() == Ok("1")
        || std::env::var_os("ICARO_CAPTURA_DISENO").is_some()
    {
        return Inicio {
            sesion: None,
            fallo: None,
        };
    }
    match diagnostico::iniciar(
        &crate::estado::directorio(false),
        env!("CARGO_PKG_VERSION"),
        env!("ICARO_COMPILACION"),
    ) {
        Ok(sesion) => {
            let registro = sesion.registro();
            sesion.instalar_global();
            sesion.instalar_panico();
            let _ = REGISTRO.set(registro);
            let op = Medicion::global("launcher", "inicio");
            op.dato(Campo::Etiqueta("sistema", std::env::consts::OS));
            op.terminar(Resultado::Exito);
            Inicio {
                sesion: Some(sesion),
                fallo: None,
            }
        }
        Err(error) => Inicio {
            sesion: None,
            fallo: Some(error),
        },
    }
}
pub fn aviso_inicio(error: ErrorRegistro) -> String {
    let codigo = match error {
        ErrorRegistro::Archivo(diagnostico::Causa::Disco {
            codigo_os: Some(codigo),
            ..
        }) => format!(" Código del sistema: {codigo}."),
        _ => String::new(),
    };
    format!("No se pudieron iniciar los registros: {error}.{codigo} Comprueba el espacio y los permisos de la carpeta de datos.")
}
pub fn activo() -> bool {
    REGISTRO.get().is_some()
}
pub fn aviso_salud() -> Option<&'static str> {
    let estado = REGISTRO.get()?.estado();
    if (estado.errores_escritura > 0 || estado.descartados > 0 || estado.cerrado)
        && !AVISADO.swap(true, Ordering::AcqRel)
    {
        Some("No se pudieron conservar todos los registros de diagnóstico. Comprueba el espacio y los permisos de la carpeta de datos.")
    } else {
        None
    }
}
pub fn terminar(inicio: &mut Inicio, exito: bool) {
    if let Some(sesion) = &mut inicio.sesion {
        let op = Medicion::global("launcher", "cierre");
        op.terminar(if exito {
            Resultado::Exito
        } else {
            Resultado::Error(diagnostico::Causa::Interno)
        });
        let _ = sesion.cerrar(std::time::Duration::from_millis(750));
    }
}
