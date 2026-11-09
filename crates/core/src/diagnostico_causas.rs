// SPDX-License-Identifier: GPL-3.0-only
//! Traducción de errores del núcleo sin serializar sus cadenas dinámicas.
use crate::{diagnostico::Causa, http, instancias, java, lanzamiento, minecraft};
use std::any::Any;

pub(crate) fn desde(error: &dyn Any) -> Option<Causa> {
    if let Some(e) = error.downcast_ref::<minecraft::Error>() {
        Some(minecraft(e))
    } else if let Some(e) = error.downcast_ref::<java::Error>() {
        Some(java(e))
    } else if let Some(e) = error.downcast_ref::<instancias::Error>() {
        Some(instancia(e))
    } else if let Some(e) = error.downcast_ref::<lanzamiento::Error>() {
        Some(lanzamiento(e))
    } else if let Some(e) = error.downcast_ref::<http::Error>() {
        Some(http(e))
    } else {
        None
    }
}
fn disco() -> Causa {
    Causa::Disco {
        tipo: "sin_detalle",
        codigo_os: None,
    }
}
fn red() -> Causa {
    Causa::Red {
        timeout: false,
        conexion: true,
        estado: None,
    }
}
fn http(e: &http::Error) -> Causa {
    match e {
        http::Error::Network(_) => red(),
        http::Error::Status { status, .. } => Causa::Red {
            timeout: false,
            conexion: false,
            estado: Some(*status),
        },
        http::Error::RateLimited(_) => Causa::Red {
            timeout: false,
            conexion: false,
            estado: Some(429),
        },
        http::Error::Response(_) => Causa::Validacion("respuesta_http"),
    }
}

fn instancia(e: &instancias::Error) -> Causa {
    match e {
        instancias::Error::Disco => disco(),
        instancias::Error::Nombre => Causa::Validacion("nombre_instancia"),
        instancias::Error::Repetida => Causa::Validacion("instancia_repetida"),
        instancias::Error::Datos => Causa::Validacion("datos_instancia"),
        instancias::Error::Papelera(_) => Causa::Validacion("papelera"),
    }
}
fn minecraft(e: &minecraft::Error) -> Causa {
    match e {
        minecraft::Error::Cancelada => Causa::Validacion("cancelada"),
        minecraft::Error::Ocupada => Causa::Conflicto,
        minecraft::Error::Disco => disco(),
        minecraft::Error::Red => red(),
        minecraft::Error::Http(estado) => Causa::Red {
            timeout: false,
            conexion: false,
            estado: Some(*estado),
        },
        minecraft::Error::Integridad => Causa::Integridad,
        minecraft::Error::Datos => Causa::Validacion("metadatos_minecraft"),
        minecraft::Error::NoSoportada(_) => Causa::Validacion("minecraft_no_soportada"),
        minecraft::Error::Version => Causa::Validacion("version_minecraft"),
        minecraft::Error::Instalador(_) => Causa::Validacion("instalador_loader"),
        minecraft::Error::FabricApi(_) => Causa::Validacion("fabric_api"),
    }
}
fn java(e: &java::Error) -> Causa {
    match e {
        java::Error::Cancelada => Causa::Validacion("cancelada"),
        java::Error::Disco => disco(),
        java::Error::Hash => Causa::Integridad,
        java::Error::Datos => Causa::Validacion("metadatos_java"),
        java::Error::Ejecutable => Causa::Validacion("ejecutable_java"),
        java::Error::Incompatible { .. } => Causa::Validacion("java_incompatible"),
        java::Error::PerfilIncompatible { .. } => Causa::Validacion("perfil_java"),
        java::Error::PruebaJvm(_) => Causa::Validacion("prueba_jvm"),
        java::Error::Arquitectura { .. } => Causa::Validacion("arquitectura_java"),
        java::Error::DescargaDesactivada(_) => Causa::Validacion("descarga_java_desactivada"),
        java::Error::NoDisponible(_) => Causa::Validacion("java_no_disponible"),
        java::Error::Plataforma => Causa::Validacion("plataforma_java"),
        java::Error::Red(http::Error::Status { status, .. }) => Causa::Red {
            timeout: false,
            conexion: false,
            estado: Some(*status),
        },
        java::Error::Red(http::Error::RateLimited(_)) => Causa::Red {
            timeout: false,
            conexion: false,
            estado: Some(429),
        },
        java::Error::Red(http::Error::Network(_)) => red(),
        java::Error::Red(http::Error::Response(_)) => Causa::Validacion("respuesta_java"),
    }
}
pub(crate) fn lanzamiento(e: &lanzamiento::Error) -> Causa {
    use lanzamiento::Error as E;
    match e {
        E::Cancelado => Causa::Validacion("cancelada"),
        E::Cuenta => Causa::Credencial,
        E::Carpeta | E::Registros => disco(),
        E::Skin => Causa::Validacion("skin_local"),
        E::Memoria => Causa::Validacion("memoria"),
        E::Resolucion => Causa::Validacion("resolucion"),
        E::ArgumentosJvm => Causa::Validacion("argumentos_jvm"),
        E::ArgumentoProtegido => Causa::Validacion("argumento_protegido"),
        E::Instalacion => Causa::Validacion("instalacion_minecraft"),
        E::Servidor => Causa::Validacion("direccion_servidor"),
        E::Java => Causa::Validacion("ejecutar_java"),
        E::Espera => Causa::Validacion("esperar_java"),
        E::Detener => Causa::Validacion("detener_java"),
    }
}
