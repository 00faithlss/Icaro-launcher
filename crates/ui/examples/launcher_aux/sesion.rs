// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Sesión del jugador en la maqueta: cuenta, estudio de skin, menú de cuentas
//! y los estados que el launcher resuelve sin bloquear (invitado, cuenta
//! vencida, código vencido y cuenta sin Minecraft).

use std::path::{Path, PathBuf};
use std::time::Instant;

use iced::widget::{column, container, mouse_area, row, Space};
use iced::{Element, Length, Padding, Task};

use icaro_ui::cuenta::{
    self, DatosCuenta, DatosEstudio, EstadoSesion, MensajesCuenta, MensajesEstudio, MensajesSelector,
};
use icaro_ui::tema::espacio;

use crate::pantallas;
use crate::{App, Mensaje};

/// Duración simulada de la subida de una skin.
const MS_SUBIDA: u128 = 1600;

/// Skins guardadas de la cuenta; cada nombre genera su cabeza.
const HISTORIAL: [&str; 5] = ["busto", "barba", "skin-a", "skin-b", "skin-c"];

/// Estado de la sesión y del estudio de skin.
pub struct Sesion {
    pub cuenta: crate::cuenta_real::Estado,
    pub estado: EstadoSesion,
    /// Menú de cuentas abierto desde la barra lateral.
    pub menu: bool,
    /// Cuándo se abrió el menú, para la caída (Losa).
    pub menu_desde: Option<Instant>,
    /// Banner al intentar jugar en línea con la cuenta vencida.
    pub banner_jugar: bool,
    pub en_uso: usize,
    pub elegida: usize,
    pub modelo: usize,
    pub capas: [bool; 4],
    /// Giro de a 90° del visor de cuenta y estudio.
    pub giro: icaro_ui::visor_skin::Giro,
    /// Estudio: vista plana, encuadre de busto y caminata desde cuándo.
    pub plano: bool,
    pub busto: bool,
    pub caminar: Option<Instant>,
    /// Capa visible cuya zona se marca en la textura plana.
    pub zona: Option<usize>,
    /// Capa de la maqueta puesta (`None`: ninguna).
    pub capa_demo: Option<usize>,
    /// Cuándo se aplicó la última skin, para confirmarlo unos segundos.
    pub aplicada: Option<Instant>,
    pub subiendo: Option<Instant>,
    pub textura: Option<String>,
    pub error: Option<String>,
    pub soltando: bool,
}

impl Sesion {
    pub fn nueva(estado: EstadoSesion) -> Self {
        Self {
            cuenta: Default::default(),
            estado,
            menu: false,
            menu_desde: None,
            banner_jugar: false,
            en_uso: 0,
            elegida: 0,
            modelo: 0,
            capas: [true, true, false, true],
            giro: Default::default(),
            plano: false,
            busto: false,
            caminar: None,
            zona: None,
            capa_demo: Some(0),
            aplicada: None,
            subiendo: None,
            textura: None,
            error: None,
            soltando: false,
        }
    }

    /// Estado pedido con `ICARO_SESION`, para abrir la maqueta en una variante.
    pub fn desde_entorno() -> Self {
        let mut s = Self::nueva(match std::env::var("ICARO_SESION").ok().as_deref() {
            Some("invitado") => EstadoSesion::SinConexion,
            Some("vencida") if std::env::var("ICARO_DEMO").as_deref() == Ok("1") => EstadoSesion::Vencida,
            Some("sinjuego") if std::env::var("ICARO_DEMO").as_deref() == Ok("1") => EstadoSesion::SinJuego,
            _ if std::env::var("ICARO_DEMO").as_deref() == Ok("1") => EstadoSesion::Activa,
            _ => EstadoSesion::SinConexion,
        });
        s.menu = std::env::var_os("ICARO_MENU_CUENTA").is_some();
        s.banner_jugar = std::env::var_os("ICARO_BANNER_JUGAR").is_some();
        if let Some(n) = std::env::var("ICARO_GIRO").ok().and_then(|v| v.parse::<i32>().ok()) {
            s.giro.girar(n, Instant::now(), true);
        }
        s.modelo = usize::from(std::env::var("ICARO_MODELO").is_ok());
        if let Ok(c) = std::env::var("ICARO_CAPAS") {
            for (i, ch) in c.chars().take(4).enumerate() {
                s.capas[i] = ch == '1';
            }
        }
        s.subiendo = std::env::var_os("ICARO_SUBIENDO").is_some().then(Instant::now);
        s
    }
}

#[derive(Debug, Clone)]
pub enum Accion {
    /// Abre o cierra el menú de cuentas.
    Menu(bool),
    ElegirActiva,
    ElegirVencida,
    ElegirInvitado,
    AgregarCuenta,
    Gestionar,
    CerrarSesion,
    IniciarSesion,
    VolverAEntrar,
    Comprar,
    JugarComoInvitado,
    Jugar,
    AbrirEstudio,
    VolverCuenta,
    ElegirSkin(usize),
    UsarDeNuevo,
    Modelo(usize),
    Capa(usize, bool),
    Girar(i32),
    IrCara(i32),
    Descartar,
    Vista(usize),
    Encuadre(usize),
    Caminar(bool),
    Zona(Option<usize>),
    ElegirCapa(Option<usize>),
    ElegirBiblioteca(usize),
    GuardarBiblioteca,
    Descargar,
    Cargar,
    Soltando(bool),
    Soltar(PathBuf),
    Aplicar,
}

fn m(a: Accion) -> Mensaje {
    Mensaje::Ses(a)
}

/// Comprueba que el archivo sea una .png de 64 × 64 leyendo solo su cabecera.
fn validar_png(ruta: &Path) -> Result<String, String> {
    use std::io::Read;
    let nombre = ruta
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "skin.png".into());
    if !ruta.extension().is_some_and(|e| e.eq_ignore_ascii_case("png")) {
        return Err("Solo se aceptan archivos .png.".into());
    }
    let mut cab = [0u8; 24];
    std::fs::File::open(ruta)
        .and_then(|mut f| f.read_exact(&mut cab))
        .map_err(|_| "No se pudo leer el archivo.".to_owned())?;
    if cab[..8] != [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a] || &cab[12..16] != b"IHDR" {
        return Err("El archivo no es una imagen PNG válida.".into());
    }
    let ancho = u32::from_be_bytes([cab[16], cab[17], cab[18], cab[19]]);
    let alto = u32::from_be_bytes([cab[20], cab[21], cab[22], cab[23]]);
    if (ancho, alto) != (64, 64) {
        return Err(format!("La skin debe medir 64 × 64 píxeles y esta mide {ancho} × {alto}."));
    }
    Ok(nombre)
}

pub fn actualizar(app: &mut App, a: Accion) -> Task<Mensaje> {
    if !app.servicios.demo {
        match &a {
            Accion::AgregarCuenta | Accion::IniciarSesion | Accion::VolverAEntrar | Accion::ElegirVencida =>
                return crate::servicios::actualizar(app, crate::servicios::Accion::Login),
            Accion::ElegirActiva if app.servicios.session.is_none() =>
                return crate::servicios::actualizar(app, crate::servicios::Accion::Login),
            Accion::ElegirInvitado | Accion::JugarComoInvitado => return crate::cuenta_sin_conexion::actualizar(app,crate::cuenta_sin_conexion::Accion::Abrir),
            Accion::CerrarSesion => {
                app.pantalla = Some(pantallas::Pantalla::Cuenta);
                return crate::servicios::actualizar(app, crate::servicios::Accion::Logout);
            }
            Accion::Gestionar => return crate::cuenta_real::actualizar(app,crate::cuenta_real::Accion::Abrir(false)),
            Accion::AbrirEstudio => return crate::cuenta_real::actualizar(app,crate::cuenta_real::Accion::Abrir(true)),
            Accion::Aplicar | Accion::UsarDeNuevo => return crate::cuenta_real::actualizar(app,crate::cuenta_real::Accion::Aplicar(matches!(a,Accion::UsarDeNuevo))),
            Accion::Cargar => return crate::cuenta_real::actualizar(app,crate::cuenta_real::Accion::ElegirArchivo),
            Accion::ElegirCapa(c) => return crate::cuenta_real::actualizar(app,crate::cuenta_real::Accion::ElegirCapa(*c)),
            Accion::ElegirBiblioteca(i) => return crate::cuenta_real::actualizar(app,crate::cuenta_real::Accion::ElegirBiblioteca(*i)),
            Accion::GuardarBiblioteca => return crate::cuenta_real::actualizar(app,crate::cuenta_real::Accion::GuardarBiblioteca),
            Accion::Descargar => return crate::cuenta_real::actualizar(app,crate::cuenta_real::Accion::Descargar),
            Accion::Soltar(path) => return crate::cuenta_real::actualizar(app,crate::cuenta_real::Accion::Archivo(Some(path.clone()))),
            Accion::ElegirSkin(i) => return crate::cuenta_real::actualizar(app,crate::cuenta_real::Accion::Elegir(*i)),
            _ => {}
        }
        if app.sesion.subiendo.is_some() && matches!(a,Accion::Modelo(_)|Accion::Capa(_,_)|Accion::Girar(_)|Accion::VolverCuenta) { return Task::none(); }
    }
    let pant = |app: &mut App, p| {
        app.pantalla = Some(p);
        app.sesion.menu = false;
        crate::arrancar(app);
    };
    match a {
        Accion::Menu(v) => {
            app.sesion.menu = v;
            if v && !app.reducir {
                app.sesion.menu_desde = Some(Instant::now());
                app.ahora = Instant::now();
            }
        }
        Accion::ElegirActiva => {
            app.sesion.estado = EstadoSesion::Activa;
            app.sesion.menu = false;
            app.sesion.banner_jugar = false;
        }
        Accion::ElegirVencida => {
            // Una cuenta vencida abre el código directamente.
            app.sesion.estado = EstadoSesion::Vencida;
            app.codigo_inicio = Instant::now();
            app.codigo_fase = pantallas::FaseCodigo::Esperando;
            pant(app, pantallas::Pantalla::Codigo);
        }
        Accion::ElegirInvitado | Accion::JugarComoInvitado => {
            app.sesion.estado = EstadoSesion::SinConexion;
            app.sesion.menu = false;
            app.sesion.banner_jugar = false;
            app.pantalla = None;
            app.avisar("Sigues como invitado. Puedes iniciar sesión luego desde la cuenta.".into());
            crate::arrancar(app);
        }
        Accion::AgregarCuenta | Accion::IniciarSesion | Accion::VolverAEntrar => {
            app.codigo_inicio = Instant::now();
            app.codigo_fase = pantallas::FaseCodigo::Esperando;
            pant(app, pantallas::Pantalla::Codigo);
        }
        Accion::Gestionar => pant(app, pantallas::Pantalla::Cuenta),
        Accion::CerrarSesion => {
            let nombre = app.sesion.estado.nombre();
            app.sesion.estado = EstadoSesion::SinConexion;
            app.sesion.menu = false;
            app.pantalla = Some(pantallas::Pantalla::Cuenta);
            app.avisar(format!("Cerraste la sesión de {nombre}. Sigues como invitado."));
        }
        Accion::Comprar => {
            let _ = std::process::Command::new("cmd")
                .args(["/C", "start", "", "https://www.minecraft.net/store/minecraft-java-bedrock-edition-pc"])
                .spawn();
        }
        Accion::Jugar => {
            if matches!(app.sesion.estado, EstadoSesion::Vencida | EstadoSesion::CodigoVencido) {
                app.sesion.banner_jugar = true;
            }
        }
        Accion::AbrirEstudio => {
            app.sesion.giro = Default::default();
            pant(app, pantallas::Pantalla::Estudio);
        }
        Accion::VolverCuenta => pant(app, pantallas::Pantalla::Cuenta),
        Accion::ElegirSkin(i) => app.sesion.elegida = i,
        Accion::UsarDeNuevo => {
            app.sesion.en_uso = app.sesion.elegida;
            app.avisar("Volviste a usar esa skin en tu cuenta.".into());
        }
        Accion::Modelo(i) => app.sesion.modelo = i,
        Accion::Capa(i, v) => {
            if let Some(c) = app.sesion.capas.get_mut(i) {
                *c = v;
            }
        }
        Accion::Girar(n) => {
            app.ahora = Instant::now();
            app.sesion.giro.girar(n, app.ahora, app.reducir);
        }
        Accion::IrCara(g) => {
            app.ahora = Instant::now();
            app.sesion.giro.ir(g, app.ahora, app.reducir);
        }
        Accion::Descartar => app.sesion.elegida = app.sesion.en_uso,
        Accion::Vista(i) => app.sesion.plano = i == 1,
        Accion::Encuadre(i) => app.sesion.busto = i == 1,
        Accion::Caminar(v) => {
            app.ahora = Instant::now();
            app.sesion.caminar = v.then_some(app.ahora);
        }
        Accion::Zona(z) => app.sesion.zona = z,
        Accion::ElegirCapa(c) => {
            app.sesion.capa_demo = c;
            // Al elegir una capa, el visor gira a la espalda para mostrarla.
            if c.is_some() {
                app.ahora = Instant::now();
                app.sesion.giro.ir(180, app.ahora, app.reducir);
            }
        }
        Accion::ElegirBiblioteca(_) | Accion::GuardarBiblioteca | Accion::Descargar => {
            app.avisar("En la maqueta no se guardan ni descargan skins.".into());
        }
        Accion::Cargar => {
            // La maqueta no abre un selector de archivos: simula una skin válida.
            app.sesion.textura = Some("mi-skin.png".into());
            app.sesion.error = None;
        }
        Accion::Soltando(v) => app.sesion.soltando = v,
        Accion::Soltar(ruta) => {
            app.sesion.soltando = false;
            match validar_png(&ruta) {
                Ok(nombre) => {
                    app.sesion.textura = Some(nombre);
                    app.sesion.error = None;
                }
                Err(e) => app.sesion.error = Some(e),
            }
        }
        Accion::Aplicar => {
            if app.sesion.subiendo.is_none() {
                app.sesion.subiendo = Some(Instant::now());
                app.ahora = Instant::now();
            }
        }
    }
    Task::none()
}

/// Termina la subida simulada cuando pasa su tiempo.
pub fn avanzar(app: &mut App) {
    if app.sesion.aplicada.is_some_and(|i| app.ahora.saturating_duration_since(i).as_millis() >= MS_APLICADA) {
        app.sesion.aplicada = None;
    }
    if !app.servicios.demo {return;}
    if let Some(inicio) = app.sesion.subiendo {
        if app.ahora.saturating_duration_since(inicio).as_millis() >= MS_SUBIDA {
            app.sesion.subiendo = None;
            app.sesion.textura = None;
            app.sesion.aplicada = Some(app.ahora);
            app.avisar(format!("Skin aplicada a {}.", app.sesion.estado.nombre()));
        }
    }
}

/// Hay algo animándose que pide cuadros.
pub fn animando(app: &App) -> bool {
    let visor = matches!(app.pantalla, Some(pantallas::Pantalla::Cuenta | pantallas::Pantalla::Estudio));
    app.sesion.subiendo.is_some()
        || (app.sesion.menu && progreso_menu(app) < 1.0)
        || (visor && app.sesion.giro.animando(app.ahora))
        || (app.pantalla == Some(pantallas::Pantalla::Estudio) && app.sesion.caminar.is_some() && !app.reducir)
        || app.sesion.aplicada.is_some()
}

/// «Skin aplicada» se muestra durante este tiempo.
pub const MS_APLICADA: u128 = 3000;

fn progreso_menu(app: &App) -> f32 {
    app.sesion.menu_desde.map_or(1.0, |i| {
        (app.ahora.saturating_duration_since(i).as_millis() as f32 / icaro_ui::tema::duracion::LOSA as f32).clamp(0.0, 1.0)
    })
}

/// Skins y capas de ejemplo de la maqueta, generadas una sola vez.
fn muestras() -> &'static (Vec<Vec<u8>>, Vec<Vec<u8>>) {
    static MUESTRAS: std::sync::OnceLock<(Vec<Vec<u8>>, Vec<Vec<u8>>)> = std::sync::OnceLock::new();
    MUESTRAS.get_or_init(|| {
        let skins = (0..4).map(icaro_ui::visor_skin::muestra).collect();
        // Capas de 64 × 32 con tres dibujos: líneas, damero y borde.
        let capas = (0..3u8)
            .map(|k| {
                let mut rgba = vec![0u8; 64 * 32 * 4];
                for y in 0..32 {
                    for x in 0..64 {
                        let claro = match k {
                            0 => y % 4 < 2,
                            1 => (x / 2 + y / 2) % 2 == 0,
                            _ => x % 11 <= 1 || y % 17 <= 1,
                        };
                        let c: [u8; 3] = if claro { [0xec, 0xec, 0xe8] } else { [0x3a, 0x3a, 0x37] };
                        let i = (y * 64 + x) * 4;
                        rgba[i..i + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
                    }
                }
                rgba
            })
            .collect();
        (skins, capas)
    })
}

const CAPAS_DEMO: [&str; 3] = ["Migración", "Aniversario", "Fundador"];

pub fn vista_cuenta(app: &App) -> Element<'_, Mensaje> {
    if !app.servicios.demo { return crate::servicios::account_view(app); }
    let p = app.paleta();
    let s = &app.sesion;
    let (skins, capas) = muestras();
    let textura = skins.get(s.elegida.min(skins.len() - 1)).map(Vec::as_slice);
    cuenta::pantalla_cuenta(
        p,
        DatosCuenta {
            estado: s.estado,
            nombre: s.estado.nombre(),
            proximo_cambio: "22-10-2026",
            historial: HISTORIAL[..skins.len()].to_vec(),
            en_uso: s.en_uso,
            elegida: s.elegida,
            textura,
            historial_real: skins.iter().map(|t| std::sync::Arc::from(t.as_slice())).collect(),
            delgado: s.modelo == 1,
            angulo: s.giro.angulo(app.ahora),
            grados: s.giro.grados(),
            capas: CAPAS_DEMO.iter().zip(capas).map(|(n, c)| cuenta::CapaCuenta { nombre: n, rgba: Some(c) }).collect(),
            capa_puesta: s.capa_demo,
            capa_cambiando: None,
            tiempo_total: "41 h".into(),
            mas_jugada: Some(("Survival con amigos".into(), "28 h".into())),
        },
        MensajesCuenta {
            abrir_estudio: m(Accion::AbrirEstudio),
            elegir_skin: Box::new(|i| m(Accion::ElegirSkin(i))),
            usar_de_nuevo: m(Accion::UsarDeNuevo),
            descartar: m(Accion::Descartar),
            girar: |n| m(Accion::Girar(n)),
            elegir_capa: Box::new(|c| m(Accion::ElegirCapa(c))),
            descargar: Some(m(Accion::Descargar)),
            cerrar_sesion: m(Accion::CerrarSesion),
            iniciar_sesion: m(Accion::IniciarSesion),
            volver_a_entrar: m(Accion::VolverAEntrar),
            comprar: m(Accion::Comprar),
            jugar_invitado: m(Accion::JugarComoInvitado),
            cambiar_nombre: None,
        },
    )
}

pub fn vista_estudio(app: &App) -> Element<'_, Mensaje> {
    let p = app.paleta();
    let s = &app.sesion;
    let subiendo = s
        .subiendo
        .map(|i| app.ahora.saturating_duration_since(i).as_millis() as u64);
    let caminar = s.caminar.filter(|_| !app.reducir).map(|i| app.ahora.saturating_duration_since(i).as_secs_f32());
    let datos = if app.servicios.demo {
        let (skins, capas) = muestras();
        DatosEstudio {
            modelo: s.modelo,
            capas: s.capas,
            angulo: s.giro.angulo(app.ahora),
            grados: s.giro.grados(),
            plano: s.plano,
            busto: s.busto,
            caminar,
            subiendo,
            textura: s.textura.as_deref(),
            error: s.error.as_deref(),
            soltando: s.soltando,
            rgba: skins.get(s.elegida.min(skins.len() - 1)).map(Vec::as_slice),
            puede_aplicar: s.textura.is_some(),
            aplicada: s.aplicada.is_some(),
            capas_cuenta: CAPAS_DEMO.iter().zip(capas).map(|(n, c)| cuenta::CapaCuenta { nombre: n, rgba: Some(c) }).collect(),
            capa_puesta: s.capa_demo,
            capa_cambiando: None,
            biblioteca: skins.iter().map(|t| std::sync::Arc::from(t.as_slice())).collect(),
            biblioteca_elegida: Some(s.elegida),
            zona: s.zona,
        }
    } else {
        let real = crate::cuenta_real::datos_estudio(app);
        DatosEstudio {
            modelo: s.modelo,
            capas: s.capas,
            angulo: s.giro.angulo(app.ahora),
            grados: s.giro.grados(),
            plano: s.plano,
            busto: s.busto,
            caminar,
            subiendo,
            textura: s.textura.as_deref(),
            error: s.error.as_deref(),
            soltando: s.soltando,
            rgba: s.cuenta.seleccion.as_ref().or(s.cuenta.actual.as_ref()).map(|t| t.rgba.as_ref()),
            puede_aplicar: s.cuenta.seleccion.is_some() || s.cuenta.actual.is_some(),
            aplicada: s.aplicada.is_some(),
            capas_cuenta: real.capas,
            capa_puesta: real.capa_puesta,
            capa_cambiando: real.capa_cambiando,
            biblioteca: real.biblioteca,
            biblioteca_elegida: real.biblioteca_elegida,
            zona: s.zona,
        }
    };
    cuenta::estudio_skin(
        p,
        datos,
        MensajesEstudio {
            volver: m(Accion::VolverCuenta),
            modelo: |i| m(Accion::Modelo(i)),
            capa: |i, v| m(Accion::Capa(i, v)),
            girar: |n| m(Accion::Girar(n)),
            ir: |g| m(Accion::IrCara(g)),
            vista: |i| m(Accion::Vista(i)),
            encuadre: |i| m(Accion::Encuadre(i)),
            caminar: |v| m(Accion::Caminar(v)),
            zona: |z| m(Accion::Zona(z)),
            elegir_capa: |c| m(Accion::ElegirCapa(c)),
            elegir_biblioteca: |i| m(Accion::ElegirBiblioteca(i)),
            guardar_biblioteca: Some(m(Accion::GuardarBiblioteca)),
            cargar: m(Accion::Cargar),
            descargar: Some(m(Accion::Descargar)),
            aplicar: m(Accion::Aplicar),
        },
    )
}

/// Menú de cuentas anclado al bloque de la barra lateral, con un fondo
/// transparente que lo cierra al pulsar fuera.
pub fn con_selector<'a>(app: &'a App, ventana: Element<'a, Mensaje>) -> Element<'a, Mensaje> {
    if !app.servicios.demo || app.servicios.local.abierta { return crate::servicios::selector(app, ventana); }
    if !app.sesion.menu {
        return ventana;
    }
    let p = app.paleta();
    let menu = cuenta::selector_cuenta(
        p,
        app.sesion.estado,
        MensajesSelector {
            activa: m(Accion::ElegirActiva),
            vencida: m(Accion::ElegirVencida),
            invitado: m(Accion::ElegirInvitado),
            agregar: m(Accion::AgregarCuenta),
            gestionar: m(Accion::Gestionar),
            cerrar_sesion: m(Accion::CerrarSesion),
        },
    );
    let fuera = mouse_area(Space::new(Length::Fill, Length::Fill)).on_press(m(Accion::Menu(false)));
    iced::widget::stack![
        ventana,
        fuera,
        container(row![menu])
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(iced::alignment::Vertical::Bottom)
            .padding(Padding { top: 0.0, right: 0.0, bottom: 72.0 + icaro_ui::movimiento::losa(progreso_menu(app)), left: espacio::S2 }),
    ]
    .into()
}

/// Banner sobre la barra de juego cuando la cuenta venció y se intenta jugar.
pub fn banner_jugar(app: &App) -> Option<Element<'_, Mensaje>> {
    if !app.sesion.banner_jugar || !matches!(app.sesion.estado, EstadoSesion::Vencida | EstadoSesion::CodigoVencido) {
        return None;
    }
    let p = app.paleta();
    Some(
        column![cuenta::banner_vencida(
            p,
            m(Accion::VolverAEntrar),
            "No se puede jugar en línea",
            "Tu cuenta venció. Vuelve a entrar para jugar en línea, o juega sin conexión.",
        )]
        .padding(Padding::from([espacio::S3, espacio::S12]))
        .into(),
    )
}
