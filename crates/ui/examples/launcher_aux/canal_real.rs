// SPDX-License-Identifier: GPL-3.0-only
//! CanalActualizaciones (Ajustes › Acerca de): cambio entre Estable y Beta sin
//! quedar atrapado en la Beta. Los pasos son páginas dentro de Ajustes, no
//! diálogos. El disco y la red van en tareas; aquí solo se decide el estado.
use crate::{App, Mensaje};
use icaro_core::actualizacion::{self, Canal, Disponible, Preparada};
use icaro_core::copias::{self, Copia};
use icaro_ui::ajustes::fila_ajuste;
use icaro_ui::componentes::{self, Estado as Tono, Variante};
use icaro_ui::fuentes;
use icaro_ui::iconos::{icono, Icono, Tam};
use icaro_ui::tema::{borde, espacio, texto, Paleta};
use iced::widget::{button, column, container, row, text, Space};
use iced::{Alignment, Background, Border, Element, Length, Padding, Task};

const ACTUAL: &str = env!("CARGO_PKG_VERSION");
const ANCHO: f32 = 720.;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paso {
    Beta,
    Volver,
    Elegir,
    Aplicando,
    SinCopia,
}

#[derive(Debug, Clone, Default)]
enum Consulta {
    #[default]
    Pendiente,
    Lista(Option<Disponible>),
    Error(String),
}

#[derive(Default)]
pub struct Estado {
    pub paso: Option<Paso>,
    copias: Vec<Copia>,
    /// Tamaño de lo que entraría en una copia hecha ahora.
    bytes_datos: u64,
    estable: Consulta,
    /// Versión beta que se instalaría al pasar a Beta, si ya se consultó.
    beta: Option<String>,
    /// «Volver ahora» en vez de «Esperar».
    ahora: bool,
    elegida: Option<String>,
    confirmar_cero: bool,
    /// Volver empezando de cero, sin restaurar una copia.
    cero: bool,
    etapa: u8,
    avance: f32,
    bytes: u64,
    ocupado: bool,
    error: Option<String>,
    copia_beta: Option<String>,
    generacion: u64,
    /// Los datos ya son los de la versión estable: no volver a guardar el
    /// estado de esta sesión encima.
    pub sin_guardar: bool,
}

#[derive(Debug, Clone)]
pub enum Accion {
    Segmento(usize),
    Copias(bool),
    Cerrar,
    Datos(u64, u64, Vec<Copia>),
    Estable(u64, Result<Option<Disponible>, String>),
    BetaOfrecida(u64, Option<String>),
    PasarABeta,
    Pasada(u64, Result<(), String>),
    Como(bool),
    Continuar,
    Esperar,
    Elegir(String),
    Atras,
    Restaurar,
    EmpezarDeCero,
    CopiaBeta(u64, Result<String, String>),
    Progreso(u64, u64),
    Descargada(u64, Result<Preparada, String>),
    Lanzada(u64, Result<(), String>),
}

fn m(a: Accion) -> Mensaje {
    Mensaje::Canal(a)
}

fn numeros(v: &str) -> Option<[u64; 3]> {
    let mut p = v.trim_start_matches('v').split(['.', '-', '+']);
    Some([p.next()?.parse().ok()?, p.next()?.parse().ok()?, p.next()?.parse().ok()?])
}

/// «0.4» para «0.4.0»: la estable que se espera.
fn siguiente() -> String {
    numeros(ACTUAL).map_or_else(|| ACTUAL.into(), |n| format!("{}.{}", n[0], n[1]))
}

fn es_beta() -> bool {
    crate::actualizacion_real::canal_compilado() == Canal::Beta
}

fn raiz(app: &App) -> std::path::PathBuf {
    crate::estado::directorio(app.servicios.demo)
}

fn fecha(segundos: u64) -> String {
    let f = crate::hora_local::local(std::time::UNIX_EPOCH + std::time::Duration::from_secs(segundos));
    format!("{:02}-{:02}-{}, {:02}:{:02}", f.dia, f.mes, f.anio, f.hora, f.minuto)
}

fn version_estable(e: &Estado) -> Option<&Disponible> {
    match &e.estable {
        Consulta::Lista(Some(d)) => Some(d),
        _ => None,
    }
}

fn compatible(c: &Copia, d: &Disponible) -> bool {
    c.compatible(&d.version, d.esquema)
}

/// Copia de los datos antes de una actualización normal. Para llamar desde
/// una tarea bloqueante; un fallo no impide actualizar.
pub fn copia_previa(raiz: &std::path::Path) {
    if copias::crear(raiz, ACTUAL, es_beta(), copias::ESQUEMA).is_ok() {
        let _ = copias::podar(raiz, copias::CONSERVAR);
    }
}

fn bloqueante<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
    listo: impl Fn(Result<T, String>) -> Mensaje + Send + 'static,
) -> Task<Mensaje> {
    Task::perform(
        async move {
            tokio::task::spawn_blocking(f)
                .await
                .map_err(|_| "No se pudo completar la operación.".to_owned())?
        },
        listo,
    )
}

/// Lee del disco las copias y el tamaño de los datos actuales.
fn leer_datos(app: &App, g: u64) -> Task<Mensaje> {
    let raiz = raiz(app);
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                let bytes = copias::ARCHIVOS
                    .iter()
                    .filter_map(|a| std::fs::metadata(raiz.join(a)).ok())
                    .map(|m| m.len())
                    .sum();
                (bytes, copias::listar(&raiz))
            })
            .await
            .unwrap_or_default()
        },
        move |(bytes, lista)| m(Accion::Datos(g, bytes, lista)),
    )
}

fn abrir(app: &mut App, paso: Paso) -> u64 {
    let g = app.canal.generacion + 1;
    app.canal = Estado { paso: Some(paso), generacion: g, ..Default::default() };
    g
}

fn cambiar_canal(app: &mut App, canal: Canal) -> Task<Mensaje> {
    if app.aj.canal == canal {
        return Task::none();
    }
    app.aj.canal = canal;
    app.actualizacion.disponible = None;
    if app.servicios.demo {
        return Task::none();
    }
    crate::actualizacion_real::actualizar(app, crate::actualizacion_real::Accion::Comprobar)
}

fn fallo(app: &mut App, e: String) {
    let c = &mut app.canal;
    c.ocupado = false;
    c.sin_guardar = false;
    c.error = Some(e);
    if c.paso == Some(Paso::Aplicando) {
        c.paso = Some(if c.cero { Paso::SinCopia } else { Paso::Elegir });
    }
}

pub fn actualizar(app: &mut App, accion: Accion) -> Task<Mensaje> {
    let demo = app.servicios.demo;
    match accion {
        Accion::Copias(v) => app.aj.copias_antes = v,
        Accion::Segmento(i) => {
            let canal = if i == 1 { Canal::Beta } else { Canal::Estable };
            if canal == app.aj.canal || app.canal.ocupado {
                return Task::none();
            }
            // Desde una compilación estable, Beta se explica antes; desde una
            // beta, volver a Estable también. Lo demás es solo la preferencia.
            if canal == Canal::Beta && !es_beta() {
                let g = abrir(app, Paso::Beta);
                if demo {
                    preparar_diseno(app, "canal-beta");
                    return Task::none();
                }
                let consulta = Task::perform(
                    async move {
                        actualizacion::comprobar_canal(ACTUAL, env!("ICARO_COMPILACION").parse().unwrap_or(0), Canal::Beta)
                            .await
                            .ok()
                            .flatten()
                            .map(|d| d.version)
                    },
                    move |v| m(Accion::BetaOfrecida(g, v)),
                );
                return Task::batch([leer_datos(app, g), consulta]);
            }
            if canal == Canal::Estable && es_beta() {
                let g = abrir(app, Paso::Volver);
                if demo {
                    preparar_diseno(app, "canal-volver");
                    return Task::none();
                }
                let consulta = Task::perform(
                    async move { actualizacion::estable_publicada().await.map_err(|e| e.to_string()) },
                    move |r| m(Accion::Estable(g, r)),
                );
                return Task::batch([leer_datos(app, g), consulta]);
            }
            return cambiar_canal(app, canal);
        }
        Accion::Cerrar => {
            if !app.canal.ocupado {
                app.canal.paso = None;
            }
        }
        Accion::Datos(g, bytes, lista) if g == app.canal.generacion => {
            app.canal.bytes_datos = bytes;
            app.canal.copias = lista;
        }
        Accion::BetaOfrecida(g, v) if g == app.canal.generacion => app.canal.beta = v,
        Accion::Estable(g, r) if g == app.canal.generacion => {
            app.canal.estable = match r {
                Ok(d) => Consulta::Lista(d),
                Err(e) => Consulta::Error(e),
            };
            // Si la estable ya alcanzó a esta beta no hay nada que perder.
            let alcanzada = version_estable(&app.canal)
                .is_some_and(|d| numeros(&d.version).zip(numeros(ACTUAL)).is_some_and(|(e, a)| e >= a));
            if alcanzada {
                app.canal.paso = None;
                app.avisar("Canal Estable: la versión estable ya incluye esta beta.".to_owned());
                return cambiar_canal(app, Canal::Estable);
            }
        }
        Accion::PasarABeta => {
            if app.canal.ocupado {
                return Task::none();
            }
            if demo || !app.aj.copias_antes {
                app.canal.paso = None;
                return cambiar_canal(app, Canal::Beta);
            }
            app.canal.ocupado = true;
            app.canal.error = None;
            let (raiz, g) = (raiz(app), app.canal.generacion);
            return bloqueante(
                move || {
                    copias::crear(&raiz, ACTUAL, es_beta(), copias::ESQUEMA)?;
                    copias::podar(&raiz, copias::CONSERVAR)
                },
                move |r| m(Accion::Pasada(g, r)),
            );
        }
        Accion::Pasada(g, r) if g == app.canal.generacion => {
            app.canal.ocupado = false;
            match r {
                Ok(()) => {
                    app.canal.paso = None;
                    app.avisar("Copia guardada. Canal Beta activado.".to_owned());
                    return cambiar_canal(app, Canal::Beta);
                }
                // Sin copia no se pasa a Beta: sería quedar sin vuelta.
                Err(e) => app.canal.error = Some(e),
            }
        }
        Accion::Como(ahora) => app.canal.ahora = ahora,
        Accion::Continuar => {
            if !app.canal.ahora {
                return actualizar(app, Accion::Esperar);
            }
            let Some(d) = version_estable(&app.canal).cloned() else {
                return Task::none();
            };
            let primera = app.canal.copias.iter().find(|c| compatible(c, &d)).map(|c| c.id.clone());
            app.canal.error = None;
            app.canal.paso = Some(if primera.is_some() { Paso::Elegir } else { Paso::SinCopia });
            app.canal.elegida = primera;
        }
        Accion::Esperar => {
            if app.canal.ocupado {
                return Task::none();
            }
            app.canal.paso = None;
            app.avisar(format!("Sin más betas: Ícaro pasará a Estable cuando salga la {}.", siguiente()));
            return cambiar_canal(app, Canal::Estable);
        }
        Accion::Elegir(id) => app.canal.elegida = Some(id),
        Accion::Atras => {
            if !app.canal.ocupado {
                app.canal.paso = Some(Paso::Volver);
                app.canal.error = None;
                app.canal.confirmar_cero = false;
            }
        }
        Accion::Restaurar | Accion::EmpezarDeCero => {
            let cero = matches!(accion, Accion::EmpezarDeCero);
            if app.canal.ocupado || version_estable(&app.canal).is_none() {
                return Task::none();
            }
            if cero && !app.canal.confirmar_cero {
                app.canal.confirmar_cero = true;
                return Task::none();
            }
            if !cero && app.canal.elegida.is_none() {
                return Task::none();
            }
            if crate::actualizacion_real::juego_abierto(app) {
                app.canal.error = Some("Cierra Minecraft y espera a que terminen sus tareas antes de volver a Estable.".into());
                return Task::none();
            }
            app.canal.cero = cero;
            app.canal.paso = Some(Paso::Aplicando);
            app.canal.etapa = 0;
            app.canal.avance = 0.;
            app.canal.error = None;
            if demo {
                app.canal.etapa = 2;
                app.canal.avance = 1.;
                return Task::none();
            }
            app.canal.ocupado = true;
            let (raiz, g) = (raiz(app), app.canal.generacion);
            return bloqueante(
                move || copias::crear(&raiz, ACTUAL, es_beta(), copias::ESQUEMA).map(|c| c.id),
                move |r| m(Accion::CopiaBeta(g, r)),
            );
        }
        Accion::CopiaBeta(g, r) if g == app.canal.generacion => match r {
            Err(e) => fallo(app, e),
            Ok(id) => {
                app.canal.copia_beta = Some(id);
                app.canal.etapa = 1;
                let Some(d) = version_estable(&app.canal).cloned() else {
                    fallo(app, "No se conoce la versión estable.".into());
                    return Task::none();
                };
                app.canal.bytes = d.bytes;
                return Task::run(
                    iced::stream::channel(32, move |mut salida| async move {
                        use iced::futures::{SinkExt, StreamExt};
                        let (tx, mut rx) = iced::futures::channel::mpsc::unbounded();
                        let descargar = actualizacion::descargar(d, move |n, total| {
                            let _ = tx.unbounded_send(m(Accion::Progreso(n, total)));
                        });
                        tokio::pin!(descargar);
                        let resultado = loop {
                            tokio::select! {
                                r = &mut descargar => break r.map_err(|e| e.to_string()),
                                Some(a) = rx.next() => { let _ = salida.send(a).await; }
                            }
                        };
                        let _ = salida.send(m(Accion::Descargada(g, resultado))).await;
                    }),
                    |a| a,
                );
            }
        },
        Accion::Progreso(n, total) => {
            app.canal.avance = if total == 0 { 0. } else { (n as f32 / total as f32).clamp(0., 1.) };
        }
        Accion::Descargada(g, r) if g == app.canal.generacion => match r {
            Err(e) => fallo(app, e),
            Ok(preparada) => {
                let Some(beta) = app.canal.copia_beta.clone() else {
                    fallo(app, "Falta la copia de la beta.".into());
                    return Task::none();
                };
                app.canal.etapa = 2;
                app.canal.avance = 1.;
                // Desde aquí el disco tiene los datos de la versión estable.
                app.canal.sin_guardar = true;
                let (raiz, cero, elegida) = (raiz(app), app.canal.cero, app.canal.elegida.clone());
                return bloqueante(
                    move || {
                        let r = if cero {
                            copias::empezar_de_cero(&raiz, ACTUAL, es_beta()).map(|_| ())
                        } else {
                            copias::restaurar(&raiz, elegida.as_deref().unwrap_or_default())
                        }
                        .and_then(|()| actualizacion::aplicar(&preparada, true).map_err(|e| e.to_string()));
                        // Si algo falla, los datos de la beta vuelven a su sitio.
                        if r.is_err() {
                            let _ = copias::restaurar(&raiz, &beta);
                        }
                        r
                    },
                    move |r| m(Accion::Lanzada(g, r)),
                );
            }
        },
        Accion::Lanzada(g, r) if g == app.canal.generacion => match r {
            Ok(()) => {
                app.canal.etapa = 3;
                return crate::juego_real::cerrar(app);
            }
            Err(e) => fallo(app, e),
        },
        // Respuestas de una página que ya se cerró.
        _ => {}
    }
    Task::none()
}

// ------------------------------------------------------------------ vista

/// Las dos filas de Ajustes › Acerca de.
pub fn filas(app: &App) -> Element<'_, Mensaje> {
    let p = app.paleta();
    column![
        fila_ajuste(
            p,
            "Canal de actualizaciones",
            "Beta recibe funciones antes, pero puede fallar. Cambiar de canal no borra tus instancias.",
            componentes::segmentado_compacto(p, &["Estable", "Beta"], usize::from(app.aj.canal == Canal::Beta), |i| m(Accion::Segmento(i))),
        ),
        fila_ajuste(
            p,
            "Copias antes de actualizar",
            "Ícaro guarda sus ajustes y datos (no los mundos) antes de cada cambio de versión. Se conservan las últimas 3.",
            componentes::interruptor(p, app.aj.copias_antes, m(Accion::Copias(!app.aj.copias_antes))),
        ),
    ]
    .spacing(espacio::S2)
    .into()
}

fn caja(fondo: Option<iced::Color>, color: iced::Color, ancho: f32) -> impl Fn(&iced::Theme) -> container::Style {
    move |_| container::Style {
        background: fondo.map(Background::Color),
        border: Border { color, width: ancho, radius: 0.0.into() },
        ..Default::default()
    }
}

fn t<'a>(s: impl Into<String>, tam: f32, color: iced::Color) -> iced::widget::Text<'a> {
    text(s.into()).size(tam).color(color)
}

fn titulo<'a>(s: impl Into<String>, tam: f32, color: iced::Color) -> iced::widget::Text<'a> {
    t(s, tam, color).font(fuentes::TITULO)
}

/// Icono de 24 px centrado en una celda fija de 40 px.
fn celda_icono<'a>(p: Paleta, glifo: Icono) -> Element<'a, Mensaje> {
    container(icono(glifo, Tam::Base, p.text))
        .center(40)
        .style(caja(Some(p.surface_raised), p.border, borde::FINO))
        .into()
}

/// Punto con icono, frase en negrita y explicación.
fn punto<'a>(p: Paleta, glifo: Icono, fuerte: &str, resto: &str) -> Element<'a, Mensaje> {
    row![
        container(icono(glifo, Tam::Base, p.text)).center(24),
        column![titulo(fuerte, texto::BODY.0, p.text), t(resto, texto::BODY_SM.0, p.text_muted)].spacing(2).width(Length::Fill),
    ]
    .spacing(espacio::S3)
    .align_y(Alignment::Start)
    .into()
}

/// Fila con icono en celda, título y explicación.
fn fila_icono<'a>(p: Paleta, glifo: Icono, nombre: String, detalle: String) -> Element<'a, Mensaje> {
    row![
        celda_icono(p, glifo),
        column![titulo(nombre, texto::BODY.0 + 1., p.text), t(detalle, texto::BODY_SM.0, p.text_muted)].spacing(3).width(Length::Fill),
    ]
    .spacing(espacio::S3)
    .align_y(Alignment::Center)
    .into()
}

/// Opción de radio: rombo, contenido y contorno marcado cuando está elegida.
fn opcion<'a>(p: Paleta, marcada: bool, mensaje: Option<Mensaje>, contenido: Element<'a, Mensaje>) -> Element<'a, Mensaje> {
    let apagada = mensaje.is_none() && !marcada;
    let ancho = if marcada { borde::MEDIO } else { borde::FINO };
    let cuerpo = container(
        row![componentes::rombo(p, marcada, apagada), contenido].spacing(espacio::S3).align_y(Alignment::Center),
    )
    .padding(espacio::S4 - ancho)
    .width(Length::Fill)
    .style(caja(marcada.then_some(p.accent_soft), if marcada { p.text } else { p.border }, ancho));
    let b = button(cuerpo).padding(0).width(Length::Fill).style(|_, _| button::Style::default());
    match mensaje {
        Some(msg) => b.on_press(msg).into(),
        None => b.into(),
    }
}

fn icono_funcion(titulo: &str) -> Icono {
    let s = titulo.to_lowercase();
    if s.contains("compart") || s.contains("invit") {
        Icono::Usuarios
    } else if s.contains("saludo") || s.contains("frase") || s.contains("novedad") {
        Icono::Notas
    } else if s.contains("instanc") || s.contains("revisi") {
        Icono::Instancias
    } else {
        Icono::Brillo
    }
}

/// Página con migas, insignia, título, cuerpo y pie de acciones.
fn marco<'a>(
    app: &App,
    miga: &str,
    insignia: Option<(Tono, &str)>,
    titular: String,
    bajada: String,
    cuerpo: Element<'a, Mensaje>,
    izquierda: Option<Element<'a, Mensaje>>,
    derecha: Vec<Element<'a, Mensaje>>,
) -> Element<'a, Mensaje> {
    let p = app.paleta();
    let e = &app.canal;
    let mut col = column![icaro_ui::migas::migas(
        p,
        &[("Acerca de".to_string(), (!e.ocupado).then(|| m(Accion::Cerrar))), (miga.to_string(), None)],
    )]
    .spacing(espacio::S4)
    .max_width(ANCHO);
    let mut cab = column![].spacing(6);
    if let Some((tono, palabra)) = insignia {
        cab = cab.push(componentes::insignia(p, tono, palabra));
    }
    cab = cab.push(titulo(titular, texto::TITLE.0, p.text)).push(t(bajada, texto::BODY_SM.0, p.text_muted));
    col = col.push(cab).push(cuerpo);
    if let Some(error) = &e.error {
        col = col.push(componentes::aviso(p, Tono::Error, error));
    }
    if izquierda.is_some() || !derecha.is_empty() {
        let mut pie = row![].spacing(espacio::S2).align_y(Alignment::Center);
        if let Some(i) = izquierda {
            pie = pie.push(i);
        }
        pie = pie.push(Space::with_width(Length::Fill));
        for d in derecha {
            pie = pie.push(d);
        }
        col = col
            .push(container(Space::new(Length::Fill, borde::FINO)).style(icaro_ui::estilo::bloque(p.border)))
            .push(pie);
    }
    container(col).width(Length::Fill).padding(Padding::from([0., 0.])).into()
}

fn boton<'a>(p: Paleta, rotulo: &str, v: Variante, msg: Option<Accion>) -> Element<'a, Mensaje> {
    componentes::boton(p, rotulo, v, msg.map(m))
}

/// La página del paso abierto, o `None` si se muestra Acerca de.
pub fn pagina(app: &App) -> Option<Element<'_, Mensaje>> {
    let p = app.paleta();
    let e = &app.canal;
    let libre = !e.ocupado;
    let estable = version_estable(e);
    let v_estable = estable.map_or_else(|| "estable".to_owned(), |d| d.version.clone());
    let sig = siguiente();
    Some(match e.paso? {
        Paso::Beta => {
            let copia: Element<'_, Mensaje> = if app.aj.copias_antes {
                container(
                    row![
                        container(icono(Icono::Archivo, Tam::Base, p.text)).center(24),
                        column![
                            titulo("Se guarda una copia antes de cambiar", texto::BODY_SM.0, p.text),
                            t(
                                format!("Ajustes, cuentas, frases y grupos · {} · tus mundos no se tocan", crate::pantallas::exportacion_nueva::tam(e.bytes_datos)),
                                texto::MONO_SM.0,
                                p.text_muted,
                            )
                            .font(fuentes::MONO),
                        ]
                        .spacing(2),
                    ]
                    .spacing(espacio::S3)
                    .align_y(Alignment::Center),
                )
                .padding(espacio::S4)
                .width(Length::Fill)
                .style(caja(Some(p.surface_raised), p.border, borde::FINO))
                .into()
            } else {
                componentes::aviso_detallado(
                    p,
                    Tono::Aviso,
                    "No se guardará una copia",
                    "«Copias antes de actualizar» está desactivado. Sin copia, la única vuelta a Estable es esperar a la próxima versión o empezar de cero.",
                )
            };
            let cuerpo = column![
                punto(p, Icono::Brillo, "Recibes funciones antes.", "Las novedades llegan a Beta antes de publicarse para todos."),
                punto(p, Icono::Bug, "Puede fallar.", "Si algo se rompe, el informe de fallo ayuda a corregirlo."),
                punto(
                    p,
                    Icono::Deshacer,
                    "Volver a Estable no es inmediato.",
                    &format!("La Beta guarda datos que la {ACTUAL} no entiende. Para volver, Ícaro restaura la copia que hace ahora, o esperas a que la beta salga en Estable."),
                ),
                Space::with_height(espacio::S1),
                copia,
            ]
            .spacing(espacio::S4);
            marco(
                app,
                "Pasar a Beta",
                Some((Tono::Info, "Canal Beta")),
                "¿Pasar a la versión Beta?".into(),
                match &e.beta {
                    Some(b) => format!("Vas a instalar Ícaro {b} beta. Tienes {ACTUAL} Estable."),
                    None => format!("Vas a recibir la beta más reciente. Tienes {ACTUAL} Estable."),
                },
                cuerpo.into(),
                None,
                vec![
                    boton(p, "Quedarme en Estable", Variante::Secundario, libre.then_some(Accion::Cerrar)),
                    boton(p, if e.ocupado { "Guardando copia…" } else { "Pasar a Beta" }, Variante::Primario, libre.then_some(Accion::PasarABeta)),
                ],
            )
        }
        Paso::Volver => {
            let mut cuerpo = column![].spacing(espacio::S5);
            let mut lista = column![].spacing(espacio::S3);
            for (nombre, detalle) in crate::novedades::al_volver(ACTUAL) {
                let glifo = icono_funcion(&nombre);
                lista = lista.push(fila_icono(p, glifo, nombre, detalle));
            }
            cuerpo = cuerpo.push(lista);
            let (bajada, hay) = match &e.estable {
                Consulta::Pendiente => (format!("Tienes {}. Consultando la versión estable más reciente…", crate::actualizacion_real::version_visible()), false),
                Consulta::Lista(Some(d)) => (format!("Tienes {}. La versión estable más reciente es la {}.", crate::actualizacion_real::version_visible(), d.version), true),
                Consulta::Lista(None) => (format!("Tienes {}. Todavía no hay una versión estable publicada.", crate::actualizacion_real::version_visible()), false),
                Consulta::Error(error) => (format!("No se pudo consultar la versión estable: {error}"), false),
            };
            let copia_reciente = estable.and_then(|d| e.copias.iter().find(|c| compatible(c, d)));
            let esperar = column![
                row![titulo(format!("Esperar a la {sig} Estable"), 15., p.text), componentes::insignia(p, Tono::Exito, "Recomendado")]
                    .spacing(10)
                    .align_y(Alignment::Center),
                t(
                    format!("Te quedas en esta beta sin recibir más betas. Cuando la {sig} salga en Estable, se instala sola y no pierdes nada."),
                    texto::BODY_SM.0,
                    p.text_muted,
                ),
            ]
            .spacing(4)
            .width(Length::Fill);
            let ahora = column![
                titulo(format!("Volver ahora a la {v_estable}"), 15., if hay { p.text } else { p.text_disabled }),
                t(
                    match (hay, copia_reciente) {
                        (false, _) => "Disponible cuando se conozca la versión estable.".to_owned(),
                        (true, Some(c)) => format!(
                            "Ícaro restaura la copia del {} y reinicia. Los cambios hechos después de esa fecha en ajustes y grupos se pierden; tus mundos e instancias quedan igual.",
                            fecha(c.fecha)
                        ),
                        (true, None) => format!("No hay una copia que la {v_estable} pueda leer: tendrías que empezar de cero. Tus mundos e instancias quedan igual."),
                    },
                    texto::BODY_SM.0,
                    p.text_muted,
                ),
            ]
            .spacing(4)
            .width(Length::Fill);
            cuerpo = cuerpo.push(
                column![
                    componentes::etiqueta(p, "Cómo quieres volver"),
                    opcion(p, !e.ahora, Some(m(Accion::Como(false))), esperar.into()),
                    opcion(p, e.ahora, hay.then(|| m(Accion::Como(true))), ahora.into()),
                ]
                .spacing(10),
            );
            marco(
                app,
                "Volver a Estable",
                Some((Tono::Aviso, "Volver a Estable")),
                format!("Al volver a la {v_estable} dejarás de tener esto"),
                bajada,
                cuerpo.into(),
                None,
                vec![
                    boton(p, "Seguir en Beta", Variante::Secundario, Some(Accion::Cerrar)),
                    boton(
                        p,
                        &if e.ahora { "Elegir copia y volver".to_owned() } else { format!("Esperar a la {sig} Estable") },
                        Variante::Primario,
                        Some(Accion::Continuar),
                    ),
                ],
            )
        }
        Paso::Elegir => {
            let d = estable?;
            let mut cuerpo = column![].spacing(10);
            let primera = e.copias.iter().find(|c| compatible(c, d)).map(|c| c.id.as_str());
            for c in &e.copias {
                let sirve = compatible(c, d);
                let nota = if !sirve {
                    if c.beta {
                        format!("Es de una beta: la {} no puede leerla.", d.version)
                    } else {
                        format!("Es más nueva que la {}: no puede leerla.", d.version)
                    }
                } else if Some(c.id.as_str()) == primera {
                    format!("La más reciente compatible con {}.", d.version)
                } else {
                    format!("Hecha con la {}.", c.version)
                };
                let tinta = if sirve { p.text } else { p.text_disabled };
                let datos = column![
                    row![
                        titulo(fecha(c.fecha), 15., tinta),
                        Space::with_width(Length::Fill),
                        t(
                            format!("{}{} · {}", c.version, if c.beta { " beta" } else { "" }, crate::pantallas::exportacion_nueva::tam(c.bytes)),
                            texto::MONO_SM.0,
                            p.text_muted,
                        )
                        .font(fuentes::MONO),
                    ]
                    .spacing(espacio::S3)
                    .align_y(Alignment::Center),
                    t(nota, texto::BODY_SM.0, p.text_muted),
                ]
                .spacing(3)
                .width(Length::Fill);
                cuerpo = cuerpo.push(opcion(
                    p,
                    e.elegida.as_deref() == Some(c.id.as_str()),
                    (sirve && libre).then(|| m(Accion::Elegir(c.id.clone()))),
                    datos.into(),
                ));
            }
            if let Some(c) = e.elegida.as_ref().and_then(|id| e.copias.iter().find(|c| &c.id == id)) {
                cuerpo = cuerpo.push(Space::with_height(espacio::S1)).push(componentes::aviso(
                    p,
                    Tono::Aviso,
                    &format!("Se pierden los cambios en ajustes, frases y grupos hechos después del {}.", fecha(c.fecha)),
                ));
            }
            marco(
                app,
                "Elegir copia",
                None,
                "¿Qué copia restauramos?".into(),
                "Solo se restauran ajustes y datos de Ícaro. Mundos, mods e instancias no cambian.".into(),
                cuerpo.into(),
                Some(componentes::boton_con_icono(p, Icono::Atras, "Atrás", Variante::Secundario, libre.then(|| m(Accion::Atras)), false)),
                vec![boton(
                    p,
                    &format!("Restaurar y volver a {}", d.version),
                    Variante::Primario,
                    (libre && e.elegida.is_some()).then_some(Accion::Restaurar),
                )],
            )
        }
        Paso::Aplicando => {
            let elegida = e.elegida.as_ref().and_then(|id| e.copias.iter().find(|c| &c.id == id));
            let pasos = [
                "Copia de seguridad de la beta guardada".to_owned(),
                format!("Ícaro {v_estable} descargado · {}", crate::pantallas::exportacion_nueva::tam(e.bytes)),
                match (e.cero, elegida) {
                    (true, _) => "Reiniciando ajustes y cuentas".to_owned(),
                    (false, Some(c)) => format!("Restaurando la copia del {}", fecha(c.fecha)),
                    (false, None) => "Restaurando la copia".to_owned(),
                },
                "Reiniciar".to_owned(),
            ];
            let mut lista = column![].spacing(espacio::S3);
            for (i, rotulo) in pasos.into_iter().enumerate() {
                let i = i as u8;
                let marca: Element<'_, Mensaje> = if i < e.etapa {
                    container(icono(Icono::Check, Tam::Sm, p.success)).center(18).into()
                } else if i == e.etapa {
                    icaro_ui::progreso::spinner(p, app.reducir)
                } else {
                    container(container(Space::new(6, 6)).style(icaro_ui::estilo::bloque(p.border_strong))).center(18).into()
                };
                lista = lista.push(
                    row![marca, t(rotulo, texto::BODY_SM.0, if i <= e.etapa { p.text } else { p.text_muted })]
                        .spacing(espacio::S3)
                        .align_y(Alignment::Center),
                );
            }
            let total = (f32::from(e.etapa.min(3)) + if e.etapa == 1 { e.avance } else { 0. }) / 4.;
            marco(
                app,
                "Volviendo a Estable",
                None,
                format!("Volviendo a Ícaro {v_estable}"),
                "No cierres Ícaro. Se reinicia solo al terminar.".into(),
                column![lista, componentes::progreso(p, total.max(0.04))].spacing(espacio::S4).into(),
                None,
                Vec::new(),
            )
        }
        Paso::SinCopia => {
            let borde_fijo = |c: Element<'static, Mensaje>| -> Element<'static, Mensaje> {
                container(c).padding(espacio::S4).width(Length::Fill).style(caja(None, p.border, borde::FINO)).into()
            };
            let cuerpo = column![
                borde_fijo(fila_icono(
                    p,
                    Icono::Espera,
                    format!("Esperar a la {sig} Estable"),
                    format!("Dejas de recibir betas y pasas a Estable en cuanto salga la {sig}. No pierdes nada."),
                )),
                borde_fijo(fila_icono(
                    p,
                    Icono::Refrescar,
                    format!("Empezar de cero en {v_estable}"),
                    "Se reinician ajustes, cuentas y frases de Ícaro. Antes se guarda una copia de la beta. Tus instancias y mundos se vuelven a detectar desde la carpeta.".into(),
                )),
            ]
            .spacing(espacio::S3 + 2.);
            marco(
                app,
                "Sin copia",
                Some((Tono::Error, "No se puede volver ahora")),
                format!("No hay una copia compatible con la {v_estable}"),
                format!("Ninguna copia guardada se hizo en una versión estable, así que la {v_estable} no podría leer tus datos actuales."),
                cuerpo.into(),
                Some(boton(
                    p,
                    if e.confirmar_cero { "Sí, empezar de cero" } else { "Empezar de cero" },
                    Variante::Peligro,
                    libre.then_some(Accion::EmpezarDeCero),
                )),
                vec![
                    boton(p, "Seguir en Beta", Variante::Secundario, libre.then_some(Accion::Cerrar)),
                    boton(p, &format!("Esperar a la {sig} Estable"), Variante::Primario, libre.then_some(Accion::Esperar)),
                ],
            )
        }
    })
}

// ----------------------------------------------------------------- diseño

/// Estados de ejemplo del componente, para capturas y el modo demostración.
pub fn preparar_diseno(app: &mut App, escena: &str) {
    let copia = |id: &str, fecha: u64, version: &str, beta: bool| Copia {
        id: id.into(),
        fecha,
        version: version.into(),
        beta,
        esquema: 1,
        bytes: 2_202_009,
        archivos: Vec::new(),
    };
    let estable: Disponible = serde_json::from_value(serde_json::json!({
        "version": "0.3.4",
        "bytes": 18_769_510,
        "url": "https://github.com/00faithlss/Icaro-launcher/releases/download/v0.3.4/icaro-windows-x86_64.exe",
        "sha256": "00".repeat(32),
        "notas": []
    }))
    .expect("Escena de CanalActualizaciones válida");
    let mut e = Estado {
        generacion: app.canal.generacion + 1,
        bytes_datos: 2_202_009,
        beta: Some("0.4.1".into()),
        estable: Consulta::Lista(Some(estable)),
        copias: vec![
            copia("c", 1_794_076_800, "0.4.0", true),
            copia("a", 1_793_654_040, "0.3.4", false),
            copia("b", 1_793_178_120, "0.3.3", false),
        ],
        bytes: 18_769_510,
        ..Default::default()
    };
    e.paso = match escena {
        "canal-beta" => Some(Paso::Beta),
        "canal-volver" => Some(Paso::Volver),
        "canal-volver-ahora" => {
            e.ahora = true;
            Some(Paso::Volver)
        }
        "canal-elegir" => {
            e.ahora = true;
            e.elegida = Some("a".into());
            Some(Paso::Elegir)
        }
        "canal-aplicando" => {
            e.elegida = Some("a".into());
            e.etapa = 2;
            Some(Paso::Aplicando)
        }
        "canal-sincopia" => {
            e.copias.truncate(1);
            Some(Paso::SinCopia)
        }
        _ => None,
    };
    app.canal = e;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_estable_que_se_espera_sale_de_la_version_instalada() {
        let n = numeros(ACTUAL).unwrap();
        assert_eq!(siguiente(), format!("{}.{}", n[0], n[1]));
        assert_eq!(numeros("v1.4.0-beta.3"), Some([1, 4, 0]));
        assert_eq!(numeros("1.4"), None);
    }
}
