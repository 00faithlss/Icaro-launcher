// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss
//! PantallaCuenta y EstudioSkin originales con perfil y texturas reales.
use crate::{pantallas, sesion, App, Mensaje};
use icaro_core::cuenta::{self, Perfil, Textura};
use icaro_ui::{
    componentes::{self, Variante},
    cuenta::{DatosCuenta, EstadoSesion, MensajesCuenta},
};
use iced::{
    widget::{column, text},
    Element, Task,
};
use std::{path::PathBuf, sync::Arc, time::Instant};

#[derive(Default)]
pub struct Estado {
    generacion: u64,
    pub actual: Option<Textura>,
    pub seleccion: Option<Textura>,
    perfil: Option<Perfil>,
    historial: Vec<(Textura, bool)>,
    cargando: bool,
    error: Option<String>,
    /// Texturas de las capas del perfil, en su mismo orden.
    capas: Vec<Option<Textura>>,
    /// Capa que se está poniendo (`Some(None)`: quitándola).
    capa_cambiando: Option<Option<usize>>,
    /// Tiempo jugado total y la instancia más jugada, en segundos.
    tiempo: Option<(u64, Option<(String, u64)>)>,
    /// Elemento de la biblioteca cargado en el estudio.
    biblioteca_elegida: Option<usize>,
}

/// Lo que se recibe al abrir la cuenta.
#[derive(Debug, Clone)]
pub struct Recibido {
    perfil: Perfil,
    textura: Option<Textura>,
    historial: Vec<(Textura, bool)>,
    capas: Vec<Option<Textura>>,
    tiempo: (u64, Option<(String, u64)>),
}

/// Datos del estudio que dependen de la cuenta real.
pub struct DatosEstudioReal<'a> {
    pub capas: Vec<icaro_ui::cuenta::CapaCuenta<'a>>,
    pub capa_puesta: Option<usize>,
    pub capa_cambiando: Option<Option<usize>>,
    pub biblioteca: Vec<Arc<[u8]>>,
    pub biblioteca_elegida: Option<usize>,
}

fn capas_cuenta(s: &Estado) -> Vec<icaro_ui::cuenta::CapaCuenta<'_>> {
    s.perfil.as_ref().map_or_else(Vec::new, |p| {
        p.capes
            .iter()
            .enumerate()
            .map(|(i, c)| icaro_ui::cuenta::CapaCuenta {
                nombre: c.alias.as_str(),
                rgba: s.capas.get(i).and_then(|t| t.as_ref()).map(|t| t.rgba.as_ref()),
            })
            .collect()
    })
}

fn capa_puesta(s: &Estado) -> Option<usize> {
    s.perfil.as_ref()?.capes.iter().position(|c| c.state == "ACTIVE")
}

pub fn datos_estudio(app: &App) -> DatosEstudioReal<'_> {
    let s = &app.sesion.cuenta;
    DatosEstudioReal {
        capas: capas_cuenta(s),
        capa_puesta: capa_puesta(s),
        capa_cambiando: s.capa_cambiando,
        biblioteca: s.historial.iter().map(|(t, _)| Arc::clone(&t.rgba)).collect(),
        biblioteca_elegida: s.biblioteca_elegida,
    }
}

/// Suma el tiempo jugado de las instancias y elige la más jugada.
fn tiempo_jugado(
    almacen: &icaro_core::instancias::Almacen,
    instancias: &[(String, String)],
) -> (u64, Option<(String, u64)>) {
    let mut total = 0u64;
    let mut mejor: Option<(String, u64)> = None;
    for (id, nombre) in instancias {
        let s = almacen.tiempo_jugado(id).unwrap_or(0);
        total = total.saturating_add(s);
        if s > 0 && mejor.as_ref().is_none_or(|(_, m)| s > *m) {
            mejor = Some((nombre.clone(), s));
        }
    }
    (total, mejor)
}
impl Estado {
    pub fn invalidar(&mut self) {
        let next = self.generacion + 1;
        *self = Self::default();
        self.generacion = next;
    }
}
#[derive(Debug, Clone)]
pub enum Accion {
    Abrir(bool),
    Recibida(u64, String, Result<Box<Recibido>, String>),
    ElegirCapa(Option<usize>),
    CapaPuesta(u64, String, Result<Perfil, String>),
    ElegirBiblioteca(usize),
    GuardarBiblioteca,
    Descargar,
    Descargada(Result<Option<String>, String>),
    ElegirArchivo,
    Archivo(Option<PathBuf>),
    Cargada(u64, Result<(Textura, String), String>),
    Elegir(usize),
    Aplicar(bool),
    Aplicada(u64, String, Textura, bool, Result<Perfil, String>),
}
fn m(a: Accion) -> Mensaje {
    Mensaje::CuentaReal(a)
}
fn historial(id: &str) -> PathBuf {
    assert!(id.len()==32 && id.bytes().all(|b|b.is_ascii_hexdigit()), "Identidad de cuenta validada por autenticación");
    crate::estado::directorio(false)
        .join("accounts")
        .join(id)
        .join("skins")
}
fn leer_historial(id: &str) -> Vec<(Textura, bool)> {
    let mut files = std::fs::read_dir(historial(id))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let hash = name.split('-').next()?;
            if hash.len() == 64
                && hash.bytes().all(|b| b.is_ascii_hexdigit())
                && (name.ends_with("-classic.png") || name.ends_with("-slim.png"))
            {
                Some((e.path(), e.metadata().ok()?.modified().ok()?))
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    files.sort_by(|a, b| b.1.cmp(&a.1));
    files
        .into_iter()
        .take(8)
        .filter_map(|(p, _)| {
            cuenta::cargar(&p)
                .ok()
                .map(|t| (t, p.to_string_lossy().ends_with("-slim.png")))
        })
        .collect()
}
fn guardar(id: &str, t: &Textura, slim: bool) -> Result<(), String> {
    // Nombres derivados del contenido; la biblioteca contiene solo PNG, sin tokens.
    let png = t.png.clone();
    let dir = historial(id);
    std::fs::create_dir_all(&dir).map_err(|_| "No se pudo guardar el historial de skins.")?;
    let hash = cuenta::huella(&png);
    std::fs::write(
        dir.join(format!(
            "{hash}-{}.png",
            if slim { "slim" } else { "classic" }
        )),
        &png,
    )
    .map_err(|_| "No se pudo guardar el historial de skins.".into())
}
pub fn actualizar(app: &mut App, a: Accion) -> Task<Mensaje> {
    match a {
        Accion::Abrir(estudio) => {
            app.sesion.menu = false;
            if app.servicios.session.as_ref().is_some_and(|s| s.es_local()) {
                return crate::cuenta_sin_conexion::actualizar(app, crate::cuenta_sin_conexion::Accion::Abrir);
            }
            if estudio && app.servicios.session.is_none() {
                return crate::servicios::actualizar(app, crate::servicios::Accion::Login);
            }
            app.pantalla = Some(if estudio {
                pantallas::Pantalla::Estudio
            } else {
                pantallas::Pantalla::Cuenta
            });
            crate::arrancar(app);
            if app.sesion.subiendo.is_some() || app.sesion.cuenta.cargando {
                return Task::none();
            }
            let Some(session) = app.servicios.session.clone() else {
                return Task::none();
            };
            app.sesion.cuenta.generacion += 1;
            let generation = app.sesion.cuenta.generacion;
            app.sesion.cuenta.cargando = true;
            app.sesion.cuenta.error = None;
            let id = session.profile.id.clone();
            let response_id = id.clone();
            let almacen = app.servicios.instancias.almacen.clone();
            let instancias: Vec<(String, String)> = app
                .servicios
                .instancias
                .registros
                .iter()
                .map(|i| (i.id.clone(), i.nombre.clone()))
                .collect();
            return Task::perform(
                async move {
                    let p = cuenta::perfil(&session).await.map_err(|e| e.to_string())?;
                    let texture = if let Some(skin) = p.skins.iter().find(|s| s.state == "ACTIVE") {
                        Some(
                            cuenta::descargar_skin(skin)
                                .await
                                .map_err(|e| e.to_string())?,
                        )
                    } else {
                        None
                    };
                    // Una capa que no se puede descargar se muestra sin textura.
                    let mut capas = Vec::with_capacity(p.capes.len());
                    for c in &p.capes {
                        capas.push(cuenta::descargar_capa(c).await.ok());
                    }
                    let (history, tiempo) = tokio::task::spawn_blocking(move || {
                        (leer_historial(&id), tiempo_jugado(&almacen, &instancias))
                    })
                    .await
                    .map_err(|_| "No se pudo leer el historial de skins.")?;
                    Ok(Box::new(Recibido { perfil: p, textura: texture, historial: history, capas, tiempo }))
                },
                move |r| m(Accion::Recibida(generation, response_id.clone(), r)),
            );
        }
        Accion::Recibida(g, id, r)
            if g == app.sesion.cuenta.generacion
                && app
                    .servicios
                    .session
                    .as_ref()
                    .is_some_and(|s| s.profile.id == id) =>
        {
            let s = &mut app.sesion.cuenta;
            s.cargando = false;
            match r {
                Ok(recibido) => {
                    let Recibido { perfil: p, textura: t, historial: mut h, capas, tiempo } = *recibido;
                    s.capas = capas;
                    s.tiempo = Some(tiempo);
                    let slim = p
                        .skins
                        .iter()
                        .find(|s| s.state == "ACTIVE")
                        .is_some_and(|s| s.variant == "SLIM");
                    if let Some(t) = &t {
                        if !h
                            .iter()
                            .any(|(old, model)| old.png == t.png && *model == slim)
                        {
                            h.insert(0, (t.clone(), slim));
                            h.truncate(8);
                        }
                    }
                    s.actual = t;
                    s.perfil = Some(p);
                    s.historial = h;
                    app.sesion.modelo = usize::from(slim);
                    app.sesion.en_uso = 0;
                    app.sesion.elegida = 0;
                }
                Err(e) => s.error = Some(e),
            }
        }
        Accion::ElegirArchivo if app.sesion.subiendo.is_none() => {
            return Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("Cargar skin PNG de 64 × 64")
                        .add_filter("Skin PNG", &["png"])
                        .pick_file()
                        .await
                        .map(|f| f.path().to_owned())
                },
                |p| m(Accion::Archivo(p)),
            );
        }
        Accion::Archivo(Some(path)) if app.sesion.subiendo.is_none() => {
            app.sesion.soltando = false;
            let generation = app.sesion.cuenta.generacion;
            return Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        let texture = cuenta::cargar(&path).map_err(|e| e.to_string())?;
                        let name = path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned();
                        Ok((texture, name))
                    })
                    .await
                    .map_err(|_| "No se pudo abrir la skin.".to_owned())?
                },
                move |r| m(Accion::Cargada(generation, r)),
            );
        }
        Accion::Cargada(g, r)
            if g == app.sesion.cuenta.generacion && app.sesion.subiendo.is_none() =>
        {
            match r {
                Ok((t, n)) => {
                    app.sesion.cuenta.seleccion = Some(t);
                    app.sesion.textura = Some(n);
                    app.sesion.error = None;
                }
                Err(e) => app.sesion.error = Some(e),
            }
        }
        Accion::Elegir(i) if app.sesion.subiendo.is_none() => {
            if i < app.sesion.cuenta.historial.len() {
                app.sesion.elegida = i;
            }
        }
        Accion::ElegirCapa(indice)
            if app.sesion.cuenta.capa_cambiando.is_none() && app.sesion.subiendo.is_none() =>
        {
            let Some(session) = app.servicios.session.clone() else {
                return Task::none();
            };
            let Some(perfil) = &app.sesion.cuenta.perfil else {
                return Task::none();
            };
            let capa = match indice {
                Some(i) => match perfil.capes.get(i) {
                    Some(c) => Some(c.id.clone()),
                    None => return Task::none(),
                },
                None => None,
            };
            app.sesion.cuenta.capa_cambiando = Some(indice);
            // Al elegir una capa, el visor gira a la espalda para mostrarla.
            if indice.is_some() {
                app.ahora = Instant::now();
                app.sesion.giro.ir(180, app.ahora, app.reducir);
            }
            let generation = app.sesion.cuenta.generacion;
            let id = session.profile.id.clone();
            return Task::perform(
                async move { cuenta::poner_capa(&session, capa.as_deref()).await.map_err(|e| e.to_string()) },
                move |r| m(Accion::CapaPuesta(generation, id.clone(), r)),
            );
        }
        Accion::CapaPuesta(g, id, r)
            if g == app.sesion.cuenta.generacion
                && app.servicios.session.as_ref().is_some_and(|s| s.profile.id == id) =>
        {
            app.sesion.cuenta.capa_cambiando = None;
            match r {
                // El perfil devuelto trae la capa activa; sus texturas no cambian.
                Ok(p) if p.capes.len() == app.sesion.cuenta.capas.len() => app.sesion.cuenta.perfil = Some(p),
                Ok(p) => {
                    app.sesion.cuenta.perfil = Some(p);
                    return actualizar(app, Accion::Abrir(app.pantalla == Some(pantallas::Pantalla::Estudio)));
                }
                Err(e) => app.avisar(e),
            }
        }
        Accion::ElegirBiblioteca(i) if app.sesion.subiendo.is_none() => {
            if let Some((t, slim)) = app.sesion.cuenta.historial.get(i).cloned() {
                app.sesion.cuenta.seleccion = Some(t);
                app.sesion.cuenta.biblioteca_elegida = Some(i);
                app.sesion.modelo = usize::from(slim);
                app.sesion.textura = Some("Skin de la biblioteca".into());
                app.sesion.error = None;
            }
        }
        Accion::GuardarBiblioteca => {
            let Some(id) = app.servicios.session.as_ref().map(|s| s.profile.id.clone()) else {
                return Task::none();
            };
            let slim = app.sesion.modelo == 1;
            let Some(t) = app.sesion.cuenta.seleccion.clone().or_else(|| app.sesion.cuenta.actual.clone()) else {
                return Task::none();
            };
            if app.sesion.cuenta.historial.iter().any(|(old, model)| old.png == t.png && *model == slim) {
                app.avisar("Esa skin ya está en la biblioteca.".into());
                return Task::none();
            }
            match guardar(&id, &t, slim) {
                Ok(()) => {
                    let s = &mut app.sesion.cuenta;
                    s.historial.insert(0, (t, slim));
                    s.historial.truncate(8);
                    s.biblioteca_elegida = Some(0);
                    app.avisar("Skin guardada en la biblioteca.".into());
                }
                Err(e) => app.avisar(e),
            }
        }
        Accion::Descargar => {
            let s = &app.sesion.cuenta;
            let elegida = (app.pantalla == Some(pantallas::Pantalla::Cuenta) && app.sesion.elegida != app.sesion.en_uso)
                .then(|| s.historial.get(app.sesion.elegida).map(|(t, _)| t.clone()))
                .flatten();
            let Some(t) = elegida.or_else(|| s.seleccion.clone()).or_else(|| s.actual.clone()) else {
                return Task::none();
            };
            let nombre = format!("{}.png", s.perfil.as_ref().map_or("skin", |p| p.name.as_str()));
            return Task::perform(
                async move {
                    let Some(destino) = rfd::AsyncFileDialog::new()
                        .set_title("Descargar skin")
                        .set_file_name(&nombre)
                        .add_filter("Skin PNG", &["png"])
                        .save_file()
                        .await
                    else {
                        return Ok(None);
                    };
                    let ruta = destino.path().to_owned();
                    tokio::task::spawn_blocking(move || std::fs::write(&ruta, &t.png).map(|_| ruta))
                        .await
                        .map_err(|_| "No se pudo guardar la skin.".to_owned())?
                        .map(|r| Some(r.file_name().unwrap_or_default().to_string_lossy().into_owned()))
                        .map_err(|_| "No se pudo guardar la skin.".to_owned())
                },
                |r| m(Accion::Descargada(r)),
            );
        }
        Accion::Descargada(r) => match r {
            Ok(Some(nombre)) => app.avisar(format!("Skin guardada como {nombre}.")),
            Ok(None) => {}
            Err(e) => app.avisar(e),
        },
        Accion::Aplicar(historia) if app.sesion.subiendo.is_none() => {
            let Some(session) = app.servicios.session.clone() else {
                return Task::none();
            };
            let selected = if historia {
                app.sesion.cuenta.historial.get(app.sesion.elegida).cloned()
            } else {
                app.sesion
                    .cuenta
                    .seleccion
                    .clone()
                    .or_else(||app.sesion.cuenta.actual.clone())
                    .map(|t| (t, app.sesion.modelo == 1))
            };
            let Some((texture, slim)) = selected else {
                return Task::none();
            };
            app.sesion.subiendo = Some(Instant::now());
            app.ahora = Instant::now();
            app.sesion.error = None;
            let generation = app.sesion.cuenta.generacion;
            let id = session.profile.id.clone();
            let output = texture.clone();
            return Task::perform(
                async move {
                    cuenta::aplicar(&session, &texture, slim)
                        .await
                        .map_err(|e| e.to_string())
                },
                move |r| {
                    m(Accion::Aplicada(
                        generation,
                        id.clone(),
                        output.clone(),
                        slim,
                        r,
                    ))
                },
            );
        }
        Accion::Aplicada(g, id, t, slim, r)
            if g == app.sesion.cuenta.generacion
                && app
                    .servicios
                    .session
                    .as_ref()
                    .is_some_and(|s| s.profile.id == id) =>
        {
            app.sesion.subiendo = None;
            match r {
                Ok(p) => {
                    if let Some(actual)=&app.sesion.cuenta.actual {
                        let old_slim=app.sesion.cuenta.perfil.as_ref().is_some_and(|p|p.skins.iter().any(|s|s.state=="ACTIVE"&&s.variant=="SLIM"));
                        let _=guardar(&id,actual,old_slim);
                    }
                    if let Err(e) = guardar(&id, &t, slim) {
                        app.avisar(e);
                    }
                    let s = &mut app.sesion.cuenta;
                    s.actual = Some(t.clone());
                    s.perfil = Some(p);
                    s.seleccion = None;
                    s.historial
                        .retain(|(old, model)| old.png != t.png || *model != slim);
                    s.historial.insert(0, (t, slim));
                    s.historial.truncate(8);
                    app.sesion.elegida = 0;
                    app.sesion.en_uso = 0;
                    app.sesion.textura = None;
                    app.sesion.modelo = usize::from(slim);
                    app.sesion.cuenta.biblioteca_elegida = Some(0);
                    app.ahora = Instant::now();
                    app.sesion.aplicada = Some(app.ahora);
                    app.avisar("Skin aplicada a tu cuenta de Minecraft.".into());
                }
                Err(e) => {
                    app.sesion.error = Some(e.clone());
                    app.sesion.cuenta.error = Some(e);
                }
            }
        }
        _ => {}
    }
    Task::none()
}
pub fn vista(app: &App) -> Element<'_, Mensaje> {
    if app.servicios.session.as_ref().is_some_and(|s| s.es_local()) {
        return column![
            text(app.servicios.nombre("Sin cuenta")).size(32).color(app.paleta().text),
            text("Sin conexión").color(app.paleta().text_muted),
            componentes::boton_con_icono(app.paleta(),icaro_ui::iconos::Icono::Usuario,"Gestionar cuenta sin conexión",Variante::Secundario,Some(crate::cuenta_sin_conexion::m(crate::cuenta_sin_conexion::Accion::Abrir)),false),
        ].spacing(20).into();
    }
    let s = &app.sesion;
    let real = &s.cuenta;
    let p = app.paleta();
    let names = vec!["skin"; real.historial.len()];
    let pixels = real
        .historial
        .iter()
        .map(|(t, _)| Arc::clone(&t.rgba))
        .collect::<Vec<_>>();
    let page = icaro_ui::cuenta::pantalla_cuenta(
        p,
        DatosCuenta {
            estado: if app.servicios.session.is_some() {
                EstadoSesion::Activa
            } else {
                EstadoSesion::SinConexion
            },
        nombre: real.perfil.as_ref().map_or_else(||app.servicios.nombre("Invitado"),|p|p.name.as_str()),
            proximo_cambio: "",
            historial: names,
            en_uso: s.en_uso,
            elegida: s.elegida,
            // La skin elegida del historial se ve en el visor como vista previa.
            textura: (s.elegida != s.en_uso)
                .then(|| real.historial.get(s.elegida).map(|(t, _)| t.rgba.as_ref()))
                .flatten()
                .or_else(|| real.actual.as_ref().map(|t| t.rgba.as_ref())),
            historial_real: pixels,
            delgado: s.modelo == 1,
            angulo: s.giro.angulo(app.ahora),
            grados: s.giro.grados(),
            capas: capas_cuenta(real),
            capa_puesta: capa_puesta(real),
            capa_cambiando: real.capa_cambiando,
            tiempo_total: real.tiempo.as_ref().map_or_else(|| "—".into(), |(t, _)| crate::estadisticas_reales::texto_jugado(Some(*t))),
            mas_jugada: real
                .tiempo
                .as_ref()
                .and_then(|(_, m)| m.as_ref())
                .map(|(n, t)| (n.clone(), crate::estadisticas_reales::texto_jugado(Some(*t)))),
        },
        MensajesCuenta {
            abrir_estudio: Mensaje::Ses(sesion::Accion::AbrirEstudio),
            elegir_skin: Box::new(|i| Mensaje::Ses(sesion::Accion::ElegirSkin(i))),
            usar_de_nuevo: Mensaje::Ses(sesion::Accion::UsarDeNuevo),
            descartar: Mensaje::Ses(sesion::Accion::Descartar),
            girar: |n| Mensaje::Ses(sesion::Accion::Girar(n)),
            elegir_capa: Box::new(|c| Mensaje::Ses(sesion::Accion::ElegirCapa(c))),
            descargar: real.actual.is_some().then_some(Mensaje::Ses(sesion::Accion::Descargar)),
            cerrar_sesion: Mensaje::Ses(sesion::Accion::CerrarSesion),
            iniciar_sesion: Mensaje::Ses(sesion::Accion::IniciarSesion),
            volver_a_entrar: Mensaje::Ses(sesion::Accion::VolverAEntrar),
            comprar: Mensaje::Ses(sesion::Accion::Comprar),
            jugar_invitado: Mensaje::Ses(sesion::Accion::JugarComoInvitado),
            cambiar_nombre: Some(Mensaje::Servicio(crate::servicios::Accion::Enlace(
                "https://www.minecraft.net/msaprofile/mygames/editprofile".into(),
            ))),
        },
    );
    let mut view = column![].width(iced::Length::Fill);
    if real.cargando {
        view = view.push(text("Consultando tu perfil y skin de Minecraft…").color(p.text_muted));
    }
    if let Some(error) = &real.error {
        view = view
            .push(componentes::aviso(p, componentes::Estado::Aviso, error))
            .push(componentes::boton_compacto(
                p,
                "Reintentar",
                Variante::Secundario,
                Some(m(Accion::Abrir(false))),
            ));
    }
    view.push(page).into()
}
