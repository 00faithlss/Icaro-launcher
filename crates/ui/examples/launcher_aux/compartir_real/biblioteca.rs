// SPDX-License-Identifier: GPL-3.0-only
use super::{biblioteca_tareas, Accion, Estado, Etapa};
use crate::{App, Mensaje};
use icaro_core::compartir::biblioteca::{EstadoGuardada, PublicacionGuardada};
#[cfg(test)]
use icaro_core::instancias::Instancia;
use icaro_ui::compartir as v;
use iced::Task;

#[derive(Default)]
pub struct Biblioteca {
    pub generacion: u64,
    pub cargando: bool,
    pub entradas: Vec<PublicacionGuardada>,
    pub error: Option<String>,
    pub contadores: u64,
    pub cancel_contadores: tokio_util::sync::CancellationToken,
    pendiente_reconexion: bool,
}
impl std::fmt::Debug for Biblioteca {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Biblioteca")
            .field("generacion", &self.generacion)
            .field("entradas", &self.entradas.len())
            .field("cargando", &self.cargando)
            .finish()
    }
}
impl Biblioteca {
    /// Tras pausar, abrir la publicación la reconecta sin revocar a nadie.
    pub fn marcar_reconexion(&mut self) {
        self.pendiente_reconexion = true;
    }
}
pub fn pedir(s: &mut Estado) -> u64 {
    s.biblioteca.generacion = s.biblioteca.generacion.wrapping_add(1);
    s.biblioteca.cargando = true;
    s.biblioteca.generacion
}
pub fn cargar(app: &mut App) -> Task<Mensaje> {
    if app.servicios.demo || !app.servicios.compartir.habilitado {
        return Task::none();
    }
    let generacion = pedir(&mut app.servicios.compartir);
    let almacen = app.servicios.instancias.almacen.clone();
    Task::perform(
        biblioteca_tareas::listar_con(crate::estado::directorio(false), almacen, None),
        move |r| Mensaje::Compartir(Accion::Biblioteca(generacion, r)),
    )
}
pub fn recibida(
    s: &mut Estado,
    generacion: u64,
    resultado: Result<Vec<PublicacionGuardada>, String>,
) {
    if generacion != s.biblioteca.generacion {
        return;
    }
    s.biblioteca.cargando = false;
    match resultado {
        Ok(entradas) => {
            s.biblioteca.entradas = entradas;
            s.biblioteca.error = None;
        }
        Err(e) => s.biblioteca.error = Some(e),
    }
}
#[cfg(test)]
pub fn resumenes(s: &Estado, instancias: &[Instancia]) -> Vec<v::ResumenPublicacion> {
    let mut salida: Vec<_> = s
        .biblioteca
        .entradas
        .iter()
        .filter(|e| {
            matches!(
                e.estado,
                EstadoGuardada::Lista | EstadoGuardada::Detenida | EstadoGuardada::PerfilCambiado
            )
        })
        .filter_map(|e| {
            let instancia = instancias.iter().find(|i| i.id == e.instancia)?;
            Some(v::ResumenPublicacion {
                id: instancia.id.clone(),
                instancia: instancia.nombre.clone(),
                revision: e.revision?,
                sesiones: e.autorizados,
                transferencias: 0,
            })
        })
        .collect();
    if let Some(p) = &s.panel {
        if instancias.iter().any(|i| i.id == p.instancia.id) {
            salida.retain(|r| r.id != p.instancia.id);
            salida.push(v::ResumenPublicacion {
                id: p.instancia.id.clone(),
                instancia: p.instancia.nombre.clone(),
                revision: p.revision,
                sesiones: p.accesos.personas.len(),
                transferencias: s.anfitrion.as_ref().map_or(0, |h| h.transferencias()),
            });
        }
    }
    salida
}
pub fn abrir(app: &mut App, id: String) -> Task<Mensaje> {
    let s = &mut app.servicios.compartir;
    if s.red_ocupada || matches!(s.etapa, Etapa::Publicando | Etapa::Guardando) {
        return Task::none();
    }
    if s.publicacion.as_ref().is_some_and(|p| p.instancia_id == id)
        && s.panel.as_ref().is_some_and(|p| p.instancia.id == id)
    {
        s.etapa = Etapa::Panel;
        if s.biblioteca.pendiente_reconexion && s.anfitrion.is_none() {
            return reconectar(s);
        }
        return super::contadores::pedir(s).unwrap_or_else(Task::none);
    }
    if s.anfitrion.is_some() {
        s.error =
            Some("Deja de compartir la instancia actual antes de abrir otra publicación.".into());
        return Task::none();
    }
    let Some(entrada) = s
        .biblioteca
        .entradas
        .iter()
        .find(|e| {
            e.instancia == id
                && e.revision.is_some()
                && !matches!(
                    e.estado,
                    EstadoGuardada::SinRevision | EstadoGuardada::Ilegible
                )
        })
        .cloned()
    else {
        return Task::none();
    };
    let Some(instancia) = app
        .servicios
        .instancias
        .registros
        .iter()
        .find(|i| i.id == id)
        .cloned()
    else {
        return Task::none();
    };
    let solicitud = s.comenzar(id);
    s.publicacion = None;
    s.biblioteca.pendiente_reconexion = false;
    s.etapa = Etapa::Panel;
    s.red_ocupada = true;
    s.panel = Some(v::PanelPublicacion {
        instancia: super::modelos::instancia(&instancia, false),
        revision: entrada.revision.unwrap_or(0),
        fecha_revision: String::new(),
        hash_revision: String::new(),
        disponibilidad: "Recuperando publicación…".into(),
        transferencias: "Sin transferencias activas".into(),
        huella: String::new(),
        contenido_privado: 0,
        estado: if entrada.estado == EstadoGuardada::Detenida {
            v::EstadoPublicacion::Detenida
        } else {
            v::EstadoPublicacion::Compartiendo
        },
        invitacion: None,
        accesos: v::Accesos::default(),
    });
    let almacen = app.servicios.instancias.almacen.clone();
    Task::perform(
        biblioteca_tareas::recuperar_con(
            crate::estado::directorio(false),
            almacen,
            instancia,
            None,
        ),
        move |r| Mensaje::Compartir(Accion::Recuperada(solicitud, r)),
    )
}
fn cierre_pendiente(s: &mut Estado) -> bool {
    if s.cerrar_al_terminar {
        s.red_ocupada = false;
        s.cerrar_al_terminar = false;
        s.cerrar();
        true
    } else {
        false
    }
}
pub fn recuperada(
    s: &mut Estado,
    id: u64,
    resultado: Result<biblioteca_tareas::Recuperada, String>,
) -> bool {
    if !s.vigente(id) {
        return false;
    }
    s.red_ocupada = false;
    let activa = match resultado {
        Ok(r) => {
            if let Some(p) = &mut s.panel {
                p.revision = r.revision.manifiesto.revision;
                p.huella = r.local.identidad.huella();
                p.estado = if r.detenida {
                    v::EstadoPublicacion::Detenida
                } else {
                    v::EstadoPublicacion::Compartiendo
                };
                p.disponibilidad = if r.detenida {
                    "Publicación detenida"
                } else {
                    "Reconectando publicación…"
                }
                .into();
            }
            s.publicacion = Some(r.local);
            !r.detenida
        }
        Err(e) => {
            if let Some(p) = &mut s.panel {
                p.estado = v::EstadoPublicacion::Error(e);
            }
            false
        }
    };
    s.biblioteca.pendiente_reconexion = activa;
    if cierre_pendiente(s) {
        return false;
    }
    s.red_ocupada = activa;
    activa
}
pub fn reconectar(s: &mut Estado) -> Task<Mensaje> {
    let Some(local) = s.publicacion.clone() else {
        return Task::none();
    };
    s.red_ocupada = true;
    let id = s.solicitud;
    Task::perform(
        biblioteca_tareas::reconectar_con(
            local,
            s.caducidad,
            s.cupo,
            super::servicio::Servicio::desde_entorno(),
        ),
        move |r| Mensaje::Compartir(Accion::Reconectada(id, r)),
    )
}
pub fn reconectada(
    s: &mut Estado,
    id: u64,
    resultado: Result<biblioteca_tareas::Reconectada, String>,
) {
    if !s.vigente(id) {
        return;
    }
    s.red_ocupada = false;
    match resultado {
        Ok((h, i)) => {
            s.biblioteca.pendiente_reconexion = false;
            s.anfitrion = Some(h);
            if let Some(p) = &mut s.panel {
                p.estado = v::EstadoPublicacion::Compartiendo;
                p.disponibilidad = "Disponible mientras Ícaro esté abierto".into();
            }
            super::resultados::invitacion(s, id, Ok(i));
        }
        Err(e) => {
            s.anfitrion = None;
            if let Some(p) = &mut s.panel {
                p.estado = v::EstadoPublicacion::Error(e);
            }
        }
    }
    cierre_pendiente(s);
}
pub fn accion(app: &mut App, accion: &v::Accion) -> Option<Task<Mensaje>> {
    if let v::Accion::VerPublicacionDe(id) = accion {
        return Some(abrir(app, id.clone()));
    }
    let s = &mut app.servicios.compartir;
    if matches!(accion, v::Accion::Reintentar)
        && s.etapa == Etapa::Panel
        && s.publicacion.is_some()
        && s.anfitrion.is_none()
        && !s.red_ocupada
        && !s
            .panel
            .as_ref()
            .is_some_and(|p| matches!(p.estado, v::EstadoPublicacion::Detenida))
    {
        return Some(reconectar(s));
    }
    None
}
