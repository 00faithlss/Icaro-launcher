// SPDX-License-Identifier: GPL-3.0-only
//! Alojar una instancia compartida como servidor dedicado
//! y entrar al servidor de un amigo por el túnel de Compartir.
//!
//! El anfitrión abre el servidor desde el detalle de lo que comparte; Ícaro lo
//! prepara (`icaro_core::alojamiento`), lo ejecuta y lo publica solo a sus
//! invitados. El invitado ve si está abierto en el detalle de lo que recibe y
//! entra con un puerto local que su juego usa como dirección.
use crate::{App, Mensaje};
use icaro_core::{
    alojamiento::{self as al, proceso, propiedades::Propiedades},
    compartir::{self as c, identidad::Identidad, transporte::servidor::Tunel},
    instancias::Instancia,
    java, minecraft,
};
use iced::Task;
use std::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
};
use tokio_util::sync::CancellationToken;

/// Líneas de consola que se conservan en memoria.
const CONSOLA_MAX: usize = 400;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Fase {
    #[default]
    Apagado,
    Preparando(String),
    Iniciando,
    Abierto,
    Deteniendo,
    Error(String),
}
impl Fase {
    pub fn en_marcha(&self) -> bool {
        matches!(self, Fase::Preparando(_) | Fase::Iniciando | Fase::Abierto | Fase::Deteniendo)
    }
}

/// Página «Ajustes del servidor»: `server.properties` y memoria.
#[derive(Debug, Clone)]
pub struct Editor {
    pub instancia: String,
    pub propiedades: Propiedades,
    pub ajustes: al::Ajustes,
    pub sucio: bool,
    pub guardando: bool,
    /// Desactivar online-mode pide confirmación en el mismo lugar.
    pub confirmar_online: bool,
}

/// Servidor del anfitrión visto por un invitado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Remoto {
    Comprobando,
    Abierto,
    Cerrado,
    SinConexion,
}

#[derive(Default)]
pub struct Estado {
    /// Instancia del servidor de este equipo (uno a la vez).
    pub instancia: Option<String>,
    pub fase: Fase,
    pub consola: VecDeque<String>,
    pub jugadores: Vec<String>,
    pub puerto: Option<u16>,
    servidor: Option<proceso::Servidor>,
    generacion: u64,
    cancel: Option<CancellationToken>,
    /// Instancia que espera aceptar el EULA antes de abrir.
    pub eula: Option<String>,
    pub editor: Option<Editor>,
    pub comando: String,
    pub cerrar_al_terminar: bool,
    // Invitado
    pub remotos: BTreeMap<String, Remoto>,
    consulta_gen: u64,
    tunel: Option<(String, Arc<Tunel>)>,
    pub entrando: Option<String>,
}
impl std::fmt::Debug for Estado {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Alojamiento")
            .field("instancia", &self.instancia)
            .field("fase", &self.fase)
            .finish_non_exhaustive()
    }
}
impl Drop for Estado {
    fn drop(&mut self) {
        if let Some(c) = &self.cancel {
            c.cancel();
        }
        if let Some(s) = &self.servidor {
            s.detener();
        }
    }
}
impl Estado {
    pub fn de(&self, instancia: &str) -> Option<&Fase> {
        (self.instancia.as_deref() == Some(instancia)).then_some(&self.fase)
    }
    pub fn tunel_activo(&self, recibida: &str) -> bool {
        self.tunel.as_ref().is_some_and(|(id, t)| id == recibida && t.activo())
    }
}

#[derive(Debug, Clone)]
pub enum Accion {
    Abrir(String),
    AceptarEula,
    CancelarEula,
    Etapa(u64, String),
    Iniciado(u64, Option<proceso::Servidor>, Result<u16, String>),
    Evento(u64, proceso::Evento),
    Detener,
    Comando(String),
    Enviar,
    Tick,
    JugarPropio(String),
    // Ajustes del servidor
    Ajustes(String),
    Cargadas(String, Result<(Propiedades, al::Ajustes), String>),
    Campo(&'static str, String),
    Memoria(u32),
    PedirOffline,
    CancelarOffline,
    Guardar,
    Guardadas(Result<(), String>),
    VolverAjustes,
    AbrirCarpeta(String),
    // Invitado
    Consultar(String),
    Consultado(u64, String, Result<bool, String>),
    Entrar(String),
    Tunel(String, Result<Arc<Tunel>, String>),
}

pub(crate) fn m(a: Accion) -> Mensaje {
    Mensaje::Alojamiento(a)
}

fn instancia(app: &App, id: &str) -> Option<(usize, Instancia)> {
    app.servicios
        .instancias
        .registros
        .iter()
        .position(|i| i.id == id)
        .map(|i| (i, app.servicios.instancias.registros[i].clone()))
}

/// Publica el puerto por el túnel cuando la publicación de esa instancia está
/// activa. Se repite a menudo porque la publicación puede reconectarse.
fn sincronizar_tunel(app: &App) {
    let s = &app.servicios.servidores.alojar;
    let compartir = &app.servicios.compartir;
    let Some(h) = compartir.anfitrion.as_ref() else {
        return;
    };
    let de_esta = compartir
        .panel
        .as_ref()
        .is_some_and(|p| Some(&p.instancia.id) == s.instancia.as_ref());
    match (s.fase == Fase::Abierto, s.puerto, de_esta) {
        (true, Some(p), true) => h.abrir_servidor(Some(p)),
        (_, _, true) => h.abrir_servidor(None),
        _ => {}
    }
}

fn cerrar_tunel_publicado(app: &App) {
    if let Some(h) = app.servicios.compartir.anfitrion.as_ref() {
        h.abrir_servidor(None);
    }
}

pub fn actualizar(app: &mut App, a: Accion) -> Task<Mensaje> {
    match a {
        Accion::Abrir(id) => {
            if app.servicios.demo {
                let s = &mut app.servicios.servidores.alojar;
                s.instancia = Some(id);
                s.fase = Fase::Abierto;
                s.puerto = Some(25565);
                return Task::none();
            }
            if app.servicios.servidores.alojar.fase.en_marcha() {
                app.avisar("Ya hay un servidor abierto. Ciérralo antes de abrir otro.".into());
                return Task::none();
            }
            let Some((_, inst)) = instancia(app, &id) else {
                return Task::none();
            };
            if !al::loader_alojable(&inst.loader) {
                app.avisar_error("Por ahora solo se pueden alojar instancias Vanilla o Fabric.".into());
                return Task::none();
            }
            let Ok(carpeta) = al::carpeta(&app.servicios.instancias.almacen, &id) else {
                return Task::none();
            };
            if !al::eula_aceptado(&carpeta) {
                app.servicios.servidores.alojar.eula = Some(id);
                return Task::none();
            }
            iniciar(app, inst)
        }
        Accion::CancelarEula => {
            app.servicios.servidores.alojar.eula = None;
            Task::none()
        }
        Accion::AceptarEula => {
            let Some(id) = app.servicios.servidores.alojar.eula.take() else {
                return Task::none();
            };
            let Some((_, inst)) = instancia(app, &id) else {
                return Task::none();
            };
            let aceptado = al::carpeta(&app.servicios.instancias.almacen, &id).and_then(|c| al::aceptar_eula(&c));
            if let Err(e) = aceptado {
                app.avisar_error(e.to_string());
                return Task::none();
            }
            iniciar(app, inst)
        }
        Accion::Etapa(g, texto) => {
            let s = &mut app.servicios.servidores.alojar;
            if s.generacion == g && matches!(s.fase, Fase::Preparando(_)) {
                s.fase = Fase::Preparando(texto);
            }
            Task::none()
        }
        Accion::Iniciado(g, servidor, r) => {
            let s = &mut app.servicios.servidores.alojar;
            if s.generacion != g {
                if let Some(sv) = servidor {
                    sv.detener();
                }
                return Task::none();
            }
            match r {
                Ok(puerto) => {
                    s.servidor = servidor;
                    s.puerto = Some(puerto);
                    s.fase = Fase::Iniciando;
                }
                Err(e) => {
                    s.fase = Fase::Error(e);
                    s.cancel = None;
                }
            }
            Task::none()
        }
        Accion::Evento(g, e) => {
            let s = &mut app.servicios.servidores.alojar;
            if s.generacion != g {
                return Task::none();
            }
            match e {
                proceso::Evento::Linea(l) => {
                    match al::jugador(&l) {
                        Some(al::Jugador::Entra(n)) if !s.jugadores.contains(&n) => s.jugadores.push(n),
                        Some(al::Jugador::Sale(n)) => s.jugadores.retain(|j| *j != n),
                        _ => {}
                    }
                    if s.consola.len() >= CONSOLA_MAX {
                        s.consola.pop_front();
                    }
                    s.consola.push_back(l);
                }
                proceso::Evento::Listo => {
                    if s.fase == Fase::Iniciando {
                        s.fase = Fase::Abierto;
                    }
                    sincronizar_tunel(app);
                }
                proceso::Evento::Fin(codigo) => {
                    let deteniendo = s.fase == Fase::Deteniendo;
                    s.servidor = None;
                    s.cancel = None;
                    s.jugadores.clear();
                    s.fase = match codigo {
                        _ if deteniendo => Fase::Apagado,
                        Some(0) => Fase::Apagado,
                        Some(c) => Fase::Error(format!("El servidor se cerró con el código {c}. Revisa la consola.")),
                        None => Fase::Error("El servidor se cerró de golpe. Revisa la consola.".into()),
                    };
                    let cerrar = s.cerrar_al_terminar;
                    cerrar_tunel_publicado(app);
                    if cerrar {
                        return Task::done(Mensaje::Cerrar);
                    }
                }
            }
            Task::none()
        }
        Accion::Detener => {
            let s = &mut app.servicios.servidores.alojar;
            match s.fase {
                Fase::Preparando(_) => {
                    if let Some(c) = s.cancel.take() {
                        c.cancel();
                    }
                    s.generacion += 1;
                    s.fase = Fase::Apagado;
                }
                Fase::Iniciando | Fase::Abierto => {
                    if let Some(sv) = &s.servidor {
                        sv.detener();
                    }
                    s.fase = Fase::Deteniendo;
                }
                _ => {}
            }
            cerrar_tunel_publicado(app);
            Task::none()
        }
        Accion::Comando(t) => {
            app.servicios.servidores.alojar.comando = t;
            Task::none()
        }
        Accion::Enviar => {
            let s = &mut app.servicios.servidores.alojar;
            let texto = std::mem::take(&mut s.comando);
            if let Some(sv) = &s.servidor {
                if sv.comando(&texto) {
                    s.consola.push_back(format!("> {}", texto.trim().trim_start_matches('/')));
                }
            }
            Task::none()
        }
        Accion::Tick => {
            sincronizar_tunel(app);
            Task::none()
        }
        Accion::JugarPropio(id) => {
            let Some(p) = app.servicios.servidores.alojar.puerto else {
                return Task::none();
            };
            let Some((index, _)) = instancia(app, &id) else {
                return Task::none();
            };
            if app.servicios.demo {
                return Task::none();
            }
            crate::juego_real::iniciar_con_servidor(app, index, true, Some(format!("127.0.0.1:{p}")))
        }
        Accion::Ajustes(id) => {
            if app.servicios.demo {
                app.servicios.servidores.alojar.editor = Some(Editor {
                    instancia: id,
                    propiedades: Propiedades::nuevas(25565),
                    ajustes: al::Ajustes::default(),
                    sucio: false,
                    guardando: false,
                    confirmar_online: false,
                });
                return Task::none();
            }
            let Ok(carpeta) = al::carpeta(&app.servicios.instancias.almacen, &id) else {
                return Task::none();
            };
            Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        al::leer_propiedades(&carpeta)
                            .map(|p| (p, al::leer_ajustes(&carpeta)))
                            .map_err(|e| e.to_string())
                    })
                    .await
                    .unwrap_or_else(|_| Err("No se pudo leer el servidor.".into()))
                },
                move |r| m(Accion::Cargadas(id.clone(), r)),
            )
        }
        Accion::Cargadas(id, r) => {
            match r {
                Ok((propiedades, ajustes)) => {
                    app.servicios.servidores.alojar.editor = Some(Editor {
                        instancia: id,
                        propiedades,
                        ajustes,
                        sucio: false,
                        guardando: false,
                        confirmar_online: false,
                    })
                }
                Err(e) => app.avisar_error(e),
            }
            Task::none()
        }
        Accion::Campo(clave, valor) => {
            if let Some(e) = &mut app.servicios.servidores.alojar.editor {
                e.propiedades.fijar(clave, &valor);
                e.sucio = true;
                if clave == "online-mode" {
                    e.confirmar_online = false;
                }
            }
            Task::none()
        }
        Accion::Memoria(mb) => {
            if let Some(e) = &mut app.servicios.servidores.alojar.editor {
                e.ajustes.memoria_mb = mb;
                e.sucio = true;
            }
            Task::none()
        }
        Accion::PedirOffline => {
            if let Some(e) = &mut app.servicios.servidores.alojar.editor {
                e.confirmar_online = true;
            }
            Task::none()
        }
        Accion::CancelarOffline => {
            if let Some(e) = &mut app.servicios.servidores.alojar.editor {
                e.confirmar_online = false;
            }
            Task::none()
        }
        Accion::Guardar => {
            let Some(e) = &mut app.servicios.servidores.alojar.editor else {
                return Task::none();
            };
            if app.servicios.demo {
                e.sucio = false;
                return Task::none();
            }
            let Ok(carpeta) = al::carpeta(&app.servicios.instancias.almacen, &e.instancia) else {
                return Task::none();
            };
            e.guardando = true;
            let (p, a) = (e.propiedades.clone(), e.ajustes.clone());
            Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        al::guardar_propiedades(&carpeta, &p)
                            .and_then(|_| al::guardar_ajustes(&carpeta, &a))
                            .map_err(|e| e.to_string())
                    })
                    .await
                    .unwrap_or_else(|_| Err("No se pudo guardar.".into()))
                },
                |r| m(Accion::Guardadas(r)),
            )
        }
        Accion::Guardadas(r) => {
            let abierto = app.servicios.servidores.alojar.fase.en_marcha();
            if let Some(e) = &mut app.servicios.servidores.alojar.editor {
                e.guardando = false;
                match r {
                    Ok(()) => {
                        e.sucio = false;
                        app.avisar(if abierto {
                            "Ajustes guardados. Se aplican al volver a abrir el servidor.".into()
                        } else {
                            "Ajustes del servidor guardados.".into()
                        });
                    }
                    Err(e) => app.avisar_error(e),
                }
            }
            Task::none()
        }
        Accion::VolverAjustes => {
            app.servicios.servidores.alojar.editor = None;
            Task::none()
        }
        Accion::AbrirCarpeta(id) => {
            if let Ok(carpeta) = al::carpeta(&app.servicios.instancias.almacen, &id) {
                let _ = std::fs::create_dir_all(&carpeta);
                let programa = if cfg!(windows) { "explorer" } else { "xdg-open" };
                if std::process::Command::new(programa).arg(carpeta).spawn().is_err() {
                    app.avisar("No se pudo abrir la carpeta del servidor.".into());
                }
            }
            Task::none()
        }
        Accion::Consultar(id) => consultar(app, id),
        Accion::Consultado(g, id, r) => {
            let s = &mut app.servicios.servidores.alojar;
            if s.consulta_gen == g {
                s.remotos.insert(
                    id,
                    match r {
                        Ok(true) => Remoto::Abierto,
                        Ok(false) => Remoto::Cerrado,
                        Err(_) => Remoto::SinConexion,
                    },
                );
            }
            Task::none()
        }
        Accion::Entrar(id) => entrar(app, id),
        Accion::Tunel(id, r) => {
            app.servicios.servidores.alojar.entrando = None;
            match r {
                Ok(t) => {
                    let direccion = t.direccion();
                    app.servicios.servidores.alojar.tunel = Some((id.clone(), t));
                    let Some((index, _)) = instancia(app, &id) else {
                        return Task::none();
                    };
                    crate::juego_real::iniciar_con_servidor(app, index, true, Some(direccion))
                }
                Err(e) => {
                    app.servicios.servidores.alojar.remotos.insert(id, Remoto::Cerrado);
                    app.avisar_error(format!("No se pudo entrar al servidor: {e}"));
                    Task::none()
                }
            }
        }
    }
}

/// Instala lo que falte, prepara la carpeta y arranca el proceso. Las etapas y
/// la consola llegan como mensajes con la generación de este arranque.
fn iniciar(app: &mut App, inst: Instancia) -> Task<Mensaje> {
    let store = app.servicios.instancias.almacen.clone();
    let (_, preferida) = crate::juego_real::config(app, &inst.id);
    let index = instancia(app, &inst.id).map_or(0, |(i, _)| i);
    let descarga = app
        .configs
        .get(index)
        .map(|c| crate::java_real::efectiva(c, &app.java_global).1)
        .unwrap_or(app.java_global.descarga);
    let base = crate::estado::directorio(false);
    let s = &mut app.servicios.servidores.alojar;
    if let Some(c) = s.cancel.take() {
        c.cancel();
    }
    s.generacion += 1;
    s.instancia = Some(inst.id.clone());
    s.fase = Fase::Preparando("Comprobando la instancia".into());
    s.consola.clear();
    s.jugadores.clear();
    s.puerto = None;
    let cancel = CancellationToken::new();
    s.cancel = Some(cancel.clone());
    let g = s.generacion;
    use iced::futures::SinkExt;
    Task::run(
        iced::stream::channel(64, move |mut salida| async move {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
            let etapa = move |t: &str| {
                let _ = tx.send(t.to_owned());
            };
            let preparacion = async {
                let instalado = match minecraft::leer(&store, &inst) {
                    Some(i) => i,
                    None => {
                        etapa("Instalando Minecraft");
                        minecraft::instalar_con_java(&store, &inst, preferida.as_deref(), descarga, cancel.clone(), |_| {})
                            .await
                            .map_err(icaro_core::diagnostico_contexto::texto)?
                    }
                };
                etapa("Preparando Java");
                let java = java::preparar_perfil(
                    &base,
                    &instalado.perfil_java(),
                    preferida.as_deref(),
                    descarga,
                    cancel.clone(),
                    |_| {},
                )
                .await
                .map_err(icaro_core::diagnostico_contexto::texto)?;
                let listo = al::preparar(&store, &inst, &cancel, &etapa).await.map_err(|e| e.to_string())?;
                etapa("Iniciando el servidor");
                let memoria = al::leer_ajustes(&listo.carpeta).memoria_mb;
                let (servidor, eventos) =
                    proceso::iniciar(&java.executable, &listo.carpeta, &listo.jar, memoria).map_err(|e| e.to_string())?;
                Ok::<_, String>((servidor, eventos, listo.puerto))
            };
            tokio::pin!(preparacion);
            let resultado = loop {
                tokio::select! {
                    r = &mut preparacion => break r,
                    Some(t) = rx.recv() => {
                        let _ = salida.send(m(Accion::Etapa(g, t))).await;
                    }
                }
            };
            match resultado {
                Ok((servidor, mut eventos, puerto)) => {
                    let _ = salida.send(m(Accion::Iniciado(g, Some(servidor), Ok(puerto)))).await;
                    while let Some(e) = eventos.recv().await {
                        if salida.send(m(Accion::Evento(g, e))).await.is_err() {
                            break;
                        }
                    }
                }
                Err(e) => {
                    let _ = salida.send(m(Accion::Iniciado(g, None, Err(e)))).await;
                }
            }
        }),
        |mensaje| mensaje,
    )
}

/// Origen y encuentro firmados de una instancia recibida, como al sincronizar.
async fn conexion(
    raiz: std::path::PathBuf,
    store: icaro_core::instancias::Almacen,
    id: String,
    host: Option<Arc<c::anfitrion::PublicacionActiva>>,
) -> Result<c::transporte::servidor::Conexion, String> {
    let servicio =
        crate::compartir_real::Servicio::desde_entorno().ok_or_else(|| "El servicio de invitaciones no está configurado.".to_string())?;
    let (vinculo, identidad) = tokio::task::spawn_blocking(move || {
        let juego = store.juego(&id).map_err(icaro_core::diagnostico_contexto::texto)?;
        let vinculo = c::vinculos::leer(&juego)
            .map_err(icaro_core::diagnostico_contexto::texto)?
            .ok_or_else(|| "Esta instancia ya no está vinculada.".to_string())?;
        let identidad = Identidad::cargar_o_crear(&raiz).map_err(icaro_core::diagnostico_contexto::texto)?;
        Ok::<_, String>((vinculo, identidad))
    })
    .await
    .map_err(|_| "No se pudo leer el vínculo.".to_string())??;
    let cliente =
        c::invitaciones::ClienteInvitaciones::nuevo(&servicio.url, identidad.clone()).map_err(icaro_core::diagnostico_contexto::texto)?;
    let r = cliente
        .reencontrar(&vinculo.origen)
        .await
        .map_err(icaro_core::diagnostico_contexto::texto)?;
    let origen = c::Origen {
        anfitrion: r.host.clone(),
        publicacion: r.publication.clone(),
    };
    match host {
        Some(h) => h.conectar_servidor(&r.meeting, &origen).await,
        None => {
            let direccion = r
                .meeting
                .direccion(&r.host, &r.publication)
                .map_err(icaro_core::diagnostico_contexto::texto)?;
            c::transporte::conectar_servidor(&identidad, direccion, &origen, servicio.modo).await
        }
    }
    .map_err(icaro_core::diagnostico_contexto::texto)
}

fn consultar(app: &mut App, id: String) -> Task<Mensaje> {
    if app.servicios.demo || !app.servicios.compartir.habilitado {
        return Task::none();
    }
    let s = &mut app.servicios.servidores.alojar;
    s.consulta_gen += 1;
    let g = s.consulta_gen;
    s.remotos.entry(id.clone()).or_insert(Remoto::Comprobando);
    let (raiz, store, host) = (
        crate::estado::directorio(false),
        app.servicios.instancias.almacen.clone(),
        app.servicios.compartir.anfitrion.clone(),
    );
    let id_r = id.clone();
    Task::perform(
        async move {
            let conexion = tokio::time::timeout(std::time::Duration::from_secs(40), conexion(raiz, store, id_r, host))
                .await
                .map_err(|_| "Sin respuesta.".to_string())??;
            conexion.abierto().await.map_err(icaro_core::diagnostico_contexto::texto)
        },
        move |r| m(Accion::Consultado(g, id.clone(), r)),
    )
}

fn entrar(app: &mut App, id: String) -> Task<Mensaje> {
    if app.servicios.demo {
        return Task::none();
    }
    let s = &mut app.servicios.servidores.alojar;
    if s.entrando.is_some() {
        return Task::none();
    }
    // Un túnel vivo hacia esa misma instancia se reutiliza.
    if let Some((tid, t)) = &s.tunel {
        if *tid == id && t.activo() {
            let direccion = t.direccion();
            let Some((index, _)) = instancia(app, &id) else {
                return Task::none();
            };
            return crate::juego_real::iniciar_con_servidor(app, index, true, Some(direccion));
        }
    }
    s.tunel = None;
    s.entrando = Some(id.clone());
    let (raiz, store, host) = (
        crate::estado::directorio(false),
        app.servicios.instancias.almacen.clone(),
        app.servicios.compartir.anfitrion.clone(),
    );
    let id_r = id.clone();
    Task::perform(
        async move {
            let conexion = tokio::time::timeout(std::time::Duration::from_secs(40), conexion(raiz, store, id_r, host))
                .await
                .map_err(|_| "Sin respuesta del anfitrión.".to_string())??;
            if !conexion.abierto().await.map_err(icaro_core::diagnostico_contexto::texto)? {
                return Err("El servidor de tu amigo está cerrado.".into());
            }
            conexion
                .tunel()
                .await
                .map(Arc::new)
                .map_err(icaro_core::diagnostico_contexto::texto)
        },
        move |r| m(Accion::Tunel(id.clone(), r)),
    )
}

/// Cerrar Ícaro con el servidor abierto: primero `stop` para guardar el mundo.
/// Devuelve verdadero si el cierre debe esperar.
pub fn al_cerrar(app: &mut App) -> bool {
    let s = &mut app.servicios.servidores.alojar;
    match s.fase {
        Fase::Iniciando | Fase::Abierto | Fase::Deteniendo if s.servidor.is_some() => {
            s.cerrar_al_terminar = true;
            if s.fase != Fase::Deteniendo {
                if let Some(sv) = &s.servidor {
                    sv.detener();
                }
                s.fase = Fase::Deteniendo;
            }
            app.avisar("Cerrando el servidor y guardando el mundo…".into());
            true
        }
        Fase::Preparando(_) => {
            if let Some(c) = s.cancel.take() {
                c.cancel();
            }
            s.generacion += 1;
            s.fase = Fase::Apagado;
            false
        }
        _ => false,
    }
}

pub fn suscripcion(app: &App) -> iced::Subscription<Mensaje> {
    if app.servicios.servidores.alojar.fase == Fase::Abierto {
        iced::time::every(std::time::Duration::from_secs(2)).map(|_| m(Accion::Tick))
    } else {
        iced::Subscription::none()
    }
}

/// Escenas de diseño: `-servidor` (abierto con consola), `-servidor-eula`,
/// `-servidor-ajustes` y `-recibida-servidor` (el del amigo abierto).
pub fn preparar_diseno(app: &mut App, scene: &str) {
    let s = &mut app.servicios.servidores.alojar;
    if scene.ends_with("-recibida-servidor") {
        s.remotos.insert("create".into(), Remoto::Abierto);
    } else if scene.ends_with("-servidor") {
        s.instancia = Some("amigos".into());
        s.fase = Fase::Abierto;
        s.puerto = Some(25565);
        s.jugadores = vec!["Forjador".into(), "Pato_Lucas".into()];
        for l in [
            "[18:02:11] [Server thread/INFO]: Starting minecraft server version 1.21.1",
            "[18:02:11] [Server thread/INFO]: Loading properties",
            "[18:02:12] [Server thread/INFO]: Preparing level \"world\"",
            "[18:02:19] [Server thread/INFO]: Done (7.412s)! For help, type \"help\"",
            "[18:04:40] [Server thread/INFO]: Forjador joined the game",
            "[18:05:02] [Server thread/INFO]: Pato_Lucas joined the game",
            "[18:05:31] [Server thread/INFO]: <Forjador> llegué, ¿vamos al nether?",
        ] {
            s.consola.push_back(l.into());
        }
    } else if scene.ends_with("-servidor-eula") {
        s.eula = Some("amigos".into());
    } else if scene.ends_with("-servidor-ajustes") {
        let mut p = Propiedades::nuevas(25565);
        p.fijar("motd", "Amigos · temporada 3");
        s.editor = Some(Editor {
            instancia: "amigos".into(),
            propiedades: p,
            ajustes: al::Ajustes { memoria_mb: 4096 },
            sucio: true,
            guardando: false,
            confirmar_online: false,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(demo: bool) -> App {
        let (mut a, _) = crate::iniciar_con_guardado(
            icaro_ui::shell::Seccion::Servidores,
            true,
            None,
            false,
            crate::servicios::Estado::nuevo(true),
            None,
        );
        a.servicios.demo = demo;
        a
    }

    fn temporal() -> std::path::PathBuf {
        let id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("icaro-alojar-{}-{id}", std::process::id()))
    }

    #[test]
    fn abrir_sin_eula_lo_pide_en_el_mismo_lugar_y_cancelar_no_escribe_nada() {
        let raiz = temporal();
        let store = icaro_core::instancias::Almacen::nuevo(raiz.clone());
        let inst = store.crear_con_loader("Amigos", "1.21.1", "fabric", 0).unwrap();
        let mut a = app(false);
        a.servicios.instancias.almacen = store.clone();
        a.servicios.instancias.registros = vec![inst.clone()];
        let _ = actualizar(&mut a, Accion::Abrir(inst.id.clone()));
        assert_eq!(a.servicios.servidores.alojar.eula.as_deref(), Some(inst.id.as_str()));
        assert_eq!(a.servicios.servidores.alojar.fase, Fase::Apagado);
        let _ = actualizar(&mut a, Accion::CancelarEula);
        assert!(a.servicios.servidores.alojar.eula.is_none());
        assert!(!al::eula_aceptado(&al::carpeta(&store, &inst.id).unwrap()));
        let _ = std::fs::remove_dir_all(raiz);
    }

    #[test]
    fn una_instancia_forge_no_se_aloja() {
        let raiz = temporal();
        let store = icaro_core::instancias::Almacen::nuevo(raiz.clone());
        let inst = store.crear_con_loader("Pesada", "1.20.1", "forge", 0).unwrap();
        let mut a = app(false);
        a.servicios.instancias.almacen = store;
        a.servicios.instancias.registros = vec![inst.clone()];
        let _ = actualizar(&mut a, Accion::Abrir(inst.id));
        assert!(a.servicios.servidores.alojar.eula.is_none());
        assert_eq!(a.servicios.servidores.alojar.fase, Fase::Apagado);
        let _ = std::fs::remove_dir_all(raiz);
    }

    #[test]
    fn la_consola_sigue_a_los_jugadores_y_un_cierre_pedido_no_es_error() {
        let mut a = app(false);
        let s = &mut a.servicios.servidores.alojar;
        s.instancia = Some("amigos".into());
        s.generacion = 3;
        s.fase = Fase::Iniciando;
        let ev = |l: &str| Accion::Evento(3, proceso::Evento::Linea(l.into()));
        let _ = actualizar(&mut a, Accion::Evento(3, proceso::Evento::Listo));
        assert_eq!(a.servicios.servidores.alojar.fase, Fase::Abierto);
        let _ = actualizar(&mut a, ev("[1] [Server thread/INFO]: Pato joined the game"));
        let _ = actualizar(&mut a, ev("[1] [Server thread/INFO]: Sombra joined the game"));
        let _ = actualizar(&mut a, ev("[1] [Server thread/INFO]: Pato left the game"));
        assert_eq!(a.servicios.servidores.alojar.jugadores, vec!["Sombra".to_string()]);
        // Un evento de un arranque anterior se ignora.
        let _ = actualizar(&mut a, Accion::Evento(2, proceso::Evento::Fin(Some(1))));
        assert_eq!(a.servicios.servidores.alojar.fase, Fase::Abierto);
        let _ = actualizar(&mut a, Accion::Detener);
        assert_eq!(a.servicios.servidores.alojar.fase, Fase::Deteniendo);
        let _ = actualizar(&mut a, Accion::Evento(3, proceso::Evento::Fin(Some(1))));
        assert_eq!(a.servicios.servidores.alojar.fase, Fase::Apagado);
        assert!(a.servicios.servidores.alojar.jugadores.is_empty());
    }

    #[test]
    fn un_cierre_inesperado_muestra_el_error() {
        let mut a = app(false);
        let s = &mut a.servicios.servidores.alojar;
        s.instancia = Some("amigos".into());
        s.fase = Fase::Abierto;
        let _ = actualizar(&mut a, Accion::Evento(0, proceso::Evento::Fin(Some(1))));
        assert!(matches!(a.servicios.servidores.alojar.fase, Fase::Error(_)));
    }

    #[test]
    fn el_editor_cambia_propiedades_y_pide_confirmar_sin_conexion() {
        let mut a = app(true);
        let _ = actualizar(&mut a, Accion::Ajustes("amigos".into()));
        let _ = actualizar(&mut a, Accion::Campo("max-players", "4".into()));
        let _ = actualizar(&mut a, Accion::PedirOffline);
        let e = a.servicios.servidores.alojar.editor.as_ref().unwrap();
        assert!(e.sucio && e.confirmar_online);
        assert!(e.propiedades.bool("online-mode"));
        let _ = actualizar(&mut a, Accion::Campo("online-mode", "false".into()));
        let e = a.servicios.servidores.alojar.editor.as_ref().unwrap();
        assert!(!e.confirmar_online && !e.propiedades.bool("online-mode"));
        assert_eq!(e.propiedades.numero("max-players"), Some(4));
    }
}
