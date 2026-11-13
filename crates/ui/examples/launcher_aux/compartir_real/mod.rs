// SPDX-License-Identifier: GPL-3.0-only
//! Controlador experimental del publicador; no amplía el update del launcher.
mod acciones;
mod avance;
mod biblioteca;
mod biblioteca_tareas;
mod compartidas;
mod contadores;
mod estado;
mod invitaciones;
mod invitado;
mod invitado_diseno;
mod invitado_instalacion;
mod invitado_tareas;
mod invitado_vistas;
mod listo;
mod modelos;
#[cfg(test)]
mod pruebas;
#[cfg(test)]
mod pruebas_avance;
#[cfg(test)]
mod pruebas_biblioteca;
#[cfg(test)]
mod pruebas_invitado;
#[cfg(test)]
mod pruebas_worker;
mod resultados;
mod selector;
mod servicio;
mod sincronizar;
#[cfg(test)]
mod sync_pruebas;
mod sync_tareas;
mod sync_vistas;
#[cfg(test)]
mod sync_worker;
mod tareas;
mod vistas;
pub use acciones::{actualizar, Accion, Operacion};
pub use biblioteca::cargar as cargar_biblioteca;
pub use compartidas::modelo as modelo_compartidas;
pub use estado::{Estado, Etapa};
pub use invitado_diseno::preparar as preparar_invitado_diseno;
pub use modelos::tamano;
pub use servicio::Servicio;
pub use sincronizar::antes_de_jugar;
pub use vistas::fase_pura;
pub use vistas::transferencias;

/// Carga lo que recibes (y, al llegar, comprueba sus servidores) sin abrir
/// ninguna página ni cerrar un flujo en curso.
pub fn cargar_recibidas(app: &mut crate::App) -> iced::Task<crate::Mensaje> {
    if !app.servicios.compartir.habilitado || app.servicios.demo {
        return iced::Task::none();
    }
    compartidas::abrir(app, None)
}

/// Con amigos › Cambiar código: anula el código vigente y genera otro.
pub fn cambiar_codigo(s: &mut Estado) -> iced::Task<crate::Mensaje> {
    s.listo.regenerar = true;
    acciones::revocar_codigo(s)
}

/// Mensaje con instrucciones para el código vigente de la publicación abierta.
pub fn mensaje_codigo(s: &Estado) -> Option<String> {
    let p = s.panel.as_ref()?;
    let codigo = p.invitacion.as_ref()?.codigo.clone();
    Some(listo::mensaje(&p.instancia.nombre, &codigo, s.listo.vence))
}
pub use invitado::puede_confiar;
pub use listo::modelo as modelo_listo;
pub use selector::modelo as modelo_selector;

/// Datos de la invitación ya consultada al anfitrión: nombre sugerido,
/// perfil, cantidad de mods, bytes, revisión y huella.
pub struct DatosVisita {
    pub minecraft: String,
    pub loader: String,
    pub mods: usize,
    pub bytes: u64,
    pub revision: u64,
    pub huella: String,
}

pub fn datos_visita(s: &Estado) -> Option<DatosVisita> {
    let m = &s.invitado.visita.as_ref()?.manifiesto;
    Some(DatosVisita {
        minecraft: m.perfil.minecraft.clone(),
        loader: match &m.perfil.loader_version {
            Some(v) => format!("{} {v}", m.perfil.loader),
            None => m.perfil.loader.clone(),
        },
        mods: m.archivos.iter().filter(|a| a.ruta.as_str().starts_with("mods/")).count(),
        bytes: m.archivos.iter().map(|a| a.bytes).sum(),
        revision: m.revision,
        huella: icaro_core::compartir::identidad::huella_publica(&m.origen.anfitrion).unwrap_or_default(),
    })
}
