// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Pantallas completas, diálogos y páginas de Ajustes del launcher: primera
//! ejecución, inicio de sesión con Microsoft, errores, asistente de
//! instancias, importar, exportar y las secciones de Ajustes.

use std::time::Instant;

use icaro_ui::ajustes::{encabezado_ajustes, fila_ajuste};
use icaro_ui::componentes::{self, casilla, etiqueta, insignia, Estado, Variante};
use icaro_ui::fuentes;
use icaro_ui::iconos::{icono, Icono, Tam};
use icaro_ui::instancias::{DatosInstancia, EstadoInstancia};
use icaro_ui::laminas::{grabado, Lamina};
use icaro_ui::tema::{espacio, texto, Paleta};
use iced::widget::{column, container, row, text, Space};
use iced::{Alignment, Element, Length, Padding, Task};

use crate::{arrancar, config_por_defecto, instancias, lista_instancias, App, Mensaje};

#[path = "selector_cambio.rs"]
mod selector_cambio;
#[path = "exportacion_nueva.rs"]
pub mod exportacion_nueva;

/// Pantallas que reemplazan el contenido de la ventana.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Pantalla {
    Primera,
    Codigo,
    Fallo,
    ErrorTienda,
    NoVerificados,
    Sincronizar,
    Integridad,
    Actualizacion,
    Cuenta,
    Estudio,
    Red,
}

/// Fase del inicio de sesión por código de dispositivo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FaseCodigo {
    Esperando,
    Confirmado(Instant),
    Vencido,
}

/// Preferencias de la ventana de Ajustes.
pub struct Ajustes {
    pub idioma: String,
    pub abrir_al_iniciar: bool,
    pub iniciar_minimizado: bool,
    pub al_jugar: usize,
    pub confirmar_borrar: bool,
    pub consola_al_jugar: bool,
    pub copias_mundos: bool,
    pub limite_mb: i32,
    pub escala: i32,
    pub metal: usize,
    pub cache_limpia: bool,
    pub canal: icaro_core::actualizacion::Canal,
}

impl Default for Ajustes {
    fn default() -> Self {
        Self {
            idioma: "Español (Chile)".into(),
            abrir_al_iniciar: false,
            iniciar_minimizado: false,
            al_jugar: 0,
            confirmar_borrar: true,
            consola_al_jugar: false,
            copias_mundos: true,
            limite_mb: 0,
            escala: 100,
            metal: 0,
            cache_limpia: false,
            canal: crate::actualizacion_real::canal_compilado(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CampoAjuste {
    AbrirAlIniciar,
    IniciarMinimizado,
    ConfirmarBorrar,
    ConsolaAlJugar,
    CopiasMundos,
}

/// Asistente de creación de instancias, en cuatro pasos.
pub struct Asistente {
    pub paso: usize,
    pub tipo_version: usize,
    pub busqueda: String,
    pub version: usize,
    pub loader: usize,
    pub nombre: String,
    pub portada: usize,
}

pub const PASOS: [&str; 4] = ["Versión", "Loader", "Nombre e icono", "Revisar"];
pub const LOADERS: [(&str, &str, &str); 5] = [
    ("Vanilla", "El juego original, sin mods.", ""),
    (
        "Fabric",
        "Liviano y rápido. Ideal para mods de rendimiento y mapas.",
        "0.16.9",
    ),
    (
        "Quilt",
        "Compatible con casi todos los mods de Fabric.",
        "0.27.1",
    ),
    (
        "NeoForge",
        "Para modpacks grandes de tecnología y magia.",
        "21.4.38",
    ),
    ("Forge", "El clásico de los mods grandes.", "54.0.1"),
];
pub const VERSIONES: [[(&str, &str); 5]; 3] = [
    [("1.21.4", "dic 2024"), ("1.21.3", "oct 2024"), ("1.21.1", "ago 2024"), ("1.20.6", "abr 2024"), ("1.20.1", "jun 2023")],
    [("25w02a", "ene 2025"), ("24w46a", "nov 2024"), ("24w39a", "sep 2024"), ("24w33a", "ago 2024"), ("24w21b", "may 2024")],
    [("1.16.5", "ene 2021"), ("1.12.2", "sep 2017"), ("1.8.9", "dic 2015"), ("1.7.10", "jun 2014"), ("1.6.4", "sep 2013")],
];
pub const PORTADAS: [(&str, Lamina); 4] = [
    ("Castillo", Lamina::CastilloPasarela),
    ("Valle", Lamina::TivoliCascada),
    ("Aldea", Lamina::MercadoAldea),
    ("Ciudad", Lamina::TivoliRocas),
];

/// Importación de una instancia desde un archivo, otro launcher o un clon.
pub struct Importar {
    pub origen: usize,
    pub archivo: Option<&'static str>,
    pub launchers: [bool; 3],
    pub clon_origen: usize,
    pub con_mundos: bool,
}

/// Exportación de una instancia como modpack `.mrpack`.
pub struct Exportar {
    pub instancia: usize,
    pub nombre: String,


    pub formato: usize,
    pub destino: String,
    pub error: Option<String>,
    pub guardando: bool,
    pub arbol: Option<icaro_core::exportar::Arbol>,
    pub preset: usize,
    pub hechos: u64,
    pub total: u64,
}

/// Todo lo que cambian estas pantallas.
#[derive(Debug, Clone)]
pub enum Accion {
    Abrir(Pantalla),
    Cerrar,
    // Ajustes
    Idioma(String),
    Alternar(CampoAjuste),
    AlJugar(String),
    Limite(i32),
    Escala(i32),
    Metal(usize),
    LimpiarCache,
    AbrirCarpetaDatos,
    // Asistente
    NuevaInstancia,
    AsistenteCancelar,
    AsistenteAtras,
    AsistenteSiguiente,
    AsistentePaso(usize),
    AsistenteTipo(usize),
    AsistenteBuscar(String),
    AsistenteVersion(usize),
    AsistenteLoader(usize),
    AsistenteLoaderVersion(String),
    VersionScroll(bool, f32),
    AsistenteNombre(String),
    AsistentePortada(usize),
    AsistenteCrear,
    // Importar
    Importar,
    ImportarOrigen(usize),
    ImportarArchivo,
    ImportarLauncher(usize),
    ImportarClon(String),
    ImportarMundos(bool),
    ImportarCancelar,
    ImportarConfirmar,
    // Exportar
    Exportar(usize),
    ExportarArbol(String, Result<icaro_core::exportar::Arbol, String>),
    ExportarPreset(usize),
    ExportarNodo(String),
    ExportarExpandir(String),
    ExportarArchivo(Option<std::path::PathBuf>),
    ExportarProgreso(u64,u64),
    MostrarExportacion(std::path::PathBuf),
    AbrirListaEnlace(String),
    AbrirRepositorio,
    Canal(String),

    ExportarCancelar,
    ExportarConfirmar,
    ExportarFormato(usize),
    Exportada(Result<std::path::PathBuf, String>),
    // Cuenta y errores
    CopiarCodigo,
    AbrirPaginaMicrosoft,
    PedirOtroCodigo,
    SimularConfirmacion,
    JugarComoInvitado,
    Reintentar,
    DesactivarYJugar,
    CopiarInforme,
    DetalleTecnico,
    CambiarVersion(usize),
    CambiarVersionLoader(usize),
    CambiarLoaderVersion(String),
    AplicarVersionLoader,
    CambiarCancelar,
    CambiarConfirmar(usize),
    CambiarTipo(usize),
    CambiarBuscar(String),
    CambiarDestino(usize),
    CambiarRevisar,
    CambiarRespaldo(bool),
    CambiarActualizar(bool),
    CambiarModo(bool),
    CambiarDetalle,
    #[allow(dead_code)] // Rechaza eventos antiguos de cambio de familia.
    CambiarLoader(String),
    CambiarCopia,
    VerInstalados,
    Duplicar(usize),
    AbrirCarpetaInstancia(usize),
}

fn nueva_instancia(nombre: &str, version: String, portada: Lamina) -> DatosInstancia {
    DatosInstancia {
        nombre: nombre.to_owned(),
        version,
        ultima_vez: "Nueva".into(),
        mods: "Sin mods".into(),
        portada,
        estado: EstadoInstancia::Lista,
        seleccionada: false,
        detalle: String::new(),
    }
}

/// Nombre que no choque con ninguna instancia existente.
fn nombre_unico(base: &str) -> String {
    let existentes: Vec<String> = instancias().into_iter().map(|d| d.nombre).collect();
    if !existentes.iter().any(|n| n == base) {
        return base.to_owned();
    }
    (2..)
        .map(|n| format!("{base} {n}"))
        .find(|c| !existentes.contains(c))
        .unwrap_or_else(|| base.to_owned())
}

/// Agrega la instancia a la lista y a los arreglos paralelos de la app.
fn agregar_instancia(app: &mut App, d: DatosInstancia) {
    lista_instancias().lock().unwrap().push(d);
    app.grupos.push(None);
    app.configs.push(config_por_defecto());
}

pub fn actualizar(app: &mut App, a: Accion) -> Task<Mensaje> {
    if let Some(task) = crate::instancias_reales::accion_cambio(app, &a) {
        return task;
    }
    if app.servicios.instancias.creando
        && matches!(
            &a,
            Accion::NuevaInstancia
                | Accion::AsistenteCancelar
                | Accion::AsistenteAtras
                | Accion::AsistenteSiguiente
                | Accion::AsistentePaso(_)
                | Accion::AsistenteTipo(_)
                | Accion::AsistenteBuscar(_)
                | Accion::AsistenteVersion(_)
                | Accion::AsistenteLoader(_)
                | Accion::AsistenteLoaderVersion(_)
                | Accion::AsistenteNombre(_)
                | Accion::AsistentePortada(_)
                | Accion::AsistenteCrear
        )
    {
        return Task::none();
    }
    if !app.servicios.demo {
        match &a {
            Accion::Cerrar if app.pantalla == Some(Pantalla::Sincronizar) => return crate::sincronizacion_real::actualizar(app,crate::sincronizacion_real::Accion::Cerrar),
            Accion::Abrir(Pantalla::Codigo)
            | Accion::PedirOtroCodigo
            | Accion::SimularConfirmacion => {
                return crate::servicios::actualizar(app, crate::servicios::Accion::Login)
            }
            Accion::AbrirPaginaMicrosoft => {
                return crate::servicios::actualizar(app, crate::servicios::Accion::LoginBrowser)
            }
            Accion::Cerrar if app.pantalla == Some(Pantalla::Codigo) => {
                return crate::servicios::actualizar(app, crate::servicios::Accion::CancelarLogin)
            }
            Accion::JugarComoInvitado => {
                return crate::servicios::actualizar(
                    app,
                    crate::servicios::Accion::ContinuarInvitado,
                );
            }
            Accion::Abrir(Pantalla::NoVerificados) => return crate::verificacion_real::actualizar(app,crate::verificacion_real::Accion::Abrir),
            Accion::AsistenteCrear => return crate::instancias_reales::crear(app),
            Accion::CambiarConfirmar(_) => return crate::instancias_reales::cambiar(app, false),
            Accion::CambiarCopia => return crate::instancias_reales::cambiar(app, true),
            Accion::CambiarRevisar => return crate::instancias_reales::revisar_cambio(app),
            Accion::CambiarVersion(i) => return crate::instancias_reales::abrir_cambio(app, *i),
            Accion::CambiarVersionLoader(i) => return crate::instancias_reales::abrir_cambio_loader(app,*i),
            Accion::AplicarVersionLoader => return crate::instancias_reales::aplicar_cambio_loader(app),
            Accion::Importar => return crate::modpacks_reales::actualizar(app,crate::modpacks_reales::Accion::Importar),
            Accion::ImportarConfirmar | Accion::Duplicar(_) => {
                app.avisar("La importación y duplicación todavía están en preparación.".into());
                return Task::none();
            }
            Accion::LimpiarCache => {
                return crate::almacenamiento_real::actualizar(app,crate::almacenamiento_real::Accion::Limpiar);
            }
            Accion::CopiarCodigo => return Task::none(),
            _ => {}
        }
    }
    match a {
        Accion::Abrir(p) => {
            app.pantalla = Some(p);
            if p == Pantalla::Codigo {
                app.codigo_inicio = Instant::now();
                app.codigo_fase = FaseCodigo::Esperando;
            }
            if p == Pantalla::ErrorTienda {
                app.intentos = 0;
            }
            if p == Pantalla::Integridad {
                app.codigo_inicio = Instant::now();
            }
            arrancar(app);
        }
        Accion::Cerrar => {
            app.pantalla = None;
            arrancar(app);
        }
        Accion::Idioma(v) => app.aj.idioma = v,
        Accion::Alternar(c) => match c {
            CampoAjuste::AbrirAlIniciar => app.aj.abrir_al_iniciar = !app.aj.abrir_al_iniciar,
            CampoAjuste::IniciarMinimizado => {
                app.aj.iniciar_minimizado = !app.aj.iniciar_minimizado
            }
            CampoAjuste::ConfirmarBorrar => app.aj.confirmar_borrar = !app.aj.confirmar_borrar,
            CampoAjuste::ConsolaAlJugar => app.aj.consola_al_jugar = !app.aj.consola_al_jugar,
            CampoAjuste::CopiasMundos => app.aj.copias_mundos = !app.aj.copias_mundos,
        },
        Accion::AlJugar(v) => {
            app.aj.al_jugar = ["Mantener abierto", "Minimizar", "Cerrar el launcher"]
                .iter()
                .position(|o| *o == v)
                .unwrap_or(0);
        }
        Accion::Limite(n) => app.aj.limite_mb = n.clamp(0, 100),
        Accion::Escala(n) => app.aj.escala = n.clamp(80, 150),
        Accion::Metal(i) => app.aj.metal = i.min(2),
        Accion::LimpiarCache => {
            app.aj.cache_limpia = true;
            app.avisar("Caché limpiada: se liberaron 380 MB.".into());
        }
        Accion::AbrirCarpetaDatos => {
            let dir = crate::estado::directorio(app.servicios.demo);
            let _ = std::fs::create_dir_all(&dir);
            if let Err(e) = abrir_en_sistema(&dir) {
                app.avisar(e);
            }
        }
        // ---- Asistente
        Accion::NuevaInstancia => {
            app.servicios.instancias.loader_elegida = None;
            app.servicios.instancias.version_offset = 0.0;
            app.asistente = Some(Asistente {
                paso: 0,
                tipo_version: 0,
                busqueda: String::new(),
                version: 0,
                loader: 0,
                nombre: String::new(),
                portada: 0,
            });
            if !app.servicios.demo {
                return crate::instancias_reales::abrir(app);
            }
        }
        Accion::AsistenteCancelar => {
            if !app.servicios.instancias.creando {
                app.asistente = None;
            }
        }
        Accion::AsistenteAtras => {
            if let Some(s) = app.asistente.as_mut() {
                s.paso = s.paso.saturating_sub(1);
            }
            if app.asistente.as_ref().is_some_and(|s| s.paso == 1) {
                return crate::instancias_reales::cargar_loaders(app);
            }
        }
        Accion::AsistenteSiguiente => {
            let version = crate::instancias_reales::elegida(app);
            if !app.servicios.demo && version.is_none() {
                return Task::none();
            }
            if !app.servicios.demo&&app.asistente.as_ref().is_some_and(|s|s.paso==1&&s.loader>0&&!app.servicios.instancias.loaders.get(version.as_deref().unwrap_or("")).is_some_and(|l|!l.versiones[s.loader.min(4)].is_empty())){return Task::none();}
            if let Some(s) = app.asistente.as_mut() {
                s.paso = (s.paso + 1).min(3);
                if s.paso == 2 && s.nombre.is_empty() {
                    s.nombre = format!(
                        "{} {}",
                        LOADERS[s.loader].0,
                        version.as_deref().unwrap_or("")
                    );
                }
            }
            if app.asistente.as_ref().is_some_and(|s| s.paso == 1) {
                return crate::instancias_reales::cargar_loaders(app);
            }
        }
        Accion::AsistentePaso(i) => {
            if let Some(s) = app.asistente.as_mut() {
                if i < s.paso {
                    s.paso = i;
                }
            }
        }
        Accion::AsistenteTipo(i) => {
            app.servicios.instancias.loader_elegida = None;
            app.servicios.instancias.version_offset = 0.0;
            if let Some(s) = app.asistente.as_mut() {
                s.tipo_version = i.min(2);
                s.version = 0;
            }
            return iced::widget::scrollable::snap_to(
                iced::widget::scrollable::Id::new("version-asistente"),
                iced::widget::scrollable::RelativeOffset::START,
            );
        }
        Accion::AsistenteBuscar(t) => {
            app.servicios.instancias.version_offset = 0.0;
            if let Some(s) = app.asistente.as_mut() {
                s.busqueda = t;
            }
            return iced::widget::scrollable::snap_to(
                iced::widget::scrollable::Id::new("version-asistente"),
                iced::widget::scrollable::RelativeOffset::START,
            );
        }
        Accion::AsistenteVersion(i) => {
            app.servicios.instancias.loader_elegida = None;
            let valid = app.asistente.as_ref().is_some_and(|s| {
                i < crate::instancias_reales::versiones(app, s.tipo_version).len()
            });
            if let Some(s) = app.asistente.as_mut() {
                if valid {
                    s.version = i;
                }
            }
        }
        Accion::AsistenteLoader(i) => {
            app.servicios.instancias.loader_elegida = None;
            if let Some(s) = app.asistente.as_mut() {
                s.loader = i.min(4);
            }
        }
        Accion::AsistenteLoaderVersion(v) => app.servicios.instancias.loader_elegida = Some(v),
        Accion::VersionScroll(cambio, offset) => {
            if cambio {
                app.servicios.instancias.cambio_offset = offset
            } else {
                app.servicios.instancias.version_offset = offset
            }
        }
        Accion::AsistenteNombre(t) => {
            if let Some(s) = app.asistente.as_mut() {
                s.nombre = t;
            }
        }
        Accion::AsistentePortada(i) => {
            if let Some(s) = app.asistente.as_mut() {
                s.portada = i.min(PORTADAS.len() - 1);
            }
        }
        Accion::AsistenteCrear => {
            if let Some(s) = app.asistente.take() {
                let nombre = nombre_unico(if s.nombre.trim().is_empty() {
                    "Nueva instancia"
                } else {
                    s.nombre.trim()
                });
                let version = format!(
                    "{} · {}",
                    VERSIONES[s.tipo_version][s.version].0, LOADERS[s.loader].0
                );
                agregar_instancia(
                    app,
                    nueva_instancia(&nombre, version, PORTADAS[s.portada].1),
                );
                app.avisar(format!("Instancia {nombre} creada."));
                arrancar(app);
            }
        }
        // ---- Importar
        Accion::Importar => {
            app.importar = Some(Importar {
                origen: 0,
                archivo: None,
                launchers: [true, false, false],
                clon_origen: 0,
                con_mundos: false,
            });
        }
        Accion::ImportarOrigen(i) => {
            if let Some(s) = app.importar.as_mut() {
                s.origen = i.min(2);
            }
        }
        Accion::ImportarArchivo => {
            if let Some(s) = app.importar.as_mut() {
                s.archivo = Some("Fabulously Optimized 2.1.0.mrpack");
            }
        }
        Accion::ImportarLauncher(i) => {
            if let Some(s) = app.importar.as_mut() {
                if let Some(c) = s.launchers.get_mut(i) {
                    *c = !*c;
                }
            }
        }
        Accion::ImportarClon(nombre) => {
            if let Some(s) = app.importar.as_mut() {
                s.clon_origen = instancias()
                    .iter()
                    .position(|d| d.nombre == nombre)
                    .unwrap_or(0);
            }
        }
        Accion::ImportarMundos(v) => {
            if let Some(s) = app.importar.as_mut() {
                s.con_mundos = v;
            }
        }
        Accion::ImportarCancelar => app.importar = None,
        Accion::ImportarConfirmar => {
            if let Some(s) = app.importar.take() {
                let mut creadas = Vec::new();
                match s.origen {
                    0 => {
                        if s.archivo.is_some() {
                            creadas.push(nueva_instancia(
                                &nombre_unico("Fabulously Optimized"),
                                "1.21.4 · Fabric".into(),
                                Lamina::CascadaMolino,
                            ));
                        }
                    }
                    1 => {
                        for (i, (nombre, version)) in [
                            ("Mundo de Prism", "1.20.1 · Fabric"),
                            ("Pruebas de MultiMC", "1.19.4 · Forge"),
                            ("Pack de CurseForge", "1.20.1 · NeoForge"),
                        ]
                        .iter()
                        .enumerate()
                        {
                            if s.launchers[i] {
                                creadas.push(nueva_instancia(
                                    &nombre_unico(nombre),
                                    (*version).into(),
                                    Lamina::ValleRocas,
                                ));
                            }
                        }
                    }
                    _ => {
                        let todas = instancias();
                        if let Some(o) = todas.get(s.clon_origen) {
                            let mut d = o.clone();
                            d.nombre = nombre_unico(&format!("{} (copia)", o.nombre));
                            d.estado = EstadoInstancia::Lista;
                            d.detalle.clear();
                            d.ultima_vez = "Nueva".into();
                            creadas.push(d);
                        }
                    }
                }
                let n = creadas.len();
                for d in creadas {
                    agregar_instancia(app, d);
                }
                app.avisar(if n == 0 {
                    "No se importó nada.".into()
                } else {
                    format!("{n} instancia(s) importada(s).")
                });
                arrancar(app);
            }
        }
        // ---- Exportar
        Accion::Exportar(index) => {
            if app.exportar.as_ref().is_some_and(|s|s.guardando) {
                app.avisar("Ya hay una exportación en curso; puedes verla en Descargas.".into());
                return Task::none();
            }
            let Some(d) = instancias().get(index).cloned() else { return Task::none(); };
            app.exportar = Some(Exportar {
                instancia:index, nombre:d.nombre, formato:0,
                destino:std::env::var_os("USERPROFILE").map(std::path::PathBuf::from).map(|p|p.join("Downloads")).unwrap_or_else(std::env::temp_dir).to_string_lossy().into_owned(),
                error:None,guardando:false,arbol:None,preset:0,hechos:0,total:0,
            });
            app.menu=None;
            if !app.servicios.demo {
                if let Some(i)=app.servicios.instancias.registros.get(index).cloned() {
                    let store=app.servicios.instancias.almacen.clone(); let id=i.id.clone();
                    return Task::perform(async move {
                        tokio::task::spawn_blocking(move ||icaro_core::exportar::arbol(&store,&i.id)).await.unwrap_or_else(|_|Err("No se pudo leer el contenido de la instancia.".into()))
                    },move |r|m(Accion::ExportarArbol(id.clone(),r)));
                }
            }
            if let Some(s)=app.exportar.as_mut() { s.arbol=Some(exportacion_nueva::demo()); }
        }
        Accion::ExportarArbol(id,result) => {
            if let Some(s)=app.exportar.as_mut().filter(|s|!s.guardando) {
                if app.servicios.instancias.registros.get(s.instancia).is_some_and(|i|i.id==id) {
                    match result {Ok(a)=>s.arbol=Some(a),Err(e)=>s.error=Some(e)}
                }
            }
        }
        Accion::ExportarPreset(n) => {
            if let Some(s)=app.exportar.as_mut().filter(|s|!s.guardando) {
                if let Some(a)=s.arbol.as_mut() {a.seleccionar(exportacion_nueva::preset(n)); s.preset=n.min(4);}
            }
        }
        Accion::ExportarNodo(path) => {
            if let Some(s)=app.exportar.as_mut().filter(|s|!s.guardando) {
                if let Some(a)=s.arbol.as_mut() {a.alternar(&path);s.preset=4;}
            }
        }
        Accion::ExportarExpandir(path) => {
            if let Some(s)=app.exportar.as_mut().filter(|s|!s.guardando) {
                if let Some(a)=s.arbol.as_mut() {
                    if let Some(n)=a.nodos.iter_mut().find(|n|n.carpeta&&n.ruta==path){n.abierto=!n.abierto;}
                }
            }
        }
        Accion::ExportarFormato(f) => {
            if let Some(s)=app.exportar.as_mut().filter(|s|!s.guardando){s.formato=f.min(1);s.error=None;}
        }
        Accion::ExportarCancelar => {
            if !app.exportar.as_ref().is_some_and(|s|s.guardando){app.exportar=None;}
        }
        Accion::ExportarConfirmar => {
            let Some(s)=app.exportar.as_ref().filter(|s|!s.guardando&&s.arbol.as_ref().is_some_and(|a|a.resumen().0>0||a.nodos.is_empty())) else {return Task::none();};
            if app.servicios.demo {app.exportar=None;app.avisar("Modo demostración: no se exportaron archivos.".into());return Task::none();}
            if app.servicios.mods.busy.is_some() || app.servicios.instancias.operando {return Task::none();}
            let Some(i)=app.servicios.instancias.registros.get(s.instancia).cloned() else {return Task::none();};
            if app.servicios.juego.ocupada(&i.id) {
                app.exportar.as_mut().unwrap().error=Some("Espera a que termine la instalación o cierra el juego antes de exportar.".into());return Task::none();
            }
            let mut directory=std::path::PathBuf::from(&s.destino);if directory.extension().is_some_and(|e|e.eq_ignore_ascii_case("zip")){if let Some(parent)=directory.parent(){directory=parent.to_owned();}}let name=icaro_core::exportar::nombre_sugerido(&i);
            return Task::perform(async move {
                rfd::AsyncFileDialog::new().set_directory(directory).set_file_name(name).add_filter("Instancia ZIP", &["zip"]).save_file().await.map(|f|f.path().to_path_buf())
            },|r|m(Accion::ExportarArchivo(r)));
        }
        Accion::ExportarArchivo(Some(path)) => {
            let Some(s)=app.exportar.as_mut().filter(|s|!s.guardando) else {return Task::none();};
            let Some(i)=app.servicios.instancias.registros.get(s.instancia).cloned() else {return Task::none();};
            if app.servicios.mods.busy.is_some() || app.servicios.instancias.operando || app.servicios.juego.ocupada(&i.id) {
                s.error=Some("La instancia está ocupada; espera o cierra el juego antes de exportar.".into());return Task::none();
            }
            let Some(tree)=s.arbol.clone() else {return Task::none();};
            s.guardando=true;s.error=None;s.destino=path.to_string_lossy().into_owned();s.total=tree.resumen().1;s.hechos=0;
            let formato=if s.formato==0 {icaro_core::exportar::Formato::Icaro}else{icaro_core::exportar::Formato::Prism};
            let store=app.servicios.instancias.almacen.clone();
            return Task::run(iced::stream::channel(16, move |mut output|async move {
                use iced::futures::{StreamExt,SinkExt};
                let (tx,mut rx)=iced::futures::channel::mpsc::unbounded();
                let export=tokio::task::spawn_blocking(move || {
                    let mut last=std::time::Instant::now();
                    icaro_core::exportar::archivo(&store,&i,&tree,formato,&path,move |n,t| {
                        if n==0 || n>=t || last.elapsed()>=std::time::Duration::from_millis(80){last=std::time::Instant::now();let _=tx.unbounded_send(m(Accion::ExportarProgreso(n,t)));}
                    })
                });
                tokio::pin!(export);
                let r=loop {tokio::select! {r=&mut export=>break r.unwrap_or_else(|_|Err("No se pudo exportar la instancia.".into())),Some(a)=rx.next()=>{let _=output.send(a).await;}}};
                let _=output.send(m(Accion::Exportada(r))).await;
            }),|a|a);
        }
        Accion::ExportarArchivo(None) => {}
        Accion::ExportarProgreso(n,total) => {
            if let Some(s)=app.exportar.as_mut().filter(|s|s.guardando){s.hechos=n;s.total=total;}
        }
        Accion::MostrarExportacion(path) => {
            #[cfg(windows)]
            let result=std::process::Command::new("explorer.exe").arg(format!("/select,{}",path.display())).spawn().map(|_|());
            #[cfg(not(windows))]
            let result=path.parent().ok_or(()).and_then(|p|std::process::Command::new("xdg-open").arg(p).spawn().map(|_|()).map_err(|_|()));
            if result.is_err(){app.avisar("No se pudo mostrar el archivo en su carpeta.".into());}
        }
        Accion::AbrirListaEnlace(url) => {
            if url.starts_with("https://modrinth.com/mod/") {
                return crate::servicios::actualizar(app,crate::servicios::Accion::Enlace(url));
            }
        }
        // La dirección sale de `workspace.package.repository`, igual que la versión.
        Accion::AbrirRepositorio => {
            return crate::servicios::actualizar(app, crate::servicios::Accion::Enlace(env!("CARGO_PKG_REPOSITORY").into()));
        }
        // Cambiar de canal vuelve a comprobar: una descarga preparada de otro
        // canal se descarta al recibir el resultado.
        Accion::Canal(v) => {
            let canal = if v == "Beta" { icaro_core::actualizacion::Canal::Beta } else { icaro_core::actualizacion::Canal::Estable };
            if canal != app.aj.canal {
                app.aj.canal = canal;
                app.actualizacion.disponible = None;
                return crate::actualizacion_real::actualizar(app, crate::actualizacion_real::Accion::Comprobar);
            }
        }
        Accion::Exportada(result) => match result {
            Ok(path)=>{
                let size=std::fs::metadata(&path).map_or(0,|m|m.len());
                let name=app.exportar.as_ref().map(|s|s.nombre.clone()).unwrap_or_else(||"Instancia".into());
                app.red.historial.push(icaro_ui::descargas::EntradaHistorial{nombre:format!("{name} exportada"),destino:path.display().to_string(),tamano:exportacion_nueva::tam(size),fecha:"Ahora".into()});
                app.exportar=None;
                app.avisar_con(format!("{name} exportada · {}",exportacion_nueva::tam(size)),"Mostrar en carpeta",m(Accion::MostrarExportacion(path)));
            },
            Err(e)=>{if let Some(s)=app.exportar.as_mut(){s.guardando=false;s.error=Some(e);}}
        },
        // ---- Cuenta y errores
        Accion::CopiarCodigo => return iced::clipboard::write("F7K2-9QXM".to_owned()),
        Accion::AbrirPaginaMicrosoft => {
            let _ = std::process::Command::new("cmd")
                .args(["/C", "start", "", "https://www.microsoft.com/link"])
                .spawn();
        }
        Accion::PedirOtroCodigo => {
            app.codigo_inicio = Instant::now();
            app.codigo_fase = FaseCodigo::Esperando;
        }
        Accion::SimularConfirmacion => {
            app.codigo_fase = FaseCodigo::Confirmado(Instant::now());
            app.sesion.estado = icaro_ui::cuenta::EstadoSesion::Activa;
            app.sesion.banner_jugar = false;
        }
        Accion::JugarComoInvitado => {
            app.pantalla = None;
            app.sesion.estado = icaro_ui::cuenta::EstadoSesion::SinConexion;
            app.avisar("Sigues como invitado. Puedes iniciar sesión luego desde la cuenta.".into());
            arrancar(app);
        }
        Accion::Reintentar => app.intentos += 1,
        Accion::DesactivarYJugar => {
            app.pantalla = None;
            app.avisar("Iris Shaders se desactivó. Ya puedes jugar.".into());
            arrancar(app);
        }
        Accion::CopiarInforme => {
            return iced::clipboard::write(
                "Icaro Launcher · El juego se cerró (código 1) · Supervivencia\nCausa probable: Iris Shaders 1.8.1 no es compatible con Sodium 0.6.5"
                    .to_owned(),
            );
        }
        Accion::CambiarVersionLoader(i) => {return crate::instancias_reales::abrir_cambio_loader(app,i);}
        Accion::AplicarVersionLoader | Accion::CambiarLoaderVersion(_) => {}
        Accion::CambiarVersion(i) => {
            app.menu = None;
            app.cambiar = Some(i);
        }
        Accion::CambiarCancelar => {
            if !app.servicios.instancias.operando {
                app.cambiar = None;
                app.servicios.instancias.cambio = None;
            }
        }
        Accion::CambiarTipo(t) => {
            if let Some(c) = &mut app.servicios.instancias.cambio {
                c.tipo = t.min(2);
                c.destino.clear();
                c.revision = None;
            }
            app.servicios.instancias.cambio_offset = 0.;
            return iced::widget::scrollable::snap_to(
                iced::widget::scrollable::Id::new("version-cambio"),
                iced::widget::scrollable::RelativeOffset::START,
            );
        }
        Accion::CambiarBuscar(t) => {
            if let Some(c) = &mut app.servicios.instancias.cambio {
                c.busqueda = t;
            }
            app.servicios.instancias.cambio_offset = 0.;
            return iced::widget::scrollable::snap_to(
                iced::widget::scrollable::Id::new("version-cambio"),
                iced::widget::scrollable::RelativeOffset::START,
            );
        }
        Accion::CambiarDestino(index) => {
            let kind = app
                .servicios
                .instancias
                .cambio
                .as_ref()
                .map_or(0, |c| c.tipo);
            let destination = crate::instancias_reales::versiones(app, kind)
                .get(index)
                .map(|v| v.0.clone());
            if let Some(c) = &mut app.servicios.instancias.cambio {
                c.destino = destination.unwrap_or_default();
                c.revision = None;
                c.error = None;
            }
        }
        Accion::CambiarRespaldo(b) => {
            if let Some(c) = &mut app.servicios.instancias.cambio {
                c.respaldo = b;
            }
        }
        Accion::CambiarRevisar | Accion::CambiarCopia
        | Accion::CambiarActualizar(_) | Accion::CambiarModo(_)
        | Accion::CambiarDetalle | Accion::CambiarLoader(_) => {}
        Accion::CambiarConfirmar(i) => {
            app.cambiar = None;
            let nombre = instancias()[i].nombre.clone();
            let destino = app
                .servicios
                .instancias
                .cambio
                .as_ref()
                .map(|c| c.destino.clone())
                .unwrap_or_default();
            if let Some(d) = lista_instancias().lock().unwrap().get_mut(i) {
                let loader = d.version.split('·').nth(1).unwrap_or("Vanilla").trim();
                d.version = format!("{destino} · {loader}");
            }
            app.avisar(format!("Vista de diseño: {nombre} ahora usa {destino}."));
        }
        Accion::DetalleTecnico => app.detalle_fallo = !app.detalle_fallo,
        Accion::Duplicar(i) => {
            app.menu = None;
            let todas = instancias();
            if let Some(o) = todas.get(i) {
                let mut d = o.clone();
                d.nombre = nombre_unico(&format!("{} (copia)", o.nombre));
                d.estado = EstadoInstancia::Lista;
                d.detalle.clear();
                d.ultima_vez = "Nueva".into();
                let nombre = d.nombre.clone();
                agregar_instancia(app, d);
                app.avisar(format!("Se creó {nombre}."));
            }
        }
        Accion::AbrirCarpetaInstancia(i) => {
            app.menu = None;
            if !app.servicios.demo {
                if let Some(record) = app.servicios.instancias.registros.get(i) {
                    match app.servicios.instancias.almacen.juego(&record.id) {
                        Ok(dir) => {
                            if let Err(e) = abrir_en_sistema(&dir) {
                                app.avisar(e);
                            }
                        }
                        Err(e) => app.avisar(e.to_string()),
                    }
                }
                return Task::none();
            }
            if let Some(d) = instancias().get(i) {
                let dir = std::env::temp_dir().join("icaro-demo").join(&d.nombre);
                let _ = std::fs::create_dir_all(&dir);
                if let Err(e) = abrir_en_sistema(&dir) {
                    app.avisar(e);
                }
            }
        }
        Accion::VerInstalados => {
            app.pantalla = None;
            app.seccion = icaro_ui::shell::Seccion::Instancias;
            arrancar(app);
        }
    }
    Task::none()
}

pub fn abrir_en_sistema(ruta: &std::path::Path) -> Result<(), String> {
    if !ruta.is_dir() {
        return Err("La carpeta de la instancia no está disponible.".into());
    }
    let programa = if cfg!(target_os = "windows") {
        "explorer.exe"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    std::process::Command::new(programa)
        .arg(ruta)
        .spawn()
        .map(|_| ())
        .map_err(|_| "No se pudo abrir la carpeta en el explorador del sistema.".into())
}

fn m(a: Accion) -> Mensaje {
    Mensaje::Pant(a)
}

// ------------------------------------------------------------------ vistas

/// Mitad izquierda de una pantalla con lámina a toda altura.
pub(crate) fn con_lamina<'a>(p: Paleta, lamina: Lamina, derecha: Element<'a, Mensaje>) -> Element<'a, Mensaje> {
    // Como en el sistema: la lámina va sin marco, en una columna con una regla
    // de 1 px a la derecha; las figuras se muestran enteras y centradas.
    let entera = matches!(lamina, Lamina::Icaro | Lamina::Hermes | Lamina::Faeton | Lamina::HermesHilos | Lamina::ApoloBelvedere);
    let izquierda: Element<Mensaje> = if entera {
        container(icaro_ui::laminas::grabado_entero(p, lamina, Length::Fill, Length::Shrink))
            .padding(espacio::S8)
            .center_y(Length::Fill)
            .width(Length::FillPortion(2))
            .height(Length::Fill)
            .into()
    } else {
        container(icaro_ui::efectos::lupa(p, lamina))
            .width(Length::FillPortion(2))
            .height(Length::Fill)
            .into()
    };
    row![
        izquierda,
        container(Space::new(1.0, Length::Fill)).style(icaro_ui::estilo::bloque(p.border)),
        container(derecha)
            .width(Length::FillPortion(3))
            .height(Length::Fill)
            .center_y(Length::Fill)
            .padding(Padding::from([espacio::S12, espacio::S12])),
    ]
    .height(Length::Fill)
    .into()
}

pub fn vista_pantalla<'a>(app: &'a App, pantalla: Pantalla) -> Element<'a, Mensaje> {
    let p = app.paleta();
    match pantalla {
        Pantalla::Primera => primera(app, p),
        Pantalla::Codigo => codigo(app, p),
        Pantalla::Fallo => fallo(app, p),
        Pantalla::ErrorTienda => error_tienda(app, p),
        Pantalla::NoVerificados => crate::verificacion_real::vista(app),
        Pantalla::Sincronizar if !app.servicios.demo || app.servicios.sincronizacion.visual => crate::sincronizacion_real::vista(app),
        Pantalla::Sincronizar => {
            con_lamina(p, Lamina::Hidra, crate::desplazable_pub(sincronizar(p)))
        }
        Pantalla::Integridad => con_lamina(
            p,
            Lamina::Minerva,
            crate::desplazable_pub(integridad(app, p)),
        ),
        Pantalla::Actualizacion => crate::actualizacion_real::vista(app),
        Pantalla::Cuenta => crate::desplazable_pub(crate::sesion::vista_cuenta(app)),
        Pantalla::Estudio => crate::sesion::vista_estudio(app),
        Pantalla::Red if !app.servicios.demo || app.servicios.servidores.visual=>crate::red_privada_real::vista(app),
        Pantalla::Red => con_lamina(
            p,
            Lamina::HermesHilos,
            crate::desplazable_pub(red_privada(p)),
        ),
    }
}

fn primera<'a>(app: &'a App, p: Paleta) -> Element<'a, Mensaje> {
    if !app.servicios.demo {
        let disponible = !app.servicios.restoring && !app.servicios.auth_busy;
        return con_lamina(p, Lamina::Icaro, column![
            text("ICARO\nLAUNCHER").font(fuentes::DISPLAY).size(64.0).color(p.text),
            crate::servicios::welcome_status(app),
            row![
                componentes::boton(p, "Iniciar sesión", Variante::Primario, disponible.then(|| m(Accion::Abrir(Pantalla::Codigo)))),
                componentes::boton(p, "Usar cuenta sin conexión", Variante::Fantasma, disponible.then(|| m(Accion::JugarComoInvitado))),
            ].spacing(espacio::S3).wrap(),
        ].spacing(espacio::S4).width(Length::Fill).into());
    }
    let paso = |n: &str, titulo: &str, ayuda: &str, activo: bool| -> Element<'a, Mensaje> {
        row![
            container(Space::new(Length::Fixed(3.0), Length::Fill))
                .style(icaro_ui::estilo::bloque(if activo { p.text } else { p.border })),
            column![
                text(n.to_owned()).font(fuentes::MONO).size(texto::MONO_SM.0).color(p.text_muted),
                text(titulo.to_owned()).font(fuentes::TITULO).size(texto::BODY.0 + 2.0).color(p.text),
                text(ayuda.to_owned()).size(texto::BODY_SM.0).color(p.text_muted),
            ]
            .spacing(espacio::S1),
        ]
        .spacing(espacio::S4)
        .height(Length::Shrink)
        .into()
    };
    let derecha = column![
        text("ICARO\nLAUNCHER").font(fuentes::DISPLAY).size(64.0).color(p.text),
        text("Vamos a dejar todo listo en tres pasos. Después, solo queda pulsar Jugar.")
            .size(texto::BODY.0)
            .color(p.text_muted),
        Space::with_height(espacio::S4),
        paso("1", "Inicia sesión", "Con tu cuenta Microsoft o una cuenta sin conexión.", true),
        paso("2", "Revisamos Java", "Si falta, Icaro Launcher lo descarga.", false),
        paso("3", "Tu primera instancia", "Vanilla, Fabric o un modpack.", false),
        Space::with_height(espacio::S4),
        row![
            componentes::boton(p, "Iniciar sesión", Variante::Primario, Some(m(Accion::Abrir(Pantalla::Codigo)))),
            componentes::boton(p, "Usar cuenta sin conexión", Variante::Fantasma, Some(m(Accion::JugarComoInvitado))),
        ]
        .spacing(espacio::S3),
    ]
    .spacing(espacio::S4);
    con_lamina(p, Lamina::Icaro, derecha.into())
}

fn codigo<'a>(app: &'a App, p: Paleta) -> Element<'a, Mensaje> {
    if !app.servicios.demo { return crate::servicios::login_view(app); }
    let restante = 900u64.saturating_sub(app.ahora.saturating_duration_since(app.codigo_inicio).as_secs());
    let fase = match app.codigo_fase {
        FaseCodigo::Esperando if restante == 0 => FaseCodigo::Vencido,
        f => f,
    };
    let vencido = fase == FaseCodigo::Vencido;
    let celdas = icaro_ui::cuenta::codigo(p, "F7K2-9QXM", vencido);
    let mut col = column![
        text("INICIA SESIÓN\nCON MICROSOFT").font(fuentes::DISPLAY).size(texto::DISPLAY.0).color(p.text),
        text("Tu cuenta de Minecraft se confirma en el navegador. Icaro Launcher nunca ve tu contraseña.")
            .size(texto::BODY.0)
            .color(p.text_muted),
        Space::with_height(espacio::S2),
        text("1  Abre microsoft.com/link").font(fuentes::TITULO).size(texto::BODY.0).color(p.text),
        row![componentes::boton(p, "Abrir página", Variante::Secundario, Some(m(Accion::AbrirPaginaMicrosoft)))],
        text("2  Escribe este código").font(fuentes::TITULO).size(texto::BODY.0).color(p.text),
        celdas,
    ]
    .spacing(espacio::S3);
    match fase {
        FaseCodigo::Vencido => {
            col = col
                .push(text("El código venció a los 15 minutos. Pide otro para seguir.").size(texto::BODY_SM.0).color(p.warning))
                .push(row![icaro_ui::cuenta::boton_icono(
                    p,
                    Some(Icono::Refrescar),
                    "Pedir otro código",
                    icaro_ui::cuenta::Tipo::Primario,
                    icaro_ui::tema::medida::CONTROL_SM,
                    Some(m(Accion::PedirOtroCodigo)),
                )]);
        }
        FaseCodigo::Confirmado(t) => {
            col = col.push(
                container(
                    row![
                        icono(Icono::Check, Tam::Base, p.success),
                        column![
                            text("Mineral_7").font(fuentes::TITULO).size(texto::BODY.0).color(p.text),
                            text("Cuenta Microsoft confirmada").size(texto::BODY_SM.0).color(p.text_muted),
                        ],
                    ]
                    .spacing(espacio::S3)
                    .align_y(Alignment::Center),
                )
                .padding(espacio::S4)
                .style(icaro_ui::estilo::tarjeta(p, false)),
            );
            let _ = t;
        }
        FaseCodigo::Esperando => {
            col = col
                .push(row![
                    componentes::boton(p, "Copiar código", Variante::Secundario, Some(m(Accion::CopiarCodigo))),
                    componentes::boton(p, "Ya lo confirmé", Variante::Fantasma, Some(m(Accion::SimularConfirmacion))),
                ].spacing(espacio::S3))
                .push(
                    row![
                        icono(Icono::Espera, Tam::Base, p.text_muted),
                        text("Esperando que confirmes en el navegador").size(texto::BODY_SM.0).color(p.text),
                        Space::with_width(Length::Fill),
                        text(format!("Vence en {:02}:{:02}", restante / 60, restante % 60))
                            .font(fuentes::MONO)
                            .size(texto::MONO_SM.0)
                            .color(p.text_muted),
                    ]
                    .spacing(espacio::S3)
                    .align_y(Alignment::Center),
                );
        }
    }
    col = col.push(row![componentes::boton(p, "Volver", Variante::Fantasma, Some(m(Accion::Cerrar)))]);
    con_lamina(p, Lamina::Hermes, col.into())
}

fn error_tienda<'a>(app: &'a App, p: Paleta) -> Element<'a, Mensaje> {
    let causa = if app.intentos >= 3 {
        "Sigue sin responder. Revisa tu conexión a internet y vuelve a intentarlo."
    } else {
        "Modrinth no responde. Tus instancias y mods instalados siguen funcionando; la búsqueda volverá cuando el servicio responda."
    };
    let derecha = column![
        text("LA TIENDA\nNO CARGÓ").font(fuentes::DISPLAY).size(texto::DISPLAY.0).color(p.text),
        text(causa).size(texto::BODY.0).color(p.text_muted),
        row![
            componentes::boton(p, "Reintentar", Variante::Primario, Some(m(Accion::Reintentar))),
            componentes::boton(p, "Ver mods instalados", Variante::Secundario, Some(m(Accion::VerInstalados))),
        ]
        .spacing(espacio::S3),
        text(format!("api.modrinth.com · HTTP 503 · {} intentos", 3 + app.intentos))
            .font(fuentes::MONO)
            .size(texto::MONO_SM.0)
            .color(p.text_muted),
        text("Faetón, Hendrick Goltzius, 1588. The Met, CC0.").size(texto::BODY_SM.0).color(p.text_muted),
    ]
    .spacing(espacio::S4);
    con_lamina(p, Lamina::Faeton, derecha.into())
}

fn fallo<'a>(app: &'a App, p: Paleta) -> Element<'a, Mensaje> {
    if !app.servicios.demo {return crate::juego_real::fallo(app);}
    let causa = container(
        row![
            icono(Icono::Alerta, Tam::Base, p.error),
            column![
                text("Causa probable: Iris Shaders 1.8.1 no es compatible con Sodium 0.6.5")
                    .font(fuentes::TITULO)
                    .size(texto::BODY.0 + 1.0)
                    .color(p.text),
                text("Iris necesita Sodium 0.6.3 o anterior. Desactívalo para jugar ahora, o instala Iris 1.8.2 cuando esté disponible.")
                    .size(texto::BODY_SM.0)
                    .color(p.text_muted),
            ]
            .spacing(espacio::S1),
        ]
        .spacing(espacio::S3),
    )
    .padding(espacio::S4)
    .width(Length::Fill)
    .style(move |_: &iced::Theme| iced::widget::container::Style {
        background: Some(iced::Background::Color(p.error_soft)),
        border: iced::Border { color: p.error, width: 2.0, radius: 0.0.into() },
        ..Default::default()
    });
    let mut col = column![
        text("EL JUEGO\nSE CERRÓ").font(fuentes::DISPLAY).size(texto::DISPLAY.0).color(p.text),
        text("Código de salida 1 · 14:02:47 · Supervivencia").font(fuentes::MONO).size(texto::MONO_SM.0).color(p.text_muted),
        causa,
        row![
            componentes::boton(p, "Desactivar Iris y jugar", Variante::Primario, Some(m(Accion::DesactivarYJugar))),
            componentes::boton(p, "Abrir registro", Variante::Secundario, Some(Mensaje::Ir(icaro_ui::shell::Seccion::Consola))),
            componentes::boton(p, "Copiar informe", Variante::Fantasma, Some(m(Accion::CopiarInforme))),
        ]
        .spacing(espacio::S3),
        iced::widget::button(
            row![
                text("Detalle técnico").font(fuentes::TITULO).size(texto::BODY.0).width(Length::Fill),
                icaro_ui::iconos::icono_texto(if app.detalle_fallo { Icono::Flecha } else { Icono::ChevronAbajo }, Tam::Base),
            ]
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .on_press(m(Accion::DetalleTecnico))
        .style(icaro_ui::estilo::boton_fantasma(p)),
    ]
    .spacing(espacio::S4);
    if app.detalle_fallo {
        col = col.push(
            container(
                text("java.lang.NoSuchMethodError: 'void net.caffeinemc.mods.sodium.client.render.chunk.ChunkRenderMatrices.<init>'\n  at net.irisshaders.iris.compat.sodium.mixin.MixinChunkRenderer.iris$setup(MixinChunkRenderer.java:58)\n  at net.minecraft.client.renderer.LevelRenderer.renderLevel(LevelRenderer.java:1204)")
                    .font(fuentes::MONO)
                    .size(texto::MONO_SM.0)
                    .color(p.text),
            )
            .padding(espacio::S4)
            .width(Length::Fill)
            .style(icaro_ui::estilo::marco(p)),
        );
    }
    con_lamina(p, Lamina::Rostro, crate::desplazable_pub(col.into()))
}

// ------------------------------------------------------------- diálogos

fn cabecera_pasos<'a>(p: Paleta, actual: usize) -> Element<'a, Mensaje> {
    let mut fila = row![].spacing(0);
    for (i, nombre) in PASOS.iter().enumerate() {
        let hecho = i < actual;
        let tinta = if i <= actual { p.text } else { p.text_muted };
        let contenido = column![
            container(Space::new(Length::Fill, 3.0))
                .style(icaro_ui::estilo::bloque(if i <= actual { p.text } else { p.border })),
            text(format!("{} de 4", i + 1)).font(fuentes::MONO).size(texto::MONO_SM.0).color(p.text_muted),
            text((*nombre).to_owned()).font(fuentes::TITULO).size(texto::BODY.0).color(tinta),
        ]
        .spacing(espacio::S1)
        .width(Length::Fill);
        fila = fila.push(if hecho {
            iced::widget::button(contenido)
                .padding(0)
                .width(Length::FillPortion(1))
                .on_press(m(Accion::AsistentePaso(i)))
                .style(icaro_ui::estilo::sin_estilo(p.text))
                .into()
        } else {
            Element::from(container(contenido).width(Length::FillPortion(1)))
        });
    }
    fila.into()
}

/// Dialogo y pie de AccionesInstancia/Asistente, sin agregar otra composición.
pub(crate) fn formulario_icono<'a>(p: Paleta, titulo: &str, cuerpo: Element<'a, Mensaje>, pie: Element<'a, Mensaje>, ancho: f32, alto: f32, cerrar: Mensaje) -> Element<'a, Mensaje> {
    formulario(p,titulo,cuerpo,pie,ancho,alto,cerrar)
}
fn formulario<'a>(p: Paleta, titulo: &str, cuerpo: Element<'a, Mensaje>, pie: Element<'a, Mensaje>, ancho: f32, alto: f32, cerrar: Mensaje) -> Element<'a, Mensaje> {
    let cabeza = row![text(titulo.to_owned()).font(fuentes::TITULO).size(texto::TITLE.0).color(p.text), Space::with_width(Length::Fill), iced::widget::button(icaro_ui::iconos::icono_texto(Icono::Cerrar, Tam::Base)).padding(espacio::S2).on_press(cerrar).style(icaro_ui::estilo::boton_fantasma(p))].align_y(Alignment::Center);
    container(container(column![container(cabeza).padding(Padding::from([espacio::S4, espacio::S6])), container(cuerpo).padding(Padding::from([espacio::S2, espacio::S6])).height(Length::Fill), container(Space::new(Length::Fill, 1)).style(icaro_ui::estilo::bloque(p.border)), container(pie).padding(Padding::from([espacio::S4, espacio::S6]))]).padding(2)).width(ancho).height(alto).style(icaro_ui::estilo::flotante(p)).into()
}

fn selector_version_completo<'a>(
    app: &'a App,
    tipo: usize,
    busqueda: &'a str,
    seleccion: &str,
    cambio: bool,
) -> Element<'a, Mensaje> {
    let p = app.paleta();
    let versiones = crate::instancias_reales::versiones(app, tipo);
    // Las tarjetas ya guardan el estado tras instalar. Leer y comprobar todos
    // los archivos de Minecraft durante cada frame bloqueaba el selector.
    let tarjetas = crate::instancias();
    let instaladas: std::collections::BTreeSet<&str> = app.servicios.instancias.registros
        .iter().zip(tarjetas.iter())
        .filter(|(_, card)| matches!(card.estado,
            icaro_ui::instancias::EstadoInstancia::Lista |
            icaro_ui::instancias::EstadoInstancia::Jugando |
            icaro_ui::instancias::EstadoInstancia::Desactualizada))
        .map(|(instance, _)| instance.minecraft.as_str()).collect();
    let total = versiones.len();
    let consulta = busqueda.trim().to_lowercase();
    let filtradas: Vec<_> = versiones
        .into_iter()
        .enumerate()
        .filter(|(_, v)| consulta.is_empty() || v.0.to_lowercase().contains(&consulta))
        .collect();
    let cantidad = filtradas.len();
    let offset = if cambio {
        app.servicios.instancias.cambio_offset
    } else {
        app.servicios.instancias.version_offset
    };
    let first = ((offset / 36.0) as usize)
        .saturating_sub(2)
        .min(cantidad.saturating_sub(1));
    let end = (first + 14).min(cantidad);
    let mut rows = column![Space::with_height(first as f32 * 36.0)];
    for (pos, (index, (version, fecha))) in filtradas
        .into_iter()
        .enumerate()
        .skip(first)
        .take(end - first)
    {
        let selected = version == seleccion;
        let installed = instaladas.contains(version.as_str());
        let badge = if tipo == 0 && index == 0 {
            Some("Recomendada")
        } else if installed {
            Some("Instalada")
        } else if tipo == 1 {
            Some("Snapshot")
        } else if tipo == 2 {
            Some(if version.starts_with('a') {
                "Alpha"
            } else {
                "Beta"
            })
        } else {
            None
        };
        let color = if selected { p.bg } else { p.text };
        let mut row = row![text(version)
            .font(fuentes::MONO)
            .size(texto::MONO.0)
            .color(color)]
        .spacing(espacio::S3)
        .align_y(Alignment::Center);
        if let Some(label) = badge {
            row = row.push(
                container(
                    text(label.to_uppercase())
                        .font(fuentes::TITULO)
                        .size(texto::LABEL.0)
                        .color(if selected { p.text } else { p.text }),
                )
                .padding([2., 6.])
                .style(move |_| iced::widget::container::Style {
                    background: selected.then_some(iced::Background::Color(p.bg)),
                    border: iced::Border {
                        color: if selected { p.bg } else { p.border },
                        width: 1.,
                        radius: 0.into(),
                    },
                    ..Default::default()
                }),
            );
        }
        row = row.push(Space::with_width(Length::Fill)).push(
            text(fecha)
                .font(fuentes::MONO)
                .size(texto::MONO_SM.0)
                .color(if selected { p.bg } else { p.text_muted }),
        );
        let _ = pos;
        rows = rows.push(
            iced::widget::button(container(row).center_y(Length::Fill))
                .height(36)
                .width(Length::Fill)
                .padding([0., espacio::S3])
                .on_press(m(if cambio {
                    Accion::CambiarDestino(index)
                } else {
                    Accion::AsistenteVersion(index)
                }))
                .style(move |_, status| iced::widget::button::Style {
                    background: (selected
                        || matches!(status, iced::widget::button::Status::Hovered))
                    .then_some(iced::Background::Color(if selected {
                        p.text
                    } else {
                        p.surface_hover
                    })),
                    text_color: color,
                    ..Default::default()
                }),
        );
    }
    rows = rows.push(Space::with_height((cantidad - end) as f32 * 36.0));
    let lista = iced::widget::scrollable(rows)
        .id(iced::widget::scrollable::Id::new(if cambio {
            "version-cambio"
        } else {
            "version-asistente"
        }))
        .height(252)
        .on_scroll(move |v| m(Accion::VersionScroll(cambio, v.absolute_offset().y)));
    let filtros = row![
        componentes::segmentado_compacto(
            p,
            &["Estables", "Snapshots", "Antiguas"],
            tipo,
            move |i| m(if cambio {
                Accion::CambiarTipo(i)
            } else {
                Accion::AsistenteTipo(i)
            })
        ),
        Space::with_width(Length::Fill),
        container(componentes::campo_con_icono(
            p,
            Icono::Buscar,
            "Buscar versión",
            busqueda,
            move |s| m(if cambio {
                Accion::CambiarBuscar(s)
            } else {
                Accion::AsistenteBuscar(s)
            })
        ))
        .width(240)
    ]
    .spacing(espacio::S3)
    .align_y(Alignment::Center);
    let status = if app.servicios.instancias.cargando {
        "Obteniendo versiones de Minecraft…".into()
    } else if let Some(e) = &app.servicios.instancias.error {
        e.clone()
    } else {
        format!(
            "{total} {} · manifiesto guardado en caché",
            ["estables", "snapshots", "antiguas"][tipo.min(2)]
        )
    };
    column![
        filtros,
        container(lista)
            .padding(2)
            .width(Length::Fill)
            .style(icaro_ui::estilo::marco(p)),
        text(status)
            .font(fuentes::MONO)
            .size(texto::MONO_SM.0)
            .color(p.text_muted)
    ]
    .spacing(espacio::S4)
    .into()
}

#[cfg(test)]
mod pruebas_scroll_version {
    use super::*;
    use iced::advanced::{layout, mouse, widget::Tree, Clipboard, Layout, Shell};
    use iced::{Event, Point, Rectangle, Size};

    #[test]
    fn rueda_del_asistente_desplaza_la_lista_de_versiones() {
        let (mut app,_) = crate::iniciar_con_guardado(icaro_ui::shell::Seccion::Instancias,false,None,false,crate::servicios::Estado::nuevo(true),None);
        app.servicios.demo=false;
        app.servicios.instancias.versiones=(0..100).map(|n|icaro_core::manifest::VersionEntry {
            id:format!("1.21.{n}"),kind:"release".into(),url:String::new(),sha1:String::new(),release_time:String::new()
        }).collect();
        let state=Asistente {paso:0,tipo_version:0,busqueda:String::new(),version:0,loader:0,nombre:String::new(),portada:0};
        let renderer=iced::Renderer::Secondary(iced_tiny_skia::Renderer::new(iced::Font::DEFAULT,16.into()));
        let mut widget=asistente(&app,&state);let mut tree=Tree::new(widget.as_widget());
        let size=Size::new(1000.,750.);
        let node=widget.as_widget().layout(&mut tree,&renderer,&layout::Limits::new(Size::ZERO,size));
        fn buscar(layout:Layout<'_>)->Option<Rectangle> {
            if (layout.bounds().height-252.).abs()<0.1 && layout.bounds().width>200. {return Some(layout.bounds());}
            layout.children().find_map(buscar)
        }
        let bounds=buscar(Layout::new(&node)).expect("Lista de versiones visible");
        let mut messages=vec![];let mut clipboard=iced::advanced::clipboard::Null;
        let _:&mut dyn Clipboard=&mut clipboard;
        widget.as_widget_mut().on_event(&mut tree,
            Event::Mouse(mouse::Event::WheelScrolled {delta:mouse::ScrollDelta::Lines{x:0.,y:-3.}}),
            Layout::new(&node),mouse::Cursor::Available(bounds.center()),&renderer,&mut clipboard,
            &mut Shell::new(&mut messages),&Rectangle::new(Point::ORIGIN,size));
        assert!(messages.iter().any(|m|matches!(m,Mensaje::Pant(Accion::VersionScroll(false,y)) if *y>0.)),"La rueda debe llegar a la lista interior");
    }
}

fn selector_loader_completo<'a>(
    app: &'a App,
    s: &Asistente,
    minecraft: &str,
) -> Element<'a, Mensaje> {
    let p = app.paleta();
    let catalog = app.servicios.instancias.loaders.get(minecraft);
    let tarjeta = |index: usize| -> Element<'a, Mensaje> {
        let (nombre, ayuda, muestra) = LOADERS[index];
        let versions = if app.servicios.demo {
            if index == 4 {
                vec![]
            } else if index == 0 {
                vec![]
            } else {
                vec![muestra.to_owned()]
            }
        } else {
            catalog.map_or(vec![], |c| c.versiones[index].clone())
        };
        let error = catalog.and_then(|c| c.errores[index].as_ref());
        let busy = app.servicios.instancias.cargando_loaders.as_deref() == Some(minecraft);
        let disabled = index > 0 && versions.is_empty();
        let selected = s.loader == index;
        let ayuda = if disabled {
            if busy {
                "Comprobando versiones disponibles…".into()
            } else if let Some(e) = error {
                e.clone()
            } else {
                format!("Aún no disponible para {minecraft}.")
            }
        } else {
            ayuda.to_owned()
        };
        let mut contenido = column![
            text(nombre)
                .font(fuentes::TITULO)
                .size(texto::BODY.0 + 1.)
                .color(if disabled { p.text_disabled } else { p.text }),
            text(ayuda).size(texto::BODY_SM.0).color(p.text_muted)
        ]
        .spacing(espacio::S1)
        .width(Length::Fill);
        if selected && index > 0 && !disabled {
            let version = app
                .servicios
                .instancias
                .loader_elegida
                .clone()
                .filter(|v| versions.contains(v))
                .or_else(|| versions.first().cloned());
            let selector = iced::widget::pick_list(versions.clone(), version, |v| {
                m(Accion::AsistenteLoaderVersion(v))
            })
            .font(fuentes::MONO)
            .text_size(texto::MONO_SM.0)
            .padding([4., 8.])
            .style(icaro_ui::estilo::selector(p))
            .menu_style(icaro_ui::estilo::menu_selector(p));
            contenido = contenido.push(
                row![etiqueta(p, "Versión"), selector]
                    .spacing(espacio::S2)
                    .align_y(Alignment::Center),
            );
        }
        let row = row![
            componentes::rombo(p, selected, disabled),
            contenido,
        ]
        .spacing(espacio::S4)
        .align_y(Alignment::Center);
        let button = iced::widget::button(container(row).padding([espacio::S4, espacio::S5]))
            .width(Length::Fill)
            .padding(0)
            .on_press_maybe((!disabled).then(|| m(Accion::AsistenteLoader(index))))
            .style(icaro_ui::estilo::sin_estilo(p.text));
        container(button)
            .width(Length::Fill)
            .style(icaro_ui::estilo::tarjeta(p, selected))
            .into()
    };
    column![
        row![tarjeta(0), tarjeta(1)].spacing(espacio::S3),
        row![tarjeta(2), tarjeta(3)].spacing(espacio::S3),
        tarjeta(4)
    ]
    .spacing(espacio::S3)
    .into()
}

pub fn asistente<'a>(app: &'a App, s: &'a Asistente) -> Element<'a, Mensaje> {
    let p = app.paleta();
    let versiones = crate::instancias_reales::versiones(app, s.tipo_version);
    let version = versiones.get(s.version).cloned().unwrap_or_default();
    let selector_version =
        || selector_version_completo(app, s.tipo_version, &s.busqueda, &version.0, false);
    let selector_loader = || selector_loader_completo(app, s, &version.0);
    let cuerpo: Element<Mensaje> = match s.paso {
        0 => selector_version(),
        1 => selector_loader(),
        2 => {
            let mut portadas = row![].spacing(espacio::S3);
            for (i, (nombre, lamina)) in PORTADAS.iter().enumerate() {
                portadas = portadas.push(
                    iced::widget::button(
                        column![
                            container(grabado(p, *lamina, Length::Fill, 70))
                                .style(icaro_ui::estilo::marco(p)),
                            text((*nombre).to_owned())
                                .size(texto::BODY_SM.0)
                                .color(if s.portada == i { p.text } else { p.text_muted }),
                        ]
                        .spacing(espacio::S1),
                    )
                    .padding(0)
                    .width(Length::FillPortion(1))
                    .on_press(m(Accion::AsistentePortada(i)))
                    .style(icaro_ui::estilo::sin_estilo(p.text)),
                );
            }
            column![
                etiqueta(p, "Nombre de la instancia"),
                componentes::campo(p, "Mi instancia", &s.nombre, |t| m(
                    Accion::AsistenteNombre(t)
                )),
                etiqueta(p, "Portada"),
                portadas,
            ]
            .spacing(espacio::S3)
            .into()
        }
        _ => {
            let fila = |k: &str, v: String| -> Element<'a, Mensaje> {
                row![
                    text(k.to_owned())
                        .size(texto::BODY.0)
                        .color(p.text_muted)
                        .width(160),
                    text(v)
                        .font(fuentes::MONO)
                        .size(texto::MONO.0)
                        .color(p.text),
                ]
                .into()
            };
            column![
                fila(
                    "Nombre",
                    if s.nombre.trim().is_empty() {
                        "Nueva instancia".into()
                    } else {
                        s.nombre.trim().to_owned()
                    }
                ),
                fila("Minecraft", version.0.to_owned()),
                fila("Loader", LOADERS[s.loader].0.to_owned()),
                fila("Java", "Automático".into()),
                fila("Memoria", "2.048 a 6.144 MB".into()),
                text("Todo esto se puede cambiar después desde el editor de la instancia.")
                    .size(texto::BODY_SM.0)
                    .color(p.text_muted),
            ]
            .spacing(espacio::S3)
            .into()
        }
    };
    let cuerpo = column![cabecera_pasos(p, s.paso), cuerpo].spacing(espacio::S4);
    let ocupado = !app.servicios.demo && app.servicios.instancias.creando;
    let loader_disponible=app.servicios.demo||s.paso!=1||s.loader==0||app.servicios.instancias.loaders.get(&version.0).is_some_and(|l|!l.versiones[s.loader.min(4)].is_empty());
    let mut acciones: Vec<Element<Mensaje>> = vec![container(componentes::boton(
        p,
        "Cancelar",
        Variante::Fantasma,
        (!ocupado).then(|| m(Accion::AsistenteCancelar)),
    ))
    .width(Length::Fill)
    .into()];
    if s.paso > 0 {
        acciones.push(componentes::boton_con_icono(
            p,
            Icono::Atras,
            "Atrás",
            Variante::Secundario,
            (!ocupado).then(|| m(Accion::AsistenteAtras)),
            false,
        ));
    }
    acciones.push(if s.paso == 3 {
        componentes::boton(
            p,
            if ocupado {
                "Guardando instancia…"
            } else {
                "Crear instancia"
            },
            Variante::Primario,
            (!ocupado).then(|| m(Accion::AsistenteCrear)),
        )
    } else {
        componentes::boton_con_icono(
            p,
            Icono::Adelante,
            "Siguiente",
            Variante::Primario,
            (!ocupado && loader_disponible && (app.servicios.demo || !version.0.is_empty()))
                .then(|| m(Accion::AsistenteSiguiente)),
            false,
        )
    });
    formulario(
        p,
        "Crear instancia",
        // El scroll nativo entrega la rueda al selector interior antes de mover el formulario.
        iced::widget::scrollable(cuerpo).height(Length::Fill).into(),
        row(acciones).spacing(espacio::S2).into(),
        880.0,
        610.0_f32.min(app.ventana.height - 48.),
        m(Accion::AsistenteCancelar),
    )
}

pub fn importar<'a>(app: &'a App, s: &'a Importar) -> Element<'a, Mensaje> {
    let p = app.paleta();
    let cuerpo: Element<Mensaje> = match s.origen {
        0 => {
            let contenido: Element<Mensaje> = match s.archivo {
                None => column![
                    text("Elige un archivo .mrpack o el .zip de un modpack de CurseForge.").size(texto::BODY.0).color(p.text_muted),
                    row![componentes::boton(p, "Examinar", Variante::Secundario, Some(m(Accion::ImportarArchivo)))],
                ]
                .spacing(espacio::S3)
                .into(),
                Some(nombre) => column![
                    text(nombre).font(fuentes::MONO).size(texto::MONO.0).color(p.text),
                    text("Fabulously Optimized · 1.21.4 · Fabric · 62 mods").size(texto::BODY.0).color(p.text_muted),
                    insignia(p, Estado::Exito, "Listo para importar"),
                ]
                .spacing(espacio::S2)
                .into(),
            };
            container(contenido).padding(espacio::S4).width(Length::Fill).style(icaro_ui::estilo::marco(p)).into()
        }
        1 => {
            let mut lista = column![text("Encontramos estas carpetas en tu equipo:").size(texto::BODY.0).color(p.text_muted)].spacing(espacio::S2);
            for (i, (n, c)) in [("Prism Launcher", "1 instancia"), ("MultiMC", "1 instancia"), ("CurseForge App", "1 instancia")].iter().enumerate() {
                lista = lista.push(casilla(p, &format!("{n} · {c}"), s.launchers[i], move |_| m(Accion::ImportarLauncher(i))));
            }
            lista.into()
        }
        _ => {
            let todas = instancias();
            let nombres: Vec<String> = todas.iter().map(|d| d.nombre.clone()).collect();
            let origen = todas.get(s.clon_origen).map_or(String::new(), |d| d.nombre.clone());
            column![
                etiqueta(p, "Instancia a clonar"),
                componentes::selector(p, nombres, Some(origen.clone()), |n| m(Accion::ImportarClon(n))),
                casilla(p, "Copiar también los mundos", s.con_mundos, |v| m(Accion::ImportarMundos(v))),
                text(format!("Se creará «{origen} (copia)».")).size(texto::BODY_SM.0).color(p.text_muted),
            ]
            .spacing(espacio::S3)
            .into()
        }
    };
    let cuerpo = column![
        componentes::segmentado(p, &["Modpack", "Otro launcher", "Clonar"], s.origen, |i| m(Accion::ImportarOrigen(i))),
        cuerpo,
    ]
    .spacing(espacio::S4);
    let listo = match s.origen {
        0 => s.archivo.is_some(),
        1 => s.launchers.iter().any(|x| *x),
        _ => true,
    };
    icaro_ui::superficies::dialogo(
        p,
        "Importar instancia",
        cuerpo.into(),
        vec![
            componentes::boton(p, "Cancelar", Variante::Fantasma, Some(m(Accion::ImportarCancelar))),
            componentes::boton(p, "Importar", Variante::Primario, listo.then(|| m(Accion::ImportarConfirmar))),
        ],
        None,
        620.0,
        420.0,
        m(Accion::ImportarCancelar),
    )
}

pub fn exportar<'a>(app: &'a App, s: &'a Exportar) -> Element<'a, Mensaje> {
    exportacion_nueva::vista(app,s)
}

// ------------------------------------------------------------- ajustes

pub const SECCIONES: [&str; 8] = [
    "General", "Apariencia", "Java y memoria", "Almacenamiento", "Red", "Comportamiento", "Atajos", "Acerca de",
];

pub fn pagina_ajustes<'a>(app: &'a App) -> Element<'a, Mensaje> {
    let p = app.paleta();
    let a = &app.aj;
    let ajustes = app.ajuste.min(SECCIONES.len() - 1);
    let titulo = SECCIONES[ajustes];
    let interruptor = |v: bool, c: CampoAjuste| componentes::interruptor(p, v, m(Accion::Alternar(c)));
    let mut col = column![encabezado_ajustes(p, titulo, None)].spacing(espacio::S2).width(Length::Fill);
    match ajustes {
        0 => {
            col = col
                .push(fila_ajuste(p, "Idioma", "El idioma de la interfaz.",
                    componentes::selector(p, vec!["Español (Chile)".to_owned(), "English".to_owned()], Some(a.idioma.clone()), |v| m(Accion::Idioma(v)))))
                .push(fila_ajuste(p, "Abrir con el sistema", "Icaro Launcher se inicia al encender el equipo.", interruptor(a.abrir_al_iniciar, CampoAjuste::AbrirAlIniciar)))
                .push(fila_ajuste(p, "Iniciar minimizado", "Arranca sin mostrar la ventana.", interruptor(a.iniciar_minimizado, CampoAjuste::IniciarMinimizado)))
                .push(fila_ajuste(p, "Al iniciar el juego", "Qué hace el launcher cuando Minecraft abre.",
                    componentes::selector(p, vec!["Mantener abierto".to_owned(), "Minimizar".to_owned(), "Cerrar el launcher".to_owned()],
                        Some(["Mantener abierto", "Minimizar", "Cerrar el launcher"][a.al_jugar.min(2)].to_owned()), |v| m(Accion::AlJugar(v)))))
                .push(fila_ajuste(p, "Carpeta de datos", "Instancias, versiones y caché.",
                    componentes::boton(p, "Abrir carpeta", Variante::Secundario, Some(m(Accion::AbrirCarpetaDatos)))));
        }
        1 => {
            col = col.push(
                componentes::apariencia(p, app.tema, a.metal, a.escala,
                    Mensaje::Tema, |i| m(Accion::Metal(i)), |n| m(Accion::Escala(n))),
            ).push(Space::with_height(espacio::S6))
             .push(fila_ajuste(p, "Reducir movimiento", "Quita animaciones; el Umbral pasa a ser un fundido.",
                    componentes::interruptor(p, app.reducir, Mensaje::Reducir)));
        }
        2 => {
            col = col
                .push(fila_ajuste(p, "Java de cada instancia", "La versión y la memoria se eligen por instancia, en la pestaña Java de su editor.", Space::with_width(0).into()))
                .push(fila_ajuste(p, "Descarga automática de Java", "Activada: usa el runtime administrado del perfil y lo descarga si falta. Desactivada: detecta Java compatible disponible.", componentes::interruptor(p, app.java_global.descarga, Mensaje::Java(crate::java_real::Accion::DescargaGlobal(!app.java_global.descarga)))))
                .push(fila_ajuste(p, "Java global", "Ruta vacía: Automático. Las instancias pueden heredar esta selección.", componentes::campo(p, "Automático", &icaro_ui::editor::ruta_java_visible(app.java_global.ruta.as_deref().unwrap_or("")), |v| Mensaje::Java(crate::java_real::Accion::RutaGlobal(v)))))
                .push(row![componentes::boton(p, "Agregar ruta", componentes::Variante::Fantasma, Some(Mensaje::Java(crate::java_real::Accion::Agregar(true)))), componentes::boton(p, "Detectar instalaciones", componentes::Variante::Secundario, Some(Mensaje::Java(crate::java_real::Accion::Detectar)))].spacing(espacio::S2))
                .push(text(app.java_estado.clone()).size(texto::BODY_SM.0).color(p.text_muted))
                .push(text("Instalaciones detectadas").font(fuentes::TITULO).size(texto::BODY.0 + 1.0).color(p.text));
            let installations = if app.servicios.demo { vec![("21.0.5".into(), "Temurin · Icaro Launcher".into(), "…/icaro/java/21".into()), ("17.0.13".into(), "Temurin · Icaro Launcher".into(), "…/icaro/java/17".into())] } else { crate::juego_real::instalaciones_java(app).into_iter().map(|i| (i.version,i.proveedor,i.ruta)).collect() };
            for (v, prov, ruta) in installations {
                col = col.push(
                    container(
                        row![
                            text(format!("Java {v}")).font(fuentes::MONO).size(texto::MONO.0).color(p.text).width(140),
                            text(prov).size(texto::BODY.0).color(p.text_muted).width(200),
                            text(icaro_ui::editor::ruta_java_visible(&ruta)).font(fuentes::MONO).size(texto::MONO_SM.0).color(p.text_muted),
                        ]
                        .spacing(espacio::S4)
                        .align_y(Alignment::Center),
                    )
                    .padding(Padding::from([espacio::S3, espacio::S4]))
                    .width(Length::Fill)
                    .style(icaro_ui::estilo::tarjeta(p, false)),
                );
            }
            for (id,label,value) in [(0,"Memoria mínima global (MB)",app.java_global.memoria_min.to_string()),(1,"Memoria máxima global (MB)",app.java_global.memoria_max.to_string()),(2,"Metaspace global (MB)",app.java_global.metaspace.to_string()),(3,"Argumentos JVM globales",app.java_global.argumentos.clone())] {
                col=col.push(fila_ajuste(p,label,"Se aplican a las instancias que heredan memoria y argumentos.",componentes::campo(p,"",&value,move|v|Mensaje::Java(crate::java_real::Accion::MemoriaGlobal(id,v)))));
            }
        }
        3 => {
            let storage=&app.almacenamiento;
            if let Some(report)=&storage.informe {
                col=col.push(text(format!("Uso de disco: {}",crate::almacenamiento_real::tamano(report.total))).font(fuentes::MONO).size(texto::MONO.0).color(p.text));
                for(n,bytes)in &report.categorias {
                    col=col.push(row![text(n.clone()).size(texto::BODY.0).color(p.text).width(220),container(componentes::progreso(p,if report.total==0{0.0}else{*bytes as f32/report.total as f32})).width(Length::Fill),text(crate::almacenamiento_real::tamano(*bytes)).font(fuentes::MONO).size(texto::MONO_SM.0).color(p.text_muted).width(90)].spacing(espacio::S4).align_y(Alignment::Center));
                }
                if !report.papeleras.is_empty(){
                    col=col.push(text(format!("{} papeleras internas antiguas. Puedes trasladar su contenido a la papelera del sistema para recuperarlo desde allí.",report.papeleras.len())).size(texto::BODY.0).color(p.text_muted));
                    col=col.push(componentes::boton(p,"Trasladar papeleras antiguas",Variante::Secundario,(!storage.ocupado).then_some(Mensaje::Almacenamiento(crate::almacenamiento_real::Accion::Trasladar))));
                }
            }else{col=col.push(text(if storage.ocupado{"Calculando uso de disco…"}else{"Actualiza para calcular el uso de disco."}).size(texto::BODY.0).color(p.text_muted));}
            if let Some(error)=&storage.error{col=col.push(text(error.clone()).size(texto::BODY.0).color(p.text_muted));}
            col=col.push(Space::with_height(espacio::S3)).push(row![
                componentes::boton(p,"Recuperar operaciones",Variante::Secundario,(!storage.ocupado).then_some(Mensaje::Almacenamiento(crate::almacenamiento_real::Accion::Recuperar))),
                componentes::boton(p,"Actualizar",Variante::Secundario,(!storage.ocupado).then_some(Mensaje::Almacenamiento(crate::almacenamiento_real::Accion::Medir))),
                componentes::boton(p,"Limpiar caché",Variante::Secundario,(!storage.ocupado).then_some(m(Accion::LimpiarCache))),
                componentes::boton(p,"Abrir carpeta de datos",Variante::Fantasma,Some(m(Accion::AbrirCarpetaDatos))),
            ].spacing(espacio::S3));
        }
        4 => {
            col = col
                .push(fila_ajuste(p, "Descargas simultáneas", "Con internet lento, bájalo a 2.",
                    componentes::contador(p, app.simultaneas, Mensaje::Simultaneas(app.simultaneas - 1), Mensaje::Simultaneas(app.simultaneas + 1))))
                .push(fila_ajuste(p, "Límite de velocidad", "En MB/s. Con 0 no hay límite.",
                    row![
                        componentes::contador(p, a.limite_mb, m(Accion::Limite(a.limite_mb - 1)), m(Accion::Limite(a.limite_mb + 1))),
                        text(if a.limite_mb == 0 { "Sin límite".to_owned() } else { format!("{} MB/s", a.limite_mb) }).font(fuentes::MONO).size(texto::MONO_SM.0).color(p.text_muted),
                    ].spacing(espacio::S3).align_y(Alignment::Center).into()));
        }
        5 => {
            col = col
                .push(fila_ajuste(p, "Confirmar antes de eliminar", "Pide confirmación al borrar una instancia.", interruptor(a.confirmar_borrar, CampoAjuste::ConfirmarBorrar)))
                .push(fila_ajuste(p, "Mostrar la consola al jugar", "Abre los registros cuando inicia el juego.", interruptor(a.consola_al_jugar, CampoAjuste::ConsolaAlJugar)))
                .push(fila_ajuste(p, "Copias automáticas de mundos", "Guarda una copia antes de cambiar la versión.", interruptor(a.copias_mundos, CampoAjuste::CopiasMundos)));
        }
        6 => {
            for (tecla, texto_) in [("Ctrl 1 a 7", "Ir a una sección"), ("Ctrl K", "Buscar"), ("Ctrl Enter", "Jugar la instancia elegida"), ("F1", "Ver los atajos"), ("Enter", "Jugar desde el menú"), ("E", "Editar desde el menú"), ("Supr", "Eliminar desde el menú")] {
                col = col.push(
                    row![
                        container(icaro_ui::shell::kbd(p, tecla)).width(160),
                        text(texto_).size(texto::BODY.0).color(p.text),
                    ]
                    .padding(Padding::from([espacio::S2, 0.0]))
                    .align_y(Alignment::Center),
                );
            }
        }
        _ => {
            col = col
                .push(text(format!("Icaro Launcher {}", crate::actualizacion_real::version_visible())).font(fuentes::TITULO).size(texto::HEADING.0).color(p.text))
                .push(text("Un launcher de Minecraft escrito en Rust, ligero y optimizado, para Windows y Arch Linux.").size(texto::BODY.0).color(p.text_muted))
                .push(text("Licencia GPL-3.0. Grabados de dominio público (CC0) de The Met, la National Gallery of Art y el Art Institute of Chicago. Íconos Pixelarticons (MIT).").size(texto::BODY.0).color(p.text_muted))
                .push(Space::with_height(espacio::S2))
                .push(row![componentes::boton_con_icono(p, Icono::Externo, "Código fuente en GitHub", Variante::Secundario, Some(m(Accion::AbrirRepositorio)), false)])
                .push(Space::with_height(espacio::S2))
                .push(fila_ajuste(p, "Canal de actualizaciones", "Beta recibe antes las versiones en desarrollo; pueden tener errores.",
                    componentes::selector(p, vec!["Estable".to_owned(), "Beta".to_owned()],
                        Some(if a.canal == icaro_core::actualizacion::Canal::Beta { "Beta" } else { "Estable" }.to_owned()), |v| m(Accion::Canal(v)))));
            if app.servicios.demo {
                col = col.push(Space::with_height(espacio::S3))
                .push(text("Pantallas de ejemplo").font(fuentes::TITULO).size(texto::BODY.0 + 1.0).color(p.text))
                .push(
                    row![
                        componentes::boton(p, "Primera ejecución", Variante::Secundario, Some(m(Accion::Abrir(Pantalla::Primera)))),
                        componentes::boton(p, "Iniciar sesión", Variante::Secundario, Some(m(Accion::Abrir(Pantalla::Codigo)))),
                        componentes::boton(p, "Informe de fallo", Variante::Secundario, Some(m(Accion::Abrir(Pantalla::Fallo)))),
                        componentes::boton(p, "Error de la tienda", Variante::Secundario, Some(m(Accion::Abrir(Pantalla::ErrorTienda)))),
                        componentes::boton(p, "Archivos sin verificar", Variante::Secundario, Some(m(Accion::Abrir(Pantalla::NoVerificados)))),
                        componentes::boton(p, "Sincronizar mods", Variante::Secundario, Some(m(Accion::Abrir(Pantalla::Sincronizar)))),
                        componentes::boton(p, "Verificar integridad", Variante::Secundario, Some(m(Accion::Abrir(Pantalla::Integridad)))),
                        componentes::boton(p, "Actualización", Variante::Secundario, Some(m(Accion::Abrir(Pantalla::Actualizacion)))),
                        componentes::boton(p, "Cuenta", Variante::Secundario, Some(m(Accion::Abrir(Pantalla::Cuenta)))),
                        componentes::boton(p, "Red privada", Variante::Secundario, Some(m(Accion::Abrir(Pantalla::Red)))),
                        componentes::boton(p, if app.capt.vacio { "Mostrar datos" } else { "Estados vacíos" }, Variante::Secundario, Some(Mensaje::Capt(crate::capt::Accion::AlternarVacios))),
                    ]
                    .spacing(espacio::S3)
                    .wrap(),
                );
            }
        }
    }
    col.into()
}

// ------------------------------------------------------- más pantallas

fn regla<'a>(p: Paleta) -> Element<'a, Mensaje> {
    container(Space::new(Length::Fill, 1.0)).style(icaro_ui::estilo::bloque(p.border)).into()
}

fn sincronizar<'a>(p: Paleta) -> Element<'a, Mensaje> {
    let fila =
        |signo: &str, tono: iced::Color, nombre: &str, accion: &str, version: &str, tam: &str| {
            column![
                row![
                    container(
                        text(signo.to_owned())
                            .font(fuentes::MONO)
                            .size(texto::MONO.0)
                            .color(tono)
                    )
                    .padding(Padding::from([espacio::S1, espacio::S2]))
                    .style(move |_: &iced::Theme| {
                        iced::widget::container::Style {
                            border: iced::Border {
                                color: tono,
                                width: 2.0,
                                radius: 0.0.into(),
                            },
                            ..Default::default()
                        }
                    }),
                    column![
                        text(nombre.to_owned())
                            .font(fuentes::TITULO)
                            .size(texto::BODY.0)
                            .color(p.text),
                        text(accion.to_owned())
                            .size(texto::BODY_SM.0)
                            .color(p.text_muted),
                    ]
                    .spacing(espacio::S1)
                    .width(Length::Fill),
                    text(version.to_owned())
                        .font(fuentes::MONO)
                        .size(texto::MONO_SM.0)
                        .color(p.text_muted),
                    text(tam.to_owned())
                        .font(fuentes::MONO)
                        .size(texto::MONO_SM.0)
                        .color(p.text_muted)
                        .width(70),
                ]
                .spacing(espacio::S3)
                .align_y(Alignment::Center),
                regla(p),
            ]
            .spacing(espacio::S2)
        };
    let derecha = column![
        text("SINCRONIZAR\nCON AMIGOS")
            .font(fuentes::DISPLAY)
            .size(texto::DISPLAY.0)
            .color(p.text),
        text("3 por agregar · 2 por actualizar · 1 por quitar · 412 MB por descargar")
            .size(texto::BODY_SM.0)
            .color(p.text_muted),
        regla(p),
        fila("+", p.success, "Create", "Agregar", "0.5.1j", "18,2 MB"),
        fila(
            "+",
            p.success,
            "Farmer's Delight",
            "Agregar",
            "1.2.6",
            "3,1 MB"
        ),
        fila(
            "~",
            p.info,
            "Lithium",
            "Actualizar",
            "0.14.3 → 0.14.5",
            "720 KB"
        ),
        fila(
            "−",
            p.error,
            "Better Combat",
            "Quitar: el servidor ya no lo usa",
            "1.8.6",
            ""
        ),
        text("Se mantienen 4 mods solo de cliente: Sodium, Iris, Xaero's Minimap, Mod Menu.")
            .size(texto::BODY_SM.0)
            .color(p.text_muted),
        row![
            componentes::boton(
                p,
                "Sincronizar y jugar",
                Variante::Primario,
                Some(m(Accion::Cerrar))
            ),
            componentes::boton(
                p,
                "Solo sincronizar",
                Variante::Secundario,
                Some(m(Accion::Cerrar))
            ),
            componentes::boton(
                p,
                "Jugar sin sincronizar",
                Variante::Fantasma,
                Some(m(Accion::Cerrar))
            ),
        ]
        .spacing(espacio::S3),
    ]
    .spacing(espacio::S3);
    derecha.into()
}

fn integridad<'a>(app: &'a App, p: Paleta) -> Element<'a, Mensaje> {
    let seg = app.ahora.saturating_duration_since(app.codigo_inicio).as_secs_f32();
    let avance = (seg / 12.0).min(1.0);
    let total = 1482u32;
    let hechos = (avance * total as f32) as u32;
    let celdas = 40usize;
    let rellenas = (avance * (celdas * 6) as f32) as usize;
    let mut cuadricula = column![].spacing(2);
    for f in 0..6 {
        let mut fila = row![].spacing(2);
        for c in 0..celdas {
            let i = f * celdas + c;
            let dañada = i == 28 || i == 107;
            let color = if i < rellenas {
                if dañada { p.error } else { p.text_muted }
            } else {
                p.surface_sunken
            };
            fila = fila.push(container(Space::new(Length::Fill, 12.0)).width(Length::Fill).style(icaro_ui::estilo::bloque(color)));
        }
        cuadricula = cuadricula.push(fila);
    }
    let dañados = if rellenas > 28 { if rellenas > 107 { 2 } else { 1 } } else { 0 };
    let mut col = column![
        text("VERIFICANDO\nSUPERVIVENCIA").font(fuentes::DISPLAY).size(texto::DISPLAY.0).color(p.text),
        row![
            text(if avance >= 1.0 { "Verificación terminada" } else { "Revisando librerías" }).size(texto::BODY_SM.0).color(p.text),
            Space::with_width(Length::Fill),
            text(format!("{hechos} de {total}")).font(fuentes::MONO).size(texto::MONO_SM.0).color(p.text_muted),
        ],
        cuadricula,
    ]
    .spacing(espacio::S3);
    if dañados > 0 {
        col = col.push(
            container(
                column![
                    text(format!("{dañados} archivos dañados hasta ahora")).font(fuentes::TITULO).size(texto::BODY.0).color(p.text),
                    text("libraries/org/lwjgl/lwjgl/3.3.3/lwjgl-3.3.3.jar").font(fuentes::MONO).size(texto::MONO_SM.0).color(p.text),
                    text("assets/objects/4f/4f8a2c19e0b7….ogg").font(fuentes::MONO).size(texto::MONO_SM.0).color(p.text),
                ]
                .spacing(espacio::S1),
            )
            .padding(espacio::S4)
            .width(Length::Fill)
            .style(move |_: &iced::Theme| iced::widget::container::Style {
                background: Some(iced::Background::Color(p.error_soft)),
                border: iced::Border { color: p.error, width: 2.0, radius: 0.0.into() },
                ..Default::default()
            }),
        );
    }
    col = col.push(
        row![
            componentes::boton(p, "Cancelar", Variante::Secundario, Some(m(Accion::Cerrar))),
            componentes::boton(p, "Reparar al terminar", Variante::Primario, (avance >= 1.0).then(|| m(Accion::Cerrar))),
        ]
        .spacing(espacio::S3),
    );
    col.into()
}

/// Diálogo para cambiar la versión de una instancia, con aviso de riesgo.
pub fn cambiar_version<'a>(app: &'a App, i: usize) -> Element<'a, Mensaje> {
    let p = app.paleta();
    let Some(c) = app.servicios.instancias.cambio.as_ref() else {
        return Space::new(0, 0).into();
    };
    let Some(record) = app
        .servicios
        .instancias
        .registros
        .iter()
        .find(|r| r.id == c.instancia)
    else {
        return Space::new(0, 0).into();
    };
    if c.solo_loader {return selector_cambio::loader(app);}
    let occupied = app.servicios.instancias.operando;
    if !c.confirmando || c.revision.is_none() {
        return selector_cambio::vista(app);
    }
    let revision = c.revision.as_ref().unwrap();
    let down = icaro_core::versiones_instancia::baja(
        &app.servicios.instancias.versiones,
        &record.minecraft,
        &c.destino,
    );
    let mods = revision.desactivar.len();
    let risk = if down {
        p.error
    } else if mods > 0 {
        p.warning
    } else {
        p.info
    };
    let soft = if down {
        p.error_soft
    } else if mods > 0 {
        p.warning_soft
    } else {
        p.info_soft
    };
    let heading = if down {
        "Bajar de versión puede dañar tus mundos"
    } else if mods > 0 {
        "Algunos mods se desactivarán"
    } else {
        "Cambiar versión de Minecraft"
    };
    let mut detail = if down {
        format!(
            "{} mundos pueden haberse abierto en {}. Minecraft no garantiza que funcionen en {}.",
            revision.mundos, record.minecraft, c.destino
        )
    } else {
        format!("La instancia usará Minecraft {}. Tus archivos se conservarán y el juego se preparará al instalarlo.",c.destino)
    };
    if mods > 0 {
        detail.push_str(&format!(
            " {mods} mods instalados no declaran compatibilidad con {} y se desactivarán: {}.",
            c.destino,
            revision.nombres.join(", ")
        ));
    }
    let current = format!(
        "{} · {}",
        record.minecraft,
        crate::ficha::loader(&record.loader)
    );
    let target = format!("{} · {}", c.destino, crate::ficha::loader(&revision.loader));
    let mut body = column![
        row![
            column![
                etiqueta(p, "Ahora"),
                text(current)
                    .font(fuentes::MONO)
                    .size(texto::MONO.0)
                    .color(p.text)
            ]
            .spacing(espacio::S1),
            icono(Icono::Adelante, Tam::Base, p.text_muted),
            column![
                etiqueta(p, "Después"),
                text(target)
                    .font(fuentes::MONO)
                    .size(texto::MONO.0)
                    .color(p.text)
            ]
            .spacing(espacio::S1)
        ]
        .spacing(espacio::S4)
        .align_y(Alignment::Center),
        container(
            row![
                icono(Icono::Alerta, Tam::Base, risk),
                column![
                    text(heading)
                        .font(fuentes::TITULO)
                        .size(texto::BODY.0)
                        .color(p.text),
                    text(detail).size(texto::BODY_SM.0).color(p.text_muted)
                ]
                .spacing(espacio::S1)
            ]
            .spacing(espacio::S3)
        )
        .padding(espacio::S4)
        .style(move |_| iced::widget::container::Style {
            background: Some(iced::Background::Color(soft)),
            border: iced::Border {
                color: risk,
                width: 2.,
                radius: 0.into()
            },
            ..Default::default()
        }),
        if down {
            iced::widget::checkbox("Hacer copia de seguridad de los mundos antes (obligatorio)", true)
                .icon(componentes::icono_casilla()).font(fuentes::CUERPO)
                .size(20).text_size(texto::BODY.0).spacing(espacio::S2)
                .style(icaro_ui::estilo::casilla(p)).into()
        } else {
            casilla(p, "Hacer copia de seguridad de la instancia antes", c.respaldo,
                |b| m(Accion::CambiarRespaldo(b)))
        },
    ]
    .spacing(espacio::S4);
    if let Some(e) = &c.error {
        body = body.push(text(e).size(texto::BODY_SM.0).color(p.error));
    }
    let label = if occupied {
        "Guardando…".into()
    } else {
        format!("Cambiar a {}", c.destino)
    };
    let enabled = !occupied && (!down || c.respaldo);
    let pie = row![
            componentes::boton_con_icono(
                p,
                Icono::Rama,
                "Crear una copia en su lugar",
                Variante::Fantasma,
                (!occupied).then(|| m(Accion::CambiarCopia)),
                false,
            ),
            Space::with_width(Length::Fill),
            column![componentes::boton(
                p,
                "Cancelar",
                Variante::Secundario,
                (!occupied).then(|| m(Accion::CambiarCancelar)),
            ),
            componentes::boton(
                p,
                &label,
                if down {
                    Variante::Peligro
                } else {
                    Variante::Primario
                },
                enabled.then(|| m(Accion::CambiarConfirmar(i))),
            )].spacing(espacio::S2).align_x(Alignment::End),
        ].spacing(espacio::S2).align_y(Alignment::Center);
    let derecha = column![
        container(text("Cambiar versión").font(fuentes::TITULO).size(26).color(p.text))
            .padding([espacio::S4, espacio::S6]),
        container(crate::desplazable_pub(body.into()))
            .padding([espacio::S2, espacio::S6]).height(Length::Fill),
        regla(p),
        container(pie).padding([espacio::S4, espacio::S6]),
    ].width(Length::Fill).height(Length::Fill);
    container(row![
        icaro_ui::laminas::grabado_posicion(p, Lamina::CaidaDeFaeton, 200, Length::Fill, (0.6, 0.4)),
        container(Space::new(2, Length::Fill)).style(icaro_ui::estilo::bloque(p.text)),
        derecha,
    ])
    .padding(2).width(820).height(440)
    .style(icaro_ui::estilo::flotante(p)).into()
}

// ------------------------------------------------- red privada

fn red_privada<'a>(p: Paleta) -> Element<'a, Mensaje> {
    let paso = |n: &str, t: &str, d: &str| -> Element<'a, Mensaje> {
        column![
            text(format!("{n}  {t}")).font(fuentes::TITULO).size(texto::BODY.0).color(p.text),
            text(d.to_owned()).size(texto::BODY_SM.0).color(p.text_muted),
        ]
        .spacing(espacio::S1)
        .into()
    };
    let miembro = |n: &str, ip: &str, en_linea: bool| -> Element<'a, Mensaje> {
        row![
            text(n.to_owned()).size(texto::BODY.0).color(p.text).width(Length::Fill),
            text(ip.to_owned()).font(fuentes::MONO).size(texto::MONO_SM.0).color(p.text_muted),
            if en_linea { insignia(p, Estado::Exito, "En línea") } else { insignia(p, Estado::Info, "Desconectado") },
        ]
        .spacing(espacio::S3)
        .align_y(Alignment::Center)
        .into()
    };
    column![
        text("JUEGA CON AMIGOS\nSIN ABRIR PUERTOS").font(fuentes::DISPLAY).size(texto::DISPLAY.0 - 8.0).color(p.text),
        paso("1", "ZeroTier instalado", "Versión 1.14.2 · servicio activo."),
        paso("2", "Únete a una red", "Pide el ID de red a quien la creó."),
        row![
            container(text("8056c2e21c000001").font(fuentes::MONO).size(texto::MONO.0).color(p.text)).padding(espacio::S3).width(Length::Fill).style(icaro_ui::estilo::marco(p)),
            componentes::boton(p, "Unirse", Variante::Primario, Some(Mensaje::Nada)),
            componentes::boton(p, "Crear una red nueva", Variante::Secundario, Some(Mensaje::Nada)),
        ]
        .spacing(espacio::S2),
        paso("3", "Espera la autorización", "Te avisaremos cuando el dueño de la red te autorice."),
        icaro_ui::componentes::etiqueta(p, "Miembros"),
        miembro("Amigos · Javier", "10.147.17.1", true),
        miembro("Camila", "10.147.17.8", true),
        miembro("Nico", "10.147.17.12", false),
        componentes::boton(p, "Volver", Variante::Fantasma, Some(m(Accion::Cerrar))),
    ]
    .spacing(espacio::S3)
    .into()
}
