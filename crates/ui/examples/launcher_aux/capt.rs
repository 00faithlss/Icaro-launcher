// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Capturas, visor, Gabinete y estados vacíos del launcher de ejemplo.
//! La maqueta usa grabados; el modo real lee los originales guardados por F2.

#[path = "capturas_disco.rs"]
mod disco;

use std::collections::HashSet;
use std::time::Instant;

use icaro_ui::capturas::{Captura, MensajesCapturas, VistaCapturas, galeria};
use icaro_ui::fuentes;
use icaro_ui::gabinete::{self, MensajesGabinete};
use icaro_ui::laminas::Lamina;
use icaro_ui::movimiento;
use icaro_ui::tema::{duracion, espacio, texto};
use icaro_ui::vacios::{self, Contexto};
use icaro_ui::visor::{
    EstadoVisor, ItemImagenVisor, ItemVisor, MensajesVisor, visor, visor_imagen,
};
use iced::futures::SinkExt;
use iced::widget::image::Handle;
use iced::widget::{column, container, text};
use iced::{Element, Padding, Subscription, Task};

use crate::{App, Mensaje, instancias};

/// De dónde sale la lista que recorre el visor.
#[derive(Debug, Clone, PartialEq)]
pub enum Ambito {
    /// Capturas, todas o de una sola instancia.
    Capturas(Option<String>),
    /// Piezas del Gabinete.
    Gabinete,
}

pub struct Abierto {
    pub estado: EstadoVisor,
    pub ambito: Ambito,
}

pub struct Estado {
    pub capturas: Vec<Captura>,
    pub filtro: usize,
    pub seleccion: Vec<usize>,
    pub encima: Option<usize>,
    pub visor: Option<Abierto>,
    pub gabinete_encima: Option<usize>,
    /// Capturas enviadas a la papelera con su lugar, para Deshacer.
    pub papelera: Vec<(usize, Captura)>,
    papelera_real: Vec<icaro_core::papelera::Ticket>,
    eliminando: bool,
    pub aviso: Option<Aviso>,
    /// Muestra todas las pantallas sin datos.
    pub vacio: bool,
    /// Inicio de la Prensa de las láminas de la pantalla.
    pub prensa: Option<Instant>,
    leyendo: bool,
    solicitud: u64,
    error_lectura: Option<String>,
    /// Miniaturas en preparación o que no se pudieron generar; evita
    /// repetirlas en cada refresco.
    miniaturas_pendientes: HashSet<disco::Clave>,
    miniaturas_fallidas: HashSet<disco::Clave>,
}

pub struct Aviso {
    pub texto: String,
    pub deshacer: bool,
    pub desde: Instant,
}

#[derive(Debug, Clone)]
pub enum Accion {
    Refrescar,
    Listadas(u64, Result<Vec<Captura>, String>),
    Miniatura(disco::Clave, Result<Handle, String>),
    Copiada(Result<(), String>),
    Copiadas(usize, Result<(), String>),
    Filtrar(String),
    Abrir(usize),
    Alternar(usize),
    Pasar(Option<usize>),
    Copiar(usize),
    Carpeta(usize),
    Eliminar(usize),
    AbrirCarpeta,
    CopiarSeleccion,
    EliminarSeleccion,
    LimpiarSeleccion,
    VerTodas,
    Deshacer,
    PapeleraTerminada(bool, Vec<icaro_core::papelera::Ticket>, Option<String>),
    // Visor
    VisorPaso(i32),
    VisorAcercar,
    VisorAlejar,
    VisorCerrar,
    VisorCopiar,
    VisorCarpeta,
    VisorEliminar,
    // Gabinete
    GabinetePasar(Option<usize>),
    GabineteAbrir(usize),
    // Estados vacíos
    AlternarVacios,
}

fn captura(archivo: &str, instancia: &str, fecha: &str, mb: f32, imagen: Lamina) -> Captura {
    Captura {
        archivo: archivo.into(),
        instancia: instancia.into(),
        fecha: fecha.into(),
        bytes: (mb * 1024.0 * 1024.0) as u64,
        imagen,
        original: None,
    }
}

impl Estado {
    pub fn vacio() -> Self {
        Self {
            capturas: Vec::new(),
            filtro: 0,
            seleccion: Vec::new(),
            encima: None,
            visor: None,
            gabinete_encima: None,
            papelera: Vec::new(),
            papelera_real: Vec::new(),
            eliminando: false,
            aviso: None,
            vacio: false,
            prensa: None,
            leyendo: false,
            solicitud: 0,
            error_lectura: None,
            miniaturas_pendientes: HashSet::new(),
            miniaturas_fallidas: HashSet::new(),
        }
    }
    pub fn nuevo() -> Self {
        let capturas = vec![
            captura(
                "2026-10-09_01.12.04.png",
                "Supervivencia",
                "09-10-2026 01:12",
                3.4,
                Lamina::ParejaAldea,
            ),
            captura(
                "2026-10-08_22.40.31.png",
                "Supervivencia",
                "08-10-2026 22:40",
                2.9,
                Lamina::CascadaAgua,
            ),
            captura(
                "2026-10-07_19.03.55.png",
                "Skyblock",
                "07-10-2026 19:03",
                4.1,
                Lamina::SoldadosCastillo,
            ),
            captura(
                "2026-10-05_16.20.10.png",
                "Better MC",
                "05-10-2026 16:20",
                3.8,
                Lamina::IxionNubes,
            ),
            captura(
                "2026-10-02_11.48.09.png",
                "Supervivencia",
                "02-10-2026 11:48",
                2.6,
                Lamina::DemogorgonCueva,
            ),
            captura(
                "2026-09-28_09.15.42.png",
                "Mundo de Javier",
                "28-09-2026 09:15",
                3.1,
                Lamina::MelencoliaCometa,
            ),
            captura(
                "2026-09-27_21.07.18.png",
                "Better MC",
                "27-09-2026 21:07",
                4.4,
                Lamina::CascadaMolino,
            ),
            captura(
                "2026-09-25_18.33.02.png",
                "Supervivencia",
                "25-09-2026 18:33",
                2.2,
                Lamina::HuidaBosque,
            ),
            captura(
                "2026-09-22_14.55.47.png",
                "Skyblock",
                "22-09-2026 14:55",
                3.7,
                Lamina::MercadoAldea,
            ),
            captura(
                "2026-09-19_23.10.26.png",
                "Mundo de Javier",
                "19-09-2026 23:10",
                2.8,
                Lamina::TivoliCascada,
            ),
            captura(
                "2026-09-15_10.02.39.png",
                "Supervivencia",
                "15-09-2026 10:02",
                3.3,
                Lamina::ValleBosque,
            ),
            captura(
                "2026-09-12_20.41.15.png",
                "Better MC",
                "12-09-2026 20:41",
                4.0,
                Lamina::FortalezasValle,
            ),
        ];
        Self {
            capturas,
            filtro: 0,
            seleccion: Vec::new(),
            encima: None,
            visor: None,
            gabinete_encima: None,
            papelera: Vec::new(),
            papelera_real: Vec::new(),
            eliminando: false,
            aviso: None,
            vacio: std::env::var_os("ICARO_VACIO").is_some(),
            prensa: None,
            leyendo: false,
            solicitud: 0,
            error_lectura: None,
            miniaturas_pendientes: HashSet::new(),
            miniaturas_fallidas: HashSet::new(),
        }
    }

    /// Capturas visibles en un ámbito, como índices de `capturas`.
    fn visibles(&self, instancia: Option<&str>) -> Vec<usize> {
        self.capturas
            .iter()
            .enumerate()
            .filter(|(_, c)| coincide(c, instancia))
            .map(|(i, _)| i)
            .collect()
    }

    fn avisar(&mut self, texto: impl Into<String>, deshacer: bool) {
        self.aviso = Some(Aviso {
            texto: texto.into(),
            deshacer,
            desde: Instant::now(),
        });
    }

    /// Si hay algo moviéndose que exija seguir dibujando cuadros.
    pub fn animando(&self, ahora: Instant, reducir: bool) -> bool {
        !reducir && prensa_en_curso(self.prensa, ahora)
    }
}

fn coincide(c: &Captura, instancia: Option<&str>) -> bool {
    instancia.is_none_or(|n| {
        c.original
            .as_ref()
            .map_or(c.instancia.as_str(), |o| o.instancia_id.as_str())
            == n
    })
}

/// No bloquea la interfaz: lee solamente las carpetas de captura de las
/// instancias guardadas. El refresco no sustituye ni mueve los originales.
pub fn refrescar(app: &mut App) -> Task<Mensaje> {
    if app.servicios.demo || app.capt.leyendo {
        return Task::none();
    }
    app.capt.leyendo = true;
    app.capt.solicitud = app.capt.solicitud.wrapping_add(1);
    let solicitud = app.capt.solicitud;
    let store = app.servicios.instancias.almacen.clone();
    let registros = app.servicios.instancias.registros.clone();
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || disco::listar(&store, &registros))
                .await
                .unwrap_or_else(|_| Err("No se pudieron leer las capturas.".into()))
        },
        move |resultado| Mensaje::Capt(Accion::Listadas(solicitud, resultado)),
    )
}

fn clave(c: &Captura) -> Option<disco::Clave> {
    c.original
        .as_ref()
        .map(|o| (o.ruta.clone(), c.bytes, o.modificada))
}

/// Prepara en segundo plano las miniaturas que faltan, de la más reciente a
/// la más antigua; cada una aparece en la cuadrícula en cuanto está lista.
fn pedir_miniaturas(est: &mut Estado) -> Task<Mensaje> {
    let claves: Vec<disco::Clave> = est
        .capturas
        .iter()
        .filter(|c| c.original.as_ref().is_some_and(|o| o.miniatura.is_none()))
        .filter_map(clave)
        .filter(|k| !est.miniaturas_pendientes.contains(k) && !est.miniaturas_fallidas.contains(k))
        .collect();
    if claves.is_empty() {
        return Task::none();
    }
    est.miniaturas_pendientes.extend(claves.iter().cloned());
    Task::run(
        iced::stream::channel(16, async move |mut salida| {
            let (tx, mut rx) = tokio::sync::mpsc::channel(16);
            let trabajo = tokio::task::spawn_blocking(move || disco::miniaturas(claves, tx));
            while let Some((clave, resultado)) = rx.recv().await {
                if salida.send(Accion::Miniatura(clave, resultado)).await.is_err() {
                    break;
                }
            }
            drop(rx);
            let _ = trabajo.await;
        }),
        Mensaje::Capt,
    )
}

fn identidad(c: &Captura) -> (&str, &str) {
    (
        c.original
            .as_ref()
            .map_or(c.instancia.as_str(), |o| o.instancia_id.as_str()),
        c.archivo.as_str(),
    )
}

fn incorporar(est: &mut Estado, capturas: Vec<Captura>) {
    let seleccion: Vec<(String, String)> = est
        .seleccion
        .iter()
        .filter_map(|&i| est.capturas.get(i))
        .map(|c| {
            let (id, archivo) = identidad(c);
            (id.to_owned(), archivo.to_owned())
        })
        .collect();
    let abierta = est.visor.as_ref().and_then(|v| {
        let Ambito::Capturas(ambito) = &v.ambito else {
            return None;
        };
        est.visibles(ambito.as_deref())
            .get(v.estado.indice)
            .and_then(|&i| est.capturas.get(i))
            .map(|c| {
                let (id, archivo) = identidad(c);
                (id.to_owned(), archivo.to_owned())
            })
    });
    // Conserva los handles de archivos sin cambios para no volver a cargar
    // la textura cada dos segundos ni alterar el zoom del visor abierto.
    est.capturas = capturas
        .into_iter()
        .map(|mut nueva| {
            if let Some(anterior) = est.capturas.iter().find(|c| {
                identidad(c) == identidad(&nueva)
                    && c.bytes == nueva.bytes
                    && c.original.as_ref().and_then(|o| o.modificada)
                        == nueva.original.as_ref().and_then(|o| o.modificada)
            }) {
                if let (Some(n), Some(a)) = (&mut nueva.original, &anterior.original) {
                    n.imagen = a.imagen.clone();
                    n.miniatura = a.miniatura.clone();
                }
            }
            nueva
        })
        .collect();
    est.seleccion = est
        .capturas
        .iter()
        .enumerate()
        .filter(|(_, c)| {
            let (id, archivo) = identidad(c);
            seleccion.iter().any(|(i, a)| i == id && a == archivo)
        })
        .map(|(i, _)| i)
        .collect();
    est.encima = None;
    if let Some(v) = &mut est.visor {
        if let Ambito::Capturas(ambito) = &v.ambito {
            let visibles: Vec<_> = est
                .capturas
                .iter()
                .enumerate()
                .filter(|(_, c)| coincide(c, ambito.as_deref()))
                .collect();
            if visibles.is_empty() {
                est.visor = None;
            } else if let Some((id, archivo)) = abierta {
                v.estado.indice = visibles
                    .iter()
                    .position(|(_, c)| identidad(c) == (id.as_str(), archivo.as_str()))
                    .unwrap_or(v.estado.indice.min(visibles.len() - 1));
            }
        }
    }
}

fn prensa_en_curso(inicio: Option<Instant>, ahora: Instant) -> bool {
    inicio.is_some_and(|i| {
        ahora.saturating_duration_since(i).as_millis() < u128::from(duracion::PRENSA)
    })
}

/// Avance de la Prensa de las láminas de la pantalla, de 0 a 1.
fn prensa(app: &App) -> f32 {
    match app.capt.prensa {
        Some(i) if !app.reducir => movimiento::prensa(
            (app.ahora.saturating_duration_since(i).as_millis() as f32 / duracion::PRENSA as f32)
                .clamp(0.0, 1.0),
        ),
        _ => 1.0,
    }
}

/// Contexto de pintura de los estados vacíos de una pantalla.
pub fn contexto(app: &App) -> Contexto {
    Contexto {
        p: app.paleta(),
        ventana: app.ventana.width,
        prensa: prensa(app),
    }
}

/// Instancia a la que se limitan las capturas según dónde se esté.
fn instancia_actual(app: &App) -> Option<String> {
    if app.editor == Some(5) {
        if !app.servicios.demo {
            return app
                .servicios
                .instancias
                .registros
                .get(app.editor_inst)
                .map(|i| i.id.clone());
        }
        return instancias().get(app.editor_inst).map(|d| d.nombre.clone());
    }
    if app.capt.filtro == 0 {
        return None;
    }
    if !app.servicios.demo {
        return app
            .servicios
            .instancias
            .registros
            .get(app.capt.filtro - 1)
            .map(|i| i.id.clone());
    }
    nombres_filtro(app).get(app.capt.filtro).cloned()
}

fn nombres_filtro(app: &App) -> Vec<String> {
    if !app.servicios.demo {
        return std::iter::once("Todas las instancias".to_owned())
            .chain(
                app.servicios
                    .instancias
                    .registros
                    .iter()
                    .map(|i| i.nombre.clone()),
            )
            .collect();
    }
    std::iter::once("Todas las instancias".to_owned())
        .chain(instancias().into_iter().map(|d| d.nombre))
        .collect()
}

pub fn actualizar(app: &mut App, a: Accion) -> Task<Mensaje> {
    let ahora = Instant::now();
    match a {
        Accion::Refrescar => return refrescar(app),
        Accion::Listadas(solicitud, resultado) => {
            if solicitud != app.capt.solicitud {
                return Task::none();
            }
            app.capt.leyendo = false;
            match resultado {
                Ok(capturas) => {
                    incorporar(&mut app.capt, capturas);
                    app.capt.error_lectura = None;
                    return pedir_miniaturas(&mut app.capt);
                }
                Err(error) => {
                    if app.capt.error_lectura.as_ref() != Some(&error) {
                        app.capt.avisar(error.clone(), false);
                    }
                    app.capt.error_lectura = Some(error);
                }
            }
        }
        Accion::Miniatura(k, resultado) => {
            app.capt.miniaturas_pendientes.remove(&k);
            match resultado {
                // Si el original cambió mientras tanto, la clave ya no coincide
                // y el siguiente refresco pide la miniatura nueva.
                Ok(miniatura) => {
                    if let Some(c) = app
                        .capt
                        .capturas
                        .iter_mut()
                        .find(|c| clave(c).as_ref() == Some(&k))
                    {
                        if let Some(o) = &mut c.original {
                            o.miniatura = Some(miniatura);
                        }
                    }
                }
                Err(_) => {
                    app.capt.miniaturas_fallidas.insert(k);
                }
            }
        }
        Accion::Copiada(resultado) => app.capt.avisar(
            resultado.map_or_else(|e| e, |_| "Captura copiada al portapapeles.".into()),
            false,
        ),
        Accion::Copiadas(n, resultado) => app.capt.avisar(
            resultado.map_or_else(
                |e| e,
                |_| format!("{n} archivos de captura copiados al portapapeles."),
            ),
            false,
        ),
        Accion::Filtrar(nombre) => {
            app.capt.filtro = nombres_filtro(app)
                .iter()
                .position(|n| *n == nombre)
                .unwrap_or(0);
            app.capt.seleccion.clear();
        }
        Accion::Abrir(i) => {
            if !app.capt.seleccion.is_empty() {
                alternar(&mut app.capt, i);
            } else {
                let instancia = instancia_actual(app);
                let lista = app.capt.visibles(instancia.as_deref());
                if let Some(pos) = lista.iter().position(|&x| x == i) {
                    abrir_visor(app, pos, Ambito::Capturas(instancia));
                }
            }
        }
        Accion::Alternar(i) => alternar(&mut app.capt, i),
        Accion::Pasar(i) => app.capt.encima = i,
        Accion::Copiar(i) => return copiar(app, i),
        Accion::Carpeta(i) => abrir_carpeta(app, Some(i)),
        Accion::AbrirCarpeta => abrir_carpeta(app, None),
        Accion::VisorCarpeta => {
            let i = captura_del_visor(app);
            abrir_carpeta(app, i);
        }
        Accion::Eliminar(i) => return eliminar(app, &[i]),
        Accion::CopiarSeleccion => {
            if !app.servicios.demo {
                if app.capt.seleccion.len() == 1 {
                    let i = app.capt.seleccion[0];
                    return copiar(app, i);
                }
                let rutas: Vec<_> = app
                    .capt
                    .seleccion
                    .iter()
                    .filter_map(|&i| app.capt.capturas.get(i))
                    .filter_map(|c| c.original.as_ref().map(|o| o.ruta.clone()))
                    .collect();
                let n = rutas.len();
                if n == 0 {
                    return Task::none();
                }
                return Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || disco::copiar_archivos(&rutas))
                            .await
                            .unwrap_or_else(|_| Err("No se pudieron copiar las capturas.".into()))
                    },
                    move |r| Mensaje::Capt(Accion::Copiadas(n, r)),
                );
            }
            let n = app.capt.seleccion.len();
            app.capt
                .avisar(format!("{n} capturas copiadas al portapapeles."), false);
        }
        Accion::EliminarSeleccion => {
            let lista = app.capt.seleccion.clone();
            return eliminar(app, &lista);
        }
        Accion::LimpiarSeleccion => app.capt.seleccion.clear(),
        Accion::VerTodas => app.capt.filtro = 0,
        Accion::PapeleraTerminada(undo,tickets,error) => {
            app.capt.eliminando=false;
            app.capt.papelera_real=tickets;
            app.capt.avisar(error.unwrap_or_else(||if undo{"Capturas recuperadas.".into()}else{"Capturas enviadas a la papelera del sistema.".into()}),!app.capt.papelera_real.is_empty());
            return refrescar(app);
        }
        Accion::Deshacer => {
            if !app.servicios.demo {
                if app.capt.eliminando{return Task::none();}
                app.capt.eliminando=true;
                let tickets=app.capt.papelera_real.clone();
                return Task::perform(async move {tokio::task::spawn_blocking(move || {
                    let mut pending=vec![];let mut error=None;
                    for ticket in tickets {if let Err(e)=icaro_core::papelera::restaurar(&ticket){pending.push(ticket);error=Some(e);}}
                    (pending,error)
                }).await.unwrap_or_else(|_|(vec![],Some("No se pudo recuperar; revisa la papelera del sistema.".into())))},|(tickets,error)|Mensaje::Capt(Accion::PapeleraTerminada(true,tickets,error)));
            }
            deshacer(&mut app.capt);
        },
        Accion::VisorPaso(d) => {
            if let Some(v) = &mut app.capt.visor {
                let total = total_visor(&app.capt.capturas, &v.ambito) as i32;
                if total > 0 {
                    let nuevo = (v.estado.indice as i32 + d).rem_euclid(total) as usize;
                    v.estado.ir_a(nuevo);
                }
            }
        }
        Accion::VisorAcercar => {
            if let Some(v) = &mut app.capt.visor {
                v.estado.acercar();
            }
        }
        Accion::VisorAlejar => {
            if let Some(v) = &mut app.capt.visor {
                v.estado.alejar();
            }
        }
        Accion::VisorCerrar => cerrar_visor(app),
        Accion::VisorCopiar => {
            if let Some(i) = captura_del_visor(app) {
                return copiar(app, i);
            }
        }
        Accion::VisorEliminar => {
            if let Some(i) = captura_del_visor(app) {
                return eliminar(app, &[i]);
            }
        }
        Accion::GabinetePasar(i) => app.capt.gabinete_encima = i,
        Accion::GabineteAbrir(i) => abrir_visor(app, i, Ambito::Gabinete),
        Accion::AlternarVacios => {
            app.capt.vacio = !app.capt.vacio;
            crate::arrancar(app);
        }
    }
    app.ahora = ahora;
    Task::none()
}

fn alternar(est: &mut Estado, i: usize) {
    if let Some(pos) = est.seleccion.iter().position(|&x| x == i) {
        est.seleccion.remove(pos);
    } else {
        est.seleccion.push(i);
    }
}

fn abrir_visor(app: &mut App, indice: usize, ambito: Ambito) {
    let ahora = Instant::now();
    app.ahora = ahora;
    app.capt.visor = Some(Abierto {
        estado: EstadoVisor::nuevo(indice, ahora),
        ambito,
    });
}

fn cerrar_visor(app: &mut App) {
    app.capt.visor = None;
}

fn total_visor(capturas: &[Captura], ambito: &Ambito) -> usize {
    match ambito {
        Ambito::Gabinete => gabinete::PIEZAS.len(),
        Ambito::Capturas(instancia) => capturas
            .iter()
            .filter(|c| coincide(c, instancia.as_deref()))
            .count(),
    }
}

/// Índice, en `capturas`, de la captura que muestra el visor.
fn captura_del_visor(app: &App) -> Option<usize> {
    let v = app.capt.visor.as_ref()?;
    let Ambito::Capturas(instancia) = &v.ambito else {
        return None;
    };
    app.capt
        .visibles(instancia.as_deref())
        .get(v.estado.indice)
        .copied()
}

fn copiar(app: &mut App, i: usize) -> Task<Mensaje> {
    if let Some(c) = app.capt.capturas.get(i) {
        if let Some(original) = &c.original {
            let ruta = original.ruta.clone();
            return Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || disco::copiar(&ruta))
                        .await
                        .unwrap_or_else(|_| Err("No se pudo copiar la captura.".into()))
                },
                |r| Mensaje::Capt(Accion::Copiada(r)),
            );
        }
        let nombre = c.archivo.clone();
        app.capt
            .avisar(format!("{nombre} copiada al portapapeles."), false);
    }
    Task::none()
}

fn abrir_carpeta(app: &mut App, captura: Option<usize>) {
    if !app.servicios.demo {
        let ruta = if let Some(captura) = captura {
            app.capt
                .capturas
                .get(captura)
                .and_then(|c| c.original.as_ref())
                .and_then(|o| {
                    disco::original_valido(&o.ruta)
                        .ok()
                        .map(|_| o.ruta.parent().unwrap().to_path_buf())
                })
                .ok_or_else(|| "La captura ya no está en su carpeta.".to_owned())
        } else if let Some(id) = instancia_actual(app) {
            disco::carpeta(&app.servicios.instancias.almacen, &id)
        } else {
            // Todas las instancias: cada una conserva su propia carpeta.
            app.capt
                .capturas
                .first()
                .and_then(|c| c.original.as_ref())
                .map(|o| disco::carpeta(&app.servicios.instancias.almacen, &o.instancia_id))
                .unwrap_or_else(|| {
                    Err(
                        "Todavía no hay capturas. Pulsa F2 dentro de Minecraft para guardar una."
                            .into(),
                    )
                })
        };
        match ruta {
            Ok(ruta) if ruta.is_dir() => {
                if let Err(error) = crate::pantallas::abrir_en_sistema(&ruta) {
                    app.capt.avisar(error, false);
                }
            }
            Ok(_) => app.capt.avisar(
                "Todavía no hay capturas. Pulsa F2 dentro de Minecraft para guardar una.",
                false,
            ),
            Err(error) => app.capt.avisar(error, false),
        }
        return;
    }
    let dir = std::env::temp_dir().join("icaro-demo").join("capturas");
    let _ = std::fs::create_dir_all(&dir);
    if let Err(error) = crate::pantallas::abrir_en_sistema(&dir) { app.capt.avisar(error, false); }
}

/// Envía capturas a la papelera; se pueden recuperar con Deshacer.
fn eliminar(app: &mut App, indices: &[usize]) -> Task<Mensaje> {
    if !app.servicios.demo {
        if app.capt.eliminando{return Task::none();}
        let mut paths:Vec<_>=indices.iter().filter_map(|n|app.capt.capturas.get(*n)?.original.as_ref().map(|o|o.ruta.clone())).collect();
        paths.sort();paths.dedup();
        if paths.is_empty(){return Task::none();}
        app.capt.eliminando=true;
        return Task::perform(async move {tokio::task::spawn_blocking(move || {
            let mut tickets=vec![];let mut error=None;
            for path in paths { match disco::original_valido(&path).and_then(|_|icaro_core::papelera::enviar(&path)) {Ok(t)=>tickets.push(t),Err(e)=>{error=Some(e);break;}} }
            (tickets,error)
        }).await.unwrap_or_else(|_|(vec![],Some("No se pudo completar la eliminación; revisa la papelera del sistema.".into())))},|(tickets,error)|Mensaje::Capt(Accion::PapeleraTerminada(false,tickets,error)));
    }
    let mut orden: Vec<usize> = indices.to_vec();
    orden.sort_unstable();
    orden.dedup();
    let mut quitadas = Vec::new();
    for &i in orden.iter().rev() {
        if i < app.capt.capturas.len() {
            quitadas.push((i, app.capt.capturas.remove(i)));
        }
    }
    if quitadas.is_empty() {
        return Task::none();
    }
    quitadas.reverse();
    let n = quitadas.len();
    app.capt.papelera = quitadas;
    app.capt.seleccion.clear();
    app.capt.encima = None;
    app.capt.avisar(
        if n == 1 {
            "Captura enviada a la papelera.".to_owned()
        } else {
            format!("{n} capturas enviadas a la papelera.")
        },
        true,
    );
    // El visor sigue en la que quedó en ese lugar, o se cierra si no queda ninguna.
    let total = app
        .capt
        .visor
        .as_ref()
        .map(|v| total_visor(&app.capt.capturas, &v.ambito));
    if total == Some(0) {
        cerrar_visor(app);
    } else if let (Some(v), Some(total)) = (&mut app.capt.visor, total) {
        v.estado.ir_a(v.estado.indice.min(total - 1));
    }
    Task::none()
}

fn deshacer(est: &mut Estado) {
    for (i, c) in std::mem::take(&mut est.papelera) {
        let lugar = i.min(est.capturas.len());
        est.capturas.insert(lugar, c);
    }
    est.aviso = None;
}

// ---------------------------------------------------------------- vistas

fn mensajes() -> MensajesCapturas<Mensaje> {
    MensajesCapturas {
        filtrar: |n| Mensaje::Capt(Accion::Filtrar(n)),
        abrir: |i| Mensaje::Capt(Accion::Abrir(i)),
        alternar: |i| Mensaje::Capt(Accion::Alternar(i)),
        pasar: |i| Mensaje::Capt(Accion::Pasar(i)),
        copiar: |i| Mensaje::Capt(Accion::Copiar(i)),
        carpeta: |i| Mensaje::Capt(Accion::Carpeta(i)),
        eliminar: |i| Mensaje::Capt(Accion::Eliminar(i)),
        abrir_carpeta: Mensaje::Capt(Accion::AbrirCarpeta),
        copiar_seleccion: Mensaje::Capt(Accion::CopiarSeleccion),
        eliminar_seleccion: Mensaje::Capt(Accion::EliminarSeleccion),
        limpiar_seleccion: Mensaje::Capt(Accion::LimpiarSeleccion),
    }
}

/// Ancho que queda para el contenido de una sección.
fn ancho_contenido(app: &App) -> f32 {
    let barra = if app.colapsada { 64.0 } else { 232.0 };
    app.ventana.width - barra - 2.0 * espacio::S12
}

/// Galería (o su estado vacío), limitada a una instancia si se indica.
fn cuerpo<'a>(app: &'a App, instancia: Option<String>, con_filtro: bool) -> Element<'a, Mensaje> {
    let p = app.paleta();
    let ctx = contexto(app);
    let est = &app.capt;
    let visibles: Vec<usize> = est.visibles(instancia.as_deref());
    if !con_filtro && (est.vacio || visibles.is_empty()) {
        return vacios::capturas_para_jugar(ctx, Mensaje::Jugar(app.editor_inst));
    }
    if est.vacio || est.capturas.is_empty() {
        return vacios::capturas(ctx, Mensaje::Ir(icaro_ui::shell::Seccion::Instancias));
    }
    // Las seleccionadas que ya no se ven dejan de contar.
    let seleccion: Vec<usize> = est
        .seleccion
        .iter()
        .copied()
        .filter(|i| visibles.contains(i))
        .collect();
    let vista = VistaCapturas {
        p,
        ancho: ancho_contenido(app),
        instancias: if con_filtro {
            nombres_filtro(app)
        } else {
            Vec::new()
        },
        filtro: est.filtro,
        capturas: &est.capturas,
        visibles: &visibles,
        seleccion: &seleccion,
        encima: est.encima,
    };
    let galeria = galeria(&vista, &mensajes());
    if visibles.is_empty() {
        let nombre = instancia
            .map(|id| {
                app.servicios
                    .instancias
                    .registros
                    .iter()
                    .find(|i| i.id == id)
                    .map_or(id.clone(), |i| i.nombre.clone())
            })
            .unwrap_or_default();
        return column![
            galeria,
            vacios::capturas_de_instancia(ctx, &nombre, Mensaje::Capt(Accion::VerTodas)),
        ]
        .spacing(espacio::S6)
        .into();
    }
    galeria
}

/// Pantalla Capturas.
pub fn pantalla(app: &App) -> Element<'_, Mensaje> {
    let p = app.paleta();
    let nombre = instancia_actual(app);
    let contenido = column![
        container(
            text("CAPTURAS")
                .font(fuentes::DISPLAY)
                .size(texto::DISPLAY_XL.0)
                .color(p.text)
        ),
        cuerpo(app, nombre, true),
    ]
    .spacing(espacio::S6)
    .padding(Padding::from([espacio::S6, espacio::S12]));
    crate::suave(app, contenido)
}

/// Pestaña Capturas del editor, limitada a la instancia abierta.
pub fn pestana(app: &App) -> Element<'_, Mensaje> {
    let nombre = instancia_actual(app);
    icaro_ui::scroll::desplazable(
        container(cuerpo(app, nombre, false)).padding(Padding::from([espacio::S6, espacio::S12])),
    )
}

/// Gabinete de grabados, con su encabezado.
pub fn pared(app: &App) -> Element<'_, Mensaje> {
    let p = app.paleta();
    column![
        gabinete::encabezado(p),
        gabinete::gabinete(
            p,
            app.capt.gabinete_encima,
            None,
            app.reducir,
            &MensajesGabinete {
                pasar: |i| Mensaje::Capt(Accion::GabinetePasar(i)),
                abrir: |i| Mensaje::Capt(Accion::GabineteAbrir(i)),
                usar: None,
            },
        ),
    ]
    .spacing(espacio::S8)
    .into()
}

/// El visor reemplaza la ventana para excluir el fondo de los eventos.
pub fn capa<'a>(app: &'a App, ventana: Element<'a, Mensaje>) -> Element<'a, Mensaje> {
    let p = app.paleta();
    let mut resultado = ventana;
    if let Some(v) = &app.capt.visor {
        if let Some(c) = captura_del_visor(app).and_then(|i| app.capt.capturas.get(i)) {
            if let Some(original) = &c.original {
                let ambito = match &v.ambito {
                    Ambito::Capturas(id) => id.as_deref(),
                    _ => None,
                };
                let item = ItemImagenVisor {
                    titulo: c.archivo.clone(),
                    detalle: format!("{} · {}", c.instancia, c.fecha),
                    handle: Some(original.imagen.clone()),
                    aviso: String::new(),
                };
                let m = |a| Mensaje::Capt(a);
                return crate::con_barra_ventana(app, visor_imagen(
                    &v.estado,
                    &item,
                    app.capt.visibles(ambito).len(),
                    MensajesVisor {
                        cerrar: m(Accion::VisorCerrar),
                        anterior: m(Accion::VisorPaso(-1)),
                        siguiente: m(Accion::VisorPaso(1)),
                        acercar: m(Accion::VisorAcercar),
                        alejar: m(Accion::VisorAlejar),
                        copiar: Some(m(Accion::VisorCopiar)),
                        carpeta: Some(m(Accion::VisorCarpeta)),
                        eliminar: Some(m(Accion::VisorEliminar)),
                    },
                ));
            }
        }
        let (item, total, acciones) = match &v.ambito {
            Ambito::Gabinete => {
                let piezas = gabinete::items_visor();
                let total = piezas.len();
                let item = piezas
                    .into_iter()
                    .nth(v.estado.indice)
                    .unwrap_or(ItemVisor {
                        titulo: String::new(),
                        detalle: String::new(),
                        lamina: Lamina::Umbral,
                        aspecto: None,
                    });
                (item, total, false)
            }
            Ambito::Capturas(instancia) => {
                let lista = app.capt.visibles(instancia.as_deref());
                let c = lista.get(v.estado.indice).map(|&i| &app.capt.capturas[i]);
                let item = c.map_or(
                    ItemVisor {
                        titulo: String::new(),
                        detalle: String::new(),
                        lamina: Lamina::Umbral,
                        aspecto: Some(16.0 / 9.0),
                    },
                    |c| ItemVisor {
                        titulo: c.archivo.clone(),
                        detalle: format!("{} · {}", c.instancia, c.fecha),
                        lamina: c.imagen,
                        aspecto: Some(16.0 / 9.0),
                    },
                );
                (item, lista.len(), true)
            }
        };
        let m = |a: Accion| Mensaje::Capt(a);
        resultado = crate::con_barra_ventana(app, visor(
            p.modo,
            &v.estado,
            app.ahora,
            app.reducir,
            &item,
            total,
            MensajesVisor {
                cerrar: m(Accion::VisorCerrar),
                anterior: m(Accion::VisorPaso(-1)),
                siguiente: m(Accion::VisorPaso(1)),
                acercar: m(Accion::VisorAcercar),
                alejar: m(Accion::VisorAlejar),
                copiar: acciones.then(|| m(Accion::VisorCopiar)),
                carpeta: acciones.then(|| m(Accion::VisorCarpeta)),
                eliminar: acciones.then(|| m(Accion::VisorEliminar)),
            },
        ));
    }
    resultado
}

/// Teclado del visor.
pub fn suscripcion(app: &App) -> Subscription<Mensaje> {
    let mut subs = Vec::new();
    if !app.servicios.demo
        && std::env::var_os("ICARO_CAPTURA_DISENO").is_none()
        && (app.seccion == icaro_ui::shell::Seccion::Capturas
            || app.editor == Some(5)
            || matches!(
                app.capt.visor.as_ref().map(|v| &v.ambito),
                Some(Ambito::Capturas(_))
            ))
    {
        subs.push(
            iced::time::every(std::time::Duration::from_secs(2))
                .map(|_| Mensaje::Capt(Accion::Refrescar)),
        );
    }
    if app.capt.visor.is_some() {
        subs.push(iced::keyboard::on_key_press(|tecla, _| {
            use iced::keyboard::Key;
            use iced::keyboard::key::Named;
            let a = match tecla.as_ref() {
                Key::Named(Named::Escape) => Accion::VisorCerrar,
                Key::Named(Named::ArrowLeft) => Accion::VisorPaso(-1),
                Key::Named(Named::ArrowRight) => Accion::VisorPaso(1),
                Key::Character("+" | "=") => Accion::VisorAcercar,
                Key::Character("-") => Accion::VisorAlejar,
                _ => return None,
            };
            Some(Mensaje::Capt(a))
        }));
    }
    Subscription::batch(subs)
}

// ------------------------------------------------------ estados vacíos

/// Pantalla con su banda y el vacío debajo.
fn con_banda<'a>(
    app: &'a App,
    lamina: Lamina,
    titulo: &str,
    vacio: Element<'a, Mensaje>,
) -> Element<'a, Mensaje> {
    let p = app.paleta();
    column![
        icaro_ui::laminas::banda(p, lamina, titulo, Some(0)),
        crate::suave(app, container(vacio).padding(vacios::margen())),
    ]
    .into()
}

pub fn vacio_instancias(app: &App) -> Element<'_, Mensaje> {
    con_banda(
        app,
        Lamina::CaidaCielo,
        "Instancias",
        vacios::instancias(
            contexto(app),
            Mensaje::Pant(crate::pantallas::Accion::NuevaInstancia),
            Mensaje::Pant(crate::pantallas::Accion::Importar),
        ),
    )
}

pub fn vacio_servidores(app: &App) -> Element<'_, Mensaje> {
    if !app.servicios.demo {
        return con_banda(
            app,
            Lamina::TivoliCiudad,
            "Servidores",
            vacios::estado_vacio(
                contexto(app),
                Lamina::Alas,
                "Ningún servidor\nguardado",
                "Aquí aparecerán los servidores que guardes. La gestión de servidores todavía está en preparación.",
                Vec::new(),
            ),
        );
    }
    con_banda(
        app,
        Lamina::TivoliCiudad,
        "Servidores",
        vacios::servidores(contexto(app), Mensaje::Nada),
    )
}

pub fn vacio_descargas(app: &App) -> Element<'_, Mensaje> {
    con_banda(
        app,
        Lamina::MercurioFriso,
        "Descargas",
        vacios::descargas(
            contexto(app),
            Mensaje::Ir(icaro_ui::shell::Seccion::Modpacks),
        ),
    )
}

pub fn vacio_consola(app: &App) -> Element<'_, Mensaje> {
    let p = app.paleta();
    column![
        container(
            text("CONSOLA")
                .font(fuentes::DISPLAY)
                .size(texto::DISPLAY_XL.0)
                .color(p.text)
        )
        .padding(Padding::from([espacio::S6, espacio::S12])),
        crate::suave(
            app,
            container(vacios::consola(
                contexto(app),
                Mensaje::Ir(icaro_ui::shell::Seccion::Instancias)
            ))
            .padding(vacios::margen())
        ),
    ]
    .into()
}

/// Búsqueda sin resultados dentro de un panel: la lámina va arriba.
pub fn vacio_busqueda<'a>(app: &App, consulta: &str, limpiar: Mensaje) -> Element<'a, Mensaje> {
    let ctx = Contexto {
        ventana: 0.0,
        ..contexto(app)
    };
    vacios::busqueda(ctx, consulta, limpiar)
}

#[cfg(test)]
mod tests {
    use super::*;
    use icaro_ui::capturas::OriginalCaptura;
    use iced::widget::image::Handle;

    fn local(id: &str, archivo: &str, nombre: &str) -> Captura {
        Captura {
            archivo: archivo.into(),
            instancia: nombre.into(),
            fecha: "11-10-2026 20:03".into(),
            bytes: 16,
            imagen: Lamina::Umbral,
            original: Some(OriginalCaptura {
                ruta: std::path::PathBuf::from(id).join(archivo),
                instancia_id: id.into(),
                imagen: Handle::from_rgba(2, 2, vec![255; 16]),
                miniatura: None,
                modificada: None,
            }),
        }
    }

    #[test]
    fn filtra_por_identidad_y_conserva_visor_tras_nueva_captura_o_renombrado() {
        let mut est = Estado::vacio();
        est.capturas = vec![
            local("a", "foto.png", "Primera"),
            local("b", "foto.png", "Segunda"),
        ];
        assert_eq!(est.visibles(Some("a")), vec![0]);
        assert_eq!(est.visibles(Some("b")), vec![1]);
        est.seleccion = vec![0];
        let mut estado = EstadoVisor::nuevo(0, Instant::now());
        estado.zoom = 200;
        est.visor = Some(Abierto {
            estado,
            ambito: Ambito::Capturas(Some("a".into())),
        });
        incorporar(
            &mut est,
            vec![
                local("a", "nueva.png", "Renombrada"),
                local("b", "foto.png", "Segunda"),
                local("a", "foto.png", "Renombrada"),
            ],
        );
        assert_eq!(est.visibles(Some("a")), vec![0, 2]);
        assert_eq!(est.seleccion, vec![2]);
        assert_eq!(est.visor.as_ref().unwrap().estado.indice, 1);
        assert_eq!(est.visor.as_ref().unwrap().estado.zoom, 200);
        assert_eq!(est.capturas[2].instancia, "Renombrada");
        incorporar(&mut est, vec![local("b", "foto.png", "Segunda")]);
        assert!(est.visor.is_none());
        assert!(est.seleccion.is_empty());
    }
}
