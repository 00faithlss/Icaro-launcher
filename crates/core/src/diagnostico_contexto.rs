// SPDX-License-Identifier: GPL-3.0-only
//! Contexto por futuro y propagación explícita a tareas bloqueantes.
//! Solo se encolan datos tipados: jamás se serializa el error de Display.
use crate::diagnostico::{Campo, Causa, Medicion, Registro, Resultado};
use std::{
    cell::RefCell,
    future::Future,
    panic::Location,
    sync::{Arc, Mutex},
};
use tokio_util::sync::CancellationToken;

struct Contexto {
    medicion: Medicion,
    causa: Mutex<Option<Causa>>,
}
struct GuardiaOperacion(Arc<Contexto>);
impl Drop for GuardiaOperacion {
    fn drop(&mut self) {
        self.0.medicion.abandonar();
    }
}
tokio::task_local! { static ASINCRONO: Arc<Contexto>; }
thread_local! { static BLOQUEANTE: RefCell<Option<Arc<Contexto>>> = const { RefCell::new(None) }; }

fn actual() -> Option<Arc<Contexto>> {
    BLOQUEANTE
        .with(|c| c.borrow().clone())
        .or_else(|| ASINCRONO.try_with(Arc::clone).ok())
}

pub fn etapa(nombre: &'static str) {
    if let Some(c) = actual() {
        if let Ok(mut causa) = c.causa.lock() {
            *causa = None;
        }
        c.medicion.etapa(nombre);
    }
}
/// Un reintento nuevo no debe heredar la causa de una respuesta anterior.
pub fn nuevo_intento() {
    if let Some(c) = actual() {
        if let Ok(mut causa) = c.causa.lock() {
            *causa = None;
        };
    }
}
pub fn dato(campo: Campo) {
    if let Some(c) = actual() {
        c.medicion.dato(campo);
    }
}
pub fn instancia(id: &str) {
    if let Some(c) = actual() {
        c.medicion.instancia(id);
    }
}

/// Devuelve el texto existente para UI y captura únicamente propiedades seguras
/// de tipos conocidos. Un String desconocido no se vuelca al registro.
#[track_caller]
pub fn texto<E: std::fmt::Display + 'static>(error: E) -> String {
    registrar_error(&error);
    let texto = error.to_string();
    // El mensaje completo, con su ruta, queda en el registro legible local.
    crate::diagnostico::anotar(crate::diagnostico::Gravedad::Error, "nucleo", &texto);
    texto
}
#[track_caller]
pub fn registrar_error<E: 'static>(error: &E) {
    let tipo = error as &dyn std::any::Any;
    if let Some(e) = tipo.downcast_ref::<crate::compartir::Error>() {
        fallo(crate::compartir::diagnostico::causa(e));
    } else if let Some(e) = tipo.downcast_ref::<std::io::Error>() {
        fallo(Causa::de_io(e));
    } else if let Some(e) = tipo.downcast_ref::<reqwest::Error>() {
        fallo(Causa::de_red(e));
    } else if let Some(e) = tipo.downcast_ref::<tokio::task::JoinError>() {
        fallo(if e.is_panic() {
            Causa::Panico
        } else {
            Causa::Interno
        });
    } else if let Some(causa) = crate::diagnostico_causas::desde(tipo) {
        fallo(causa);
    }
}

/// Captura la causa antes de que un servicio la convierta en un mensaje de UI.
#[track_caller]
pub fn fallo(causa: Causa) {
    let sitio = Location::caller();
    let archivo = sitio
        .file()
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("desconocido");
    let Some(c) = actual() else {
        let op = Medicion::global("nucleo", "incidente");
        op.etapa("captura_error");
        op.dato(Campo::Etiqueta("origen", archivo));
        op.dato(Campo::Numero("linea", sitio.line() as u64));
        op.terminar(Resultado::Error(causa));
        return;
    };
    c.medicion.dato(Campo::Etiqueta("origen", archivo));
    c.medicion.dato(Campo::Numero("linea", sitio.line() as u64));
    // La causa completa se conserva para el resultado. El incidente también
    // deja una marca si el llamante se recupera y finalmente tiene éxito.
    c.medicion
        .dato(Campo::Etiqueta("incidente", "causa_capturada"));
    match &causa {
        Causa::Disco { tipo, codigo_os } => {
            c.medicion.dato(Campo::Etiqueta("causa", "disco"));
            c.medicion.dato(Campo::Etiqueta("tipo_io", tipo));
            if let Some(codigo) = codigo_os {
                c.medicion.dato(Campo::Numero(
                    "codigo_os_absoluto",
                    i64::from(*codigo).unsigned_abs(),
                ));
            }
        }
        Causa::Red {
            timeout,
            conexion,
            estado,
        } => {
            c.medicion.dato(Campo::Etiqueta("causa", "red"));
            c.medicion
                .dato(Campo::Numero("timeout", u64::from(*timeout)));
            c.medicion
                .dato(Campo::Numero("conexion", u64::from(*conexion)));
            if let Some(estado) = estado {
                c.medicion.dato(Campo::Numero("http", *estado as u64));
            }
        }
        Causa::Servicio { estado, codigo } => {
            c.medicion.dato(Campo::Etiqueta("causa", "servicio"));
            c.medicion.dato(Campo::Numero("http", *estado as u64));
            c.medicion.dato(Campo::Etiqueta("codigo_servicio", codigo));
        }
        Causa::Validacion(detalle) => {
            c.medicion.dato(Campo::Etiqueta("validacion", detalle));
        }
        _ => (),
    }
    if let Ok(mut ultima) = c.causa.lock() {
        if ultima.is_none() || precision(&causa) > 0 {
            *ultima = Some(causa);
        }
    };
}

fn precision(causa: &Causa) -> u8 {
    match causa {
        Causa::Panico => 4,
        Causa::Disco {
            codigo_os: Some(_), ..
        }
        | Causa::Servicio { .. } => 3,
        Causa::Disco {
            tipo: "sin_detalle",
            ..
        } => 0,
        Causa::Red {
            timeout: false,
            estado: None,
            ..
        } => 0,
        Causa::Interno => 0,
        Causa::Validacion("respuesta_http") => 0,
        _ => 2,
    }
}

pub async fn observar<T, E, F>(
    componente: &'static str,
    accion: &'static str,
    id: Option<&str>,
    cancel: &CancellationToken,
    futuro: F,
) -> Result<T, E>
where
    F: Future<Output = Result<T, E>>,
{
    observar_valor(componente, accion, id, cancel, futuro, Result::is_ok).await
}

pub async fn observar_valor<T, F>(
    componente: &'static str,
    accion: &'static str,
    id: Option<&str>,
    cancel: &CancellationToken,
    futuro: F,
    exito: impl FnOnce(&T) -> bool,
) -> T
where
    F: Future<Output = T>,
{
    // Los servicios llamados desde otra operación pertenecen a esa misma
    // referencia; no sustituyen un contexto inyectado por un global vacío.
    if actual().is_some() {
        return futuro.await;
    }
    ejecutar(
        Medicion::global(componente, accion),
        id,
        cancel,
        futuro,
        exito,
    )
    .await
}

pub async fn observar_con<T, E, F>(
    registro: &Registro,
    componente: &'static str,
    accion: &'static str,
    cancel: &CancellationToken,
    futuro: F,
) -> Result<T, E>
where
    F: Future<Output = Result<T, E>>,
{
    ejecutar(
        registro.medir(componente, accion),
        None,
        cancel,
        futuro,
        Result::is_ok,
    )
    .await
}

async fn ejecutar<T, F>(
    medicion: Medicion,
    id: Option<&str>,
    cancel: &CancellationToken,
    futuro: F,
    exito: impl FnOnce(&T) -> bool,
) -> T
where
    F: Future<Output = T>,
{
    medicion.etapa("preparacion");
    if let Some(id) = id {
        medicion.instancia(id);
    }
    let c = Arc::new(Contexto {
        medicion,
        causa: Mutex::new(None),
    });
    let _guardia = GuardiaOperacion(c.clone());
    let resultado = ASINCRONO.scope(c.clone(), futuro).await;
    let causa = c.causa.lock().ok().and_then(|mut c| c.take());
    let cancelada = cancel.is_cancelled()
        && causa
            .is_none_or(|causa| matches!(causa, Causa::Validacion("cancelada" | "auth_cancelada")));
    let fin = if exito(&resultado) {
        Resultado::Exito
    } else if cancelada {
        Resultado::Cancelado
    } else {
        Resultado::Error(causa.unwrap_or(Causa::Interno))
    };
    c.medicion.terminar(fin);
    resultado
}

/// Conserva el contexto al cambiar de futuro a hilo. La guardia restaura cualquier
/// contexto previo incluso si el cierre hace panic; no contamina otro trabajo.
pub async fn bloqueante<T, F>(trabajo: F) -> Result<T, tokio::task::JoinError>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let contexto = actual();
    let r = tokio::task::spawn_blocking(move || {
        struct Restaurar(Option<Arc<Contexto>>);
        impl Drop for Restaurar {
            fn drop(&mut self) {
                BLOQUEANTE.with(|c| {
                    c.replace(self.0.take());
                });
            }
        }
        let _anterior = Restaurar(BLOQUEANTE.with(|c| c.replace(contexto)));
        trabajo()
    })
    .await;
    if let Err(e) = &r {
        fallo(if e.is_panic() {
            Causa::Panico
        } else {
            Causa::Interno
        });
    }
    r
}
