// SPDX-License-Identifier: GPL-3.0-only
//! Lista real de servidores con formularios y consultas independientes de la maqueta.
use crate::{App, Mensaje};
use icaro_core::{
    servidores::{self, Servidor},
    zerotier,
};
use icaro_ui::{
    componentes::{self, Variante},
    fuentes,
    iconos::Icono,
    laminas::{banda_completa, DatoBanda, Lamina, ALTO_BANDA},
    servidores::{AccionesLista, EstadoServidor},
};
use iced::{
    widget::{column, container, pick_list, row, text, text_input, Space},
    Alignment, Element, Length, Task,
};
use std::collections::BTreeMap;
use tokio_util::sync::CancellationToken;

#[derive(Default)]
pub struct Estado {
    loaded: bool,
    loading: bool,
    saving: bool,
    revision: u64,
    request: u64,
    zero_request: u64,
    pub(crate) registros: Vec<Servidor>,
    estados: BTreeMap<String, Result<servidores::Estado, String>>,
    cancel: Option<CancellationToken>,
    error: Option<String>,
    editor: Option<Editor>,
    pub visual: bool,
    zero: Option<zerotier::Estado>,
    pub arrastre: Option<icaro_ui::servidores::Arrastre>,
    sobre: Option<usize>,
    toma_y: f32,
    cerrando: Option<std::time::Instant>,
}
impl Drop for Estado {
    fn drop(&mut self) {
        if let Some(c) = &self.cancel {
            c.cancel();
        }
    }
}
impl Estado {
    pub fn guardados(&self) -> &[Servidor] {
        &self.registros
    }
    pub fn panel_abierto(&self) -> bool {
        self.editor.is_some()
    }
    pub fn mover(&mut self, y: f32) {
        if let Some(a) = &mut self.arrastre {
            let delta = ((y - self.toma_y)
                / (icaro_ui::servidores::ALTO_FILA + icaro_ui::servidores::SEPARACION))
                .round() as i64;
            a.destino = (a.origen as i64 + delta)
                .clamp(0, self.registros.len().saturating_sub(1) as i64)
                as usize;
        }
    }
    pub fn avanzar(&mut self) {
        if self
            .cerrando
            .is_some_and(|t| t.elapsed().as_millis() >= icaro_ui::tema::duracion::FAST as u128)
        {
            self.editor = None;
            self.cerrando = None;
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instancia {
    id: String,
    nombre: String,
}
impl std::fmt::Display for Instancia {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.nombre)
    }
}
struct Editor {
    id: Option<String>,
    nombre: String,
    direccion: String,
    instancia: Option<Instancia>,
    error: Option<String>,
    sincronizar: bool,
}
#[derive(Clone, Debug)]
pub enum Accion {
    Abrir,
    Cargados(Result<Vec<Servidor>, String>),
    Consultar,
    ConsultarZero,
    Estado(u64, String, Result<servidores::Estado, String>),
    Zero(u64, zerotier::Estado),
    Nuevo,
    Editar(usize),
    Nombre(String),
    Direccion(String),
    Instancia(Instancia),
    Sincronizar(bool),
    Guardar,
    Guardados(u64, Result<Vec<Servidor>, String>),
    Cerrar,
    Jugar(usize),
    Tomar(usize),
    Sobre(Option<usize>),
    Soltar,
    CancelarOrden,
    GestionarRed,
    EliminarPanel,
}
fn m(a: Accion) -> Mensaje {
    Mensaje::Servidores(a)
}
fn recargar_instancias(app: &mut App) {
    if !app.servicios.servidores.visual {
        if let Err(error) = crate::instancias_reales::recargar(app) {
            app.avisar(error);
        } else if let Some(editor) = &mut app.servicios.servidores.editor {
            if let Some(elegida) = &editor.instancia {
                editor.instancia = app.servicios.instancias.registros.iter().find(|i| i.id == elegida.id)
                    .map(|i| Instancia { id: i.id.clone(), nombre: i.nombre.clone() });
            }
        }
    }
}
fn persistir(app: &mut App, v: Vec<Servidor>) -> Task<Mensaje> {
    let s = &mut app.servicios.servidores;
    if s.saving || s.error.is_some() {
        return Task::none();
    }
    s.saving = true;
    s.revision += 1;
    let g = s.revision;
    let root = crate::estado::directorio(false);
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                servidores::guardar(&root, &v)?;
                Ok(v)
            })
            .await
            .map_err(|_| "No se pudo guardar la lista.".to_string())
            .and_then(|r| r)
        },
        move |r| m(Accion::Guardados(g, r)),
    )
}
pub fn actualizar(app: &mut App, a: Accion) -> Task<Mensaje> {
    if app.servicios.demo {
        return Task::none();
    }
    if app.servicios.servidores.saving
        && !matches!(
            a,
            Accion::Guardados(..) | Accion::Estado(..) | Accion::Zero(..) | Accion::ConsultarZero
        )
    {
        return Task::none();
    }
    match a {
        Accion::Abrir => {
            recargar_instancias(app);
            let s = &mut app.servicios.servidores;
            if s.loading {
                return Task::none();
            }
            if s.editor.is_some() || s.arrastre.is_some() {
                return actualizar(app, Accion::Consultar);
            }
            s.loading = true;
            let root = crate::estado::directorio(false);
            return Task::perform(
                async move {
                    tokio::task::spawn_blocking(move || servidores::cargar(&root))
                        .await
                        .map_err(|_| "No se pudo leer la lista.".to_string())
                        .and_then(|r| r)
                },
                |r| m(Accion::Cargados(r)),
            );
        }
        Accion::Cargados(r) => {
            let s = &mut app.servicios.servidores;
            s.loading = false;
            s.loaded = true;
            match r {
                Ok(v) => {
                    s.registros = v;
                    s.estados.retain(|id, _| s.registros.iter().any(|v| &v.id == id));
                    s.error = None;
                }
                Err(e) => {
                    s.error = Some(e);
                }
            }
            return actualizar(app, Accion::Consultar);
        }
        Accion::Consultar => {
            let s = &mut app.servicios.servidores;
            if s.loading || !s.loaded {
                return Task::none();
            }
            if let Some(c) = s.cancel.take() {
                c.cancel();
            }
            s.request += 1;
            let g = s.request;
            let c = CancellationToken::new();
            s.cancel = Some(c.clone());
            let slots = std::sync::Arc::new(tokio::sync::Semaphore::new(4));
            let mut tasks = vec![];
            for server in s.registros.clone() {
                let c = c.clone();
                let slots = slots.clone();
                tasks.push(Task::perform(async move{
                    tokio::select!{
                        _=c.cancelled()=>return Err("Consulta cancelada.".into()),
                        permit=slots.acquire_owned()=>{let _permit=permit.map_err(|_|"Consulta cerrada.")?;servidores::consultar(&server.direccion,c).await}
                    }
                },move|r|m(Accion::Estado(g,server.id.clone(),r))));
            }
            tasks.push(actualizar(app, Accion::ConsultarZero));
            return Task::batch(tasks);
        }
        Accion::ConsultarZero => {
            let s = &mut app.servicios.servidores;
            s.zero_request += 1;
            let g = s.zero_request;
            return Task::perform(zerotier::detectar(), move |z| m(Accion::Zero(g, z)));
        }
        Accion::Estado(g, id, r) => {
            let s = &mut app.servicios.servidores;
            if g == s.request && s.registros.iter().any(|v| v.id == id) {
                s.estados.insert(id, r);
            }
        }
        Accion::Zero(g, z) => {
            let s = &mut app.servicios.servidores;
            if g == s.zero_request {
                s.zero = Some(z);
            }
        }
        Accion::Nuevo => {
            if app.servicios.servidores.error.is_some() {
                return Task::none();
            }
            recargar_instancias(app);
            app.servicios.servidores.editor = Some(Editor {
                id: None,
                nombre: String::new(),
                direccion: String::new(),
                instancia: None,
                error: None,
                sincronizar: false,
            });
            app.mod_panel = None;
            app.anim_panel = Some(std::time::Instant::now());
            app.servicios.servidores.cerrando = None;
            return iced::widget::text_input::focus(iced::widget::text_input::Id::new(
                "servidor-nombre",
            ));
        }
        Accion::Editar(i) => {
            recargar_instancias(app);
            if let Some(v) = app.servicios.servidores.registros.get(i) {
                let instancia = v
                    .instancia
                    .as_ref()
                    .and_then(|id| {
                        app.servicios
                            .instancias
                            .registros
                            .iter()
                            .find(|i| &i.id == id)
                    })
                    .map(|i| Instancia {
                        id: i.id.clone(),
                        nombre: i.nombre.clone(),
                    });
                app.servicios.servidores.editor = Some(Editor {
                    id: Some(v.id.clone()),
                    nombre: v.nombre.clone(),
                    direccion: v.direccion.clone(),
                    instancia,
                    error: None,
                    sincronizar: v.sincronizar,
                });
                app.mod_panel = None;
                app.anim_panel = Some(std::time::Instant::now());
                app.servicios.servidores.cerrando = None;
                return iced::widget::text_input::focus(iced::widget::text_input::Id::new(
                    "servidor-nombre",
                ));
            }
        }
        Accion::Nombre(v) => {
            if let Some(e) = &mut app.servicios.servidores.editor {
                e.nombre = v;
            }
        }
        Accion::Direccion(v) => {
            if let Some(e) = &mut app.servicios.servidores.editor {
                e.direccion = v;
            }
        }
        Accion::Instancia(v) => {
            if let Some(e) = &mut app.servicios.servidores.editor {
                e.instancia = Some(v);
            }
        }
        Accion::Sincronizar(v) => {
            if let Some(e) = &mut app.servicios.servidores.editor {
                e.sincronizar = v;
            }
        }
        Accion::Cerrar => {
            let s = &mut app.servicios.servidores;
            if app.reducir {
                s.editor = None;
            } else {
                s.cerrando = Some(std::time::Instant::now());
            }
            app.anim_panel = Some(std::time::Instant::now());
        }
        Accion::Guardar => {
            let Some(e) = &mut app.servicios.servidores.editor else {
                return Task::none();
            };
            match servidores::nuevo(
                e.nombre.clone(),
                e.direccion.clone(),
                e.instancia.as_ref().map(|i| i.id.clone()),
            ) {
                Err(error) => e.error = Some(error),
                Ok(mut v) => {
                    v.sincronizar = e.sincronizar;
                    let mut lista = app.servicios.servidores.registros.clone();
                    if let Some(id) = &e.id {
                        v.id = id.clone();
                        let Some(old) = lista.iter_mut().find(|v| &v.id == id) else {
                            return Task::none();
                        };
                        *old = v;
                    } else {
                        lista.push(v);
                    }
                    return persistir(app, lista);
                }
            }
        }
        Accion::Guardados(g, r) => {
            let s = &mut app.servicios.servidores;
            if g != s.revision {
                return Task::none();
            }
            s.saving = false;
            match r {
                Ok(v) => {
                    s.registros = v;
                    s.editor = None;
                    app.avisar("Lista de servidores guardada.".into());
                    return actualizar(app, Accion::Consultar);
                }
                Err(e) => {
                    if let Some(editor) = &mut s.editor {
                        editor.error = Some(e);
                    } else {
                        app.avisar(format!("No se pudo guardar: {e}"));
                    }
                }
            }
        }
        Accion::GestionarRed => {
            app.servicios.servidores.editor = None;
            app.pantalla = Some(crate::pantallas::Pantalla::Red);
            return crate::red_privada_real::actualizar(
                app,
                crate::red_privada_real::Accion::Abrir,
            );
        }
        Accion::EliminarPanel => {
            let s = &app.servicios.servidores;
            let Some(id) = s.editor.as_ref().and_then(|e| e.id.as_ref()) else {
                return Task::none();
            };
            return persistir(
                app,
                s.registros
                    .iter()
                    .filter(|v| &v.id != id)
                    .cloned()
                    .collect(),
            );
        }
        Accion::Tomar(i) => {
            let s = &mut app.servicios.servidores;
            if i < s.registros.len() && s.editor.is_none() {
                s.arrastre = Some(icaro_ui::servidores::Arrastre {
                    origen: i,
                    destino: i,
                });
                s.toma_y = app.cursor.y;
            }
        }
        Accion::Sobre(i) => app.servicios.servidores.sobre = i,
        Accion::CancelarOrden => app.servicios.servidores.arrastre = None,
        Accion::Soltar => {
            let s = &mut app.servicios.servidores;
            if let Some(drag) = s.arrastre.take() {
                if drag.origen < s.registros.len()
                    && drag.destino < s.registros.len()
                    && drag.origen != drag.destino
                {
                    let mut lista = s.registros.clone();
                    let v = lista.remove(drag.origen);
                    lista.insert(drag.destino, v);
                    return persistir(app, lista);
                }
            }
        }
        Accion::Jugar(i) => {
            let Some(v) = app.servicios.servidores.registros.get(i).cloned() else {
                return Task::none();
            };
            let Some(index) = v.instancia.as_ref().and_then(|id| {
                app.servicios
                    .instancias
                    .registros
                    .iter()
                    .position(|i| &i.id == id)
            }) else {
                app.avisar("Vincula una instancia al servidor antes de jugar.".into());
                return actualizar(app, Accion::Editar(i));
            };
            if let Some(Ok(status)) = app.servicios.servidores.estados.get(&v.id) {
                let mc = &app.servicios.instancias.registros[index].minecraft;
                // Solo identifica nombres de versiones exactos; proxies pueden publicar etiquetas propias.
                if status.version != *mc
                    && app
                        .servicios
                        .instancias
                        .versiones
                        .iter()
                        .any(|version| version.id == status.version)
                {
                    app.avisar(format!("El servidor anuncia Minecraft {} y la instancia usa {mc}. Cambia la versión o vincula otra instancia.",status.version));
                    return Task::none();
                }
            }
            if v.sincronizar {
                return crate::sincronizacion_real::actualizar(
                    app,
                    crate::sincronizacion_real::Accion::Abrir(i),
                );
            }
            return crate::juego_real::iniciar_con_servidor(app, index, true, Some(v.direccion));
        }
    }
    Task::none()
}
pub fn vista(app: &App) -> Element<'_, Mensaje> {
    let p = app.paleta();
    let s = &app.servicios.servidores;
    let mut body = column![row![
        text("Arrastra para ordenar").size(13).color(p.text_muted),
        Space::with_width(Length::Fill),
        componentes::boton_con_icono(
            p,
            Icono::Mas,
            "Agregar servidor",
            Variante::Primario,
            (!s.saving && s.loaded && s.error.is_none()).then(|| m(Accion::Nuevo)),
            false
        )
    ]
    .spacing(12)
    .align_y(Alignment::Center)]
    .spacing(24);
    let (connected, name, ip) = match &s.zero {
        Some(zerotier::Estado::Disponible { redes, .. }) => redes
            .iter()
            .find(|r| r.estado == "OK")
            .map(|r| {
                (
                    true,
                    r.nombre.clone(),
                    r.ips
                        .first()
                        .map(|ip| ip.split('/').next().unwrap_or(ip).to_owned())
                        .unwrap_or_default(),
                )
            })
            .unwrap_or((
                false,
                if redes.is_empty() {
                    "Sin red configurada".into()
                } else {
                    "Esperando autorización".into()
                },
                String::new(),
            )),
        Some(zerotier::Estado::Ausente) => {
            (false, "ZeroTier no está instalado".into(), String::new())
        }
        Some(zerotier::Estado::SinAcceso) => (
            false,
            "El servicio de ZeroTier no responde".into(),
            String::new(),
        ),
        None => (false, "Comprobando ZeroTier…".into(), String::new()),
    };
    let network = icaro_ui::servidores::red_privada(
        p,
        connected,
        &name,
        &ip,
        (0, 0),
        m(Accion::GestionarRed),
    );
    body = if app.servicios.compartir.habilitado && !app.servicios.demo {
        column![crate::compartir_real::accesos(app), network].spacing(24)
    } else {column![network, body].spacing(24)};
    if let Some(error) = &s.error {
        body = body.push(
            column![
                text(error.clone()).color(p.error),
                componentes::boton(
                    p,
                    "Releer lista",
                    Variante::Secundario,
                    (!s.loading && !s.saving).then(|| m(Accion::Abrir))
                )
            ]
            .spacing(12),
        );
    }
    if s.loading {
        body = body.push(text("Cargando servidores…").color(p.text_muted));
    } else if s.registros.is_empty() && s.error.is_none() {
        body = body.push(icaro_ui::vacios::servidores(
            icaro_ui::vacios::Contexto {
                p,
                ventana: app.ventana.width,
                prensa: 1.,
            },
            m(Accion::Nuevo),
        ));
    }
    let acciones = AccionesLista {
        jugar: |i| m(Accion::Jugar(i)),
        tomar: Some(|i| m(Accion::Tomar(i))),
        sobre: |i| m(Accion::Sobre(i)),
        editar: Some(|i| m(Accion::Editar(i))),
        eliminar: None,
        permitir_sin_estado: false,
    };
    let filas: Vec<_> = s
        .registros
        .iter()
        .map(|v| {
            let status = s.estados.get(&v.id);
            let instance = v.instancia.as_ref().and_then(|id| {
                app.servicios
                    .instancias
                    .registros
                    .iter()
                    .find(|i| &i.id == id)
            });
            icaro_ui::servidores::Servidor {
                nombre: v.nombre.clone(),
                direccion: v.direccion.clone(),
                motd: status
                    .and_then(|r| r.as_ref().ok())
                    .map(|r| r.motd.replace('\n', " ")),
                estado: match status {
                    Some(Ok(e)) => EstadoServidor::EnLinea {
                        jugadores: e.jugadores,
                        maximo: e.maximo,
                        ms: e.ms.min(u32::MAX as u64) as u32,
                    },
                    Some(Err(_)) => EstadoServidor::FueraDeLinea("Sin respuesta".into()),
                    None => EstadoServidor::FueraDeLinea("Consultando…".into()),
                },
                vinculo: instance.map(|i| (i.nombre.clone(), v.sincronizar)),
                version_distinta: status
                    .and_then(|e| e.as_ref().ok())
                    .filter(|e| {
                        instance.is_some_and(|i| i.minecraft != e.version)
                            && app
                                .servicios
                                .instancias
                                .versiones
                                .iter()
                                .any(|v| v.id == e.version)
                    })
                    .map(|e| e.version.clone()),
            }
        })
        .collect();
    body = body.push(icaro_ui::servidores::lista_servidores(
        p, &filas, &acciones, s.arrastre, s.sobre, None,
    ));
    column![
        banda_completa(
            p,
            Lamina::TivoliCiudad,
            Some((0.5, 0.4)),
            "Servidores",
            DatoBanda::Conteo(s.registros.len()),
            ALTO_BANDA,
            icaro_ui::tema::texto::DISPLAY_XL
        ),
        crate::suave(app, body.padding(32))
    ]
    .into()
}
pub fn panel(app: &App) -> Option<Element<'_, Mensaje>> {
    use iced::widget::{mouse_area, opaque};
    let s = &app.servicios.servidores;
    let e = s.editor.as_ref()?;
    if app.seccion != icaro_ui::shell::Seccion::Servidores || app.pantalla.is_some() {
        return None;
    }
    let p = app.paleta();
    let enabled = !s.saving && s.cerrando.is_none();
    let field = |placeholder: &str, value: &str, f: fn(String) -> Accion, id: &str| {
        let input = text_input(placeholder, value)
            .id(iced::widget::text_input::Id::new(id.to_owned()))
            .padding(12)
            .style(icaro_ui::estilo::campo(p));
        if enabled {
            input.on_input(move |v| m(f(v)))
        } else {
            input
        }
    };
    let instances: Vec<_> = app
        .servicios
        .instancias
        .registros
        .iter()
        .map(|i| Instancia {
            id: i.id.clone(),
            nombre: i.nombre.clone(),
        })
        .collect();
    let mut body = column![
        componentes::etiqueta(p, "Nombre"),
        field("Amigos", &e.nombre, Accion::Nombre, "servidor-nombre"),
        componentes::etiqueta(p, "Dirección"),
        field(
            "10.147.17.1:25565",
            &e.direccion,
            Accion::Direccion,
            "servidor-direccion"
        )
        .font(fuentes::MONO),
        componentes::etiqueta(p, "Instancia vinculada"),
        pick_list(instances, e.instancia.clone(), |v| m(Accion::Instancia(v)))
            .placeholder("Selecciona una instancia")
            .padding(12)
            .width(Length::Fill)
            .style(icaro_ui::estilo::selector(p))
            .menu_style(icaro_ui::estilo::menu_selector(p)),
        iced::widget::checkbox("Sincronizar mods al iniciar", e.sincronizar)
            .on_toggle_maybe(enabled.then_some(|v| m(Accion::Sincronizar(v))))
            .icon(componentes::icono_casilla())
            .style(icaro_ui::estilo::casilla(p)),
    ]
    .spacing(12)
    .padding(24);
    if let Some(error) = &e.error {
        body = body.push(text(error.clone()).size(13).color(p.error));
    }
    let rule = || {
        container(Space::with_height(1))
            .width(Length::Fill)
            .style(icaro_ui::estilo::bloque(p.border))
    };
    let header = row![
        text(if e.id.is_some() {
            "Editar servidor"
        } else {
            "Agregar servidor"
        })
        .font(fuentes::TITULO)
        .size(18)
        .width(Length::Fill),
        componentes::boton_con_icono(
            p,
            Icono::Cerrar,
            "",
            Variante::Fantasma,
            enabled.then(|| m(Accion::Cerrar)),
            true
        )
    ]
    .align_y(Alignment::Center)
    .padding(20);
    let footer = row![
        componentes::boton(
            p,
            "Eliminar",
            Variante::Peligro,
            (enabled && e.id.is_some()).then(|| m(Accion::EliminarPanel))
        ),
        Space::with_width(Length::Fill),
        componentes::boton(
            p,
            "Cancelar",
            Variante::Secundario,
            enabled.then(|| m(Accion::Cerrar))
        ),
        componentes::boton(
            p,
            if s.saving { "Guardando…" } else { "Guardar" },
            Variante::Primario,
            enabled.then(|| m(Accion::Guardar))
        )
    ]
    .spacing(8)
    .padding(16)
    .align_y(Alignment::Center);
    let content = container(
        column![
            header,
            rule(),
            crate::desplazable_pub(body.into()),
            rule(),
            footer
        ]
        .height(Length::Fill),
    )
    .height(Length::Fill)
    .width(398)
    .style(icaro_ui::estilo::bloque(p.surface_raised));
    let line = container(Space::with_width(2))
        .height(Length::Fill)
        .style(icaro_ui::estilo::bloque(p.text));
    let t = if app.reducir {
        0.
    } else if let Some(t) = s.cerrando {
        (t.elapsed().as_millis() as f32 / icaro_ui::tema::duracion::FAST as f32).clamp(0., 1.)
    } else {
        1. - icaro_ui::movimiento::Curva::Salida.aplicar(crate::progreso(
            app.anim_panel,
            app.ahora,
            crate::MS_PANEL,
        ))
    };
    let pane = icaro_ui::efectos::panel_desde_derecha(
        opaque(row![line, content].width(400).height(Length::Fill)).into(),
        t * 400.,
    );
    let capa: Element<_> = if app.ventana.width < 1100. {
        let scrim = mouse_area(
            container(Space::new(Length::Fill, Length::Fill))
                .width(Length::Fill)
                .height(Length::Fill)
                .style(move |_| iced::widget::container::Style {
                    background: Some(p.scrim.into()),
                    ..Default::default()
                }),
        )
        .on_press(m(Accion::Cerrar));
        iced::widget::stack![scrim, pane].into()
    } else {
        pane
    };
    Some(
        container(capa)
            .padding(iced::Padding {
                top: 38.,
                ..iced::Padding::ZERO
            })
            .width(Length::Fill)
            .height(Length::Fill)
            .into(),
    )
}
pub fn suscripcion(app: &App) -> iced::Subscription<Mensaje> {
    if app.servicios.demo {
        return iced::Subscription::none();
    }
    if app.seccion == icaro_ui::shell::Seccion::Servidores
        && app.pantalla.is_none()
        && app.servicios.servidores.arrastre.is_none()
    {
        iced::Subscription::batch([
            iced::time::every(std::time::Duration::from_secs(30)).map(|_| m(Accion::Abrir)),
            iced::time::every(std::time::Duration::from_secs(10)).map(|_| m(Accion::ConsultarZero)),
        ])
    } else {
        iced::Subscription::none()
    }
}
pub fn preparar_diseno(app: &mut App, scene: &str) {
    if scene.ends_with("angosto") {
        app.ventana = iced::Size::new(940., 720.);
    }
    app.seccion = icaro_ui::shell::Seccion::Servidores;
    app.editor = None;
    let i = icaro_core::instancias::Instancia {
        id: "qa-servidores".into(),
        nombre: "Supervivencia".into(),
        minecraft: "26.3".into(),
        loader: "fabric".into(),
        portada: 0,
        creada: 0,
    };
    app.servicios.instancias.registros = vec![i.clone()];
    let s = &mut app.servicios.servidores;
    s.visual = true;
    s.loaded = true;
    if scene == "servidores-vacio" {
        s.zero = Some(zerotier::Estado::Ausente);
        return;
    }
    let a = servidores::nuevo(
        "Amigos".into(),
        "10.147.17.1:25565".into(),
        Some(i.id.clone()),
    )
    .unwrap();
    let b = servidores::nuevo(
        "Creativo de la U".into(),
        "play.example.net:25570".into(),
        None,
    )
    .unwrap();
    s.estados.insert(
        a.id.clone(),
        Ok(servidores::Estado {
            version: "26.3".into(),
            protocolo: 999,
            jugadores: 3,
            maximo: 20,
            motd: "Temporada 4: el nether está abierto".into(),
            ms: 24,
        }),
    );
    s.estados.insert(
        b.id.clone(),
        Err("No se pudo conectar; revisa la dirección, la red o ZeroTier.".into()),
    );
    s.registros = vec![a, b];
    s.zero = Some(zerotier::Estado::Disponible {
        version: "1.16.0".into(),
        online: true,
        redes: vec![zerotier::Red {
            id: "1234567890abcdef".into(),
            nombre: "Amigos".into(),
            estado: "ACCESS_DENIED".into(),
            ips: vec![],
        }],
    });
    if scene.starts_with("servidores-formulario") {
        s.editor = Some(Editor {
            id: None,
            nombre: "Servidor con amigos".into(),
            direccion: "play.example.net".into(),
            instancia: Some(Instancia {
                id: i.id,
                nombre: i.nombre,
            }),
            error: Some("Ese servidor ya está guardado.".into()),
            sincronizar: false,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct DatosTemporales(std::path::PathBuf);
    impl DatosTemporales {
        fn nuevos() -> Self {
            let id = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
            Self(std::env::temp_dir().join(format!("icaro-selector-servidores-{}-{id}", std::process::id())))
        }
    }
    impl Drop for DatosTemporales {
        fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
    }
    #[test]
    fn selector_recarga_instancias_del_disco_y_conserva_java_por_identidad() {
        let temp = DatosTemporales::nuevos();
        let store = icaro_core::instancias::Almacen::nuevo(temp.0.clone());
        let primera = store.crear_con_loader("Primera", "1.21.1", "fabric", 0).unwrap();
        let mut a = app();
        a.servicios.servidores.visual = false;
        a.servicios.instancias.almacen = store.clone();
        a.servicios.instancias.registros = vec![primera.clone()];
        *crate::lista_instancias().lock().unwrap() = vec![crate::instancias_reales::Estado::datos(&store, &primera)];
        a.configs = vec![crate::config_por_defecto()];
        a.configs[0].memoria_max = 6144;
        a.grupos = vec![Some("Amigos".into())];
        a.seleccionada = Some(0);
        // Simula una instancia creada por otra copia del launcher después del inicio.
        let segunda = store.crear_con_loader("Segunda", "26.2", "fabric", 0).unwrap();
        store.guardar_configuracion(&segunda.id, &serde_json::json!({"memoria_max":8192})).unwrap();
        let _ = actualizar(&mut a, Accion::Nuevo);
        assert_eq!(a.servicios.instancias.registros.len(), 2);
        let n = a.servicios.instancias.registros.iter().position(|i| i.id == primera.id).unwrap();
        assert_eq!(a.configs[n].memoria_max, 6144);
        assert_eq!(a.grupos[n].as_deref(), Some("Amigos"));
        assert_eq!(a.seleccionada, Some(n));
        let n = a.servicios.instancias.registros.iter().position(|i| i.id == segunda.id).unwrap();
        assert_eq!(a.configs[n].memoria_max, 8192);
        let _ = actualizar(&mut a, Accion::Instancia(Instancia { id: segunda.id.clone(), nombre: segunda.nombre }));
        assert_eq!(a.servicios.servidores.editor.as_ref().unwrap().instancia.as_ref().unwrap().id, segunda.id);
    }
    #[test]
    fn refresco_conserva_el_estado_y_la_edicion_sin_guardar() {
        let mut a = app();
        let _ = actualizar(&mut a, Accion::Editar(0));
        let _ = actualizar(&mut a, Accion::Nombre("Nombre sin guardar".into()));
        let estados = a.servicios.servidores.estados.len();
        let _ = actualizar(&mut a, Accion::Abrir);
        assert_eq!(a.servicios.servidores.editor.as_ref().unwrap().nombre, "Nombre sin guardar");
        assert_eq!(a.servicios.servidores.estados.len(), estados);
        assert!(a.servicios.servidores.zero.is_some());
        assert!(!a.servicios.servidores.loading);
    }
    #[test]
    fn al_reabrir_se_relee_la_lista_y_se_descarta_el_estado_antiguo_de_zerotier() {
        let mut a = app();
        let _ = actualizar(&mut a, Accion::Abrir);
        assert!(a.servicios.servidores.loading);
        a.servicios.servidores.zero_request = 4;
        let _ = actualizar(&mut a, Accion::Zero(3, zerotier::Estado::Ausente));
        assert!(matches!(a.servicios.servidores.zero, Some(zerotier::Estado::Disponible { .. })));
        let _ = actualizar(&mut a, Accion::Zero(4, zerotier::Estado::Ausente));
        assert!(matches!(a.servicios.servidores.zero, Some(zerotier::Estado::Ausente)));
    }
    fn app() -> App {
        let (mut a, _) = crate::iniciar_con_guardado(
            icaro_ui::shell::Seccion::Servidores,
            true,
            None,
            false,
            crate::servicios::Estado::nuevo(true),
            None,
        );
        preparar_diseno(&mut a, "servidores");
        a.servicios.demo = false;
        a
    }
    #[test]
    fn respuestas_antiguas_y_servidores_eliminados_no_regresan_a_la_lista() {
        let mut a = app();
        let id = a.servicios.servidores.registros[0].id.clone();
        a.servicios.servidores.request = 7;
        let _ = actualizar(&mut a, Accion::Estado(6, id.clone(), Err("antiguo".into())));
        assert!(a.servicios.servidores.estados[&id].is_ok());
        a.servicios.servidores.registros.remove(0);
        a.servicios.servidores.estados.remove(&id);
        let _ = actualizar(
            &mut a,
            Accion::Estado(7, id.clone(), Err("eliminado".into())),
        );
        assert!(!a.servicios.servidores.estados.contains_key(&id));
    }
    #[test]
    fn fallo_de_guardado_conserva_la_lista_y_no_cierra_el_editor() {
        let mut a = app();
        let previous = a.servicios.servidores.registros.clone();
        let _ = actualizar(&mut a, Accion::Editar(0));
        a.servicios.servidores.saving = true;
        a.servicios.servidores.revision = 4;
        let _ = actualizar(&mut a, Accion::Guardados(4, Err("Disco ocupado".into())));
        assert_eq!(a.servicios.servidores.registros, previous);
        assert!(a
            .servicios
            .servidores
            .editor
            .as_ref()
            .unwrap()
            .error
            .is_some());
        assert!(!a.servicios.servidores.saving);
    }
    #[test]
    fn la_maqueta_no_guarda_servidores_ni_lanza_el_juego() {
        let mut a = app();
        a.servicios.demo = true;
        let _ = actualizar(&mut a, Accion::Jugar(0));
        assert!(a.servicios.juego.trabajos.is_empty());
        let _ = actualizar(&mut a, Accion::Nuevo);
        assert!(a.servicios.servidores.editor.is_none());
    }
    #[test]
    fn arrastre_cancelado_conserva_el_orden_y_espera_el_guardado() {
        let mut a = app();
        a.cursor.y = 100.;
        let original = a.servicios.servidores.registros.clone();
        let _ = actualizar(&mut a, Accion::Tomar(0));
        a.servicios.servidores.mover(194.);
        assert_eq!(a.servicios.servidores.arrastre.unwrap().destino, 1);
        let _ = actualizar(&mut a, Accion::CancelarOrden);
        assert_eq!(a.servicios.servidores.registros, original);
        let _ = actualizar(&mut a, Accion::Tomar(0));
        a.servicios.servidores.mover(194.);
        let _ = actualizar(&mut a, Accion::Soltar);
        assert!(a.servicios.servidores.saving);
        assert_eq!(a.servicios.servidores.registros, original);
    }
    #[test]
    fn escape_cierra_panel_sin_modificar_lista_y_respeta_guardado_en_curso() {
        let mut a = app();
        a.reducir = true;
        let _ = actualizar(&mut a, Accion::Editar(0));
        let original = a.servicios.servidores.registros.clone();
        a.servicios.servidores.saving = true;
        let _ = actualizar(&mut a, Accion::Cerrar);
        assert!(a.servicios.servidores.panel_abierto());
        a.servicios.servidores.saving = false;
        let _ = actualizar(&mut a, Accion::Cerrar);
        assert!(!a.servicios.servidores.panel_abierto());
        assert_eq!(a.servicios.servidores.registros, original);
    }
}
