// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

#![cfg_attr(all(windows, not(test)), windows_subsystem = "windows")]

//! Launcher con autenticación y catálogo conectados. ICARO_DEMO=1 carga ejemplos.
//! Ejecutar con
//! `cargo run -p icaro-ui --example launcher`.

use icaro_ui::ajustes::indice_ajustes;
use icaro_ui::componentes::{self, Variante};
use icaro_ui::consola::{consola, Linea, MensajesConsola, Nivel};
use icaro_ui::editor::{
    pestana_archivos, pestana_general, pestana_java, pestana_mundos,
    Copia, DatosEditor, EstadoGeneral,
    InstalacionJava, MensajesEncabezado, MensajesGeneral, Mundo, AccionArchivo, ConfigJava,
    MensajesArchivos, MensajesJava, VistaArchivos,
};
use icaro_ui::descargas::{cola_descargas, historial, limites, AccionesCola};
use icaro_ui::fuentes;
use icaro_ui::iconos::{icono, Icono, Tam};
use icaro_ui::instancias::{
    tarjeta_nueva, AccionesTarjeta, DatosInstancia, EstadoInstancia,
};
use icaro_ui::laminas::{banda, banda_completa, DatoBanda, Lamina, ALTO_BANDA};
use icaro_ui::servidores::{lista_servidores, red_privada, AccionesLista};
#[path = "launcher_aux/archivos.rs"]
mod archivos;
#[path = "launcher_aux/estado.rs"]
mod estado;
#[path = "launcher_aux/servidores_reales.rs"]
mod servidores_reales;
#[path = "launcher_aux/red_privada_real.rs"]
mod red_privada_real;
#[path = "launcher_aux/sincronizacion_real.rs"]
mod sincronizacion_real;
#[path = "launcher_aux/almacenamiento_real.rs"]
mod almacenamiento_real;
#[path = "launcher_aux/estadisticas_reales.rs"]
mod estadisticas_reales;
#[path = "launcher_aux/java_real.rs"]
mod java_real;
#[path = "launcher_aux/capt.rs"]
mod capt;
#[path = "launcher_aux/mods.rs"]
mod mods;
#[path = "launcher_aux/modpacks.rs"]
mod modpacks;
#[path = "launcher_aux/pantallas.rs"]
mod pantallas;
#[path = "launcher_aux/sesion.rs"]
mod sesion;
#[path = "launcher_aux/cuenta_real.rs"]
mod cuenta_real;
#[path = "launcher_aux/red.rs"]
mod red;
#[path = "launcher_aux/busqueda.rs"]
mod busqueda;
#[path = "launcher_aux/atajos.rs"]
mod atajos;
#[path = "launcher_aux/juego_real.rs"]
mod juego_real;

#[path = "launcher_aux/verificacion_real.rs"]
mod verificacion_real;
#[path = "launcher_aux/icono_instancia.rs"]
mod icono_instancia;
#[path = "launcher_aux/exportar_lista_mods.rs"]
mod exportar_lista_mods;
#[path = "launcher_aux/picar_real.rs"]
mod picar_real;
#[path = "launcher_aux/apariencia_real.rs"]
mod apariencia_real;
#[path = "launcher_aux/componentes_nuevos_qa.rs"]
mod componentes_nuevos_qa;
#[path = "launcher_aux/actualizacion_real.rs"]
mod actualizacion_real;
#[path = "launcher_aux/instancias_reales.rs"]
mod instancias_reales;
#[path = "launcher_aux/modpacks_reales.rs"]
mod modpacks_reales;
#[path = "launcher_aux/mods_reales.rs"]
mod mods_reales;
#[path = "launcher_aux/diseno_mods.rs"]
mod diseno_mods;
#[path = "launcher_aux/pruebas_diseno.rs"]
mod pruebas_diseno;
#[path = "launcher_aux/servicios.rs"]
mod servicios;
#[path = "launcher_aux/cuenta_sin_conexion.rs"]
mod cuenta_sin_conexion;
#[path = "launcher_aux/skin_sin_conexion.rs"]
mod skin_sin_conexion;
#[path = "launcher_aux/catalogo.rs"]
mod catalogo;
#[path = "launcher_aux/ficha.rs"]
mod ficha;
#[path = "launcher_aux/info_proyecto.rs"]
mod info_proyecto;
#[path = "launcher_aux/ventana.rs"]
mod ventana;

use archivos::Explorador;
use icaro_ui::scroll::desplazable;
use icaro_ui::shell::{app_shell, Cuenta, DescargasActivas, MensajesShell, Seccion};
use icaro_ui::superficies::{con_velo_losa, dialogo, menu_contextual, posicion_menu, tamano_menu, toast_con_opacidad, ElementoMenu};
use icaro_ui::tema::{espacio, texto, Modo};
use iced::widget::{column, container, row, text, Space};
use iced::{window, Alignment, Element, Length, Padding, Point, Size, Task};

struct App {
    actualizacion: actualizacion_real::Estado,
    verificacion: verificacion_real::Estado,
    iconos: icono_instancia::Estado,
    exportar_lista: exportar_lista_mods::Estado,
    servicios: servicios::Estado,
    modo: Modo,
    seccion: Seccion,
    colapsada: bool,
    barra_angosta: bool,
    filtro: usize,
    seleccionada: Option<usize>,
    menu: Option<usize>,
    dialogo: bool,
    aviso: Option<icaro_ui::avisos::Aviso<Mensaje>>,
    avisos: icaro_ui::avisos::Cola<Mensaje>,
    tema: usize,
    reducir: bool,
    simultaneas: i32,
    ajuste: usize,
    editor: Option<usize>,
    general: EstadoGeneral,
    configs: Vec<ConfigJava>,
    config_error: Option<String>,
    /// Texto completo del aviso de error abierto con "Ver detalle".
    detalle_error: Option<String>,
    almacenamiento: almacenamiento_real::Estado,
    estadisticas: estadisticas_reales::Estado,
    java_global: java_real::Preferencias,
    java_detectadas: Vec<icaro_core::java::Java>,
    java_estado: String,
    java_generacion: u64,
    java_prueba_generacion: u64,
    lista_grupos: Vec<String>,
    grupos: Vec<Option<String>>,
    cerrados: Vec<Option<String>>,
    edicion_grupo: Option<EdicionGrupo>,
    busqueda_mods: String,
    mods_vista: usize,
    mods_filtro: usize,
    mod_panel: Option<String>,
    anim_panel: Option<std::time::Instant>,
    anim_dialogo: Option<std::time::Instant>,
    tienda_lista: Vec<String>,
    tienda_cat: Vec<String>,
    tienda_lado: usize,
    tienda_orden: usize,
    modpack_filtro: usize,
    pantalla: Option<pantallas::Pantalla>,
    aj: pantallas::Ajustes,
    asistente: Option<pantallas::Asistente>,
    importar: Option<pantallas::Importar>,
    cambiar: Option<usize>,
    exportar: Option<pantallas::Exportar>,
    codigo_inicio: std::time::Instant,
    codigo_fase: pantallas::FaseCodigo,
    intentos: u32,
    detalle_fallo: bool,
    modpack_pagina: Option<usize>,
    modpack_pestana: usize,
    modpack_galeria: usize,
    busqueda_modpacks: String,
    instalando: Vec<(String, std::time::Instant)>,
    mod_pagina: Option<String>,
    mod_filtro_mc: String,
    mod_filtro_loader: String,
    mod_betas: bool,
    mod_galeria: usize,
    mod_opcionales: Vec<String>,
    mod_pestana: usize,
    ultimo_guardado: String,
    anim: Option<std::time::Instant>,
    anim_menu: Option<std::time::Instant>,
    ahora: std::time::Instant,
    ultimo_clic: Option<std::time::Instant>,
    menu_grupos: bool,
    editor_inst: usize,
    exp: Explorador,
    mundo: usize,
    filtro_consola: usize,
    busqueda_consola: String,
    niveles_consola: [bool; 3],
    autoscroll_consola: bool,
    desplazamiento_consola: icaro_ui::consola::DesplazamientoConsola,
    capt: capt::Estado,
    cursor: Point,
    red: red::Estado,
    menu_en: Point,
    ventana: Size,
    maximizada: bool,
    /// Instante en que se pulsó Jugar; mientras exista, el Umbral cubre la ventana.
    umbral: Option<std::time::Instant>,
    /// Eliminación en curso manteniendo Supr sobre la instancia seleccionada.
    picar: Option<Picar>,
    picar_objetivo: Option<usize>,
    picar_revision: Option<picar_real::Revision>,
    picar_solicitud: u64,
    /// Instancia recién devuelta a la cuadrícula (Colocar).
    colocada: Option<(usize, std::time::Instant)>,
    sesion: sesion::Sesion,
    busqueda: busqueda::Estado,
}

/// Eliminación por Picar: se mantiene presionado hasta romper la tarjeta.
struct Picar {
    instancia: usize,
    inicio: std::time::Instant,
    /// Si se soltó antes de tiempo: cuándo y desde qué etapa se retiran las grietas.
    soltada: Option<(std::time::Instant, u32)>,
    /// Instante en que la tarjeta se rompió.
    rota: Option<std::time::Instant>,
}

fn ms_desde(inicio: std::time::Instant, ahora: std::time::Instant) -> f32 {
    ahora.saturating_duration_since(inicio).as_secs_f32() * 1000.0
}

/// Etapa de grietas visible de una eliminación en curso.
fn etapa_picar(p: &Picar, ahora: std::time::Instant) -> u32 {
    match p.soltada {
        Some((t, desde)) => icaro_ui::movimiento::picar_soltar(desde, ms_desde(t, ahora)),
        None => icaro_ui::movimiento::picar_etapa(ms_desde(p.inicio, ahora)),
    }
}

/// Creación o renombrado de un grupo en curso.
struct EdicionGrupo {
    /// Nombre actual si se renombra; `None` si se crea uno nuevo.
    original: Option<String>,
    texto: String,
    /// Instancia que se mueve al grupo recién creado.
    mover: Option<usize>,
}

impl App {
    fn barra_colapsada(&self) -> bool { self.colapsada || self.barra_angosta }
    fn paleta(&self) -> icaro_ui::tema::Paleta { self.modo.paleta().con_metal(icaro_ui::tema::Metal::desde_indice(self.aj.metal)) }
    fn avisar_con(&mut self, texto:String, etiqueta:&str, accion:Mensaje) {
        let mut aviso=icaro_ui::avisos::Aviso::nuevo(texto,Some(accion),std::time::Instant::now());
        aviso.etiqueta_accion=etiqueta.into();self.avisos.agregar(aviso);
    }
    fn avisar_error(&mut self,texto:String) {
        self.avisos.agregar(icaro_ui::avisos::Aviso::nuevo(texto.clone(),Some(Mensaje::DetalleAviso(texto)),std::time::Instant::now()).de_error());
    }
    fn avisar(&mut self, texto: String) {
        let accion = if !self.servicios.demo && self.servicios.instancias.deshacer.is_some() {
            Some(Mensaje::DeshacerAviso)
        } else if self.red.anterior.is_some() {
            Some(Mensaje::Red(red::Mensaje::Deshacer))
        } else if self.picar.as_ref().is_some_and(|p| p.rota.is_some()) {
            Some(Mensaje::DeshacerAviso)
        } else { None };
        let aviso=icaro_ui::avisos::Aviso::nuevo(texto,accion.clone(),std::time::Instant::now());
        self.avisos.agregar(if accion.is_some(){aviso.con_duracion(std::time::Duration::from_secs(10))}else{aviso});
    }
}

#[derive(Debug, Clone)]
enum Mensaje {
    Java(java_real::Accion),
    Almacenamiento(almacenamiento_real::Accion),
    Estadisticas(estadisticas_reales::Accion),
    Verificacion(verificacion_real::Accion),
    Icono(icono_instancia::Accion),
    ExportarLista(exportar_lista_mods::Accion),
    IconoEvento(iced::Event),
    ModpackEvento(iced::Event),
    TemaSistema(bool),
    ComprobarTemaSistema,
    Actualizacion(actualizacion_real::Accion),
    Juego(juego_real::Accion),
    CapturarDiseno,
    DisenoCapturado(window::Screenshot),
    DisenoMods(diseno_mods::Accion),
    Modpacks(modpacks_reales::Accion),
    Servidores(servidores_reales::Accion),
    RedPrivada(red_privada_real::Accion),
    Sincronizacion(sincronizacion_real::Accion),
    ModsReales(mods_reales::Accion),
    PacksReales(mods_reales::packs::Accion),
    Instancias(instancias_reales::Accion),
    AtajoSeccion(Seccion),
    Servicio(servicios::Accion),
    AbrirBusqueda,
    CerrarBusqueda,
    BuscarGlobal(String),
    MoverBusqueda(i32),
    ConfirmarBusqueda,
    AbrirResultado(busqueda::Destino),
    Editar,
    VolverEditor,
    Pestana(usize),
    Nombre(String),
    Grupo(String),
    Ancho(String),
    Alto(String),
    Completa(bool),
    Servidor(&'static str),
    JavaVersion(Option<String>),
    MemMin(u32),
    MemMax(u32),
    Metaspace(u32),
    ArgsJvm(String),
    Mundo(usize),
    AbrirInstancia(usize),
    MoverGrupo(Option<String>),
    AlternarGrupo(Option<String>),
    NuevoGrupo,
    NuevoGrupoParaInstancia,
    RenombrarGrupo(String),
    EliminarGrupo(String),
    TextoGrupo(String),
    GuardarGrupo,
    CancelarGrupo,
    VerGrupos(bool),
    BuscarMods(String),
    AlternarMod(String),
    VistaMods(usize),
    FiltroMods(usize),
    PanelMod(Option<String>),
    TiendaAgregar(String),
    TiendaQuitar(String),
    TiendaVaciar,
    TiendaInstalar,
    TiendaCat(String),
    TiendaLado(usize),
    TiendaOrden(usize),
    Pant(pantallas::Accion),
    Ses(sesion::Accion),
    CuentaReal(cuenta_real::Accion),
    FiltroModpacks(usize),
    AbrirModpack(usize),
    CerrarModpack,
    PestanaModpack(usize),
    GaleriaModpack(usize),
    BuscarModpacks(String),
    InstalarModpack(usize),
    AbrirModPagina(String),
    CerrarModPagina,
    PestanaMod(usize),
    FiltroMcMod(String),
    FiltroLoaderMod(String),
    BetasMod,
    GaleriaMod(usize),
    OpcionalMod(String),
    VersionMod(String, String),
    InstalarMod(String, String),
    EliminarMod(String),
    Cuadro(std::time::Instant),
    ElegirArchivo(usize),
    AccionArchivo(AccionArchivo),
    EditarArchivo(iced::widget::text_editor::Action),
    NombreArchivo(String),
    FiltroConsola(usize),
    BuscarConsola(String),
    NivelConsola(usize),
    AutoscrollConsola(bool),
    DesplazadoConsola(icaro_ui::consola::DesplazamientoConsola),
    FinalConsola,
    Capt(capt::Accion),
    Ir(Seccion),
    Filtro(usize),
    Menu(Option<usize>),
    PedirEliminar,
    CerrarDialogo,
    Eliminar,
    CerrarAviso,
    Escape,
    CerrarAvisoId(u64),
    PausarAviso(u64,bool),
    AccionAviso(u64,Box<Mensaje>),
    DetalleAviso(String),
    DeshacerAviso,
    AvisoTic(std::time::Instant),
    Tema(usize),
    Reducir,
    Simultaneas(i32),
    Ajuste(usize),
    Arrastrar,
    Minimizar,
    Redimensionar(ventana::Borde),
    Maximizada(bool),
    Maximizar,
    Cerrar,
    Nada,
    ImagenesActualizadas,
    Cursor(Point),
    Tamano(Size),
    Jugar(usize),
    UmbralVolver,
    Picar(picar_real::Accion),
    PicarEmpezar,
    PicarSoltar,
    Red(red::Mensaje),
}

/// Inicia la entrada suave del contenido, salvo con movimiento reducido.
fn arrancar(app: &mut App) {
    if !app.reducir {
        app.anim = Some(std::time::Instant::now());
        app.ahora = std::time::Instant::now();
    }
}

/// Progreso de 0 a 1 de una animación que empezó en `inicio`.
fn progreso(inicio: Option<std::time::Instant>, ahora: std::time::Instant, ms: u64) -> f32 {
    inicio.map_or(1.0, |i| {
        (ahora.saturating_duration_since(i).as_millis() as f32 / ms as f32).clamp(0.0, 1.0)
    })
}

const MS_ENTRADA: u64 = 320;
const MS_MENU: u64 = 160;
const MS_PANEL: u64 = 240;
const MS_LOSA: u64 = 280;

/// Entrada del contenido: aparece despacio. Un velo parcial del color de fondo
/// se disipa de forma continua (sin pasos, para que no parpadee) y arranca
/// lejos de ser opaco para que el contenido nunca desaparezca del todo.
fn con_entrada<'a>(app: &App, contenido: Element<'a, Mensaje>) -> Element<'a, Mensaje> {
    let p = app.paleta();
    let t = progreso(app.anim, app.ahora, MS_ENTRADA);
    let mut velo = p.bg;
    velo.a = 0.7 * (1.0 - icaro_ui::movimiento::Curva::Lineal.aplicar(t));
    // Se apila siempre para que el árbol de widgets no cambie al terminar.
    iced::widget::stack![
        contenido,
        container(Space::new(Length::Fill, Length::Fill))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(icaro_ui::estilo::bloque(velo)),
    ]
    .into()
}

fn abrir_instancia(app: &mut App, i: usize) {
    if app.pantalla == Some(pantallas::Pantalla::Fallo) { app.pantalla = None; }
    if i >= con_instancias(|l| l.len()) { return; }
    if !app.servicios.demo && app.editor.is_some() {
        let nombre = app.general.nombre.clone();
        if let Err(error) = instancias_reales::renombrar(app, &nombre) {
            app.avisar(error); return;
        }
    }
    let datos = con_instancias(|l| l[i].clone());
    let exp = if app.servicios.demo { Explorador::abrir(&datos.nombre) } else {
        let Some(record) = app.servicios.instancias.registros.get(i) else { return; };
        match app.servicios.instancias.almacen.juego(&record.id) {
            Ok(path) => Explorador::desde_ruta(path),
            Err(error) => { app.avisar(error.to_string()); return; }
        }
    };
    app.servicios.reset_catalog();
    app.mods_vista = 0;
    app.seleccionada = Some(i);
    app.editor_inst = i;
    // Al abrirla se vuelve a medir: mods, mundos o capturas pudieron cambiar fuera.
    if let Some(id) = app.servicios.instancias.registros.get(i).map(|r| r.id.clone()) {
        estadisticas_reales::vencer(app, &id);
    }
    mods_reales::cargar_local(app);
    mods_reales::cargar_recursos(app);
    app.exp = exp;
    app.general.nombre = datos.nombre.clone();
    app.general.grupo = app.grupos[i].clone();
    app.mod_pagina = None;
    app.menu = None;
    app.editor = Some(0);
    arrancar(app);
}

fn actualizar(app: &mut App, m: Mensaje) -> Task<Mensaje> {
    let guardar = !matches!(&m, Mensaje::Cuadro(_) | Mensaje::Cursor(_)
        | Mensaje::Tamano(_) | Mensaje::Maximizada(_) | Mensaje::AvisoTic(_) | Mensaje::Nada | Mensaje::ImagenesActualizadas
        | Mensaje::Pant(pantallas::Accion::VersionScroll(..) | pantallas::Accion::AsistenteBuscar(_) | pantallas::Accion::CambiarBuscar(_))
        | Mensaje::Juego(juego_real::Accion::Progreso(..) | juego_real::Accion::Evento(..)));
    let instalaciones = app.instalando.len();
    mods::fijar_recurso(app.editor == Some(3));
    let deshacer_captura = matches!(&m, Mensaje::Capt(capt::Accion::Deshacer));
    let habia = hay_dialogo(app);
    let tarea = actualizar_estado(app, m);
    app.servicios.servidores.avanzar();
    if let Some(aviso)=app.aviso.take(){app.avisos.agregar(aviso);}
    if deshacer_captura { app.aviso = None; }
    if let Some(aviso) = app.capt.aviso.take() {
        app.aviso = Some(icaro_ui::avisos::Aviso::nuevo(aviso.texto,
            aviso.deshacer.then_some(Mensaje::Capt(capt::Accion::Deshacer)), aviso.desde));
    }
    if !habia && hay_dialogo(app) && !app.reducir {
        app.anim_dialogo = Some(std::time::Instant::now());
        app.ahora = std::time::Instant::now();
    }
    // Los cuadros normales no cambian datos persistentes. Al terminar una
    // instalación sí hay que guardar el modpack, aunque no haya otro clic.
    if (guardar || instalaciones != app.instalando.len()) && std::env::var_os("ICARO_CAPTURA_DISENO").is_none() {
        guardar_si_cambio(app);
    }
    // Solo hace trabajo si el editor muestra una instancia sin datos vigentes.
    Task::batch([tarea, estadisticas_reales::pedir(app)])
}

fn hay_dialogo(app: &App) -> bool {
    if app.servicios.sincronizacion.modal(){return true;}
    if app.ventana.width<1100. && app.servicios.servidores.panel_abierto(){return true;}
    if app.servicios.mods.confirmacion.is_some() || app.servicios.mods.confirmacion_recurso.is_some()
        || app.servicios.mods.packs.confirmacion.is_some() { return true; }
    app.exportar_lista.dialogo.is_some() || app.picar_objetivo.is_some() || app.iconos.dialogo.is_some() || app.dialogo || app.asistente.is_some() || app.importar.is_some() || app.exportar.as_ref().is_some_and(|s|!s.guardando) || app.cambiar.is_some()
}

/// Desfase de la losa del diálogo según su progreso.
fn desfase_losa(app: &App) -> f32 {
    icaro_ui::movimiento::losa(progreso(app.anim_dialogo, app.ahora, MS_LOSA))
}

/// Escribe el estado organizado por el usuario solo cuando cambia.
fn guardar_si_cambio(app: &mut App) {
    if app.config_error.is_some() { return; }
    // Modpacks ya instalados (las 5 primeras instancias son las de ejemplo).
    let instalados: Vec<String> = instancias()
        .into_iter()
        .skip(5)
        .filter(|d| d.estado == EstadoInstancia::Lista)
        .map(|d| d.nombre)
        .collect();
    let mut guardado = estado::Guardado::nuevo(&app.lista_grupos, &app.grupos, &app.cerrados, &app.configs, &instalados);
    if !app.servicios.demo {
        guardado.asociar(app.servicios.instancias.registros.iter().map(|i| i.id.clone()).collect());
    }
    guardado.java_global = app.java_global.clone();
    guardado.apariencia = estado::AparienciaGuardada { tema: app.tema, metal: app.aj.metal, escala: app.aj.escala, reducir: app.reducir };
    guardado.canal = (app.aj.canal != actualizacion_real::canal_compilado()).then_some(app.aj.canal);
    let texto = guardado.texto();
    if texto != app.ultimo_guardado {
        match estado::Guardado::escribir(&texto, &app.ultimo_guardado, &app.servicios.instancias.almacen, app.servicios.demo) {
            Ok(()) => app.ultimo_guardado = texto,
            Err(error) => app.avisar(format!("No se pudo guardar la configuración: {error}")),
        }
    }
}

fn actualizar_estado(app: &mut App, m: Mensaje) -> Task<Mensaje> {
    if app.servicios.sincronizacion.modal() && matches!(m,Mensaje::Ir(_)){return Task::none();}
    if matches!(&m, Mensaje::Pant(pantallas::Accion::Abrir(pantallas::Pantalla::Actualizacion))) {
        return actualizacion_real::actualizar(app, actualizacion_real::Accion::Abrir);
    }
    if !app.servicios.demo {
        if let Mensaje::Red(red::Mensaje::Agregar)=&m{return servidores_reales::actualizar(app,servidores_reales::Accion::Nuevo);}
        if let Mensaje::Red(red::Mensaje::Jugar(i))=&m{return servidores_reales::actualizar(app,servidores_reales::Accion::Jugar(*i));}
    }
    if !app.servicios.demo && matches!(&m, Mensaje::TiendaInstalar | Mensaje::InstalarMod(_, _) | Mensaje::VersionMod(_, _)
        | Mensaje::InstalarModpack(_) | Mensaje::AlternarMod(_) | Mensaje::EliminarMod(_)) {
        app.avisar("La instalación y gestión de archivos de mods todavía están en desarrollo.".into());
        return Task::none();
    }
    match m {
        Mensaje::Modpacks(a) => return modpacks_reales::actualizar(app,a),
        Mensaje::Servidores(a)=>return servidores_reales::actualizar(app,a),
        Mensaje::RedPrivada(a)=>return red_privada_real::actualizar(app,a),
        Mensaje::Sincronizacion(a)=>return sincronizacion_real::actualizar(app,a),
        Mensaje::Almacenamiento(a) => return almacenamiento_real::actualizar(app,a),
        Mensaje::Estadisticas(a) => return estadisticas_reales::actualizar(app, a),
        Mensaje::Java(a) => return java_real::actualizar(app, a),
        Mensaje::Actualizacion(a) => return actualizacion_real::actualizar(app, a),
        Mensaje::Juego(a) => return juego_real::actualizar(app, a),
        Mensaje::CapturarDiseno => return window::get_latest().then(|id| id.map_or(Task::none(),|id| window::screenshot(id).map(Mensaje::DisenoCapturado))),
        Mensaje::DisenoCapturado(s) => {
            if let Some(path)=std::env::var_os("ICARO_CAPTURA_DISENO") { let _=image::save_buffer(path,&s.bytes,s.size.width,s.size.height,image::ColorType::Rgba8); }
            return window::get_latest().then(|id| id.map_or(Task::none(),window::close));
        }
        Mensaje::DisenoMods(a) => return diseno_mods::actualizar(app,a),
        Mensaje::Instancias(action) => return instancias_reales::actualizar(app, action),
        Mensaje::AtajoSeccion(seccion) => {
            if hay_dialogo(app) || app.capt.visor.is_some() || app.servicios.ficha.visor.is_some()
                || app.umbral.is_some() || matches!(app.pantalla, Some(pantallas::Pantalla::Primera | pantallas::Pantalla::Codigo)) {
                return Task::none();
            }
            app.busqueda.abierta = false;
            app.menu = None;
            app.menu_grupos = false;
            if app.pantalla == Some(pantallas::Pantalla::Sincronizar) {
                sincronizacion_real::salir(app);
            }
            app.pantalla = None;
            return actualizar_estado(app, Mensaje::Ir(seccion));
        }
        Mensaje::Servicio(action) => return servicios::actualizar(app, action),
        Mensaje::ModsReales(action) => return mods_reales::actualizar(app, action),
        Mensaje::PacksReales(action) => return mods_reales::packs::actualizar(app, action),
        Mensaje::AbrirBusqueda => {
            if hay_dialogo(app) || app.capt.visor.is_some() || app.servicios.ficha.visor.is_some() || app.umbral.is_some() {
                return Task::none();
            }
            app.busqueda.abierta = true;
            app.busqueda.consulta.clear();
            app.busqueda.seleccion = 0;
            app.sesion.menu = false;
            app.menu = None;
            return iced::widget::text_input::focus(busqueda::CAMPO);
        }
        Mensaje::CerrarBusqueda => app.busqueda.abierta = false,
        Mensaje::BuscarGlobal(consulta) => {
            app.busqueda.consulta = consulta;
            app.busqueda.seleccion = 0;
            return iced::widget::scrollable::snap_to(iced::widget::scrollable::Id::new(busqueda::LISTA),
                iced::widget::scrollable::RelativeOffset::START);
        }
        Mensaje::MoverBusqueda(paso) => {
            let total = busqueda::resultados(app).len();
            app.busqueda.seleccion = busqueda::mover(app.busqueda.seleccion, paso, total);
            let y = app.busqueda.seleccion as f32 / total.saturating_sub(1).max(1) as f32;
            return iced::widget::scrollable::snap_to(iced::widget::scrollable::Id::new(busqueda::LISTA),
                iced::widget::scrollable::RelativeOffset { x: 0.0, y });
        }
        Mensaje::ConfirmarBusqueda => {
            if let Some(r) = busqueda::resultados(app).get(app.busqueda.seleccion) {
                return actualizar_estado(app, Mensaje::AbrirResultado(r.destino.clone()));
            }
        }
        Mensaje::AbrirResultado(destino) => {
            app.busqueda.abierta = false;
            app.pantalla = None;
            app.sesion.menu = false;
            app.mod_panel = None;
            app.mod_pagina = None;
            match destino {
                busqueda::Destino::Instancia(i) if i < instancias().len() => {
                    app.seccion = Seccion::Instancias;
                    abrir_instancia(app, i);
                }
                busqueda::Destino::Mod(id) if !instancias().is_empty() => {
                    let i = if app.editor.is_some() { Some(app.editor_inst) } else { app.seleccionada };
                    let Some(i) = i else { return Task::none(); };
                    app.seccion = Seccion::Instancias;
                    abrir_instancia(app, i.min(instancias().len() - 1));
                    app.editor = Some(2);
                    app.mods_vista = 1;
                    mods::fijar_recurso(false);
                    return actualizar_estado(app, Mensaje::AbrirModPagina(id));
                }
                busqueda::Destino::Servidor(i) if i < if app.servicios.demo {app.red.servidores.len()}else{app.servicios.servidores.guardados().len()} => {
                    app.seccion = Seccion::Servidores;
                    app.editor = None;
                    app.red.sobre = Some(i);
                    arrancar(app);
                }
                _ => {}
            }
        }
        Mensaje::Editar => {
            let Some(i) = app.menu.or(app.seleccionada) else { return Task::none(); };
            abrir_instancia(app, i);
        }
        Mensaje::AbrirInstancia(i) => abrir_instancia(app, i),
        Mensaje::AlternarGrupo(g) => {
            arrancar(app);
            if let Some(pos) = app.cerrados.iter().position(|c| *c == g) {
                app.cerrados.remove(pos);
            } else {
                app.cerrados.push(g);
            }
        }
        Mensaje::MoverGrupo(g) => {
            arrancar(app);
            let Some(i) = app.menu.or(app.seleccionada) else { return Task::none(); };
            app.grupos[i] = g.clone();
            app.menu = None;
            let nombre = con_instancias(|l| l[i].nombre.clone());
            app.avisar(match g {
                Some(g) => format!("{nombre} ahora está en el grupo {g}."),
                None => format!("{nombre} quedó sin grupo."),
            });
        }
        Mensaje::VolverEditor => {
            if !app.servicios.demo && app.mods_vista == 1 {
                app.mods_vista = 0;
                app.servicios.reset_catalog();
                return Task::none();
            }
            if !app.servicios.demo && app.editor.is_some() {
                let nombre = app.general.nombre.clone();
                if let Err(error) = instancias_reales::renombrar(app, &nombre) {
                    app.avisar(error); return Task::none();
                }
            }
            app.editor = None;
            app.tienda_cat.clear();
            app.tienda_lado = 2;
            arrancar(app);
        }
        Mensaje::Pestana(i) => {
            if app.editor!=Some(i) {app.tienda_cat.clear();app.tienda_lado=2;app.servicios.reset_catalog();}
            if i == 2 && !mods_reales::permite_mods(app) { return Task::none(); }
            app.mods_vista = 0;
            app.editor = Some(i);
            arrancar(app);
            if i == 2 && !app.servicios.demo { return mods_reales::actualizar(app, mods_reales::Accion::Cargar); }
            if matches!(i, 3 | 8) && !app.servicios.demo {
                return mods_reales::packs::cargar(app, if i == 3 {
                    icaro_core::mods::TipoPack::Recursos
                } else { icaro_core::mods::TipoPack::Shaders });
            }
            if i == 5 { return capt::refrescar(app); }
            if i == 6 && !app.servicios.demo { return juego_real::cargar_consola(app); }
            if matches!(i, 2 | 3 | 8) && app.mods_vista == 1 && !app.servicios.demo { return servicios::buscar(app, 0, false); }
        }
        Mensaje::Nombre(v) => app.general.nombre = v,
        Mensaje::Grupo(v) => {
            let g = (v != "Sin grupo").then_some(v);
            app.general.grupo = g.clone();
            app.grupos[app.editor_inst] = g;
        }
        Mensaje::NuevoGrupo => {
            app.edicion_grupo = Some(EdicionGrupo { original: None, texto: String::new(), mover: None });
        }
        Mensaje::NuevoGrupoParaInstancia => {
            let Some(i) = app.menu.or(app.seleccionada) else { return Task::none(); };
            app.menu = None;
            app.edicion_grupo = Some(EdicionGrupo { original: None, texto: String::new(), mover: Some(i) });
        }
        Mensaje::RenombrarGrupo(nombre) => {
            app.edicion_grupo = Some(EdicionGrupo { texto: nombre.clone(), original: Some(nombre), mover: None });
        }
        Mensaje::TextoGrupo(t) => {
            if let Some(e) = app.edicion_grupo.as_mut() {
                e.texto = t;
            }
        }
        Mensaje::CancelarGrupo => app.edicion_grupo = None,
        Mensaje::GuardarGrupo => {
            if let Some(e) = app.edicion_grupo.take() {
                let nuevo = e.texto.trim().to_owned();
                let repetido = app.lista_grupos.iter().any(|g| g.to_lowercase() == nuevo.to_lowercase() && Some(g) != e.original.as_ref());
                if nuevo.is_empty() {
                    app.avisar("El grupo necesita un nombre.".into());
                    app.edicion_grupo = Some(e);
                } else if repetido {
                    app.avisar(format!("Ya existe un grupo llamado {nuevo}."));
                    app.edicion_grupo = Some(e);
                } else {
                    arrancar(app);
                    match e.original {
                        Some(viejo) => {
                            if let Some(pos) = app.lista_grupos.iter().position(|g| *g == viejo) {
                                app.lista_grupos[pos] = nuevo.clone();
                            }
                            for g in app.grupos.iter_mut().chain(app.cerrados.iter_mut()) {
                                if g.as_ref() == Some(&viejo) {
                                    *g = Some(nuevo.clone());
                                }
                            }
                            if app.general.grupo.as_ref() == Some(&viejo) {
                                app.general.grupo = Some(nuevo.clone());
                            }
                            app.avisar(format!("Grupo renombrado a {nuevo}."));
                        }
                        None => {
                            app.lista_grupos.push(nuevo.clone());
                            if let Some(i) = e.mover {
                                app.grupos[i] = Some(nuevo.clone());
                                app.avisar(format!("{} ahora está en el grupo {nuevo}.", instancias()[i].nombre));
                            } else {
                                app.avisar(format!("Grupo {nuevo} creado."));
                            }
                        }
                    }
                }
            }
        }
        Mensaje::EliminarGrupo(nombre) => {
            arrancar(app);
            app.lista_grupos.retain(|g| *g != nombre);
            app.cerrados.retain(|g| g.as_ref() != Some(&nombre));
            let mut afectadas = 0;
            for g in app.grupos.iter_mut() {
                if g.as_ref() == Some(&nombre) {
                    *g = None;
                    afectadas += 1;
                }
            }
            if app.general.grupo.as_ref() == Some(&nombre) {
                app.general.grupo = None;
            }
            app.avisar(format!("Grupo {nombre} eliminado; {afectadas} instancias quedaron sin grupo."));
        }
        Mensaje::Ancho(v) => app.general.ancho = v,
        Mensaje::Alto(v) => app.general.alto = v,
        Mensaje::Completa(v) => app.general.pantalla_completa = v,
        Mensaje::Servidor(v) => app.general.servidor = Some(v),
        Mensaje::JavaVersion(v) => { app.configs[app.editor_inst].java = v; app.configs[app.editor_inst].java_requiere_seleccion=false; app.configs[app.editor_inst].heredar = false; },
        Mensaje::MemMin(v) => {
            let c = &mut app.configs[app.editor_inst];
            c.heredar_memoria = false;
            c.memoria_min = v;
            c.memoria_max = c.memoria_max.max(v);
        }
        Mensaje::MemMax(v) => {
            let c = &mut app.configs[app.editor_inst];
            c.heredar_memoria = false;
            c.memoria_max = v;
            c.memoria_min = c.memoria_min.min(v);
        }
        Mensaje::Metaspace(v) => {app.configs[app.editor_inst].heredar_memoria=false;app.configs[app.editor_inst].metaspace = v;},
        Mensaje::ArgsJvm(v) => {app.configs[app.editor_inst].heredar_memoria=false;app.configs[app.editor_inst].argumentos = v;},
        Mensaje::Mundo(i) => app.mundo = i,
        Mensaje::ElegirArchivo(i) => app.exp.elegir(i),
        Mensaje::EditarArchivo(a) => app.exp.editar(a),
        Mensaje::NombreArchivo(n) => app.exp.escribir_nombre(n),
        Mensaje::AccionArchivo(a) => {
            if !app.servicios.demo && matches!(a,AccionArchivo::ConfirmarNombre | AccionArchivo::Duplicar | AccionArchivo::Papelera | AccionArchivo::Guardar)
                && app.servicios.instancias.registros.get(app.editor_inst).is_some_and(|i| app.servicios.juego.ocupada(&i.id)) {
                app.avisar("Espera a que termine la instalación o cierra el juego antes de cambiar sus archivos.".into());
                return Task::none();
            }
            match a {
            AccionArchivo::Abrir => app.exp.abrir_en_sistema(),
            AccionArchivo::AbrirCarpeta => app.exp.abrir_carpeta(),
            AccionArchivo::Renombrar => {
                app.exp.iniciar_renombrar();
                if app.exp.renombrando.is_some() { return iced::widget::text_input::focus("archivo-nombre"); }
            },
            AccionArchivo::ConfirmarNombre => app.exp.confirmar_nombre(),
            AccionArchivo::CancelarNombre => app.exp.cancelar_nombre(),
            AccionArchivo::Duplicar => app.exp.duplicar(),
            AccionArchivo::Papelera => app.exp.a_la_papelera(),
            AccionArchivo::Guardar => app.exp.guardar(),
            AccionArchivo::Descartar => app.exp.descartar(),
            AccionArchivo::CopiarRuta => {
                if let Some(ruta) = app.exp.ruta_texto() {
                    return iced::clipboard::write(ruta);
                }
            }
            }
        },
        Mensaje::FiltroConsola(i) => app.filtro_consola = i,
        Mensaje::BuscarConsola(v) => app.busqueda_consola = v,
        Mensaje::NivelConsola(i) => {
            if let Some(nivel) = app.niveles_consola.get_mut(i) { *nivel = !*nivel; }
            app.desplazamiento_consola = Default::default();
            if app.autoscroll_consola { return juego_real::seguir_consola(app); }
        }
        Mensaje::AutoscrollConsola(activo) => {
            app.autoscroll_consola = activo;
            if activo { return juego_real::seguir_consola(app); }
        }
        Mensaje::DesplazadoConsola(d) => {
            let subio = d.offset + 2. < app.desplazamiento_consola.offset;
            app.desplazamiento_consola = d;
            if d.esta_al_final { app.autoscroll_consola = true; }
            else if subio { app.autoscroll_consola = false; }
            else if app.autoscroll_consola { return juego_real::seguir_consola(app); }
        }
        Mensaje::FinalConsola => {
            app.autoscroll_consola = true;
            return juego_real::seguir_consola(app);
        }
        Mensaje::Capt(a) => return capt::actualizar(app, a),
        Mensaje::Ir(s) => {
            if app.pantalla == Some(pantallas::Pantalla::Sincronizar) {
                sincronizacion_real::salir(app);
            }
            if !app.servicios.demo && app.editor.is_some() {
                let nombre = app.general.nombre.clone();
                if let Err(error) = instancias_reales::renombrar(app, &nombre) {
                    app.avisar(error); return Task::none();
                }
            }
            app.servicios.reset_catalog();
            app.seccion = s;
            if matches!(app.pantalla, Some(pantallas::Pantalla::Cuenta | pantallas::Pantalla::Estudio | pantallas::Pantalla::Fallo)) {
                app.pantalla = None;
            }
            app.sesion.menu = false;
            arrancar(app);
            app.editor = None;
            app.tienda_cat.clear();
            app.tienda_lado = 2;
            if s == Seccion::Modpacks && !app.servicios.demo { return servicios::buscar(app, 0, false); }
            if s == Seccion::Servidores && !app.servicios.demo { return servidores_reales::actualizar(app,servidores_reales::Accion::Abrir); }
            if s == Seccion::Capturas { return capt::refrescar(app); }
            if s == Seccion::Consola && !app.servicios.demo { return juego_real::cargar_consola(app); }
        }
        Mensaje::Filtro(i) => {
            app.filtro = i;
            arrancar(app);
        }
        Mensaje::Menu(i) => {
            app.menu = i;
            app.menu_grupos = false;
            if i.is_some() && !app.reducir {
                app.anim_menu = Some(std::time::Instant::now());
                app.ahora = std::time::Instant::now();
            }
            app.menu_en = app.cursor;
        }
        Mensaje::VerGrupos(v) => app.menu_grupos = v,
        Mensaje::BuscarMods(t) => app.busqueda_mods = t,
        Mensaje::AlternarMod(archivo) => mods::alternar(&con_instancias(|l| l[app.editor_inst].nombre.clone()), &archivo),
        Mensaje::Pant(a) => return pantallas::actualizar(app, a),
        Mensaje::Ses(a) => return sesion::actualizar(app, a),
        Mensaje::CuentaReal(a) => return cuenta_real::actualizar(app, a),
        Mensaje::FiltroMods(i) => {
            app.mods_filtro = i;
            arrancar(app);
        }
        Mensaje::PanelMod(id) => {
            if !app.servicios.demo {
                if let Some(project) = id {
                    let task = servicios::actualizar(app, servicios::Accion::Proyecto(project.clone()));
                    app.mod_panel = Some(project.clone());
                    app.anim_panel = Some(std::time::Instant::now());
                    return task;
                }
                app.mod_panel = None;
                return servicios::actualizar(app, servicios::Accion::CerrarPanel);
            }
            if id.is_some() && !app.reducir {
                app.anim_panel = Some(std::time::Instant::now());
                app.ahora = std::time::Instant::now();
            }
            app.mod_panel = id;
        }
        Mensaje::TiendaAgregar(id) => {
            if !app.tienda_lista.contains(&id) {
                app.tienda_lista.push(id);
            }
        }
        Mensaje::TiendaQuitar(id) => app.tienda_lista.retain(|i| *i != id),
        Mensaje::TiendaVaciar => app.tienda_lista.clear(),
        Mensaje::TiendaCat(k) => {
            if let Some(pos) = app.tienda_cat.iter().position(|c| *c == k) {
                app.tienda_cat.remove(pos);
            } else {
                app.tienda_cat.push(k);
            }
        }
        Mensaje::TiendaLado(i) => app.tienda_lado = i,
        Mensaje::TiendaOrden(i) => app.tienda_orden = i,
        Mensaje::TiendaInstalar => {
            let inst = con_instancias(|l| l[app.editor_inst].nombre.clone());
            let (mc, cargador) = juego_de_instancia(app);
            let mut hechos = 0;
            for id in app.tienda_lista.clone() {
                if let Some(c) = mods::en_catalogo(&id) {
                    let puestos = mods::listar(&inst);
                    for dep in c.dependencias.iter().copied().chain(std::iter::once(c.id)) {
                        if puestos.iter().all(|m| m.id != dep) {
                            if let Some(d) = mods::en_catalogo(dep) {
                                let v = compatibles(d, &mc, &cargador).first().map_or(d.lanzamientos[0].version, |l| l.version);
                                mods::instalar(&inst, dep, v);
                                hechos += 1;
                            }
                        }
                    }
                }
            }
            app.tienda_lista.clear();
            app.mods_vista = 0;
            app.avisar(format!("{hechos} instalado(s) en {inst}."));
            arrancar(app);
        }
        Mensaje::VistaMods(v) => {
            app.mods_vista = v;
            arrancar(app);
            if !app.servicios.demo {
                if v == 1 { app.servicios.reset_catalog(); app.servicios.order=icaro_core::modrinth::Order::Downloads; return servicios::buscar(app, 0, false); }
                app.servicios.reset_catalog();
            }
        }
        Mensaje::AbrirModPagina(id) => {
            if !app.servicios.demo {
                if app.mods_vista == 0 { app.servicios.reset_catalog(); }
                app.mods_vista = 1;
                app.mod_pagina = None;
                return servicios::actualizar(app, servicios::Accion::Proyecto(id));
            }
            app.mod_pagina = Some(id);
            app.mod_pestana = 0;
            app.mod_filtro_mc = "Todas".into();
            app.mod_filtro_loader = "Todos".into();
            app.mod_betas = false;
            app.mod_galeria = 0;
            app.mod_opcionales.clear();
            arrancar(app);
        }
        Mensaje::CerrarModPagina => {
            app.mod_pagina = None;
            arrancar(app);
        }
        Mensaje::FiltroMcMod(v) => app.mod_filtro_mc = v,
        Mensaje::FiltroLoaderMod(v) => app.mod_filtro_loader = v,
        Mensaje::BetasMod => app.mod_betas = !app.mod_betas,
        Mensaje::GaleriaMod(i) => app.mod_galeria = i,
        Mensaje::OpcionalMod(id) => {
            if let Some(pos) = app.mod_opcionales.iter().position(|o| *o == id) {
                app.mod_opcionales.remove(pos);
            } else {
                app.mod_opcionales.push(id);
            }
        }
        Mensaje::PestanaMod(i) => {
            app.mod_pestana = i;
            arrancar(app);
        }
        Mensaje::VersionMod(archivo, version) => {
            let inst = con_instancias(|l| l[app.editor_inst].nombre.clone());
            let id = mods::listar(&inst).into_iter().find(|m| m.archivo == archivo).map(|m| m.id);
            if let Some(id) = id {
                if mods::cambiar_version(&inst, &archivo, &id, &version).is_some() {
                    app.avisar(format!("{id} cambió a la versión {version}."));
                }
            }
        }
        Mensaje::InstalarMod(id, version) => {
            let inst = con_instancias(|l| l[app.editor_inst].nombre.clone());
            mods::instalar(&inst, &id, &version);
            // Dependencias obligatorias que falten y opcionales marcadas.
            if let Some(c) = mods::en_catalogo(&id) {
                let puestos = mods::listar(&inst);
                let opcionales = app.mod_opcionales.clone();
                for dep in c.dependencias.iter().copied().chain(opcionales.iter().map(String::as_str)) {
                    if puestos.iter().all(|m| m.id != dep) {
                        if let Some(d) = mods::en_catalogo(dep) {
                            mods::instalar(&inst, dep, d.lanzamientos[0].version);
                        }
                    }
                }
            }
            app.mod_opcionales.clear();
            app.avisar(format!("{id} {version} instalado."));
        }
        Mensaje::EliminarMod(archivo) => {
            mods::eliminar(&con_instancias(|l| l[app.editor_inst].nombre.clone()), &archivo);
            app.avisar(format!("{archivo} se movió a la papelera."));
        }
        Mensaje::Cuadro(t) => {
            // Las capturas de las fases del Umbral usan un reloj fijo.
            if std::env::var_os("ICARO_CAPTURA_DISENO").is_some()
                && std::env::var("ICARO_VISTA_DISENO").is_ok_and(|v| v.starts_with("juego-umbral") || v=="picar")
            {
                return Task::none();
            }
            app.ahora = t;
            app.red.tick(t, app.simultaneas);
            avanzar_instalaciones(app);
            avanzar_picar(app);
            sesion::avanzar(app);
            if !app.servicios.demo && !app.servicios.instancias.operando {
                if let Some(p) = &app.picar {
                    if p.rota.is_some_and(|t| ms_desde(t, app.ahora) >= icaro_ui::movimiento::PICAR_ROTURA_MS) {
                        if let Some(record) = app.servicios.instancias.registros.get(p.instancia) {
                            app.servicios.instancias.eliminar = Some(record.id.clone());
                            return instancias_reales::eliminar(app);
                        }
                    }
                }
            }
        }
        Mensaje::Jugar(i) => {
            if !app.servicios.demo {
                let jugar = instancias().get(i).is_some_and(|d| matches!(d.estado, EstadoInstancia::Lista | EstadoInstancia::Desactualizada | EstadoInstancia::Jugando));
                return juego_real::iniciar(app,i,jugar);
            }
            // Una sesión vencida muestra el aviso en lugar de abrir el Umbral.
            let _ = sesion::actualizar(app, sesion::Accion::Jugar);
            if app.sesion.banner_jugar {
                return Task::none();
            }
            // El Umbral solo se dispara desde Jugar, nunca al abrir el launcher.
            let ahora = std::time::Instant::now();
            app.menu = None;
            app.seleccionada = Some(i);
            app.umbral = Some(ahora);
            app.ahora = ahora;
        }
        Mensaje::UmbralVolver => {
            if app.servicios.demo {
                app.umbral = None;
            } else {
                return juego_real::volver_umbral(app);
            }
        }
        Mensaje::Picar(picar_real::Accion::Revisada(id,seq,r)) => {
            if seq!=app.picar_solicitud || app.servicios.instancias.eliminar.as_ref()!=Some(&id) {return Task::none();}
            match r {Ok(rev)=>{app.dialogo=rev.sin_respaldo>0;app.picar_revision=Some(rev);},Err(e)=>{app.picar_objetivo=None;app.servicios.instancias.eliminar=None;app.avisar_error(e);}}
        }
        Mensaje::PicarEmpezar => {
            if app.servicios.instancias.operando || app.servicios.mods.busy.is_some() { return Task::none(); }
            if app.picar_objetivo.is_none() {
                if app.editor.is_some() || hay_dialogo(app) || app.seccion!=Seccion::Instancias {return Task::none();}
                return actualizar_estado(app,Mensaje::PedirEliminar);
            }
            if app.dialogo || app.picar_revision.is_none() {return Task::none();}
            let Some(instancia)=app.picar_objetivo else {return Task::none();};
            if !app.servicios.demo && !app.servicios.instancias.registros.get(instancia).is_some_and(|i|Some(&i.id)==app.servicios.instancias.eliminar.as_ref()&&!app.servicios.juego.ocupada(&i.id)){return Task::none();}
            let reintento=app.picar.as_ref().is_some_and(|p|p.soltada.is_some()&&p.rota.is_none());
            if app.picar.is_none()||reintento {let ahora=std::time::Instant::now();app.picar=Some(Picar{instancia,inicio:ahora,soltada:None,rota:None});app.ahora=ahora;}
        }
        Mensaje::PicarSoltar => {
            let ahora = std::time::Instant::now();
            if let Some(p) = &app.picar {
                if p.rota.is_none() && p.soltada.is_none() {
                    let etapa = etapa_picar(p, ahora);
                    if let Some(p) = &mut app.picar {
                        p.soltada = Some((ahora, etapa));
                    }
                }
            }
        }
        Mensaje::FiltroModpacks(i) => {
            app.modpack_filtro = i;
            arrancar(app);
        }
        Mensaje::BuscarModpacks(t) => app.busqueda_modpacks = t,
        Mensaje::AbrirModpack(i) => {
            app.modpack_pagina = Some(i);
            app.modpack_pestana = 0;
            app.modpack_galeria = 0;
            arrancar(app);
        }
        Mensaje::CerrarModpack => {
            app.modpack_pagina = None;
            arrancar(app);
        }
        Mensaje::PestanaModpack(i) => {
            app.modpack_pestana = i;
            arrancar(app);
        }
        Mensaje::GaleriaModpack(i) => app.modpack_galeria = i,
        Mensaje::InstalarModpack(i) => {
            let m = &modpacks::CATALOGO[i];
            if con_instancias(|l| l.iter().any(|d| d.nombre == m.nombre)) {
                return Task::none();
            }
            lista_instancias().lock().unwrap().push(DatosInstancia {
                nombre: m.nombre.into(),
                version: format!("{} · {}", m.minecraft, m.cargador),
                ultima_vez: "Nueva".into(),
                mods: format!("{} mods", m.mods),
                portada: m.portada,
                estado: EstadoInstancia::Instalando(0.0),
                seleccionada: false,
                detalle: format!("Descargando mods · 0 de {}", m.mods),
            });
            app.grupos.push(None);
            app.configs.push(config_por_defecto());
            app.instalando.push((m.nombre.to_owned(), std::time::Instant::now()));
            app.ahora = std::time::Instant::now();
            app.avisar(format!("Instalando {} como una instancia nueva.", m.nombre));
        }
        Mensaje::Cursor(c) => {
            app.cursor = c;
            app.red.mover(c.y);
            app.servicios.servidores.mover(c.y);
        }
        Mensaje::Red(m) => {
            if let Some(a) = app.red.actualizar(m, app.cursor.y, std::time::Instant::now()) {
                app.avisar(a);
            }
            app.ahora = std::time::Instant::now();
        }
        Mensaje::Tamano(s) => {
            // DisenoAngosto original: 64 px y 24 px de histéresis al redimensionar.
            app.barra_angosta = if app.barra_angosta {s.width < 1112.} else {s.width < 1088.};
            app.ventana = s;
            return window::get_oldest().and_then(window::get_maximized).map(Mensaje::Maximizada);
        }
        Mensaje::Maximizada(maximizada) => app.maximizada = maximizada,
        Mensaje::Redimensionar(borde) => return ventana::redimensionar(borde),
        Mensaje::PedirEliminar => {
            if app.servicios.instancias.operando || app.servicios.mods.busy.is_some() {return Task::none();}
            let Some(i)=app.menu.or(app.seleccionada) else{return Task::none();};
            if !app.servicios.demo {
                let Some(record)=app.servicios.instancias.registros.get(i) else{return Task::none();};
                if app.servicios.juego.ocupada(&record.id) {app.avisar_error("Detén el juego o su instalación antes de eliminar esta instancia.".into());return Task::none();}
                app.servicios.instancias.eliminar=Some(record.id.clone());
            }
            app.menu=None;app.picar_objetivo=Some(i);app.picar=None;app.dialogo=false;app.picar_revision=None;app.picar_solicitud=app.picar_solicitud.wrapping_add(1);
            if app.servicios.demo {app.picar_revision=Some(picar_real::Revision::default());}
            else {return picar_real::revisar(app,i,app.picar_solicitud);}
        }
        Mensaje::CerrarDialogo => {
            if app.servicios.instancias.operando || app.picar.as_ref().is_some_and(|p|p.rota.is_some()){return Task::none();}
            app.dialogo=false;app.picar=None;app.picar_objetivo=None;app.picar_revision=None;app.picar_solicitud=app.picar_solicitud.wrapping_add(1);app.servicios.instancias.eliminar=None;
        }
        Mensaje::Eliminar => {app.dialogo=false;}
        Mensaje::Escape => {
            if app.servicios.local.abierta {
                return cuenta_sin_conexion::actualizar(app, cuenta_sin_conexion::Accion::Cerrar);
            }
            if app.pantalla==Some(pantallas::Pantalla::Sincronizar)&&!app.servicios.demo{return sincronizacion_real::actualizar(app,sincronizacion_real::Accion::Cerrar);}
            if app.servicios.servidores.arrastre.is_some(){return servidores_reales::actualizar(app,servidores_reales::Accion::CancelarOrden);}
            if app.servicios.servidores.panel_abierto(){return servidores_reales::actualizar(app,servidores_reales::Accion::Cerrar);}
            if app.pantalla==Some(pantallas::Pantalla::Red)&&!app.servicios.demo{return red_privada_real::actualizar(app,red_privada_real::Accion::Volver);}
            if app.servicios.modpacks.abierto{return modpacks_reales::actualizar(app,modpacks_reales::Accion::Cerrar);}
            if app.exportar_lista.dialogo.is_some(){return exportar_lista_mods::actualizar(app,exportar_lista_mods::Accion::Cerrar);}
            if app.iconos.dialogo.is_some(){return icono_instancia::actualizar(app,icono_instancia::Accion::Cerrar);}
            if app.picar_objetivo.is_some(){return actualizar_estado(app,Mensaje::CerrarDialogo);}
            if app.busqueda.abierta {app.busqueda.abierta=false;}else if app.mod_panel.is_some(){app.mod_panel=None;}else if let Some(id)=app.avisos.ultimo(){app.avisos.cerrar(id,std::time::Instant::now());}
        }
        Mensaje::CerrarAvisoId(id) => {if app.reducir{app.avisos.quitar(id);}else{app.avisos.cerrar(id,std::time::Instant::now());}}
        Mensaje::PausarAviso(id,pausa) => app.avisos.pausar(id,pausa,std::time::Instant::now()),
        Mensaje::AccionAviso(id,accion) => {app.avisos.quitar(id);return actualizar_estado(app,*accion);}
        // El detalle muestra el error del aviso, no la falla genérica de la tienda.
        Mensaje::DetalleAviso(texto) => {app.detalle_error=Some(texto);app.pantalla=Some(pantallas::Pantalla::ErrorTienda);}
        Mensaje::AvisoTic(ahora) => {
            app.avisos.actualizar(ahora);
            app.ahora = ahora;
            if app.aviso.as_ref().is_some_and(|a| a.vencido(ahora)) { app.aviso = None; }
        }
        Mensaje::CerrarAviso => {
            if let Some(id)=app.avisos.ultimo(){app.avisos.cerrar(id,std::time::Instant::now());}
            if app.reducir { app.aviso = None; }
            else if let Some(aviso) = &mut app.aviso {
                app.ahora = std::time::Instant::now();
                aviso.cerrar(app.ahora);
            }
        }
        Mensaje::DeshacerAviso => {
            if !app.servicios.demo { return instancias_reales::deshacer(app); }
            app.aviso = None;
            // Deshacer una eliminación por Picar: la tarjeta vuelve y se asienta.
            if app.picar.as_ref().is_some_and(|p| p.rota.is_some()) {
                if let Some(p) = app.picar.take() {
                    let ahora = std::time::Instant::now();
                    app.colocada = Some((p.instancia, ahora));
                    app.ahora = ahora;
                }
            }
        }
        Mensaje::ComprobarTemaSistema => { if app.tema==2 { return apariencia_real::comprobar(); } }
        Mensaje::Verificacion(a) => return verificacion_real::actualizar(app,a),
        Mensaje::ExportarLista(a) => return exportar_lista_mods::actualizar(app,a),
        Mensaje::Icono(a) => return icono_instancia::actualizar(app,a),
        Mensaje::IconoEvento(e) => {if let Some(a)=icono_instancia::evento(app,&e){return icono_instancia::actualizar(app,a);}}
        Mensaje::ModpackEvento(e) => {if let Some(a)=modpacks_reales::evento(app,e){return modpacks_reales::actualizar(app,a);}}
        Mensaje::TemaSistema(claro) => { if app.tema==2 {app.modo=if claro {Modo::Piedra}else{Modo::Tinta};Lamina::precargar(app.modo);} }
        Mensaje::Tema(i) => {
            app.tema = i.min(2);
            app.modo = if i == 1 { Modo::Piedra } else { Modo::Tinta };
            Lamina::precargar(app.modo);
            if app.tema==2 { return apariencia_real::comprobar(); }
        }
        Mensaje::Reducir => {
            app.reducir = !app.reducir;
            icaro_ui::movimiento::fijar_reducir(app.reducir);
        }
        Mensaje::Simultaneas(n) => app.simultaneas = n.clamp(1, 8),
        Mensaje::Ajuste(i) => {app.ajuste = i;if i==3 {return almacenamiento_real::actualizar(app,almacenamiento_real::Accion::Medir);}},
        Mensaje::Arrastrar => {
            // Doble clic en la barra: maximizar o restaurar.
            let ahora = std::time::Instant::now();
            if app.ultimo_clic.is_some_and(|u| ahora.duration_since(u).as_millis() < 400) {
                app.ultimo_clic = None;
                return window::get_oldest().and_then(window::toggle_maximize);
            }
            app.ultimo_clic = Some(ahora);
            return window::get_oldest().and_then(window::drag);
        }
        Mensaje::Minimizar => {
            return window::get_oldest().and_then(|id| window::minimize(id, true))
        }
        Mensaje::Maximizar => return window::get_oldest().and_then(window::toggle_maximize),
        Mensaje::Cerrar => {
            if !actualizacion_real::al_cerrar(app) { return Task::none(); }
            if !app.servicios.demo && app.editor.is_some() {
                let nombre = app.general.nombre.clone();
                if let Err(error) = instancias_reales::renombrar(app, &nombre) {
                    app.avisar(error); return Task::none();
                }
            }
            app.servicios.cancel();
            return if app.servicios.demo {window::get_oldest().and_then(window::close)}else{juego_real::cerrar(app)};
        }
        Mensaje::Nada | Mensaje::ImagenesActualizadas => {}
    }
    Task::none()
}

fn instancias_base() -> Vec<DatosInstancia> {
    let nueva = |nombre: &str, version: &str, vez: &str, mods: &str, portada, estado, detalle: &str| {
        DatosInstancia {
            nombre: nombre.into(),
            version: version.into(),
            ultima_vez: vez.into(),
            mods: mods.into(),
            portada,
            estado,
            seleccionada: false,
            detalle: detalle.into(),
        }
    };
    vec![
        nueva("Supervivencia", "1.21.4 · Fabric", "Hace 2 h", "214 mods", Lamina::CaballeroCastillo, EstadoInstancia::Lista, ""),
        nueva("Better MC", "1.20.1 · NeoForge", "Hace 6 días", "312 mods", Lamina::CastilloTorres, EstadoInstancia::Desactualizada, ""),
        nueva("Skyblock", "1.21.4 · Quilt", "Nueva", "48 mods", Lamina::IcaroDedalo, EstadoInstancia::Instalando(0.58), "Descargando librerías · 112 de 186"),
        nueva("Mundo de Javier", "1.21.1 · Fabric", "Ahora", "96 mods", Lamina::ProdigoAldea, EstadoInstancia::Jugando, "En ejecución · 00:42:17"),
        nueva("Vanilla", "1.21.4 · Vanilla", "Hace 1 mes", "Sin mods", Lamina::ValleRocas, EstadoInstancia::Error, "Falta Java 21"),
    ]
}

/// Lista viva de instancias. Los ejemplos se cargan solo en el modo de demostración.
fn lista_instancias() -> &'static std::sync::Mutex<Vec<DatosInstancia>> {
    static LISTA: std::sync::OnceLock<std::sync::Mutex<Vec<DatosInstancia>>> = std::sync::OnceLock::new();
    LISTA.get_or_init(|| std::sync::Mutex::new(Vec::new()))
}

fn instancias() -> Vec<DatosInstancia> {
    lista_instancias().lock().unwrap().clone()
}

/// Consulta la lista sin copiarla, para vistas que se reconstruyen en cada
/// cuadro. `f` no debe volver a llamar a `instancias()`.
fn con_instancias<R>(f: impl FnOnce(&[DatosInstancia]) -> R) -> R {
    f(&lista_instancias().lock().unwrap())
}

fn config_por_defecto() -> ConfigJava {
    ConfigJava {
        java: None,
        heredar: true,
        heredar_memoria: true,
        java_requiere_seleccion: false,
        descarga: true,
        memoria_min: 2048,
        memoria_max: 6144,
        metaspace: 512,
        argumentos: "-XX:+UseG1GC".into(),
    }
}

/// Hace avanzar las instalaciones de modpacks en curso (5 s de ejemplo).
fn avanzar_instalaciones(app: &mut App) {
    let ahora = app.ahora;
    let mut terminadas = Vec::new();
    {
        let mut lista = lista_instancias().lock().unwrap();
        for (nombre, inicio) in &app.instalando {
            let avance = (ahora.saturating_duration_since(*inicio).as_secs_f32() / 5.0).min(1.0);
            if let Some(d) = lista.iter_mut().find(|d| &d.nombre == nombre) {
                let total = modpacks::CATALOGO.iter().find(|m| m.nombre == nombre).map_or(0, |m| m.mods);
                if avance >= 1.0 {
                    d.estado = EstadoInstancia::Lista;
                    d.detalle.clear();
                    d.ultima_vez = "Ahora".into();
                    terminadas.push(nombre.clone());
                } else {
                    d.estado = EstadoInstancia::Instalando(avance);
                    d.detalle = format!("Descargando mods · {} de {total}", (avance * total as f32) as u32);
                }
            }
        }
    }
    if let Some(n) = terminadas.last() {
        app.avisar(format!("{n} quedó instalado."));
    }
    app.instalando.retain(|(n, _)| !terminadas.contains(n));
}

/// Avanza la eliminación por Picar: rompe la tarjeta al llegar a la décima
/// etapa y retira las grietas si se soltó antes.
fn avanzar_picar(app: &mut App) {
    let ahora = app.ahora;
    let Some(p) = &app.picar else { return };
    if p.rota.is_some() {
        return;
    }
    let etapa = etapa_picar(p, ahora);
    if p.soltada.is_some() {
        if etapa == 0 {
            app.picar = None;
        }
    } else if etapa >= icaro_ui::movimiento::PICAR_ETAPAS {
        let nombre = con_instancias(|l| l.get(p.instancia).map(|d| d.nombre.clone())).unwrap_or_default();
        if let Some(p) = &mut app.picar {
            p.rota = Some(ahora);
        }
        // Deshacer dura mientras el aviso siga abierto.
        if app.servicios.demo { app.avisar(format!("{nombre} eliminada.")); }
    }
}

/// Envuelve una tarjeta con las capas de Picar y Colocar que le correspondan.
fn con_efectos<'a>(app: &App, i: usize, nombre: &str, tarjeta: Element<'a, Mensaje>) -> Element<'a, Mensaje> {
    use icaro_ui::efectos::{self, FasePicar};
    use icaro_ui::instancias::{ALTO_TARJETA, ANCHO_TARJETA};
    let p = app.paleta();
    if let Some(pi) = app.picar.as_ref().filter(|pi| pi.instancia == i) {
        return match pi.rota {
            None => iced::widget::stack![tarjeta, efectos::picar(p, nombre, FasePicar::Grietas(etapa_picar(pi, app.ahora)))].into(),
            Some(t) if app.reducir => picar_real::fundido(p,tarjeta,ms_desde(t,app.ahora)),
            Some(t) => iced::widget::stack![
                Space::new(ANCHO_TARJETA, ALTO_TARJETA),
                efectos::picar(p, nombre, FasePicar::Rota(ms_desde(t, app.ahora)))
            ]
            .into(),
        };
    }
    match app.colocada {
        Some((c, t)) if c == i => {
            let avance = (ms_desde(t, app.ahora) / icaro_ui::tema::duracion::FAST as f32).clamp(0.0, 1.0);
            iced::widget::stack![tarjeta, efectos::colocar(p, avance, ANCHO_TARJETA, ALTO_TARJETA)].into()
        }
        _ => tarjeta,
    }
}

/// Contenido desplazable para las pantallas de otro módulo.
fn desplazable_pub<'a>(e: Element<'a, Mensaje>) -> Element<'a, Mensaje> {
    desplazable(e)
}

fn suave<'a>(app: &'a App, cuerpo: impl Into<Element<'a, Mensaje>>) -> Element<'a, Mensaje> {
    con_entrada(app, desplazable(cuerpo))
}

fn pantalla_instancias(app: &App) -> Element<'_, Mensaje> {
    let p = app.paleta();
    if let Some(pestana) = app.editor {
        return pantalla_editor(app, pestana);
    }
    if app.capt.vacio || con_instancias(|l| l.is_empty()) {
        return capt::vacio_instancias(app);
    }
    let mut chips = row![].spacing(espacio::S2);
    for (i, f) in ["Todas", "Fabric", "NeoForge", "Vanilla"].iter().enumerate() {
        chips = chips.push(componentes::chip(p, f, app.filtro == i, Mensaje::Filtro(i)));
    }
    let herramientas = row![
        chips,
        Space::with_width(Length::Fill),
        componentes::boton(p, "Nuevo grupo", Variante::Secundario, Some(Mensaje::NuevoGrupo)),
        componentes::boton(p, "Importar", Variante::Secundario, Some(Mensaje::Pant(pantallas::Accion::Importar))),
        componentes::boton(p, "Crear instancia", Variante::Primario, Some(Mensaje::Pant(pantallas::Accion::NuevaInstancia))),
    ]
    .spacing(espacio::S2)
    .align_y(Alignment::Center);

    // Columnas que caben: barra lateral, márgenes y separación entre tarjetas.
    let disponible = app.ventana.width - 232.0 - 2.0 * espacio::S12 + espacio::S6;
    let columnas = ((disponible / (icaro_ui::instancias::ANCHO_TARJETA + espacio::S6)).floor() as usize).max(1);
    let todas = instancias();
    let filtro = ["", "fabric", "neoforge", "vanilla"][app.filtro.min(3)];
    let visible = |i: usize| filtro.is_empty() || todas[i].version.to_lowercase().contains(filtro);
    let mut cuadricula = column![].spacing(espacio::S8);
    let mut etiquetas: Vec<Option<String>> = app.lista_grupos.iter().cloned().map(Some).collect();
    etiquetas.push(None);
    let hay_grupos = etiquetas.len() > 1;
    for etiqueta in etiquetas {
        let miembros: Vec<usize> = (0..todas.len())
            .filter(|&i| app.grupos[i] == etiqueta && visible(i))
            .collect();
        if miembros.is_empty() && (!filtro.is_empty() || etiqueta.is_none()) {
            continue;
        }
        let mut seccion = column![].spacing(espacio::S4);
        if hay_grupos {
            let cerrado = app.cerrados.contains(&etiqueta);
            seccion = seccion.push(encabezado_grupo(p, etiqueta.clone(), miembros.len(), cerrado));
            if cerrado {
                cuadricula = cuadricula.push(seccion);
                continue;
            }
        }
        if miembros.is_empty() {
            seccion = seccion.push(
                text("Este grupo no tiene instancias. Muévelas desde el menú de sus tres puntos.")
                    .size(texto::BODY.0)
                    .color(p.text_muted),
            );
        }
        let mut tarjetas: Vec<Element<Mensaje>> = Vec::new();
        for i in miembros {
            let mut d = todas[i].clone();
            d.seleccionada = Some(i) == app.seleccionada;
            let tarjeta = icaro_ui::instancias::tarjeta_instancia_con_icono(
                p,
                &d,
                AccionesTarjeta {
                    seleccionar: Mensaje::AbrirInstancia(i),
                    principal: if app.servicios.demo && matches!(d.estado, EstadoInstancia::Error) { Mensaje::Pant(pantallas::Accion::Abrir(pantallas::Pantalla::Fallo)) } else if matches!(d.estado, EstadoInstancia::Lista | EstadoInstancia::Pendiente | EstadoInstancia::Error) { Mensaje::Jugar(i) } else { Mensaje::Nada },
                    mas: Mensaje::Menu(Some(i)),
                },
                Some(icono_instancia::imagen(app,i,64.)),
            );
            tarjetas.push(con_efectos(app, i, &d.nombre, tarjeta));
        }
        let mut it = tarjetas.into_iter();
        let mut rejilla = column![].spacing(espacio::S6);
        loop {
            let mut fila = row![].spacing(espacio::S6);
            let mut n = 0;
            for _ in 0..columnas {
                if let Some(t) = it.next() {
                    fila = fila.push(t);
                    n += 1;
                }
            }
            if n == 0 {
                break;
            }
            rejilla = rejilla.push(fila);
        }
        cuadricula = cuadricula.push(seccion.push(rejilla));
    }
    cuadricula = cuadricula.push(tarjeta_nueva(p, Mensaje::Pant(pantallas::Accion::NuevaInstancia)));
    let mut cuerpo_col = column![herramientas].spacing(espacio::S6);
    if let Some(e) = &app.edicion_grupo {
        cuerpo_col = cuerpo_col.push(
            row![
                container(componentes::campo(
                    p,
                    if e.original.is_some() { "Nuevo nombre del grupo" } else { "Nombre del grupo" },
                    &e.texto,
                    Mensaje::TextoGrupo,
                ))
                .width(320),
                componentes::boton(p, "Guardar grupo", Variante::Primario, Some(Mensaje::GuardarGrupo)),
                componentes::boton(p, "Cancelar", Variante::Fantasma, Some(Mensaje::CancelarGrupo)),
            ]
            .spacing(espacio::S2)
            .align_y(Alignment::Center),
        );
    }
    cuerpo_col = cuerpo_col.push(cuadricula);
    let cuerpo = cuerpo_col
        .spacing(espacio::S6)
        .padding(Padding::from([espacio::S6, espacio::S12]));
    column![
        banda(p, Lamina::CaidaCielo, "Instancias", Some(todas.len())),
        suave(app, cuerpo)
    ]
    .into()
}

fn encabezado_grupo(
    p: icaro_ui::tema::Paleta,
    grupo: Option<String>,
    cantidad: usize,
    cerrado: bool,
) -> Element<'static, Mensaje> {
    use icaro_ui::iconos::{icono, Icono, Tam};
    let nombre = grupo.clone().unwrap_or_else(|| "Sin grupo".to_owned());
    let cabeza = iced::widget::button(
        container(
            row![
                icono(if cerrado { Icono::ChevronDerecha } else { Icono::ChevronAbajo }, Tam::Base, p.text),
                text(nombre.to_uppercase())
                    .font(fuentes::ETIQUETA)
                    .size(texto::LABEL.0 + 2.0)
                    .color(p.text),
                text(cantidad.to_string())
                    .font(fuentes::MONO)
                    .size(texto::MONO.0)
                    .color(p.text_muted),
            ]
            .spacing(espacio::S3)
            .align_y(Alignment::Center),
        )
        .center_y(Length::Fill),
    )
    .height(40)
    .padding(Padding::from([0.0, espacio::S2]))
    .on_press(Mensaje::AlternarGrupo(grupo.clone()))
    .style(icaro_ui::estilo::boton_fantasma(p));
    let mut acciones = row![cabeza, Space::with_width(Length::Fill)].align_y(Alignment::Center);
    if let Some(g) = grupo {
        acciones = acciones
            .push(componentes::boton(p, "Renombrar", Variante::Fantasma, Some(Mensaje::RenombrarGrupo(g.clone()))))
            .push(componentes::boton(p, "Eliminar grupo", Variante::Fantasma, Some(Mensaje::EliminarGrupo(g))));
    }
    column![
        acciones,
        container(Space::new(Length::Fill, 2.0)).style(icaro_ui::estilo::bloque(p.text)),
    ]
    .spacing(espacio::S1)
    .into()
}

/// Pestaña Mods del editor: lista de los mods de la instancia y descarga de
/// nuevos; al pulsar el nombre de un mod se abre su página de detalle.
fn pestana_mods(app: &App) -> Element<'_, Mensaje> {
    match &app.mod_pagina {
        Some(id) => pagina_mod(app, id),
        None => lista_mods(app),
    }
}

/// Versión del juego y loader de la instancia que se edita.
fn juego_de_instancia(app: &App) -> (String, String) {
    let lista = instancias();
    let Some(datos) = lista.get(app.editor_inst) else { return (String::new(), String::new()); };
    let mut partes = datos.version.split(" · ");
    (partes.next().unwrap_or("").to_owned(), partes.next().unwrap_or("").to_owned())
}

/// Publicaciones que sirven para la instancia, de la más nueva a la más antigua.
fn compatibles<'a>(c: &'a mods::ModCatalogo, mc: &str, cargador: &str) -> Vec<&'a mods::Lanzamiento> {
    c.lanzamientos
        .iter()
        .filter(|l| {
            l.canal == "release"
                && l.minecraft == mc
                && (mods::es_recurso() || c.cargadores_de(l).iter().any(|x| x.eq_ignore_ascii_case(cargador)))
        })
        .collect()
}

/// Icono procedural pequeño para un mod, derivado de su identificador.
fn icono_mod<'a>(p: icaro_ui::tema::Paleta, id: &str, lado: f32) -> Element<'a, Mensaje> {
    let mut h: u32 = 2_166_136_261;
    for b in id.bytes() {
        h = (h ^ u32::from(b)).wrapping_mul(16_777_619);
    }
    let celda = lado / 8.0;
    let mut col = column![];
    for _ in 0..8 {
        let mut mitad = [0u32; 4];
        for m in mitad.iter_mut() {
            h ^= h << 13;
            h ^= h >> 17;
            h ^= h << 5;
            *m = h % 3;
        }
        let mut fila = row![];
        for i in 0..8 {
            let color = match mitad[if i < 4 { i } else { 7 - i }] {
                0 => p.surface_sunken,
                1 => p.text_muted,
                _ => p.text,
            };
            fila = fila.push(container(Space::new(celda, celda)).style(icaro_ui::estilo::bloque(color)));
        }
        col = col.push(fila);
    }
    container(col).style(icaro_ui::estilo::marco(p)).into()
}

fn lista_mods(app: &App) -> Element<'_, Mensaje> {
    if app.mods_vista == 1 {
        return tienda_mods(app);
    }
    if !app.servicios.demo && app.editor == Some(2) { return mods_reales::vista(app); }
    if !app.servicios.demo && app.editor == Some(3) { return mods_reales::vista_recursos(app); }
    let p = app.paleta();
    let instancia = con_instancias(|l| l[app.editor_inst].nombre.clone());
    let (mc_inst, cargador_inst) = juego_de_instancia(app);
    let instalados = if app.servicios.demo { mods::listar(&instancia) } else { Vec::new() };
    let consulta = app.busqueda_mods.trim().to_lowercase();
    let coincide = |texto: &str| consulta.is_empty() || texto.to_lowercase().contains(&consulta);

    // Estado de cada mod respecto de la instancia.
    let estado_de = |m: &mods::ModInstalado| -> (u8, Option<&'static str>) {
        let Some(c) = mods::en_catalogo(&m.id) else { return (3, None) };
        let comp = compatibles(c, &mc_inst, &cargador_inst);
        match comp.first() {
            None => (3, None),
            Some(nueva) if nueva.version != m.version => (2, Some(nueva.version)),
            _ => (1, None),
        }
    };

    let nombres = ["Todos", "Activos", "Con actualización", "Con problemas"];
    let mut chips = row![].spacing(espacio::S2);
    for (i, n) in nombres.iter().enumerate() {
        chips = chips.push(componentes::chip(p, n, app.mods_filtro == i, Mensaje::FiltroMods(i)));
    }
    let acciones = row![
        container(componentes::campo(p, "Buscar", &app.busqueda_mods, Mensaje::BuscarMods)).width(Length::Fill),
        text(format!(
            "{}/{} activos",
            instalados.iter().filter(|m| m.activo).count(),
            instalados.len()
        ))
        .font(fuentes::MONO)
        .size(texto::MONO_SM.0)
        .color(p.text_muted)
        .width(Length::Shrink),
        componentes::boton(
            p,
            if mods::es_recurso() { "Descargar packs" } else { "Descargar mods" },
            Variante::Primario,
            Some(Mensaje::VistaMods(1)),
        ),
    ]
    .spacing(espacio::S4)
    .align_y(Alignment::Center);
    let barra = column![acciones, chips].spacing(espacio::S4);

    let mut lista = column![].spacing(espacio::S2);
    let mut n = 0;
    for m in instalados.iter().filter(|m| coincide(&m.archivo)) {
        let (estado, nueva) = estado_de(m);
        let pasa = match app.mods_filtro {
            1 => m.activo,
            2 => estado == 2,
            3 => estado == 3,
            _ => true,
        };
        if !pasa {
            continue;
        }
        n += 1;
        let cat = mods::en_catalogo(&m.id);
        let titulo = cat.map_or_else(|| m.id.clone(), |c| c.titulo.to_owned());
        let tinta = if m.activo { p.text } else { p.text_disabled };
        let nombre = iced::widget::button(
            column![
                text(titulo).font(fuentes::TITULO).size(texto::BODY.0).color(tinta),
                text(cat.map_or("Sin ficha en el catálogo", |c| c.descripcion)).size(texto::BODY_SM.0).color(if m.activo { p.text_muted } else { p.text_disabled }),
            ]
            .spacing(espacio::S1),
        )
        .width(Length::Fill)
        .padding(0)
        .on_press(Mensaje::AbrirModPagina(m.id.clone()))
        .style(icaro_ui::estilo::sin_estilo(tinta));
        let insignia_estado: Element<Mensaje> = match estado {
            2 => icaro_ui::componentes::insignia(p, icaro_ui::componentes::Estado::Info, &format!("Nueva {}", nueva.unwrap_or(""))),
            3 => icaro_ui::componentes::insignia(p, icaro_ui::componentes::Estado::Aviso, "Sin versión para tu instancia"),
            _ => icaro_ui::componentes::insignia(p, icaro_ui::componentes::Estado::Exito, "Compatible"),
        };
        let mut fila = row![
            componentes::interruptor(p, m.activo, Mensaje::AlternarMod(m.archivo.clone())),
            icono_mod(p, &m.id, 40.0),
            nombre,
            insignia_estado,
        ]
        .spacing(espacio::S4)
        .align_y(Alignment::Center);
        if let Some(v) = nueva {
            fila = fila.push(componentes::boton(p, "Actualizar", Variante::Secundario, Some(Mensaje::VersionMod(m.archivo.clone(), v.to_owned()))));
        }
        fila = fila
            .push(text(m.version.clone()).font(fuentes::MONO).size(texto::MONO_SM.0).color(p.text_muted).width(70))
            .push(
                iced::widget::button(icaro_ui::iconos::icono_texto(Icono::Acciones, Tam::Base))
                    .padding(espacio::S2)
                    .on_press(Mensaje::PanelMod(Some(m.id.clone())))
                    .style(icaro_ui::estilo::boton_fantasma(p)),
            );
        lista = lista.push(
            container(fila.padding(Padding::from([14.0, 16.0])))
                .width(Length::Fill)
                .style(icaro_ui::estilo::tarjeta(p, app.mod_panel.as_deref() == Some(m.id.as_str()))),
        );
    }
    if n == 0 {
        lista = lista.push(
            text(if instalados.is_empty() {
                "Esta instancia no tiene nada instalado aquí. Pulsa Descargar para agregar algo."
            } else {
                "Nada coincide con el filtro."
            })
            .size(texto::BODY.0)
            .color(p.text_muted),
        );
    }
    // Las filas mantienen su altura natural; el sobrante se desplaza y recorta.
    desplazable(column![barra, lista]
        .spacing(espacio::S6)
        .padding(Padding::from([espacio::S6, espacio::S12])))
}

/// Tienda: filtros a la izquierda, resultados y barra inferior invertida con
/// la lista de instalación que se arma antes de instalar todo junto.
fn tienda_mods(app: &App) -> Element<'_, Mensaje> {
    if !app.servicios.demo { return servicios::store_view(app); }
    use icaro_ui::componentes::{casilla, etiqueta, insignia, Estado};
    let p = app.paleta();
    let instancia = con_instancias(|l| l[app.editor_inst].nombre.clone());
    let (mc_inst, cargador_inst) = juego_de_instancia(app);
    let instalados = mods::listar(&instancia);
    let consulta = app.busqueda_mods.trim().to_lowercase();

    let mut categorias: Vec<&str> = Vec::new();
    for c in mods::catalogo_actual() {
        for k in c.categorias {
            if !categorias.contains(k) {
                categorias.push(k);
            }
        }
    }
    let mut filtros = column![etiqueta(p, "Fuente"), componentes::segmentado(p, &["Modrinth", "CurseForge"], 0, |_| Mensaje::Nada), Space::with_height(espacio::S3), etiqueta(p, "Categorías")]
        .spacing(espacio::S2)
        .width(190);
    for k in &categorias {
        let marcada = app.tienda_cat.iter().any(|c| c == k);
        let k2 = (*k).to_owned();
        filtros = filtros.push(casilla(p, k, marcada, move |_| Mensaje::TiendaCat(k2.clone())));
    }
    filtros = filtros.push(Space::with_height(espacio::S3)).push(etiqueta(p, "Lado"));
    for (i, n) in ["Cliente y servidor", "Solo cliente"].iter().enumerate() {
        filtros = filtros.push(
            iced::widget::button(
                row![
                    icono(if app.tienda_lado == i { Icono::Check } else { Icono::Carpeta }, Tam::Base, p.text),
                    text(*n).size(texto::BODY_SM.0).color(p.text),
                ]
                .spacing(espacio::S2)
                .align_y(Alignment::Center),
            )
            .padding(0)
            .on_press(Mensaje::TiendaLado(i))
            .style(icaro_ui::estilo::sin_estilo(p.text)),
        );
    }

    let mut resultados = column![].spacing(espacio::S2);
    let mut hay = 0;
    let mut orden: Vec<&mods::ModCatalogo> = mods::catalogo_actual().iter().collect();
    if app.tienda_orden == 1 {
        orden.sort_by(|a, b| a.titulo.cmp(b.titulo));
    }
    for c in orden {
        if !(consulta.is_empty() || c.titulo.to_lowercase().contains(&consulta) || c.descripcion.to_lowercase().contains(&consulta)) {
            continue;
        }
        if !app.tienda_cat.is_empty() && !c.categorias.iter().any(|k| app.tienda_cat.iter().any(|s| s == k)) {
            continue;
        }
        let solo_cliente = mods::extras(c.id).entorno == "Cliente";
        if app.tienda_lado == 1 && !solo_cliente {
            continue;
        }
        hay += 1;
        let puesto = instalados.iter().find(|m| m.id == c.id);
        let en_lista = app.tienda_lista.iter().any(|i| i == c.id);
        let compat = !compatibles(c, &mc_inst, &cargador_inst).is_empty();
        let accion: Element<Mensaje> = if puesto.is_some() {
            insignia(p, Estado::Exito, "Instalado")
        } else if en_lista {
            componentes::boton(p, "En la lista", Variante::Secundario, Some(Mensaje::TiendaQuitar(c.id.to_owned())))
        } else if compat {
            componentes::boton(p, "Agregar", Variante::Secundario, Some(Mensaje::TiendaAgregar(c.id.to_owned())))
        } else {
            insignia(p, Estado::Aviso, "Sin versión para tu instancia")
        };
        let titulo = iced::widget::button(
            row![
                text(c.titulo).font(fuentes::TITULO).size(texto::BODY.0 + 1.0).color(p.text),
                text(c.autor).font(fuentes::MONO).size(texto::MONO_SM.0).color(p.text_muted),
            ]
            .spacing(espacio::S3)
            .align_y(Alignment::Center),
        )
        .padding(0)
        .on_press(Mensaje::PanelMod(Some(c.id.to_owned())))
        .style(icaro_ui::estilo::sin_estilo(p.text));
        let mut chips = row![].spacing(espacio::S2).align_y(Alignment::Center);
        for k in c.categorias.iter().take(2) {
            chips = chips.push(insignia(p, Estado::Info, k));
        }
        chips = chips.push(text(format!("{} descargas", c.descargas)).font(fuentes::MONO).size(texto::MONO_SM.0).color(p.text_muted));
        resultados = resultados.push(
            container(
                row![
                    icono_mod(p, c.id, 48.0),
                    column![titulo, text(c.descripcion).size(texto::BODY_SM.0).color(p.text_muted), chips].spacing(espacio::S1).width(Length::Fill),
                    accion,
                ]
                .spacing(espacio::S4)
                .align_y(Alignment::Center),
            )
            .padding(espacio::S4)
            .width(Length::Fill)
            .style(icaro_ui::estilo::tarjeta(p, en_lista)),
        );
    }
    if hay == 0 {
        resultados = resultados.push(capt::vacio_busqueda(app, &app.busqueda_mods, Mensaje::BuscarMods(String::new())));
    }

    let cabecera = column![
        row![
            componentes::boton(p, "Volver a instalados", Variante::Fantasma, Some(Mensaje::VistaMods(0))),
            text(if mods::es_recurso() { "AGREGAR PACKS" } else { "AGREGAR MODS" }).font(fuentes::DISPLAY).size(texto::DISPLAY.0 - 8.0).color(p.text),
        ]
        .spacing(espacio::S4)
        .align_y(Alignment::Center),
        row![
            chip_dato(p, &format!("{instancia} · {mc_inst} · {cargador_inst}")),
            Space::with_width(Length::Fill),
            container(componentes::campo(p, "Buscar", &app.busqueda_mods, Mensaje::BuscarMods)).width(260),
            componentes::selector(p, vec!["Más descargados".to_owned(), "Nombre".to_owned()], Some(["Más descargados", "Nombre"][app.tienda_orden.min(1)].to_owned()), |v| Mensaje::TiendaOrden(usize::from(v == "Nombre"))),
        ]
        .spacing(espacio::S4)
        .align_y(Alignment::Center),
    ]
    .spacing(espacio::S3);

    let cuerpo = row![
        filtros,
        column![text(format!("{hay} resultados")).font(fuentes::MONO).size(texto::MONO_SM.0).color(p.text_muted), resultados].spacing(espacio::S3).width(Length::Fill),
    ]
    .spacing(espacio::S6);

    let n = app.tienda_lista.len();
    let barra: Element<Mensaje> = if n == 0 {
        Space::with_height(0).into()
    } else {
        let mut nombres: Vec<String> = Vec::new();
        let mut deps: Vec<String> = Vec::new();
        for id in &app.tienda_lista {
            if let Some(c) = mods::en_catalogo(id) {
                nombres.push(c.titulo.to_owned());
                for d in c.dependencias {
                    let titulo = mods::en_catalogo(d).map_or((*d).to_owned(), |x| x.titulo.to_owned());
                    if !instalados.iter().any(|m| m.id == *d) && !app.tienda_lista.iter().any(|i| i == d) && !deps.contains(&titulo) {
                        deps.push(titulo);
                    }
                }
            }
        }
        let mut resumen = nombres.join(", ");
        if !deps.is_empty() {
            resumen.push_str(&format!(" + {} (dependencia)", deps.join(", ")));
        }
        container(
            row![
                text(format!("{n} en la lista")).font(fuentes::TITULO).size(texto::BODY.0).color(p.bg),
                text(resumen).size(texto::BODY_SM.0).color(p.bg).width(Length::Fill),
                iced::widget::button(text("VACIAR").font(fuentes::ETIQUETA).size(texto::LABEL.0).color(p.bg))
                    .padding(Padding::from([espacio::S2, espacio::S4]))
                    .on_press(Mensaje::TiendaVaciar)
                    .style(move |_, _| iced::widget::button::Style {
                        text_color: p.bg,
                        border: iced::Border { color: p.bg, width: 2.0, radius: 0.0.into() },
                        ..Default::default()
                    }),
                iced::widget::button(text(format!("INSTALAR {n}")).font(fuentes::ETIQUETA).size(texto::LABEL.0).color(p.text))
                    .padding(Padding::from([espacio::S2, espacio::S4]))
                    .on_press(Mensaje::TiendaInstalar)
                    .style(move |_, _| iced::widget::button::Style {
                        background: Some(iced::Background::Color(p.bg)),
                        text_color: p.text,
                        ..Default::default()
                    }),
            ]
            .spacing(espacio::S4)
            .align_y(Alignment::Center)
            .padding(Padding::from([espacio::S3, espacio::S4])),
        )
        .width(Length::Fill)
        .style(icaro_ui::estilo::bloque(p.text))
        .into()
    };
    column![
        column![cabecera, cuerpo].spacing(espacio::S6).padding(Padding::from([espacio::S6, espacio::S12])),
        barra,
    ]
    .into()
}

/// Panel lateral de 640 px con la ficha rápida de un mod.
fn ficha_mod<'a>(app: &'a App, id: &str) -> Element<'a, Mensaje> {
    if !app.servicios.demo { return diseno_mods::ficha(app); }
    use icaro_ui::componentes::{etiqueta, insignia, Estado};
    let p = app.paleta();
    let instancia = con_instancias(|l| l[app.editor_inst].nombre.clone());
    let (mc_inst, cargador_inst) = juego_de_instancia(app);
    let instalados = mods::listar(&instancia);
    let puesto = instalados.iter().find(|m| m.id == id);
    let Some(c) = mods::en_catalogo(id) else {
        return container(text("Sin ficha").size(texto::BODY.0).color(p.text)).into();
    };
    let en_lista = app.tienda_lista.iter().any(|i| i == id);
    let accion: Element<Mensaje> = if let Some(m) = puesto {
        insignia(p, Estado::Exito, &format!("Instalado {}", m.version))
    } else if en_lista {
        componentes::boton(p, "Quitar de la lista", Variante::Secundario, Some(Mensaje::TiendaQuitar(id.to_owned())))
    } else {
        componentes::boton(p, "Agregar a la lista", Variante::Primario, Some(Mensaje::TiendaAgregar(id.to_owned())))
    };
    let cabecera = row![
        icono_mod(p, id, 56.0),
        column![
            text(c.titulo).font(fuentes::TITULO).size(texto::HEADING.0 + 2.0).color(p.text),
            text(format!("{} · {} descargas", c.autor, c.descargas)).font(fuentes::MONO).size(texto::MONO_SM.0).color(p.text_muted),
        ]
        .spacing(espacio::S1)
        .width(Length::Fill),
        iced::widget::button(icaro_ui::iconos::icono_texto(Icono::Cerrar, Tam::Base))
            .padding(espacio::S2)
            .on_press(Mensaje::PanelMod(None))
            .style(icaro_ui::estilo::boton_fantasma(p)),
    ]
    .spacing(espacio::S4)
    .align_y(Alignment::Center);
    let laminas = galeria_de(id);
    let mut galeria = row![].spacing(espacio::S2);
    for l in laminas {
        galeria = galeria.push(container(icaro_ui::laminas::grabado(p, l, Length::Fill, 90)).width(Length::FillPortion(1)).style(icaro_ui::estilo::marco(p)));
    }
    let mut versiones = column![etiqueta(p, &format!("Versiones para {mc_inst} · {cargador_inst}"))].spacing(espacio::S2);
    let comp = compatibles(c, &mc_inst, &cargador_inst);
    if comp.is_empty() {
        versiones = versiones.push(text("No hay versiones para tu instancia.").size(texto::BODY_SM.0).color(p.text_muted));
    }
    for l in comp {
        versiones = versiones.push(
            row![
                text(l.version).font(fuentes::MONO).size(texto::MONO.0).color(p.text).width(100),
                insignia(p, Estado::Exito, "Estable"),
                Space::with_width(Length::Fill),
                text(l.fecha).font(fuentes::MONO).size(texto::MONO_SM.0).color(p.text_muted),
            ]
            .spacing(espacio::S3)
            .align_y(Alignment::Center),
        );
    }
    let mut deps = column![etiqueta(p, "Dependencias")].spacing(espacio::S2);
    if c.dependencias.is_empty() {
        deps = deps.push(text("No necesita otros mods.").size(texto::BODY_SM.0).color(p.text_muted));
    }
    for d in c.dependencias {
        let titulo = mods::en_catalogo(d).map_or((*d).to_owned(), |x| x.titulo.to_owned());
        let puesta = instalados.iter().any(|m| m.id == *d);
        deps = deps.push(
            row![
                icono_mod(p, d, 24.0),
                text(titulo).size(texto::BODY.0).color(p.text).width(Length::Fill),
                if puesta { insignia(p, Estado::Exito, "Instalada") } else { insignia(p, Estado::Aviso, "Se instalará") },
            ]
            .spacing(espacio::S3)
            .align_y(Alignment::Center),
        );
    }
    let cuerpo = column![
        cabecera,
        row![
            accion,
            componentes::boton(p, "Ver página", Variante::Secundario, Some(Mensaje::AbrirModPagina(id.to_owned()))),
        ]
        .spacing(espacio::S3),
        text(c.descripcion).size(texto::BODY.0).color(p.text),
        galeria,
        versiones,
        deps,
    ]
    .spacing(espacio::S5);
    row![
        container(Space::new(2.0, Length::Fill)).style(icaro_ui::estilo::bloque(p.text)),
        container(desplazable(container(cuerpo).padding(espacio::S6)))
            .width(Length::Fill)
            .height(Length::Fill)
            .style(icaro_ui::estilo::flotante(p)),
    ]
    .width(640)
    .into()
}

/// Días desde 1970 de una fecha civil (algoritmo de Howard Hinnant).
fn dias_civil(a: i64, m: i64, d: i64) -> i64 {
    let a = if m <= 2 { a - 1 } else { a };
    let era = a.div_euclid(400);
    let aoe = a - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = aoe * 365 + aoe / 4 - aoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// "hace 3 meses" a partir de una fecha `AAAA-MM-DD`.
fn hace(fecha: &str) -> String {
    let partes: Vec<i64> = fecha.split('-').filter_map(|v| v.parse().ok()).collect();
    let [a, m, d] = partes[..] else {
        return fecha.to_owned();
    };
    let hoy = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |t| (t.as_secs() / 86_400) as i64);
    let dias = (hoy - dias_civil(a, m, d)).max(0);
    let (n, unidad) = if dias < 1 {
        return "hoy".into();
    } else if dias < 30 {
        (dias, "día")
    } else if dias < 365 {
        (dias / 30, "mes")
    } else {
        (dias / 365, "año")
    };
    let plural = match (n, unidad) {
        (1, u) => u.to_owned(),
        (_, "mes") => "meses".to_owned(),
        (_, u) => format!("{u}s"),
    };
    format!("hace {n} {plural}")
}

/// Etiqueta pequeña con contorno, como las de compatibilidad y etiquetas.
fn chip_dato<'a>(p: icaro_ui::tema::Paleta, texto: &str) -> Element<'a, Mensaje> {
    container(text(texto.to_owned()).font(fuentes::MONO).size(texto::MONO_SM.0).color(p.text))
        .padding(Padding::from([espacio::S1, espacio::S2]))
        .style(icaro_ui::estilo::marco(p))
        .into()
}

/// Chips en filas de hasta tres, porque iced no parte las filas solo.
fn chips_en_filas<'a>(p: icaro_ui::tema::Paleta, items: &[String]) -> Element<'a, Mensaje> {
    let mut col = column![].spacing(espacio::S2);
    for grupo in items.chunks(3) {
        let mut fila = row![].spacing(espacio::S2);
        for i in grupo {
            fila = fila.push(chip_dato(p, i));
        }
        col = col.push(fila);
    }
    col.into()
}

/// Tarjeta de la columna lateral, con título opcional.
fn tarjeta_lateral<'a>(
    p: icaro_ui::tema::Paleta,
    titulo: Option<&str>,
    cuerpo: Element<'a, Mensaje>,
) -> Element<'a, Mensaje> {
    let mut col = column![].spacing(espacio::S3);
    if let Some(t) = titulo {
        col = col.push(text(t.to_owned()).font(fuentes::TITULO).size(texto::BODY.0 + 2.0).color(p.text));
    }
    container(col.push(cuerpo))
        .padding(espacio::S4)
        .width(Length::Fill)
        .style(icaro_ui::estilo::tarjeta(p, false))
        .into()
}

/// Fila de la tarjeta Enlaces: texto con el icono de enlace externo.
fn enlace_externo<'a>(p: icaro_ui::tema::Paleta, texto: &str) -> Element<'a, Mensaje> {
    iced::widget::button(
        row![
            text(texto.to_owned()).size(texto::BODY.0).color(p.text).width(Length::Fill),
            icono(Icono::Externo, Tam::Base, p.text_muted),
        ]
        .spacing(espacio::S2)
        .align_y(Alignment::Center),
    )
    .width(Length::Fill)
    .padding(Padding::from([espacio::S1, 0.0]))
    .on_press(Mensaje::Nada)
    .style(icaro_ui::estilo::sin_estilo(p.text))
    .into()
}

/// Fila de la tarjeta Detalles: icono y texto.
fn fila_detalle<'a>(p: icaro_ui::tema::Paleta, glifo: Icono, texto: String) -> Element<'a, Mensaje> {
    row![
        icono(glifo, Tam::Base, p.text_muted),
        text(texto).size(texto::BODY.0).color(p.text),
    ]
    .spacing(espacio::S3)
    .align_y(Alignment::Center)
    .into()
}

/// Tarjetas Enlaces, Etiquetas, Creadores y Detalles, comunes a mods y modpacks.
#[allow(clippy::too_many_arguments)]
fn tarjetas_informacion<'a>(
    p: icaro_ui::tema::Paleta,
    etiquetas: &[String],
    creador: (&str, &str, String, Vec<&str>),
    licencia: &str,
    publicado: &str,
    actualizado: &str,
    extra: Vec<(Icono, String)>,
) -> Vec<Element<'a, Mensaje>> {
    use icaro_ui::componentes::etiqueta;
    let enlaces = column![
        enlace_externo(p, "Reportar problemas"),
        enlace_externo(p, "Ver código fuente"),
        enlace_externo(p, "Unirse al servidor de Discord"),
        container(Space::new(Length::Fill, 1.0)).style(icaro_ui::estilo::bloque(p.border)),
        enlace_externo(p, "Donar"),
    ]
    .spacing(espacio::S1);
    let (nombre, rol, resumen, top) = creador;
    let mut ficha = column![
        row![
            container(icono(Icono::Instancias, Tam::Base, p.text))
                .padding(espacio::S2)
                .style(icaro_ui::estilo::marco(p)),
            column![
                text(nombre.to_owned()).font(fuentes::TITULO).size(texto::BODY.0 + 1.0).color(p.text),
                text(rol.to_owned()).size(texto::BODY_SM.0).color(p.text_muted),
            ]
            .spacing(espacio::S1),
        ]
        .spacing(espacio::S3)
        .align_y(Alignment::Center),
        text(resumen).size(texto::BODY_SM.0).color(p.text_muted),
    ]
    .spacing(espacio::S2);
    if !top.is_empty() {
        ficha = ficha.push(etiqueta(p, "Más descargados"));
        for t in top {
            ficha = ficha.push(text(t.to_owned()).size(texto::BODY_SM.0).color(p.text));
        }
    }
    ficha = ficha.push(componentes::boton(p, "Ver perfil", Variante::Secundario, Some(Mensaje::Nada)));
    let mut detalles = column![
        fila_detalle(p, Icono::Copiar, format!("Licencia {licencia}")),
        fila_detalle(p, Icono::Check, format!("Publicado {}", hace(publicado))),
        fila_detalle(p, Icono::Actualizar, format!("Actualizado {}", hace(actualizado))),
    ]
    .spacing(espacio::S3);
    for (g, t) in extra {
        detalles = detalles.push(fila_detalle(p, g, t));
    }
    vec![
        tarjeta_lateral(p, Some("Enlaces"), enlaces.into()),
        tarjeta_lateral(p, Some("Etiquetas"), chips_en_filas(p, etiquetas)),
        tarjeta_lateral(p, Some("Creadores"), ficha.into()),
        tarjeta_lateral(p, Some("Detalles"), detalles.into()),
    ]
}

/// Tres láminas para la galería de un mod o modpack, siempre las mismas para el
/// mismo nombre y sin repetir entre ellos (cuando alcanzan).
fn galeria_de(clave: &str) -> [Lamina; 3] {
    const POZO: [Lamina; 15] = [
        Lamina::ArcoPuente, Lamina::CadenasReja, Lamina::FuegoArco, Lamina::EscaleraPuentes, Lamina::TorreReja,
        Lamina::PuenteTorre, Lamina::MercadoCarros, Lamina::SoldadosAldea, Lamina::TivoliRocas, Lamina::FortalezasValle,
        Lamina::CabanaCasas, Lamina::OceanoPez, Lamina::TetisDelfin, Lamina::JeronimoCalabaza, Lamina::HuidaCiudad,
    ];
    let mut h: usize = 0;
    for b in clave.bytes() {
        h = h.wrapping_mul(31).wrapping_add(usize::from(b));
    }
    [POZO[h % 15], POZO[(h + 5) % 15], POZO[(h + 10) % 15]]
}

/// Índice de la versión que sirve para la instancia: la más nueva con su
/// versión del juego y su cargador (y estable, salvo que se pidan pruebas).
fn para_instancia(c: &mods::ModCatalogo, mc: &str, cargador: &str, pruebas: bool) -> Option<usize> {
    c.lanzamientos.iter().position(|l| {
        l.minecraft == mc
            && (mods::es_recurso() || c.cargadores_de(l).iter().any(|x| x.eq_ignore_ascii_case(cargador)))
            && (pruebas || l.canal == "release")
    })
}

/// Página completa de un mod: encabezado, pestañas y columna lateral fija con
/// la acción principal, la compatibilidad, los datos y el creador.
fn pagina_mod<'a>(app: &'a App, id: &str) -> Element<'a, Mensaje> {
    use icaro_ui::componentes::{casilla, etiqueta, insignia, Estado};
    let p = app.paleta();
    let datos = &con_instancias(|l| l[app.editor_inst].clone());
    let instancia = datos.nombre.clone();
    let (mc_inst, cargador_inst) = {
        let mut partes = datos.version.split(" · ");
        (partes.next().unwrap_or("").to_owned(), partes.next().unwrap_or("").to_owned())
    };
    let todos = mods::listar(&instancia);
    let instalado = todos.iter().find(|m| m.id == id).cloned();
    let catalogo = mods::en_catalogo(id);
    let extras = mods::extras(id);
    let titulo = catalogo.map_or_else(|| id.to_owned(), |c| c.titulo.to_owned());

    let encabezado = column![
        row![componentes::boton(p, if mods::es_recurso() { "Volver a los resourcepacks" } else { "Volver a los mods" }, Variante::Fantasma, Some(Mensaje::CerrarModPagina))],
        text(titulo.to_uppercase()).font(fuentes::DISPLAY).size(texto::DISPLAY.0).color(p.text),
        text(catalogo.map_or("Mod sin ficha en el catálogo.", |c| c.descripcion))
            .size(texto::BODY.0)
            .color(p.text_muted),
    ]
    .spacing(espacio::S2);

    // Las pestañas dependen del mod: sin imágenes no hay Galería.
    let mut nombres = vec!["Descripción"];
    if !extras.galeria.is_empty() {
        nombres.push("Galería");
    }
    nombres.extend(["Versiones", "Cambios"]);
    if !mods::es_recurso() {
        nombres.push("Dependencias");
    }
    let activa = app.mod_pestana.min(nombres.len() - 1);
    let barra = componentes::pestanas(
        p,
        &nombres.iter().map(|n| (*n, None)).collect::<Vec<_>>(),
        activa,
        Mensaje::PestanaMod,
    );

    let idx_para = catalogo.and_then(|c| para_instancia(c, &mc_inst, &cargador_inst, app.mod_betas));
    let idx_instalado = catalogo.zip(instalado.as_ref()).and_then(|(c, m)| {
        c.lanzamientos.iter().position(|l| l.version == m.version)
    });

    let contenido: Element<Mensaje> = match (catalogo, nombres[activa]) {
        (None, _) => text("Este mod no está en el catálogo, así que no hay más datos que mostrar.")
            .size(texto::BODY.0)
            .color(p.text_muted)
            .into(),
        (Some(c), "Descripción") => {
            let mut cat = row![].spacing(espacio::S2);
            for k in c.categorias {
                cat = cat.push(insignia(p, Estado::Info, k));
            }
            column![
                text(c.detalle).size(texto::BODY.0).color(p.text),
                text(if mods::es_recurso() {
                    format!("Resolución {} para Minecraft {}.", c.cargadores.join(", "), c.minecraft().join(", "))
                } else {
                    format!("Funciona con {} en {}.", c.cargadores.join(", "), c.minecraft().join(", "))
                })
                    .size(texto::BODY.0)
                    .color(p.text_muted),
                cat,
            ]
            .spacing(espacio::S4)
            .into()
        }
        (Some(_), "Galería") => {
            let laminas = galeria_de(id);
            let sel = app.mod_galeria.min(extras.galeria.len() - 1);
            let principal = column![
                container(icaro_ui::laminas::grabado(p, laminas[sel % laminas.len()], Length::Fill, 300))
                    .style(icaro_ui::estilo::marco(p)),
                text(extras.galeria[sel]).size(texto::BODY.0).color(p.text),
            ]
            .spacing(espacio::S2);
            let mut miniaturas = row![].spacing(espacio::S4);
            for (i, titulo) in extras.galeria.iter().enumerate().take(3) {
                miniaturas = miniaturas.push(
                    iced::widget::button(
                        column![
                            container(icaro_ui::laminas::grabado(p, laminas[i % laminas.len()], Length::Fill, 110))
                                .style(icaro_ui::estilo::marco(p)),
                            text(*titulo).size(texto::BODY_SM.0).color(if i == sel { p.text } else { p.text_muted }),
                        ]
                        .spacing(espacio::S1),
                    )
                    .width(Length::FillPortion(1))
                    .padding(0)
                    .on_press(Mensaje::GaleriaMod(i))
                    .style(icaro_ui::estilo::sin_estilo(p.text)),
                );
            }
            column![
                principal,
                miniaturas,
                text(format!("Imágenes de ejemplo; las capturas reales llegarán con la conexión al catálogo."))
                    .size(texto::BODY_SM.0)
                    .color(p.text_muted),
            ]
            .spacing(espacio::S4)
            .into()
        }
        (Some(c), "Versiones") => {
            let mut mc_opciones = vec!["Todas".to_owned()];
            mc_opciones.extend(c.minecraft().iter().map(|v| (*v).to_owned()));
            let mut loaders = vec!["Todos".to_owned()];
            for l in c.lanzamientos {
                for x in c.cargadores_de(l) {
                    if !loaders.iter().any(|y| y == x) {
                        loaders.push((*x).to_owned());
                    }
                }
            }
            let filtros = row![
                container(componentes::selector(p, mc_opciones, Some(app.mod_filtro_mc.clone()), Mensaje::FiltroMcMod)).width(160),
                container(componentes::selector(p, loaders, Some(app.mod_filtro_loader.clone()), Mensaje::FiltroLoaderMod)).width(160),
                casilla(p, "Mostrar betas y alphas", app.mod_betas, |_| Mensaje::BetasMod),
            ]
            .spacing(espacio::S4)
            .align_y(Alignment::Center);
            let mut tabla = column![container(
                row![
                    container(etiqueta(p, "Versión")).width(Length::FillPortion(2)),
                    container(etiqueta(p, "Juego")).width(Length::FillPortion(2)),
                    container(etiqueta(p, if mods::es_recurso() { "Resolución" } else { "Cargadores" })).width(Length::FillPortion(3)),
                    container(etiqueta(p, "Fecha")).width(Length::FillPortion(3)),
                    Space::with_width(Length::FillPortion(8)),
                ]
            )
            .padding(Padding::from([espacio::S2, espacio::S3]))]
            .spacing(espacio::S1);
            let mut filas = 0;
            for (i, l) in c.lanzamientos.iter().enumerate() {
                let cargadores = c.cargadores_de(l);
                if (app.mod_filtro_mc != "Todas" && l.minecraft != app.mod_filtro_mc)
                    || (app.mod_filtro_loader != "Todos" && !cargadores.contains(&app.mod_filtro_loader.as_str()))
                    || (!app.mod_betas && l.canal != "release")
                {
                    continue;
                }
                filas += 1;
                let es_para = idx_para == Some(i);
                let actual = idx_instalado == Some(i);
                let mut derecha = row![].spacing(espacio::S2).align_y(Alignment::Center);
                if actual {
                    derecha = derecha.push(insignia(p, Estado::Exito, "Instalada"));
                }
                if es_para {
                    derecha = derecha.push(insignia(p, Estado::Info, "Para tu instancia"));
                    if !actual {
                        let (texto, msg) = match &instalado {
                            Some(m) => ("Usar", Mensaje::VersionMod(m.archivo.clone(), l.version.to_owned())),
                            None => ("Agregar", Mensaje::InstalarMod(c.id.to_owned(), l.version.to_owned())),
                        };
                        derecha = derecha.push(componentes::boton(p, texto, Variante::Primario, Some(msg)));
                    }
                }
                let mut version = row![text(l.version).font(fuentes::MONO).size(texto::MONO.0).color(p.text)]
                    .spacing(espacio::S2)
                    .align_y(Alignment::Center);
                if l.canal != "release" {
                    version = version.push(insignia(p, Estado::Aviso, l.canal));
                }
                tabla = tabla.push(
                    container(
                        row![
                            container(version).width(Length::FillPortion(2)),
                            text(l.minecraft).font(fuentes::MONO).size(texto::MONO.0).color(p.text).width(Length::FillPortion(2)),
                            text(cargadores.join(", ")).size(texto::BODY_SM.0).color(p.text_muted).width(Length::FillPortion(3)),
                            text(l.fecha).font(fuentes::MONO).size(texto::MONO_SM.0).color(p.text_muted).width(Length::FillPortion(3)),
                            container(derecha).width(Length::FillPortion(8)),
                        ]
                        .align_y(Alignment::Center),
                    )
                    .padding(Padding::from([espacio::S2, espacio::S3]))
                    .style(icaro_ui::estilo::tarjeta(p, es_para)),
                );
            }
            if filas == 0 {
                tabla = tabla.push(
                    text("Ninguna versión coincide con los filtros.").size(texto::BODY.0).color(p.text_muted),
                );
            }
            column![filtros, tabla].spacing(espacio::S4).into()
        }
        (Some(c), "Cambios") => {
            let mut lista = column![].spacing(espacio::S5);
            for (i, l) in c.lanzamientos.iter().enumerate() {
                let mut cabeza = row![
                    text(format!("{} · {}", l.version, l.fecha)).font(fuentes::MONO).size(texto::MONO_SM.0).color(p.text),
                ]
                .spacing(espacio::S2)
                .align_y(Alignment::Center);
                if l.canal != "release" {
                    cabeza = cabeza.push(insignia(p, Estado::Aviso, l.canal));
                }
                if idx_instalado == Some(i) {
                    cabeza = cabeza.push(insignia(p, Estado::Exito, "Instalada"));
                } else if idx_instalado.is_some_and(|ins| i < ins) {
                    cabeza = cabeza.push(insignia(p, Estado::Info, "Nueva"));
                }
                let mut bloque = column![cabeza].spacing(espacio::S1);
                for linea in l.cambios {
                    bloque = bloque.push(text(format!("— {linea}")).size(texto::BODY.0).color(p.text_muted));
                }
                lista = lista.push(bloque);
            }
            lista.into()
        }
        (Some(c), _) => {
            let nombre_de = |dep: &str| mods::en_catalogo(dep).map_or_else(|| dep.to_owned(), |d| d.titulo.to_owned());
            let mut lista = column![].spacing(espacio::S3);
            lista = lista.push(etiqueta(p, "Obligatorias"));
            if c.dependencias.is_empty() {
                lista = lista.push(text("Este mod no necesita otros.").size(texto::BODY.0).color(p.text_muted));
            }
            for dep in c.dependencias {
                let puesta = todos.iter().any(|m| m.id == *dep);
                lista = lista.push(
                    row![
                        text(nombre_de(dep)).size(texto::BODY.0).color(p.text).width(Length::Fill),
                        if puesta { insignia(p, Estado::Exito, "Instalada") } else { insignia(p, Estado::Aviso, "Se instalará") },
                    ]
                    .align_y(Alignment::Center),
                );
            }
            if !extras.opcionales.is_empty() {
                lista = lista.push(Space::with_height(espacio::S2)).push(etiqueta(p, "Opcionales"));
                for dep in extras.opcionales {
                    let puesta = todos.iter().any(|m| m.id == *dep);
                    let marcada = puesta || app.mod_opcionales.iter().any(|o| o == dep);
                    let id_dep = (*dep).to_owned();
                    lista = lista.push(
                        row![
                            casilla(p, &nombre_de(dep), marcada, move |_| Mensaje::OpcionalMod(id_dep.clone())),
                            Space::with_width(Length::Fill),
                            if puesta { insignia(p, Estado::Exito, "Instalada") } else { Space::with_width(0).into() },
                        ]
                        .align_y(Alignment::Center),
                    );
                }
            }
            if !extras.incompatibles.is_empty() {
                lista = lista.push(Space::with_height(espacio::S2)).push(etiqueta(p, "Incompatibles"));
                for (dep, motivo) in extras.incompatibles {
                    let presente = todos.iter().any(|m| m.id == *dep);
                    lista = lista.push(
                        row![
                            text(nombre_de(dep)).size(texto::BODY.0).color(p.text),
                            text(*motivo).size(texto::BODY_SM.0).color(p.text_muted).width(Length::Fill),
                            if presente { insignia(p, Estado::Error, "Instalado") } else { Space::with_width(0).into() },
                        ]
                        .spacing(espacio::S4)
                        .align_y(Alignment::Center),
                    );
                }
            }
            lista.into()
        }
    };

    // Columna lateral fija, con tarjetas como en Modrinth.
    let mut lateral = column![].spacing(espacio::S3);
    let accion: Element<Mensaje> = match (&instalado, catalogo) {
        (Some(m), _) => column![
            insignia(p, Estado::Exito, &format!("Instalado {}", m.version)),
            row![
                text("Activo").size(texto::BODY.0).color(p.text).width(Length::Fill),
                componentes::interruptor(p, m.activo, Mensaje::AlternarMod(m.archivo.clone())),
            ]
            .align_y(Alignment::Center),
            componentes::boton(p, if mods::es_recurso() { "Eliminar pack" } else { "Eliminar mod" }, Variante::Peligro, Some(Mensaje::EliminarMod(m.archivo.clone()))),
        ]
        .spacing(espacio::S3)
        .into(),
        (None, Some(c)) => {
            let version = c.lanzamientos[idx_para.unwrap_or(0)].version;
            componentes::boton(
                p,
                &format!("Agregar {version}"),
                Variante::Primario,
                Some(Mensaje::InstalarMod(c.id.to_owned(), version.to_owned())),
            )
        }
        _ => Space::with_height(0).into(),
    };
    lateral = lateral.push(tarjeta_lateral(p, None, accion));
    if let Some(c) = catalogo {
        let versiones: Vec<String> = c.minecraft().iter().map(|v| (*v).to_owned()).collect();
        let mut plataformas: Vec<String> = Vec::new();
        for l in c.lanzamientos {
            for x in c.cargadores_de(l) {
                if !plataformas.iter().any(|y| y == x) {
                    plataformas.push((*x).to_owned());
                }
            }
        }
        let compat = column![
            row![
                text(format!("{instancia}: {mc_inst} · {cargador_inst}")).size(texto::BODY_SM.0).color(p.text_muted).width(Length::Fill),
                if idx_para.is_some() { insignia(p, Estado::Exito, "Compatible") } else { insignia(p, Estado::Error, "No compatible") },
            ]
            .spacing(espacio::S2)
            .align_y(Alignment::Center),
            text("Minecraft: Java Edition").size(texto::BODY.0).color(p.text),
            chips_en_filas(p, &versiones),
            text(if mods::es_recurso() { "Resolución" } else { "Plataformas" }).size(texto::BODY.0).color(p.text),
            chips_en_filas(p, &plataformas),
            text("Entornos compatibles").size(texto::BODY.0).color(p.text),
            chip_dato(p, extras.entorno),
        ]
        .spacing(espacio::S2);
        lateral = lateral.push(tarjeta_lateral(p, Some("Compatibilidad"), compat.into()));
        let etiquetas: Vec<String> = c.categorias.iter().map(|k| (*k).to_owned()).collect();
        let resumen = format!(
            "{} · {} proyectos · {} descargas",
            extras.creador_tipo, extras.creador_proyectos, extras.creador_descargas
        );
        for t in tarjetas_informacion(
            p,
            &etiquetas,
            (c.autor, if extras.creador_tipo == "Organización" { "Propietario" } else { "Miembro" }, resumen, extras.creador_top.to_vec()),
            c.licencia,
            extras.publicado,
            extras.actualizado,
            vec![
                (Icono::Descargas, format!("{} descargas", c.descargas)),
                (Icono::Info, format!("{} seguidores", extras.seguidores)),
            ],
        ) {
            lateral = lateral.push(t);
        }
    }

    let principal = desplazable(
        column![barra, con_entrada(app, contenido)]
            .spacing(espacio::S6)
            .padding(Padding { right: espacio::S4, ..Padding::ZERO }),
    );
    let cuerpo = row![
        container(principal).width(Length::Fill).height(Length::Fill),
        container(desplazable(lateral))
            .width(300)
            .height(Length::Fill),
    ]
    .spacing(espacio::S8)
    .height(Length::Fill);
    column![encabezado, cuerpo]
        .spacing(espacio::S6)
        .padding(Padding::from([espacio::S6, espacio::S12]))
        .into()
}

/// Publicaciones de ejemplo de un modpack: versión, fecha y cambios.
fn publicaciones_modpack(m: &modpacks::Modpack) -> [(&'static str, &'static str, [&'static str; 2]); 3] {
    let _ = m;
    [
        ("2.1.0", "2026-09-30", ["Actualiza los mods a su última versión", "Corrige un fallo al iniciar el mundo"]),
        ("2.0.0", "2026-08-12", ["Agrega mods nuevos y quita los que ya no se mantienen", "Ajusta la configuración de rendimiento"]),
        ("1.5.3", "2026-05-20", ["Corrección de errores", "Mejora el tiempo de carga"]),
    ]
}

/// Mods que trae un modpack (muestra tomada del catálogo de mods).
fn mods_incluidos(i: usize) -> Vec<&'static mods::ModCatalogo> {
    let n = mods::CATALOGO.len();
    (0..5).map(|k| &mods::CATALOGO[(i + k) % n]).collect()
}

/// Página de un modpack: mismas vistas que la de un mod, con una pestaña con
/// los mods que incluye en lugar de dependencias.
fn pagina_modpack(app: &App, i: usize) -> Element<'_, Mensaje> {
    use icaro_ui::componentes::{etiqueta, insignia, Estado};
    let p = app.paleta();
    let m = &modpacks::CATALOGO[i];
    let instalado = con_instancias(|l| l.iter().any(|d| d.nombre == m.nombre));
    let publicaciones = publicaciones_modpack(m);
    let incluidos = mods_incluidos(i);

    let encabezado = column![
        row![componentes::boton(p, "Volver a los modpacks", Variante::Fantasma, Some(Mensaje::CerrarModpack))],
        text(m.nombre.to_uppercase()).font(fuentes::DISPLAY).size(texto::DISPLAY.0).color(p.text),
        text(m.descripcion).size(texto::BODY.0).color(p.text_muted),
        text(format!("{} descargas · {} mods · {}", m.descargas, m.mods, m.tamano))
            .font(fuentes::MONO)
            .size(texto::MONO_SM.0)
            .color(p.text_muted),
    ]
    .spacing(espacio::S2);

    let nombres = ["Descripción", "Galería", "Versiones", "Cambios", "Mods incluidos"];
    let activa = app.modpack_pestana.min(nombres.len() - 1);
    let barra = componentes::pestanas(
        p,
        &nombres.iter().map(|n| (*n, None)).collect::<Vec<_>>(),
        activa,
        Mensaje::PestanaModpack,
    );

    let contenido: Element<Mensaje> = match nombres[activa] {
        "Descripción" => column![
            text(m.descripcion).size(texto::BODY.0).color(p.text),
            text(format!(
                "Incluye {} mods para Minecraft {} con {}. Al instalarlo se crea una instancia nueva con su propia configuración de Java y memoria.",
                m.mods, m.minecraft, m.cargador
            ))
            .size(texto::BODY.0)
            .color(p.text_muted),
            row![insignia(p, Estado::Info, m.categoria), insignia(p, Estado::Info, "Modpack")].spacing(espacio::S2),
        ]
        .spacing(espacio::S4)
        .into(),
        "Galería" => {
            let laminas = [m.portada, galeria_de(m.nombre)[0], galeria_de(m.nombre)[1]];
            let titulos = ["Primer vistazo", "Exploración", "Construcciones"];
            let sel = app.modpack_galeria.min(2);
            let mut miniaturas = row![].spacing(espacio::S4);
            for (k, t) in titulos.iter().enumerate() {
                miniaturas = miniaturas.push(
                    iced::widget::button(
                        column![
                            container(icaro_ui::laminas::grabado(p, laminas[k], Length::Fill, 110)).style(icaro_ui::estilo::marco(p)),
                            text(*t).size(texto::BODY_SM.0).color(if k == sel { p.text } else { p.text_muted }),
                        ]
                        .spacing(espacio::S1),
                    )
                    .width(Length::FillPortion(1))
                    .padding(0)
                    .on_press(Mensaje::GaleriaModpack(k))
                    .style(icaro_ui::estilo::sin_estilo(p.text)),
                );
            }
            column![
                container(icaro_ui::laminas::grabado(p, laminas[sel], Length::Fill, 300)).style(icaro_ui::estilo::marco(p)),
                text(titulos[sel]).size(texto::BODY.0).color(p.text),
                miniaturas,
            ]
            .spacing(espacio::S3)
            .into()
        }
        "Versiones" => {
            let mut tabla = column![container(
                row![
                    container(etiqueta(p, "Versión")).width(Length::FillPortion(2)),
                    container(etiqueta(p, "Juego")).width(Length::FillPortion(2)),
                    container(etiqueta(p, "Cargador")).width(Length::FillPortion(2)),
                    container(etiqueta(p, "Fecha")).width(Length::FillPortion(3)),
                    Space::with_width(Length::FillPortion(4)),
                ]
            )
            .padding(Padding::from([espacio::S2, espacio::S3]))]
            .spacing(espacio::S1);
            for (k, (version, fecha, _)) in publicaciones.iter().enumerate() {
                let derecha: Element<Mensaje> = if k == 0 {
                    if instalado {
                        insignia(p, Estado::Exito, "Instalado")
                    } else {
                        componentes::boton(p, "Instalar", Variante::Primario, Some(Mensaje::InstalarModpack(i)))
                    }
                } else {
                    Space::with_width(0).into()
                };
                tabla = tabla.push(
                    container(
                        row![
                            text(*version).font(fuentes::MONO).size(texto::MONO.0).color(p.text).width(Length::FillPortion(2)),
                            text(m.minecraft).font(fuentes::MONO).size(texto::MONO.0).color(p.text).width(Length::FillPortion(2)),
                            text(m.cargador).size(texto::BODY_SM.0).color(p.text_muted).width(Length::FillPortion(2)),
                            text(*fecha).font(fuentes::MONO).size(texto::MONO_SM.0).color(p.text_muted).width(Length::FillPortion(3)),
                            container(derecha).width(Length::FillPortion(4)),
                        ]
                        .align_y(Alignment::Center),
                    )
                    .padding(Padding::from([espacio::S2, espacio::S3]))
                    .style(icaro_ui::estilo::tarjeta(p, k == 0)),
                );
            }
            tabla.into()
        }
        "Cambios" => {
            let mut lista = column![].spacing(espacio::S5);
            for (k, (version, fecha, cambios)) in publicaciones.iter().enumerate() {
                let mut cabeza = row![text(format!("{version} · {fecha}")).font(fuentes::MONO).size(texto::MONO_SM.0).color(p.text)]
                    .spacing(espacio::S2)
                    .align_y(Alignment::Center);
                if k == 0 {
                    cabeza = cabeza.push(insignia(p, Estado::Info, "Última"));
                }
                let mut bloque = column![cabeza].spacing(espacio::S1);
                for c in cambios {
                    bloque = bloque.push(text(format!("— {c}")).size(texto::BODY.0).color(p.text_muted));
                }
                lista = lista.push(bloque);
            }
            lista.into()
        }
        _ => {
            let mut lista = column![text(format!("Muestra de los {} mods que incluye.", m.mods)).size(texto::BODY.0).color(p.text_muted)]
                .spacing(espacio::S2);
            for c in incluidos {
                lista = lista.push(
                    container(
                        row![
                            column![
                                text(c.titulo).font(fuentes::TITULO).size(texto::BODY.0).color(p.text),
                                text(c.descripcion).size(texto::BODY_SM.0).color(p.text_muted),
                            ]
                            .spacing(espacio::S1)
                            .width(Length::Fill),
                            text(c.lanzamientos.iter().find(|l| l.canal == "release").map_or("", |l| l.version)).font(fuentes::MONO).size(texto::MONO_SM.0).color(p.text_muted),
                        ]
                        .spacing(espacio::S4)
                        .align_y(Alignment::Center),
                    )
                    .padding(espacio::S3)
                    .width(Length::Fill)
                    .style(icaro_ui::estilo::tarjeta(p, false)),
                );
            }
            lista.into()
        }
    };

    let accion: Element<Mensaje> = if instalado {
        insignia(p, Estado::Exito, "Instalado como instancia")
    } else {
        componentes::boton(p, "Instalar como instancia", Variante::Primario, Some(Mensaje::InstalarModpack(i)))
    };
    let compat = column![
        text("Minecraft: Java Edition").size(texto::BODY.0).color(p.text),
        chips_en_filas(p, &[m.minecraft.to_owned()]),
        text("Plataformas").size(texto::BODY.0).color(p.text),
        chips_en_filas(p, &[m.cargador.to_owned()]),
        text("Entornos compatibles").size(texto::BODY.0).color(p.text),
        chip_dato(p, "Cliente y servidor"),
    ]
    .spacing(espacio::S2);
    let mut lateral = column![
        tarjeta_lateral(p, None, accion),
        tarjeta_lateral(p, Some("Compatibilidad"), compat.into()),
    ]
    .spacing(espacio::S3);
    let top: Vec<&str> = modpacks::CATALOGO.iter().filter(|o| o.autor == m.autor).map(|o| o.nombre).take(3).collect();
    for t in tarjetas_informacion(
        p,
        &[m.categoria.to_owned(), "Modpack".to_owned()],
        (m.autor, "Equipo", format!("{} modpacks", top.len().max(1)), top),
        "Código abierto",
        "2025-04-02",
        publicaciones[0].1,
        vec![
            (Icono::Descargas, format!("{} descargas", m.descargas)),
            (Icono::Instancias, format!("{} mods · {}", m.mods, m.tamano)),
        ],
    ) {
        lateral = lateral.push(t);
    }

    let cuerpo = row![
        container(desplazable(column![barra, con_entrada(app, contenido)].spacing(espacio::S6).padding(Padding { right: espacio::S4, ..Padding::ZERO })))
            .width(Length::Fill)
            .height(Length::Fill),
        container(desplazable(lateral)).width(300).height(Length::Fill),
    ]
    .spacing(espacio::S8)
    .height(Length::Fill);
    column![encabezado, cuerpo]
        .spacing(espacio::S6)
        .padding(Padding::from([espacio::S6, espacio::S12]))
        .into()
}

/// Catálogo de modpacks: cada uno se instala como una instancia nueva.
fn pantalla_modpacks(app: &App) -> Element<'_, Mensaje> {
    if !app.servicios.demo { return servicios::store_view(app); }
    let p = app.paleta();
    if let Some(i) = app.modpack_pagina {
        return pagina_modpack(app, i);
    }
    let consulta = app.busqueda_modpacks.trim().to_lowercase();
    let categoria = modpacks::CATEGORIAS[app.modpack_filtro.min(modpacks::CATEGORIAS.len() - 1)];
    let existentes: Vec<String> = instancias().into_iter().map(|d| d.nombre).collect();

    let mut chips = row![].spacing(espacio::S2);
    for (i, c) in modpacks::CATEGORIAS.iter().enumerate() {
        chips = chips.push(componentes::chip(p, c, app.modpack_filtro == i, Mensaje::FiltroModpacks(i)));
    }
    let herramientas = row![
        chips,
        Space::with_width(Length::Fill),
        container(componentes::campo(p, "Buscar modpacks", &app.busqueda_modpacks, Mensaje::BuscarModpacks)).width(300),
    ]
    .spacing(espacio::S4)
    .align_y(Alignment::Center);

    let mut tarjetas: Vec<Element<Mensaje>> = Vec::new();
    for (i, m) in modpacks::CATALOGO.iter().enumerate() {
        let coincide = (categoria == "Todos" || m.categoria == categoria)
            && (consulta.is_empty()
                || m.nombre.to_lowercase().contains(&consulta)
                || m.descripcion.to_lowercase().contains(&consulta));
        if !coincide {
            continue;
        }
        let instalado = existentes.iter().any(|n| n == m.nombre);
        let instalando = app.instalando.iter().any(|(n, _)| n == m.nombre);
        let accion = if instalado {
            componentes::boton(
                p,
                if instalando { "Instalando" } else { "Instalado" },
                Variante::Secundario,
                None,
            )
        } else {
            componentes::boton(p, "Instalar", Variante::Primario, Some(Mensaje::InstalarModpack(i)))
        };
        let cuerpo = column![
            iced::widget::button(text(m.nombre).font(fuentes::TITULO).size(texto::HEADING.0).color(p.text))
                .padding(0)
                .on_press(Mensaje::AbrirModpack(i))
                .style(icaro_ui::estilo::sin_estilo(p.text)),
            text(format!("{} · {}", m.minecraft, m.cargador).to_uppercase())
                .font(fuentes::MONO)
                .size(texto::MONO_SM.0)
                .color(p.text_muted),
            text(m.descripcion).size(texto::BODY_SM.0).color(p.text),
            text(format!("{} · {} descargas
{} mods · {}", m.autor, m.descargas, m.mods, m.tamano))
                .font(fuentes::MONO)
                .size(texto::MONO_SM.0)
                .color(p.text_muted),
            Space::with_height(Length::Fill),
            row![
                icaro_ui::componentes::insignia(p, icaro_ui::componentes::Estado::Info, m.categoria),
                Space::with_width(Length::Fill),
                accion,
            ]
            .align_y(Alignment::Center),
        ]
        .spacing(espacio::S2)
        .height(Length::Fill);
        tarjetas.push(
            container(
                column![
                    icaro_ui::laminas::grabado(p, m.portada, Length::Fill, 110),
                    container(cuerpo).padding(espacio::S4).height(Length::Fill),
                ],
            )
            .width(icaro_ui::instancias::ANCHO_TARJETA)
            .height(330)
            .style(icaro_ui::estilo::tarjeta(p, false))
            .into(),
        );
    }

    let disponible = app.ventana.width - 232.0 - 2.0 * espacio::S12 + espacio::S6;
    let columnas = ((disponible / (icaro_ui::instancias::ANCHO_TARJETA + espacio::S6)).floor() as usize).max(1);
    let mut rejilla = column![].spacing(espacio::S6);
    let mut it = tarjetas.into_iter();
    let mut hay = false;
    loop {
        let mut fila = row![].spacing(espacio::S6);
        let mut n = 0;
        for _ in 0..columnas {
            if let Some(t) = it.next() {
                fila = fila.push(t);
                n += 1;
            }
        }
        if n == 0 {
            break;
        }
        hay = true;
        rejilla = rejilla.push(fila);
    }
    if !hay {
        rejilla = rejilla.push(capt::vacio_busqueda(app, &app.busqueda_modpacks, Mensaje::BuscarModpacks(String::new())));
    }
    let cuerpo = column![
        text("Cada modpack se instala como una instancia nueva, con sus mods y su versión del juego.")
            .size(texto::BODY.0)
            .color(p.text_muted),
        herramientas,
        rejilla,
    ]
    .spacing(espacio::S6)
    .padding(Padding::from([espacio::S6, espacio::S12]));
    column![
        banda(p, Lamina::HidraCabezas, "Modpacks", Some(modpacks::CATALOGO.len())),
        suave(app, cuerpo)
    ]
    .into()
}

fn pantalla_editor(app: &App, pestana: usize) -> Element<'_, Mensaje> {
    if !app.servicios.demo && matches!(pestana, 2 | 3 | 8) && app.mods_vista == 1 { return servicios::store_view(app); }
    let p = app.paleta();
    if matches!(pestana, 2 | 3 | 8) {
        if let Some(id) = &app.mod_pagina {
            return pagina_mod(app, id);
        }
    }
    let instancia = con_instancias(|l| l[app.editor_inst].clone());
    let cantidad_mods = instancia.mods.split_whitespace().next()
        .and_then(|n| n.parse::<usize>().ok()).unwrap_or(0);
    let medidas = app.servicios.instancias.registros.get(app.editor_inst)
        .map(|r| estadisticas_reales::datos(app, &r.id)).unwrap_or_default();
    let datos = DatosEditor {
        nombre: app.general.nombre.clone(),
        version: instancia.version,
        jugado: if app.servicios.demo { if app.editor_inst == 0 { "41 h".into() } else { "—".into() } }
            else { estadisticas_reales::texto_jugado(medidas.segundos) },
        mods: cantidad_mods.to_string(),
        tamano: if app.servicios.demo { if app.editor_inst == 0 { "2,4 GB".into() } else { "—".into() } }
            else { estadisticas_reales::texto_tamano(medidas.bytes) },
        memoria: format!("{} MB", app.configs[app.editor_inst].memoria_max),
        portada: instancia.portada,
    };
    let encabezado = icaro_ui::editor::encabezado_editor_icono(
        p,
        &datos,
        MensajesEncabezado {
            volver: Mensaje::VolverEditor,
            abrir_carpeta: Mensaje::Pant(pantallas::Accion::AbrirCarpetaInstancia(app.editor_inst)),
            mas: Mensaje::Menu(Some(app.editor_inst)),
            jugar: Mensaje::Jugar(app.editor_inst),
        },
        mods_reales::permite_mods(app),
        if app.servicios.demo {componentes::boton_jugar(p,"Jugar",Some(&datos.memoria),Mensaje::Jugar(app.editor_inst))}else{juego_real::boton(app,app.editor_inst)},
        Some(iced::widget::button(container(icono_instancia::imagen(app,app.editor_inst,64.)).padding(2).style(icaro_ui::estilo::marco(p))).padding(0).on_press(Mensaje::Icono(icono_instancia::Accion::Abrir(app.editor_inst))).style(icaro_ui::estilo::boton_fantasma(p)).into()),
    );
    let barra = container(icaro_ui::editor::pestanas_editor_con_mods(p, pestana, cantidad_mods, if app.servicios.demo { 3 } else { 0 }, mods_reales::permite_mods(app), Mensaje::Pestana))
        .padding(Padding::from([0.0, espacio::S12]));
    let cuerpo: Element<Mensaje> = match pestana {
        0 => pestana_general(
            p,
            &EstadoGeneral {
                grupos: app.lista_grupos.iter().cloned().chain(["Sin grupo".to_owned()]).collect(),
                ..app.general.clone()
            },
            &MensajesGeneral {
                nombre: Mensaje::Nombre,
                grupo: Mensaje::Grupo,
                ancho: Mensaje::Ancho,
                alto: Mensaje::Alto,
                pantalla_completa: Mensaje::Completa,
                servidor: Mensaje::Servidor,
            },
        ),
        1 => pestana_java(
            p,
            &java_real::visible_config(app),
            &MensajesJava {
                java: Mensaje::JavaVersion,
                heredar_memoria: |v| Mensaje::Java(java_real::Accion::HeredarMemoria(v)),
                heredar: |v| Mensaje::Java(java_real::Accion::Heredar(v)),
                descarga: |v| Mensaje::Java(java_real::Accion::DescargaInstancia(v)),
                detectar: Mensaje::Java(java_real::Accion::Detectar),
                agregar: Mensaje::Java(java_real::Accion::Agregar(false)),
                probar: |v| Mensaje::Java(java_real::Accion::Probar(v)),
                memoria_min: Mensaje::MemMin,
                memoria_max: Mensaje::MemMax,
                metaspace: Mensaje::Metaspace,
                argumentos: Mensaje::ArgsJvm,
            },
            if app.servicios.demo {16384} else {juego_real::memoria_total_mb()},
            &if app.servicios.demo { vec![
                InstalacionJava { version: "21.0.5".into(), proveedor: "Temurin".into(), gestionada: true, ruta: "…/icaro/java/21".into(), usado_por: "1.20.5 en adelante".into(), existe: true },
                InstalacionJava { version: "17.0.13".into(), proveedor: "Temurin".into(), gestionada: true, ruta: "…/icaro/java/17".into(), usado_por: "1.18 a 1.20.4".into(), existe: true },
                InstalacionJava { version: "19".into(), proveedor: "No encontrado".into(), gestionada: false, ruta: "D:/jdk-19/bin/javaw.exe".into(), usado_por: "Ninguna".into(), existe: false },
            ] } else { juego_real::instalaciones_java(app) },
            &java_real::resumen(app),
        ),
        2 => pestana_mods(app),
        3 => pestana_mods(app),
        8 => mods_reales::packs::vista(app, icaro_core::mods::TipoPack::Shaders),
        4 => pestana_mundos(
            p,
            &if app.servicios.demo { vec![
                Mundo { nombre: "Isla del Faro".into(), modo: "Supervivencia".into(), hardcore: false, ultima_vez: "Hace 2 h".into(), tamano: "1,2 GB".into(), copias: 4 },
                Mundo { nombre: "Prueba Hardcore".into(), modo: "Hardcore".into(), hardcore: true, ultima_vez: "Hace 3 días".into(), tamano: "310 MB".into(), copias: 0 },
                Mundo { nombre: "Creativo".into(), modo: "Creativo".into(), hardcore: false, ultima_vez: "Hace 1 mes".into(), tamano: "880 MB".into(), copias: 2 },
            ] } else { Vec::new() },
            app.mundo,
            Mensaje::Mundo,
            &if app.servicios.demo { vec![
                Copia { fecha: "2026-10-09 01:12".into(), tamano: "1,1 GB".into(), motivo: "Automática al jugar".into() },
                Copia { fecha: "2026-10-04 22:40".into(), tamano: "1,0 GB".into(), motivo: "Antes de cambiar versión".into() },
            ] } else { Vec::new() },
        ),
        5 => capt::pestana(app),
        6 => if app.servicios.demo {container(consola(p, &lineas_consola(), app.filtro_consola, &app.busqueda_consola, MensajesConsola { filtro: Mensaje::FiltroConsola, buscar: Mensaje::BuscarConsola, copiar: Mensaje::Nada, limpiar: Mensaje::Nada })).padding(Padding::from([espacio::S6, espacio::S12])).into()}else{juego_real::consola(app)},
        7 => {
            pestana_archivos(
                p,
                &VistaArchivos {
                    ruta: app.exp.ruta_relativa(),
                    arbol: &app.exp.nodos,
                    elegido: app.exp.elegido,
                    contenido: app.exp.contenido.as_ref(),
                    modificado: app.exp.modificado(),
                    nota: app.exp.nota.clone(),
                    renombrando: app.exp.renombrando.as_deref(),
                },
                &MensajesArchivos {
                    elegir: Mensaje::ElegirArchivo,
                    accion: Mensaje::AccionArchivo,
                    editar: Mensaje::EditarArchivo,
                    nombre: Mensaje::NombreArchivo,
                },
            )
        }
        _ => container(Space::new(0, 0)).into(),
    };
    column![encabezado, barra, con_entrada(app, cuerpo)].into()
}

fn lineas_consola() -> Vec<Linea> {
    let l = |h: &str, n, m: &str| Linea { hora: h.into(), nivel: n, mensaje: m.into() };
    vec![
        l("14:02:11", Nivel::Info, "[main/INFO]: Loading Minecraft 1.21.4 with Fabric Loader 0.16.9"),
        l("14:02:12", Nivel::Info, "[main/INFO]: Loading 214 mods"),
        l("14:02:19", Nivel::Warn, "[main/WARN]: Mod sodium requests a newer version of fabric-api"),
        l("14:02:24", Nivel::Info, "[Render thread/INFO]: Setting user: Mineral_7"),
        l("14:02:31", Nivel::Error, "[Render thread/ERROR]: Failed to load texture minecraft:textures/block/stone.png"),
        l("14:02:33", Nivel::Info, "[Render thread/INFO]: Reloading ResourceManager"),
    ]
}

fn pantalla_consola(app: &App) -> Element<'_, Mensaje> {
    if !app.servicios.demo && (!juego_real::lineas(app).is_empty() || !app.servicios.juego.procesos.is_empty()) {return juego_real::consola(app);}
    if app.capt.vacio || !app.servicios.demo {
        return capt::vacio_consola(app);
    }
    let p = app.paleta();
    column![
        container(
            text("CONSOLA")
                .font(fuentes::DISPLAY)
                .size(texto::DISPLAY_XL.0)
                .color(p.text)
        )
        .padding(Padding::from([espacio::S6, espacio::S12])),
        con_entrada(
            app,
            container(consola(p, &lineas_consola(), app.filtro_consola, &app.busqueda_consola, MensajesConsola { filtro: Mensaje::FiltroConsola, buscar: Mensaje::BuscarConsola, copiar: Mensaje::Nada, limpiar: Mensaje::Nada }))
                .padding(Padding::from([0.0, espacio::S12]))
                .height(Length::Fill)
                .into(),
        ),
        Space::with_height(espacio::S6),
    ]
    .into()
}

fn pantalla_capturas(app: &App) -> Element<'_, Mensaje> {
    capt::pantalla(app)
}

fn pantalla_servidores(app: &App) -> Element<'_, Mensaje> {
    if !app.servicios.demo || app.servicios.servidores.visual {return servidores_reales::vista(app);}
    if app.capt.vacio || app.red.servidores.is_empty() {
        return capt::vacio_servidores(app);
    }
    let p = app.paleta();
    let acciones = AccionesLista {
        editar:None,eliminar:None,permitir_sin_estado:false,
        jugar: |i| Mensaje::Red(red::Mensaje::Jugar(i)),
        tomar: Some(|i| Mensaje::Red(red::Mensaje::Tomar(i))),
        sobre: |i| Mensaje::Red(red::Mensaje::Sobre(i)),
    };
    // Colocar dura `dur-fast`: el avance de 0 a 1 sale del reloj de la ventana.
    let colocando = app.red.colocando.map(|(i, t0)| {
        let ms = app.ahora.saturating_duration_since(t0).as_millis() as f32;
        (i, (ms / icaro_ui::tema::duracion::FAST as f32).clamp(0.0, 1.0))
    });
    let cuerpo = column![
        red_privada(p, true, "Red Amigos", "10.147.17.4", (5, 7), Mensaje::Nada),
        row![
            text("Arrastra para ordenar").size(texto::BODY_SM.0).color(p.text_muted),
            Space::with_width(Length::Fill),
            componentes::boton_con_icono(
                p,
                Icono::Mas,
                "Agregar servidor",
                Variante::Primario,
                Some(Mensaje::Red(red::Mensaje::Agregar)),
                false,
            ),
        ]
        .align_y(Alignment::Center),
        lista_servidores(p, &app.red.servidores, &acciones, app.red.arrastre, app.red.sobre, colocando),
    ]
    .spacing(espacio::S8)
    .padding(Padding::from([espacio::S8, espacio::S12]));
    column![
        banda_completa(
            p,
            Lamina::TivoliCiudad,
            Some((0.5, 0.4)),
            "Servidores",
            DatoBanda::Conteo(app.red.servidores.len()),
            ALTO_BANDA,
            texto::DISPLAY_XL,
        ),
        suave(app, cuerpo)
    ]
    .into()
}

fn pantalla_descargas(app: &App) -> Element<'_, Mensaje> {
    if let Some(trabajo)=pantallas::exportacion_nueva::trabajo(app){
        let resto=if !app.servicios.juego.orden.is_empty(){juego_real::descargas(app)}else{capt::vacio_descargas(app)};
        return column![trabajo,container(resto).height(Length::Fill)].height(Length::Fill).into();
    }
    if !app.servicios.demo && !app.servicios.juego.orden.is_empty() {return juego_real::descargas(app);}
    if app.capt.vacio || (!app.red.hay_cola() && app.red.historial.is_empty()) {
        return capt::vacio_descargas(app);
    }
    let p = app.paleta();
    let acciones = AccionesCola {
        pausar: |i| Mensaje::Red(red::Mensaje::Pausar(i)),
        cancelar: |i| Mensaje::Red(red::Mensaje::Cancelar(i)),
        reintentar: |i| Mensaje::Red(red::Mensaje::Reintentar(i)),
        quitar: |i| Mensaje::Red(red::Mensaje::QuitarRegistro(i)),
        seguir: |i| Mensaje::Red(red::Mensaje::Seguir(i)),
    };
    let cola: Element<'_, Mensaje> = if app.red.hay_cola() {
        cola_descargas(p, &app.red.descargas(), &acciones)
    } else {
        text("No hay descargas en curso.")
            .size(texto::BODY.0)
            .color(p.text_muted)
            .into()
    };
    let cuerpo = column![
        cola,
        historial(
            p,
            &app.red.historial,
            app.red.historial_abierto,
            Mensaje::Red(red::Mensaje::AlternarHistorial),
            Mensaje::Red(red::Mensaje::LimpiarHistorial),
        ),
    ]
    .spacing(espacio::S8)
    .padding(Padding::from([espacio::S8, espacio::S12]));
    let resumen = app.red.resumen();
    column![
        banda_completa(
            p,
            Lamina::MercurioFriso,
            None,
            "Descargas",
            DatoBanda::Nota(&resumen),
            ALTO_BANDA,
            texto::DISPLAY_XL,
        ),
        suave(app, cuerpo),
        limites(
            p,
            app.simultaneas,
            Mensaje::Simultaneas(app.simultaneas - 1),
            Mensaje::Simultaneas(app.simultaneas + 1),
            "Sin límite",
        ),
    ]
    .into()
}

fn pantalla_ajustes(app: &App) -> Element<'_, Mensaje> {
    let p = app.paleta();
    let secciones = pantallas::SECCIONES;
    let pagina = pantallas::pagina_ajustes(app);
    let fila = row![
        indice_ajustes(p, &secciones, app.ajuste, Mensaje::Ajuste),
        pagina
    ]
    .spacing(40.0);
    let mut contenido = column![fila].spacing(espacio::S12);
    // Acerca de cierra con el Gabinete, a todo el ancho.
    if app.ajuste == pantallas::SECCIONES.len() - 1 {
        contenido = contenido.push(capt::pared(app));
    }
    let cuerpo = contenido.padding(Padding::from([espacio::S6, espacio::S12]));
    column![
        banda(p, Lamina::FaetonPaisaje, "Ajustes", None),
        suave(app, cuerpo)
    ]
    .into()
}

pub(crate) fn con_barra_ventana<'a>(app: &App, contenido: Element<'a, Mensaje>) -> Element<'a, Mensaje> {
    container(column![icaro_ui::shell::barra_titulo(app.paleta(), &MensajesShell {
        ir_a: Box::new(Mensaje::Ir),
        arrastrar: Mensaje::Arrastrar,
        buscar: Mensaje::AbrirBusqueda,
        cuenta: Mensaje::Ses(sesion::Accion::Menu(true)),
        minimizar: Mensaje::Minimizar,
        maximizar: Mensaje::Maximizar,
        cerrar: Mensaje::Cerrar,
    }), container(contenido).width(Length::Fill).height(Length::Fill)]
    .width(Length::Fill).height(Length::Fill))
    .style(icaro_ui::estilo::bloque(app.paleta().bg)).into()
}

fn vista(app: &App) -> Element<'_, Mensaje> {
    mods::fijar_recurso(app.editor == Some(3));
    let p = app.paleta();
    let contenido = if let Some(pant) = app.pantalla {
        pantallas::vista_pantalla(app, pant)
    } else {
        match app.seccion {
        Seccion::Instancias => pantalla_instancias(app),
        Seccion::Servidores => pantalla_servidores(app),
        Seccion::Descargas => pantalla_descargas(app),
        Seccion::Ajustes => pantalla_ajustes(app),
        Seccion::Modpacks => pantalla_modpacks(app),
        Seccion::Consola => pantalla_consola(app),
        Seccion::Capturas => pantalla_capturas(app),
        }
    };
    let contenido = if let Ok(scene)=std::env::var("ICARO_VISTA_DISENO") {componentes_nuevos_qa::vista(app,&scene).unwrap_or(contenido)}else{contenido};
    let contenido = if let Some(banner) = actualizacion_real::banner(app) {
        column![banner, container(contenido).height(Length::Fill)].height(Length::Fill).into()
    } else { contenido };
    let pie = app.seleccionada.and_then(|i| {
        let lista = instancias();
        let datos = lista.get(i)?;
        let cuenta_activa = if app.servicios.demo {
            app.sesion.estado == icaro_ui::cuenta::EstadoSesion::Activa
        } else {
            app.servicios.session.is_some()
        };
        if !cuenta_activa || app.pantalla.is_some() || app.editor.is_some()
            || app.seccion != Seccion::Instancias
            || app.picar.as_ref().is_some_and(|p| p.instancia == i && p.rota.is_some()) {
            return None;
        }
        let pie = row![
            icono_instancia::imagen(app,i,24.),
            column![
                text(datos.nombre.clone())
                    .font(fuentes::TITULO)
                    .size(texto::HEADING.0)
                    .color(p.text),
                text(datos.version.clone())
                    .font(fuentes::MONO)
                    .size(texto::MONO_SM.0)
                    .color(p.text_muted),
            ],
            Space::with_width(Length::Fill),
            if app.servicios.demo {componentes::boton(p, "Jugar", Variante::Primario, Some(Mensaje::Jugar(i)))}else{juego_real::boton(app,i)},
        ]
        .align_y(Alignment::Center)
        .spacing(12)
        .padding(Padding::from([espacio::S2, espacio::S12]));
        let pie: Element<Mensaje> = match sesion::banner_jugar(app) {
            Some(banner) => column![banner, pie].into(),
            None => pie.into(),
        };
        Some(pie)
    });
    let mut ventana = if app.pantalla == Some(pantallas::Pantalla::Actualizacion) {
        con_barra_ventana(app, contenido)
    } else { app_shell(
        p,
        if matches!(app.pantalla, Some(pantallas::Pantalla::Cuenta | pantallas::Pantalla::Estudio)) { None } else { Some(app.seccion) },
        app.barra_colapsada(),
        Cuenta { nombre: app.servicios.nombre(app.sesion.estado.nombre()), proveedor: if app.servicios.session.as_ref().is_some_and(|s|s.es_local()) {"Sin conexión"} else {"Microsoft"}, estado: app.sesion.estado, textura:cuenta_sin_conexion::textura_activa(app) },
        if app.servicios.demo {Some(DescargasActivas { cantidad: 2, velocidad: "6,1 MB/s".into(), avance: 0.58 })}else{juego_real::indicador(app)},
        contenido,
        pie,
        MensajesShell {
            ir_a: Box::new(Mensaje::Ir),
            arrastrar: Mensaje::Arrastrar,
            buscar: Mensaje::AbrirBusqueda,
            cuenta: Mensaje::Ses(sesion::Accion::Menu(true)),
            minimizar: Mensaje::Minimizar,
            maximizar: Mensaje::Maximizar,
            cerrar: Mensaje::Cerrar,
        },
    ) };
    let capa_menu = if app.menu.is_some() {
        let grupo_menu = app.grupos[app.menu.unwrap_or(0)].clone();
        let mut elementos_grupos: Vec<ElementoMenu<Mensaje>> = app
            .lista_grupos
            .iter()
            .map(|g| ElementoMenu::Opcion {
                icono: if grupo_menu.as_ref() == Some(g) { Icono::Check } else { Icono::Carpeta },
                texto: g.clone().into(),
                atajo: None,
                peligro: false,
                mensaje: Mensaje::MoverGrupo(Some(g.clone())),
            })
            .collect();
        elementos_grupos.push(ElementoMenu::Opcion { icono: if grupo_menu.is_none() { Icono::Check } else { Icono::Carpeta }, texto: "Sin grupo".into(), atajo: None, peligro: false, mensaje: Mensaje::MoverGrupo(None) });
        elementos_grupos.push(ElementoMenu::Separador);
        elementos_grupos.push(ElementoMenu::Opcion { icono: Icono::Mas, texto: "Nuevo grupo".into(), atajo: None, peligro: false, mensaje: Mensaje::NuevoGrupoParaInstancia });
        elementos_grupos.push(ElementoMenu::Opcion { icono: Icono::ChevronDerecha, texto: "Volver".into(), atajo: None, peligro: false, mensaje: Mensaje::VerGrupos(false) });
        let mut elementos = if app.menu_grupos { elementos_grupos } else { vec![
                ElementoMenu::Opcion { icono: Icono::Jugar, texto: "Jugar".into(), atajo: Some("Enter"), peligro: false, mensaje: Mensaje::Jugar(app.menu.unwrap_or(0)) },
                ElementoMenu::Opcion { icono: Icono::Carpeta, texto: "Abrir carpeta".into(), atajo: Some("O"), peligro: false, mensaje: Mensaje::Pant(pantallas::Accion::AbrirCarpetaInstancia(app.menu.unwrap_or(0))) },
                ElementoMenu::Opcion { icono: Icono::Editar, texto: "Editar".into(), atajo: Some("E"), peligro: false, mensaje: Mensaje::Editar },
                ElementoMenu::Opcion { icono: Icono::Imagen, texto: "Cambiar icono".into(), atajo: None, peligro: false, mensaje: Mensaje::Icono(icono_instancia::Accion::Abrir(app.menu.unwrap_or(0))) },
                ElementoMenu::Separador,
                ElementoMenu::Opcion { icono: Icono::Carpeta, texto: "Mover a grupo".into(), atajo: Some(">"), peligro: false, mensaje: Mensaje::VerGrupos(true) },
                ElementoMenu::Separador,
                ElementoMenu::Opcion { icono: Icono::Copiar, texto: "Duplicar".into(), atajo: None, peligro: false, mensaje: Mensaje::Pant(pantallas::Accion::Duplicar(app.menu.unwrap_or(0))) },
                ElementoMenu::Opcion { icono: Icono::Actualizar, texto: "Cambiar versión".into(), atajo: None, peligro: false, mensaje: Mensaje::Pant(pantallas::Accion::CambiarVersion(app.menu.unwrap_or(0))) },
                ElementoMenu::Opcion { icono: Icono::Actualizar, texto: "Cambiar versión del loader".into(), atajo: None, peligro: false, mensaje: Mensaje::Pant(pantallas::Accion::CambiarVersionLoader(app.menu.unwrap_or(0))) },
                ElementoMenu::Opcion { icono: Icono::Subir, texto: "Exportar instancia".into(), atajo: None, peligro: false, mensaje: Mensaje::Pant(pantallas::Accion::Exportar(app.menu.unwrap_or(0))) },
                ElementoMenu::Opcion { icono: Icono::Externo, texto: "Crear acceso directo".into(), atajo: None, peligro: false, mensaje: Mensaje::Menu(None) },
                ElementoMenu::Separador,
                ElementoMenu::Opcion { icono: Icono::Papelera, texto: "Eliminar".into(), atajo: Some("Supr"), peligro: true, mensaje: Mensaje::PedirEliminar },
            ] };
        let posicion = posicion_menu(app.menu_en, app.ventana, tamano_menu(&elementos),
            if app.reducir { 1.0 } else { progreso(app.anim_menu, app.ahora, MS_MENU) });
        if !app.servicios.demo && app.servicios.instancias.registros.get(app.menu.unwrap_or(0)).is_some_and(|i|i.loader=="vanilla") {
            elementos.retain(|e|!matches!(e,ElementoMenu::Opcion{mensaje:Mensaje::Pant(pantallas::Accion::CambiarVersionLoader(_)),..}));
        }

        let menu = menu_contextual(p, elementos);
        Some(iced::widget::opaque(iced::widget::mouse_area(
                container(menu)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .padding(Padding {
                    top: posicion.y,
                    left: posicion.x,
                    ..Padding::ZERO
                }),
            )
            .on_press(Mensaje::Menu(None))
            .on_scroll(|_| Mensaje::Menu(None))).into())
    } else { None };
    ventana = icaro_ui::superficies::capa_flotante(ventana, capa_menu);
    let capa_mod = if let (Some(id), Some(2 | 3 | 8)) = (&app.mod_panel, app.editor) {
        let t = icaro_ui::movimiento::Curva::Salida.aplicar(progreso(app.anim_panel, app.ahora, MS_PANEL));
        let panel = icaro_ui::efectos::panel_desde_derecha(
                iced::widget::opaque(container(ficha_mod(app, id)).height(Length::Fill)).into(),
                (1.0 - t) * 640.0,
            );
        Some(container(iced::widget::opaque(iced::widget::mouse_area(panel)
                .on_press(Mensaje::PanelMod(None))))
            .padding(Padding { top: 38.0, ..Padding::ZERO })
            .width(Length::Fill)
            .height(Length::Fill).into())
    } else { None };
    ventana = icaro_ui::superficies::capa_flotante(ventana, capa_mod);
    ventana=icaro_ui::superficies::capa_flotante(ventana,servidores_reales::panel(app));
    if let Some(s) = &app.asistente {
        ventana = con_velo_losa(p, ventana, pantallas::asistente(app, s), Mensaje::Pant(pantallas::Accion::AsistenteCancelar), desfase_losa(app));
    }
    if app.servicios.modpacks.abierto {
        ventana=con_velo_losa(p,ventana,modpacks_reales::vista(app),Mensaje::Modpacks(modpacks_reales::Accion::Cerrar),desfase_losa(app));
    }
    if let Some(i) = app.cambiar {
        ventana = con_velo_losa(p, ventana, pantallas::cambiar_version(app, i), Mensaje::Pant(pantallas::Accion::CambiarCancelar), desfase_losa(app));
    }
    if let Some(s) = &app.importar {
        ventana = con_velo_losa(p, ventana, pantallas::importar(app, s), Mensaje::Pant(pantallas::Accion::ImportarCancelar), desfase_losa(app));
    }
    if let Some(s) = app.exportar.as_ref().filter(|s|!s.guardando) {
        ventana = con_velo_losa(p, ventana, pantallas::exportar(app, s), Mensaje::Pant(pantallas::Accion::ExportarCancelar), desfase_losa(app));
    }
    if app.picar_objetivo.is_some() && !app.dialogo {
        ventana=con_velo_losa(p,ventana,picar_real::vista(app),Mensaje::CerrarDialogo,desfase_losa(app));
    }
    if app.dialogo {
        let nombre = if app.servicios.demo { "Supervivencia".to_owned() } else {
            app.servicios.instancias.registros.iter()
                .find(|i| Some(&i.id) == app.servicios.instancias.eliminar.as_ref())
                .map(|i| i.nombre.clone()).unwrap_or_default()
        };
        let ocupado = app.servicios.instancias.operando;
        let cuerpo = text(if app.servicios.demo { "Se borrarán la instancia, sus 214 mods y 3 mundos (2,4 GB). Esta acción no se puede deshacer." } else {
            "Hay mundos sin una copia de seguridad vigente. La instancia se moverá a la papelera del sistema y podrás recuperarla con Deshacer. Confirma para continuar con la eliminación."
        })
            .size(texto::BODY.0)
            .color(p.text_muted)
            .into();
        ventana = con_velo_losa(
            p,
            ventana,
            dialogo(
                p,
                &format!("¿Eliminar {nombre}?"),
                cuerpo,
                vec![
                    componentes::boton(p, "Cancelar", Variante::Secundario, (!ocupado).then_some(Mensaje::CerrarDialogo)),
                    componentes::boton(p, &if ocupado { "Eliminando…".to_owned() } else { "Continuar con la eliminación".to_owned() }, Variante::Peligro, (!ocupado).then_some(Mensaje::Eliminar)),
                ],
                Some(Lamina::FaetonMano),
                620.0,
                300.0,
                Mensaje::CerrarDialogo,
            ),
            Mensaje::CerrarDialogo,
            desfase_losa(app),
        );
    }

    // Mantener el árbol de la vista debajo del Umbral evita remontar la
    // instancia, sus imágenes y sus controles cuando se retira la animación.
    let umbral: Element<'_, Mensaje> = if let Some(t) = app.umbral {
        let nombre = app.seleccionada.and_then(|i| con_instancias(|l| l.get(i).map(|d| d.nombre.clone()))).unwrap_or_default();
        let pref = icaro_ui::movimiento::Preferencia { reducir: app.reducir };
        if app.servicios.demo {
            icaro_ui::efectos::umbral(p, ms_desde(t, app.ahora), pref, &nombre, Mensaje::UmbralVolver)
        } else {
            icaro_ui::efectos::umbral_registro_transicion(
                p, ms_desde(t,app.ahora), pref,
                &juego_real::lineas(app).iter().rev().take(8).collect::<Vec<_>>().into_iter().rev()
                    .map(|l|format!("[{}] {}",l.hora,l.mensaje)).collect::<Vec<_>>(),
                juego_real::retirada_umbral(app), Mensaje::UmbralVolver,
            )
        }
    } else {
        Space::new(Length::Fill, Length::Fill).into()
    };
    ventana = iced::widget::stack![ventana, umbral].into();
    let ventana = sesion::con_selector(app, ventana);
    let ventana = if let Some(lista)=exportar_lista_mods::vista(app){con_velo_losa(p,ventana,lista,Mensaje::ExportarLista(exportar_lista_mods::Accion::Cerrar),desfase_losa(app))}else{ventana};
    let ventana = if let Some(icono) = icono_instancia::vista(app) {con_velo_losa(p,ventana,icono,Mensaje::Icono(icono_instancia::Accion::Cerrar),desfase_losa(app))}else{ventana};
    let mut ventana = mods_reales::packs::capa(app, mods_reales::capa(app, servicios::capa(app, capt::capa(app, ventana))));
    let mut avisos=column![].spacing(8);
    for entry in app.avisos.entradas() {
        let a=&entry.aviso; let id=entry.id;
        let toast=toast_con_opacidad(p,if a.error{Icono::Alerta}else{Icono::Check},&a.texto,a.accion.clone().map(|m|(a.etiqueta_accion.as_str(),Mensaje::AccionAviso(id,Box::new(m)))),Some(Mensaje::CerrarAvisoId(id)),a.error,a.opacidad(app.ahora,app.reducir));
        avisos=avisos.push(iced::widget::mouse_area(icaro_ui::efectos::aviso_desde_abajo(toast,a.entrada(app.ahora,app.reducir))).on_enter(Mensaje::PausarAviso(id,true)).on_exit(Mensaje::PausarAviso(id,false)));
    }
    if let Some(a)=&app.aviso {avisos=avisos.push(toast_con_opacidad(p,if a.error{Icono::Alerta}else{Icono::Check},&a.texto,a.accion.clone().map(|m|(a.etiqueta_accion.as_str(),m)),Some(Mensaje::CerrarAviso),a.error,a.opacidad(app.ahora,app.reducir)));}
    let aviso:Element<Mensaje>=container(avisos).align_x(iced::alignment::Horizontal::Left).align_y(iced::alignment::Vertical::Bottom).width(Length::Fill).height(Length::Fill).padding(Padding {left:if app.colapsada{88.}else{256.},bottom:24.,top:0.,right:24.}).into();
    ventana = iced::widget::stack![ventana, aviso].into();
    if app.busqueda.abierta {
        ventana = icaro_ui::superficies::con_velo(p, ventana,
            busqueda::vista(app), Mensaje::CerrarBusqueda);
    }
    ventana::con_bordes(iced::widget::mouse_area(ventana)
        .on_move(Mensaje::Cursor)
        .into(), app.maximizada)
}

fn main() -> iced::Result {
    let mut arguments = std::env::args();
    let _ = arguments.next();
    if arguments.next().as_deref() == Some("--oauth-return") {
        if let Some(url) = arguments.next() {
            // El proceso de retorno solo entrega la respuesta; nunca abre otra cuenta/ventana.
            let _ = icaro_core::auth::forward_app_return(&url);
        }
        return Ok(());
    }
    let mut app = iced::application("Icaro Launcher", actualizar, vista)
        .theme(|app: &App| if app.modo==Modo::Piedra {iced::Theme::Light}else{iced::Theme::Dark})
        .scale_factor(|app: &App| app.aj.escala.clamp(80,150) as f64/100.)
        .default_font(fuentes::CUERPO)
        .subscription(|app| {
            let mut subs = vec![window::resize_events().map(|(_, s)| Mensaje::Tamano(s)),
                icaro_ui::laminas::suscripcion().map(|()| Mensaje::ImagenesActualizadas)];
            if !app.servicios.demo && app.servicios.session.is_some() {
                subs.push(iced::time::every(std::time::Duration::from_secs(30)).map(|_| Mensaje::Servicio(servicios::Accion::Renovar)));
            }
            if !app.servicios.demo && app.pantalla == Some(pantallas::Pantalla::Codigo) {
                subs.push(iced::time::every(std::time::Duration::from_secs(1)).map(Mensaje::Cuadro));
            }
            if app.tema==2 {subs.push(iced::time::every(std::time::Duration::from_secs(5)).map(|_|Mensaje::ComprobarTemaSistema));}
            if !app.servicios.juego.procesos.is_empty() {
                subs.push(iced::time::every(std::time::Duration::from_millis(600)).map(Mensaje::Cuadro));
            }
            // Las descargas reales no piden cuadros: sus barras (`progreso`) se
            // redibujan solas sin reconstruir la vista y el avance llega por mensajes.
            if !app.avisos.esta_vacia() {
                let tic = if app.avisos.animando(app.ahora, app.reducir) {
                    std::time::Duration::from_millis(16)
                } else {
                    icaro_ui::avisos::TIC_QUIETO
                };
                subs.push(iced::time::every(tic).map(Mensaje::AvisoTic));
            }
            if let Some(aviso) = &app.aviso {
                let ms = if !app.reducir && aviso.saliendo(app.ahora) { 16 } else { 60 };
                subs.push(iced::time::every(std::time::Duration::from_millis(ms)).map(Mensaje::AvisoTic));
            }
            if !app.instalando.is_empty()
                || app.pantalla == Some(pantallas::Pantalla::Integridad)
                || (app.servicios.demo && app.pantalla == Some(pantallas::Pantalla::Codigo))
                || app.red.animando(app.seccion == Seccion::Descargas)
                || progreso(app.anim, app.ahora, MS_ENTRADA) < 1.0
                || progreso(app.anim_menu, app.ahora, MS_MENU) < 1.0
                || progreso(app.anim_panel, app.ahora, MS_PANEL) < 1.0
                || (hay_dialogo(app) && progreso(app.anim_dialogo, app.ahora, MS_LOSA) < 1.0)
                || sesion::animando(app)
                || app.capt.animando(app.ahora, app.reducir)
                || (!app.reducir && app.servicios.instancias.cambio.as_ref().is_some_and(|c| !c.confirmando && !c.cargando_compatibilidad.is_empty()))
            {
                subs.push(window::frames().map(Mensaje::Cuadro));
            }
            let umbral_activo = if app.servicios.demo {
                app.umbral.is_some_and(|t| ms_desde(t, app.ahora) < 1700.0)
            } else {
                juego_real::animando_umbral(app)
            };
            let picar_activo = app.picar.as_ref().is_some_and(|p| {
                p.rota.map_or(true, |t| ms_desde(t, app.ahora) < icaro_ui::movimiento::PICAR_ROTURA_MS + 80.0)
            });
            let colocada_activa = app
                .colocada
                .is_some_and(|(_, t)| ms_desde(t, app.ahora) < icaro_ui::tema::duracion::FAST as f32);
            if umbral_activo || picar_activo || colocada_activa {
                subs.push(window::frames().map(Mensaje::Cuadro));
            }
            subs.push(iced::event::listen_with(|evento, _, _| {
                match evento {
                    iced::Event::Window(window::Event::CloseRequested) => Some(Mensaje::Cerrar),
                    iced::Event::Window(window::Event::Unfocused) | iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) => Some(Mensaje::PicarSoltar),
                    iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key:iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape), .. }) => Some(Mensaje::Escape),
                    iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) =>
                        atajos::global(&key, modifiers),
                    _ => None,
                }
            }));
            if app.iconos.dialogo.is_some() {
                let subs_icono = iced::event::listen_with(|e,_,_|Some(Mensaje::IconoEvento(e)));
                subs.push(subs_icono);
                return iced::Subscription::batch(subs);
            }
            if app.busqueda.abierta {
                subs.push(iced::event::listen_with(|evento, _, _| match evento {
                    iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) => match key {
                        iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape) => Some(Mensaje::CerrarBusqueda),
                        iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowUp) => Some(Mensaje::MoverBusqueda(-1)),
                        iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowDown) => Some(Mensaje::MoverBusqueda(1)),
                        _ => None,
                    },
                    _ => None,
                }));
                return iced::Subscription::batch(subs);
            }
            if app.servicios.local.skin.is_some() {
                subs.push(iced::event::listen_with(|e,_,_| match e {
                    iced::Event::Window(window::Event::FileHovered(_))=>Some(skin_sin_conexion::m(skin_sin_conexion::Accion::Encima(true))),
                    iced::Event::Window(window::Event::FilesHoveredLeft)=>Some(skin_sin_conexion::m(skin_sin_conexion::Accion::Encima(false))),
                    iced::Event::Window(window::Event::FileDropped(p))=>Some(skin_sin_conexion::m(skin_sin_conexion::Accion::Soltar(p))),
                    _=>None,
                }));
                return iced::Subscription::batch(subs);
            }
            if app.servicios.modpacks.abierto {
                subs.push(iced::event::listen_with(|e,_,_| match e {
                    iced::Event::Window(window::Event::FileHovered(_) | window::Event::FilesHoveredLeft | window::Event::FileDropped(_)) => Some(Mensaje::ModpackEvento(e)),
                    iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Enter), .. }) => Some(Mensaje::ModpackEvento(e)),
                    _=>None,
                }));
            }
            subs.push(iced::keyboard::on_key_press(|k, _| {
                (k == iced::keyboard::Key::Named(iced::keyboard::key::Named::Delete)).then_some(Mensaje::PicarEmpezar)
            }));
            subs.push(iced::keyboard::on_key_release(|k, _| {
                (k == iced::keyboard::Key::Named(iced::keyboard::key::Named::Delete)).then_some(Mensaje::PicarSoltar)
            }));
            if matches!(app.pantalla, Some(pantallas::Pantalla::Estudio)) {
                // Flechas: giran la escena en pasos de 45°. Soltar una .png la carga.
                subs.push(iced::keyboard::on_key_press(|tecla, _| match tecla {
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowLeft) => Some(Mensaje::Ses(sesion::Accion::Girar(-1))),
                    iced::keyboard::Key::Named(iced::keyboard::key::Named::ArrowRight) => Some(Mensaje::Ses(sesion::Accion::Girar(1))),
                    _ => None,
                }));
                subs.push(iced::event::listen_with(|evento, _, _| match evento {
                    iced::Event::Window(window::Event::FileHovered(_)) => Some(Mensaje::Ses(sesion::Accion::Soltando(true))),
                    iced::Event::Window(window::Event::FilesHoveredLeft) => Some(Mensaje::Ses(sesion::Accion::Soltando(false))),
                    iced::Event::Window(window::Event::FileDropped(ruta)) => Some(Mensaje::Ses(sesion::Accion::Soltar(ruta))),
                    _ => None,
                }));
            }
            if app.red.arrastre.is_some() {
                // Soltar en cualquier parte termina el orden; Escape lo cancela.
                subs.push(iced::event::listen_with(|e, _, _| match e {
                    iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) => {
                        Some(Mensaje::Red(red::Mensaje::Soltar))
                    }
                    iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                        key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                        ..
                    }) => Some(Mensaje::Red(red::Mensaje::CancelarOrden)),
                    _ => None,
                }));
            }
            subs.push(capt::suscripcion(app));
            if app.editor == Some(7) && app.exp.renombrando.is_some() {
                subs.push(iced::event::listen_with(|e, _, _| match e {
                    iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape), .. }) => Some(Mensaje::AccionArchivo(AccionArchivo::CancelarNombre)),
                    _ => None,
                }));
            }
            subs.push(servicios::suscripcion(app));
            subs.push(mods_reales::suscripcion(app));
            subs.push(servidores_reales::suscripcion(app));
            subs.push(red_privada_real::suscripcion(app));
            if app.servicios.servidores.arrastre.is_some(){
                subs.push(iced::event::listen_with(|e,_,_|match e{
                    iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left))=>Some(Mensaje::Servidores(servidores_reales::Accion::Soltar)),
                    _=>None,
                }));
            }
            iced::Subscription::batch(subs)
        })
        .window(window::Settings {
            decorations: false,
            resizable: true,
            exit_on_close_request: false,
            size: if std::env::var_os("ICARO_CAPTURA_DISENO").is_some() && std::env::var("ICARO_VISTA_DISENO").is_ok_and(|v| v.ends_with("angosto")) { Size::new(940.,720.) } else { Size::new(1280.0, 800.0) },
            min_size: Some(Size::new(940.0, 600.0)),
            icon: window::icon::from_file_data(
                include_bytes!("../assets/icono/icaro-256.png"),
                None,
            )
            .ok(),
            ..Default::default()
        });
    for f in fuentes::BYTES {
        app = app.font(f);
    }
    // Variables opcionales para abrir el ejemplo en un estado concreto:
    // ICARO_SECCION (1 a 7), ICARO_PIEDRA, ICARO_MENU, ICARO_DIALOGO.
    let seccion = std::env::var("ICARO_SECCION")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .and_then(|n| Seccion::TODAS.get(n.wrapping_sub(1)).copied())
        .unwrap_or(Seccion::Instancias);
    let piedra = std::env::var_os("ICARO_PIEDRA").is_some();
    let menu = std::env::var_os("ICARO_MENU").is_some().then_some(0);
    let con_dialogo = std::env::var_os("ICARO_DIALOGO").is_some();
    app.run_with(move || {
        let (app, inicio) = if std::env::var_os("ICARO_CAPTURA_DISENO").is_some() {
            pruebas_diseno::iniciar(piedra)
        } else {
            iniciar_app(seccion, piedra, menu, con_dialogo, servicios::Estado::default())
        };
        (app, Task::batch([inicio, ventana::configurar_esquinas()]))
    })
}

fn iniciar_app(seccion: Seccion, piedra: bool, menu: Option<usize>, con_dialogo: bool, mut servicios: servicios::Estado) -> (App, Task<Mensaje>) {
    if !servicios.demo { servicios.instancias.cargar(); let _=icaro_core::almacenamiento::registrar(&estado::directorio(false),"Inicio del launcher"); icaro_core::mods::sincronizacion::activar_registro(&estado::directorio(false)); }
    let mut guardado = estado::Guardado::cargar(servicios.demo);
    if let Some(g)=&mut guardado {g.apariencia.normalizar();}
    let (mut app, inicio) = iniciar_con_guardado(seccion, piedra, menu, con_dialogo, servicios, guardado);
    if let Some(error)=app.config_error.clone(){app.avisar(error);}
    if !app.servicios.demo {
        if let Some(resultado)=icaro_core::actualizacion::tomar_resultado(env!("CARGO_PKG_VERSION"),env!("ICARO_COMPILACION").parse().unwrap_or(0)) {
            app.actualizacion.resultado=Some(resultado);
            app.pantalla=Some(pantallas::Pantalla::Actualizacion);
        }
        app.actualizacion.comprobando = true;
        {
            let servidores = if app.seccion == Seccion::Servidores {
                servidores_reales::actualizar(&mut app, servidores_reales::Accion::Abrir)
            } else { Task::none() };
            let iconos = icono_instancia::cargar(&app);
            let java = java_real::detectar(&mut app);
            let actualizacion = actualizacion_real::comprobar(app.aj.canal);
            (app, Task::batch([inicio, actualizacion, iconos, java, servidores]))
        }
    } else { (app, inicio) }
}

fn iniciar_con_guardado(seccion: Seccion, piedra: bool, menu: Option<usize>, con_dialogo: bool, mut servicios: servicios::Estado, mut guardado: Option<estado::Guardado>) -> (App, Task<Mensaje>) {
        let demo = servicios.demo;
        let mut config_error = None;
        if !demo {
            if guardado.is_none() { guardado = Some(estado::Guardado::nuevo(&[], &[], &[], &[], &[])); }
            if let Some(g) = &mut guardado {
                g.restaurar_instancias(servicios.instancias.registros.iter().map(|i| i.id.clone()).collect());
                if let Err(error) = g.cargar_configs(&servicios.instancias.almacen) {
                    config_error=Some(format!("No se pudo recuperar la configuración; se conservó el archivo para corregirlo: {error}"));
                }
            }
        }
        *lista_instancias().lock().unwrap() = if demo { instancias_base() } else {
            servicios.instancias.registros.iter().map(|i|instancias_reales::Estado::datos(&servicios.instancias.almacen,i)).collect()
        };
        if !demo {
            for (n, record) in servicios.instancias.registros.iter().enumerate().filter(|(_, i)| i.loader != "vanilla") {
                match icaro_core::mods::inventario(&servicios.instancias.almacen, &record.id) {
                    Ok(list) => {
                        lista_instancias().lock().unwrap()[n].mods = format!("{} mods", list.len());
                        servicios.mods.listas.insert(record.id.clone(), list);
                    }
                    Err(e) => { servicios.mods.errores.insert(record.id.clone(), e.to_string()); }
                }
            }
        }
        Lamina::precargar(if piedra { Modo::Piedra } else { Modo::Tinta });
        // Vuelve a crear las instancias que vinieron de modpacks.
        if let Some(g) = guardado.as_ref().filter(|_| demo) {
            for nombre in &g.modpacks {
                if let Some(m) = modpacks::CATALOGO.iter().find(|m| m.nombre == nombre) {
                    lista_instancias().lock().unwrap().push(DatosInstancia {
                        nombre: m.nombre.into(),
                        version: format!("{} · {}", m.minecraft, m.cargador),
                        ultima_vez: "Hace un momento".into(),
                        mods: format!("{} mods", m.mods),
                        portada: m.portada,
                        estado: EstadoInstancia::Lista,
                        seleccionada: false,
                        detalle: String::new(),
                    });
                }
            }
        }
        let total = con_instancias(|l| l.len());
        let prefs=guardado.as_ref().map(|g|g.apariencia.clone()).unwrap_or_else(||estado::AparienciaGuardada {tema:usize::from(piedra),..Default::default()});
        icaro_ui::movimiento::fijar_reducir(prefs.reducir);
        (
            App {
                config_error,
                detalle_error: None,
                almacenamiento: Default::default(),
                estadisticas: Default::default(),
                actualizacion: actualizacion_real::Estado::default(),
                verificacion: Default::default(),
                iconos: Default::default(),
                exportar_lista: Default::default(),
                servicios,
                modo: if prefs.tema==1 || (prefs.tema==2 && apariencia_real::sistema_claro()) {Modo::Piedra}else{Modo::Tinta},
                busqueda: busqueda::Estado::default(),
                seccion,
                colapsada: false,
                barra_angosta: false,
                filtro: 0,
                seleccionada: std::env::var("ICARO_EDITOR").ok().and_then(|_| (total > 0).then_some(0)),
                menu: if demo { menu } else { None },
                dialogo: demo && con_dialogo,
                aviso: None,
                avisos: Default::default(),
                tema: prefs.tema,
                reducir: prefs.reducir,
                simultaneas: 4,
                ajuste: std::env::var("ICARO_AJUSTE").ok().and_then(|v| v.parse().ok()).unwrap_or(1),
                editor: demo.then(|| std::env::var("ICARO_EDITOR").ok().and_then(|v| v.parse().ok())).flatten().filter(|_| total > 0),
                general: EstadoGeneral {
                    nombre: if demo { "Supervivencia".into() } else { String::new() },
                    grupo: demo.then(|| "Con amigos".to_owned()),
                    grupos: if demo { vec!["Con amigos".into(), "Técnicos".into(), "Sin grupo".into()] } else { vec!["Sin grupo".into()] },
                    ancho: "1920".into(),
                    alto: "1080".into(),
                    pantalla_completa: false,
                    servidor: demo.then_some("Amigos · 10.147.17.1"),
                },
                cerrados: guardado.as_ref().map_or_else(Vec::new, |g| g.cerrados.clone()),
                busqueda_mods: std::env::var("ICARO_BUSCAR").unwrap_or_default(),
                mods_vista: usize::from(demo && std::env::var("ICARO_TIENDA").is_ok()),
                mods_filtro: 0,
                mod_panel: std::env::var("ICARO_PANEL").ok(),
                anim_panel: None,
                anim_dialogo: None,
                tienda_lista: Vec::new(),
                tienda_cat: Vec::new(),
                tienda_lado: 2,
                tienda_orden: 0,
                modpack_filtro: 0,
                pantalla: if !demo { Some(pantallas::Pantalla::Primera) } else { match std::env::var("ICARO_PANTALLA").ok().as_deref() {
                    Some("primera") => Some(pantallas::Pantalla::Primera),
                    Some("codigo") => Some(pantallas::Pantalla::Codigo),
                    Some("fallo") => Some(pantallas::Pantalla::Fallo),
                    Some("tienda") => Some(pantallas::Pantalla::ErrorTienda),
                    Some("noverif") => Some(pantallas::Pantalla::NoVerificados),
                    Some("sync") => Some(pantallas::Pantalla::Sincronizar),
                    Some("integridad") => Some(pantallas::Pantalla::Integridad),
                    Some("update") => Some(pantallas::Pantalla::Actualizacion),
                    Some("cuenta") => Some(pantallas::Pantalla::Cuenta),
                    Some("estudio") => Some(pantallas::Pantalla::Estudio),
                    _ => guardado.is_none().then_some(pantallas::Pantalla::Primera),
                } },
                aj: pantallas::Ajustes {metal:prefs.metal,escala:prefs.escala,canal:guardado.as_ref().and_then(|g|g.canal).unwrap_or_else(actualizacion_real::canal_compilado),..Default::default()},
                asistente: (demo && std::env::var("ICARO_DIALOGO_NUEVO").is_ok()).then(|| pantallas::Asistente {
                    paso: std::env::var("ICARO_PASO").ok().and_then(|v| v.parse().ok()).unwrap_or(0),
                    tipo_version: 0,
                    busqueda: String::new(),
                    version: 0,
                    loader: 1,
                    nombre: "Fabric 1.21.4".into(),
                    portada: 0,
                }),
                cambiar: (demo && std::env::var("ICARO_CAMBIAR").is_ok()).then_some(0),
                importar: (demo && std::env::var("ICARO_IMPORTAR").is_ok()).then(|| pantallas::Importar { origen: 0, archivo: None, launchers: [true, false, false], clon_origen: 0, con_mundos: false }),
                exportar: (demo && std::env::var("ICARO_EXPORTAR").is_ok()).then(|| pantallas::Exportar { instancia: 0, nombre: "Supervivencia".into(), formato:0, destino:std::env::temp_dir().to_string_lossy().into_owned(), error:None, guardando:false, arbol:Some(pantallas::exportacion_nueva::demo()),preset:0,hechos:0,total:0 }),
                codigo_inicio: std::time::Instant::now(),
                codigo_fase: if std::env::var("ICARO_CODIGO").as_deref() == Ok("vencido") { pantallas::FaseCodigo::Vencido } else { pantallas::FaseCodigo::Esperando },
                intentos: 0,
                detalle_fallo: false,
                modpack_pagina: std::env::var("ICARO_PACK").ok().and_then(|v| v.parse().ok()),
                modpack_pestana: std::env::var("ICARO_PACKTAB").ok().and_then(|v| v.parse().ok()).unwrap_or(0),
                modpack_galeria: 0,
                busqueda_modpacks: String::new(),
                instalando: Vec::new(),
                mod_pagina: std::env::var("ICARO_MOD").ok(),
                mod_filtro_mc: "Todas".into(),
                mod_filtro_loader: "Todos".into(),
                mod_betas: false,
                mod_galeria: 0,
                mod_opcionales: Vec::new(),
                mod_pestana: std::env::var("ICARO_MODTAB").ok().and_then(|v| v.parse().ok()).unwrap_or(0),
                ultimo_guardado: String::new(),
                anim: None,
                anim_menu: None,
                red: if demo { red::Estado::nuevo() } else { red::Estado::vacio() },
                ahora: std::time::Instant::now(),
                ultimo_clic: None,
                menu_grupos: false,
                lista_grupos: guardado.as_ref().map_or_else(|| if demo { vec!["Con amigos".into(), "Técnicos".into()] } else { Vec::new() }, |g| g.lista_grupos.clone()),
                edicion_grupo: None,
                grupos: guardado.as_ref().filter(|g| g.grupos.len() == total).map_or_else(
                    || {
                        let mut v: Vec<Option<String>> = if demo { vec![Some("Con amigos".into()), Some("Técnicos".into()), Some("Con amigos".into()), None, None] } else { Vec::new() };
                        v.resize(total, None);
                        v
                    },
                    |g| g.grupos.clone(),
                ),
                java_global: guardado.as_ref().map(|g| g.java_global.clone()).unwrap_or_default(),
                java_detectadas: vec![],
                java_estado: "Detecta instalaciones para comprobar Java antes de jugar.".into(),
                java_generacion: 0,
                java_prueba_generacion: 0,
                configs: guardado.as_ref().map(|g| g.configs()).filter(|c| c.len() == total).unwrap_or_else(|| {
                    (0..total)
                        .map(|_| ConfigJava {
                            java: None,
                            heredar: true,
        heredar_memoria: true,
        java_requiere_seleccion: false,
                            descarga: true,
                            memoria_min: 2048,
                            memoria_max: 6144,
                            metaspace: 512,
                            argumentos: "-XX:+UseG1GC".into(),
                        })
                        .collect()
                }),
                editor_inst: 0,
                exp: if demo { Explorador::abrir("Supervivencia") } else { Explorador::desde_ruta(estado::directorio(false).join("instances")) },
                mundo: 0,
                filtro_consola: 0,
                busqueda_consola: String::new(),
                niveles_consola: [true; 3],
                autoscroll_consola: true,
                desplazamiento_consola: Default::default(),
                capt: if demo { capt::Estado::nuevo() } else { capt::Estado::vacio() },
                cursor: Point::ORIGIN,
                menu_en: Point::new(420.0, 140.0),
                ventana: Size::new(1280.0, 800.0),
                maximizada: false,
                umbral: std::env::var_os("ICARO_UMBRAL").is_some().then(std::time::Instant::now),
                // ICARO_PICAR=<etapa>[:<instancia>]: abre con la eliminación detenida en esa etapa.
                picar_objetivo: None,
                picar_revision: None,
                picar_solicitud: 0,
                picar: std::env::var("ICARO_PICAR").ok().and_then(|v| {
                    let mut partes = v.split(':');
                    let etapa = partes.next()?.parse::<u32>().ok()?;
                    let instancia = partes.next().and_then(|i| i.parse::<usize>().ok()).unwrap_or(0);
                    let ahora = std::time::Instant::now();
                    let lejos = ahora + std::time::Duration::from_secs(3600);
                    Some(Picar { instancia, inicio: ahora, soltada: Some((lejos, etapa)), rota: None })
                }),
                colocada: None,
                sesion: sesion::Sesion::desde_entorno(),
            },
            Task::batch([
                Task::perform(async {}, |_| Mensaje::Servicio(servicios::Accion::Inicializar)),
                Task::perform(async {}, |_| Mensaje::Servidores(servidores_reales::Accion::Abrir)),
            ]),
        )
}
