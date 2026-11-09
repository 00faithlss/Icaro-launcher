// SPDX-License-Identifier: GPL-3.0-only
//! Encuentro y consulta verificada; no descarga contenido ni crea instancias.
use super::servicio::Servicio;
use icaro_core::compartir::{
    self as c,
    identidad::Identidad,
    invitaciones::{ClienteInvitaciones, Codigo, InvitacionResuelta},
};
use std::{path::PathBuf, sync::Arc};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub enum ErrorEntrada {
    Invalido,
    Vencido,
    Completo,
    Inaccesible(String),
}
impl From<String> for ErrorEntrada {
    fn from(e: String) -> Self {
        Self::Inaccesible(e)
    }
}
impl From<&str> for ErrorEntrada {
    fn from(e: &str) -> Self {
        Self::Inaccesible(e.into())
    }
}
impl From<c::Error> for ErrorEntrada {
    fn from(e: c::Error) -> Self {
        icaro_core::diagnostico_contexto::fallo(c::diagnostico::causa(&e));
        match e {
            c::Error::Servicio { estado: 404, .. } => Self::Invalido,
            c::Error::Servicio { codigo, .. } if codigo == "invitation_expired" => Self::Vencido,
            c::Error::Servicio { codigo, .. } if codigo == "sharing_stopped" => Self::Inaccesible(
                "El anfitrión dejó de compartir esta publicación. Pídele una invitación nueva."
                    .into(),
            ),
            c::Error::Servicio { codigo, .. } if codigo == "capacity_full" => Self::Completo,
            otro => Self::Inaccesible(otro.to_string()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Resuelta {
    pub identidad: Identidad,
    pub servicio: Servicio,
    pub codigo: Codigo,
    pub respuesta: InvitacionResuelta,
}
#[derive(Debug, Clone)]
pub struct Visita {
    pub invitacion: Resuelta,
    pub manifiesto: c::Manifiesto,
    pub confianza: c::confianza::EstadoConfianza,
}
pub async fn resolver(
    raiz: PathBuf,
    codigo: Codigo,
    servicio: Option<Servicio>,
    cancel: CancellationToken,
) -> Result<Resuelta, ErrorEntrada> {
    let cancel_registro = cancel.clone();
    icaro_core::diagnostico_contexto::observar(
        "compartir",
        "resolver_invitacion",
        None,
        &cancel_registro,
        resolver_interno(raiz, codigo, servicio, cancel),
    )
    .await
}

async fn resolver_interno(
    raiz: PathBuf,
    codigo: Codigo,
    servicio: Option<Servicio>,
    cancel: CancellationToken,
) -> Result<Resuelta, ErrorEntrada> {
    let servicio =
        servicio.ok_or_else(|| "El servicio de invitaciones no está configurado.".to_string())?;
    let identidad = icaro_core::diagnostico_contexto::bloqueante(move || {
        Identidad::cargar_o_crear(&raiz).map_err(icaro_core::diagnostico_contexto::texto)
    })
    .await
    .map_err(|_| "No se pudo leer la identidad del equipo.".to_string())??;
    resolver_con(identidad, codigo, servicio, cancel).await
}
pub async fn resolver_con(
    identidad: Identidad,
    codigo: Codigo,
    servicio: Servicio,
    cancel: CancellationToken,
) -> Result<Resuelta, ErrorEntrada> {
    let cliente = ClienteInvitaciones::nuevo(&servicio.url, identidad.clone())
        .map_err(icaro_core::diagnostico_contexto::texto)?;
    let respuesta = tokio::select! {
        _ = cancel.cancelled() => return Err("Consulta cancelada.".into()),
        r = cliente.resolver(&codigo) => r.map_err(ErrorEntrada::from)?,
    };
    respuesta
        .meeting
        .direccion(&respuesta.host, &respuesta.publication)
        .map_err(icaro_core::diagnostico_contexto::texto)?;
    Ok(Resuelta {
        identidad,
        servicio,
        codigo,
        respuesta,
    })
}
pub async fn consultar(
    raiz: PathBuf,
    invitacion: Resuelta,
    anfitrion: Option<Arc<c::anfitrion::PublicacionActiva>>,
    cancel: CancellationToken,
) -> Result<Visita, String> {
    let cancel_registro = cancel.clone();
    icaro_core::diagnostico_contexto::observar(
        "compartir",
        "aceptar_consultar",
        None,
        &cancel_registro,
        consultar_interno(raiz, invitacion, anfitrion, cancel),
    )
    .await
}

async fn consultar_interno(
    raiz: PathBuf,
    mut invitacion: Resuelta,
    anfitrion: Option<Arc<c::anfitrion::PublicacionActiva>>,
    cancel: CancellationToken,
) -> Result<Visita, String> {
    let cliente =
        ClienteInvitaciones::nuevo(&invitacion.servicio.url, invitacion.identidad.clone())
            .map_err(icaro_core::diagnostico_contexto::texto)?;
    let aceptada = tokio::select! {
        _ = cancel.cancelled() => return Err("Aceptación cancelada.".into()),
        r = cliente.aceptar(&invitacion.codigo) => r.map_err(icaro_core::diagnostico_contexto::texto)?,
    };
    if !aceptada.authorized
        || aceptada.host != invitacion.respuesta.host
        || aceptada.publication != invitacion.respuesta.publication
    {
        return Err("La identidad de la invitación cambió. Vuelve a comprobar el código.".into());
    }
    invitacion.respuesta = aceptada;
    let conexion = conectar(&invitacion, anfitrion, &cancel).await?;
    let resultado = tokio::select! {
        _ = cancel.cancelled() => Err("Consulta cancelada.".into()),
        r = conexion.consultar() => r.map_err(icaro_core::diagnostico_contexto::texto),
    };
    conexion.cerrar().await;
    let manifiesto = resultado?;
    let origen = manifiesto.origen.clone();
    let confianza = icaro_core::diagnostico_contexto::bloqueante(move || {
        c::confianza::consultar(&raiz, &origen).map_err(icaro_core::diagnostico_contexto::texto)
    })
    .await
    .map_err(|_| "No se pudo consultar la confianza guardada.".to_string())??;
    if cancel.is_cancelled() {
        return Err("Consulta cancelada.".into());
    }
    Ok(Visita {
        invitacion,
        manifiesto,
        confianza,
    })
}
pub async fn conectar(
    invitacion: &Resuelta,
    anfitrion: Option<Arc<c::anfitrion::PublicacionActiva>>,
    cancel: &CancellationToken,
) -> Result<c::transporte::Cliente, String> {
    let r = &invitacion.respuesta;
    let origen = c::Origen {
        anfitrion: r.host.clone(),
        publicacion: r.publication.clone(),
    };
    let direccion = r
        .meeting
        .direccion(&r.host, &r.publication)
        .map_err(icaro_core::diagnostico_contexto::texto)?;
    tokio::select! {
        _ = cancel.cancelled() => Err("Conexión cancelada.".into()),
        r = async {
            match anfitrion {
                Some(h) => h.conectar(&invitacion.respuesta.meeting, &origen).await,
                None => c::transporte::Cliente::conectar(&invitacion.identidad, direccion, &origen, invitacion.servicio.modo, c::Limites::default()).await,
            }
        } => r.map_err(icaro_core::diagnostico_contexto::texto),
    }
}
pub async fn confiar(raiz: PathBuf, visita: Visita, cambio: bool) -> Result<(), String> {
    icaro_core::diagnostico_contexto::bloqueante(move || {
        c::confianza::confirmar(&raiz, &visita.manifiesto.origen, cambio)
            .map_err(icaro_core::diagnostico_contexto::texto)
    })
    .await
    .map_err(|_| "No se pudo guardar la confianza.".to_string())?
}
pub async fn confiar_y_recuperar(
    raiz: PathBuf,
    store: icaro_core::instancias::Almacen,
    visita: Visita,
    cambio: bool,
) -> Result<Option<icaro_core::instancias::Instancia>, String> {
    let cancel_registro = tokio_util::sync::CancellationToken::new();
    icaro_core::diagnostico_contexto::observar(
        "compartir",
        "confiar_recuperar",
        None,
        &cancel_registro,
        confiar_y_recuperar_interno(raiz, store, visita, cambio),
    )
    .await
}

async fn confiar_y_recuperar_interno(
    raiz: PathBuf,
    store: icaro_core::instancias::Almacen,
    visita: Visita,
    cambio: bool,
) -> Result<Option<icaro_core::instancias::Instancia>, String> {
    confiar(raiz, visita.clone(), cambio).await?;
    icaro_core::diagnostico_contexto::bloqueante(move || {
        c::intentos::buscar(&store, &visita.manifiesto)
            .map_err(icaro_core::diagnostico_contexto::texto)
    })
    .await
    .map_err(|_| "No se pudo buscar una instalación incompleta.".to_string())?
}
