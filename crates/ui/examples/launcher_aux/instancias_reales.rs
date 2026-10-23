// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Conecta el asistente del diseño con el almacén local y las versiones de Mojang.

use crate::{pantallas, App, Mensaje};
use icaro_core::{
    instancias::{Almacen, Instancia},
    manifest::{self, VersionManifest},
};
use icaro_ui::instancias::{DatosInstancia, EstadoInstancia};
use iced::Task;

pub struct Estado {
    pub almacen: Almacen,
    pub registros: Vec<Instancia>,
    pub versiones: Vec<manifest::VersionEntry>,
    pub cargando: bool,
    pub creando: bool,
    pub error: Option<String>,
    pub eliminar: Option<String>,
    pub operando: bool,
    pub deshacer: Option<Eliminada>,
    pub loaders: std::collections::BTreeMap<String, icaro_core::versiones_instancia::Loaders>,
    pub cargando_loaders: Option<String>,
    pub loader_elegida: Option<String>,
    pub cambio: Option<Cambio>,
    pub version_offset: f32,
    pub cambio_offset: f32,
    request: u64,
    cambio_request: u64,
}
impl Default for Estado {
    fn default() -> Self {
        Self {
            almacen: Almacen::nuevo(crate::estado::directorio(false).join("instances")),
            registros: vec![],
            versiones: vec![],
            cargando: false,
            creando: false,
            error: None,
            eliminar: None,
            operando: false,
            deshacer: None,
            loaders: Default::default(),
            cargando_loaders: None,
            loader_elegida: None,
            cambio: None,
            version_offset: 0.0,
            cambio_offset: 0.0,
            request: 0,
            cambio_request: 0,
        }
    }
}

pub struct Cambio {
    pub instancia: String,
    pub tipo: usize,
    pub busqueda: String,
    pub destino: String,
    pub revision: Option<icaro_core::versiones_instancia::Revision>,
    pub revisando: bool,
    pub respaldo: bool,
    pub error: Option<String>,
    pub compatibilidades:
        std::collections::BTreeMap<String, icaro_core::versiones_instancia::Compatibilidad>,
    pub cargando_compatibilidad: std::collections::BTreeSet<String>,
    pub errores_compatibilidad: std::collections::BTreeMap<String, String>,
    pub loader_destino: String,
    pub loader_actual_version: Option<String>,
    pub solo_loader: bool,
    pub loader_version_destino: Option<String>,
    pub actualizar_mods: bool,
    pub copia: bool,
    pub detalle_desactivar: bool,
    pub confirmando: bool,
    pub compat_generacion: u64,
    pub mods: Option<Vec<icaro_core::mods::Instalado>>,
    pub destino_manual: bool,
    pub cancelar_compatibilidad: tokio_util::sync::CancellationToken,
}

pub struct Eliminada {
    registro: Instancia,
    grupo: Option<String>,
    config: icaro_ui::editor::ConfigJava,
}
impl Estado {
    pub fn cargar(&mut self) {
        match self.almacen.cargar() {
            Ok(report) => {
                self.registros = report.instancias;
                for instance in &self.registros {
                    if let Err(e) =
                        icaro_core::minecraft::migrar_directorio(&self.almacen, instance)
                    {
                        self.error = Some(format!(
                            "No se pudieron migrar los datos de {}: {e}",
                            instance.nombre
                        ));
                    }
                }
                if report.omitidas > 0 {
                    self.error = Some(format!(
                        "No se pudieron leer {} instancias. Sus archivos se conservan en disco.",
                        report.omitidas
                    ));
                }
            }
            Err(e) => self.error = Some(e.to_string()),
        }
    }
    pub fn datos(store: &Almacen, i: &Instancia) -> DatosInstancia {
        let ready = icaro_core::minecraft::listo(store, i);
        DatosInstancia {
            nombre: i.nombre.clone(),
            version: format!("{} · {}", i.minecraft, crate::ficha::loader(&i.loader)),
            ultima_vez: "Sin jugar".into(),
            mods: "Sin mods".into(),
            portada: pantallas::PORTADAS[i.portada as usize].1,
            estado: if ready {
                EstadoInstancia::Lista
            } else {
                EstadoInstancia::Pendiente
            },
            seleccionada: false,
            detalle: if ready {
                "Lista para jugar"
            } else {
                "Minecraft pendiente de instalación"
            }
            .into(),
        }
    }
}

/// Relee las instancias conservando los ajustes y la selección por identidad.
pub fn recargar(app: &mut App) -> Result<(), String> {
    if app.servicios.demo || app.servicios.instancias.operando || app.servicios.instancias.creando
        || app.servicios.mods.busy.is_some() || app.editor.is_some()
        || app.servicios.instancias.registros.iter().any(|i| app.servicios.juego.ocupada(&i.id)) {
        return Ok(());
    }
    let store = app.servicios.instancias.almacen.clone();
    let report = store.cargar().map_err(|e| e.to_string())?;
    if report.omitidas > 0 {
        return Err(format!("No se pudieron leer {} instancias. Sus archivos se conservan en disco.", report.omitidas));
    }
    let anteriores = &app.servicios.instancias.registros;
    let elegida = app.seleccionada.and_then(|n| anteriores.get(n)).map(|i| i.id.clone());
    let editor = anteriores.get(app.editor_inst).map(|i| i.id.clone());
    let tarjetas = crate::instancias();
    let anteriores: std::collections::BTreeMap<_, _> = anteriores.iter().enumerate()
        .map(|(n, i)| (i.id.clone(), tarjetas.get(n).cloned())).collect();
    let mut guardado = crate::estado::Guardado::nuevo(&app.lista_grupos, &app.grupos, &app.cerrados, &app.configs, &[]);
    guardado.asociar(app.servicios.instancias.registros.iter().map(|i| i.id.clone()).collect());
    guardado.restaurar_instancias(report.instancias.iter().map(|i| i.id.clone()).collect());
    guardado.cargar_configs(&store)?;
    let mut tarjetas = vec![];
    for i in &report.instancias {
        icaro_core::minecraft::migrar_directorio(&store, i).map_err(|e| e.to_string())?;
        let version = format!("{} · {}", i.minecraft, crate::ficha::loader(&i.loader));
        // Navegar no requiere revisar miles de assets en el hilo de la interfaz.
        // La instalación vuelve a verificar los archivos antes de iniciar Java.
        let mut tarjeta = anteriores.get(&i.id).cloned().flatten()
            .filter(|t| t.version == version)
            .unwrap_or_else(|| Estado::datos(&store, i));
        tarjeta.nombre = i.nombre.clone();
        tarjeta.version = version;
        tarjeta.portada = pantallas::PORTADAS[i.portada as usize].1;
        tarjetas.push(tarjeta);
    }
    app.seleccionada = elegida.and_then(|id| report.instancias.iter().position(|i| i.id == id));
    app.editor_inst = editor.and_then(|id| report.instancias.iter().position(|i| i.id == id)).unwrap_or(0);
    app.grupos = guardado.grupos.clone();
    app.configs = guardado.configs();
    app.servicios.instancias.registros = report.instancias;
    *crate::lista_instancias().lock().unwrap() = tarjetas;
    Ok(())
}

#[derive(Debug, Clone)]
pub enum Accion {
    ModsCompatibles(
        String,
        u64,
        Result<(Vec<icaro_core::mods::Instalado>, Option<String>), String>,
    ),
    Compatibilidad(
        String,
        u64,
        String,
        String,
        Result<icaro_core::versiones_instancia::Compatibilidad, String>,
    ),
    Versiones(u64, Result<VersionManifest, String>),
    Creada(Result<Instancia, String>),
    Eliminada(String, Result<(), String>),
    Restaurada(String, Result<Instancia, String>),
    Loaders(
        String,
        Result<icaro_core::versiones_instancia::Loaders, String>,
    ),
    Revision(
        String,
        u64,
        String,
        String,
        Result<icaro_core::versiones_instancia::Revision, String>,
    ),
    Cambiada(
        String,
        Result<icaro_core::versiones_instancia::Resultado, String>,
    ),
}

pub fn cargar_loaders(app: &mut App) -> Task<Mensaje> {
    let Some(mc) = elegida(app) else {
        return Task::none();
    };
    cargar_loaders_version(app, mc)
}
fn cargar_loaders_version(app: &mut App, mc: String) -> Task<Mensaje> {
    if app.servicios.demo
        || app.servicios.instancias.loaders.contains_key(&mc)
        || app.servicios.instancias.cargando_loaders.as_ref() == Some(&mc)
    {
        return Task::none();
    }
    app.servicios.instancias.cargando_loaders = Some(mc.clone());
    let request = mc.clone();
    Task::perform(
        async move { icaro_core::versiones_instancia::loaders(&request).await },
        move |r| Mensaje::Instancias(Accion::Loaders(mc.clone(), r)),
    )
}
pub fn abrir_cambio(app: &mut App, index: usize) -> Task<Mensaje> {
    abrir_cambio_tipo(app,index,false)
}
fn abrir_cambio_tipo(app:&mut App,index:usize,solo_loader:bool)->Task<Mensaje> {
    let Some(i) = app.servicios.instancias.registros.get(index).cloned() else {
        return Task::none();
    };
    if app.servicios.juego.ocupada(&i.id) || app.servicios.mods.busy.is_some() {
        app.avisar(
            "Detén el juego o espera a que termine la instalación antes de cambiar la versión."
                .into(),
        );
        return Task::none();
    }
    app.menu = None;
    app.cambiar = Some(index);
    if let Some(previo) = &app.servicios.instancias.cambio {
        previo.cancelar_compatibilidad.cancel();
    }
    app.servicios.instancias.cambio_offset = 0.;
    app.servicios.instancias.cambio_request += 1;
    let generation = app.servicios.instancias.cambio_request;
    app.servicios.instancias.cambio = Some(Cambio {
        instancia: i.id.clone(),
        tipo: 0,
        busqueda: String::new(),
        destino: if solo_loader {i.minecraft.clone()} else {String::new()},
        revision: None,
        revisando: false,
        respaldo: true,
        error: None,
        compatibilidades: Default::default(),
        cargando_compatibilidad: Default::default(),
        errores_compatibilidad: Default::default(),
        loader_destino: i.loader.clone(),
        loader_actual_version: None,
        solo_loader,
        loader_version_destino: None,
        actualizar_mods: !solo_loader,
        copia: false,
        detalle_desactivar: false,
        confirmando: false,
        compat_generacion: generation,
        mods: None,
        destino_manual: false,
        cancelar_compatibilidad: tokio_util::sync::CancellationToken::new(),
    });
    let store = app.servicios.instancias.almacen.clone();
    let id = i.id.clone();
    Task::batch([
        if solo_loader {cargar_loaders_cambio(app)} else {abrir(app)},
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let mods = if solo_loader {Vec::new()} else {
                        icaro_core::versiones_instancia::mods_para_compatibilidad(&store, &i)?
                    };
                    Ok((
                        mods,
                        icaro_core::versiones_instancia::leer_loader(&store, &i),
                    ))
                })
                .await
                .map_err(|_| "No se pudieron revisar los mods instalados.".to_owned())?
            },
            move |r| Mensaje::Instancias(Accion::ModsCompatibles(id.clone(), generation, r)),
        ),
    ])
}
pub fn abrir_cambio_loader(app:&mut App,index:usize)->Task<Mensaje> {
    if app.servicios.instancias.registros.get(index).is_none_or(|i|i.loader=="vanilla") {return Task::none();}
    abrir_cambio_tipo(app,index,true)
}
pub fn aplicar_cambio_loader(app:&mut App)->Task<Mensaje> {
    let Some(c)=app.servicios.instancias.cambio.as_ref().filter(|c|c.solo_loader) else{return Task::none();};
    let Some(i)=app.servicios.instancias.registros.iter().find(|i|i.id==c.instancia).cloned() else{return Task::none();};
    if app.servicios.instancias.operando || app.servicios.juego.ocupada(&i.id) || app.servicios.mods.busy.is_some(){return Task::none();}
    let Some(version)=c.loader_version_destino.clone() else{return Task::none();};
    let index=crate::pantallas::LOADERS.iter().position(|(name,_,_)|name.eq_ignore_ascii_case(&i.loader)).unwrap_or(0);
    if !app.servicios.instancias.loaders.get(&i.minecraft).is_some_and(|l|l.versiones[index].contains(&version)) || c.loader_actual_version.as_ref()==Some(&version){return Task::none();}
    let backup=c.respaldo;let store=app.servicios.instancias.almacen.clone();let id=i.id.clone();
    app.servicios.instancias.operando=true;
    Task::perform(async move {tokio::task::spawn_blocking(move || icaro_core::versiones_instancia::cambiar_solo_loader(&store,&i,&version,backup)).await.map_err(|_|"No se pudo cambiar la versión del loader.".to_owned())?},move |r|Mensaje::Instancias(Accion::Cambiada(id.clone(),r)))
}
pub fn revisar_cambio(app: &mut App) -> Task<Mensaje> {
    let Some(c) = app.servicios.instancias.cambio.as_mut() else {
        return Task::none();
    };
    if c.solo_loader || c.revisando || c.destino.is_empty() {
        return Task::none();
    }
    let Some(i) = app
        .servicios
        .instancias
        .registros
        .iter()
        .find(|i| i.id == c.instancia)
        .cloned()
    else {
        return Task::none();
    };
    if c.destino == i.minecraft {
        return Task::none();
    }
    c.revisando = true;
    c.error = None;
    let target = c.destino.clone();
    let dest = target.clone();
    let id = i.id.clone();
    let loader = c.loader_destino.clone();
    let generation = c.compat_generacion;
    let response_loader = loader.clone();
    let store = app.servicios.instancias.almacen.clone();
    Task::perform(
        async move {
            icaro_core::versiones_instancia::revisar_con_loader(&store, &i, &target, &loader).await
        },
        move |r| {
            Mensaje::Instancias(Accion::Revision(
                id.clone(),
                generation,
                dest.clone(),
                response_loader.clone(),
                r,
            ))
        },
    )
}
pub fn cambiar(app: &mut App, copia: bool) -> Task<Mensaje> {
    if app.servicios.instancias.operando {
        return Task::none();
    }
    let Some(c) = app.servicios.instancias.cambio.as_ref() else {
        return Task::none();
    };
    let Some(i) = app
        .servicios
        .instancias
        .registros
        .iter()
        .find(|i| i.id == c.instancia)
        .cloned()
    else {
        return Task::none();
    };
    if app.servicios.juego.ocupada(&i.id) || app.servicios.mods.busy.is_some() {
        app.avisar(
            "Detén el juego o espera a que termine la instalación antes de cambiar la versión."
                .into(),
        );
        return Task::none();
    }
    let Some(revision) = c.revision.clone().filter(|r|r.loader.is_empty() || r.loader==i.loader) else {
        return Task::none();
    };
    if !c.respaldo
        && icaro_core::versiones_instancia::baja(
            &app.servicios.instancias.versiones,
            &i.minecraft,
            &c.destino,
        )
    {
        return Task::none();
    }
    let respaldo = c.respaldo;
    let actualizar_mods = c.actualizar_mods;
    let destination = c.destino.clone();
    let id = i.id.clone();
    let store = app.servicios.instancias.almacen.clone();
    app.servicios.instancias.operando = true;
    Task::perform(
        async move {
            let catalog = icaro_core::modrinth::Catalog::new().map_err(|e| e.to_string())?;
            icaro_core::versiones_instancia::aplicar_con_mods(
                &catalog,
                &store,
                &i,
                &destination,
                &revision,
                copia,
                respaldo,
                actualizar_mods,
            )
            .await
        },
        move |r| Mensaje::Instancias(Accion::Cambiada(id.clone(), r)),
    )
}

pub fn eliminar(app: &mut App) -> Task<Mensaje> {
    if app.servicios.mods.busy.is_some() {
        app.avisar(
            "Espera a que termine la operación de mods antes de eliminar una instancia.".into(),
        );
        return Task::none();
    }
    let s = &mut app.servicios.instancias;
    if s.operando {
        return Task::none();
    }
    let Some(id) = s.eliminar.clone() else {
        return Task::none();
    };
    if app.servicios.juego.ocupada(&id) {
        app.avisar("Detén el juego o su instalación antes de eliminar esta instancia.".into());
        return Task::none();
    }
    if !s.registros.iter().any(|i| i.id == id) {
        return Task::none();
    }
    s.operando = true;
    let store = s.almacen.clone();
    let target = id.clone();
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || store.eliminar(&target))
                .await
                .map_err(|_| "No se pudo eliminar la instancia.".to_owned())?
                .map_err(|e| e.to_string())
        },
        move |result| Mensaje::Instancias(Accion::Eliminada(id.clone(), result)),
    )
}

pub fn deshacer(app: &mut App) -> Task<Mensaje> {
    let s = &mut app.servicios.instancias;
    if s.operando {
        return Task::none();
    }
    let Some(deleted) = &s.deshacer else {
        return Task::none();
    };
    let id = deleted.registro.id.clone();
    let target = id.clone();
    let store = s.almacen.clone();
    s.operando = true;
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || store.restaurar(&target))
                .await
                .map_err(|_| "No se pudo recuperar la instancia.".to_owned())?
                .map_err(|e| e.to_string())
        },
        move |result| Mensaje::Instancias(Accion::Restaurada(id.clone(), result)),
    )
}

pub fn versiones(app: &App, tipo: usize) -> Vec<(String, String)> {
    if app.servicios.demo {
        return pantallas::VERSIONES[tipo.min(2)]
            .iter()
            .map(|(v, d)| ((*v).into(), (*d).into()))
            .collect();
    }
    app.servicios
        .instancias
        .versiones
        .iter()
        .filter(|v| match tipo {
            0 => v.kind == "release",
            1 => v.kind == "snapshot",
            _ => matches!(v.kind.as_str(), "old_alpha" | "old_beta"),
        })
        .map(|v| (v.id.clone(), crate::ficha::fecha(&v.release_time)))
        .collect()
}
pub fn versiones_cambio(app: &App, tipo: usize) -> Vec<(String, String)> {
    if tipo < 2 {
        return versiones(app, tipo);
    }
    if app.servicios.demo {
        return (0..3).flat_map(|t| versiones(app, t)).collect();
    }
    app.servicios
        .instancias
        .versiones
        .iter()
        .map(|v| (v.id.clone(), crate::ficha::fecha(&v.release_time)))
        .collect()
}

pub fn accion_cambio(app: &mut App, accion: &pantallas::Accion) -> Option<Task<Mensaje>> {
    use pantallas::Accion as A;
    if app.servicios.instancias.cambio.is_none() {
        return None;
    }
    if app.servicios.instancias.operando
        && matches!(
            accion,
            A::CambiarActualizar(_)
                | A::CambiarModo(_)
                | A::CambiarLoader(_)
                | A::CambiarLoaderVersion(_)
                | A::CambiarTipo(_)
                | A::CambiarBuscar(_)
                | A::CambiarDestino(_)
                | A::CambiarConfirmar(_)
        )
    {
        return Some(Task::none());
    }
    if app.servicios.instancias.cambio.as_ref().is_some_and(|c|c.solo_loader) && matches!(accion,A::CambiarDestino(_)|A::CambiarTipo(_)|A::CambiarModo(_)|A::CambiarActualizar(_)|A::CambiarConfirmar(_)) {return Some(Task::none());}
    match accion {
        A::CambiarActualizar(b) => {
            let c = app.servicios.instancias.cambio.as_mut().unwrap();
            c.actualizar_mods = *b;
            c.revision = None;
            c.confirmando = false;
            c.revisando = false;
        }
        A::CambiarModo(b) => app.servicios.instancias.cambio.as_mut().unwrap().copia = *b,
        A::CambiarDetalle => {
            let c = app.servicios.instancias.cambio.as_mut().unwrap();
            c.detalle_desactivar = !c.detalle_desactivar;
        }
        A::CambiarLoader(_) => { return Some(Task::none()); }
        A::CambiarLoaderVersion(version) => {
            let c=app.servicios.instancias.cambio.as_mut().unwrap();
            if c.solo_loader {c.loader_version_destino=Some(version.clone());c.error=None;}
        }
        A::CambiarTipo(tipo) => {
            let c = app.servicios.instancias.cambio.as_mut().unwrap();
            c.tipo = (*tipo).min(2);
            c.revision = None;
            c.confirmando = false;
            c.revisando = false;
            app.servicios.instancias.cambio_offset = 0.;
            return Some(Task::batch([
                iced::widget::scrollable::snap_to(
                    iced::widget::scrollable::Id::new("version-cambio"),
                    iced::widget::scrollable::RelativeOffset::START,
                ),
                cargar_compatibilidad(app),
            ]));
        }
        A::CambiarBuscar(query) => {
            app.servicios.instancias.cambio.as_mut().unwrap().busqueda = query.clone();
            app.servicios.instancias.cambio_offset = 0.;
            return Some(Task::batch([
                iced::widget::scrollable::snap_to(
                    iced::widget::scrollable::Id::new("version-cambio"),
                    iced::widget::scrollable::RelativeOffset::START,
                ),
                cargar_compatibilidad(app),
            ]));
        }
        A::CambiarDestino(index) => {
            let c = app.servicios.instancias.cambio.as_ref().unwrap();
            let Some((destino, _)) = versiones_cambio(app, c.tipo).get(*index).cloned() else {
                return Some(Task::none());
            };
            let actual = app
                .servicios
                .instancias
                .registros
                .iter()
                .find(|i| i.id == c.instancia)
                .map(|i| i.minecraft.clone())
                .unwrap_or_default();
            if actual == destino {
                return Some(Task::none());
            }
            let down = icaro_core::versiones_instancia::baja(
                &app.servicios.instancias.versiones,
                &actual,
                &destino,
            );
            let c = app.servicios.instancias.cambio.as_mut().unwrap();
            c.destino = destino;
            c.destino_manual = true;
            c.revision = None;
            c.confirmando = false;
            c.revisando = false;
            c.error = None;
            if down {
                c.respaldo = true;
            }
            return Some(Task::batch([
                cargar_compatibilidad(app),
                cargar_loaders_cambio(app),
            ]));
        }
        A::VersionScroll(true, offset) => {
            app.servicios.instancias.cambio_offset = *offset;
            return Some(cargar_compatibilidad(app));
        }
        A::CambiarCancelar => {
            if !app.servicios.instancias.operando {
                if let Some(c) = app.servicios.instancias.cambio.take() {
                    c.cancelar_compatibilidad.cancel();
                }
                app.cambiar = None;
            }
        }
        A::CambiarConfirmar(_) => {
            let copia = app.servicios.instancias.cambio.as_ref().unwrap().copia;
            return Some(cambiar(app, copia));
        }
        _ => return None,
    }
    Some(Task::none())
}

pub fn cargar_compatibilidad(app: &mut App) -> Task<Mensaje> {
    if app.servicios.demo {
        return Task::none();
    }
    let Some(c) = &app.servicios.instancias.cambio else {
        return Task::none();
    };
    if c.solo_loader {return Task::none();}
    let Some(installed) = c.mods.clone() else {
        return Task::none();
    };
    let disponibles = versiones_cambio(app, c.tipo);
    let query = c.busqueda.trim().to_lowercase();
    let first = (app.servicios.instancias.cambio_offset / 64.).max(0.) as usize;
    let mut destinos: Vec<_> = disponibles
        .into_iter()
        .filter(|(v, _)| query.is_empty() || v.to_lowercase().contains(&query))
        .skip(first.saturating_sub(2))
        .take(14)
        .map(|(v, _)| v)
        .collect();
    if !c.destino.is_empty() && !destinos.contains(&c.destino) {
        destinos.insert(0, c.destino.clone());
    }
    let c = app.servicios.instancias.cambio.as_mut().unwrap();
    let slots = 4usize.saturating_sub(c.cargando_compatibilidad.len());
    let mut tasks = Vec::new();
    for mc in destinos {
        if tasks.len() == slots {
            break;
        }
        if c.compatibilidades.contains_key(&mc)
            || c.errores_compatibilidad.contains_key(&mc)
            || !c.cargando_compatibilidad.insert(mc.clone())
        {
            continue;
        }
        let id = c.instancia.clone();
        let generation = c.compat_generacion;
        let loader = c.loader_destino.clone();
        let installed = installed.clone();
        let cancel = c.cancelar_compatibilidad.clone();
        let dest = mc.clone();
        let target_loader = loader.clone();
        tasks.push(Task::perform(async move {
            tokio::select! {
                _ = cancel.cancelled() => Err("Consulta cancelada.".to_owned()),
                r = async {
                    let catalog = icaro_core::modrinth::Catalog::new().map_err(|e| e.to_string())?;
                    icaro_core::versiones_instancia::compatibilidad(&catalog, &installed, &dest, &target_loader).await
                } => r,
            }
        }, move |r| Mensaje::Instancias(Accion::Compatibilidad(id.clone(), generation, mc.clone(), loader.clone(), r))));
    }
    Task::batch(tasks)
}

pub fn cargar_loaders_cambio(app: &mut App) -> Task<Mensaje> {
    let Some(mc) = app
        .servicios
        .instancias
        .cambio
        .as_ref()
        .map(|c| c.destino.clone())
        .filter(|s| !s.is_empty())
    else {
        return Task::none();
    };
    cargar_loaders_version(app, mc)
}
pub fn elegida(app: &App) -> Option<String> {
    let s = app.asistente.as_ref()?;
    versiones(app, s.tipo_version)
        .get(s.version)
        .map(|v| v.0.clone())
}
pub fn abrir(app: &mut App) -> Task<Mensaje> {
    let s = &mut app.servicios.instancias;
    if !s.versiones.is_empty() || s.cargando {
        return Task::none();
    }
    s.request += 1;
    let request = s.request;
    s.cargando = true;
    s.error = None;
    let cache = crate::estado::directorio(false).join("cache/minecraft-versions.json");
    Task::perform(
        async move {
            manifest::fetch_manifest_cached(&cache)
                .await
                .map_err(|e| e.to_string())
        },
        move |r| Mensaje::Instancias(Accion::Versiones(request, r)),
    )
}
pub fn crear(app: &mut App) -> Task<Mensaje> {
    if app.servicios.instancias.creando {
        return Task::none();
    }
    let Some(version) = elegida(app) else {
        app.avisar("Selecciona una versión de Minecraft.".into());
        return Task::none();
    };
    let Some(s) = app.asistente.as_ref() else {
        return Task::none();
    };
    if s.paso != 3 {
        return Task::none();
    }
    let nombre = if s.nombre.trim().is_empty() {
        format!("{} {version}", pantallas::LOADERS[s.loader.min(4)].0)
    } else {
        s.nombre.trim().into()
    };
    let portada = s.portada.min(3) as u8;
    let loader = pantallas::LOADERS[s.loader.min(4)].0.to_lowercase();
    let loader_version = app.servicios.instancias.loader_elegida.clone().or_else(|| {
        app.servicios
            .instancias
            .loaders
            .get(&version)
            .and_then(|l| l.versiones[s.loader.min(4)].first())
            .cloned()
    });
    if loader != "vanilla" && loader_version.is_none() {
        app.avisar("Selecciona una versión disponible del loader.".into());
        return Task::none();
    }
    let store = app.servicios.instancias.almacen.clone();
    app.servicios.instancias.creando = true;
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                let record = store
                    .crear_con_loader(&nombre, &version, &loader, portada)
                    .map_err(|e| e.to_string())?;
                if let Some(v) = loader_version {
                    if let Err(e) =
                        icaro_core::versiones_instancia::guardar_loader(&store, &record, &v)
                    {
                        let _ = store.eliminar(&record.id);
                        return Err(e);
                    }
                }
                Ok::<_, String>(record)
            })
            .await
            .map_err(|_| "No se pudo crear la instancia.".to_owned())?
        },
        |r| Mensaje::Instancias(Accion::Creada(r)),
    )
}
pub fn actualizar(app: &mut App, action: Accion) -> Task<Mensaje> {
    match action {
        Accion::ModsCompatibles(id, generation, result) => {
            let Some(c) = app
                .servicios
                .instancias
                .cambio
                .as_mut()
                .filter(|c| c.instancia == id && c.compat_generacion == generation)
            else {
                return Task::none();
            };
            match result {
                Ok((mods, loader_version)) => {
                    c.mods = Some(mods);
                    c.loader_actual_version = loader_version;
                }
                Err(e) => c.error = Some(e),
            }
            return cargar_compatibilidad(app);
        }
        Accion::Compatibilidad(id, generation, mc, loader, result) => {
            let Some(c) = app.servicios.instancias.cambio.as_mut().filter(|c| {
                c.instancia == id && c.compat_generacion == generation && c.loader_destino == loader
            }) else {
                return Task::none();
            };
            c.cargando_compatibilidad.remove(&mc);
            match result {
                Ok(datos) => {
                    c.compatibilidades.insert(mc, datos);
                }
                Err(e) => {
                    c.errores_compatibilidad.insert(mc, e);
                }
            }
            let actual = app
                .servicios
                .instancias
                .registros
                .iter()
                .find(|i| i.id == id)
                .map(|i| i.minecraft.as_str())
                .unwrap_or_default();
            let recomendada = app
                .servicios
                .instancias
                .versiones
                .iter()
                .filter(|v| v.kind == "release" && v.id != actual)
                .find(|v| {
                    c.compatibilidades.get(&v.id).is_some_and(|d| {
                        d.total == 0 || d.compatibles.saturating_mul(100) / d.total >= 95
                    })
                })
                .map(|v| v.id.clone());
            if !c.destino_manual && !c.solo_loader {
                if let Some(destino) = recomendada {
                    c.destino = destino;
                }
            }
            return Task::batch([cargar_compatibilidad(app), cargar_loaders_cambio(app)]);
        }
        Accion::Loaders(mc, result) => {
            if app.servicios.instancias.cargando_loaders.as_ref() == Some(&mc) {
                app.servicios.instancias.cargando_loaders = None;
            }
            match result {
                Ok(l) => {
                    app.servicios.instancias.loaders.insert(mc, l);
                }
                Err(e) => app.avisar(e),
            }
        }
        Accion::Revision(id, generation, dest, loader, result) => {
            let bajando = app
                .servicios
                .instancias
                .registros
                .iter()
                .find(|i| i.id == id)
                .is_some_and(|i| {
                    icaro_core::versiones_instancia::baja(
                        &app.servicios.instancias.versiones,
                        &i.minecraft,
                        &dest,
                    )
                });
            if let Some(c) = app.servicios.instancias.cambio.as_mut().filter(|c| {
                c.instancia == id
                    && c.destino == dest
                    && c.compat_generacion == generation
                    && c.loader_destino == loader
            }) {
                c.revisando = false;
                match result {
                    Ok(mut r) => {
                        r.baja = bajando;
                        if bajando {
                            c.respaldo = true;
                        }
                        if !c.actualizar_mods {
                            r.desactivar = r.compatibilidad.desactivar_sin_actualizar.clone();
                            r.nombres = r.compatibilidad.nombres_sin_actualizar.clone();
                        }
                        c.confirmando = true;
                        c.revision = Some(r)
                    }
                    Err(e) => c.error = Some(e),
                }
            }
        }
        Accion::Cambiada(id, result) => {
            app.servicios.instancias.operando = false;
            match result {
                Ok(r) => {
                    let loader_cambiado=app.servicios.instancias.cambio.as_ref().filter(|c|c.solo_loader).and_then(|c|c.loader_version_destino.clone());
                    let record = r.instancia;
                    if r.copia {
                        let original = app
                            .servicios
                            .instancias
                            .registros
                            .iter()
                            .position(|i| i.id == id);
                        let grupo = original.and_then(|n| app.grupos.get(n)).cloned().flatten();
                        let config = original
                            .and_then(|n| app.configs.get(n))
                            .cloned()
                            .unwrap_or_else(crate::config_por_defecto);
                        crate::lista_instancias()
                            .lock()
                            .unwrap()
                            .push(Estado::datos(&app.servicios.instancias.almacen, &record));
                        app.servicios.instancias.registros.push(record.clone());
                        app.grupos.push(grupo);
                        app.configs.push(config);
                        app.editor_inst = app.servicios.instancias.registros.len() - 1;
                    } else if let Some(index) = app
                        .servicios
                        .instancias
                        .registros
                        .iter()
                        .position(|i| i.id == id)
                    {
                        app.servicios.instancias.registros[index] = record.clone();
                        let mut list = crate::lista_instancias().lock().unwrap();
                        if let Some(d) = list.get_mut(index) {
                            *d = Estado::datos(&app.servicios.instancias.almacen, &record);
                        }
                    }
                    app.cambiar = None;
                    if let Some(c) = app.servicios.instancias.cambio.take() {
                        c.cancelar_compatibilidad.cancel();
                    }
                    crate::mods_reales::recargar(app, &record.id);
                    app.servicios.mods.updates.remove(&record.id);
                    let backup = r
                        .respaldo
                        .map(|p| format!(" Respaldo guardado en {}.", p.display()))
                        .unwrap_or_default();
                    app.avisar(if let Some(version)=loader_cambiado {
                        format!("{} ahora usa {} {}.{}",record.nombre,crate::ficha::loader(&record.loader),version,backup)
                    } else {format!("{} ahora usa {}.{}",record.nombre,record.minecraft,backup)});
                    crate::arrancar(app);
                }
                Err(e) => {
                    if let Some(c) = &mut app.servicios.instancias.cambio {
                        c.error = Some(e);
                    }
                }
            }
        }
        Accion::Eliminada(id, result) => {
            app.servicios.instancias.operando = false;
            match result {
                Ok(()) => {
                    let Some(index) = app
                        .servicios
                        .instancias
                        .registros
                        .iter()
                        .position(|i| i.id == id)
                    else {
                        return Task::none();
                    };
                    let record = app.servicios.instancias.registros.remove(index);
                    let name = record.nombre.clone();
                    crate::lista_instancias().lock().unwrap().remove(index);
                    app.servicios.instancias.deshacer = Some(Eliminada {
                        registro: record,
                        grupo: app.grupos.remove(index),
                        config: app.configs.remove(index),
                    });
                    app.seleccionada = app.seleccionada.and_then(|i| {
                        if i == index {
                            None
                        } else {
                            Some(i - usize::from(i > index))
                        }
                    });
                    if app.editor_inst == index {
                        app.editor = None;
                    }
                    app.editor_inst = app
                        .editor_inst
                        .saturating_sub(usize::from(app.editor_inst > index));
                    app.servicios.instancias.eliminar = None;
                    app.dialogo = false;
                    app.menu = None;
                    app.busqueda.abierta = false;
                    app.picar = None;
                    app.picar_objetivo = None;
                    app.picar_revision = None;
                    app.colocada = None;
                    app.avisar(format!("{name} se movió a la papelera."));
                    crate::arrancar(app);
                }
                Err(error) => {
                    app.picar = None;
                    app.picar_objetivo = None;
                    app.picar_revision = None;
                    app.avisar(error);
                }
            }
        }
        Accion::Restaurada(id, result) => {
            app.servicios.instancias.operando = false;
            match result {
                Ok(record) => {
                    if !app
                        .servicios
                        .instancias
                        .deshacer
                        .as_ref()
                        .is_some_and(|i| i.registro.id == id)
                    {
                        return Task::none();
                    }
                    let previous = app.servicios.instancias.deshacer.take().unwrap();
                    let index = app
                        .servicios
                        .instancias
                        .registros
                        .partition_point(|i| (i.creada, &i.id) < (record.creada, &record.id));
                    if let Some(i) = &mut app.seleccionada {
                        *i += usize::from(*i >= index);
                    }
                    if app.editor.is_some() {
                        app.editor_inst += usize::from(app.editor_inst >= index);
                    }
                    crate::lista_instancias().lock().unwrap().insert(
                        index,
                        Estado::datos(&app.servicios.instancias.almacen, &record),
                    );
                    let name = record.nombre.clone();
                    app.servicios.instancias.registros.insert(index, record);
                    app.grupos.insert(index, previous.grupo);
                    app.configs.insert(index, previous.config);
                    app.menu = None;
                    app.busqueda.abierta = false;
                    app.colocada = Some((index, std::time::Instant::now()));
                    app.avisar(format!("Instancia {name} recuperada."));
                    crate::arrancar(app);
                }
                Err(error) => app.avisar(error),
            }
        }
        Accion::Versiones(id, result) if id == app.servicios.instancias.request => {
            app.servicios.instancias.cargando = false;
            match result {
                Ok(m) => {
                    app.servicios.instancias.versiones = m.versions;
                    app.servicios.instancias.error = None;
                    if let Some(s) = &mut app.asistente {
                        s.version = 0;
                    }
                }
                Err(e) => app.servicios.instancias.error = Some(e),
            }
            return cargar_compatibilidad(app);
        }
        Accion::Creada(result) => {
            app.servicios.instancias.creando = false;
            match result {
                Ok(i) => {
                    let name = i.nombre.clone();
                    crate::lista_instancias()
                        .lock()
                        .unwrap()
                        .push(Estado::datos(&app.servicios.instancias.almacen, &i));
                    app.servicios.instancias.registros.push(i);
                    app.grupos.push(None);
                    app.configs.push(crate::config_por_defecto());
                    app.asistente = None;
                    app.editor = None;
                    app.seccion = icaro_ui::shell::Seccion::Instancias;
                    app.avisar(format!("Instancia {name} creada y guardada."));
                    crate::arrancar(app);
                    let index = app.servicios.instancias.registros.len() - 1;
                    if icaro_core::minecraft::loader_instalable(
                        &app.servicios.instancias.registros[index].loader,
                    ) {
                        return crate::juego_real::iniciar(app, index, false);
                    }
                }
                Err(e) => app.avisar(e),
            }
        }
        _ => {}
    }
    Task::none()
}
pub fn renombrar(app: &mut App, nombre: &str) -> Result<(), String> {
    let Some(record) = app.servicios.instancias.registros.get(app.editor_inst) else {
        return Ok(());
    };
    if record.nombre == nombre.trim() {
        return Ok(());
    }
    if app.servicios.juego.ocupada(&record.id)
        || app.servicios.mods.busy.is_some()
        || app.servicios.instancias.operando
    {
        return Err("Detén el juego o espera a que termine la operación antes de renombrar la carpeta de esta instancia.".into());
    }
    let mut record = record.clone();
    record.nombre = nombre.trim().into();
    app.servicios
        .instancias
        .almacen
        .guardar(&record)
        .map_err(|e| e.to_string())?;
    if let Some(d) = crate::lista_instancias()
        .lock()
        .unwrap()
        .get_mut(app.editor_inst)
    {
        d.nombre = record.nombre.clone();
    }
    if let Ok(game) = app.servicios.instancias.almacen.juego(&record.id) {
        app.exp = crate::Explorador::desde_ruta(game);
    }
    app.servicios.instancias.registros[app.editor_inst] = record;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn selector() -> App {
        let (mut app, _) = crate::iniciar_con_guardado(
            icaro_ui::shell::Seccion::Instancias,
            false,
            None,
            false,
            crate::servicios::Estado::nuevo(true),
            None,
        );
        crate::pruebas_diseno::preparar_cambio(&mut app, "cambiar-version-seleccion");
        app
    }
    #[test]
    fn cambiar_solo_loader_bloquea_minecraft_y_familia_y_rechaza_version_ajena() {
        let mut app=selector();let minecraft=app.servicios.instancias.registros[0].minecraft.clone();
        let _=abrir_cambio_loader(&mut app,0);
        assert!(app.servicios.instancias.cambio.as_ref().unwrap().solo_loader);
        let _=pantallas::actualizar(&mut app,pantallas::Accion::CambiarDestino(0));
        let _=pantallas::actualizar(&mut app,pantallas::Accion::CambiarLoader("quilt".into()));
        let c=app.servicios.instancias.cambio.as_ref().unwrap();
        assert_eq!(c.destino,minecraft);assert_eq!(c.loader_destino,"fabric");assert!(!c.actualizar_mods);
        let _=pantallas::actualizar(&mut app,pantallas::Accion::CambiarLoaderVersion("version-no-publicada".into()));
        let _=aplicar_cambio_loader(&mut app);assert!(!app.servicios.instancias.operando);
        let _=pantallas::actualizar(&mut app,pantallas::Accion::CambiarCancelar);
        assert!(app.cambiar.is_none() && app.servicios.instancias.cambio.is_none());
    }

    #[test]
    fn late_results_cannot_replace_selected_destination_or_reopen_closed_selector() {
        let mut app = selector();
        let c = app.servicios.instancias.cambio.as_mut().unwrap();
        c.compatibilidades.clear();
        c.destino_manual = false;
        c.destino.clear();
        let id = c.instancia.clone();
        let generation = c.compat_generacion;
        let compatible = icaro_core::versiones_instancia::Compatibilidad {
            total: 100,
            compatibles: 96,
            ..Default::default()
        };
        let _ = actualizar(
            &mut app,
            Accion::Compatibilidad(
                id.clone(),
                generation,
                "1.21.5".into(),
                "fabric".into(),
                Ok(compatible.clone()),
            ),
        );
        assert_eq!(
            app.servicios.instancias.cambio.as_ref().unwrap().destino,
            "1.21.5"
        );
        let _ = pantallas::actualizar(&mut app, pantallas::Accion::CambiarDestino(7));
        assert_eq!(
            app.servicios.instancias.cambio.as_ref().unwrap().destino,
            "1.20.1"
        );
        let _ = actualizar(
            &mut app,
            Accion::Compatibilidad(
                id.clone(),
                generation,
                "26.3".into(),
                "fabric".into(),
                Ok(compatible.clone()),
            ),
        );
        assert_eq!(
            app.servicios.instancias.cambio.as_ref().unwrap().destino,
            "1.20.1"
        );
        let _ = pantallas::actualizar(&mut app, pantallas::Accion::CambiarLoader("quilt".into()));
        let _ = actualizar(
            &mut app,
            Accion::Compatibilidad(
                id.clone(),
                generation,
                "26.2".into(),
                "fabric".into(),
                Ok(compatible.clone()),
            ),
        );
        assert_eq!(app.servicios.instancias.cambio.as_ref().unwrap().loader_destino,"fabric");
        let token = app
            .servicios
            .instancias
            .cambio
            .as_ref()
            .unwrap()
            .cancelar_compatibilidad
            .clone();
        let _ = pantallas::actualizar(&mut app, pantallas::Accion::CambiarCancelar);
        assert!(token.is_cancelled());
        let _ = actualizar(
            &mut app,
            Accion::Compatibilidad(
                id,
                generation,
                "26.3".into(),
                "fabric".into(),
                Ok(compatible),
            ),
        );
        assert!(app.servicios.instancias.cambio.is_none() && app.cambiar.is_none());
    }
    #[test]
    fn review_from_another_loader_cannot_change_fixed_family() {
        let mut app = selector();
        let c = app.servicios.instancias.cambio.as_mut().unwrap();
        let id = c.instancia.clone();
        let generation = c.compat_generacion;
        let dest = c.destino.clone();
        c.revisando = true;
        let _ = pantallas::actualizar(&mut app, pantallas::Accion::CambiarLoader("quilt".into()));
        let review = icaro_core::versiones_instancia::Revision {
            mundos: 0,
            desactivar: vec![],
            nombres: vec![],
            baja: false,
            loader_version: Some("0.16.14".into()),
            loader: "quilt".into(),
            compatibilidad: Default::default(),
        };
        let _ = actualizar(
            &mut app,
            Accion::Revision(id, generation, dest, "quilt".into(), Ok(review)),
        );
        let c = app.servicios.instancias.cambio.as_ref().unwrap();
        assert!(c.revision.is_none() && !c.confirmando && c.revisando);
        assert_eq!(c.loader_destino,"fabric");
    }
}
