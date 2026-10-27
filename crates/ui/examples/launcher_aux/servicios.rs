// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Servicios reales del launcher; ICARO_DEMO=1 conserva la maqueta sin red.

use crate::{catalogo, ficha, pantallas, App, Mensaje};
use icaro_core::{
    auth::{self, CancellationToken, Session},
    modrinth::{
        self, Catalog, Kind, Member, Order, Organization, Project, Results, Search, Tags, Version,
    },
    vault::Vault,
};
use icaro_ui::{
    componentes::{self, Variante},
    cuenta::{self, AccionSelector, CuentaSelector, EstadoSesion},
    fuentes,
    iconos::Icono,
    shell::Seccion,
    tema::{espacio, texto},
};
use iced::futures::SinkExt;
use iced::futures::StreamExt;
use iced::widget::{column, container, row, text, Space};
use iced::{Element, Length, Padding, Task};
use std::sync::Arc;
use std::time::Duration;

pub struct Estado {
    pub sincronizacion:crate::sincronizacion_real::Estado,
    pub red_privada:crate::red_privada_real::Estado,
    pub servidores:crate::servidores_reales::Estado,
    pub modpacks:crate::modpacks_reales::Estado,
    pub juego: crate::juego_real::Estado,
    pub mods: crate::mods_reales::Estado,
    anteriores: Vec<PaginaAnterior>,
    pub dependencias: std::collections::BTreeMap<String, Result<(String, String), String>>,
    pub instancias: crate::instancias_reales::Estado,
    pub demo: bool,
    pub session: Option<Arc<Session>>,
    perfil_recordado: Option<String>,
    pub local: crate::cuenta_sin_conexion::Estado,
    vault: Arc<Vault>,
    pub(crate) catalog: Result<Catalog, modrinth::Error>,
    pub(crate) auth_id: u64,
    auth_cancel: Option<CancellationToken>,
    pub auth_busy: bool,
    pub restoring: bool,
    progress: Option<FaseAcceso>,
    auth_error: Option<String>,
    remembered: bool,
    pub query: String,
    search_id: u64,
    search_cancel: Option<CancellationToken>,
    filter: Option<Search>,
    pub results: Option<Results>,
    pub detail: Option<(Project, Vec<Version>)>,
    busy: bool,
    pub error: Option<String>,
    pub ficha: ficha::Estado,
    pub(crate) minecraft: String,
    pub(crate) loader: String,
    pub order: Order,
    pub(crate) tags: Option<Tags>,
    tags_busy: bool,
    tags_error: Option<String>,
    pub images: catalogo::Imagenes,
    pub document: catalogo::Documento,
    media_slots: Arc<tokio::sync::Semaphore>,
}

impl Default for Estado {
    fn default() -> Self {
        Self::nuevo(std::env::var("ICARO_DEMO").as_deref() == Ok("1"))
    }
}
impl Estado {
    pub fn nuevo(demo: bool) -> Self {
        Self {
            sincronizacion:Default::default(),
            red_privada:Default::default(),
            servidores:Default::default(),
            modpacks:Default::default(),
            juego: Default::default(),
            mods: Default::default(),
            anteriores: vec![],
            dependencias: Default::default(),
            demo,
            session: None,
            perfil_recordado: None,
            local: Default::default(),
            vault: Arc::new(Vault::default()),
            catalog: Catalog::new().map(|catalog| {
                if demo { catalog.with_metadata_cache() } else {
                    catalog.with_persistent_metadata_cache(crate::estado::directorio(false).join("cache/modrinth-metadatos.json"))
                }
            }),
            auth_id: 0,
            auth_cancel: None,
            auth_busy: false,
            restoring: !demo,
            instancias: Default::default(),
            progress: None,
            auth_error: None,
            remembered: false,
            query: String::new(),
            search_id: 0,
            search_cancel: None,
            filter: None,
            results: None,
            detail: None,
            busy: false,
            error: None,
            ficha: Default::default(),
            minecraft: "Todas las versiones".into(),
            loader: "Todos los loaders".into(),
            order: Order::Relevance,
            tags: None,
            tags_busy: false,
            tags_error: None,
            images: Default::default(),
            document: Default::default(),
            media_slots: Arc::new(tokio::sync::Semaphore::new(4)),
        }
    }
}
impl Estado {
    pub fn hay_anterior(&self) -> bool {
        !self.anteriores.is_empty()
    }
    pub fn hay_catalogo(&self) -> bool {
        self.results.is_some()
    }
    pub fn nombre<'a>(&'a self, demo_name: &'a str) -> &'a str {
        if let Some(sesion) = self.session.as_ref().filter(|s| s.es_local()) {
            return &sesion.profile.name;
        }
        if self.demo {
            demo_name
        } else if self.restoring {
            self.perfil_recordado.as_deref().unwrap_or("Recuperando sesión…")
        } else {
            self.session
                .as_ref()
                .map(|s| s.profile.name.as_str())
                .or(self.perfil_recordado.as_deref())
                .unwrap_or("Sin cuenta")
        }
    }
    pub fn cancel(&mut self) {
        if let Some(cancel) = self.auth_cancel.take() {
            cancel.cancel();
        }
        self.auth_id = self.vault.invalidate();
        self.perfil_recordado = None;
        self.auth_busy = false;
        self.restoring = false;
        self.progress = None;
    }
    pub fn reset_catalog(&mut self) {
        self.anteriores.clear();
        if let Some(cancel) = self.search_cancel.take() {
            cancel.cancel();
        }
        self.search_id += 1;
        self.results = None;
        self.detail = None;
        self.ficha = Default::default();
        self.filter = None;
        self.error = None;
        self.busy = false;
        self.query.clear();
    }
}
struct PaginaAnterior {
    panel: Option<String>,
    mods_vista: usize,
    detail: (Project, Vec<Version>),
    ficha: ficha::Estado,
    documento: catalogo::Documento,
}
impl Drop for Estado {
    fn drop(&mut self) {
        self.cancel();
        if let Some(cancel) = &self.search_cancel {
            cancel.cancel();
        }
    }
}

#[derive(Debug, Clone)]
pub enum Accion {
    Local(crate::cuenta_sin_conexion::Accion),
    Dependencias(u64, Vec<(String, Result<(String, String), String>)>),
    ReintentarDependencias,
    Inicializar,
    Login,
    LoginBrowser,
    CancelarLogin,
    ContinuarInvitado,
    Logout,
    Renovar,
    Progreso(u64, FaseAcceso),
    PerfilRecordado(u64, String),
    Autenticada(u64, Result<Option<Arc<Session>>, String>, bool),
    Cerrada(u64, Result<(), String>),
    Consulta(String),
    Buscar,
    Pagina(u32),
    Buscada(u64, Result<Results, String>),
    Proyecto(String),
    ProyectoVersiones(String),
    ProyectoRecibido(u64, Result<(Project, Vec<Version>), String>),
    Volver,
    CerrarPanel,
    FichaPestana(ficha::Pestana),
    Galeria(usize),
    VisorAbrir(usize),
    VisorPaso(i32),
    VisorZoom(bool),
    VisorCerrar,
    VisorRecibida(u64, u64, String, Result<catalogo::Imagen, String>),
    CargarMiembros,
    MiembrosRecibidos(u64, Result<Vec<Member>, String>),
    OrganizacionRecibida(u64, Result<Organization, String>),
    CargarRelacionados,
    Relacionados(u64, Result<Vec<modrinth::RelatedProject>, String>),
    VersionMinecraft(String),
    VersionLoader(String),
    VersionPruebas(bool),
    VersionPagina(usize),
    CargarVersiones,
    VersionesRecibidas(u64, u64, Result<Vec<Version>, String>),
    CambiosVersion(String),
    CambiosSeleccionar(String),
    CambiosRecibidos(u64, u64, Result<Version, String>),
    InfoSnapshots(bool),
    InfoVersiones,
    MostrarTodas(bool),
    Minecraft(String),
    Loader(String),
    Orden(Order),
    LimpiarFiltros,
    Tags(Result<Tags, String>),
    CargarTags,
    Imagen(String, Option<catalogo::Imagen>),
    Enlace(String),
    ErrorEnlace(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaseAcceso {
    Autenticacion(auth::Stage),
    Guardando,
}
impl FaseAcceso {
    fn texto(self) -> &'static str {
        match self {
            Self::Autenticacion(auth::Stage::WaitingForBrowser) => {
                "Completa el acceso en Microsoft. Estamos esperando tu autorización."
            }
            Self::Autenticacion(auth::Stage::Microsoft) => {
                "Autorización recibida. Obteniendo tu sesión…"
            }
            Self::Autenticacion(auth::Stage::Xbox) => "Validando tu cuenta de Xbox…",
            Self::Autenticacion(auth::Stage::Minecraft) => "Preparando el acceso a Minecraft…",
            Self::Autenticacion(auth::Stage::Ownership) => {
                "Comprobando tu acceso a Minecraft Java…"
            }
            Self::Autenticacion(auth::Stage::Profile) => "Obteniendo los datos de tu perfil…",
            Self::Guardando => "Guardando tu sesión de forma segura…",
        }
    }
    fn paso(self) -> Option<usize> {
        match self {
            Self::Autenticacion(auth::Stage::WaitingForBrowser) => None,
            Self::Autenticacion(auth::Stage::Microsoft | auth::Stage::Xbox) => Some(0),
            Self::Autenticacion(auth::Stage::Minecraft | auth::Stage::Ownership) => Some(1),
            Self::Autenticacion(auth::Stage::Profile) => Some(2),
            Self::Guardando => Some(3),
        }
    }
}
fn m(action: Accion) -> Mensaje {
    Mensaje::Servicio(action)
}

async fn save(
    vault: Arc<Vault>,
    id: u64,
    session: Arc<Session>,
) -> (Result<Option<Arc<Session>>, String>, bool) {
    let copy = session.clone();
    let stored = tokio::task::spawn_blocking(move || {
        vault.remember(id, &copy)?;
        icaro_core::cuentas_locales::Cuentas::desactivar(&crate::estado::directorio(false))
            .map_err(|_| icaro_core::vault::Error::Unavailable)
    }).await;
    match stored {
        Ok(Ok(())) => (Ok(Some(session)), true),
        Ok(Err(icaro_core::vault::Error::Stale)) => {
            (Err("Inicio de sesión cancelado.".into()), false)
        }
        _ => (Ok(Some(session)), false),
    }
}

pub(crate) fn begin_auth(app: &mut App, restore: bool) -> Task<Mensaje> {
    if app.servicios.auth_busy {
        return Task::none();
    }
    app.servicios.cancel();
    let id = app.servicios.auth_id;
    let cancel = CancellationToken::new();
    app.servicios.auth_cancel = Some(cancel.clone());
    app.servicios.auth_busy = true;
    app.servicios.restoring = restore;
    app.servicios.auth_error = None;
    let vault = app.servicios.vault.clone();
    let previous = app.servicios.session.clone();
    Task::run(
        iced::stream::channel(16, move |mut output| async move {
            let report = output.clone();
            let progress = move |stage| {
                let _ = report
                    .clone()
                    .try_send(Accion::Progreso(id, FaseAcceso::Autenticacion(stage)));
            };
            if restore {
                let locales = tokio::task::spawn_blocking(|| icaro_core::cuentas_locales::Cuentas::cargar(&crate::estado::directorio(false))).await;
                match locales {
                    Ok(Ok(cuentas)) => {
                        let activa = cuentas.activa.clone();
                        let _ = output.send(Accion::Local(crate::cuenta_sin_conexion::Accion::Cargadas(Ok(cuentas)))).await;
                        if let Some(nombre) = activa {
                            let r = Session::local(&nombre).map(|s| Some(Arc::new(s)));
                            let _ = output.send(Accion::Autenticada(id, r, true)).await;
                            return;
                        }
                    }
                    _ => {
                        let _ = output.send(Accion::Local(crate::cuenta_sin_conexion::Accion::Cargadas(Err("No se pudieron recuperar las cuentas sin conexión; el archivo original se conserva.".into())))).await;
                    }
                }
            }
            let result = if restore {
                let read_vault = vault.clone();
                match tokio::task::spawn_blocking(move || read_vault.load()).await {
                    Ok(Ok(Some(record))) => {
                        if let Some(nombre) = record.profile_name {
                            let _ = output.send(Accion::PerfilRecordado(id, nombre)).await;
                        }
                        match auth::refresh_with_progress(record.refresh, cancel, &progress).await {
                            Ok(session) if session.profile.id == record.profile_id => {
                                Ok(Some(session))
                            }
                            Ok(_) => Err(auth::Error::Callback),
                            Err(error) => Err(error),
                        }
                    }
                    Ok(Ok(None)) => Ok(None),
                    _ => {
                        let _ = output.send(Accion::Autenticada(id, Err("No se pudo recuperar la sesión guardada. Puedes iniciar sesión de nuevo.".into()), false)).await;
                        return;
                    }
                }
            } else if let Some(session) = previous {
                auth::renew_with_progress(&session, cancel, &progress)
                    .await
                    .map(Some)
            } else {
                Ok(None)
            };
            let (result, stored) = match result {
                Ok(Some(session)) => {
                    let _ = output
                        .send(Accion::Progreso(id, FaseAcceso::Guardando))
                        .await;
                    save(vault, id, Arc::new(session)).await
                }
                Ok(None) => (Ok(None), false),
                Err(error) => (Err(error.to_string()), false),
            };
            let _ = output.send(Accion::Autenticada(id, result, stored)).await;
        }),
        m,
    )
}

fn begin_browser(app: &mut App) -> Task<Mensaje> {
    if app.servicios.auth_busy {
        return Task::none();
    }
    app.servicios.cancel();
    let id = app.servicios.auth_id;
    let cancel = CancellationToken::new();
    app.servicios.auth_cancel = Some(cancel.clone());
    app.servicios.auth_busy = true;
    app.servicios.progress = Some(FaseAcceso::Autenticacion(auth::Stage::WaitingForBrowser));
    app.servicios.auth_error = None;
    app.sesion.menu = false;
    let vault = app.servicios.vault.clone();
    Task::run(
        iced::stream::channel(16, move |mut output| async move {
            let report = output.clone();
            let progress = move |stage| {
                let _ = report
                    .clone()
                    .try_send(Accion::Progreso(id, FaseAcceso::Autenticacion(stage)));
            };
            let result = auth::login_with_progress(cancel, auth::ReturnMethod::default(), progress).await;
            let (result, stored) = match result {
                Ok(session) => {
                    let _ = output
                        .send(Accion::Progreso(id, FaseAcceso::Guardando))
                        .await;
                    save(vault, id, Arc::new(session)).await
                }
                Err(error) => (Err(error.to_string()), false),
            };
            let _ = output.send(Accion::Autenticada(id, result, stored)).await;
        }),
        m,
    )
}
fn context(app: &App) -> Search {
    let kind = if app.seccion == Seccion::Modpacks && app.editor.is_none() {
        Kind::Modpack
    } else if app.editor == Some(3) {
        Kind::Resourcepack
    } else if app.editor == Some(8) {
        Kind::Shader
    } else {
        Kind::Mod
    };
    let (minecraft, loader) = if kind == Kind::Modpack {
        (
            (app.servicios.minecraft != "Todas las versiones")
                .then(|| app.servicios.minecraft.clone()),
            (app.servicios.loader != "Todos los loaders").then(|| app.servicios.loader.clone()),
        )
    } else {
        let (mc, loader) = crate::juego_de_instancia(app);
        (
            matches!(kind, Kind::Mod | Kind::Shader).then_some(mc),
            (kind == Kind::Mod && !loader.eq_ignore_ascii_case("vanilla"))
                .then(|| loader.to_lowercase()),
        )
    };
    Search {
        query: app.servicios.query.clone(),
        kind,
        minecraft,
        loader,
        offset: 0,
        order: app.servicios.order,
    }
}

pub fn buscar(app: &mut App, offset: u32, debounce: bool) -> Task<Mensaje> {
    if app.servicios.demo {
        return Task::none();
    }
    if let Some(cancel) = app.servicios.search_cancel.take() {
        cancel.cancel();
    }
    app.servicios.search_id += 1;
    let id = app.servicios.search_id;
    let cancel = CancellationToken::new();
    app.servicios.search_cancel = Some(cancel.clone());
    let mut filter = context(app);
    filter.offset = offset;
    app.servicios.filter = Some(filter.clone());
    app.servicios.results = None;
    app.servicios.detail = None;
    app.servicios.ficha = Default::default();
    app.servicios.busy = true;
    app.servicios.error = None;
    let catalog = app.servicios.catalog.clone();
    let categories = if filter.kind == Kind::Modpack || matches!(app.editor, Some(2 | 3 | 8)) {
        app.tienda_cat.clone()
    } else {
        vec![]
    };
    let environment = if app.editor == Some(2) {
        match app.tienda_lado {
            0 => Some("client_and_server"),
            1 => Some("client_only"),
            _ => None,
        }
    } else {
        None
    };
    let tags = cargar_tags(app);
    let search = Task::perform(
        async move {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => Err("Búsqueda cancelada.".into()),
                result = async {
                    if debounce { tokio::time::sleep(Duration::from_millis(350)).await; }
                    catalog?.search_filtered(&filter, &categories, environment).await
                } => result.map_err(|e| e.to_string()),
            }
        },
        move |result| m(Accion::Buscada(id, result)),
    );
    Task::batch([tags, search])
}

fn cargar_tags(app: &mut App) -> Task<Mensaje> {
    if app.servicios.tags.is_some() || app.servicios.tags_busy {
        return Task::none();
    }
    app.servicios.tags_busy = true;
    app.servicios.tags_error = None;
    let catalog = app.servicios.catalog.clone();
    Task::perform(
        async move {
            match catalog {
                Ok(catalog) => catalog.tags().await.map_err(|e| e.to_string()),
                Err(error) => Err(error.to_string()),
            }
        },
        |r| m(Accion::Tags(r)),
    )
}

pub fn cargar_imagenes(app: &mut App, urls: Vec<String>) -> Task<Mensaje> {
    let urls = app.servicios.images.pedir(urls);
    let catalog = app.servicios.catalog.clone();
    let slots = app.servicios.media_slots.clone();
    let stream = iced::futures::stream::iter(urls)
        .map(move |url| {
            let catalog = catalog.clone();
            let slots = slots.clone();
            async move {
                let image = async {
                    let _permit = slots.acquire_owned().await.ok()?;
                    let bytes = catalog.ok()?.image(&url).await.ok()?;
                    tokio::task::spawn_blocking(move || catalogo::decodificar(bytes))
                        .await
                        .ok()?
                }
                .await;
                Accion::Imagen(url, image)
            }
        })
        .buffer_unordered(4);
    Task::run(stream, m)
}

fn imagenes_visibles(app: &App) -> Vec<String> {
    if app.mod_panel.is_some() {
        if let Some((p, _)) = &app.servicios.detail {
            return p
                .icon_url
                .iter()
                .cloned()
                .chain(p.gallery.iter().take(3).map(|g| g.url.clone()))
                .collect();
        }
    }
    if let Some((project, _)) = &app.servicios.detail {
        app.servicios
            .ficha
            .imagenes(project, &app.servicios.document)
    } else {
        app.servicios.results.as_ref().map_or_else(Vec::new, |r| {
            r.hits.iter().filter_map(|h| h.icon_url.clone()).collect()
        })
    }
}

fn cargar_miembros(app: &mut App) -> Task<Mensaje> {
    let Some((project, _)) = &app.servicios.detail else {
        return Task::none();
    };
    if app.servicios.ficha.miembros_busy {
        return Task::none();
    }
    let id = project.id.clone();
    let organization = project.organization.clone();
    let generation = app.servicios.search_id;
    let catalog = app.servicios.catalog.clone();
    app.servicios.ficha.miembros_busy = true;
    app.servicios.ficha.miembros_error = None;
    let organization_task = if let Some(id) = organization {
        let catalog = catalog.clone();
        app.servicios.ficha.organizacion_busy = true;
        app.servicios.ficha.organizacion_error = None;
        Task::perform(
            async move {
                async { catalog?.organization(&id).await }
                    .await
                    .map_err(|e| e.to_string())
            },
            move |r| m(Accion::OrganizacionRecibida(generation, r)),
        )
    } else {
        Task::none()
    };
    let members_task = Task::perform(
        async move {
            let result = async { catalog?.members(&id).await }.await;
            result.map_err(|e| e.to_string())
        },
        move |r| m(Accion::MiembrosRecibidos(generation, r)),
    );
    Task::batch([members_task, organization_task])
}
fn cargar_relacionados(app:&mut App)->Task<Mensaje> {
    let Some((project,_))=&app.servicios.detail else{return Task::none();};
    if app.servicios.ficha.relacionados_busy{return Task::none();}
    let owner=project.organization.as_ref().map(|id|(id.clone(),true)).or_else(||app.servicios.ficha.miembros.iter().find(|m|m.is_owner).map(|m|(m.user.id.clone(),false)));
    let Some((owner,organization))=owner else{return Task::none();};
    let id=app.servicios.search_id;let catalog=app.servicios.catalog.clone();
    app.servicios.ficha.relacionados_busy=true;app.servicios.ficha.relacionados_error=None;
    Task::perform(async move{async{catalog?.creator_projects(&owner,organization).await}.await.map_err(|e|e.to_string())},move |r|m(Accion::Relacionados(id,r)))
}

fn cargar_versiones(app: &mut App) -> Task<Mensaje> {
    let bound = (app.editor.is_some() && !app.servicios.ficha.mostrar_todas).then(|| context(app));
    let Some((project, _)) = &app.servicios.detail else {
        return Task::none();
    };
    let id = project.id.clone();
    let resource = project.project_type == "resourcepack";
    let generation = app.servicios.search_id;
    let catalog = app.servicios.catalog.clone();
    let s = &mut app.servicios.ficha;
    if let Some(c) = s.versiones_cancel.take() {
        c.cancel();
    }
    s.versiones_id += 1;
    let request = s.versiones_id;
    let cancel = CancellationToken::new();
    s.versiones_cancel = Some(cancel.clone());
    s.versiones_busy = true;
    s.versiones_error = None;
    s.pagina = 0;
    s.seleccionada = None;
    s.limpiar_cambios();
    let minecraft = (!resource)
        .then(|| {
            bound.as_ref().map_or_else(
                || (s.minecraft != "Todas las versiones").then(|| s.minecraft.clone()),
                |b| b.minecraft.clone(),
            )
        })
        .flatten();
    let loader = (!matches!(project.project_type.as_str(), "resourcepack" | "shader"))
        .then(|| {
            bound.as_ref().map_or_else(
                || (s.loader != "Todos los loaders").then(|| s.loader.clone()),
                |b| b.loader.clone(),
            )
        })
        .flatten();
    Task::perform(
        async move {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => Err("Consulta cancelada.".into()),
                result = async { catalog?.versions(&id, minecraft.as_deref(), loader.as_deref()).await } => result.map_err(|e| e.to_string()),
            }
        },
        move |r| m(Accion::VersionesRecibidas(generation, request, r)),
    )
}

fn cargar_dependencias(app: &App) -> Task<Mensaje> {
    let s = &app.servicios;
    let Some((_, versions)) = &s.detail else {
        return Task::none();
    };
    let Some(v) = versions
        .iter()
        .find(|v| Some(&v.id) == s.ficha.seleccionada.as_ref())
    else {
        return Task::none();
    };
    let deps: Vec<_> = v
        .dependencies
        .iter()
        .take(128)
        .filter(|d| !s.dependencias.contains_key(&clave_dependencia(d)))
        .cloned()
        .collect();
    if deps.is_empty() {
        return Task::none();
    }
    let catalog = s.catalog.clone();
    let generation = s.search_id;
    Task::perform(
        async move {
            let mut result = vec![];
            for dep in deps {
                let key = clave_dependencia(&dep);
                let value = async {
                    let catalog = catalog.as_ref().map_err(|e| e.to_string())?;
                    let id = if let Some(p) = dep.project_id {
                        p
                    } else if let Some(v) = dep.version_id {
                        catalog
                            .version(&v)
                            .await
                            .map_err(|e| e.to_string())?
                            .project_id
                    } else {
                        return Err("Dependencia externa".into());
                    };
                    let project = catalog.project(&id).await.map_err(|e| e.to_string())?;
                    Ok((project.id, project.title))
                }
                .await;
                result.push((key, value));
            }
            result
        },
        move |r| m(Accion::Dependencias(generation, r)),
    )
}
pub fn clave_dependencia(dep: &modrinth::Dependency) -> String {
    if let Some(p) = &dep.project_id {
        format!("p:{p}")
    } else if let Some(v) = &dep.version_id {
        format!("v:{v}")
    } else {
        format!("f:{}", dep.file_name.as_deref().unwrap_or("externa"))
    }
}
fn cargar_cambios(app: &mut App) -> Task<Mensaje> {
    let Some((project, _)) = &app.servicios.detail else {
        return Task::none();
    };
    let project_id = project.id.clone();
    let generation = app.servicios.search_id;
    let catalog = app.servicios.catalog.clone();
    let s = &mut app.servicios.ficha;
    let Some(id) = s.seleccionada.clone() else {
        return Task::none();
    };
    if s.cambios_busy || s.cambio.as_ref().is_some_and(|v| v.id == id) {
        return Task::none();
    }
    s.cambios_id += 1;
    let request = s.cambios_id;
    let cancel = CancellationToken::new();
    s.cambios_cancel = Some(cancel.clone());
    s.cambios_busy = true;
    s.cambios_error = None;
    Task::perform(
        async move {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => Err("Consulta cancelada.".into()),
                result = async {
                    let version = catalog?.version(&id).await?;
                    if version.project_id != project_id || version.id != id { return Err(modrinth::Error::Response("los cambios")); }
                    Ok(version)
                } => result.map_err(|e| e.to_string()),
            }
        },
        move |r| m(Accion::CambiosRecibidos(generation, request, r)),
    )
}

fn cargar_visor(app: &mut App) -> Task<Mensaje> {
    let Some((project, _)) = &app.servicios.detail else {
        return Task::none();
    };
    let Some(visor) = &app.servicios.ficha.visor else {
        return Task::none();
    };
    let Some(image) = project.gallery.get(visor.indice) else {
        return Task::none();
    };
    let url = image.original_url().to_owned();
    let generation = app.servicios.search_id;
    let catalog = app.servicios.catalog.clone();
    let slots = app.servicios.media_slots.clone();
    let s = &mut app.servicios.ficha;
    if let Some(c) = s.visor_cancel.take() {
        c.cancel();
    }
    s.visor_id += 1;
    let request = s.visor_id;
    let cancel = CancellationToken::new();
    s.visor_cancel = Some(cancel.clone());
    s.visor_busy = true;
    s.visor_error = None;
    s.visor_imagen = None;
    let result_url = url.clone();
    Task::perform(
        async move {
            tokio::select! {
                biased;
                _ = cancel.cancelled() => Err("Carga cancelada.".into()),
                result = async {
                    let _permit = slots.acquire_owned().await.map_err(|_| "No se pudo cargar la imagen.".to_owned())?;
                    let bytes = catalog.map_err(|e| e.to_string())?.gallery_image(&url).await.map_err(|e| e.to_string())?;
                    tokio::task::spawn_blocking(move || catalogo::decodificar_visor(bytes)).await
                        .map_err(|_| "No se pudo preparar la imagen.".to_owned())?
                        .ok_or_else(|| "El formato o tamaño de esta imagen no se admite.".to_owned())
                } => result,
            }
        },
        move |r| {
            m(Accion::VisorRecibida(
                generation,
                request,
                result_url.clone(),
                r,
            ))
        },
    )
}

pub fn capa<'a>(app: &'a App, ventana: Element<'a, Mensaje>) -> Element<'a, Mensaje> {
    if let Some((project, _)) = &app.servicios.detail {
        if let Some(visor) = ficha::visor(app, project) {
            return crate::con_barra_ventana(app, visor);
        }
    }
    ventana
}

pub fn suscripcion(app: &App) -> iced::Subscription<Mensaje> {
    if app.servicios.ficha.visor.is_none() {
        if app.mod_panel.is_some() && !app.busqueda.abierta && !crate::hay_dialogo(app)
            && app.capt.visor.is_none() && app.umbral.is_none() {
            return iced::keyboard::on_key_press(|key, _| {
                (key == iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape))
                    .then_some(Mensaje::PanelMod(None))
            });
        }
        return iced::Subscription::none();
    }
    iced::keyboard::on_key_press(|key, _| {
        use iced::keyboard::{key::Named, Key};
        let action = match key.as_ref() {
            Key::Named(Named::Escape) => Accion::VisorCerrar,
            Key::Named(Named::ArrowLeft) => Accion::VisorPaso(-1),
            Key::Named(Named::ArrowRight) => Accion::VisorPaso(1),
            Key::Character("+" | "=") => Accion::VisorZoom(true),
            Key::Character("-") => Accion::VisorZoom(false),
            _ => return None,
        };
        Some(m(action))
    })
}

pub fn actualizar(app: &mut App, action: Accion) -> Task<Mensaje> {
    if app.servicios.demo {
        return Task::none();
    }
    match action {
        Accion::Local(a) => return crate::cuenta_sin_conexion::actualizar(app, a),
        Accion::ReintentarDependencias => {
            app.servicios.dependencias.retain(|_, value| value.is_ok());
            return cargar_dependencias(app);
        }
        Accion::Dependencias(id, values) if id == app.servicios.search_id => {
            app.servicios.dependencias.extend(values);
        }
        Accion::VisorAbrir(i) => {
            if app
                .servicios
                .detail
                .as_ref()
                .is_none_or(|(p, _)| i >= p.gallery.len())
            {
                return Task::none();
            }
            app.servicios.ficha.visor = Some(icaro_ui::visor::EstadoVisor::nuevo(i, app.ahora));
            app.servicios.ficha.galeria = i;
            app.busqueda.abierta = false;
            app.sesion.menu = false;
            return cargar_visor(app);
        }
        Accion::VisorPaso(delta) => {
            let total = app
                .servicios
                .detail
                .as_ref()
                .map_or(0, |(p, _)| p.gallery.len());
            if total == 0 {
                return Task::none();
            }
            if let Some(v) = &mut app.servicios.ficha.visor {
                let indice = (v.indice as i64 + i64::from(delta)).rem_euclid(total as i64) as usize;
                v.ir_a(indice);
                app.servicios.ficha.galeria = indice;
                let full = cargar_visor(app);
                let miniaturas = cargar_imagenes(app, imagenes_visibles(app));
                return Task::batch([full, miniaturas]);
            }
        }
        Accion::VisorZoom(acercar) => {
            if let Some(v) = &mut app.servicios.ficha.visor {
                if acercar {
                    v.acercar();
                } else {
                    v.alejar();
                }
            }
        }
        Accion::VisorCerrar => {
            let s = &mut app.servicios.ficha;
            if let Some(c) = s.visor_cancel.take() {
                c.cancel();
            }
            s.visor_id += 1;
            s.visor = None;
            s.visor_imagen = None;
            s.visor_busy = false;
            s.visor_error = None;
        }
        Accion::VisorRecibida(generation, request, url, result)
            if generation == app.servicios.search_id
                && request == app.servicios.ficha.visor_id
                && app.servicios.ficha.visor.is_some() =>
        {
            let s = &mut app.servicios.ficha;
            s.visor_busy = false;
            s.visor_cancel = None;
            match result {
                Ok(image) => s.visor_imagen = Some((url, image)),
                Err(error) => s.visor_error = Some(error),
            }
        }
        Accion::OrganizacionRecibida(id, result)
            if id == app.servicios.search_id && app.servicios.detail.is_some() =>
        {
            let s = &mut app.servicios.ficha;
            s.organizacion_busy = false;
            match result {
                Ok(org) => {
                    let urls = org.icon_url.iter().cloned().collect();
                    s.organizacion = Some(org);
                    return Task::batch([cargar_imagenes(app, urls),cargar_relacionados(app)]);
                }
                Err(error) => s.organizacion_error = Some(error),
            }
        }
        Accion::InfoSnapshots(value) => app.servicios.ficha.info_snapshots=value,
        Accion::MostrarTodas(value) => {
            let instancia=context(app);
            app.servicios.ficha.mostrar_todas=value;
            if value {app.servicios.ficha.minecraft="Todas las versiones".into();app.servicios.ficha.loader="Todos los loaders".into();}
            else {app.servicios.ficha.minecraft=instancia.minecraft.unwrap_or_else(||"Todas las versiones".into());app.servicios.ficha.loader=instancia.loader.unwrap_or_else(||"Todos los loaders".into());}
            return cargar_versiones(app);
        }
        Accion::InfoVersiones => {
            let page = if app.mod_panel.is_some() { ampliar_ficha(app) } else { Task::none() };
            app.servicios.ficha.tab=ficha::Pestana::Versiones;
            app.servicios.ficha.mostrar_todas=true;
            app.servicios.ficha.minecraft="Todas las versiones".into();
            app.servicios.ficha.loader="Todos los loaders".into();
            return Task::batch([page,cargar_versiones(app)]);
        }
        Accion::CargarMiembros => return cargar_miembros(app),
        Accion::CargarRelacionados=>return cargar_relacionados(app),
        Accion::Relacionados(id,result) if id==app.servicios.search_id && app.servicios.detail.is_some()=>{
            let current=app.servicios.detail.as_ref().unwrap().0.id.clone();
            let state=&mut app.servicios.ficha;state.relacionados_busy=false;
            match result {Ok(projects)=>{state.relacionados=projects.into_iter().filter(|p|p.id!=current&&(p.project_type=="mod"||p.project_types.iter().any(|t|t=="mod"))).take(3).collect();let urls=state.relacionados.iter().filter_map(|p|p.icon_url.clone()).collect();return cargar_imagenes(app,urls);},Err(error)=>state.relacionados_error=Some(error)}
        }
        Accion::MiembrosRecibidos(id, result)
            if id == app.servicios.search_id && app.servicios.detail.is_some() =>
        {
            let s = &mut app.servicios.ficha;
            s.miembros_busy = false;
            match result {
                Ok(members) => {
                    let urls = members
                        .iter()
                        .filter_map(|m| m.user.avatar_url.clone())
                        .collect();
                    s.miembros = members;
                    let related=if app.servicios.detail.as_ref().is_some_and(|(p,_)|p.organization.is_none()){cargar_relacionados(app)}else{Task::none()};
                    return Task::batch([cargar_imagenes(app, urls),related]);
                }
                Err(error) => s.miembros_error = Some(error),
            }
        }
        Accion::VersionMinecraft(v) => {
            if app.editor.is_some() && !app.servicios.ficha.mostrar_todas {
                return Task::none();
            }
            app.servicios.ficha.minecraft = v;
            return cargar_versiones(app);
        }
        Accion::VersionLoader(v) => {
            if app.editor.is_some() && !app.servicios.ficha.mostrar_todas {
                return Task::none();
            }
            app.servicios.ficha.loader = v;
            return cargar_versiones(app);
        }
        Accion::CargarVersiones => return cargar_versiones(app),
        Accion::VersionesRecibidas(generation, request, result)
            if generation == app.servicios.search_id
                && request == app.servicios.ficha.versiones_id
                && app.servicios.detail.is_some() =>
        {
            let s = &mut app.servicios.ficha;
            s.versiones_busy = false;
            s.versiones_cancel = None;
            match result {
                Ok(versions) => {
                    s.elegir_predeterminada(&versions);
                    app.servicios.detail.as_mut().unwrap().1 = versions;
                    if s.tab == ficha::Pestana::Dependencias {
                        return cargar_dependencias(app);
                    }
                    if s.tab == ficha::Pestana::Cambios {
                        return cargar_cambios(app);
                    }
                }
                Err(error) => s.versiones_error = Some(error),
            }
        }
        Accion::VersionPruebas(pruebas) => {
            let s = &mut app.servicios.ficha;
            s.pruebas = pruebas;
            s.pagina = 0;
            if let Some((_, versions)) = &app.servicios.detail {
                s.elegir_predeterminada(versions);
            }
            if s.tab == ficha::Pestana::Cambios {
                return cargar_cambios(app);
            }
            if s.tab == ficha::Pestana::Dependencias {
                return cargar_dependencias(app);
            }
        }
        Accion::VersionPagina(page) => {
            if let Some((_, versions)) = &app.servicios.detail {
                let s = &mut app.servicios.ficha;
                s.pagina =
                    page.min(s.visibles(versions).len().saturating_sub(1) / ficha::PAGE_SIZE);
            }
        }
        Accion::CambiosVersion(id) | Accion::CambiosSeleccionar(id) => {
            let Some((_, versions)) = &app.servicios.detail else {
                return Task::none();
            };
            if !app
                .servicios
                .ficha
                .visibles(versions)
                .iter()
                .any(|v| v.id == id)
            {
                return Task::none();
            }
            let s = &mut app.servicios.ficha;
            s.limpiar_cambios();
            s.seleccionada = Some(id);
            if s.tab != ficha::Pestana::Dependencias {
                s.tab = ficha::Pestana::Cambios;
            }
            if s.tab == ficha::Pestana::Dependencias {
                return cargar_dependencias(app);
            }
            if s.tab == ficha::Pestana::Cambios {
                return cargar_cambios(app);
            }
        }
        Accion::CambiosRecibidos(generation, request, result)
            if generation == app.servicios.search_id
                && request == app.servicios.ficha.cambios_id
                && app.servicios.detail.is_some() =>
        {
            let s = &mut app.servicios.ficha;
            s.cambios_busy = false;
            s.cambios_cancel = None;
            match result {
                Ok(version)
                    if Some(&version.id) == s.seleccionada.as_ref()
                        && app
                            .servicios
                            .detail
                            .as_ref()
                            .is_some_and(|(p, _)| p.id == version.project_id) =>
                {
                    s.documento_cambios =
                        catalogo::Documento::parse(version.changelog.as_deref().unwrap_or(""));
                    s.cambio = Some(version);
                    return cargar_imagenes(app, imagenes_visibles(app));
                }
                Ok(_) => {
                    s.cambios_error =
                        Some("Los cambios recibidos no corresponden a esta versión.".into())
                }
                Err(error) => s.cambios_error = Some(error),
            }
        }
        Accion::Galeria(i) => {
            if app
                .servicios
                .detail
                .as_ref()
                .is_some_and(|(p, _)| i < p.gallery.len())
            {
                app.servicios.ficha.galeria = i;
                return cargar_imagenes(app, imagenes_visibles(app));
            }
        }
        Accion::FichaPestana(tab) => {
            if app.servicios.detail.is_none() {
                return Task::none();
            }
            app.servicios.ficha.tab = tab;
            crate::arrancar(app);
            if tab == ficha::Pestana::Dependencias {
                return cargar_dependencias(app);
            }
            if tab == ficha::Pestana::Cambios {
                return cargar_cambios(app);
            }
            return cargar_imagenes(app, imagenes_visibles(app));
        }
        Accion::ErrorEnlace(error) => app.avisar(error),
        Accion::Minecraft(value) => {
            app.servicios.minecraft = value;
            return buscar(app, 0, false);
        }
        Accion::Loader(value) => {
            app.servicios.loader = value;
            return buscar(app, 0, false);
        }
        Accion::Orden(value) => {
            app.servicios.order = value;
            return buscar(app, 0, false);
        }
        Accion::LimpiarFiltros => {
            app.tienda_cat.clear();
            app.servicios.minecraft = "Todas las versiones".into();
            app.servicios.loader = "Todos los loaders".into();
            app.servicios.order = Order::Relevance;
            return buscar(app, 0, false);
        }
        Accion::CargarTags => return cargar_tags(app),
        Accion::Tags(result) => {
            app.servicios.tags_busy = false;
            match result {
                Ok(tags) => app.servicios.tags = Some(tags),
                Err(error) => app.servicios.tags_error = Some(error),
            }
        }
        Accion::Imagen(url, image) => {
            app.servicios.images.recibir(url, image);
            return cargar_imagenes(app, imagenes_visibles(app));
        }
        Accion::Enlace(url) => {
            return Task::perform(
                async move { modrinth::open_link(&url).await },
                |r| match r {
                    Ok(()) => Mensaje::Nada,
                    Err(e) => Mensaje::Servicio(Accion::ErrorEnlace(e.to_string())),
                },
            )
        }
        Accion::Inicializar => {
            if let Some(error) = app.servicios.instancias.error.clone() {
                app.avisar(error);
            }
            let auth = begin_auth(app, true);
            let catalog = if app.seccion == Seccion::Modpacks || app.mods_vista == 1 {
                buscar(app, 0, false)
            } else {
                Task::none()
            };
            return Task::batch([auth, catalog]);
        }
        Accion::Login => {
            if !app.servicios.auth_busy {
                app.servicios.auth_error = None;
                app.pantalla = Some(pantallas::Pantalla::Codigo);
                app.sesion.menu = false;
            }
        }
        Accion::LoginBrowser => return begin_browser(app),
        Accion::Renovar => {
            if !app.servicios.auth_busy
                && app
                    .servicios
                    .session
                    .as_ref()
                    .is_some_and(|s| s.needs_refresh())
            {
                return begin_auth(app, false);
            }
        }
        Accion::CancelarLogin => {
            if app.servicios.local.ocupado { return Task::none(); }
            crate::juego_real::cancelar_espera_sesion(app);
            app.servicios.cancel();
            app.servicios.auth_error = None;
            app.pantalla = app
                .servicios
                .session
                .is_none()
                .then_some(pantallas::Pantalla::Primera);
        }
        Accion::ContinuarInvitado => {
            return crate::cuenta_sin_conexion::actualizar(app, crate::cuenta_sin_conexion::Accion::Abrir);
        }
        Accion::Logout => {
            if app.servicios.auth_busy { return Task::none(); }
            if app.servicios.session.as_ref().is_some_and(|s| s.es_local()) {
                return crate::cuenta_sin_conexion::actualizar(app, crate::cuenta_sin_conexion::Accion::Abrir);
            }
            app.servicios.local.microsoft = None;
            app.sesion.cuenta.invalidar();
            app.sesion.subiendo = None;
            app.servicios.juego.cancelar_todo();
            app.servicios.cancel();
            app.servicios.session = None;
            app.servicios.perfil_recordado = None;
            app.servicios.remembered = false;
            app.sesion.estado = EstadoSesion::SinConexion;
            app.sesion.menu = false;
            app.servicios.auth_busy = true;
            let id = app.servicios.auth_id;
            let vault = app.servicios.vault.clone();
            return Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || vault.forget(id))
                        .await
                        .map_err(|_| "No se pudo acceder al almacén de credenciales.".to_owned())?
                        .map_err(|e| e.to_string())
                },
                move |result| m(Accion::Cerrada(id, result)),
            );
        }
        Accion::Cerrada(id, result) if id == app.servicios.auth_id => {
            app.servicios.auth_busy = false;
            if let Err(error) = result {
                app.servicios.auth_error = Some(format!(
                    "{error} No se pudo borrar la sesión guardada. Vuelve a pulsar Cerrar sesión."
                ));
            } else {
                app.servicios.auth_error = None;
                app.avisar("Sesión cerrada y credenciales guardadas eliminadas.".into());
            }
        }
        Accion::PerfilRecordado(id, nombre) if id == app.servicios.auth_id && app.servicios.restoring => {
            app.servicios.perfil_recordado = Some(nombre);
            if app.pantalla == Some(pantallas::Pantalla::Primera) { app.pantalla = None; }
        }
        Accion::Progreso(id, phase) if id == app.servicios.auth_id && app.servicios.auth_busy => {
            let volver = app.pantalla == Some(pantallas::Pantalla::Codigo)
                && !app.servicios.progress.is_some_and(|p| p.paso().is_some())
                && phase.paso().is_some();
            app.servicios.progress = Some(phase);
            if volver {
                return iced::window::get_latest().and_then(|id| {
                    iced::window::minimize(id, false).chain(iced::window::gain_focus(id))
                });
            }
        }
        Accion::Autenticada(id, result, stored) if id == app.servicios.auth_id => {
            let restoring = app.servicios.restoring;
            let interactive = app.pantalla == Some(pantallas::Pantalla::Codigo);
            app.servicios.auth_busy = false;
            app.servicios.restoring = false;
            app.servicios.progress = None;
            app.servicios.auth_cancel = None;
            match result {
                Ok(session) => {
                    if session.is_none() { app.servicios.perfil_recordado = None; }
                    app.servicios.remembered = stored;
                    if app.servicios.session.as_ref().map(|s| &s.profile.id) != session.as_ref().map(|s| &s.profile.id) {
                        app.sesion.cuenta.invalidar(); app.sesion.subiendo=None; app.sesion.textura=None;
                    }
                    app.sesion.estado = if session.as_ref().is_some_and(|s| !s.es_local()) {
                        EstadoSesion::Activa
                    } else {
                        EstadoSesion::SinConexion
                    };
                    app.servicios.session = session;
                    if app.servicios.session.is_some() {
                        if app.pantalla == Some(pantallas::Pantalla::Codigo) {
                            app.pantalla = Some(pantallas::Pantalla::Cuenta);
                        } else if restoring && app.pantalla == Some(pantallas::Pantalla::Primera) {
                            app.pantalla = None;
                        }
                        if !stored {
                            app.servicios.auth_error = Some("Sesión activa solo durante esta apertura: no se pudo guardar la credencial en el almacén del sistema.".into());
                            app.avisar("La cuenta está activa, pero no pudo guardarse para la próxima apertura.".into());
                        }
                    }
                }
                Err(error) => {
                    app.servicios.auth_error = Some(error);
                    app.sesion.cuenta.invalidar(); app.sesion.subiendo=None;
                    if app.servicios.session.is_some() || (restoring && app.servicios.perfil_recordado.is_some()) {
                        app.servicios.session = None;
                        app.sesion.estado = EstadoSesion::Vencida;
                    }
                }
            }
            if restoring && app.servicios.session.is_none() && app.servicios.perfil_recordado.is_none()
                && !(app.pantalla == Some(pantallas::Pantalla::Actualizacion) && app.actualizacion.resultado.is_some()) {
                app.pantalla = Some(pantallas::Pantalla::Primera);
            }
            if interactive {
                let window = iced::window::get_latest().and_then(|id| {
                    iced::window::minimize(id, false).chain(iced::window::gain_focus(id))
                });
                let cuenta = if app.pantalla == Some(pantallas::Pantalla::Cuenta) && app.servicios.session.is_some() { crate::cuenta_real::actualizar(app,crate::cuenta_real::Accion::Abrir(false)) }else{Task::none()};
                return Task::batch([window, cuenta, crate::juego_real::reanudar_sesion(app)]);
            }
            return crate::juego_real::reanudar_sesion(app);
        }
        Accion::Consulta(query) => {
            app.servicios.query = query.chars().take(512).collect();
            return buscar(app, 0, true);
        }
        Accion::Buscar => return buscar(app, 0, false),
        Accion::Pagina(offset) => return buscar(app, offset, false),
        Accion::Buscada(id, result) if id == app.servicios.search_id => {
            app.servicios.busy = false;
            match result {
                Ok(results) => {
                    let urls = results
                        .hits
                        .iter()
                        .filter_map(|h| h.icon_url.clone())
                        .collect();
                    app.servicios.results = Some(results);
                    return cargar_imagenes(app, urls);
                }
                Err(error) => app.servicios.error = Some(error),
            }
        }
        Accion::ProyectoVersiones(id) => {
            let task = actualizar(app, Accion::Proyecto(id));
            app.servicios.ficha.tab = ficha::Pestana::Versiones;
            return task;
        }
        Accion::Proyecto(id) => {
            if let Some(detail) = app.servicios.detail.take() {
                if app.servicios.anteriores.len() >= 32 {
                    app.servicios.anteriores.remove(0);
                }
                let mut state = std::mem::take(&mut app.servicios.ficha);
                if let Some(c) = state.versiones_cancel.take() {
                    c.cancel();
                }
                if let Some(c) = state.cambios_cancel.take() {
                    c.cancel();
                }
                state.versiones_busy = false;
                state.cambios_busy = false;
                state.miembros_busy = false;
                state.organizacion_busy = false;
                state.relacionados_busy = false;
                app.servicios.anteriores.push(PaginaAnterior {
                    panel: app.mod_panel.clone(),
                    mods_vista: app.mods_vista,
                    detail,
                    ficha: state,
                    documento: std::mem::take(&mut app.servicios.document),
                });
            }
            if let Some(cancel) = app.servicios.search_cancel.take() {
                cancel.cancel();
            }
            app.servicios.search_id += 1;
            let generation = app.servicios.search_id;
            let cancel = CancellationToken::new();
            app.servicios.search_cancel = Some(cancel.clone());
            let filter = context(app);
            app.servicios.filter = Some(filter.clone());
            let catalog = app.servicios.catalog.clone();
            app.servicios.busy = true;
            app.servicios.error = None;
            app.servicios.detail = None;
            app.servicios.ficha = Default::default();
            return Task::perform(
                async move {
                    tokio::select! {
                        biased;
                        _ = cancel.cancelled() => Err("Consulta cancelada.".into()),
                        result = async {
                            let catalog = catalog?;
                            let project = catalog.project(&id).await?;
                            let resource = project.project_type == "resourcepack";
                            let pack = matches!(project.project_type.as_str(), "resourcepack" | "shader");
                            let versions = catalog.versions(&project.id, if resource { None } else { filter.minecraft.as_deref() }, if pack { None } else { filter.loader.as_deref() }).await?;
                            Ok::<_, modrinth::Error>((project, versions))
                        } => result.map_err(|e| e.to_string()),
                    }
                },
                move |result| m(Accion::ProyectoRecibido(generation, result)),
            );
        }
        Accion::ProyectoRecibido(id, result) if id == app.servicios.search_id => {
            app.servicios.busy = false;
            match result {
                Ok(detail) => {
                    app.servicios.document = catalogo::Documento::parse(&detail.0.body);
                    let tab = app.servicios.ficha.tab;
                    app.servicios.ficha =
                        ficha::Estado::nuevo(app.servicios.filter.as_ref(), &detail.1);
                    app.servicios.ficha.tab = tab;
                    app.servicios.detail = Some(detail);
                    crate::arrancar(app);
                    let images = cargar_imagenes(app, imagenes_visibles(app));
                    return Task::batch([images, cargar_miembros(app), cargar_dependencias(app),crate::instancias_reales::abrir(app)]);
                }
                Err(error) => app.servicios.error = Some(error),
            }
        }
        Accion::CerrarPanel => {
            if let Some(cancel) = app.servicios.search_cancel.take() {
                cancel.cancel();
            }
            app.servicios.search_id += 1;
            app.servicios.anteriores.clear();
            app.servicios.detail = None;
            app.servicios.ficha = Default::default();
            app.servicios.document = Default::default();
            app.servicios.error = None;
            app.servicios.busy = false;
            app.mod_panel = None;
            crate::arrancar(app);
        }
        Accion::Volver => {
            if let Some(cancel) = app.servicios.search_cancel.take() {
                cancel.cancel();
            }
            app.servicios.search_id += 1;
            app.servicios.detail = None;
            app.servicios.ficha = Default::default();
            app.servicios.error = None;
            app.servicios.busy = false;
            app.mod_panel = None;
            if let Some(previous) = app.servicios.anteriores.pop() {
                app.mod_panel = previous.panel;
                app.mods_vista = previous.mods_vista;
                app.servicios.detail = Some(previous.detail);
                app.servicios.ficha = previous.ficha;
                app.servicios.document = previous.documento;
                let changes = if app.servicios.ficha.tab == ficha::Pestana::Cambios {
                    cargar_cambios(app)
                } else {
                    Task::none()
                };
                return Task::batch([cargar_dependencias(app), cargar_miembros(app), changes]);
            }
            crate::arrancar(app);
            if app.servicios.results.is_none() {
                if app.editor.is_some() {
                    app.mods_vista = 0;
                    return Task::none();
                }
                return buscar(app, 0, false);
            }
        }
        _ => {}
    }
    Task::none()
}

pub fn ampliar_ficha(app: &mut App) -> Task<Mensaje> {
    let Some(detail) = app.servicios.detail.clone() else {
        return Task::none();
    };
    let mut previous = std::mem::take(&mut app.servicios.ficha);
    let mut current = ficha::Estado::nuevo(app.servicios.filter.as_ref(), &detail.1);
    current.miembros = previous.miembros.clone();
    current.organizacion = previous.organizacion.clone();
    current.info_snapshots = previous.info_snapshots;
    current.relacionados = previous.relacionados.clone();
    if let Some(c) = previous.versiones_cancel.take() {
        c.cancel();
    }
    if let Some(c) = previous.cambios_cancel.take() {
        c.cancel();
    }
    previous.versiones_busy = false;
    previous.cambios_busy = false;
    previous.miembros_busy = false;
    previous.organizacion_busy = false;
    previous.relacionados_busy = false;
    if app.servicios.anteriores.len() >= 32 {
        app.servicios.anteriores.remove(0);
    }
    app.servicios.anteriores.push(PaginaAnterior {
        panel: app.mod_panel.take(),
        mods_vista: app.mods_vista,
        detail,
        ficha: previous,
        documento: app.servicios.document.clone(),
    });
    app.servicios.ficha = current;
    app.mods_vista = 1;
    cargar_miembros(app)
}

/// La captura visual usa respuestas públicas e imágenes ya guardadas en Temp;
/// no inicia sesiones, peticiones ni escrituras en las instancias del usuario.
pub fn preparar_creadores_diseno(app: &mut App) {
    let Some(directory) = std::env::var_os("ICARO_QA_CREATORES") else { return; };
    let json = std::fs::read(std::path::PathBuf::from(directory).join("creadores.json"))
        .expect("Fixture local de creadores");
    let fixture: serde_json::Value = serde_json::from_slice(&json).expect("Fixture JSON");
    app.servicios.ficha.miembros = serde_json::from_value(fixture["members"].clone()).expect("Miembros");
    app.servicios.ficha.organizacion = Some(serde_json::from_value(fixture["organization"].clone()).expect("Organización"));
    for image in fixture["images"].as_array().expect("Imágenes") {
        let bytes = std::fs::read(image["path"].as_str().expect("Ruta local")).expect("Imagen descargada");
        let decoded = catalogo::decodificar(bytes).expect("Avatar decodificable");
        app.servicios.images.recibir(image["url"].as_str().expect("URL pública").into(), Some(decoded));
    }
}

pub fn welcome_status(app: &App) -> Element<'_, Mensaje> {
    let p = app.paleta();
    let status = if app.servicios.restoring {
        app.servicios
            .progress
            .map_or("Recuperando tu sesión…", FaseAcceso::texto)
    } else {
        app.servicios
            .auth_error
            .as_deref()
            .unwrap_or("Elige una cuenta Microsoft o sin conexión para comenzar.")
    };
    text(status)
        .width(Length::Fill)
        .color(if app.servicios.auth_error.is_some() {
            p.warning
        } else {
            p.text_muted
        })
        .into()
}

pub fn login_view(app: &App) -> Element<'_, Mensaje> {
    let p = app.paleta();
    let paso = app.servicios.progress.and_then(FaseAcceso::paso);
    let mut content = column![
        text(if paso.is_some() {
            "PREPARANDO TU CUENTA"
        } else {
            "CONECTA TU CUENTA"
        })
        .font(fuentes::DISPLAY)
        .size(texto::DISPLAY.0)
        .color(p.text),
        text(if app.servicios.auth_busy {
            app.servicios
                .progress
                .map_or("Preparando el inicio de sesión…", FaseAcceso::texto)
        } else {
            "Continúa en Microsoft para vincular tu cuenta de Minecraft."
        })
        .color(p.text_muted)
        .width(Length::Fill),
    ]
    .spacing(espacio::S4)
    .width(Length::Fill);
    if let Some(actual) = paso {
        content = content.push(componentes::progreso_bloques(p, 4, actual));
        for (i, etiqueta) in [
            "Validar la sesión",
            "Comprobar Minecraft Java",
            "Obtener tu perfil",
            "Guardar la cuenta",
        ]
        .into_iter()
        .enumerate()
        {
            content = content.push(
                row![
                    text(if i < actual {
                        "✓".to_owned()
                    } else {
                        format!("{}", i + 1)
                    })
                    .font(fuentes::MONO)
                    .color(if i < actual { p.success } else { p.text_muted }),
                    text(etiqueta).color(if i == actual { p.text } else { p.text_muted }),
                ]
                .spacing(espacio::S3),
            );
        }
    }
    if !app.servicios.auth_busy {
        content = content.push(componentes::boton(
            p,
            "Continuar con Microsoft",
            Variante::Primario,
            Some(m(Accion::LoginBrowser)),
        ));
    }
    if !app.servicios.auth_busy {
        content = content.push(componentes::boton_con_icono(p, Icono::Usuario, "Usar cuenta sin conexión", Variante::Secundario, Some(crate::cuenta_sin_conexion::m(crate::cuenta_sin_conexion::Accion::Abrir)), false));
    }
    if let Some(error) = &app.servicios.auth_error {
        content = content.push(text(error).color(p.warning).width(Length::Fill));
    }
    content = content.push(componentes::boton(
        p,
        "Cancelar",
        Variante::Fantasma,
        Some(m(Accion::CancelarLogin)),
    ));
    crate::pantallas::con_lamina(
        p,
        icaro_ui::laminas::Lamina::Hermes,
        crate::desplazable_pub(content.into()),
    )
}
pub fn account_view(app: &App) -> Element<'_, Mensaje> {
    crate::cuenta_real::vista(app)
}

pub fn selector<'a>(app: &'a App, window: Element<'a, Mensaje>) -> Element<'a, Mensaje> {
    if app.servicios.local.abierta {
        return icaro_ui::superficies::con_velo(app.paleta(), window, crate::cuenta_sin_conexion::vista(app), crate::cuenta_sin_conexion::m(crate::cuenta_sin_conexion::Accion::Cerrar));
    }
    if !app.sesion.menu {
        return window;
    }
    let p = app.paleta();
    let gestionar = Mensaje::Ses(crate::sesion::Accion::Gestionar);
    let activa = app.servicios.session.is_some();
    let mut cuentas = Vec::new();
    if activa && app.servicios.session.as_ref().is_some_and(|s| !s.es_local()) {
        cuentas.push(CuentaSelector { textura: None,
            estado: EstadoSesion::Activa,
            nombre: app.servicios.nombre("Invitado").into(),
            seleccionada: true,
            al_pulsar: Some(gestionar.clone()),
        });
    }
    if app.servicios.session.as_ref().is_some_and(|s| s.es_local()) {
        if let Some(ms) = &app.servicios.local.microsoft {
            cuentas.push(CuentaSelector { textura: None,estado:EstadoSesion::Activa,nombre:ms.profile.name.clone(),seleccionada:false,
                al_pulsar:(!app.servicios.auth_busy).then(||crate::cuenta_sin_conexion::m(crate::cuenta_sin_conexion::Accion::UsarMicrosoft))});
        }
    }
    for nombre in &app.servicios.local.cuentas.nombres {
        cuentas.push(CuentaSelector { textura: app.servicios.local.imagenes.get(nombre).map(|(i,_)|i.textura.rgba.clone()),
            estado: EstadoSesion::SinConexion,
            nombre: nombre.clone(),
            seleccionada: app.servicios.session.as_ref().is_some_and(|s| s.es_local() && s.profile.name == *nombre),
            al_pulsar: (!app.servicios.auth_busy).then(|| crate::cuenta_sin_conexion::m(crate::cuenta_sin_conexion::Accion::Usar(nombre.clone()))),
        });
    }
    let mut acciones = vec![AccionSelector {
        icono: Icono::UsuarioMas, etiqueta: "Agregar cuenta Microsoft".into(), peligro: false,
        al_pulsar: (!app.servicios.auth_busy).then(||m(Accion::Login)),
    }, AccionSelector {
        icono: Icono::Usuario, etiqueta: "Añadir cuenta sin conexión".into(), peligro: false,
        al_pulsar: (!app.servicios.auth_busy).then(||crate::cuenta_sin_conexion::m(crate::cuenta_sin_conexion::Accion::Abrir)),
    }, AccionSelector {
        icono: Icono::Ajustes,
        etiqueta: if app.servicios.session.as_ref().is_some_and(|s|s.es_local()) {"Gestionar cuentas sin conexión"}else{"Gestionar cuenta y skin"}.into(),
        peligro: false,
        al_pulsar: Some(gestionar),
    }];
    if app.servicios.session.as_ref().is_some_and(|s|!s.es_local()) {
        acciones.push(AccionSelector {
            icono: Icono::Salir,
            etiqueta: format!("Cerrar sesión de {}", app.servicios.nombre("Sin cuenta")),
            peligro: true,
            al_pulsar: (!app.servicios.auth_busy).then(|| Mensaje::Ses(crate::sesion::Accion::CerrarSesion)),
        });
    }
    let panel = cuenta::selector_cuentas(p, cuentas, acciones);
    let outside = iced::widget::mouse_area(Space::new(Length::Fill, Length::Fill))
        .on_press(Mensaje::Ses(crate::sesion::Accion::Menu(false)));
    iced::widget::stack![
        window,
        outside,
        container(row![panel])
            .width(Length::Fill)
            .height(Length::Fill)
            .align_y(iced::alignment::Vertical::Bottom)
            .padding(Padding {
                left: espacio::S2,
                bottom: 72.0,
                ..Padding::ZERO
            })
    ]
    .into()
}

pub fn store_view(app: &App) -> Element<'_, Mensaje> {
    let p = app.paleta();
    let s = &app.servicios;
    let kind = context(app).kind;
    let mut content = column![].spacing(espacio::S4);
    if let Some((project, versions)) = &s.detail {
        if app.mod_panel.is_none() {
            return ficha::vista(app, project, versions, kind);
        }
    }
    if (kind == Kind::Modpack && !s.hay_anterior()) || (matches!(kind, Kind::Mod | Kind::Resourcepack | Kind::Shader) && matches!(app.editor, Some(2 | 3 | 8))) {
        return crate::diseno_mods::tienda(app);
    }
    if s.hay_anterior() {
        let status = s.error.as_deref().unwrap_or("Obteniendo el proyecto…");
        return container(
            column![
                componentes::boton_con_icono(
                    p,
                    Icono::ChevronIzquierda,
                    "Volver",
                    Variante::Fantasma,
                    Some(m(Accion::Volver)),
                    false
                ),
                text(status).color(if s.error.is_some() {
                    p.warning
                } else {
                    p.text_muted
                })
            ]
            .spacing(espacio::S4),
        )
        .padding(espacio::S6)
        .into();
    }
    if app.editor.is_some() {
        content = content.push(componentes::boton_con_icono(
            p,
            Icono::ChevronIzquierda,
            if app.editor == Some(2) {
                "Volver a mis mods"
            } else if app.editor == Some(8) {
                "Volver a mis shaders"
            } else {
                "Volver a mis recursos"
            },
            Variante::Fantasma,
            Some(Mensaje::VistaMods(0)),
            false,
        ));
        if app.editor == Some(2) {
            content = content.push(crate::mods_reales::estado_operacion(app));
        }
    }
    {
        content = content
            .push(
                text(if kind == Kind::Modpack {
                    "MODPACKS"
                } else {
                    if kind == Kind::Mod {
                        "MODS"
                    } else if kind == Kind::Shader {
                        "SHADERS"
                    } else {
                        "RECURSOS"
                    }
                })
                .font(fuentes::DISPLAY)
                .size(40)
                .color(p.text),
            )
            .push(
                row![
                    container(componentes::campo(
                        p,
                        "Buscar en Modrinth",
                        &s.query,
                        |q| m(Accion::Consulta(q))
                    ))
                    .width(Length::Fill),
                    componentes::boton(
                        p,
                        "Buscar",
                        Variante::Secundario,
                        (!s.busy).then(|| m(Accion::Buscar))
                    )
                ]
                .spacing(espacio::S3),
            );
        if let Some(filter) = &s.filter {
            if let Some(mc) = &filter.minecraft {
                content = content.push(
                    text(format!(
                        "Minecraft {mc} · {}",
                        filter.loader.as_deref().unwrap_or("todos los loaders")
                    ))
                    .color(p.text_muted),
                );
            }
        }
        if kind == Kind::Modpack {
            let mut versiones = vec!["Todas las versiones".to_owned()];
            let mut loaders = vec!["Todos los loaders".to_owned()];
            if let Some(tags) = &s.tags {
                versiones.extend(
                    tags.versions
                        .iter()
                        .filter(|v| v.version_type == "release")
                        .map(|v| v.version.clone()),
                );
                loaders.extend(
                    tags.loaders
                        .iter()
                        .filter(|l| l.supported_project_types.iter().any(|t| t == "modpack"))
                        .map(|l| match l.name.as_str() {
                            "fabric" => "Fabric".to_owned(),
                            "forge" => "Forge".to_owned(),
                            "neoforge" => "NeoForge".to_owned(),
                            "quilt" => "Quilt".to_owned(),
                            _ => l.name.clone(),
                        }),
                );
            }
            content = content.push(
                row![
                    column![
                        text("Minecraft").color(p.text_muted),
                        componentes::selector(p, versiones, Some(s.minecraft.clone()), |v| m(
                            Accion::Minecraft(v)
                        ))
                    ]
                    .spacing(espacio::S2),
                    column![
                        text("Loader").color(p.text_muted),
                        componentes::selector(p, loaders, Some(s.loader.clone()), |v| m(
                            Accion::Loader(v)
                        ))
                    ]
                    .spacing(espacio::S2),
                    column![
                        text("Ordenar por").color(p.text_muted),
                        componentes::selector(
                            p,
                            vec![
                                Order::Relevance,
                                Order::Downloads,
                                Order::Updated,
                                Order::Newest
                            ],
                            Some(s.order),
                            |v| m(Accion::Orden(v))
                        )
                    ]
                    .spacing(espacio::S2),
                    componentes::boton(
                        p,
                        "Restablecer",
                        Variante::Fantasma,
                        Some(m(Accion::LimpiarFiltros))
                    ),
                ]
                .spacing(espacio::S3)
                .wrap(),
            );
            if s.tags_busy {
                content = content.push(text("Cargando filtros…").color(p.text_muted));
            }
            if let Some(error) = &s.tags_error {
                content = content.push(
                    row![
                        text(error).color(p.warning),
                        componentes::boton(
                            p,
                            "Reintentar filtros",
                            Variante::Fantasma,
                            Some(m(Accion::CargarTags))
                        )
                    ]
                    .spacing(espacio::S3)
                    .wrap(),
                );
            }
        }
        if let Some(results) = &s.results {
            content = content.push(
                text(format!(
                    "{} resultados",
                    catalogo::numero(results.total_hits as u64)
                ))
                .color(p.text_muted),
            );
            for hit in &results.hits {
                content = content.push(if kind == Kind::Mod && app.editor == Some(2) {
                    catalogo::tarjeta_proyecto_con_accion(
                        p,
                        hit,
                        &s.images,
                        Some(crate::mods_reales::accion_proyecto(
                            app,
                            &hit.project_id,
                            None,
                        )),
                    )
                } else if kind == Kind::Resourcepack && app.editor == Some(3) {
                    catalogo::tarjeta_proyecto_con_accion(
                        p,
                        hit,
                        &s.images,
                        Some(crate::mods_reales::accion_recurso(
                            app,
                            &hit.project_id,
                            None,
                        )),
                    )
                } else if kind == Kind::Shader && app.editor == Some(8) {
                    catalogo::tarjeta_proyecto_con_accion(
                        p,
                        hit,
                        &s.images,
                        Some(crate::mods_reales::packs::accion_instalar(
                            app,
                            icaro_core::mods::TipoPack::Shaders,
                            &hit.project_id,
                            None,
                        )),
                    )
                } else {
                    catalogo::tarjeta_proyecto(p, hit, &s.images)
                });
            }
            if results.hits.is_empty() {
                content = content.push(
                    text("No encontramos resultados. Prueba otra búsqueda o cambia los filtros.")
                        .color(p.text_muted),
                );
            }
            content = content.push(
                row![
                    componentes::boton(
                        p,
                        "Anterior",
                        Variante::Secundario,
                        (results.offset > 0 && !s.busy).then(|| m(Accion::Pagina(
                            results.offset.saturating_sub(modrinth::PAGE_SIZE)
                        )))
                    ),
                    componentes::boton(
                        p,
                        "Siguiente",
                        Variante::Secundario,
                        (results.offset.saturating_add(results.hits.len() as u32)
                            < results.total_hits
                            && !s.busy)
                            .then(|| m(Accion::Pagina(
                                results.offset.saturating_add(modrinth::PAGE_SIZE)
                            )))
                    ),
                ]
                .spacing(espacio::S3),
            );
        }
    }
    if s.busy {
        content = content.push(text("Consultando Modrinth…").color(p.text_muted));
    }
    if let Some(error) = &s.error {
        content = content
            .push(text(error).color(p.warning))
            .push(componentes::boton(
                p,
                "Reintentar",
                Variante::Secundario,
                Some(m(Accion::Buscar)),
            ));
    }
    content = content.push(
        text("La instalación desde el catálogo todavía está en desarrollo.")
            .size(12)
            .color(p.text_muted),
    );
    crate::desplazable_pub(container(content).padding(espacio::S6).into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn perfil_recordado_no_autentica_y_descarta_respuestas_antiguas() {
        let (mut app, _) = crate::iniciar_con_guardado(Seccion::Instancias,false,None,false,Estado::nuevo(false),None);
        app.servicios.restoring = true;
        let id = app.servicios.auth_id;
        let _ = actualizar(&mut app, Accion::PerfilRecordado(id.wrapping_add(1), "Otro".into()));
        assert_eq!(app.servicios.nombre("demo"), "Recuperando sesión…");
        let _ = actualizar(&mut app, Accion::PerfilRecordado(id, "Jugador".into()));
        assert_eq!(app.servicios.nombre("demo"), "Jugador");
        assert!(app.servicios.session.is_none());
        assert_eq!(app.pantalla, None);
    }
    #[test]
    fn cancelar_recuperacion_borra_perfil_y_rechaza_eventos_tardios() {
        let (mut app, _) = crate::iniciar_con_guardado(Seccion::Instancias,false,None,false,Estado::nuevo(false),None);
        let id = app.servicios.auth_id;
        app.servicios.restoring = true;
        let _ = actualizar(&mut app, Accion::PerfilRecordado(id, "Jugador".into()));
        app.servicios.cancel();
        let _ = actualizar(&mut app, Accion::PerfilRecordado(id, "Jugador".into()));
        assert!(app.servicios.perfil_recordado.is_none());
        assert!(app.servicios.session.is_none());
        assert_eq!(app.servicios.nombre("demo"), "Sin cuenta");
    }

    #[test]
    fn perfil_recordado_permanece_visible_si_falla_la_renovacion() {
        let (mut app, _) = crate::iniciar_con_guardado(Seccion::Instancias,false,None,false,Estado::nuevo(false),None);
        let id = app.servicios.auth_id;
        app.servicios.restoring = true;
        let _ = actualizar(&mut app, Accion::PerfilRecordado(id, "Jugador".into()));
        let _ = actualizar(&mut app, Accion::Autenticada(id, Err("Sin conexión".into()), false));
        assert_eq!(app.pantalla, None);
        assert_eq!(app.servicios.nombre("demo"), "Jugador");
        assert!(app.servicios.session.is_none());
        assert!(matches!(app.sesion.estado, EstadoSesion::Vencida));
        assert!(app.servicios.auth_error.is_some());
    }

    #[test]
    fn el_resultado_de_actualizacion_sigue_visible_sin_cuenta_recuperada() {
        let (mut app,_) = crate::iniciar_con_guardado(Seccion::Instancias,false,None,false,Estado::nuevo(false),None);
        crate::actualizacion_real::preparar_diseno_estado(&mut app,"actualizacion-completada");
        let id=app.servicios.auth_id;
        let _=actualizar(&mut app,Accion::Autenticada(id,Ok(None),false));
        assert_eq!(app.pantalla,Some(pantallas::Pantalla::Actualizacion));
        assert!(app.actualizacion.resultado.as_ref().unwrap().correcto);
        let _=crate::actualizacion_real::actualizar(&mut app,crate::actualizacion_real::Accion::MasTarde);
        assert_eq!(app.pantalla,Some(pantallas::Pantalla::Primera));
        assert!(app.servicios.session.is_none());
    }
    #[test]
    fn startup_restoration_guest_and_demo_data_stay_separate() {
        let (mut app, _) = crate::iniciar_con_guardado(
            Seccion::Instancias,
            false,
            None,
            false,
            Estado::nuevo(false),
            None,
        );
        assert_eq!(app.pantalla, Some(pantallas::Pantalla::Primera));
        assert!(app.servicios.restoring);
        let _ = crate::actualizar_estado(&mut app, Mensaje::AtajoSeccion(Seccion::Ajustes));
        assert_eq!(app.seccion, Seccion::Instancias);
        assert_eq!(app.pantalla, Some(pantallas::Pantalla::Primera));
        assert!(crate::instancias().is_empty());
        assert!(app.lista_grupos.is_empty() && app.grupos.is_empty());
        assert!(
            app.red.servidores.is_empty() && app.red.historial.is_empty() && !app.red.hay_cola()
        );
        assert!(app.capt.capturas.is_empty());
        assert!(crate::busqueda::resultados(&app).is_empty());
        assert_ne!(
            crate::estado::directorio(false),
            crate::estado::directorio(true)
        );

        let _ = actualizar(&mut app, Accion::ContinuarInvitado);
        assert_eq!(app.pantalla, Some(pantallas::Pantalla::Primera));
        let _ = actualizar(&mut app, Accion::Inicializar);
        let id = app.servicios.auth_id;
        let _ = actualizar(
            &mut app,
            Accion::Autenticada(
                id,
                Err("No se pudo recuperar la sesión guardada.".into()),
                false,
            ),
        );
        assert!(!app.servicios.restoring && !app.servicios.auth_busy);
        assert_eq!(app.pantalla, Some(pantallas::Pantalla::Primera));
        assert!(app.servicios.auth_error.is_some());

        let _ = actualizar(&mut app, Accion::ContinuarInvitado);
        assert!(app.servicios.local.abierta);
        assert_eq!(app.pantalla, Some(pantallas::Pantalla::Primera));
        assert_eq!(app.servicios.nombre("Mineral_7"), "Sin cuenta");
        let _ = crate::cuenta_sin_conexion::actualizar(&mut app, crate::cuenta_sin_conexion::Accion::Cerrar);
        for seccion in Seccion::TODAS {
            app.seccion = seccion;
            let _ = crate::vista(&app);
        }
        let _ = pantallas::actualizar(&mut app, pantallas::Accion::NuevaInstancia);
        assert!(app.asistente.is_some() && crate::instancias().is_empty());
        let _ = pantallas::actualizar(&mut app, pantallas::Accion::AsistenteCancelar);
        assert!(app.asistente.is_none());
        let _ = actualizar(&mut app, Accion::Login);
        assert_eq!(app.pantalla, Some(pantallas::Pantalla::Codigo));
        let antes = app.seccion;
        let _ = crate::actualizar_estado(&mut app, Mensaje::AtajoSeccion(Seccion::Instancias));
        assert_eq!(app.seccion, antes);
        assert_eq!(app.pantalla, Some(pantallas::Pantalla::Codigo));
        let _ = begin_browser(&mut app);
        let login = app.servicios.auth_id;
        let waiting = FaseAcceso::Autenticacion(auth::Stage::WaitingForBrowser);
        assert_eq!(app.servicios.progress, Some(waiting));
        let received = FaseAcceso::Autenticacion(auth::Stage::Microsoft);
        let _ = actualizar(&mut app, Accion::Progreso(login - 1, received));
        assert_eq!(app.servicios.progress, Some(waiting));
        let _ = actualizar(&mut app, Accion::Progreso(login, received));
        assert_eq!(app.servicios.progress, Some(received));
        let _ = login_view(&app);
        let _ = actualizar(&mut app, Accion::CancelarLogin);
        assert_eq!(app.pantalla, Some(pantallas::Pantalla::Primera));
        assert!(app.servicios.progress.is_none() && !app.servicios.auth_busy);
        let _ = actualizar(&mut app, Accion::Progreso(login, FaseAcceso::Guardando));
        assert!(app.servicios.progress.is_none());
        let current = app.servicios.auth_id;
        let _ = actualizar(&mut app, Accion::Autenticada(current - 1, Ok(None), false));
        assert_eq!(app.pantalla, Some(pantallas::Pantalla::Primera));

        app.seccion = Seccion::Modpacks;
        let _ = actualizar(&mut app, Accion::Minecraft("1.21.4".into()));
        let _ = actualizar(&mut app, Accion::Loader("Fabric".into()));
        let _ = actualizar(&mut app, Accion::Orden(Order::Downloads));
        let filtro = app.servicios.filter.as_ref().unwrap();
        assert_eq!(filtro.minecraft.as_deref(), Some("1.21.4"));
        assert_eq!(filtro.loader.as_deref(), Some("Fabric"));
        assert_eq!(filtro.order, Order::Downloads);
        assert_eq!(filtro.offset, 0);
        let actual = app.servicios.search_id;
        let results = Results {
            hits: Vec::new(),
            offset: 0,
            limit: 20,
            total_hits: 0,
        };
        let _ = actualizar(&mut app, Accion::Buscada(actual - 1, Ok(results.clone())));
        assert!(app.servicios.results.is_none());
        let _ = actualizar(&mut app, Accion::Buscada(actual, Ok(results)));
        let project: Project = serde_json::from_str(r##"{"id":"sample","slug":"sample","title":"Ejemplo","description":"Descripción","body":"# Título\n\n**Descripción**","downloads":1000,"license":{"id":"MIT","name":"MIT"}}"##).unwrap();
        let mut project = project;
        project.game_versions = vec!["1.21.4".into(), "1.21.5".into()];
        project.loaders = vec!["fabric".into(), "neoforge".into()];
        project.gallery = serde_json::from_value(serde_json::json!((0..9).map(|i| serde_json::json!({"url":format!("https://cdn.modrinth.com/data/sample/{i}.png"),"featured":false,"title":format!("Imagen {i}")})).collect::<Vec<_>>())).unwrap();
        let make_version = |id: &str| -> Version {
            serde_json::from_value(serde_json::json!({
                "id":id, "project_id":"sample", "name":id, "version_number":id, "version_type":"release",
                "game_versions":["1.21.4"], "loaders":["fabric"], "files":[], "dependencies":[],
                "date_published":"2026-10-11T03:00:00Z", "changelog":format!("## Cambios de {id}")
            })).unwrap()
        };
        let v1 = make_version("v1");
        let v2 = make_version("v2");
        let _ = actualizar(
            &mut app,
            Accion::ProyectoRecibido(actual, Ok((project, vec![v1.clone(), v2.clone()]))),
        );
        assert_eq!(app.servicios.ficha.minecraft, "1.21.4");
        assert_eq!(app.servicios.ficha.loader, "Fabric");
        let _ = actualizar(
            &mut app,
            Accion::MiembrosRecibidos(actual - 1, Ok(Vec::new())),
        );
        assert!(app.servicios.ficha.miembros_busy);
        let _ = actualizar(&mut app, Accion::MiembrosRecibidos(actual, Ok(Vec::new())));
        assert!(!app.servicios.ficha.miembros_busy);
        let creator_avatar = "https://avatars3.githubusercontent.com/u/31803019?v=4";
        let member: Member = serde_json::from_value(serde_json::json!({
            "user": {"id":"creator", "username":"IMS", "avatar_url":creator_avatar},
            "role":"Maintainer"
        })).unwrap();
        let _ = actualizar(&mut app, Accion::MiembrosRecibidos(actual, Ok(vec![member])));
        assert_eq!(app.servicios.ficha.miembros.len(), 1);
        assert!(app.servicios.images.pedir([creator_avatar.into()]).is_empty(),
            "El avatar se encola al recibir el creador, sin esperar otra navegación");
        let org_avatar = "https://cdn.modrinth.com/data/LjcZDkRW/icon.webp";
        let organization: Organization = serde_json::from_value(serde_json::json!({
            "id":"LjcZDkRW", "name":"CaffeineMC", "icon_url":org_avatar
        })).unwrap();
        let _ = actualizar(&mut app, Accion::OrganizacionRecibida(actual, Ok(organization)));
        assert!(app.servicios.images.pedir([org_avatar.into()]).is_empty(),
            "También se solicita la imagen de la organización");
        let _ = actualizar(&mut app, Accion::FichaPestana(ficha::Pestana::Galeria));
        let _ = actualizar(&mut app, Accion::Galeria(8));
        let _ = actualizar(&mut app, Accion::Galeria(999));
        assert_eq!(app.servicios.ficha.galeria, 8);
        assert!(imagenes_visibles(&app)
            .iter()
            .any(|u| u.ends_with("/8.png")));
        let _ = actualizar(&mut app, Accion::VisorAbrir(999));
        assert!(app.servicios.ficha.visor.is_none());
        let _ = actualizar(&mut app, Accion::VisorAbrir(8));
        let old_image = app.servicios.ficha.visor_id;
        let old_image_cancel = app.servicios.ficha.visor_cancel.clone().unwrap();
        let _ = actualizar(&mut app, Accion::VisorZoom(true));
        assert_eq!(app.servicios.ficha.visor.as_ref().unwrap().zoom, 125);
        let _ = crate::actualizar_estado(&mut app, Mensaje::AbrirBusqueda);
        assert!(!app.busqueda.abierta);
        let antes = app.seccion;
        let _ = crate::actualizar_estado(&mut app, Mensaje::AtajoSeccion(Seccion::Ajustes));
        assert_eq!(app.seccion, antes);
        assert!(app.servicios.ficha.visor.is_some());
        let _ = actualizar(&mut app, Accion::VisorPaso(1));
        assert!(old_image_cancel.is_cancelled());
        assert_eq!(app.servicios.ficha.visor.as_ref().unwrap().indice, 0);
        assert_eq!(app.servicios.ficha.visor.as_ref().unwrap().zoom, 100);
        let image_latest = app.servicios.ficha.visor_id;
        let _ = actualizar(
            &mut app,
            Accion::VisorRecibida(
                actual,
                old_image,
                "anterior".into(),
                Err("Foto anterior".into()),
            ),
        );
        assert!(app.servicios.ficha.visor_busy && app.servicios.ficha.visor_error.is_none());
        let _ = actualizar(
            &mut app,
            Accion::VisorRecibida(
                actual,
                image_latest,
                "actual".into(),
                Err("Imagen no disponible".into()),
            ),
        );
        assert!(!app.servicios.ficha.visor_busy && app.servicios.ficha.visor_error.is_some());
        let _ = capa(&app, text("Fondo").into());
        let _ = actualizar(&mut app, Accion::VisorPaso(-1));
        assert_eq!(app.servicios.ficha.visor.as_ref().unwrap().indice, 8);
        let closed_request = app.servicios.ficha.visor_id;
        let closed_cancel = app.servicios.ficha.visor_cancel.clone().unwrap();
        let _ = actualizar(&mut app, Accion::VisorCerrar);
        assert!(closed_cancel.is_cancelled() && app.servicios.ficha.visor.is_none());
        let _ = actualizar(
            &mut app,
            Accion::VisorRecibida(
                actual,
                closed_request,
                "cerrada".into(),
                Err("Respuesta después de cerrar".into()),
            ),
        );
        assert!(app.servicios.ficha.visor_error.is_none());
        let _ = actualizar(&mut app, Accion::CambiosVersion("v1".into()));
        let old = app.servicios.ficha.cambios_id;
        let old_cancel = app.servicios.ficha.cambios_cancel.clone().unwrap();
        let _ = actualizar(&mut app, Accion::CambiosSeleccionar("v2".into()));
        assert!(old_cancel.is_cancelled());
        let latest = app.servicios.ficha.cambios_id;
        let _ = actualizar(
            &mut app,
            Accion::CambiosRecibidos(actual, old, Ok(v1.clone())),
        );
        assert!(app.servicios.ficha.cambios_busy && app.servicios.ficha.cambio.is_none());
        let _ = actualizar(
            &mut app,
            Accion::CambiosRecibidos(actual, latest, Ok(v2.clone())),
        );
        assert_eq!(app.servicios.ficha.cambio.as_ref().unwrap().id, "v2");
        let _ = actualizar(&mut app, Accion::FichaPestana(ficha::Pestana::Versiones));
        let _ = actualizar(
            &mut app,
            Accion::VersionMinecraft("Todas las versiones".into()),
        );
        let request_old = app.servicios.ficha.versiones_id;
        let _ = actualizar(&mut app, Accion::VersionLoader("Todos los loaders".into()));
        let request_latest = app.servicios.ficha.versiones_id;
        let _ = actualizar(
            &mut app,
            Accion::VersionesRecibidas(actual, request_old, Ok(Vec::new())),
        );
        assert!(app.servicios.ficha.versiones_busy);
        let _ = actualizar(
            &mut app,
            Accion::VersionesRecibidas(actual, request_latest, Ok(vec![v1, v2])),
        );
        assert!(!app.servicios.ficha.versiones_busy);
        assert_eq!(app.servicios.detail.as_ref().unwrap().1.len(), 2);
        assert_eq!(context(&app).minecraft.as_deref(), Some("1.21.4"));
        assert_eq!(context(&app).loader.as_deref(), Some("Fabric"));
        for modo in [icaro_ui::tema::Modo::Piedra, icaro_ui::tema::Modo::Tinta] {
            app.modo = modo;
            for width in [940., 1280.] {
                app.ventana.width = width;
                for tab in [
                    ficha::Pestana::Descripcion,
                    ficha::Pestana::Galeria,
                    ficha::Pestana::Versiones,
                    ficha::Pestana::Cambios,
                    ficha::Pestana::Dependencias,
                ] {
                    app.servicios.ficha.tab = tab;
                    let _ = store_view(&app);
                }
            }
        }
        let _ = store_view(&app);
        // Una dependencia vuelve a su ficha exacta; después vuelve al catálogo.
        let original = app.servicios.detail.clone().unwrap();
        let original_tab = app.servicios.ficha.tab;
        let original_selection = app.servicios.ficha.seleccionada.clone();
        let _ = actualizar(&mut app, Accion::Proyecto("dependency".into()));
        assert_eq!(app.servicios.anteriores.len(), 1);
        let mut dependency = original.0.clone();
        dependency.id = "dependency".into();
        let request = app.servicios.search_id;
        let _ = actualizar(
            &mut app,
            Accion::ProyectoRecibido(request, Ok((dependency, vec![]))),
        );
        let _ = actualizar(&mut app, Accion::Volver);
        assert_eq!(app.servicios.detail.as_ref().unwrap().0.id, original.0.id);
        assert_eq!(app.servicios.ficha.tab, original_tab);
        assert_eq!(app.servicios.ficha.seleccionada, original_selection);
        let _ = actualizar(&mut app, Accion::Volver);
        let _ = actualizar(
            &mut app,
            Accion::MiembrosRecibidos(actual, Err("Respuesta anterior".into())),
        );
        assert!(app.servicios.ficha.miembros_error.is_none());
        assert!(app.servicios.detail.is_none() && app.servicios.results.is_some());
        assert_eq!(context(&app).minecraft.as_deref(), Some("1.21.4"));
        let _ = store_view(&app);
        let _ = actualizar(&mut app, Accion::LimpiarFiltros);
        assert!(context(&app).minecraft.is_none() && context(&app).loader.is_none());
        assert_eq!(context(&app).order, Order::Relevance);

        app.pantalla = None;
        for seccion in Seccion::TODAS {
            app.busqueda.abierta = true;
            let _ = crate::actualizar_estado(&mut app, Mensaje::AtajoSeccion(seccion));
            assert_eq!(app.seccion, seccion);
            assert!(app.pantalla.is_none() && !app.busqueda.abierta);
        }

        // Instancias reales: disco, editor, asistente y recuperación por identidad.
        let temp = std::env::temp_dir();
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = temp.join(format!(
            "icaro-instance-ui-test-{}-{suffix}",
            std::process::id()
        ));
        let store = icaro_core::instancias::Almacen::nuevo(&root);
        let first = store.crear("Mi mundo", "1.21.4", 1).unwrap();
        let second = store.crear("Otro mundo", "1.20.1", 2).unwrap();
        let mut services = Estado::nuevo(false);
        services.instancias.almacen = store.clone();
        services.instancias.cargar();
        let mut config_b = crate::config_por_defecto();
        config_b.memoria_max = 4096;
        let mut config_a = crate::config_por_defecto();
        config_a.memoria_max = 8192;
        let mut saved = crate::estado::Guardado::nuevo(
            &["Amigos".into()],
            &[None, Some("Amigos".into())],
            &[],
            &[config_b, config_a],
            &[],
        );
        saved.asociar(vec![second.id.clone(), first.id.clone()]);
        let (mut real, _) = crate::iniciar_con_guardado(
            Seccion::Instancias,
            false,
            None,
            false,
            services,
            Some(saved),
        );
        assert_eq!(crate::instancias().len(), 2);
        assert_eq!(crate::instancias()[0].nombre, "Mi mundo");
        assert_eq!(real.grupos, vec![Some("Amigos".into()), None]);
        assert_eq!(real.configs[0].memoria_max, 8192);
        assert_eq!(real.configs[1].memoria_max, 4096);
        assert!(real.seleccionada.is_none());
        real.pantalla = None;
        crate::abrir_instancia(&mut real, 0);
        assert!(real.general.servidor.is_none());
        let _ = crate::actualizar_estado(&mut real, Mensaje::Pestana(2));
        assert_eq!(real.editor, Some(0));
        assert!(!crate::mods_reales::permite_mods(&real));
        for tab in 0..8 {
            real.editor = Some(tab);
            let _ = crate::vista(&real);
        }
        let _ = crate::actualizar_estado(&mut real, Mensaje::Nombre("Renombrada".into()));
        let _ = crate::actualizar_estado(&mut real, Mensaje::Ir(Seccion::Instancias));
        assert_eq!(store.cargar().unwrap().instancias[0].nombre, "Renombrada");
        assert_eq!(real.servicios.instancias.registros[0].id, first.id);
        crate::abrir_instancia(&mut real, 0);
        assert_eq!(real.general.nombre, "Renombrada");
        let _ = crate::actualizar_estado(&mut real, Mensaje::VolverEditor);

        let _ = pantallas::actualizar(&mut real, pantallas::Accion::NuevaInstancia);
        real.servicios.instancias.versiones = serde_json::from_str::<icaro_core::manifest::VersionManifest>(
            r#"{"latest":{"release":"1.21.4","snapshot":"25w01a"},"versions":[{"id":"1.21.4","type":"release","url":"https://piston-meta.mojang.com/test","sha1":"test","releaseTime":"2024-12-03T10:00:00Z"},{"id":"25w01a","type":"snapshot","url":"https://piston-meta.mojang.com/test","sha1":"test","releaseTime":"2025-01-01T10:00:00Z"}]}"#,
        ).unwrap().versions;
        real.servicios.instancias.cargando = false;
        let mut loaders=icaro_core::versiones_instancia::Loaders::default();
        loaders.versiones[1]=vec!["0.16.9".into()];
        real.servicios.instancias.loaders.insert("1.21.4".into(),loaders);
        let _ = pantallas::actualizar(&mut real, pantallas::Accion::AsistenteVersion(999));
        assert_eq!(real.asistente.as_ref().unwrap().version, 0);
        let _ = pantallas::actualizar(&mut real, pantallas::Accion::AsistenteSiguiente);
        let _ = pantallas::actualizar(&mut real, pantallas::Accion::AsistenteLoader(1));
        assert_eq!(real.asistente.as_ref().unwrap().loader, 1);
        let _ = pantallas::actualizar(&mut real, pantallas::Accion::AsistenteSiguiente);
        assert_eq!(real.asistente.as_ref().unwrap().nombre, "Fabric 1.21.4");
        let _ = pantallas::actualizar(&mut real, pantallas::Accion::AsistentePortada(3));
        let _ = pantallas::actualizar(&mut real, pantallas::Accion::AsistenteSiguiente);
        let _ = crate::instancias_reales::crear(&mut real);
        assert!(real.servicios.instancias.creando);
        let _ = pantallas::actualizar(&mut real, pantallas::Accion::AsistenteCancelar);
        assert!(real.asistente.is_some());
        let selected = crate::instancias_reales::elegida(&real).unwrap();
        let created = store
            .crear_con_loader(
                &real.asistente.as_ref().unwrap().nombre,
                &selected,
                "fabric",
                3,
            )
            .unwrap();
        let _ = crate::actualizar_estado(
            &mut real,
            Mensaje::Instancias(crate::instancias_reales::Accion::Creada(
                Ok(created.clone()),
            )),
        );
        assert!(real.asistente.is_none() && !real.servicios.instancias.creando);
        assert_eq!(crate::instancias().len(), 3);
        assert_eq!(real.servicios.instancias.registros[2], created);
        assert_eq!(real.grupos.len(), 3);
        assert_eq!(real.configs.len(), 3);
        assert_eq!(store.cargar().unwrap().instancias.len(), 3);
        assert_eq!(crate::instancias()[2].estado, icaro_ui::instancias::EstadoInstancia::Preparando);
        let generation=real.servicios.juego.trabajos[&created.id].generacion;
        // Esta prueba no ejecuta tareas asíncronas: simula cancelación y su acuse.
        let _=crate::juego_real::actualizar(&mut real,crate::juego_real::Accion::Cancelar(0));
        let _=crate::juego_real::actualizar(&mut real,crate::juego_real::Accion::Preparada(created.id.clone(),generation,Err("Prueba cancelada".into())));
        assert!(crate::instancias()
            .iter()
            .all(|i| i.estado == icaro_ui::instancias::EstadoInstancia::Pendiente));
        // Catálogo de mods y versiones quedan limitados a la instancia Fabric.
        crate::abrir_instancia(&mut real, 2);
        let _ = crate::actualizar_estado(&mut real, Mensaje::Pestana(2));
        assert!(crate::mods_reales::permite_mods(&real));
        real.busqueda_mods = "mis mods".into();
        real.mods_filtro = 1;
        let _ = crate::actualizar_estado(&mut real, Mensaje::VistaMods(1));
        let filter = context(&real);
        assert_eq!(filter.minecraft.as_deref(), Some("1.21.4"));
        assert_eq!(filter.loader.as_deref(), Some("fabric"));
        real.editor = Some(8);
        let shader_filter = context(&real);
        assert_eq!(shader_filter.kind, Kind::Shader);
        assert_eq!(shader_filter.minecraft.as_deref(), Some("1.21.4"));
        assert!(shader_filter.loader.is_none(), "Los shaders no son mods de Fabric");
        real.editor = Some(2);
        real.servicios.ficha.minecraft = "1.21.4".into();
        real.servicios.ficha.loader = "Fabric".into();
        let _ = actualizar(&mut real, Accion::VersionMinecraft("1.20.1".into()));
        let _ = actualizar(&mut real, Accion::VersionLoader("Forge".into()));
        assert_eq!(real.servicios.ficha.minecraft, "1.21.4");
        assert_eq!(real.servicios.ficha.loader, "Fabric");
        let _ = crate::actualizar_estado(&mut real, Mensaje::VolverEditor);
        assert_eq!(real.editor, Some(2));
        assert_eq!(real.mods_vista, 0);
        assert_eq!(real.busqueda_mods, "mis mods");
        assert_eq!(real.mods_filtro, 1);
        let version: Version = serde_json::from_value(serde_json::json!({"id":"new", "project_id":"mod", "name":"Actualización", "version_number":"2", "version_type":"release", "game_versions":["1.21.4"], "loaders":["fabric"], "files":[], "dependencies":[]})).unwrap();
        let before = real.servicios.mods.busy.clone();
        let _ =
            crate::mods_reales::actualizar(&mut real, crate::mods_reales::Accion::ActualizarTodos);
        assert!(real.servicios.mods.confirmacion.is_none());
        crate::diseno_mods::comprobar_cesta(&mut real, version.clone());
        real.servicios
            .mods
            .updates
            .insert(created.id.clone(), vec![version.clone()]);
        let _ =
            crate::mods_reales::actualizar(&mut real, crate::mods_reales::Accion::ActualizarTodos);
        assert!(real.servicios.mods.confirmacion.is_some());
        assert_eq!(real.servicios.mods.busy, before);
        assert!(crate::hay_dialogo(&real));
        let _ = crate::vista(&real);
        let _ = crate::mods_reales::actualizar(&mut real, crate::mods_reales::Accion::Cancelar);
        assert!(real.servicios.mods.confirmacion.is_none());
        // Ver página conserva la ficha rápida; cerrarla devuelve a los mods.
        let _ = crate::actualizar_estado(&mut real, Mensaje::PanelMod(Some(original.0.id.clone())));
        let generation = real.servicios.search_id;
        let _ = actualizar(
            &mut real,
            Accion::ProyectoRecibido(generation, Ok((original.0.clone(), vec![version.clone()]))),
        );
        real.servicios.ficha.tab = ficha::Pestana::Versiones;
        let selected = real.servicios.ficha.seleccionada.clone();
        // La regla de la fila compatible debe caber en contenido desplazable,
        // también cuando su altura crece por la insignia de la instancia.
        let _ = ficha::vista(&real, &original.0, std::slice::from_ref(&version), Kind::Mod);
        let _ = crate::diseno_mods::actualizar(&mut real, crate::diseno_mods::Accion::VerPagina);
        assert!(real.mod_panel.is_none());
        let _ = actualizar(&mut real, Accion::Volver);
        assert_eq!(real.mod_panel.as_deref(), Some(original.0.id.as_str()));
        assert_eq!(real.mods_vista, 0, "Volver desde Ver página restaura la lista instalada, no la tienda");
        assert_eq!(real.editor, Some(2));
        assert_eq!(real.servicios.ficha.tab, ficha::Pestana::Versiones);
        assert_eq!(real.servicios.ficha.seleccionada, selected);
        let _ = crate::actualizar_estado(&mut real, Mensaje::PanelMod(None));
        assert!(real.mod_panel.is_none() && real.servicios.detail.is_none());
        assert_eq!(real.mods_vista, 0);
        assert_eq!(real.busqueda_mods, "mis mods");
        // El panel que sí se abrió desde la tienda conserva la tienda al volver.
        real.mods_vista = 1;
        real.mod_panel = Some(original.0.id.clone());
        real.servicios.detail = Some((original.0.clone(), vec![version.clone()]));
        let _ = ampliar_ficha(&mut real);
        let _ = actualizar(&mut real, Accion::Volver);
        assert_eq!(real.mods_vista, 1);
        assert_eq!(real.mod_panel.as_deref(), Some(original.0.id.as_str()));
        let _ = crate::actualizar_estado(&mut real, Mensaje::PanelMod(None));
        real.mods_vista = 0;
        // Cerrar una ficha con historial no reabre una dependencia anterior,
        // y las respuestas tardías no restauran el panel cerrado.
        real.mod_panel = Some(original.0.id.clone());
        real.servicios.detail = Some((original.0.clone(), vec![version.clone()]));
        real.servicios.query = "sodium".into();
        real.servicios.results = Some(Results { hits: vec![], offset: 20, limit: 20, total_hits: 24 });
        let _ = actualizar(&mut real, Accion::Proyecto("dependency".into()));
        let closed_generation = real.servicios.search_id;
        assert!(real.servicios.hay_anterior());
        let _ = actualizar(&mut real, Accion::CerrarPanel);
        assert!(real.mod_panel.is_none() && real.servicios.detail.is_none());
        assert!(!real.servicios.hay_anterior());
        assert_eq!(real.mods_vista, 0);
        assert_eq!(real.editor, Some(2));
        assert_eq!(real.servicios.query, "sodium");
        assert_eq!(real.servicios.results.as_ref().unwrap().offset, 20);
        let _ = actualizar(&mut real, Accion::ProyectoRecibido(closed_generation, Ok((original.0.clone(), vec![version.clone()]))));
        let _ = actualizar(&mut real, Accion::MiembrosRecibidos(closed_generation, Err("Anterior".into())));
        assert!(real.servicios.detail.is_none() && real.servicios.ficha.miembros_error.is_none());
        // Abrir un mod instalado directamente vuelve a la lista, sin buscar.
        let _ = crate::actualizar_estado(&mut real, Mensaje::AbrirModPagina("mod".into()));
        assert_eq!(real.mods_vista, 1);
        assert!(real.servicios.results.is_none());
        let _ = actualizar(&mut real, Accion::Volver);
        assert_eq!(real.mods_vista, 0);
        assert_eq!(real.editor, Some(2));
        let _ = crate::actualizar_estado(&mut real, Mensaje::VolverEditor);
        assert!(real.editor.is_none());
        // El menú elimina su propia instancia, aunque la selección sea otra.
        real.seleccionada = Some(0);
        real.menu=Some(1);
        let _=crate::actualizar_estado(&mut real,Mensaje::PedirEliminar);
        let seq=real.picar_solicitud;
        let _=crate::actualizar_estado(&mut real,Mensaje::Picar(crate::picar_real::Accion::Revisada(second.id.clone(),seq-1,Ok(crate::picar_real::Revision::default()))));
        assert!(real.picar_revision.is_none());
        let _=crate::actualizar_estado(&mut real,Mensaje::Picar(crate::picar_real::Accion::Revisada(second.id.clone(),seq,Ok(crate::picar_real::Revision{mundos:1,sin_respaldo:1}))));
        let _=crate::actualizar_estado(&mut real,Mensaje::PicarEmpezar);
        assert!(real.picar.is_none());
        let _=crate::actualizar_estado(&mut real,Mensaje::Eliminar);
        let _=crate::actualizar_estado(&mut real,Mensaje::PicarEmpezar);
        let start=real.picar.as_ref().unwrap().inicio;
        let _=crate::actualizar_estado(&mut real,Mensaje::PicarSoltar);
        let _=crate::actualizar_estado(&mut real,Mensaje::Cuadro(start+std::time::Duration::from_secs(2)));
        assert!(real.picar.is_none()&&!real.servicios.instancias.operando);
        let _=crate::actualizar_estado(&mut real,Mensaje::PicarEmpezar);
        let start=real.picar.as_ref().unwrap().inicio;
        let broken=start+std::time::Duration::from_secs(2);
        let _=crate::actualizar_estado(&mut real,Mensaje::Cuadro(broken));
        let _=crate::actualizar_estado(&mut real,Mensaje::Cuadro(broken+std::time::Duration::from_secs(2)));
        assert!(real.servicios.instancias.operando);
        let _=crate::actualizar_estado(&mut real,Mensaje::CerrarDialogo);
        assert_eq!(real.picar_objetivo,Some(1));
        store.eliminar(&second.id).unwrap();
        let _ = crate::actualizar_estado(
            &mut real,
            Mensaje::Instancias(crate::instancias_reales::Accion::Eliminada(
                second.id.clone(),
                Ok(()),
            )),
        );
        assert!(!real.dialogo && real.menu.is_none());
        assert_eq!(crate::instancias().len(), 2);
        assert_eq!(store.cargar().unwrap().instancias.len(), 2);
        assert_eq!(real.servicios.instancias.registros[1].id, created.id);
        assert_eq!(real.configs[0].memoria_max, 8192);
        assert_eq!(real.seleccionada, Some(0));
        assert!(real.avisos.entradas().last().unwrap().aviso.accion.is_some());
        let _ = crate::actualizar_estado(&mut real, Mensaje::DeshacerAviso);
        let restored = store.restaurar(&second.id).unwrap();
        let _ = crate::actualizar_estado(
            &mut real,
            Mensaje::Instancias(crate::instancias_reales::Accion::Restaurada(
                second.id.clone(),
                Ok(restored),
            )),
        );
        assert_eq!(crate::instancias().len(), 3);
        assert_eq!(real.servicios.instancias.registros[1].id, second.id);
        assert_eq!(real.configs[1].memoria_max, 4096);
        assert_eq!(real.grupos[0], Some("Amigos".into()));
        assert!(real.servicios.instancias.deshacer.is_none());
        assert_eq!(store.cargar().unwrap().instancias.len(), 3);
        real.seleccionada = Some(2);
        let _ = crate::actualizar_estado(&mut real, Mensaje::PicarEmpezar);
        assert!(real.picar.is_none());
        let seq=real.picar_solicitud;
        let _=crate::actualizar_estado(&mut real,Mensaje::Picar(crate::picar_real::Accion::Revisada(created.id.clone(),seq,Ok(crate::picar_real::Revision::default()))));
        let _=crate::actualizar_estado(&mut real,Mensaje::PicarEmpezar);
        let started = real.picar.as_ref().unwrap().inicio;
        let broken = started + std::time::Duration::from_secs(5);
        let _ = crate::actualizar_estado(&mut real, Mensaje::Cuadro(broken));
        assert!(real.picar.as_ref().unwrap().rota.is_some());
        assert!(!real.servicios.instancias.operando);
        let _ = crate::actualizar_estado(
            &mut real,
            Mensaje::Cuadro(broken + std::time::Duration::from_secs(2)),
        );
        assert!(real.servicios.instancias.operando);
        assert_eq!(
            real.servicios.instancias.eliminar.as_deref(),
            Some(created.id.as_str())
        );
        store.eliminar(&created.id).unwrap();
        let _ = crate::actualizar_estado(
            &mut real,
            Mensaje::Instancias(crate::instancias_reales::Accion::Eliminada(
                created.id.clone(),
                Ok(()),
            )),
        );
        assert!(real.picar.is_none() && real.seleccionada.is_none());
        assert_eq!(crate::instancias().len(), 2);
        assert_eq!(store.cargar().unwrap().instancias.len(), 2);
        assert_eq!(root.parent(), Some(temp.as_path()));
        std::fs::remove_dir_all(&root).unwrap();

        let (demo, _) = crate::iniciar_con_guardado(
            Seccion::Instancias,
            false,
            None,
            false,
            Estado::nuevo(true),
            None,
        );
        assert_eq!(crate::instancias().len(), 5);
        assert!(!demo.red.servidores.is_empty() && demo.red.hay_cola());
        assert!(!demo.capt.capturas.is_empty());
        assert!(!crate::busqueda::resultados(&demo).is_empty());
        crate::lista_instancias().lock().unwrap().clear();
    }
    #[test]
    fn no_demo_account_is_reported_as_authenticated() {
        let mut state = Estado::nuevo(false);
        assert_eq!(state.nombre("Mineral_7"), "Recuperando sesión…");
        assert!(state.session.is_none());
        state.cancel();
        assert_eq!(state.nombre("Mineral_7"), "Sin cuenta");
    }
}
