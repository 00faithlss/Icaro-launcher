// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Preparación de Vanilla y proceso del juego, usando las vistas del System Design.

use crate::{App, Mensaje};
use icaro_core::{auth::CancellationToken, instancias::Instancia, java, lanzamiento, minecraft};
use icaro_ui::{
    componentes::{self, EstadoJugar},
    consola::{self, Linea, Nivel},
    descargas::{self, Descarga, EstadoDescarga},
    fuentes,
    iconos::{icono, Icono, Tam},
    instancias::EstadoInstancia,
    laminas::{banda_completa, DatoBanda, Lamina, ALTO_BANDA},
    tema::{espacio, texto},
};
use iced::{
    futures::SinkExt,
    widget::{column, container, row, text, Space},
    Alignment, Element, Length, Padding, Task,
};
#[cfg(not(windows))]
use std::time::{SystemTime, UNIX_EPOCH};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};

#[derive(Debug, Clone)]
pub struct Preparacion {
    pub minecraft: minecraft::Instalado,
    pub java: java::Java,
}

struct Solicitud {
    instancia: Instancia,
    jugar: bool,
    java: Option<std::path::PathBuf>,
    descarga_java: bool,
}
pub struct Trabajo {
    pub nombre: String,
    pub generacion: u64,
    pub estado: EstadoDescarga,
    pub etapa: String,
    pub jugar: bool,
    cancel: CancellationToken,
    pausando: bool,
    reanudar: bool,
    preparando: bool,
    partes: Option<(u64, u64)>,
    bytes: bool,
}
pub struct Proceso {
    generacion: u64,
    cancel: CancellationToken,
    desde: Instant,
    cerrar_hasta: Option<Instant>,
    pid: Option<u32>,
    ventana_lista: bool,
}

struct RetiradaUmbral {
    id: String,
    generacion: u64,
    desde: Instant,
    fallo: bool,
    minimizar: bool,
    consola: bool,
}

#[derive(Default)]
pub struct Estado {
    pub trabajos: BTreeMap<String, Trabajo>,
    pub orden: Vec<String>,
    pendientes: VecDeque<Solicitud>,
    activa: Option<String>,
    secuencia: u64,
    pub procesos: BTreeMap<String, Proceso>,
    pub logs: BTreeMap<String, Vec<Linea>>,
    pub java: BTreeMap<String, java::Java>,
    pub preparada: BTreeMap<String, minecraft::Instalado>,
    pub errores: BTreeMap<String, String>,
    pub ultimo: Option<String>,
    espera_sesion: Option<String>,
    servidores: BTreeMap<String,String>,
    cerrando: bool,
    umbral_generacion: Option<(String, u64)>,
    retirada_umbral: Option<RetiradaUmbral>,
    resultados: BTreeMap<String, (Option<lanzamiento::Salida>, String)>,
    consola_seleccionada: Option<String>,
    mostrar_ultimo_registro: bool,
    cargando_registro: Option<String>,
}
impl Drop for Estado {
    fn drop(&mut self) {
        self.cancelar_todo();
    }
}
impl Estado {
    fn quitar_registro(&mut self, index: usize) {
        let Some(id)=self.orden.get(index) else {return;};
        // Una cancelación puede tener estado Fallo mientras espera el acuse.
        if self.activa.as_ref()==Some(id) || self.pendientes.iter().any(|s| &s.instancia.id==id) {
            return;
        }
        if self.trabajos.get(id).is_some_and(|t|matches!(t.estado,EstadoDescarga::Completada|EstadoDescarga::Fallo(_))) {
            self.orden.remove(index);
        }
    }
    fn indicador_descarga(&self) -> Option<icaro_ui::shell::DescargasActivas> {
        let t = self.trabajos.get(self.activa.as_ref()?)?;
        if t.preparando || t.cancel.is_cancelled() {
            return None;
        }
        let EstadoDescarga::Activa(avance) = t.estado else {
            return None;
        };
        Some(icaro_ui::shell::DescargasActivas {
            cantidad: 1,
            velocidad: format!("{:.0} %", avance * 100.),
            avance,
        })
    }
    fn progreso(
        &mut self,
        id: &str,
        g: u64,
        text: String,
        done: u64,
        total: u64,
    ) -> Option<(f32, String)> {
        if self.activa.as_deref() != Some(id) {
            return None;
        }
        let t = self.trabajos.get_mut(id).filter(|t| {
            t.generacion == g
                && !t.cancel.is_cancelled()
                && matches!(t.estado, EstadoDescarga::Activa(_))
        })?;
        let ratio = if total == 0 {
            0.
        } else {
            (done as f32 / total as f32).clamp(0., 1.)
        };
        t.partes = (total > 0).then_some((done, total));
        t.bytes = text == "Descargando Java";
        t.etapa = if total > 0 && text == "Descargando Java" {
            format!("{text} · {} de {} MB", decimal_mb(done), decimal_mb(total))
        } else if total > 0 {
            format!("{text} · {done} de {total}")
        } else {
            text
        };
        // Los archivos se descargan y verifican intercalados. Una vez que
        // empieza la descarga, esos eventos no deben alternar el botón entero.
        t.preparando &= total == 0 || !t.etapa.starts_with("Descargando");
        t.estado = EstadoDescarga::Activa(ratio);
        Some((ratio, t.etapa.clone()))
    }
    /// Devuelve true solamente cuando Pausar representa el botón Reanudar.
    fn suspender(&mut self, id: &str, pausando: bool) -> bool {
        if pausando
            && self
                .trabajos
                .get(id)
                .is_some_and(|t| matches!(t.estado, EstadoDescarga::Pausada(_)))
        {
            return true;
        }
        if let Some(t) = self.trabajos.get_mut(id) {
            let avance = match t.estado {
                EstadoDescarga::Activa(p) | EstadoDescarga::Pausada(p) => p,
                _ => 0.,
            };
            t.pausando = pausando;
            t.reanudar = false;
            t.cancel.cancel();
            t.estado = if pausando {
                EstadoDescarga::Pausada(avance)
            } else {
                EstadoDescarga::Fallo(
                    "Instalación cancelada; los archivos verificados se conservan.".into(),
                )
            };
            t.etapa = if pausando {
                "Instalación en pausa"
            } else {
                "Instalación cancelada"
            }
            .into();
        }
        self.pendientes.retain(|s| s.instancia.id != id);
        false
    }
    fn reanudar_pendiente(&mut self, id: &str) -> bool {
        if self.activa.as_deref() != Some(id) {
            return false;
        }
        if let Some(t) = self.trabajos.get_mut(id) {
            if t.cancel.is_cancelled() {
                t.reanudar = true;
                t.etapa = "Esperando para reanudar".into();
            }
        }
        true
    }
    pub fn cancelar_todo(&mut self) {
        self.servidores.clear();
        self.pendientes.clear();
        self.espera_sesion = None;
        for t in self.trabajos.values_mut() {
            t.cancel.cancel();
            t.reanudar = false;
            t.pausando = false;
            if matches!(
                t.estado,
                EstadoDescarga::Activa(_) | EstadoDescarga::EnCola(_) | EstadoDescarga::Pausada(_)
            ) {
                t.estado = EstadoDescarga::Fallo(
                    "Instalación cancelada; los archivos verificados se conservan.".into(),
                );
                t.etapa = "Instalación cancelada".into();
            }
        }
        for p in self.procesos.values() {
            p.cancel.cancel();
        }
        if let Some(r) = &mut self.retirada_umbral {
            r.minimizar = false;
            r.consola = false;
        }
    }
    pub fn ocupada(&self, id: &str) -> bool {
        self.procesos.contains_key(id)
            || self.activa.as_deref() == Some(id)
            || self.pendientes.iter().any(|s| s.instancia.id == id)
    }
    pub fn permite_cambiar_cuenta(&self) -> bool {
        self.libre() && self.pendientes.is_empty()
    }
    fn libre(&self) -> bool {
        self.activa.is_none() && self.procesos.is_empty()
    }

    fn ventana_lista(&mut self, id: &str, g: u64, pid: u32) -> bool {
        let Some(p) = self
            .procesos
            .get_mut(id)
            .filter(|p| p.generacion == g && p.pid == Some(pid) && !p.cancel.is_cancelled())
        else {
            return false;
        };
        p.ventana_lista = true;
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultadoVentana {
    Lista,
    SinConfirmar,
    Finalizada,
    Cancelada,
}

#[derive(Debug, Clone)]
pub enum Accion {
    Progreso(String, u64, String, u64, u64),
    Preparada(String, u64, Result<Preparacion, String>),
    Evento(String, u64, lanzamiento::Evento),
    Terminada(String, u64, Result<lanzamiento::Salida, String>),
    Pausar(usize),
    Cancelar(usize),
    Reintentar(usize),
    QuitarRegistro(usize),
    Detener(String),
    Ventana(String, u64, u32, ResultadoVentana),
    UmbralRetirado(String, u64, Instant),
    Copiar,
    Limpiar,
    CopiarInforme,
    SeleccionarConsola(String),
    RegistroCargado(String, Vec<Linea>),
    ExportarRegistro,
    RegistroExportado(Result<std::path::PathBuf, String>),
    AbrirRegistros,
    UltimoRegistro,
}
fn m(a: Accion) -> Mensaje {
    Mensaje::Juego(a)
}
pub fn hora() -> String {
    #[cfg(windows)]
    {
        let mut reloj = [0u16; 8];
        // SYSTEMTIME contiene ocho WORD. Kernel32 aplica la zona horaria local.
        #[link(name = "kernel32")]
        extern "system" {
            fn GetLocalTime(reloj: *mut u16);
        }
        unsafe {
            GetLocalTime(reloj.as_mut_ptr());
        }
        return format!("{:02}:{:02}:{:02}", reloj[4], reloj[5], reloj[6]);
    }
    #[cfg(not(windows))]
    {
        let s = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_secs())
            % 86400;
        format!("{:02}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    }
}
pub fn memoria_total_mb() -> u32 {
    #[cfg(windows)]
    {
        #[repr(C)]
        struct Memoria {
            longitud: u32,
            carga: u32,
            total_fisica: u64,
            disponible: u64,
            total_paginada: u64,
            disponible_paginada: u64,
            total_virtual: u64,
            disponible_virtual: u64,
            extendida: u64,
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn GlobalMemoryStatusEx(memoria: *mut Memoria) -> i32;
        }
        let mut memoria = Memoria {
            longitud: std::mem::size_of::<Memoria>() as u32,
            carga: 0,
            total_fisica: 0,
            disponible: 0,
            total_paginada: 0,
            disponible_paginada: 0,
            total_virtual: 0,
            disponible_virtual: 0,
            extendida: 0,
        };
        if unsafe { GlobalMemoryStatusEx(&mut memoria) } != 0 {
            return (memoria.total_fisica / 1048576).min(u32::MAX as u64) as u32;
        }
    }
    #[cfg(target_os = "linux")]
    if let Ok(info) = std::fs::read_to_string("/proc/meminfo") {
        if let Some(kb) = info.lines().find_map(|l| {
            l.strip_prefix("MemTotal:")?
                .split_whitespace()
                .next()?
                .parse::<u64>()
                .ok()
        }) {
            return (kb / 1024).min(u32::MAX as u64) as u32;
        }
    }
    16384
}
fn decimal_mb(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / 1_048_576.).replace('.', ",")
}
fn registro(app: &App, id: &str) -> Option<Instancia> {
    app.servicios
        .instancias
        .registros
        .iter()
        .find(|i| i.id == id)
        .cloned()
}
fn tarjeta(app: &App, id: &str, estado: EstadoInstancia, detalle: &str) {
    if let Some(n) = app
        .servicios
        .instancias
        .registros
        .iter()
        .position(|i| i.id == id)
    {
        if let Some(d) = crate::lista_instancias().lock().unwrap().get_mut(n) {
            d.estado = estado;
            d.detalle = detalle.to_owned();
        }
    }
}
fn listo(app: &App, id: &str) {
    if let Some(i) = registro(app, id) {
        let ready = minecraft::listo(&app.servicios.instancias.almacen, &i);
        tarjeta(
            app,
            id,
            if ready {
                EstadoInstancia::Lista
            } else {
                EstadoInstancia::Pendiente
            },
            if ready {
                "Lista para jugar"
            } else {
                "Minecraft pendiente de instalación"
            },
        );
    }
}
pub(crate) fn config(app: &App, id: &str) -> (lanzamiento::Config, Option<std::path::PathBuf>) {
    let index = app
        .servicios
        .instancias
        .registros
        .iter()
        .position(|i| i.id == id)
        .unwrap_or(0);
    let c = app
        .configs
        .get(index)
        .cloned()
        .unwrap_or_else(crate::config_por_defecto);
    let c = crate::java_real::memoria_efectiva(&c, &app.java_global);
    let resolution = (app.editor.is_some() && app.editor_inst == index)
        .then(|| {
            Some((
                app.general.ancho.parse().ok()?,
                app.general.alto.parse().ok()?,
            ))
        })
        .flatten();
    (
        lanzamiento::Config {
            memoria_min_mb: c.memoria_min,
            memoria_max_mb: c.memoria_max,
            argumentos_jvm: format!("{} -XX:MaxMetaspaceSize={}M", c.argumentos, c.metaspace),
            resolucion: resolution,
            fullscreen: app.editor.is_some()
                && app.editor_inst == index
                && app.general.pantalla_completa,
            servidor: None,
            opciones_compartidas: icaro_core::opciones_minecraft::ruta(&app.servicios.instancias.almacen).ok(),
            cuentas_locales: None,
        },
        crate::java_real::efectiva(&c, &app.java_global).0,
    )
}
pub fn iniciar(app: &mut App, index: usize, jugar: bool) -> Task<Mensaje> {
    iniciar_con_servidor(app,index,jugar,None)
}
pub fn iniciar_con_servidor(app:&mut App,index:usize,jugar:bool,servidor:Option<String>)->Task<Mensaje> {
    if let Some(i)=app.servicios.instancias.registros.get(index) {
        if servidor.is_some()&&app.servicios.juego.ocupada(&i.id) {
            app.avisar("Espera a que termine la actividad de esta instancia antes de conectar al servidor.".into());
            return Task::none();
        }
        match servidor {Some(d)=>{app.servicios.juego.servidores.insert(i.id.clone(),d);},None=>{app.servicios.juego.servidores.remove(&i.id);}}
    }
    if app.actualizacion.descargando || app.actualizacion.aplicando {
        app.avisar(
            "Espera a que termine la actualización de Icaro Launcher.".into(),
        );
        return Task::none();
    }
    let Some(i) = app.servicios.instancias.registros.get(index).cloned() else {
        return Task::none();
    };
    if app
        .configs
        .get(index)
        .is_some_and(|c| c.java_requiere_seleccion && !c.heredar && c.java.is_none())
    {
        app.avisar("Esta instancia exportada usaba un Java manual. Selecciona una instalación o Automático en su pestaña Java.".into());
        return Task::none();
    }
    if !matches!(
        i.loader.as_str(),
        "vanilla" | "fabric" | "forge" | "neoforge"
    ) {
        app.avisar("La instalación de este loader será la siguiente etapa. Puedes seguir gestionando sus mods.".into());
        return Task::none();
    }
    if app.servicios.juego.cerrando || app.servicios.instancias.operando || app.servicios.mods.busy.as_ref() == Some(&i.id) {
        return Task::none();
    }
    if app.servicios.juego.procesos.contains_key(&i.id) {
        return actualizar(app, Accion::Detener(i.id));
    }
    if app.pantalla == Some(crate::pantallas::Pantalla::Fallo) {
        app.pantalla = None;
    }
    if jugar {
        let Some(session) = &app.servicios.session else {
            app.servicios.juego.espera_sesion = Some(i.id);
            app.avisar("Elige una cuenta Microsoft o sin conexión para jugar.".into());
            return crate::servicios::actualizar(app, crate::servicios::Accion::Login);
        };
        if session.needs_refresh() || app.servicios.auth_busy {
            app.servicios.juego.espera_sesion = Some(i.id);
            return crate::servicios::actualizar(app, crate::servicios::Accion::Renovar);
        }
    }
    app.menu = None;
    app.seleccionada = Some(index);
    if app.servicios.juego.ocupada(&i.id) {
        app.servicios.juego.reanudar_pendiente(&i.id);
        if jugar {
            if let Some(t) = app.servicios.juego.trabajos.get_mut(&i.id) {
                t.jugar = true;
            }
            if let Some(s) = app
                .servicios
                .juego
                .pendientes
                .iter_mut()
                .find(|s| s.instancia.id == i.id)
            {
                s.jugar = true;
            }
        }
        return Task::none();
    }
    let (_, java) = config(app, &i.id);
    let descarga_java = app
        .configs
        .get(index)
        .map(|c| crate::java_real::efectiva(c, &app.java_global).1)
        .unwrap_or(app.java_global.descarga);
    let s = &mut app.servicios.juego;
    s.secuencia += 1;
    if !s.orden.contains(&i.id) {
        s.orden.push(i.id.clone());
    }
    s.errores.remove(&i.id);
    s.trabajos.insert(
        i.id.clone(),
        Trabajo {
            nombre: i.nombre.clone(),
            generacion: s.secuencia,
            estado: EstadoDescarga::EnCola(s.pendientes.len() + 1),
            etapa: "Preparando instalación".into(),
            jugar,
            cancel: CancellationToken::new(),
            pausando: false,
            reanudar: false,
            preparando: true,
            partes: None,
            bytes: false,
        },
    );
    s.pendientes.push_back(Solicitud {
        instancia: i,
        jugar,
        java,
        descarga_java,
    });
    siguiente(app)
}
pub fn reanudar_sesion(app: &mut App) -> Task<Mensaje> {
    let Some(id) = app.servicios.juego.espera_sesion.take() else {
        return Task::none();
    };
    if app.servicios.session.is_none() {
        app.avisar("No se pudo renovar la sesión. Vuelve a iniciar sesión para jugar.".into());
        return Task::none();
    }
    let Some(n) = app
        .servicios
        .instancias
        .registros
        .iter()
        .position(|i| i.id == id)
    else {
        return Task::none();
    };
    let servidor=app.servicios.juego.servidores.get(&id).cloned();
    iniciar_con_servidor(app, n, true,servidor)
}
pub fn cancelar_espera_sesion(app: &mut App) {
    if let Some(id)=app.servicios.juego.espera_sesion.take(){app.servicios.juego.servidores.remove(&id);}
}
fn siguiente(app: &mut App) -> Task<Mensaje> {
    if app.servicios.juego.activa.is_some() || app.servicios.juego.cerrando {
        return Task::none();
    }
    let Some(solicitud) = app.servicios.juego.pendientes.pop_front() else {
        return Task::none();
    };
    let id = solicitud.instancia.id.clone();
    let Some(t) = app.servicios.juego.trabajos.get_mut(&id) else {
        return siguiente(app);
    };
    t.estado = EstadoDescarga::Activa(0.);
    t.cancel = CancellationToken::new();
    t.pausando = false;
    t.reanudar = false;
    t.preparando = true;
    let generation = t.generacion;
    let cancel = t.cancel.clone();
    app.servicios.juego.activa = Some(id.clone());
    tarjeta(
        app,
        &id,
        EstadoInstancia::Preparando,
        "Preparando Minecraft",
    );
    let store = app.servicios.instancias.almacen.clone();
    let base = crate::estado::directorio(false);
    Task::run(
        iced::stream::channel(32, async move |mut output| {
            let (tx, mut rx) = tokio::sync::mpsc::channel::<(String, u64, u64)>(32);
            let operation = async {
                icaro_core::modpacks::instalar(&store, &solicitud.instancia, cancel.clone(), |p| {
                    let _ = tx.try_send((p.etapa.to_string(), p.completados, p.total));
                })
                .await?;
                let game = minecraft::instalar_con_java(
                    &store,
                    &solicitud.instancia,
                    solicitud.java.as_deref(),
                    solicitud.descarga_java,
                    cancel.clone(),
                    |p| {
                        let _ = tx.try_send((p.etapa.to_string(), p.completados, p.total));
                    },
                )
                .await
                .map_err(|e| e.to_string())?;
                let runtime = java::preparar_perfil(
                    &base,
                    &game.perfil_java(),
                    solicitud.java.as_deref(),
                    solicitud.descarga_java,
                    cancel.clone(),
                    |p| {
                        let _ = tx.try_send((p.etapa.to_string(), p.completados, p.total));
                    },
                )
                .await
                .map_err(|e| e.to_string())?;
                Ok::<_, String>(Preparacion {
                    minecraft: game,
                    java: runtime,
                })
            };
            tokio::pin!(operation);
            loop {
                tokio::select! {
                    Some((text,done,total))=rx.recv()=>{if output.send(m(Accion::Progreso(id.clone(),generation,text,done,total))).await.is_err(){cancel.cancel();let _=(&mut operation).await;break}},
                    result=&mut operation=>{let _=output.send(m(Accion::Preparada(id.clone(),generation,result))).await;break},
                }
            }
        }),
        |message| message,
    )
}
fn lanzar(app: &mut App, id: String, preparada: Preparacion) -> Task<Mensaje> {
    if app.servicios.juego.cerrando {
        return Task::none();
    }
    if app.pantalla == Some(crate::pantallas::Pantalla::Fallo) {
        app.pantalla = None;
    }
    let Some(session) = app.servicios.session.clone() else {
        app.servicios.juego.espera_sesion = Some(id);
        app.avisar("Inicia sesión para jugar. La instalación ya está lista.".into());
        return crate::servicios::actualizar(app, crate::servicios::Accion::Login);
    };
    if session.needs_refresh() || app.servicios.auth_busy {
        app.servicios.juego.espera_sesion = Some(id);
        return crate::servicios::actualizar(app, crate::servicios::Accion::Renovar);
    }
    let (mut config, _) = config(app, &id);
    config.cuentas_locales=Some(crate::estado::directorio(false));
    config.servidor=app.servicios.juego.servidores.remove(&id);
    let game = match app.servicios.instancias.almacen.juego(&id) {
        Ok(p) => p,
        Err(e) => {
            app.avisar(e.to_string());
            return Task::none();
        }
    };
    let s = &mut app.servicios.juego;
    s.secuencia += 1;
    let generation = s.secuencia;
    let cancel = CancellationToken::new();
    s.ultimo = Some(id.clone());
    s.logs.insert(
        id.clone(),
        vec![
            Linea {
                hora: hora(),
                nivel: Nivel::Info,
                mensaje: format!(
                    "Java {} · {} MB asignados",
                    preparada.java.version,
                    componentes::miles(config.memoria_max_mb)
                ),
            },
            Linea {
                hora: hora(),
                nivel: Nivel::Info,
                mensaje: format!(
                    "Archivos de Minecraft {} verificados. Iniciando el juego.",
                    preparada.minecraft.version
                ),
            },
        ],
    );
    s.errores.remove(&id);
    s.resultados.remove(&id);
    s.umbral_generacion = Some((id.clone(), generation));
    s.retirada_umbral = None;
    s.procesos.insert(
        id.clone(),
        Proceso {
            generacion: generation,
            cancel: cancel.clone(),
            desde: Instant::now(),
            cerrar_hasta: None,
            pid: None,
            ventana_lista: false,
        },
    );
    app.umbral = Some(Instant::now());
    app.ahora = Instant::now();
    tarjeta(app, &id, EstadoInstancia::Jugando, "Preparando Minecraft");
    Task::run(
        iced::stream::channel(64, async move |mut output| {
            let (tx, mut rx) = tokio::sync::mpsc::channel(256);
            let omitidas = AtomicU64::new(0);
            let fin = Mutex::new(None);
            let operation = lanzamiento::ejecutar(
                &preparada.minecraft,
                &preparada.java.executable,
                &game,
                &session,
                config,
                cancel.clone(),
                |event| {
                    if let lanzamiento::Evento::Fin(salida) = event {
                        *fin.lock().unwrap() = Some(salida);
                    } else if tx.try_send(event).is_err() {
                        omitidas.fetch_add(1, Ordering::Relaxed);
                    }
                },
            );
            tokio::pin!(operation);
            loop {
                tokio::select! {
                    Some(event)=rx.recv()=>{
                        if output.send(m(Accion::Evento(id.clone(),generation,event))).await.is_err(){cancel.cancel();let _=(&mut operation).await;break}
                        let cantidad=omitidas.swap(0,Ordering::Relaxed);
                        if cantidad>0{let _=output.send(m(Accion::Evento(id.clone(),generation,aviso_omitidas(cantidad)))).await;}
                    },
                    result=&mut operation=>{
                        while let Ok(event)=rx.try_recv(){let _=output.send(m(Accion::Evento(id.clone(),generation,event))).await;}
                        let cantidad=omitidas.swap(0,Ordering::Relaxed);
                        if cantidad>0{let _=output.send(m(Accion::Evento(id.clone(),generation,aviso_omitidas(cantidad)))).await;}
                        let ultimo=fin.lock().unwrap().take();
                        if let Some(salida)=ultimo{let _=output.send(m(Accion::Evento(id.clone(),generation,lanzamiento::Evento::Fin(salida)))).await;}
                        let _=output.send(m(Accion::Terminada(id.clone(),generation,result.map_err(|e|e.to_string())))).await;break
                    },
                }
            }
        }),
        |message| message,
    )
}
fn aviso_omitidas(cantidad: u64) -> lanzamiento::Evento {
    lanzamiento::Evento::Linea {
        origen: lanzamiento::Origen::Stdout,
        nivel: lanzamiento::Nivel::Advertencia,
        texto: format!(
            "Se omitieron {cantidad} líneas porque el juego produjo registros muy rápido."
        ),
    }
}

/// La entrada tiene su propio reloj; la retirada empieza únicamente al
/// confirmar la ventana del juego, al volver o cuando termina el proceso.
pub fn retirada_umbral(app: &App) -> Option<(f32, bool)> {
    app.servicios.juego.retirada_umbral.as_ref().map(|r| {
        (
            app.ahora.saturating_duration_since(r.desde).as_secs_f32() * 1000.,
            r.fallo,
        )
    })
}

pub fn animando_umbral(app: &App) -> bool {
    app.umbral.is_some_and(|inicio| {
        app.servicios.juego.retirada_umbral.is_some()
            || app.ahora.saturating_duration_since(inicio)
                < Duration::from_millis(if app.reducir { 120 } else { 1700 })
    })
}

pub fn volver_umbral(app: &mut App) -> Task<Mensaje> {
    let Some((id, g)) = app.servicios.juego.umbral_generacion.clone() else {
        // Umbral de demostración, sin ningún proceso ni cuenta.
        app.umbral = None;
        return Task::none();
    };
    empezar_retirada(app, &id, g, false, false)
}

fn empezar_retirada(app: &mut App, id: &str, g: u64, fallo: bool, listo: bool) -> Task<Mensaje> {
    let s = &mut app.servicios.juego;
    if app.umbral.is_none()
        || !s
            .umbral_generacion
            .as_ref()
            .is_some_and(|(owner, gen)| owner == id && *gen == g)
    {
        return Task::none();
    }
    if s.retirada_umbral.is_some() && !fallo {
        return Task::none();
    }
    let ahora = Instant::now();
    let entrada_minima = Duration::from_millis(if app.reducir { 120 } else { 1400 });
    let espera = if listo {
        entrada_minima.saturating_sub(ahora.saturating_duration_since(app.umbral.unwrap()))
    } else {
        Duration::ZERO
    };
    let desde = ahora + espera;
    s.retirada_umbral = Some(RetiradaUmbral {
        id: id.into(),
        generacion: g,
        desde,
        fallo,
        minimizar: listo && app.aj.al_jugar == 1,
        consola: listo && app.aj.consola_al_jugar,
    });
    app.ahora = ahora;
    let id = id.to_owned();
    let duracion = icaro_ui::movimiento::Preferencia {
        reducir: app.reducir,
    }
    .duracion(200);
    Task::perform(
        async move {
            tokio::time::sleep(espera + duracion).await;
            (id, g, desde)
        },
        |(id, g, desde)| m(Accion::UmbralRetirado(id, g, desde)),
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SondaVentana {
    Esperando,
    Lista,
    Finalizada,
}

async fn esperar_ventana(
    cancel: CancellationToken,
    intervalo: Duration,
    limite: Duration,
    mut sonda: impl FnMut() -> SondaVentana,
) -> ResultadoVentana {
    let hasta = tokio::time::Instant::now() + limite;
    let mut visibles = 0;
    loop {
        if cancel.is_cancelled() {
            return ResultadoVentana::Cancelada;
        }
        match sonda() {
            SondaVentana::Lista => {
                visibles += 1;
                // Evita confirmar una ventana que aparece y desaparece en
                // el mismo instante de su creación.
                if visibles >= 2 {
                    return ResultadoVentana::Lista;
                }
            }
            SondaVentana::Esperando => visibles = 0,
            SondaVentana::Finalizada => return ResultadoVentana::Finalizada,
        }
        if tokio::time::Instant::now() >= hasta {
            return ResultadoVentana::SinConfirmar;
        }
        tokio::select! {
            _ = cancel.cancelled() => return ResultadoVentana::Cancelada,
            _ = tokio::time::sleep_until((tokio::time::Instant::now() + intervalo).min(hasta)) => {}
        }
    }
}

async fn esperar_ventana_pid(pid: u32, cancel: CancellationToken) -> ResultadoVentana {
    #[cfg(windows)]
    let mut sonda = VentanaWindows::nueva(pid);
    esperar_ventana(
        cancel,
        Duration::from_millis(150),
        Duration::from_secs(120),
        move || {
            #[cfg(windows)]
            {
                sonda.estado()
            }
            #[cfg(not(windows))]
            {
                let _ = pid;
                SondaVentana::Esperando
            }
        },
    )
    .await
}

#[cfg(windows)]
struct VentanaWindows {
    pid: u32,
    // Guardado como entero para poder conservar el handle mientras el
    // futuro duerme. Lo posee esta sonda y Drop lo cierra exactamente una vez.
    proceso: usize,
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn OpenProcess(acceso: u32, heredar: i32, pid: u32) -> *mut std::ffi::c_void;
    fn WaitForSingleObject(handle: *mut std::ffi::c_void, ms: u32) -> u32;
}

#[cfg(windows)]
impl VentanaWindows {
    fn nueva(pid: u32) -> Self {
        Self {
            pid,
            proceso: unsafe { OpenProcess(0x0010_0000, 0, pid) } as usize,
        }
    }

    fn estado(&mut self) -> SondaVentana {
        use windows_sys::Win32::{
            Foundation::{HWND, LPARAM, RECT},
            UI::WindowsAndMessaging::{
                EnumWindows, GetClassNameW, GetClientRect, GetWindow, GetWindowThreadProcessId,
                IsWindowVisible, GW_OWNER,
            },
        };
        if self.proceso == 0 {
            // No poder observar un proceso nunca se interpreta como listo.
            return SondaVentana::Esperando;
        }
        let vivo = || unsafe { WaitForSingleObject(self.proceso as *mut _, 0) == 258 };
        if !vivo() {
            return SondaVentana::Finalizada;
        }
        struct Busqueda {
            pid: u32,
            lista: bool,
        }
        unsafe extern "system" fn visitar(hwnd: HWND, datos: LPARAM) -> i32 {
            let busqueda = &mut *(datos as *mut Busqueda);
            let mut pid = 0;
            GetWindowThreadProcessId(hwnd, &mut pid);
            if pid != busqueda.pid
                || IsWindowVisible(hwnd) == 0
                || !GetWindow(hwnd, GW_OWNER).is_null()
            {
                return 1;
            }
            let mut area = RECT::default();
            if GetClientRect(hwnd, &mut area) == 0
                || area.right - area.left < 64
                || area.bottom - area.top < 64
            {
                return 1;
            }
            let mut clase = [0u16; 128];
            let n = GetClassNameW(hwnd, clase.as_mut_ptr(), clase.len() as i32);
            if n <= 0 {
                return 1;
            }
            let clase = String::from_utf16_lossy(&clase[..n as usize]);
            // Ambas familias son ventanas reales del cliente, desde LWJGL
            // legacy hasta SDL; excluye diálogos Java y ventanas auxiliares.
            if clase.starts_with("GLFW") || clase.starts_with("SDL") || clase.starts_with("LWJGL") {
                busqueda.lista = true;
                return 0;
            }
            1
        }
        let mut busqueda = Busqueda {
            pid: self.pid,
            lista: false,
        };
        unsafe {
            EnumWindows(Some(visitar), &mut busqueda as *mut _ as LPARAM);
        }
        if !vivo() {
            SondaVentana::Finalizada
        } else if busqueda.lista {
            SondaVentana::Lista
        } else {
            SondaVentana::Esperando
        }
    }
}

#[cfg(windows)]
impl Drop for VentanaWindows {
    fn drop(&mut self) {
        if self.proceso != 0 {
            unsafe {
                windows_sys::Win32::Foundation::CloseHandle(self.proceso as *mut _);
            }
        }
    }
}

pub fn cerrar(app: &mut App) -> Task<Mensaje> {
    app.servicios.juego.cerrando = true;
    app.servicios.juego.cancelar_todo();
    cerrar_si_listo(app)
}
fn cerrar_si_listo(app: &App) -> Task<Mensaje> {
    if app.servicios.juego.cerrando && app.servicios.juego.libre() {
        iced::window::get_oldest().and_then(iced::window::close)
    } else {
        Task::none()
    }
}
pub fn actualizar(app: &mut App, a: Accion) -> Task<Mensaje> {
    match a {
        Accion::Progreso(id, g, text, done, total) => {
            if let Some((ratio, detail)) = app.servicios.juego.progreso(&id, g, text, done, total) {
                let preparando = app
                    .servicios
                    .juego
                    .trabajos
                    .get(&id)
                    .is_some_and(|t| t.preparando);
                tarjeta(
                    app,
                    &id,
                    if preparando {
                        EstadoInstancia::Preparando
                    } else {
                        EstadoInstancia::Instalando(ratio)
                    },
                    &detail,
                );
            }
        }
        Accion::Preparada(id, g, result) => {
            if !app
                .servicios
                .juego
                .trabajos
                .get(&id)
                .is_some_and(|t| t.generacion == g)
            {
                return Task::none();
            }
            app.servicios.juego.activa = None;
            let t = app.servicios.juego.trabajos.get_mut(&id).unwrap();
            let jugar = t.jugar;
            let canceled = t.cancel.is_cancelled();
            let pausando = t.pausando;
            let reanudar = t.reanudar && !app.servicios.juego.cerrando;
            t.reanudar = false;
            let avance = match t.estado {
                EstadoDescarga::Activa(p) | EstadoDescarga::Pausada(p) => p,
                _ => 0.,
            };
            let launch = match result {
                Ok(p) if !canceled => {
                    t.estado = EstadoDescarga::Completada;
                    t.etapa = "Minecraft y Java preparados".into();
                    app.servicios.juego.java.insert(id.clone(), p.java.clone());
                    app.servicios
                        .juego
                        .preparada
                        .insert(id.clone(), p.minecraft.clone());
                    listo(app, &id);
                    crate::mods_reales::recargar(app,&id);
                    if jugar {
                        lanzar(app, id.clone(), p)
                    } else {
                        Task::none()
                    }
                }
                _ if canceled => {
                    t.estado = if pausando {
                        EstadoDescarga::Pausada(avance)
                    } else {
                        EstadoDescarga::Fallo(
                            "Instalación cancelada; los archivos verificados se conservan.".into(),
                        )
                    };
                    listo(app, &id);
                    if reanudar {
                        if let Some(index) = app
                            .servicios
                            .instancias
                            .registros
                            .iter()
                            .position(|i| i.id == id)
                        {
                            iniciar(app, index, jugar)
                        } else {
                            Task::none()
                        }
                    } else {
                        Task::none()
                    }
                }
                Err(error) => {
                    t.estado = EstadoDescarga::Fallo(error.clone());
                    app.servicios
                        .juego
                        .errores
                        .insert(id.clone(), error.clone());
                    tarjeta(app, &id, EstadoInstancia::Error, &error);
                    app.avisar(error);
                    Task::none()
                }
                _ => Task::none(),
            };
            return Task::batch([launch, siguiente(app), cerrar_si_listo(app)]);
        }
        Accion::Evento(id, g, event) => {
            if !app
                .servicios
                .juego
                .procesos
                .get(&id)
                .is_some_and(|p| p.generacion == g)
            {
                return Task::none();
            }
            match event {
                lanzamiento::Evento::Inicio { pid } => {
                    let p = app.servicios.juego.procesos.get_mut(&id).unwrap();
                    if p.pid.is_some() || p.cancel.is_cancelled() {
                        return Task::none();
                    }
                    p.pid = Some(pid);
                    let cancel = p.cancel.clone();
                    // El reloj pertenece a Jugar. Crear Java no vuelve a abrir
                    // el Umbral ni significa que Minecraft tenga una ventana.
                    return Task::perform(
                        async move {
                            let resultado = esperar_ventana_pid(pid, cancel).await;
                            (id, g, pid, resultado)
                        },
                        |(id, g, pid, resultado)| m(Accion::Ventana(id, g, pid, resultado)),
                    );
                }
                lanzamiento::Evento::Linea {
                    origen: _,
                    nivel,
                    texto,
                } => {
                    let logs = app.servicios.juego.logs.entry(id).or_default();
                    if logs.len() >= 2000 {
                        logs.drain(..500);
                    }
                    logs.push(Linea {
                        hora: hora(),
                        nivel: match nivel {
                            lanzamiento::Nivel::Error => Nivel::Error,
                            lanzamiento::Nivel::Advertencia => Nivel::Warn,
                            _ => Nivel::Info,
                        },
                        mensaje: if app.servicios.session.as_ref().is_some_and(|s|s.es_local()) && (texto.contains("Failed to verify username") || texto.contains("Invalid session") || texto.contains("multiplayer.disconnect.unverified_username")) {
                            format!("Este servidor pide una cuenta Microsoft. {texto}")
                        } else {texto},
                    });
                    return seguir_consola(app);
                }
                lanzamiento::Evento::Fin(_) => {}
            }
        }
        Accion::Terminada(id, g, result) => {
            if !app
                .servicios
                .juego
                .procesos
                .get(&id)
                .is_some_and(|p| p.generacion == g)
            {
                return Task::none();
            }
            if let Some(p) = app.servicios.juego.procesos.remove(&id) {
                // También corta el observador de ventana si Java terminó
                // normalmente: no depende de que Salida sea cancelada.
                p.cancel.cancel();
            }
            app.servicios
                .juego
                .resultados
                .insert(id.clone(), (result.as_ref().ok().copied(), hora()));
            listo(app, &id);
            let error = match result {
                Ok(s) if s.cancelado || s.exito => None,
                Ok(s) => Some(format!(
                    "Minecraft se cerró con código {}. Abre el registro para ver qué pasó.",
                    s.codigo.map_or("desconocido".into(), |c| c.to_string())
                )),
                Err(e) => Some(e),
            };
            let fallo = error.is_some();
            if let Some(e) = error {
                app.servicios.juego.errores.insert(id.clone(), e.clone());
                app.servicios.juego.ultimo = Some(id.clone());
                app.pantalla = Some(crate::pantallas::Pantalla::Fallo);
                app.avisar(e);
            }
            return Task::batch([
                empezar_retirada(app, &id, g, fallo, false),
                cerrar_si_listo(app),
            ]);
        }
        Accion::Pausar(n) | Accion::Cancelar(n) => {
            let pausando = matches!(a, Accion::Pausar(_));
            let Some(id) = app.servicios.juego.orden.get(n).cloned() else {
                return Task::none();
            };
            if app.servicios.juego.suspender(&id, pausando) {
                return actualizar(app, Accion::Reintentar(n));
            }
            if app.servicios.juego.activa.as_deref() != Some(&id) {
                listo(app, &id);
            }
        }
        Accion::QuitarRegistro(n) => app.servicios.juego.quitar_registro(n),
        Accion::Reintentar(n) => {
            let Some(id) = app.servicios.juego.orden.get(n).cloned() else {
                return Task::none();
            };
            let jugar = app
                .servicios
                .juego
                .trabajos
                .get(&id)
                .is_some_and(|t| t.jugar);
            if app.servicios.juego.reanudar_pendiente(&id) {
                return Task::none();
            }
            let Some(index) = app
                .servicios
                .instancias
                .registros
                .iter()
                .position(|i| i.id == id)
            else {
                return Task::none();
            };
            return iniciar(app, index, jugar);
        }
        Accion::Detener(id) => {
            if let Some(p) = app.servicios.juego.procesos.get_mut(&id) {
                if p.cerrar_hasta.is_some_and(|until| until > Instant::now()) {
                    p.cancel.cancel();
                    p.cerrar_hasta = None;
                } else {
                    p.cerrar_hasta = Some(Instant::now() + std::time::Duration::from_secs(3));
                }
            }
        }
        Accion::Ventana(id, g, pid, ResultadoVentana::Lista) => {
            if app.servicios.juego.ventana_lista(&id, g, pid) {
                tarjeta(app, &id, EstadoInstancia::Jugando, "En ejecución");
                return empezar_retirada(app, &id, g, false, true);
            }
        }
        Accion::Ventana(..) => {
            // Un límite del observador no es una ventana lista. El registro
            // permanece y Volver sigue disponible mientras el juego arranca.
        }
        Accion::UmbralRetirado(id, g, desde) => {
            let s = &mut app.servicios.juego;
            if s.retirada_umbral
                .as_ref()
                .is_some_and(|r| r.id == id && r.generacion == g && r.desde == desde)
                && s.umbral_generacion
                    .as_ref()
                    .is_some_and(|(owner, gen)| owner == &id && *gen == g)
            {
                let retirada = s.retirada_umbral.take().unwrap();
                s.umbral_generacion = None;
                app.umbral = None;
                let vivo = s.procesos.get(&id).is_some_and(|p| {
                    p.generacion == g && p.ventana_lista && !p.cancel.is_cancelled()
                });
                if vivo && retirada.consola {
                    app.editor = None;
                    app.seccion = icaro_ui::shell::Seccion::Consola;
                }
                if vivo && retirada.minimizar {
                    return iced::window::get_oldest()
                        .and_then(|id| iced::window::minimize(id, true));
                }
            }
        }
        Accion::Copiar | Accion::CopiarInforme => {
            let text = texto_registro(app);
            return iced::clipboard::write(text);
        }
        Accion::Limpiar => {
            if let Some(id) = id_consola(app) {
                app.servicios.juego.logs.insert(id, vec![]);
            }
            app.desplazamiento_consola = Default::default();
        }
        Accion::SeleccionarConsola(id) => {
            app.servicios.juego.consola_seleccionada = Some(id);
            app.servicios.juego.mostrar_ultimo_registro = false;
            app.desplazamiento_consola = Default::default();
            app.autoscroll_consola = true;
            return cargar_consola(app);
        }
        Accion::RegistroCargado(id, lineas) => {
            if app.servicios.juego.cargando_registro.as_deref() == Some(&id) {
                app.servicios.juego.cargando_registro = None;
            }
            // El resultado del disco no reemplaza una nueva sesión en vivo.
            if !app.servicios.juego.procesos.contains_key(&id) {
                app.servicios.juego.logs.entry(id).or_insert(lineas);
            }
            return seguir_consola(app);
        }
        Accion::UltimoRegistro => {
            app.servicios.juego.mostrar_ultimo_registro = true;
            return Task::batch([cargar_consola(app), seguir_consola(app)]);
        }
        Accion::AbrirRegistros => {
            if let Some(id) = id_consola(app) {
                if let Ok(game) = app.servicios.instancias.almacen.juego(&id) {
                    let logs = game.join("logs");
                    let destino = if logs.is_dir() { logs } else { game };
                    let programa = if cfg!(windows) {
                        "explorer"
                    } else {
                        "xdg-open"
                    };
                    if std::process::Command::new(programa)
                        .arg(destino)
                        .spawn()
                        .is_err()
                    {
                        app.avisar("No se pudo abrir la carpeta de registros.".into());
                    }
                }
            }
        }
        Accion::ExportarRegistro => return exportar_registro(app),
        Accion::RegistroExportado(result) => match result {
            Ok(path) => app.avisar(format!("Registro guardado en {}", path.display())),
            Err(error) => app.avisar(error),
        },
    }
    Task::none()
}
fn id_consola(app: &App) -> Option<String> {
    if app.umbral.is_some() {
        if let Some((id, _)) = &app.servicios.juego.umbral_generacion {
            return Some(id.clone());
        }
    }
    if app.editor.is_some() {
        app.servicios
            .instancias
            .registros
            .get(app.editor_inst)
            .map(|i| i.id.clone())
    } else {
        app.servicios
            .juego
            .consola_seleccionada
            .clone()
            .or_else(|| app.servicios.juego.procesos.keys().next().cloned())
            .or_else(|| app.servicios.juego.ultimo.clone())
            .or_else(|| {
                app.servicios
                    .instancias
                    .registros
                    .first()
                    .map(|i| i.id.clone())
            })
    }
}

pub fn seguir_consola(app: &App) -> Task<Mensaje> {
    if app.autoscroll_consola
        && app.umbral.is_none()
        && (app.editor == Some(6)
            || (app.editor.is_none() && app.seccion == icaro_ui::shell::Seccion::Consola))
    {
        iced::widget::scrollable::snap_to(
            iced::widget::scrollable::Id::new(consola::LISTA_CONSOLA),
            iced::widget::scrollable::RelativeOffset::END,
        )
    } else {
        Task::none()
    }
}

pub fn cargar_consola(app: &mut App) -> Task<Mensaje> {
    let Some(id) = id_consola(app) else {
        return Task::none();
    };
    if app.servicios.juego.logs.contains_key(&id)
        || app.servicios.juego.procesos.contains_key(&id)
        || app.servicios.juego.cargando_registro.as_deref() == Some(&id)
    {
        return seguir_consola(app);
    }
    let Ok(game) = app.servicios.instancias.almacen.juego(&id) else {
        return Task::none();
    };
    app.servicios.juego.cargando_registro = Some(id.clone());
    let session = app.servicios.session.clone();
    Task::perform(
        async move {
            let lineas = tokio::task::spawn_blocking(move || {
                leer_registro_disco(&game)
                    .map(|text| {
                        let text = lanzamiento::registro_para_exportar(&text, session.as_deref());
                        lineas_guardadas(&text)
                    })
                    .unwrap_or_default()
            })
            .await
            .unwrap_or_default();
            (id, lineas)
        },
        |(id, lineas)| m(Accion::RegistroCargado(id, lineas)),
    )
}

fn leer_registro_disco(game: &std::path::Path) -> Option<String> {
    let root = game.canonicalize().ok()?;
    let path = root.join("logs/latest.log");
    if !path.canonicalize().ok()?.starts_with(&root) {
        return None;
    }
    let meta = std::fs::metadata(&path).ok()?;
    if !meta.is_file() || meta.len() > 32 * 1024 * 1024 {
        return None;
    }
    std::fs::read_to_string(path).ok()
}

fn lineas_guardadas(text: &str) -> Vec<Linea> {
    let mut lines: Vec<_> = text
        .lines()
        .rev()
        .take(2000)
        .map(|line| {
            let hora = line
                .get(1..9)
                .filter(|h| {
                    h.as_bytes().get(2) == Some(&b':') && h.as_bytes().get(5) == Some(&b':')
                })
                .unwrap_or("—")
                .to_owned();
            let nivel = if line.contains("/ERROR]") || line.contains("/FATAL]") {
                Nivel::Error
            } else if line.contains("/WARN]") {
                Nivel::Warn
            } else {
                Nivel::Info
            };
            let mensaje = if hora != "—" {
                line.get(11..).unwrap_or(line)
            } else {
                line
            };
            Linea {
                hora,
                nivel,
                mensaje: mensaje.to_owned(),
            }
        })
        .collect();
    lines.reverse();
    lines
}

fn exportar_registro(app: &App) -> Task<Mensaje> {
    let id = id_consola(app);
    let game = id
        .as_ref()
        .and_then(|id| app.servicios.instancias.almacen.juego(id).ok());
    let session = app.servicios.session.clone();
    let fallback = texto_registro(app);
    let error = id
        .as_ref()
        .and_then(|id| app.servicios.juego.errores.get(id))
        .cloned();
    Task::perform(
        async move {
            let Some(file) = rfd::AsyncFileDialog::new()
                .set_title("Exportar registros")
                .set_file_name("icaro-registro.txt")
                .add_filter("Registro de texto", &["txt"])
                .save_file()
                .await
            else {
                return None;
            };
            Some(
                tokio::task::spawn_blocking(move || {
                    let content = game
                        .as_deref()
                        .and_then(leer_registro_disco)
                        .map(|text| {
                            format!(
                                "Icaro Launcher · Minecraft\n{}\n{}",
                                error.unwrap_or_default(),
                                lanzamiento::registro_para_exportar(&text, session.as_deref())
                            )
                        })
                        .unwrap_or(fallback);
                    std::fs::write(file.path(), content)
                        .map_err(|_| "No se pudo guardar el registro.".to_owned())?;
                    Ok(file.path().to_owned())
                })
                .await
                .unwrap_or_else(|_| Err("No se pudo guardar el registro.".into())),
            )
        },
        |result| result.map_or(Mensaje::Nada, |r| m(Accion::RegistroExportado(r))),
    )
}
pub fn lineas(app: &App) -> &[Linea] {
    id_consola(app)
        .as_ref()
        .and_then(|id| app.servicios.juego.logs.get(id))
        .map_or(&[], Vec::as_slice)
}
fn texto_registro(app: &App) -> String {
    let id = id_consola(app);
    let error = id
        .as_ref()
        .and_then(|id| app.servicios.juego.errores.get(id))
        .cloned()
        .unwrap_or_default();
    format!(
        "Icaro Launcher · Minecraft Vanilla\n{error}\n{}",
        lineas(app)
            .iter()
            .map(|l| format!("[{}] {:?} {}", l.hora, l.nivel, l.mensaje))
            .collect::<Vec<_>>()
            .join("\n")
    )
}
pub fn estado_boton(app: &App, index: usize) -> (EstadoJugar, String, Option<Mensaje>) {
    let Some(i) = app.servicios.instancias.registros.get(index) else {
        return (EstadoJugar::Listo, String::new(), None);
    };
    if let Some(p) = app.servicios.juego.procesos.get(&i.id) {
        if !p.ventana_lista && p.cerrar_hasta.is_none() {
            return (
                EstadoJugar::Preparando,
                String::new(),
                Some(m(Accion::Detener(i.id.clone()))),
            );
        }
        let confirm = p.cerrar_hasta.is_some_and(|t| t > Instant::now());
        let secs = p.desde.elapsed().as_secs();
        return (
            if confirm {
                EstadoJugar::Cerrar
            } else {
                EstadoJugar::Ejecutando
            },
            if confirm {
                String::new()
            } else {
                format!(
                    "{:02}:{:02}:{:02}",
                    secs / 3600,
                    (secs / 60) % 60,
                    secs % 60
                )
            },
            Some(m(Accion::Detener(i.id.clone()))),
        );
    }
    if let Some(t) = app.servicios.juego.trabajos.get(&i.id) {
        match t.estado {
            EstadoDescarga::Activa(_) if t.preparando => {
                return (
                    EstadoJugar::Preparando,
                    if t.etapa.contains("Java") { "Java" } else { "" }.into(),
                    None,
                )
            }
            EstadoDescarga::Activa(p) => {
                return (
                    EstadoJugar::Instalando(p),
                    format!("{:.0} %", p * 100.),
                    None,
                )
            }
            EstadoDescarga::EnCola(_) => return (EstadoJugar::Preparando, "En cola".into(), None),
            _ => {}
        }
    }
    let listo = crate::instancias().get(index).is_some_and(|d| {
        matches!(
            d.estado,
            EstadoInstancia::Lista | EstadoInstancia::Desactualizada
        )
    });
    (
        if listo {
            EstadoJugar::Listo
        } else {
            EstadoJugar::PorInstalar
        },
        String::new(),
        Some(Mensaje::Jugar(index)),
    )
}
pub fn boton(app: &App, index: usize) -> Element<'_, Mensaje> {
    let (state, meta, message) = estado_boton(app, index);
    let ms = if app.reducir {
        0
    } else {
        app.ahora
            .saturating_duration_since(app.codigo_inicio)
            .as_millis()
    };
    componentes::boton_jugar_estado_animado(app.paleta(), state, Some(&meta), message, ms as f32)
}
pub fn indicador(app: &App) -> Option<icaro_ui::shell::DescargasActivas> {
    app.servicios.juego.indicador_descarga()
}
pub fn descargas(app: &App) -> Element<'_, Mensaje> {
    let p = app.paleta();
    let entries: Vec<_> = app
        .servicios
        .juego
        .orden
        .iter()
        .filter_map(|id| app.servicios.juego.trabajos.get(id))
        .map(|t| Descarga {
            nombre: t.nombre.clone(),
            destino: app.servicios.instancias.registros.iter()
                .find(|i|i.nombre==t.nombre)
                .map(|i|format!("Minecraft {}",crate::ficha::loader(&i.loader)))
                .unwrap_or_else(||"Minecraft".into()),
            estado: t.estado.clone(),
            detalle: t.etapa.clone(),
        })
        .collect();
    let indeterminadas: Vec<usize> = app
        .servicios
        .juego
        .orden
        .iter()
        .filter_map(|id| app.servicios.juego.trabajos.get(id))
        .enumerate()
        .filter_map(|(i, t)| t.partes.is_none().then_some(i))
        .collect();
    let contables: Vec<(usize, usize, usize)> = app
        .servicios
        .juego
        .orden
        .iter()
        .filter_map(|id| app.servicios.juego.trabajos.get(id))
        .enumerate()
        .filter_map(|(i, t)| {
            if t.bytes {
                None
            } else {
                t.partes.map(|(hechos, total)| {
                    (
                        i,
                        total.min(usize::MAX as u64) as usize,
                        hechos.min(usize::MAX as u64) as usize,
                    )
                })
            }
        })
        .collect();
    let resumen = format!(
        "{} activa · {} en cola",
        usize::from(app.servicios.juego.activa.is_some()),
        app.servicios.juego.pendientes.len()
    );
    let cuerpo = column![descargas::cola_descargas_animada(
        p,
        &entries,
        &descargas::AccionesCola {
            pausar: |n| m(Accion::Pausar(n)),
            cancelar: |n| m(Accion::Cancelar(n)),
            reintentar: |n| m(Accion::Reintentar(n)),
            quitar: |n| m(Accion::QuitarRegistro(n)),
            seguir: |n| m(Accion::Reintentar(n))
        },
        app.reducir,
        &indeterminadas,
        &contables
    )]
    .spacing(espacio::S8)
    .padding(Padding::from([espacio::S8, espacio::S12]));
    column![
        banda_completa(
            p,
            Lamina::MercurioFriso,
            None,
            "Descargas",
            DatoBanda::Nota(&resumen),
            ALTO_BANDA,
            texto::DISPLAY_XL
        ),
        crate::suave(app, cuerpo)
    ]
    .into()
}
#[derive(Clone, PartialEq, Eq)]
struct OpcionConsola {
    id: String,
    nombre: String,
    ejecutando: bool,
}
impl std::fmt::Display for OpcionConsola {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{}",
            if self.ejecutando { "● " } else { "" },
            self.nombre
        )
    }
}

pub fn consola(app: &App) -> Element<'_, Mensaje> {
    use icaro_ui::{
        botones::{boton, TamBoton},
        componentes::Variante,
    };
    let p = app.paleta();
    let id = id_consola(app);
    let instancia = id.as_ref().and_then(|id| registro(app, id));
    let nombre = instancia
        .as_ref()
        .map_or("Minecraft", |i| i.nombre.as_str());
    let content = consola::consola_viva(
        p,
        lineas(app),
        consola::VistaConsola {
            nombre,
            niveles: app.niveles_consola,
            busqueda: &app.busqueda_consola,
            autoscroll: app.autoscroll_consola,
            desplazamiento: app.desplazamiento_consola,
        },
        consola::MensajesConsolaViva {
            nivel: Mensaje::NivelConsola,
            buscar: Mensaje::BuscarConsola,
            autoscroll: Mensaje::AutoscrollConsola,
            desplazado: Mensaje::DesplazadoConsola,
            ir_al_final: Mensaje::FinalConsola,
            copiar: m(Accion::Copiar),
            limpiar: m(Accion::Limpiar),
        },
    );
    if app.editor.is_some() {
        return container(content)
            .padding(Padding::from([espacio::S6, espacio::S12]))
            .into();
    }
    let mut opciones: Vec<_> = app
        .servicios
        .instancias
        .registros
        .iter()
        .map(|i| OpcionConsola {
            id: i.id.clone(),
            nombre: i.nombre.clone(),
            ejecutando: app.servicios.juego.procesos.contains_key(&i.id),
        })
        .collect();
    opciones.sort_by_key(|i| !i.ejecutando);
    let elegida = opciones
        .iter()
        .find(|i| Some(&i.id) == id.as_ref())
        .cloned();
    let herramientas = row![
        container(componentes::selector(p, opciones, elegida, |i| m(
            Accion::SeleccionarConsola(i.id)
        )))
        .width(280),
        Space::with_width(Length::Fill),
        boton(
            p,
            Some(Icono::Descargas),
            "Exportar registros",
            Variante::Secundario,
            TamBoton::Lg,
            id.as_ref().map(|_| m(Accion::ExportarRegistro))
        )
    ]
    .spacing(espacio::S4)
    .align_y(Alignment::Center);
    let en_ejecucion = id
        .as_ref()
        .is_some_and(|id| app.servicios.juego.procesos.contains_key(id));
    let content: Element<Mensaje> = if en_ejecucion || app.servicios.juego.mostrar_ultimo_registro {
        content
    } else {
        let ultima = id.as_ref().and_then(|id| app.servicios.juego.resultados.get(id))
            .map_or_else(|| "El registro aparece aquí en vivo mientras juegas. Puedes abrir el último registro guardado.".to_owned(),
                |(salida, hora)| format!("El registro aparece aquí en vivo mientras juegas. La última sesión de {nombre} terminó a las {hora}{}.",
                    if salida.is_some_and(|s| !s.exito && !s.cancelado) { " con errores" } else { "" }));
        let indice = id.as_ref().and_then(|id| {
            app.servicios
                .instancias
                .registros
                .iter()
                .position(|i| &i.id == id)
        });
        container(
            row![
                iced::widget::image(Lamina::JeronimoSanto.imagen(p.modo))
                    .content_fit(iced::ContentFit::Contain)
                    .width(240)
                    .height(160),
                column![
                    text("EL JUEGO NO\nESTÁ ABIERTO")
                        .font(fuentes::DISPLAY)
                        .size(28)
                        .color(p.text),
                    text(ultima).size(texto::BODY.0).color(p.text_muted),
                    row![
                        boton(
                            p,
                            Some(Icono::Jugar),
                            &format!("Jugar {nombre}"),
                            Variante::Primario,
                            TamBoton::Lg,
                            indice.map(Mensaje::Jugar)
                        ),
                        boton(
                            p,
                            Some(Icono::Consola),
                            "Abrir último registro",
                            Variante::Secundario,
                            TamBoton::Lg,
                            id.as_ref().map(|_| m(Accion::UltimoRegistro))
                        ),
                        boton(
                            p,
                            Some(Icono::Carpeta),
                            "Abrir carpeta de registros",
                            Variante::Fantasma,
                            TamBoton::Lg,
                            id.as_ref().map(|_| m(Accion::AbrirRegistros))
                        ),
                    ]
                    .spacing(espacio::S2)
                    .wrap()
                ]
                .spacing(espacio::S4)
                .width(Length::Fill),
            ]
            .spacing(40)
            .align_y(Alignment::Center),
        )
        .padding(espacio::S8)
        .style(move |_: &iced::Theme| container::Style {
            background: Some(iced::Background::Color(p.surface)),
            border: iced::Border {
                color: p.border,
                width: 1.,
                radius: 0.0.into(),
            },
            ..Default::default()
        })
        .into()
    };
    column![
        container(
            text("CONSOLA")
                .font(fuentes::DISPLAY)
                .size(texto::DISPLAY_XL.0)
                .color(p.text)
        )
        .padding(Padding::from([espacio::S6, espacio::S12])),
        crate::con_entrada(
            app,
            container(
                column![herramientas, content]
                    .spacing(espacio::S6)
                    .height(Length::Fill)
            )
            .padding(Padding::from([0., espacio::S12]))
            .height(Length::Fill)
            .into()
        ),
        Space::with_height(espacio::S6)
    ]
    .into()
}
pub fn fallo(app: &App) -> Element<'_, Mensaje> {
    let p = app.paleta();
    let error = app
        .servicios
        .juego
        .ultimo
        .as_ref()
        .and_then(|id| app.servicios.juego.errores.get(id))
        .cloned()
        .unwrap_or_else(|| {
            "No sabemos qué pasó. Abre el registro o copia el informe para pedir ayuda.".into()
        });
    let id = app.servicios.juego.ultimo.as_deref();
    let instancia = id.and_then(|id| registro(app, id));
    let nombre = instancia
        .as_ref()
        .map_or("Minecraft Vanilla", |i| i.nombre.as_str());
    let salida = id.and_then(|id| app.servicios.juego.resultados.get(id));
    let codigo = salida
        .and_then(|(salida, _)| *salida)
        .and_then(|s| s.codigo);
    let detalle = if let Some((_, hora)) = salida {
        format!(
            "Código de salida {} · {hora} · {nombre}",
            codigo.map_or("desconocido".into(), |c| c.to_string())
        )
    } else {
        format!("Sin código de salida · {nombre}")
    };
    let causa = container(
        row![
            icono(Icono::Alerta, Tam::Base, p.error),
            column![
                text("Causa desconocida")
                    .font(fuentes::TITULO)
                    .size(texto::BODY.0 + 1.)
                    .color(p.text),
                text(error).size(texto::BODY_SM.0).color(p.text_muted),
            ]
            .spacing(espacio::S1)
        ]
        .spacing(espacio::S3),
    )
    .padding(espacio::S4)
    .width(Length::Fill)
    .style(move |_: &iced::Theme| iced::widget::container::Style {
        background: Some(iced::Background::Color(p.error_soft)),
        border: iced::Border {
            color: p.error,
            width: 2.,
            radius: 0.0.into(),
        },
        ..Default::default()
    });
    let mut botones = row![].spacing(espacio::S3);
    if let Some(index) = id.and_then(|id| {
        app.servicios
            .instancias
            .registros
            .iter()
            .position(|i| i.id == id)
    }) {
        botones = botones.push(componentes::boton(
            p,
            "Volver a la instancia",
            componentes::Variante::Primario,
            Some(Mensaje::AbrirInstancia(index)),
        ));
    }
    botones = botones
        .push(componentes::boton(
            p,
            "Abrir registro",
            componentes::Variante::Secundario,
            Some(Mensaje::Ir(icaro_ui::shell::Seccion::Consola)),
        ))
        .push(componentes::boton(
            p,
            "Copiar informe",
            componentes::Variante::Fantasma,
            Some(m(Accion::CopiarInforme)),
        ));
    let mut col = column![
        text("EL JUEGO\nSE CERRÓ")
            .font(fuentes::DISPLAY)
            .size(texto::DISPLAY.0)
            .color(p.text),
        text(detalle)
            .font(fuentes::MONO)
            .size(texto::MONO_SM.0)
            .color(p.text_muted),
        causa,
        botones,
        iced::widget::button(
            row![
                text("Detalle técnico")
                    .font(fuentes::TITULO)
                    .size(texto::BODY.0)
                    .width(Length::Fill),
                icaro_ui::iconos::icono_texto(
                    if app.detalle_fallo {
                        Icono::Flecha
                    } else {
                        Icono::ChevronAbajo
                    },
                    Tam::Base
                )
            ]
            .align_y(Alignment::Center)
        )
        .width(Length::Fill)
        .on_press(Mensaje::Pant(crate::pantallas::Accion::DetalleTecnico))
        .style(icaro_ui::estilo::boton_fantasma(p))
    ]
    .spacing(espacio::S4);
    if app.detalle_fallo {
        col = col.push(
            container(
                text(texto_registro(app))
                    .font(fuentes::MONO)
                    .size(texto::MONO_SM.0)
                    .color(p.text),
            )
            .padding(espacio::S4)
            .width(Length::Fill)
            .style(icaro_ui::estilo::marco(p)),
        );
    }
    crate::pantallas::con_lamina(p, Lamina::Rostro, crate::desplazable_pub(col.into()))
}

pub fn instalaciones_java(app: &App) -> Vec<icaro_ui::editor::InstalacionJava> {
    let captura = std::env::var_os("ICARO_CAPTURA_DISENO").is_some();
    let mut detectadas: BTreeMap<std::path::PathBuf, (java::Java, Vec<String>)> = BTreeMap::new();
    for runtime in &app.java_detectadas {
        detectadas
            .entry(runtime.executable.clone())
            .or_insert((runtime.clone(), vec![]));
    }
    for (id, runtime) in &app.servicios.juego.java {
        let (_, usado) = detectadas
            .entry(runtime.executable.clone())
            .or_insert_with(|| (runtime.clone(), vec![]));
        if let Some(instancia) = registro(app, id) {
            if !usado.contains(&instancia.nombre) {
                usado.push(instancia.nombre);
            }
        }
    }
    detectadas
        .into_values()
        .map(|(runtime, usado)| icaro_ui::editor::InstalacionJava {
            version: format!("{} · {}", runtime.version, runtime.arquitectura),
            proveedor: if runtime.managed { "Mojang" } else { "Sistema" }.into(),
            gestionada: runtime.managed,
            ruta: runtime.executable.to_string_lossy().into_owned(),
            usado_por: if usado.is_empty() {
                "Ninguna".into()
            } else {
                usado.join(", ")
            },
            existe: captura || runtime.executable.is_file(),
        })
        .collect()
}

/// Datos aislados para capturar los widgets reales; no ejecuta tareas ni usa
/// cuentas, red, archivos del usuario o procesos externos.
pub fn preparar_diseno(app: &mut App, scene: &str) {
    let cantidad = if scene == "juego-descargas" { 4 } else { 1 };
    let nombres = ["Supervivencia", "Creativo", "Pruebas", "Vanilla 1.20.4"];
    let registros: Vec<_> = (0..cantidad)
        .map(|n| Instancia {
            id: format!("{:032x}", n + 1),
            nombre: nombres[n].into(),
            minecraft: if n == 3 { "1.20.4" } else { "1.21.1" }.into(),
            loader: "vanilla".into(),
            portada: n as u8,
            creada: 0,
        })
        .collect();
    let almacen = icaro_core::instancias::Almacen::nuevo(
        std::env::temp_dir().join("icaro-qa-diseno-vanilla-aislado"),
    );
    *crate::lista_instancias().lock().unwrap() = registros
        .iter()
        .map(|i| crate::instancias_reales::Estado::datos(&almacen, i))
        .collect();
    app.servicios.demo = false;
    app.servicios.restoring = false;
    app.servicios.session = None;
    app.servicios.instancias.almacen = almacen;
    app.servicios.instancias.registros = registros;
    app.servicios.juego = Estado::default();
    app.servicios.mods = Default::default();
    app.configs.resize_with(cantidad, crate::config_por_defecto);
    app.configs.truncate(cantidad);
    app.grupos = vec![None; cantidad];
    app.lista_grupos.clear();
    app.seleccionada = Some(0);
    app.editor = None;
    app.editor_inst = 0;
    app.pantalla = None;
    app.umbral = None;
    app.menu = None;
    app.asistente = None;
    app.picar = None;
    app.aviso = None;
    app.general.nombre = "Supervivencia".into();
    let id = app.servicios.instancias.registros[0].id.clone();
    app.servicios.juego.ultimo = Some(id.clone());
    app.servicios.juego.logs.insert(
        id.clone(),
        vec![
            Linea {
                hora: "14:02:11".into(),
                nivel: Nivel::Info,
                mensaje: "[main/INFO]: Iniciando Minecraft 1.21.1".into(),
            },
            Linea {
                hora: "14:02:12".into(),
                nivel: Nivel::Info,
                mensaje: "[main/INFO]: Archivos del juego verificados".into(),
            },
            Linea {
                hora: "14:02:19".into(),
                nivel: Nivel::Warn,
                mensaje: "[Render thread/WARN]: No se pudo cargar una textura".into(),
            },
            Linea {
                hora: "14:02:24".into(),
                nivel: Nivel::Info,
                mensaje: "[Render thread/INFO]: Preparando los recursos del juego".into(),
            },
            Linea {
                hora: "14:02:31".into(),
                nivel: Nivel::Info,
                mensaje: "[Render thread/INFO]: Minecraft listo".into(),
            },
        ],
    );
    match scene {
        "juego-descargas" => {
            app.seccion = icaro_ui::shell::Seccion::Descargas;
            let registros = app.servicios.instancias.registros.clone();
            for (n, instancia) in registros.into_iter().enumerate() {
                let estado = match n {
                    0 => EstadoDescarga::Activa(0.48),
                    1 => EstadoDescarga::Pausada(0.31),
                    2 => EstadoDescarga::EnCola(1),
                    _ => EstadoDescarga::Completada,
                };
                let pausa = n == 1;
                let cancel = CancellationToken::new();
                if pausa {
                    cancel.cancel();
                }
                app.servicios.juego.orden.push(instancia.id.clone());
                app.servicios.juego.trabajos.insert(
                    instancia.id.clone(),
                    Trabajo {
                        nombre: instancia.nombre.clone(),
                        generacion: n as u64 + 1,
                        estado,
                        etapa: match n {
                            0 => "Descargando bibliotecas · 48 de 100",
                            1 => "Instalación en pausa",
                            2 => "Preparando instalación",
                            _ => "Minecraft y Java preparados",
                        }
                        .into(),
                        jugar: false,
                        cancel,
                        pausando: pausa,
                        reanudar: false,
                        preparando: false,
                        partes: if n == 0 { Some((48, 100)) } else { None },
                        bytes: false,
                    },
                );
                if n == 2 {
                    app.servicios.juego.pendientes.push_back(Solicitud {
                        instancia,
                        jugar: false,
                        java: None,
                        descarga_java: true,
                    });
                }
            }
            app.servicios.juego.activa = Some(id.clone());
            app.servicios.juego.secuencia = 4;
            tarjeta(
                app,
                &id,
                EstadoInstancia::Instalando(0.48),
                "Descargando Minecraft",
            );
        }
        "juego-consola" => {
            app.seccion = icaro_ui::shell::Seccion::Consola;
            app.servicios.juego.mostrar_ultimo_registro = true;
            app.busqueda_consola = "chunk".into();
            app.servicios.juego.logs.insert(id.clone(), [
                ("14:02:31", Nivel::Info, "[Render thread] Backend library: LWJGL version 3.3.3"),
                ("14:02:33", Nivel::Info, "[Worker-Main-4] Loaded 214 mods via Fabric Loader 0.16.9"),
                ("14:02:34", Nivel::Warn, "[Render thread] Shader iris: incompatible mipmap level with warnings"),
                ("14:02:40", Nivel::Info, "[Server thread] Preparing spawn area: 84%"),
                ("14:02:41", Nivel::Warn, "[Server thread] Can't keep up! Is the server overloaded? Running 2304ms behind"),
                ("14:02:44", Nivel::Info, "[Server thread] Saving chunks for level 'ServerLevel[Mundo]'/minecraft:overworld"),
                ("14:02:47", Nivel::Error, "[Worker-Main-2] Failed to load chunk at [-12, 8]: region file corrupted"),
                ("14:02:47", Nivel::Info, "[Server thread] Regenerating chunk [-12, 8]"),
            ].into_iter().map(|(hora, nivel, mensaje)| Linea { hora: hora.into(), nivel, mensaje: mensaje.into() }).collect());
        }
        "juego-consola-vacia" => {
            app.seccion = icaro_ui::shell::Seccion::Consola;
        }
        "juego-capturas" | "juego-capturas-visor" => {
            app.seccion = icaro_ui::shell::Seccion::Capturas;
            app.capt = crate::capt::Estado::vacio();
            if let Some(ruta) = std::env::var_os("ICARO_QA_CAPTURA").map(std::path::PathBuf::from) {
                if let Ok(meta) = std::fs::metadata(&ruta) {
                    app.capt.capturas.push(icaro_ui::capturas::Captura {
                        archivo: ruta
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned(),
                        instancia: "Supervivencia".into(),
                        fecha: "11-10-2026 20:03".into(),
                        bytes: meta.len(),
                        imagen: Lamina::Umbral,
                        original: Some(icaro_ui::capturas::OriginalCaptura {
                            imagen: iced::widget::image::Handle::from_path(&ruta),
                            ruta,
                            instancia_id: id.clone(),
                            modificada: meta.modified().ok(),
                        }),
                    });
                    if scene == "juego-capturas-visor" {
                        let _ = crate::capt::actualizar(app, crate::capt::Accion::Abrir(0));
                    }
                }
            }
        }
        "juego-fallo" => {
            app.pantalla = Some(crate::pantallas::Pantalla::Fallo);
            app.servicios.juego.errores.insert(
                id.clone(),
                "Minecraft se cerró con código 1. Abre el registro para ver qué pasó.".into(),
            );
            app.servicios.juego.resultados.insert(
                id.clone(),
                (
                    Some(lanzamiento::Salida {
                        codigo: Some(1),
                        exito: false,
                        cancelado: false,
                    }),
                    "14:02:47".into(),
                ),
            );
            app.servicios.juego.logs.get_mut(&id).unwrap().push(Linea {
                hora: "14:02:47".into(),
                nivel: Nivel::Error,
                mensaje:
                    "[main/ERROR]: El cliente terminó durante la preparación de componentes nativos"
                        .into(),
            });
        }
        "juego-editor" => {
            app.editor = Some(1);
            app.seccion = icaro_ui::shell::Seccion::Instancias;
            app.servicios.juego.java.insert(
                id.clone(),
                java::Java {
                    executable: std::path::PathBuf::from("C:/Ícaro/java/21/bin/java.exe"),
                    major: 21,
                    version: "21.0.4".into(),
                    managed: true,
                    arquitectura: "amd64".into(),
                },
            );
            tarjeta(app, &id, EstadoInstancia::Lista, "Lista para jugar");
        }
        "juego-instalando" | "juego-preparando" => {
            app.seccion = icaro_ui::shell::Seccion::Instancias;
            app.editor = Some(0);
            app.servicios.juego.trabajos.insert(
                id.clone(),
                Trabajo {
                    nombre: "Supervivencia".into(),
                    generacion: 1,
                    estado: EstadoDescarga::Activa(0.34),
                    etapa: "Descargando bibliotecas".into(),
                    jugar: false,
                    cancel: CancellationToken::new(),
                    pausando: false,
                    reanudar: false,
                    preparando: scene == "juego-preparando",
                    partes: Some((34, 100)),
                    bytes: false,
                },
            );
        }
        s if s.starts_with("juego-umbral") => {
            app.seccion = icaro_ui::shell::Seccion::Instancias;
            app.editor = Some(0);
            app.reducir = false;
            let ms = match s {
                "juego-umbral-entrada" => 60,
                "juego-umbral-arco" => 700,
                _ => 3500,
            };
            let ahora = Instant::now();
            app.ahora = ahora;
            app.umbral = Some(
                ahora
                    .checked_sub(Duration::from_millis(ms))
                    .unwrap_or(ahora),
            );
            app.servicios.juego.umbral_generacion = Some((id.clone(), 1));
            if s == "juego-umbral-retirada" || s == "juego-umbral-fallo" {
                app.servicios.juego.retirada_umbral = Some(RetiradaUmbral {
                    id: id.clone(),
                    generacion: 1,
                    desde: ahora
                        .checked_sub(Duration::from_millis(100))
                        .unwrap_or(ahora),
                    fallo: s == "juego-umbral-fallo",
                    minimizar: false,
                    consola: false,
                });
            }
        }
        "juego-ejecutando" => {
            app.seccion = icaro_ui::shell::Seccion::Instancias;
            app.editor = Some(0);
            let ahora = Instant::now();
            app.servicios.juego.procesos.insert(
                id.clone(),
                Proceso {
                    generacion: 1,
                    cancel: CancellationToken::new(),
                    desde: ahora.checked_sub(Duration::from_secs(372)).unwrap_or(ahora),
                    cerrar_hasta: None,
                    pid: Some(1234),
                    ventana_lista: true,
                },
            );
            tarjeta(app, &id, EstadoInstancia::Jugando, "En ejecución");
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn umbral_espera_ventana_estable_y_no_termina_por_spawn_o_por_limite() {
        let mut muestras = VecDeque::from([
            SondaVentana::Esperando,
            SondaVentana::Lista,
            SondaVentana::Esperando,
            SondaVentana::Lista,
            SondaVentana::Lista,
        ]);
        let resultado = esperar_ventana(
            CancellationToken::new(),
            Duration::from_millis(1),
            Duration::from_secs(1),
            || muestras.pop_front().unwrap_or(SondaVentana::Esperando),
        )
        .await;
        assert_eq!(resultado, ResultadoVentana::Lista);
        assert!(muestras.is_empty());
        let resultado = esperar_ventana(
            CancellationToken::new(),
            Duration::from_millis(1),
            Duration::ZERO,
            || SondaVentana::Esperando,
        )
        .await;
        assert_eq!(resultado, ResultadoVentana::SinConfirmar);
    }

    #[tokio::test]
    async fn observador_termina_al_cancelar_o_finalizar_el_proceso() {
        let cancel = CancellationToken::new();
        cancel.cancel();
        let resultado = esperar_ventana(
            cancel,
            Duration::from_millis(1),
            Duration::from_secs(1),
            || panic!("No debe observar después de cancelar"),
        )
        .await;
        assert_eq!(resultado, ResultadoVentana::Cancelada);
        let resultado = esperar_ventana(
            CancellationToken::new(),
            Duration::from_millis(1),
            Duration::from_secs(1),
            || SondaVentana::Finalizada,
        )
        .await;
        assert_eq!(resultado, ResultadoVentana::Finalizada);
    }

    #[test]
    fn ventana_de_otro_pid_o_intento_no_cambia_la_instancia() {
        let mut estado = Estado::default();
        estado.procesos.insert(
            "juego".into(),
            Proceso {
                generacion: 9,
                cancel: CancellationToken::new(),
                desde: Instant::now(),
                cerrar_hasta: None,
                pid: Some(1234),
                ventana_lista: false,
            },
        );
        assert!(!estado.ventana_lista("juego", 8, 1234));
        assert!(!estado.ventana_lista("juego", 9, 4321));
        assert!(!estado.procesos["juego"].ventana_lista);
        assert!(estado.ventana_lista("juego", 9, 1234));
        estado.procesos["juego"].cancel.cancel();
        assert!(!estado.ventana_lista("juego", 9, 1234));
    }
    fn trabajo(estado: EstadoDescarga) -> Trabajo {
        Trabajo {
            nombre: "Vanilla".into(),
            generacion: 7,
            estado,
            etapa: "Descargando".into(),
            jugar: false,
            cancel: CancellationToken::new(),
            pausando: false,
            reanudar: false,
            preparando: false,
            partes: Some((40, 100)),
            bytes: false,
        }
    }
    fn estado() -> Estado {
        let mut estado = Estado::default();
        estado.activa = Some("a".into());
        estado
            .trabajos
            .insert("a".into(), trabajo(EstadoDescarga::Activa(0.4)));
        estado
    }

    #[test]
    fn quitar_registro_conserva_datos_y_espera_el_acuse_de_cancelacion() {
        let mut state=estado();
        state.orden=vec!["a".into(),"completa".into(),"pausada".into()];
        state.trabajos.insert("completa".into(),trabajo(EstadoDescarga::Completada));
        state.trabajos.insert("pausada".into(),trabajo(EstadoDescarga::Pausada(0.3)));
        state.errores.insert("completa".into(),"registro conservado".into());
        state.quitar_registro(0);assert_eq!(state.orden.len(),3);
        state.quitar_registro(2);assert_eq!(state.orden.len(),3);
        state.quitar_registro(1);
        assert_eq!(state.orden,vec!["a","pausada"]);
        assert!(state.trabajos.contains_key("completa"));
        assert_eq!(state.errores["completa"],"registro conservado");
        state.suspender("a",false);
        state.quitar_registro(0);assert_eq!(state.orden.len(),2);
        state.activa=None;
        state.quitar_registro(0);assert_eq!(state.orden,vec!["pausada"]);
        state.quitar_registro(999);assert_eq!(state.orden.len(),1);
    }

    #[test]
    fn pausar_reanudar_antes_del_ack_y_cancelar_una_pausa() {
        let mut estado = estado();
        assert!(!estado.suspender("a", true));
        assert_eq!(estado.trabajos["a"].estado, EstadoDescarga::Pausada(0.4));
        assert!(estado.reanudar_pendiente("a"));
        assert!(estado.trabajos["a"].reanudar);
        assert!(estado.trabajos["a"].cancel.is_cancelled());
        assert!(estado
            .progreso("a", 7, "Avance tardío".into(), 90, 100)
            .is_none());
        // Cancelar la fila pausada revoca también el reintento que esperaba.
        assert!(!estado.suspender("a", false));
        assert!(!estado.trabajos["a"].reanudar && !estado.trabajos["a"].pausando);
        assert!(matches!(
            estado.trabajos["a"].estado,
            EstadoDescarga::Fallo(_)
        ));
    }

    #[test]
    fn logout_vacia_cola_y_cancela_actividad_sin_perder_el_ack_del_proceso() {
        let mut estado = estado();
        estado
            .trabajos
            .insert("b".into(), trabajo(EstadoDescarga::EnCola(1)));
        estado
            .trabajos
            .insert("c".into(), trabajo(EstadoDescarga::Completada));
        estado.pendientes.push_back(Solicitud {
            instancia: Instancia {
                id: "b".into(),
                nombre: "Otra".into(),
                minecraft: "1.21.1".into(),
                loader: "vanilla".into(),
                portada: 0,
                creada: 0,
            },
            jugar: false,
            java: None,
            descarga_java: true,
        });
        estado.espera_sesion = Some("b".into());
        let cancel = CancellationToken::new();
        estado.procesos.insert(
            "juego".into(),
            Proceso {
                generacion: 8,
                cancel: cancel.clone(),
                desde: Instant::now(),
                cerrar_hasta: None,
                pid: Some(1234),
                ventana_lista: false,
            },
        );
        estado.cancelar_todo();
        assert!(estado.pendientes.is_empty() && estado.espera_sesion.is_none());
        assert!(matches!(
            estado.trabajos["a"].estado,
            EstadoDescarga::Fallo(_)
        ));
        assert!(matches!(
            estado.trabajos["b"].estado,
            EstadoDescarga::Fallo(_)
        ));
        assert_eq!(estado.trabajos["c"].estado, EstadoDescarga::Completada);
        assert!(cancel.is_cancelled());
        assert!(estado.procesos.contains_key("juego") && !estado.libre());
    }

    #[test]
    fn generaciones_anteriores_no_sobrescriben_el_avance_del_reintento() {
        let mut estado = estado();
        assert!(estado
            .progreso("a", 6, "Mensaje de otro intento".into(), 99, 100)
            .is_none());
        assert_eq!(estado.trabajos["a"].estado, EstadoDescarga::Activa(0.4));
        assert!(estado
            .progreso("a", 7, "Verificando Java".into(), 0, 0)
            .is_some());
        assert!(!estado.trabajos["a"].preparando);
        assert!(estado.trabajos["a"].partes.is_none());
        estado.trabajos.get_mut("a").unwrap().generacion = 8;
        assert!(estado
            .progreso("a", 7, "Mensaje de la generación retirada".into(), 99, 100)
            .is_none());
        assert!(estado
            .progreso("a", 8, "Descargando Java".into(), 4, 10)
            .is_some());
        assert_eq!(estado.trabajos["a"].estado, EstadoDescarga::Activa(0.4));
        assert!(!estado.trabajos["a"].preparando);
        assert_eq!(estado.trabajos["a"].partes, Some((4, 10)));
        assert!(estado.trabajos["a"].bytes);
    }

    #[test]
    fn eventos_intercalados_no_alternan_el_boton_durante_la_instalacion() {
        let mut estado = estado();
        estado.trabajos.get_mut("a").unwrap().preparando = true;
        estado.progreso("a", 7, "Verificando Java".into(), 0, 0);
        assert!(estado.trabajos["a"].preparando);
        estado.progreso("a", 7, "Descargando Java".into(), 1, 100);
        for n in 1..64 {
            for etapa in [
                "Verificando archivos de Minecraft",
                "Descargando librerías",
                "Preparando componentes nativos",
            ] {
                assert!(estado.progreso("a", 7, etapa.into(), n, 100).is_some());
                assert!(!estado.trabajos["a"].preparando, "{etapa}");
                assert!(matches!(
                    estado.trabajos["a"].estado,
                    EstadoDescarga::Activa(_)
                ));
            }
        }
    }

    #[test]
    fn cache_y_componentes_guardados_permanecen_en_preparando() {
        let mut estado = estado();
        estado.trabajos.get_mut("a").unwrap().preparando = true;
        for etapa in [
            "Verificando archivos de Minecraft",
            "Preparando componentes nativos",
            "Preparando recursos de esta versión",
            "Minecraft preparado",
        ] {
            assert!(estado.progreso("a", 7, etapa.into(), 90, 100).is_some());
            assert!(estado.trabajos["a"].preparando, "{etapa}");
            assert!(estado.indicador_descarga().is_none(), "{etapa}");
        }
        estado.progreso("a", 7, "Descargando Bibliotecas".into(), 90, 100);
        assert!(!estado.trabajos["a"].preparando);
        let indicador = estado.indicador_descarga().expect("Descarga real visible");
        assert_eq!(indicador.cantidad, 1);
        assert_eq!(indicador.velocidad, "90 %");
        assert_eq!(indicador.avance, 0.9);
        estado.trabajos["a"].cancel.cancel();
        assert!(estado.indicador_descarga().is_none());
    }

    #[test]
    fn recuperar_log_preserva_niveles_hora_y_excepciones_multilinea() {
        let lineas = lineas_guardadas("[14:02:41] [main/WARN]: Advertencia\n[14:02:47] [Render thread/ERROR]: Fallo\n\tat minecraft.Main.run(Main.java:42)\n");
        assert_eq!(lineas.len(), 3);
        assert_eq!(lineas[0].hora, "14:02:41");
        assert_eq!(lineas[0].nivel, Nivel::Warn);
        assert_eq!(lineas[1].nivel, Nivel::Error);
        assert!(lineas[2].mensaje.contains("Main.java:42"));
    }
}
