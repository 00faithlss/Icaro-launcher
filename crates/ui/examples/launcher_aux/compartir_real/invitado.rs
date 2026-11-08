// SPDX-License-Identifier: GPL-3.0-only
//! Estado y transiciones del invitado; las operaciones viven en tareas.
use super::{invitado_tareas as tareas, servicio::Servicio, Accion, Estado, Etapa};
use crate::{App, Mensaje};
use icaro_core::compartir::{self as c, invitaciones::Codigo};
use icaro_ui::compartir as v;
use iced::Task;

pub struct Invitado {
    pub entrada: v::EstadoEntrada,
    pub resuelta: Option<tareas::Resuelta>,
    pub visita: Option<tareas::Visita>,
    pub confianza: v::Confianza,
    pub confirmar_nueva: bool,
    pub nombre: String,
    pub error: Option<String>,
    pub destino: Option<icaro_core::instancias::Instancia>,
    pub destino_vinculo: Option<c::vinculos::Vinculo>,
    pub completada: bool,
    pub revisar_nueva: bool,
    pub ver_detalle: bool,
    pub cambio_perfil: bool,
}
impl Default for Invitado {
    fn default() -> Self {
        Self {
            entrada: v::EstadoEntrada::Vacio,
            resuelta: None,
            visita: None,
            confianza: v::Confianza::PrimeraVez,
            confirmar_nueva: false,
            nombre: "Instancia compartida".into(),
            error: None,
            destino: None,
            destino_vinculo: None,
            completada: false,
            revisar_nueva: false,
            ver_detalle: false,
            cambio_perfil: false,
        }
    }
}
pub fn abrir(s: &mut Estado) {
    s.cancel.cancel();
    s.cancel = tokio_util::sync::CancellationToken::new();
    s.solicitud = s.solicitud.wrapping_add(1);
    s.invitado = Invitado::default();
    s.codigo_entrada.clear();
    s.etapa = Etapa::Entrada;
}
pub fn cambiar_codigo(s: &mut Estado, codigo: String) -> Option<Codigo> {
    s.cancel.cancel();
    s.cancel = tokio_util::sync::CancellationToken::new();
    s.solicitud = s.solicitud.wrapping_add(1);
    s.codigo_entrada = codigo;
    s.invitado.resuelta = None;
    s.invitado.visita = None;
    s.invitado.error = None;
    s.red_ocupada = false;
    let codigo = Codigo::parse(&s.codigo_entrada);
    s.invitado.entrada = if s.codigo_entrada.trim().is_empty() {
        v::EstadoEntrada::Vacio
    } else if codigo.is_ok() {
        v::EstadoEntrada::Validando
    } else {
        v::EstadoEntrada::Invalido
    };
    codigo.ok()
}
pub fn resuelta(
    s: &mut Estado,
    id: u64,
    resultado: Result<tareas::Resuelta, tareas::ErrorEntrada>,
) {
    if !s.vigente(id) || s.etapa != Etapa::Entrada {
        return;
    }
    match resultado {
        Ok(r) => {
            s.invitado.entrada = v::EstadoEntrada::Valido {
                anfitrion: "el equipo anfitrión".into(),
                disponibles: r.respuesta.capacity.saturating_sub(r.respuesta.accepted),
            };
            s.invitado.resuelta = Some(r);
        }
        Err(e) => {
            s.invitado.entrada = match e {
                tareas::ErrorEntrada::Invalido => v::EstadoEntrada::Invalido,
                tareas::ErrorEntrada::Vencido => v::EstadoEntrada::Vencido {
                    fecha: String::new(),
                    anfitrion: "el anfitrión".into(),
                },
                tareas::ErrorEntrada::Completo => v::EstadoEntrada::Completo {
                    anfitrion: "el anfitrión".into(),
                },
                tareas::ErrorEntrada::Inaccesible(e) => v::EstadoEntrada::Inaccesible(e),
            }
        }
    }
}
pub fn puede_confiar(s: &Estado) -> bool {
    !matches!(s.invitado.confianza, v::Confianza::Distinto { .. }) || s.invitado.confirmar_nueva
}
pub fn accion(app: &mut App, a: v::Accion) -> Option<Task<Mensaje>> {
    let s = &mut app.servicios.compartir;
    let task = match a {
        v::Accion::AceptarInvitacion => {
            abrir(s);
            Task::none()
        }
        v::Accion::Codigo(codigo) if s.etapa == Etapa::Entrada => {
            let Some(codigo) = cambiar_codigo(s, codigo) else {
                return Some(Task::none());
            };
            let (id, cancel) = (s.solicitud, s.cancel.clone());
            Task::perform(
                tareas::resolver(
                    crate::estado::directorio(false),
                    codigo,
                    Servicio::desde_entorno(),
                    cancel,
                ),
                move |r| Mensaje::Compartir(Accion::Resuelta(id, r)),
            )
        }
        v::Accion::PegarCodigo if s.etapa == Etapa::Entrada => iced::clipboard::read()
            .map(|c| Mensaje::Compartir(Accion::Vista(v::Accion::Codigo(c.unwrap_or_default())))),
        v::Accion::Continuar if s.etapa == Etapa::Entrada => {
            let Some(r) = s.invitado.resuelta.clone() else {
                return Some(Task::none());
            };
            s.red_ocupada = true;
            s.invitado.entrada = v::EstadoEntrada::Validando;
            let (id, cancel, h) = (s.solicitud, s.cancel.clone(), s.anfitrion.clone());
            Task::perform(
                tareas::consultar(crate::estado::directorio(false), r, h, cancel),
                move |r| Mensaje::Compartir(Accion::Visitada(id, r)),
            )
        }
        v::Accion::ConfirmarHuella(confirmar) if s.etapa == Etapa::Identidad => {
            s.invitado.confirmar_nueva = confirmar;
            Task::none()
        }
        v::Accion::ConfiarAnfitrion if s.etapa == Etapa::Identidad && puede_confiar(s) => {
            let Some(visita) = s.invitado.visita.clone() else {
                return Some(Task::none());
            };
            s.red_ocupada = true;
            let (id, cambio) = (s.solicitud, s.invitado.confirmar_nueva);
            Task::perform(
                tareas::confiar(crate::estado::directorio(false), visita, cambio),
                move |r| Mensaje::Compartir(Accion::Confiada(id, r)),
            )
        }
        v::Accion::NombreDestino(nombre) if s.etapa == Etapa::Destino => {
            s.invitado.nombre = nombre;
            s.invitado.error = None;
            Task::none()
        }
        v::Accion::RevisarInstalacion if s.etapa == Etapa::Destino => {
            let nombre = s.invitado.nombre.trim();
            if nombre.is_empty()
                || nombre.chars().count() > 80
                || nombre.chars().any(char::is_control)
            {
                s.invitado.error = Some(
                    "El nombre debe contener entre 1 y 80 caracteres, sin caracteres de control."
                        .into(),
                );
            } else if app.servicios.instancias.registros.iter().any(|i| {
                i.nombre.eq_ignore_ascii_case(nombre)
                    && s.invitado.destino.as_ref().is_none_or(|d| d.id != i.id)
            }) {
                s.invitado.error = Some("Ya existe una instancia con ese nombre.".into());
            } else {
                s.etapa = Etapa::RevisionInvitado;
            }
            Task::none()
        }
        v::Accion::Sincronizar if s.etapa == Etapa::RevisionInvitado => return Some(instalar(app)),
        v::Accion::Reintentar
            if s.etapa == Etapa::ResultadoInvitado
                && !s.invitado.completada
                && !s.invitado.revisar_nueva =>
        {
            return Some(instalar(app))
        }
        v::Accion::VerCambios if s.etapa == Etapa::ResultadoInvitado => {
            s.etapa = Etapa::RevisionInvitado;
            Task::none()
        }
        v::Accion::RevisarPublicacion if s.etapa == Etapa::ResultadoInvitado => {
            let Some(visita) = s.invitado.visita.as_ref() else {
                return Some(Task::none());
            };
            let invitacion = visita.invitacion.clone();
            s.red_ocupada = true;
            let (id, cancel, h) = (s.solicitud, s.cancel.clone(), s.anfitrion.clone());
            Task::perform(
                tareas::consultar(crate::estado::directorio(false), invitacion, h, cancel),
                move |r| Mensaje::Compartir(Accion::Visitada(id, r)),
            )
        }
        v::Accion::VerDetalle if s.etapa == Etapa::ResultadoInvitado => {
            s.invitado.ver_detalle = true;
            Task::none()
        }
        v::Accion::Jugar if s.etapa == Etapa::ResultadoInvitado && s.invitado.completada => {
            let id = s.invitado.destino.as_ref()?.id.clone();
            s.cerrar();
            let indice = app
                .servicios
                .instancias
                .registros
                .iter()
                .position(|i| i.id == id)?;
            return Some(crate::juego_real::iniciar(app, indice, true));
        }
        v::Accion::Atras if s.etapa == Etapa::Destino => {
            s.etapa = Etapa::Identidad;
            Task::none()
        }
        v::Accion::Atras if s.etapa == Etapa::RevisionInvitado => {
            s.etapa = Etapa::Destino;
            Task::none()
        }
        v::Accion::CopiarHuella if s.etapa == Etapa::Identidad => {
            let huella = s
                .invitado
                .visita
                .as_ref()
                .and_then(|v| c::identidad::huella_publica(&v.manifiesto.origen.anfitrion).ok())
                .unwrap_or_default();
            iced::clipboard::write(huella)
        }
        _ => return None,
    };
    Some(task)
}
pub fn visitada(s: &mut Estado, id: u64, r: Result<tareas::Visita, String>) {
    if !s.vigente(id) {
        return;
    }
    s.red_ocupada = false;
    match r {
        Ok(v) => {
            s.invitado.cambio_perfil = s.invitado.destino.is_some()
                && s.invitado.destino_vinculo.as_ref().is_some_and(|anterior| {
                    anterior.perfil != v.manifiesto.perfil || anterior.origen != v.manifiesto.origen
                });
            if s.invitado.cambio_perfil {
                s.invitado.destino = None;
                s.invitado.destino_vinculo = None;
                s.invitado.nombre = format!("{} (nueva)", s.invitado.nombre);
            }
            let fecha = v
                .confianza
                .fecha()
                .unwrap_or_else(|| "día de la confirmación guardada".into());
            s.invitado.error = None;
            s.invitado.revisar_nueva = false;
            s.invitado.confirmar_nueva = false;
            s.invitado.confianza = match v.confianza {
                c::confianza::EstadoConfianza::PrimeraVez => v::Confianza::PrimeraVez,
                c::confianza::EstadoConfianza::Conocido { .. } => v::Confianza::Conocido {
                    fecha: fecha.clone(),
                },
                c::confianza::EstadoConfianza::Distinto { .. } => v::Confianza::Distinto { fecha },
            };
            s.invitado.visita = Some(v);
            s.etapa = Etapa::Identidad;
        }
        Err(e) => {
            s.invitado.entrada = v::EstadoEntrada::Inaccesible(e);
            s.etapa = Etapa::Entrada;
        }
    }
    cerrar_pendiente(s);
}
pub fn confiada(s: &mut Estado, id: u64, r: Result<(), String>) {
    if !s.vigente(id) {
        return;
    }
    s.red_ocupada = false;
    match r {
        Ok(()) => {
            s.invitado.error = s.invitado.cambio_perfil.then(||"El origen o el perfil cambió. La instancia anterior se conserva; elige el nombre de otro destino.".into());
            s.etapa = Etapa::Destino;
        }
        Err(e) => {
            s.invitado.entrada = v::EstadoEntrada::Inaccesible(e);
            s.etapa = Etapa::Entrada;
        }
    }
    cerrar_pendiente(s);
}
fn cerrar_pendiente(s: &mut Estado) {
    if s.cerrar_al_terminar {
        s.cerrar_al_terminar = false;
        s.cerrar();
    }
}
fn instalar(app: &mut App) -> Task<Mensaje> {
    let s = &mut app.servicios.compartir;
    let Some(visita) = s.invitado.visita.clone() else {
        return Task::none();
    };
    if s.invitado
        .destino
        .as_ref()
        .is_some_and(|i| app.servicios.juego.ocupada(&i.id))
    {
        s.invitado.error = Some("La instancia está en uso. Ciérrala antes de reintentar.".into());
        return Task::none();
    }
    s.red_ocupada = true;
    s.invitado.error = None;
    s.invitado.completada = false;
    s.etapa = Etapa::InstalandoInvitado;
    s.cancel = tokio_util::sync::CancellationToken::new();
    let (id, nombre, anterior, h, cancel) = (
        s.solicitud,
        s.invitado.nombre.clone(),
        s.invitado.destino.clone(),
        s.anfitrion.clone(),
        s.cancel.clone(),
    );
    Task::perform(
        super::invitado_instalacion::instalar(
            crate::estado::directorio(false),
            app.servicios.instancias.almacen.clone(),
            visita,
            nombre,
            anterior,
            app.servicios.catalog.as_ref().ok().cloned(),
            h,
            cancel,
        ),
        move |r| Mensaje::Compartir(Accion::Instalada(id, r)),
    )
}
pub fn instalada(app: &mut App, id: u64, r: super::invitado_instalacion::Resultado) {
    if !app.servicios.compartir.vigente(id)
        || app.servicios.compartir.etapa != Etapa::InstalandoInvitado
    {
        return;
    }
    if let (Some(instancia), Some(tarjeta)) = (&r.instancia, r.tarjeta) {
        let mut tarjetas = crate::lista_instancias().lock().unwrap();
        if let Some(n) = app
            .servicios
            .instancias
            .registros
            .iter()
            .position(|i| i.id == instancia.id)
        {
            if let Some(t) = tarjetas.get_mut(n) {
                *t = tarjeta;
            }
        } else {
            app.servicios.instancias.registros.push(instancia.clone());
            tarjetas.push(tarjeta);
            app.grupos.push(None);
            app.configs.push(crate::config_por_defecto());
        }
    }
    let s = &mut app.servicios.compartir;
    if r.instancia.is_some() {
        if s.invitado.destino.is_none() {
            s.invitado.destino_vinculo = s.invitado.visita.as_ref().map(|v| c::vinculos::Vinculo {
                origen: v.manifiesto.origen.clone(),
                perfil: v.manifiesto.perfil.clone(),
            });
        }
        s.invitado.destino = r.instancia;
    }
    s.invitado.completada = r.resultado.is_ok();
    s.invitado.revisar_nueva = r.revisar_nueva;
    s.invitado.error = r.resultado.err();
    s.etapa = Etapa::ResultadoInvitado;
    s.red_ocupada = false;
    cerrar_pendiente(s);
}
