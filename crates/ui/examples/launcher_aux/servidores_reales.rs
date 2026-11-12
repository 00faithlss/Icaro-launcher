// SPDX-License-Identifier: GPL-3.0-only
//! Lista real de servidores con formularios y consultas independientes de la maqueta.
use crate::{App, Mensaje};
use icaro_core::{
    servidores::{self, Servidor},
    zerotier,
};
use iced::{Element, Task};
use icaro_ui::pantalla_servidores::Pestana;
use std::collections::BTreeMap;
use std::time::Instant;
use tokio_util::sync::CancellationToken;

#[derive(Default)]
pub struct Estado {
    pub(crate) loaded: bool,
    pub(crate) loading: bool,
    pub(crate) saving: bool,
    revision: u64,
    request: u64,
    zero_request: u64,
    pub(crate) registros: Vec<Servidor>,
    pub(crate) estados: BTreeMap<String, Result<servidores::Estado, String>>,
    cancel: Option<CancellationToken>,
    pub(crate) error: Option<String>,
    pub(crate) editor: Option<Editor>,
    pub visual: bool,
    pub(crate) zero: Option<zerotier::Estado>,
    /// PantallaServidores: pestaña actual, la anterior y cuándo cambió.
    pub(crate) pestana: Pestana,
    pub(crate) pestana_desde: Pestana,
    pub(crate) pestana_t: Option<Instant>,
    pub(crate) busqueda: String,
    /// Cuándo se abrió el formulario o la fila del editor (despliegue).
    pub(crate) editor_t: Option<Instant>,
    /// Fila que pide confirmar Quitar.
    pub(crate) confirmar: Option<String>,
    /// Fila quitada que todavía sale deslizándose, con su posición.
    pub(crate) saliendo: Option<(Servidor, usize, Instant)>,
    /// Servidor recién guardado: brilla 1,6 s.
    pub(crate) nuevo: Option<(String, Instant)>,
    nuevo_pendiente: Option<String>,
    pub(crate) copiado: Option<(String, Instant)>,
    prueba_gen: u64,
    prueba_cancel: Option<CancellationToken>,
    /// Red privada: confirmar Desconectar y operación en curso.
    pub(crate) red_confirmar: bool,
    pub(crate) red_ocupada: bool,
    /// Con amigos: lo elegido y las confirmaciones.
    pub(crate) amigos: crate::con_amigos::Estado,
    pub arrastre: Option<icaro_ui::servidores::Arrastre>,
    toma_y: f32,
    cerrando: Option<std::time::Instant>,
}
impl Drop for Estado {
    fn drop(&mut self) {
        if let Some(c) = &self.cancel {
            c.cancel();
        }
    }
}
impl Estado {
    pub fn guardados(&self) -> &[Servidor] {
        &self.registros
    }
    pub fn panel_abierto(&self) -> bool {
        self.editor.is_some()
    }
    pub fn mover(&mut self, y: f32) {
        if let Some(a) = &mut self.arrastre {
            let delta = ((y - self.toma_y)
                / (icaro_ui::servidores::ALTO_FILA + icaro_ui::servidores::SEPARACION))
                .round() as i64;
            a.destino = (a.origen as i64 + delta)
                .clamp(0, self.registros.len().saturating_sub(1) as i64)
                as usize;
        }
    }
    /// Hay algo de PantallaServidores animándose.
    pub fn animando(&self, ahora: Instant) -> bool {
        let vivo = |t: Option<Instant>, ms: u64| t.is_some_and(|t| ahora.saturating_duration_since(t).as_millis() < ms as u128);
        vivo(self.pestana_t, MS_PESTANA)
            || vivo(self.editor_t, MS_DESPLIEGUE)
            || vivo(self.saliendo.as_ref().map(|s| s.2), MS_SALIDA)
            || vivo(self.nuevo.as_ref().map(|n| n.1), MS_NUEVO)
            || vivo(self.copiado.as_ref().map(|c| c.1), MS_COPIADO)
    }
    pub fn avanzar(&mut self) {
        let ahora = Instant::now();
        let pasado = |t: Instant, ms: u64| ahora.saturating_duration_since(t).as_millis() >= ms as u128;
        if self.saliendo.as_ref().is_some_and(|s| pasado(s.2, MS_SALIDA)) {
            self.saliendo = None;
        }
        if self.nuevo.as_ref().is_some_and(|n| pasado(n.1, MS_NUEVO)) {
            self.nuevo = None;
        }
        if self.copiado.as_ref().is_some_and(|c| pasado(c.1, MS_COPIADO)) {
            self.copiado = None;
        }
        if self
            .cerrando
            .is_some_and(|t| t.elapsed().as_millis() >= icaro_ui::tema::duracion::FAST as u128)
        {
            self.editor = None;
            self.cerrando = None;
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instancia {
    pub(crate) id: String,
    pub(crate) nombre: String,
}
impl std::fmt::Display for Instancia {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.nombre)
    }
}
pub(crate) struct Editor {
    pub(crate) id: Option<String>,
    pub(crate) nombre: String,
    pub(crate) direccion: String,
    pub(crate) instancia: Option<Instancia>,
    pub(crate) error: Option<String>,
    pub(crate) sincronizar: bool,
    /// Prueba de la dirección mientras se escribe (solo al agregar).
    pub(crate) prueba: Prueba,
}

/// Resultado de probar la dirección del formulario.
#[derive(Debug, Clone, Default)]
pub(crate) enum Prueba {
    #[default]
    Vacia,
    Formato,
    Probando,
    Responde(servidores::Estado),
    NoResponde,
}
#[derive(Clone, Debug)]
pub enum Accion {
    Abrir,
    Cargados(Result<Vec<Servidor>, String>),
    Consultar,
    ConsultarZero,
    Estado(u64, String, Result<servidores::Estado, String>),
    Zero(u64, zerotier::Estado),
    Nuevo,
    Editar(usize),
    Nombre(String),
    Direccion(String),
    Instancia(Instancia),
    /// «Sincronizar mods al iniciar»: en una fila abierta se guarda al tiro.
    Sincronizar(bool),
    Guardar,
    Guardados(u64, Result<Vec<Servidor>, String>),
    Cerrar,
    Jugar(usize),
    Soltar,
    CancelarOrden,
    GestionarRed,
    // PantallaServidores
    Pestana(Pestana),
    /// Flecha izquierda (-1) o derecha (1) sobre las pestañas.
    PestanaVecina(i32),
    Buscar(String),
    /// Abre o cierra la fila de ese servidor (solo una a la vez).
    Alternar(String),
    /// Prueba diferida de la dirección: generación y texto.
    Probar(u64, String),
    Probado(u64, Result<servidores::Estado, String>),
    CopiarDireccion(String),
    PedirQuitar(String),
    CancelarQuitar,
    Quitar(String),
    Reintentar(String),
    PedirDesconectar,
    CancelarDesconectar,
    Desconectar,
    RedHecha(Result<zerotier::Estado, String>),
    CopiarIp(String),
    InvitarRed,
    Amigos(crate::con_amigos::Accion),
}

/// Duraciones de PantallaServidores (ms).
pub const MS_PESTANA: u64 = 280;
pub const MS_DESPLIEGUE: u64 = 300;
pub const MS_SALIDA: u64 = 200;
pub const MS_NUEVO: u64 = 1600;
pub const MS_COPIADO: u64 = 1800;

fn m(a: Accion) -> Mensaje {
    Mensaje::Servidores(a)
}
fn recargar_instancias(app: &mut App) {
    if !app.servicios.servidores.visual {
        if let Err(error) = crate::instancias_reales::recargar(app) {
            app.avisar(error);
        } else if let Some(editor) = &mut app.servicios.servidores.editor {
            if let Some(elegida) = &editor.instancia {
                editor.instancia = app.servicios.instancias.registros.iter().find(|i| i.id == elegida.id)
                    .map(|i| Instancia { id: i.id.clone(), nombre: i.nombre.clone() });
            }
        }
    }
}
fn persistir(app: &mut App, v: Vec<Servidor>) -> Task<Mensaje> {
    let s = &mut app.servicios.servidores;
    if s.saving || s.error.is_some() {
        return Task::none();
    }
    s.saving = true;
    s.revision += 1;
    let g = s.revision;
    let root = crate::estado::directorio(false);
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                servidores::guardar(&root, &v)?;
                Ok(v)
            })
            .await
            .map_err(|_| "No se pudo guardar la lista.".to_string())
            .and_then(|r| r)
        },
        move |r| m(Accion::Guardados(g, r)),
    )
}
/// Acciones de la vista que no tocan disco ni red: también en la demo.
fn actualizar_vista(app: &mut App, a: &Accion) -> Option<Task<Mensaje>> {
    let reducir = app.reducir;
    let s = &mut app.servicios.servidores;
    match a {
        Accion::Pestana(p) => {
            if *p != s.pestana {
                s.pestana_desde = s.pestana;
                s.pestana = *p;
                s.pestana_t = (!reducir).then(Instant::now);
                app.ahora = Instant::now();
            }
            let tarea = match p {
                Pestana::Red if !app.servicios.demo => actualizar(app, Accion::ConsultarZero),
                Pestana::ConAmigos => crate::con_amigos::entrar(app),
                _ => Task::none(),
            };
            Some(tarea)
        }
        Accion::PestanaVecina(paso) => {
            let p = s.pestana.vecina(*paso);
            Some(actualizar(app, Accion::Pestana(p)))
        }
        Accion::Buscar(q) => {
            s.busqueda = q.clone();
            Some(Task::none())
        }
        Accion::PedirQuitar(id) => {
            s.confirmar = Some(id.clone());
            Some(Task::none())
        }
        Accion::CancelarQuitar => {
            s.confirmar = None;
            Some(Task::none())
        }
        Accion::PedirDesconectar => {
            s.red_confirmar = true;
            Some(Task::none())
        }
        Accion::CancelarDesconectar => {
            s.red_confirmar = false;
            Some(Task::none())
        }
        _ => None,
    }
}

/// «Play.ejemplo.cl» → «Play»; una IP queda tal cual.
fn nombre_de(direccion: &str) -> String {
    let host = direccion.split(':').next().unwrap_or(direccion);
    if host.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return direccion.to_owned();
    }
    let parte = host.split('.').find(|p| !p.is_empty() && *p != "play" && *p != "mc").unwrap_or(host);
    let mut c = parte.chars();
    c.next().map(|p| p.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// Una dirección se ve como `play.ejemplo.cl` o `10.0.0.5:25565`.
fn formato_valido(v: &str) -> bool {
    !v.chars().any(char::is_whitespace) && v.contains(['.', ':']) && servidores::direccion(v).is_ok()
}

pub fn actualizar(app: &mut App, a: Accion) -> Task<Mensaje> {
    if let Some(t) = actualizar_vista(app, &a) {
        return t;
    }
    if app.servicios.demo {
        return Task::none();
    }
    if app.servicios.servidores.saving
        && !matches!(
            a,
            Accion::Guardados(..) | Accion::Estado(..) | Accion::Zero(..) | Accion::ConsultarZero
        )
    {
        return Task::none();
    }
    match a {
        Accion::Pestana(_) | Accion::PestanaVecina(_) | Accion::Buscar(_) | Accion::PedirQuitar(_)
        | Accion::CancelarQuitar | Accion::PedirDesconectar | Accion::CancelarDesconectar => {}
        Accion::Alternar(id) => {
            let s = &mut app.servicios.servidores;
            let abierta = s.editor.as_ref().and_then(|e| e.id.as_deref()) == Some(id.as_str());
            s.confirmar = None;
            if abierta {
                // Al cerrar se guarda el nombre si cambió.
                let cambio = s.editor.as_ref().is_some_and(|e| {
                    s.registros.iter().any(|v| Some(&v.id) == e.id.as_ref() && v.nombre != e.nombre && !e.nombre.trim().is_empty())
                });
                if cambio {
                    let t = actualizar(app, Accion::Guardar);
                    app.servicios.servidores.editor = None;
                    return t;
                }
                s.editor = None;
                return Task::none();
            }
            if let Some(i) = s.registros.iter().position(|v| v.id == id) {
                return actualizar(app, Accion::Editar(i));
            }
        }
        Accion::Probar(g, direccion) => {
            let s = &mut app.servicios.servidores;
            if g != s.prueba_gen || s.editor.as_ref().is_none_or(|e| e.id.is_some() || e.direccion.trim() != direccion) {
                return Task::none();
            }
            if let Some(c) = s.prueba_cancel.take() {
                c.cancel();
            }
            let c = CancellationToken::new();
            s.prueba_cancel = Some(c.clone());
            return Task::perform(async move { servidores::consultar(&direccion, c).await }, move |r| m(Accion::Probado(g, r)));
        }
        Accion::Probado(g, r) => {
            let s = &mut app.servicios.servidores;
            if g == s.prueba_gen {
                if let Some(e) = s.editor.as_mut().filter(|e| e.id.is_none()) {
                    e.prueba = match r {
                        Ok(estado) => Prueba::Responde(estado),
                        Err(_) => Prueba::NoResponde,
                    };
                }
            }
        }
        Accion::CopiarDireccion(id) => {
            let s = &mut app.servicios.servidores;
            if let Some(v) = s.registros.iter().find(|v| v.id == id) {
                let dir = v.direccion.clone();
                s.copiado = Some((id, Instant::now()));
                app.ahora = Instant::now();
                return iced::clipboard::write(dir);
            }
        }
        Accion::Quitar(id) => {
            let s = &mut app.servicios.servidores;
            s.confirmar = None;
            let Some(i) = s.registros.iter().position(|v| v.id == id) else {
                return Task::none();
            };
            if s.editor.as_ref().and_then(|e| e.id.as_deref()) == Some(id.as_str()) {
                s.editor = None;
            }
            let mut lista = s.registros.clone();
            let fuera = lista.remove(i);
            // La fila sale deslizándose aunque ya no esté en la lista.
            if !app.reducir {
                s.saliendo = Some((fuera, i, Instant::now()));
                app.ahora = Instant::now();
            }
            return persistir(app, lista);
        }
        Accion::Reintentar(id) => {
            let s = &mut app.servicios.servidores;
            let Some(v) = s.registros.iter().find(|v| v.id == id).cloned() else {
                return Task::none();
            };
            s.estados.remove(&id);
            let g = s.request;
            let c = s.cancel.clone().unwrap_or_default();
            return Task::perform(async move { servidores::consultar(&v.direccion, c).await }, move |r| {
                m(Accion::Estado(g, v.id.clone(), r))
            });
        }
        Accion::Desconectar => {
            let s = &mut app.servicios.servidores;
            s.red_confirmar = false;
            let Some(id) = red_conectada(s.zero.as_ref()).map(|r| r.id.clone()) else {
                return Task::none();
            };
            s.red_ocupada = true;
            return Task::perform(
                async move { zerotier::cambiar_red(&id, zerotier::Accion::Salir, CancellationToken::new()).await },
                |r| m(Accion::RedHecha(r)),
            );
        }
        Accion::RedHecha(r) => {
            let s = &mut app.servicios.servidores;
            s.red_ocupada = false;
            match r {
                Ok(z) => s.zero = Some(z),
                Err(e) => app.avisar_error(format!("No se pudo desconectar de la red: {e}")),
            }
        }
        Accion::CopiarIp(ip) => {
            app.servicios.servidores.copiado = Some(("ip".into(), Instant::now()));
            app.ahora = Instant::now();
            return iced::clipboard::write(ip);
        }
        Accion::InvitarRed => {
            let Some(r) = red_conectada(app.servicios.servidores.zero.as_ref()) else {
                return Task::none();
            };
            let texto = format!(
                "Únete a mi red privada para jugar en LAN: abre ZeroTier, entra a la red {} y avísame para autorizarte.",
                r.id
            );
            app.servicios.servidores.copiado = Some(("invitar".into(), Instant::now()));
            app.ahora = Instant::now();
            app.avisar("Invitación copiada: envíala a tu amigo.".into());
            return iced::clipboard::write(texto);
        }
        Accion::Amigos(a) => return crate::con_amigos::actualizar(app, a),
        Accion::Abrir => {
            recargar_instancias(app);
            let s = &mut app.servicios.servidores;
            if s.loading {
                return Task::none();
            }
            if s.editor.is_some() || s.arrastre.is_some() {
                return actualizar(app, Accion::Consultar);
            }
            s.loading = true;
            let root = crate::estado::directorio(false);
            return Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || servidores::cargar(&root))
                        .await
                        .map_err(|_| "No se pudo leer la lista.".to_string())
                        .and_then(|r| r)
                },
                |r| m(Accion::Cargados(r)),
            );
        }
        Accion::Cargados(r) => {
            let s = &mut app.servicios.servidores;
            s.loading = false;
            s.loaded = true;
            match r {
                Ok(v) => {
                    s.registros = v;
                    s.estados.retain(|id, _| s.registros.iter().any(|v| &v.id == id));
                    s.error = None;
                }
                Err(e) => {
                    s.error = Some(e);
                }
            }
            return actualizar(app, Accion::Consultar);
        }
        Accion::Consultar => {
            let s = &mut app.servicios.servidores;
            if s.loading || !s.loaded {
                return Task::none();
            }
            if let Some(c) = s.cancel.take() {
                c.cancel();
            }
            s.request += 1;
            let g = s.request;
            let c = CancellationToken::new();
            s.cancel = Some(c.clone());
            let slots = std::sync::Arc::new(tokio::sync::Semaphore::new(4));
            let mut tasks = vec![];
            for server in s.registros.clone() {
                let c = c.clone();
                let slots = slots.clone();
                tasks.push(Task::perform(async move{
                    tokio::select!{
                        _=c.cancelled()=>return Err("Consulta cancelada.".into()),
                        permit=slots.acquire_owned()=>{let _permit=permit.map_err(|_|"Consulta cerrada.")?;servidores::consultar(&server.direccion,c).await}
                    }
                },move|r|m(Accion::Estado(g,server.id.clone(),r))));
            }
            tasks.push(actualizar(app, Accion::ConsultarZero));
            return Task::batch(tasks);
        }
        Accion::ConsultarZero => {
            let s = &mut app.servicios.servidores;
            s.zero_request += 1;
            let g = s.zero_request;
            return Task::perform(zerotier::detectar(), move |z| m(Accion::Zero(g, z)));
        }
        Accion::Estado(g, id, r) => {
            let s = &mut app.servicios.servidores;
            if g == s.request && s.registros.iter().any(|v| v.id == id) {
                s.estados.insert(id, r);
            }
        }
        Accion::Zero(g, z) => {
            let s = &mut app.servicios.servidores;
            if g == s.zero_request {
                s.zero = Some(z);
            }
        }
        Accion::Nuevo => {
            if app.servicios.servidores.error.is_some() {
                return Task::none();
            }
            recargar_instancias(app);
            app.servicios.servidores.editor = Some(Editor {
                id: None,
                nombre: String::new(),
                direccion: String::new(),
                instancia: None,
                error: None,
                sincronizar: false,
                prueba: Prueba::Vacia,
            });
            // El formulario se despliega en la misma página.
            let s = &mut app.servicios.servidores;
            s.editor_t = (!app.reducir).then(Instant::now);
            s.confirmar = None;
            s.cerrando = None;
            app.ahora = Instant::now();
            return iced::widget::text_input::focus(iced::widget::text_input::Id::new(
                "servidor-direccion",
            ));
        }
        Accion::Editar(i) => {
            recargar_instancias(app);
            if let Some(v) = app.servicios.servidores.registros.get(i) {
                let instancia = v
                    .instancia
                    .as_ref()
                    .and_then(|id| {
                        app.servicios
                            .instancias
                            .registros
                            .iter()
                            .find(|i| &i.id == id)
                    })
                    .map(|i| Instancia {
                        id: i.id.clone(),
                        nombre: i.nombre.clone(),
                    });
                app.servicios.servidores.editor = Some(Editor {
                    id: Some(v.id.clone()),
                    nombre: v.nombre.clone(),
                    direccion: v.direccion.clone(),
                    instancia,
                    error: None,
                    sincronizar: v.sincronizar,
                prueba: Prueba::Vacia,
                });
                // La fila se abre hacia abajo; solo una a la vez.
                let s = &mut app.servicios.servidores;
                s.editor_t = (!app.reducir).then(Instant::now);
                s.confirmar = None;
                s.cerrando = None;
                app.ahora = Instant::now();
            }
        }
        Accion::Nombre(v) => {
            if let Some(e) = &mut app.servicios.servidores.editor {
                e.nombre = v;
            }
        }
        Accion::Direccion(v) => {
            let s = &mut app.servicios.servidores;
            let Some(e) = &mut s.editor else {
                return Task::none();
            };
            e.direccion = v.clone();
            e.error = None;
            if e.id.is_some() {
                return Task::none();
            }
            // Se prueba mientras se escribe: 350 ms después de la última tecla.
            let v = v.trim().to_owned();
            e.prueba = if v.is_empty() {
                Prueba::Vacia
            } else if !formato_valido(&v) {
                Prueba::Formato
            } else {
                Prueba::Probando
            };
            let probar = matches!(e.prueba, Prueba::Probando);
            s.prueba_gen += 1;
            if let Some(c) = s.prueba_cancel.take() {
                c.cancel();
            }
            if !probar {
                return Task::none();
            }
            let g = s.prueba_gen;
            return Task::perform(
                async move {
                    tokio::time::sleep(std::time::Duration::from_millis(350)).await;
                    v
                },
                move |v| m(Accion::Probar(g, v)),
            );
        }
        Accion::Instancia(v) => {
            if let Some(e) = &mut app.servicios.servidores.editor {
                e.instancia = Some(v);
                // En una fila abierta, elegir la instancia se guarda al tiro.
                if e.id.is_some() {
                    return actualizar(app, Accion::Guardar);
                }
            }
        }
        Accion::Sincronizar(v) => {
            if let Some(e) = &mut app.servicios.servidores.editor {
                e.sincronizar = v;
                if e.id.is_some() {
                    return actualizar(app, Accion::Guardar);
                }
            }
        }
        Accion::Cerrar => {
            let s = &mut app.servicios.servidores;
            s.editor = None;
            s.prueba_gen += 1;
            if let Some(c) = s.prueba_cancel.take() {
                c.cancel();
            }
        }
        Accion::Guardar => {
            let Some(e) = &mut app.servicios.servidores.editor else {
                return Task::none();
            };
            let nombre = if e.nombre.trim().is_empty() { nombre_de(e.direccion.trim()) } else { e.nombre.clone() };
            match servidores::nuevo(
                nombre,
                e.direccion.clone(),
                e.instancia.as_ref().map(|i| i.id.clone()),
            ) {
                Err(error) => e.error = Some(error),
                Ok(mut v) => {
                    v.sincronizar = e.sincronizar;
                    let mut lista = app.servicios.servidores.registros.clone();
                    if let Some(id) = &e.id {
                        v.id = id.clone();
                        let Some(old) = lista.iter_mut().find(|v| &v.id == id) else {
                            return Task::none();
                        };
                        *old = v;
                    } else {
                        // El nuevo aparece primero en la lista.
                        app.servicios.servidores.nuevo_pendiente = Some(v.id.clone());
                        lista.insert(0, v);
                    }
                    return persistir(app, lista);
                }
            }
        }
        Accion::Guardados(g, r) => {
            let s = &mut app.servicios.servidores;
            if g != s.revision {
                return Task::none();
            }
            s.saving = false;
            match r {
                Ok(v) => {
                    s.registros = v;
                    // Agregar cierra el formulario; editar una fila la deja abierta.
                    if s.editor.as_ref().is_some_and(|e| e.id.is_none()) {
                        s.editor = None;
                    } else if let Some(e) = &mut s.editor {
                        e.error = None;
                    }
                    if let Some(id) = s.nuevo_pendiente.take() {
                        if !app.reducir {
                            s.nuevo = Some((id, Instant::now()));
                            app.ahora = Instant::now();
                        }
                    }
                    return actualizar(app, Accion::Consultar);
                }
                Err(e) => {
                    if let Some(editor) = &mut s.editor {
                        editor.error = Some(e);
                    } else {
                        app.avisar(format!("No se pudo guardar: {e}"));
                    }
                }
            }
        }
        Accion::GestionarRed => {
            app.servicios.servidores.editor = None;
            app.pantalla = Some(crate::pantallas::Pantalla::Red);
            return crate::red_privada_real::actualizar(
                app,
                crate::red_privada_real::Accion::Abrir,
            );
        }
        Accion::CancelarOrden => app.servicios.servidores.arrastre = None,
        Accion::Soltar => {
            let s = &mut app.servicios.servidores;
            if let Some(drag) = s.arrastre.take() {
                if drag.origen < s.registros.len()
                    && drag.destino < s.registros.len()
                    && drag.origen != drag.destino
                {
                    let mut lista = s.registros.clone();
                    let v = lista.remove(drag.origen);
                    lista.insert(drag.destino, v);
                    return persistir(app, lista);
                }
            }
        }
        Accion::Jugar(i) => {
            let Some(v) = app.servicios.servidores.registros.get(i).cloned() else {
                return Task::none();
            };
            let Some(index) = v.instancia.as_ref().and_then(|id| {
                app.servicios
                    .instancias
                    .registros
                    .iter()
                    .position(|i| &i.id == id)
            }) else {
                app.avisar("Vincula una instancia al servidor antes de jugar.".into());
                return actualizar(app, Accion::Editar(i));
            };
            if let Some(Ok(status)) = app.servicios.servidores.estados.get(&v.id) {
                let mc = &app.servicios.instancias.registros[index].minecraft;
                // Solo identifica nombres de versiones exactos; proxies pueden publicar etiquetas propias.
                if status.version != *mc
                    && app
                        .servicios
                        .instancias
                        .versiones
                        .iter()
                        .any(|version| version.id == status.version)
                {
                    app.avisar(format!("El servidor anuncia Minecraft {} y la instancia usa {mc}. Cambia la versión o vincula otra instancia.",status.version));
                    return Task::none();
                }
            }
            if v.sincronizar {
                return crate::sincronizacion_real::actualizar(
                    app,
                    crate::sincronizacion_real::Accion::Abrir(i),
                );
            }
            return crate::juego_real::iniciar_con_servidor(app, index, true, Some(v.direccion));
        }
    }
    Task::none()
}
/// PantallaServidores: banda, pestañas y la pestaña elegida.
pub fn vista(app: &App) -> Element<'_, Mensaje> {
    crate::servidores_vista::vista(app)
}
/// El editor ya no abre un panel lateral: agregar y editar ocurren en la página.
pub fn panel(_app: &App) -> Option<Element<'_, Mensaje>> {
    None
}
/// Primera red conectada (estado «OK») de ZeroTier.
pub(crate) fn red_conectada(z: Option<&zerotier::Estado>) -> Option<&zerotier::Red> {
    match z? {
        zerotier::Estado::Disponible { redes, .. } => redes.iter().find(|r| r.estado == "OK"),
        _ => None,
    }
}
/// Flechas izquierda y derecha: pestaña vecina. Solo llegan las teclas que
/// ningún campo de texto tomó.
fn flechas(tecla: iced::keyboard::Key, _: iced::keyboard::Modifiers) -> Option<Mensaje> {
    use iced::keyboard::{key::Named, Key};
    match tecla {
        Key::Named(Named::ArrowLeft) => Some(m(Accion::PestanaVecina(-1))),
        Key::Named(Named::ArrowRight) => Some(m(Accion::PestanaVecina(1))),
        _ => None,
    }
}
pub fn suscripcion(app: &App) -> iced::Subscription<Mensaje> {
    let visible = app.seccion == icaro_ui::shell::Seccion::Servidores && app.pantalla.is_none();
    let teclas = if visible {
        iced::keyboard::on_key_press(flechas)
    } else {
        iced::Subscription::none()
    };
    if app.servicios.demo || !visible {
        return teclas;
    }
    // El ping se repite cada 30 s mientras la sección está a la vista.
    iced::Subscription::batch([
        teclas,
        iced::time::every(std::time::Duration::from_secs(30)).map(|_| m(Accion::Abrir)),
        iced::time::every(std::time::Duration::from_secs(10)).map(|_| m(Accion::ConsultarZero)),
    ])
}
/// Escenas de PantallaServidores: `servidores` (lista), `-vacio`,
/// `-agregar`, `-agregar-error`, `-abierto`, `-quitar`, `-sin-respuesta`,
/// `-red`, `-red-off`, `-amigos…` (ver `con_amigos::preparar_diseno`) y
/// `-angosto` (ventana pequeña).
pub fn preparar_diseno(app: &mut App, scene: &str) {
    if scene.ends_with("angosto") {
        app.ventana = iced::Size::new(940., 720.);
    }
    app.seccion = icaro_ui::shell::Seccion::Servidores;
    app.editor = None;
    let instancia = |id: &str, nombre: &str, mc: &str, loader: &str| icaro_core::instancias::Instancia {
        id: id.into(),
        nombre: nombre.into(),
        minecraft: mc.into(),
        loader: loader.into(),
        portada: 0,
        creada: 0,
    };
    let survival = instancia("qa-survival", "Survival 26", "1.21.1", "fabric");
    let super_v = instancia("qa-supervivencia", "Supervivencia", "1.21.4", "vanilla");
    let better = instancia("qa-better", "Better MC", "1.20.1", "neoforge");
    app.servicios.instancias.registros = vec![super_v.clone(), survival.clone(), better.clone()];
    let s = &mut app.servicios.servidores;
    s.visual = true;
    s.loaded = true;
    s.zero = Some(zerotier::Estado::Disponible {
        version: "1.16.0".into(),
        online: true,
        redes: vec![zerotier::Red {
            id: "8056c2e21c000001".into(),
            nombre: "maincra o nose".into(),
            estado: if scene.ends_with("-red-off") { "ACCESS_DENIED" } else { "OK" }.into(),
            ips: vec!["10.168.182.50/24".into()],
        }],
    });
    if scene.contains("-red") {
        s.pestana = Pestana::Red;
        s.pestana_desde = Pestana::Red;
    }
    if scene.contains("-amigos") {
        s.pestana = Pestana::ConAmigos;
        s.pestana_desde = Pestana::ConAmigos;
    }
    if scene.ends_with("-vacio") && !scene.contains("-amigos") {
        return;
    }
    let estado = |jugadores, maximo, version: &str, motd: &str, ms| -> Result<servidores::Estado, String> {
        Ok(servidores::Estado { version: version.into(), protocolo: 767, jugadores, maximo, motd: motd.into(), ms })
    };
    let a = servidores::nuevo("SMP de Forjador".into(), "smp.forjador.cl".into(), Some(survival.id.clone())).unwrap();
    let b = servidores::nuevo("Hypixel".into(), "mc.hypixel.net".into(), Some(super_v.id.clone())).unwrap();
    let c = servidores::nuevo("Server del colegio".into(), "192.168.1.40:25570".into(), Some(better.id.clone())).unwrap();
    s.estados.insert(a.id.clone(), estado(3, 20, "1.21.1", "Temporada 4 · reinicio el domingo", 34));
    s.estados.insert(b.id.clone(), estado(38_214, 100_000, "1.8 a 1.21.4", "SkyBlock 0.21 · Bed Wars", 128));
    s.estados.insert(c.id.clone(), Err("No se pudo conectar.".into()));
    let abrir = |v: &Servidor, i: Option<&icaro_core::instancias::Instancia>| Editor {
        id: Some(v.id.clone()),
        nombre: v.nombre.clone(),
        direccion: v.direccion.clone(),
        instancia: i.map(|i| Instancia { id: i.id.clone(), nombre: i.nombre.clone() }),
        error: None,
        sincronizar: false,
        prueba: Prueba::Vacia,
    };
    if scene.ends_with("-abierto") || scene.ends_with("-quitar") {
        s.editor = Some(abrir(&a, Some(&survival)));
    }
    if scene.ends_with("-quitar") {
        s.confirmar = Some(a.id.clone());
    }
    if scene.ends_with("-sin-respuesta") {
        s.editor = Some(abrir(&c, Some(&better)));
    }
    if scene.contains("-agregar") {
        let error = scene.ends_with("-error");
        s.editor = Some(Editor {
            id: None,
            nombre: String::new(),
            direccion: if error { "play ejemplo".into() } else { "play.ejemplo.cl".into() },
            instancia: None,
            error: None,
            sincronizar: false,
            prueba: if error {
                Prueba::Formato
            } else {
                Prueba::Responde(servidores::Estado {
                    version: "1.21.4".into(),
                    protocolo: 769,
                    jugadores: 14,
                    maximo: 50,
                    motd: "Bienvenidos a ejemplo".into(),
                    ms: 46,
                })
            },
        });
    }
    s.registros = vec![a, b, c];
    if scene.contains("-amigos") {
        crate::con_amigos::preparar_diseno(app, scene);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct DatosTemporales(std::path::PathBuf);
    impl DatosTemporales {
        fn nuevos() -> Self {
            let id = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            Self(std::env::temp_dir().join(format!("icaro-selector-servidores-{}-{id}", std::process::id())))
        }
    }
    impl Drop for DatosTemporales {
        fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
    }
    #[test]
    fn selector_recarga_instancias_del_disco_y_conserva_java_por_identidad() {
        let temp = DatosTemporales::nuevos();
        let store = icaro_core::instancias::Almacen::nuevo(temp.0.clone());
        let primera = store.crear_con_loader("Primera", "1.21.1", "fabric", 0).unwrap();
        let mut a = app();
        a.servicios.servidores.visual = false;
        a.servicios.instancias.almacen = store.clone();
        a.servicios.instancias.registros = vec![primera.clone()];
        *crate::lista_instancias().lock().unwrap() = vec![crate::instancias_reales::Estado::datos(&store, &primera)];
        a.configs = vec![crate::config_por_defecto()];
        a.configs[0].memoria_max = 6144;
        a.grupos = vec![Some("Amigos".into())];
        a.seleccionada = Some(0);
        // Simula una instancia creada por otra copia del launcher después del inicio.
        let segunda = store.crear_con_loader("Segunda", "26.2", "fabric", 0).unwrap();
        store.guardar_configuracion(&segunda.id, &serde_json::json!({"memoria_max":8192})).unwrap();
        let _ = actualizar(&mut a, Accion::Nuevo);
        assert_eq!(a.servicios.instancias.registros.len(), 2);
        let n = a.servicios.instancias.registros.iter().position(|i| i.id == primera.id).unwrap();
        assert_eq!(a.configs[n].memoria_max, 6144);
        assert_eq!(a.grupos[n].as_deref(), Some("Amigos"));
        assert_eq!(a.seleccionada, Some(n));
        let n = a.servicios.instancias.registros.iter().position(|i| i.id == segunda.id).unwrap();
        assert_eq!(a.configs[n].memoria_max, 8192);
        let _ = actualizar(&mut a, Accion::Instancia(Instancia { id: segunda.id.clone(), nombre: segunda.nombre }));
        assert_eq!(a.servicios.servidores.editor.as_ref().unwrap().instancia.as_ref().unwrap().id, segunda.id);
    }
    #[test]
    fn refresco_conserva_el_estado_y_la_edicion_sin_guardar() {
        let mut a = app();
        let _ = actualizar(&mut a, Accion::Editar(0));
        let _ = actualizar(&mut a, Accion::Nombre("Nombre sin guardar".into()));
        let estados = a.servicios.servidores.estados.len();
        let _ = actualizar(&mut a, Accion::Abrir);
        assert_eq!(a.servicios.servidores.editor.as_ref().unwrap().nombre, "Nombre sin guardar");
        assert_eq!(a.servicios.servidores.estados.len(), estados);
        assert!(a.servicios.servidores.zero.is_some());
        assert!(!a.servicios.servidores.loading);
    }
    #[test]
    fn la_barra_se_puede_plegar_y_desplegar_en_ventana_angosta() {
        let mut a = app();
        let _ = crate::actualizar_estado(&mut a, Mensaje::Tamano(iced::Size::new(940.0, 720.0)));
        assert!(a.barra_colapsada());
        let _ = crate::actualizar_estado(&mut a, Mensaje::AlternarBarra);
        assert!(!a.barra_colapsada());
        let _ = crate::actualizar_estado(&mut a, Mensaje::AlternarBarra);
        assert!(a.barra_colapsada());
    }

    #[test]
    fn al_reabrir_se_relee_la_lista_y_se_descarta_el_estado_antiguo_de_zerotier() {
        let mut a = app();
        let _ = actualizar(&mut a, Accion::Abrir);
        assert!(a.servicios.servidores.loading);
        a.servicios.servidores.zero_request = 4;
        let _ = actualizar(&mut a, Accion::Zero(3, zerotier::Estado::Ausente));
        assert!(matches!(a.servicios.servidores.zero, Some(zerotier::Estado::Disponible { .. })));
        let _ = actualizar(&mut a, Accion::Zero(4, zerotier::Estado::Ausente));
        assert!(matches!(a.servicios.servidores.zero, Some(zerotier::Estado::Ausente)));
    }
    fn app() -> App {
        let (mut a, _) = crate::iniciar_con_guardado(
            icaro_ui::shell::Seccion::Servidores,
            true,
            None,
            false,
            crate::servicios::Estado::nuevo(true),
            None,
        );
        preparar_diseno(&mut a, "servidores");
        a.servicios.demo = false;
        a
    }
    #[test]
    fn respuestas_antiguas_y_servidores_eliminados_no_regresan_a_la_lista() {
        let mut a = app();
        let id = a.servicios.servidores.registros[0].id.clone();
        a.servicios.servidores.request = 7;
        let _ = actualizar(&mut a, Accion::Estado(6, id.clone(), Err("antiguo".into())));
        assert!(a.servicios.servidores.estados[&id].is_ok());
        a.servicios.servidores.registros.remove(0);
        a.servicios.servidores.estados.remove(&id);
        let _ = actualizar(
            &mut a,
            Accion::Estado(7, id.clone(), Err("eliminado".into())),
        );
        assert!(!a.servicios.servidores.estados.contains_key(&id));
    }
    #[test]
    fn fallo_de_guardado_conserva_la_lista_y_no_cierra_el_editor() {
        let mut a = app();
        let previous = a.servicios.servidores.registros.clone();
        let _ = actualizar(&mut a, Accion::Editar(0));
        a.servicios.servidores.saving = true;
        a.servicios.servidores.revision = 4;
        let _ = actualizar(&mut a, Accion::Guardados(4, Err("Disco ocupado".into())));
        assert_eq!(a.servicios.servidores.registros, previous);
        assert!(a
            .servicios
            .servidores
            .editor
            .as_ref()
            .unwrap()
            .error
            .is_some());
        assert!(!a.servicios.servidores.saving);
    }
    #[test]
    fn la_maqueta_no_guarda_servidores_ni_lanza_el_juego() {
        let mut a = app();
        a.servicios.demo = true;
        let _ = actualizar(&mut a, Accion::Jugar(0));
        assert!(a.servicios.juego.trabajos.is_empty());
        let _ = actualizar(&mut a, Accion::Nuevo);
        assert!(a.servicios.servidores.editor.is_none());
    }
    #[test]
    fn quitar_saca_la_fila_deslizandose_y_guarda_sin_ella() {
        let mut a = app();
        a.reducir = false;
        let id = a.servicios.servidores.registros[0].id.clone();
        let _ = actualizar(&mut a, Accion::PedirQuitar(id.clone()));
        assert_eq!(a.servicios.servidores.confirmar.as_deref(), Some(id.as_str()));
        let _ = actualizar(&mut a, Accion::Quitar(id.clone()));
        let s = &a.servicios.servidores;
        assert!(s.confirmar.is_none());
        assert!(s.saving);
        assert_eq!(s.saliendo.as_ref().map(|(v, i, _)| (v.id.clone(), *i)), Some((id, 0)));
    }
    #[test]
    fn la_direccion_se_prueba_mientras_se_escribe() {
        let mut a = app();
        let _ = actualizar(&mut a, Accion::Nuevo);
        let prueba = |a: &App| a.servicios.servidores.editor.as_ref().unwrap().prueba.clone();
        assert!(matches!(prueba(&a), Prueba::Vacia));
        let _ = actualizar(&mut a, Accion::Direccion("play ejemplo".into()));
        assert!(matches!(prueba(&a), Prueba::Formato));
        let _ = actualizar(&mut a, Accion::Direccion("play.ejemplo.cl".into()));
        assert!(matches!(prueba(&a), Prueba::Probando));
        // Una respuesta vieja no pisa la prueba vigente.
        let _ = actualizar(&mut a, Accion::Probado(0, Err("x".into())));
        assert!(matches!(prueba(&a), Prueba::Probando));
        let g = a.servicios.servidores.prueba_gen;
        let _ = actualizar(&mut a, Accion::Probado(g, Err("x".into())));
        assert!(matches!(prueba(&a), Prueba::NoResponde));
        assert_eq!(nombre_de("play.ejemplo.cl"), "Ejemplo");
        assert_eq!(nombre_de("10.0.0.5:25565"), "10.0.0.5:25565");
    }
    #[test]
    fn el_interruptor_de_sincronizacion_se_guarda_en_la_fila_abierta() {
        let mut a = app();
        let _ = actualizar(&mut a, Accion::Editar(0));
        assert!(!a.servicios.servidores.editor.as_ref().unwrap().sincronizar);
        let _ = actualizar(&mut a, Accion::Sincronizar(true));
        let s = &a.servicios.servidores;
        // La fila sigue abierta y se pidió guardar el valor nuevo.
        assert!(s.editor.as_ref().unwrap().sincronizar);
        assert!(s.saving);
    }
    #[test]
    fn las_flechas_cambian_de_pestana() {
        let mut a = app();
        let _ = actualizar(&mut a, Accion::PestanaVecina(1));
        assert_eq!(a.servicios.servidores.pestana, Pestana::ConAmigos);
        let _ = actualizar(&mut a, Accion::PestanaVecina(-1));
        let _ = actualizar(&mut a, Accion::PestanaVecina(-1));
        assert_eq!(a.servicios.servidores.pestana, Pestana::Red);
    }
    #[test]
    fn escape_cierra_panel_sin_modificar_lista_y_respeta_guardado_en_curso() {
        let mut a = app();
        a.reducir = true;
        let _ = actualizar(&mut a, Accion::Editar(0));
        let original = a.servicios.servidores.registros.clone();
        a.servicios.servidores.saving = true;
        let _ = actualizar(&mut a, Accion::Cerrar);
        assert!(a.servicios.servidores.panel_abierto());
        a.servicios.servidores.saving = false;
        let _ = actualizar(&mut a, Accion::Cerrar);
        assert!(!a.servicios.servidores.panel_abierto());
        assert_eq!(a.servicios.servidores.registros, original);
    }
}
