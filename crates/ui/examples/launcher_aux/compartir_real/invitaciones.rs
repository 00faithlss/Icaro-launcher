// SPDX-License-Identifier: GPL-3.0-only
//! Edición del mismo código y reapertura explícita; nunca publicar al cambiar un ajuste.
use super::{
    acciones::{caducidad, Accion},
    servicio::Servicio,
    Estado,
};
use crate::Mensaje;
use icaro_core::compartir::{
    self as c,
    invitaciones::{ClienteInvitaciones, Codigo, InvitacionResuelta},
};
use icaro_ui::compartir as v;
use iced::Task;

fn datos_publicacion(s: &Estado) -> Option<super::tareas::PublicacionLocal> {
    s.publicacion
        .as_ref()
        .filter(|local| {
            s.panel
                .as_ref()
                .is_some_and(|p| p.instancia.id == local.instancia_id)
        })
        .cloned()
}

pub fn ajustar(s: &mut Estado, cad: v::Caducidad, cupo: u32) -> Task<Mensaje> {
    if s.red_ocupada || cupo == 0 {
        return Task::none();
    }
    let Some(i) = s.panel.as_ref().and_then(|p| p.invitacion.as_ref()) else {
        return Task::none();
    };
    if cupo < i.aceptados || !matches!(i.estado, v::EstadoInvitacion::Vigente { .. }) {
        return Task::none();
    }
    let Ok(codigo) = Codigo::parse(&i.codigo) else {
        return Task::none();
    };
    let Some(t) = datos_publicacion(s) else {
        return Task::none();
    };
    let identidad = t.identidad.clone();
    let id = s.solicitud;
    s.red_ocupada = true;
    if let Some(i) = s.panel.as_mut().and_then(|p| p.invitacion.as_mut()) {
        i.ajustes_editables = false;
    }
    Task::perform(
        async move { ajustar_con(identidad, codigo, cad, cupo, Servicio::desde_entorno()).await },
        move |r| Mensaje::Compartir(Accion::Ajustada(id, cad, r)),
    )
}
pub async fn ajustar_con(
    identidad: c::identidad::Identidad,
    codigo: Codigo,
    cad: v::Caducidad,
    cupo: u32,
    servicio: Option<Servicio>,
) -> Result<InvitacionResuelta, String> {
    let Servicio { url, .. } =
        servicio.ok_or_else(|| "El servicio de invitaciones no está configurado.".to_string())?;
    ClienteInvitaciones::nuevo(&url, identidad)
        .map_err(|e| e.to_string())?
        .actualizar(&codigo, caducidad(cad), cupo, None)
        .await
        .map_err(|e| e.to_string())
}
pub fn ajustada(s: &mut Estado, id: u64, cad: v::Caducidad, r: Result<InvitacionResuelta, String>) {
    if !s.vigente(id) {
        return;
    }
    s.red_ocupada = false;
    if let Some(p) = &mut s.panel {
        if let Some(i) = &mut p.invitacion {
            i.ajustes_editables = true;
            match r {
                Ok(r) => {
                    i.caducidad = cad;
                    i.cupo = r.capacity;
                    i.aceptados = r.accepted;
                    s.caducidad = cad;
                    s.cupo = r.capacity;
                }
                Err(e) => {
                    p.disponibilidad = format!("No se cambiaron los ajustes de la invitación: {e}")
                }
            }
        }
    }
    if s.cerrar_al_terminar {
        s.cerrar_al_terminar = false;
        s.cerrar();
    }
}
pub fn reanudar(s: &mut Estado) -> Task<Mensaje> {
    if s.red_ocupada || s.anfitrion.is_some() {
        return Task::none();
    }
    let Some(t) = datos_publicacion(s) else {
        return Task::none();
    };
    let (id, cad, cupo) = (s.solicitud, s.caducidad, s.cupo);
    s.red_ocupada = true;
    Task::perform(
        async move { reanudar_con(t, cad, cupo, Servicio::desde_entorno()).await },
        move |r| Mensaje::Compartir(Accion::Reanudada(id, r)),
    )
}
pub async fn reanudar_con(
    t: super::tareas::PublicacionLocal,
    cad: v::Caducidad,
    cupo: u32,
    servicio: Option<Servicio>,
) -> Result<
    (
        std::sync::Arc<c::anfitrion::PublicacionActiva>,
        c::invitaciones::InvitacionCreada,
    ),
    String,
> {
    let Servicio { url, modo } =
        servicio.ok_or_else(|| "El servicio de invitaciones no está configurado.".to_string())?;
    let contexto = t.contexto.clone();
    let revision = tokio::task::spawn_blocking(move || {
        c::revisiones::vigente(&contexto).map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| "No se pudo leer la publicación guardada.".to_string())??
    .ok_or_else(|| "No hay una revisión publicada para reabrir.".to_string())?;
    let h = std::sync::Arc::new(
        c::anfitrion::PublicacionActiva::iniciar(
            t.identidad,
            revision,
            &t.contexto.almacen.join("accesos.json"),
            &url,
            modo,
            t.contexto.limites,
        )
        .await
        .map_err(|e| e.to_string())?,
    );
    match h.volver_a_compartir(caducidad(cad), cupo).await {
        Ok(i) => Ok((h, i)),
        Err(e) => {
            h.cerrar().await;
            Err(e.to_string())
        }
    }
}
