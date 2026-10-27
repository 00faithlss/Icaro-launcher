// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Archivos reales de mods; usa FilaMod, los filtros y el diálogo existentes.
use crate::{App, Mensaje};
use icaro_core::{
    instancias::Instancia,
    modrinth::Version,
    mods::{self, Instalado},
};
use icaro_ui::{
    componentes::{self, Variante},
    tema::espacio,
};
use iced::futures::SinkExt;
use iced::{
    widget::{column, row, text},
    Element, Task,
};
use std::collections::BTreeMap;
#[path = "packs_reales.rs"]
pub mod packs;

#[derive(Default)]
pub struct Estado {
    pub diseno: crate::diseno_mods::Estado,
    pub packs: packs::Estado,
    pub listas: BTreeMap<String, Vec<Instalado>>,
    pub recursos: BTreeMap<String, Vec<Instalado>>,
    pub tamanos: BTreeMap<String, u64>,
    pub updates: BTreeMap<String, Vec<Version>>,
    pub errores: BTreeMap<String, String>,
    pub busy: Option<String>,
    pub progreso: String,
    pub confirmacion: Option<(Instancia, Vec<Version>)>,
    pub confirmacion_recurso: Option<(Instancia, Version)>,
    selector_local: bool,
    comprobando: BTreeMap<String, u64>,
    secuencia: u64,
    pub(crate) operacion: u64,
}
#[derive(Debug, Clone)]
pub enum Accion {
    Cargar,
    ImportarLocal,
    LocalesElegidos(String, Option<Vec<std::path::PathBuf>>),
    LocalesImportados(String, u64, Result<usize, String>),
    Revisadas(String, u64, Result<Vec<Version>, String>),
    Instalar(String, Option<String>),
    Preparada(Instancia, u64, Result<Version, String>),
    ActualizarTodos,
    Actualizar(String),
    Confirmar,
    Cancelar,
    Progreso(u64, String),
    /// Con éxito, los mods que no se pudieron guardar y su motivo.
    Terminada(String, u64, Result<Vec<(String, String)>, String>),
    Alternar(String),
}
fn m(a: Accion) -> Mensaje {
    Mensaje::ModsReales(a)
}
pub fn instancia(app: &App) -> Option<&Instancia> {
    (!app.servicios.demo)
        .then(|| app.servicios.instancias.registros.get(app.editor_inst))
        .flatten()
}
pub fn permite_mods(app: &App) -> bool {
    if app.servicios.demo {
        return !crate::juego_de_instancia(app)
            .1
            .eq_ignore_ascii_case("vanilla");
    }
    instancia(app).is_some_and(|i| i.loader != "vanilla")
}
pub fn lista(app: &App) -> &[Instalado] {
    instancia(app)
        .and_then(|i| app.servicios.mods.listas.get(&i.id))
        .map_or(&[], Vec::as_slice)
}
pub fn updates(app: &App) -> &[Version] {
    instancia(app)
        .and_then(|i| app.servicios.mods.updates.get(&i.id))
        .map_or(&[], Vec::as_slice)
}
fn revisar(app: &mut App, i: Instancia) -> Task<Mensaje> {
    let source = app.servicios.catalog.clone();
    let s = &mut app.servicios.mods;
    s.secuencia += 1;
    let request = s.secuencia;
    s.comprobando.insert(i.id.clone(), request);
    let installed = s.listas.get(&i.id).cloned().unwrap_or_default();
    let id = i.id.clone();
    Task::perform(
        async move {
            let catalog = source.map_err(|e| e.to_string())?;
            let result = mods::actualizaciones(&catalog, &installed, &i.minecraft, &i.loader)
                .await
                .map_err(|e| e.to_string());
            let _ = catalog.guardar_metadata_cache().await;
            result
        },
        move |r| m(Accion::Revisadas(id.clone(), request, r)),
    )
}
pub fn cargar_local(app: &mut App) {
    let Some(i) = instancia(app).cloned() else {
        return;
    };
    recargar(app, &i.id);
}
/// Actualiza la instancia del ticket, aunque el usuario haya navegado a otra.
pub fn recargar(app: &mut App, id: &str) {
    let Some(n) = app
        .servicios
        .instancias
        .registros
        .iter()
        .position(|i| i.id == id)
    else {
        return;
    };
    let i = app.servicios.instancias.registros[n].clone();
    if i.loader == "vanilla" {
        return;
    }
    match mods::inventario(&app.servicios.instancias.almacen, &i.id) {
        Ok(list) => {
            if let Some(card) = crate::lista_instancias().lock().unwrap().get_mut(n) {
                card.mods = format!("{} mods", list.len());
            }
            let bytes = app
                .servicios
                .instancias
                .almacen
                .juego(&i.id)
                .ok()
                .map_or(0, |game| {
                    list.iter()
                        .filter_map(|m| {
                            std::fs::metadata(game.join("mods").join(if m.activo {
                                m.archivo.clone()
                            } else {
                                format!("{}.disabled", m.archivo)
                            }))
                            .ok()
                        })
                        .map(|m| m.len())
                        .sum()
                });
            app.servicios.mods.tamanos.insert(i.id.clone(), bytes);
            app.servicios.mods.listas.insert(i.id.clone(), list);
            app.servicios.mods.updates.remove(&i.id);
            app.servicios.mods.errores.remove(&i.id);
        }
        Err(e) => {
            app.servicios.mods.errores.insert(i.id, e.to_string());
        }
    }
}
/// «Encabezado Sodium: motivo · Iris: motivo».
fn texto_fallidos(encabezado: &str, fallidos: &[(String, String)]) -> String {
    let detalle: Vec<_> = fallidos.iter().map(|(n, e)| format!("{n}: {e}")).collect();
    format!("{encabezado} {}", detalle.join(" · "))
}

pub fn ejecutar(app: &mut App, i: Instancia, versions: Vec<Version>) -> Task<Mensaje> {
    let source = app.servicios.catalog.clone();
    let s = &mut app.servicios.mods;
    s.secuencia += 1;
    let request = s.secuencia;
    s.operacion = request;
    s.busy = Some(i.id.clone());
    s.errores.remove(&i.id);
    s.updates.remove(&i.id);
    s.comprobando.remove(&i.id);
    s.progreso = "Preparando los mods…".into();
    let store = app.servicios.instancias.almacen.clone();
    let id = i.id.clone();
    let nombres: std::collections::HashMap<String, String> = app
        .servicios
        .mods
        .listas
        .get(&i.id)
        .map(|l| l.iter().map(|m| (m.project_id.clone(), m.nombre.clone())).collect())
        .unwrap_or_default();
    Task::run(
        iced::stream::channel(32, async move |mut output| {
            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
            let operation = async {
                let catalog = source.map_err(|e| e.to_string())?;
                let avance = |tx: tokio::sync::mpsc::UnboundedSender<String>| {
                    move |p: &str| {
                        let _ = tx.send(p.into());
                    }
                };
                let total = versions.len();
                let result = match mods::instalar(
                    &catalog,
                    &store,
                    &id,
                    versions.clone(),
                    &i.minecraft,
                    &i.loader,
                    avance(tx.clone()),
                )
                .await
                {
                    Ok(_) => Ok(Vec::new()),
                    Err(e) if total < 2 => Err(e.to_string()),
                    // La transacción conjunta descarta todo ante un fallo: cada
                    // mod se repite en su propia transacción para conservar los
                    // que sí se pueden guardar.
                    Err(_) => {
                        let mut fallidos = Vec::new();
                        for v in versions {
                            let nombre = nombres
                                .get(&v.project_id)
                                .cloned()
                                .unwrap_or_else(|| v.name.clone());
                            if let Err(e) = mods::instalar(
                                &catalog,
                                &store,
                                &id,
                                vec![v],
                                &i.minecraft,
                                &i.loader,
                                avance(tx.clone()),
                            )
                            .await
                            {
                                fallidos.push((nombre, e.to_string()));
                            }
                        }
                        if fallidos.len() == total {
                            Err(texto_fallidos("No se pudo guardar ningún mod.", &fallidos))
                        } else {
                            Ok(fallidos)
                        }
                    }
                };
                drop(tx);
                let _ = catalog.guardar_metadata_cache().await;
                result
            };
            tokio::pin!(operation);
            let result = loop {
                tokio::select! {
                    r = &mut operation => break r,
                    Some(p) = rx.recv() => { let _ = output.send(Accion::Progreso(request, p)).await; }
                }
            };
            let _ = output
                .send(Accion::Terminada(i.id.clone(), request, result))
                .await;
        }),
        m,
    )
}
pub fn cargar_recursos(app: &mut App) {
    packs::cargar_local(app, mods::TipoPack::Recursos)
}
pub fn actualizar(app: &mut App, action: Accion) -> Task<Mensaje> {
    if app.servicios.demo {
        return Task::none();
    }
    match action {
        Accion::ImportarLocal => {
            let Some(i) = instancia(app).cloned().filter(|i| i.loader != "vanilla") else {
                return Task::none();
            };
            if app.servicios.mods.selector_local
                || app.servicios.mods.busy.is_some()
                || app.servicios.instancias.operando
                || app.servicios.sincronizacion.ocupado
                || app.servicios.juego.ocupada(&i.id)
            {
                return Task::none();
            }
            app.servicios.mods.selector_local = true;
            return Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .set_title("Importar mods propios")
                        .add_filter("Mods Java", &["jar"])
                        .pick_files()
                        .await
                        .map(|archivos| {
                            archivos
                                .into_iter()
                                .map(|f| f.path().to_path_buf())
                                .collect()
                        })
                },
                move |r| m(Accion::LocalesElegidos(i.id.clone(), r)),
            );
        }
        Accion::LocalesElegidos(id, archivos) => {
            app.servicios.mods.selector_local = false;
            let Some(archivos) = archivos.filter(|a| !a.is_empty()) else {
                return Task::none();
            };
            if app.servicios.mods.busy.is_some()
                || app.servicios.instancias.operando
                || app.servicios.sincronizacion.ocupado
                || app.servicios.juego.ocupada(&id)
            {
                return Task::none();
            }
            app.servicios.mods.secuencia += 1;
            let request = app.servicios.mods.secuencia;
            app.servicios.mods.operacion = request;
            app.servicios.mods.busy = Some(id.clone());
            app.servicios.mods.progreso = "Importando JAR personales…".into();
            let store = app.servicios.instancias.almacen.clone();
            let instancia = id.clone();
            return Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || {
                        mods::locales::importar(&store, &instancia, &archivos)
                    })
                    .await
                    .map_err(|_| "No se pudo importar el mod local.".to_owned())?
                    .map_err(|e| e.to_string())
                },
                move |r| m(Accion::LocalesImportados(id.clone(), request, r)),
            );
        }
        Accion::LocalesImportados(id, request, resultado)
            if request == app.servicios.mods.operacion =>
        {
            app.servicios.mods.busy = None;
            match resultado {
                Ok(n) => {
                    app.servicios.mods.errores.remove(&id);
                    app.servicios.mods.updates.remove(&id);
                    recargar(app, &id);
                    app.avisar(format!("Se importaron {n} JAR personales. Revisa sus requisitos y compatibilidad antes de jugar."));
                }
                Err(e) => {
                    app.servicios.mods.errores.insert(id, e.clone());
                    app.avisar_error(e);
                }
            }
        }
        Accion::Cargar => {
            let Some(i) = instancia(app).cloned().filter(|i| i.loader != "vanilla") else {
                return Task::none();
            };
            if app.servicios.mods.busy.as_ref() == Some(&i.id) {
                return Task::none();
            }
            cargar_local(app);
            if app.servicios.mods.errores.contains_key(&i.id) {
                return Task::none();
            }
            return Task::batch([
                revisar(app, i),
                crate::diseno_mods::actualizar(app, crate::diseno_mods::Accion::Metadata),
            ]);
        }
        Accion::Revisadas(id, request, result)
            if app.servicios.mods.comprobando.get(&id) == Some(&request) =>
        {
            let s = &mut app.servicios.mods;
            s.comprobando.remove(&id);
            match result {
                Ok(v) => {
                    s.updates.insert(id.clone(), v);
                    s.errores.remove(&id);
                }
                Err(e) => {
                    s.errores.insert(id, e);
                }
            }
        }
        Accion::Instalar(project, version) => {
            if app.servicios.mods.busy.is_some()
                || app.servicios.instancias.operando
                || app.editor != Some(2)
            {
                return Task::none();
            }
            let Some(i) = instancia(app).cloned().filter(|i| i.loader != "vanilla") else {
                return Task::none();
            };
            app.servicios.mods.secuencia += 1;
            let request = app.servicios.mods.secuencia;
            app.servicios.mods.operacion = request;
            app.servicios.mods.busy = Some(i.id.clone());
            app.servicios.mods.errores.remove(&i.id);
            app.servicios.mods.progreso = "Buscando una versión compatible…".into();
            let record = i.clone();
            let source = app.servicios.catalog.clone();
            return Task::perform(
                async move {
                    let catalog = source.map_err(|e| e.to_string())?;
                    let mut versions = if let Some(v) = version {
                        vec![catalog.version(&v).await.map_err(|e| e.to_string())?]
                    } else {
                        catalog
                            .versions(&project, Some(&i.minecraft), Some(&i.loader))
                            .await
                            .map_err(|e| e.to_string())?
                    };
                    versions.retain(|v| {
                        v.project_id == project && mods::compatible(v, &i.minecraft, &i.loader)
                    });
                    versions.sort_by(|a, b| b.date_published.cmp(&a.date_published));
                    versions
                        .iter()
                        .find(|v| v.version_type == "release")
                        .cloned()
                        .or_else(|| versions.into_iter().next())
                        .ok_or("No hay una versión compatible para esta instancia.".into())
                },
                move |r| m(Accion::Preparada(record.clone(), request, r)),
            );
        }
        Accion::Preparada(i, request, result) if app.servicios.mods.operacion == request => {
            app.servicios.mods.busy = None;
            match result {
                Ok(v) => {
                    let old = app
                        .servicios
                        .mods
                        .listas
                        .get(&i.id)
                        .and_then(|l| l.iter().find(|m| m.project_id == v.project_id));
                    if old.is_some_and(|m| m.version_id == v.id) {
                        // Resolver también permite reparar dependencias retiradas.
                        return ejecutar(app, i, vec![v]);
                    } else if old.is_some() {
                        app.servicios.mods.confirmacion = Some((i, vec![v]));
                    } else {
                        return ejecutar(app, i, vec![v]);
                    }
                }
                Err(e) => {
                    app.servicios.mods.errores.insert(i.id, e.clone());
                    app.avisar_error(e);
                }
            }
        }
        Accion::ActualizarTodos | Accion::Actualizar(_) if app.servicios.mods.busy.is_none() => {
            let Some(i) = instancia(app).cloned() else {
                return Task::none();
            };
            let chosen: Vec<_> = updates(app)
                .iter()
                .filter(|v| match &action {
                    Accion::Actualizar(id) => v.project_id == *id,
                    _ => true,
                })
                .cloned()
                .collect();
            if !chosen.is_empty() {
                app.servicios.mods.confirmacion = Some((i, chosen));
            }
        }
        Accion::Confirmar
            if app.servicios.mods.busy.is_none() && !app.servicios.instancias.operando =>
        {
            if let Some((i, versions)) = app.servicios.mods.confirmacion.take() {
                return ejecutar(app, i, versions);
            }
        }
        Accion::Cancelar => {
            app.servicios.mods.confirmacion = None;
            app.servicios.mods.confirmacion_recurso = None;
        }
        Accion::Progreso(request, p) if request == app.servicios.mods.operacion => {
            app.servicios.mods.progreso = p
        }
        Accion::Terminada(id, request, result) if request == app.servicios.mods.operacion => {
            app.servicios.mods.busy = None;
            app.servicios.mods.progreso.clear();
            match result {
                Ok(fallidos) => {
                    app.servicios.mods.diseno.cestas.remove(&id);
                    app.servicios.mods.diseno.seleccion.remove(&id);
                    let store = &app.servicios.instancias.almacen;
                    match mods::inventario(store, &id) {
                        Ok(l) => {
                            app.servicios.mods.listas.insert(id.clone(), l);
                            app.servicios.mods.errores.remove(&id);
                        }
                        Err(e) => {
                            app.servicios.mods.errores.insert(id.clone(), e.to_string());
                        }
                    }
                    for (n, record) in app.servicios.instancias.registros.iter().enumerate() {
                        if record.id == id {
                            if let Some(card) = crate::lista_instancias().lock().unwrap().get_mut(n)
                            {
                                card.mods = format!(
                                    "{} mods",
                                    app.servicios.mods.listas.get(&id).map_or(0, Vec::len)
                                );
                            }
                        }
                    }
                    recargar(app, &id);
                    if fallidos.is_empty() {
                        app.avisar(
                            "Mods guardados en la instancia, incluidas sus dependencias obligatorias."
                                .into(),
                        );
                    } else {
                        app.avisar_error(texto_fallidos(
                            "Se guardaron los demás mods. No se pudieron guardar:",
                            &fallidos,
                        ));
                    }
                    if let Some(i) = app
                        .servicios
                        .instancias
                        .registros
                        .iter()
                        .find(|i| i.id == id)
                        .cloned()
                    {
                        return Task::batch([
                            revisar(app, i),
                            crate::diseno_mods::actualizar(
                                app,
                                crate::diseno_mods::Accion::Metadata,
                            ),
                        ]);
                    }
                }
                Err(e) => {
                    app.servicios.mods.errores.insert(id, e.clone());
                    app.avisar_error(e);
                }
            }
        }
        Accion::Alternar(file)
            if app.servicios.mods.busy.is_none() && !app.servicios.instancias.operando =>
        {
            if let Some(i) = instancia(app) {
                if let Err(e) = mods::alternar(&app.servicios.instancias.almacen, &i.id, &file) {
                    app.avisar(e.to_string());
                }
                cargar_local(app);
            }
        }
        _ => {}
    }
    Task::none()
}
pub fn accion_proyecto<'a>(
    app: &'a App,
    project: &str,
    version: Option<&str>,
) -> Element<'a, Mensaje> {
    let p = app.paleta();
    let old = lista(app).iter().find(|m| m.project_id == project);
    let update =
        version.is_none() && old.is_some() && updates(app).iter().any(|v| v.project_id == project);
    let installed = !update && old.is_some_and(|m| version.is_none_or(|v| v == m.version_id));
    componentes::boton(
        p,
        if installed {
            "Instalado"
        } else if old.is_some() {
            "Actualizar"
        } else {
            "Instalar"
        },
        Variante::Primario,
        (!installed && app.servicios.mods.busy.is_none()).then(|| {
            if update {
                m(Accion::Actualizar(project.into()))
            } else {
                m(Accion::Instalar(project.into(), version.map(str::to_owned)))
            }
        }),
    )
}
pub fn accion_recurso<'a>(
    app: &'a App,
    project: &str,
    version: Option<&str>,
) -> Element<'a, Mensaje> {
    packs::accion_instalar(app, mods::TipoPack::Recursos, project, version)
}
/// Reutiliza las filas del System Design para el inventario real de ZIP.
pub fn vista_recursos(app: &App) -> Element<'_, Mensaje> {
    packs::vista(app, mods::TipoPack::Recursos)
}

pub fn estado_operacion(app: &App) -> Element<'_, Mensaje> {
    let p = app.paleta();
    let mut body = column![].spacing(espacio::S2);
    if app.servicios.mods.busy.is_some() {
        body = body.push(row![icaro_ui::progreso::spinner(p, app.reducir),
            text(&app.servicios.mods.progreso).color(p.text_muted)]
            .spacing(espacio::S3).align_y(iced::Alignment::Center))
            .push(icaro_ui::progreso::indeterminado(p, app.reducir));
    }
    if instancia(app).is_some_and(|i| app.servicios.mods.comprobando.contains_key(&i.id)) {
        body = body.push(row![icaro_ui::progreso::spinner(p, app.reducir),
            text("Comprobando actualizaciones…").color(p.text_muted)]
            .spacing(espacio::S3).align_y(iced::Alignment::Center));
    }
    if let Some(error) = instancia(app).and_then(|i| app.servicios.mods.errores.get(&i.id)) {
        body = body.push(text(error).color(p.warning));
    }
    body.into()
}
pub fn vista(app: &App) -> Element<'_, Mensaje> {
    crate::diseno_mods::lista(app)
}
pub fn capa<'a>(app: &'a App, window: Element<'a, Mensaje>) -> Element<'a, Mensaje> {
    let Some((i, versions)) = &app.servicios.mods.confirmacion else {
        return window;
    };
    let p = app.paleta();
    let body = column![text(format!("Se actualizarán {} mods de {}.", versions.len(), i.nombre)).color(p.text), text("Las nuevas versiones pueden ser incompatibles con otros mods. ¿Quieres continuar?").color(p.warning), text("Los archivos actuales se conservan si falla la descarga o no se pueden resolver las dependencias.").color(p.text_muted)].spacing(espacio::S4);
    crate::con_velo_losa(
        p,
        window,
        crate::dialogo(
            p,
            "¿Actualizar mods?",
            body.into(),
            vec![
                componentes::boton(
                    p,
                    "Cancelar",
                    Variante::Secundario,
                    Some(m(Accion::Cancelar)),
                ),
                componentes::boton(
                    p,
                    "Actualizar",
                    Variante::Primario,
                    Some(m(Accion::Confirmar)),
                ),
            ],
            Some(icaro_ui::laminas::Lamina::CaidaDeFaeton),
            620.,
            320.,
            m(Accion::Cancelar),
        ),
        m(Accion::Cancelar),
        crate::desfase_losa(app),
    )
}
pub fn suscripcion(app: &App) -> iced::Subscription<Mensaje> {
    let regular = if app.servicios.mods.confirmacion.is_some() {
        iced::keyboard::on_key_press(|key, _| {
            matches!(
                key,
                iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape)
            )
            .then(|| m(Accion::Cancelar))
        })
    } else {
        iced::Subscription::none()
    };
    iced::Subscription::batch([regular, packs::suscripcion(app)])
}

/// Escenas de revisión sin descargas ni acceso a archivos de la cuenta.
pub fn preparar_diseno(app: &mut App, scene: &str) {
    use icaro_core::modrinth::Project;
    app.servicios.demo = false;
    app.servicios.restoring = false;
    app.editor = Some(3);
    app.editor_inst = 0;
    app.mods_vista = usize::from(scene.contains("ficha"));
    let i = Instancia {
        id: "00000000000000000000000000000001".into(),
        nombre: "Supervivencia".into(),
        minecraft: "26.3".into(),
        loader: if scene.contains("vanilla") {
            "vanilla"
        } else {
            "fabric"
        }
        .into(),
        portada: 0,
        creada: 0,
    };
    app.general.nombre = i.nombre.clone();
    app.servicios.instancias.registros = vec![i.clone()];
    if let Some(card) = crate::lista_instancias().lock().unwrap().first_mut() {
        card.nombre = i.nombre.clone();
        card.version = format!("{} · {}", i.minecraft, i.loader);
    }
    let mc = if scene.contains("otra") {
        "1.20.1"
    } else {
        "26.3"
    };
    let mut project: Project = serde_json::from_value(serde_json::json!({"id":"50dA9Sha", "project_type":"resourcepack", "slug":"fresh-animations", "title":"Fresh Animations", "description":"Animaciones para mobs", "body":"# Fresh Animations\n\nPaquete de recursos que mejora las animaciones de las criaturas.", "license":{"id":"custom","name":"Licencia personalizada"}, "downloads":49620830, "game_versions":[mc], "loaders":["minecraft"], "categories":["entities"], "client_side":"required", "server_side":"unsupported"})).unwrap();
    let mut v: Version = serde_json::from_value(serde_json::json!({"id":"resourceqa", "project_id":"50dA9Sha", "name":"Fresh Animations", "version_number":"1.10.5", "version_type":"release", "game_versions":[mc], "loaders":["minecraft"], "downloads":10000, "date_published":"2026-04-01T00:00:00Z", "files":[], "dependencies":[]})).unwrap();
    if scene.starts_with("shaders") {
        project.project_type="shader".into();
        project.title="Complementary Reimagined".into();
        project.description="Iluminación, sombras y reflejos para Minecraft.".into();
        project.body="# Complementary Reimagined\n\nPaquete de shaders para Iris y OptiFine.".into();
        project.loaders=vec!["iris".into(),"optifine".into()];
        project.categories=vec!["realistic".into()];
        v.name=project.title.clone(); v.loaders=project.loaders.clone(); v.version_number="r5.6.1".into();
    }
    app.servicios.mods.recursos.insert(
        i.id.clone(),
        vec![Instalado {
            project_id: project.id.clone(),
            nombre: project.title.clone(),
            version_id: v.id.clone(),
            version: v.version_number.clone(),
            publicada: v.date_published.clone(),
            archivo: "FreshAnimations_v1.10.5.zip".into(),
            sha512: String::new(),
            activo: true,
            dependencias: vec![],
        }],
    );
    packs::preparar_diseno(app, scene, &i, &project, &v);
    if scene.contains("ficha") {
        app.servicios.mods.recursos.clear();
        app.servicios.document = crate::catalogo::Documento::parse(&project.body);
        app.servicios.ficha = crate::ficha::Estado::nuevo(None, std::slice::from_ref(&v));
        app.servicios.detail = Some((project, vec![v.clone()]));
    }
}
