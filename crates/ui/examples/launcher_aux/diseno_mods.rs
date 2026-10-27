// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! TiendaMods, FilaMod, ModsInstancia y FichaMod del RAR, con datos reales.
use crate::{mods_reales, servicios, App, Mensaje};
use icaro_core::{
    modrinth::{Kind, Order, Project, Version},
    mods,
};
use icaro_ui::{
    componentes::{self, Estado as Senal, Variante},
    fuentes,
    iconos::{icono, Icono, Tam},
    tema::{texto, Paleta},
};
use iced::futures::SinkExt;
use iced::{
    widget::{button, column, container, row, text, Space},
    Alignment, Element, Length, Padding, Task,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub struct Cesta {
    pub raices: Vec<Version>,
    pub plan: Vec<(Version, String)>,
    pub busy: bool,
    pub error: Option<String>,
    revision: u64,
}
impl Cesta {
    pub fn para_prueba(v: Version, n: String) -> Self {
        Self {
            raices: vec![v.clone()],
            plan: vec![(v, n)],
            ..Default::default()
        }
    }
}
#[derive(Default)]
pub struct Estado {
    pub menu_barra: bool,
    pub cestas: BTreeMap<String, Cesta>,
    pub seleccion: BTreeMap<String, BTreeSet<String>>,
    pub proyectos: BTreeMap<String, (Project, String)>,
    pendientes: BTreeSet<String>,
    pub papelera: Option<(String, mods::Retirados)>,
}
#[derive(Debug, Clone)]
pub enum Accion {
    MenuBarra,
    ExportarLista,
    VerificarArchivos,
    Agregar(String, Option<String>),
    Quitar(String),
    Vaciar,
    Instalar,
    Plan(
        String,
        u64,
        Result<(Vec<Version>, Vec<(Version, String)>), String>,
    ),
    Metadata,
    Metadatos(Vec<(String, Result<(Project, String), String>)>),
    Categoria(String),
    Lado(usize),
    Seleccionar(String),
    SeleccionarTodos,
    LimpiarSeleccion,
    Activar(bool),
    ActualizarSeleccion,
    Retirar,
    Deshacer,
    VerPagina,
    VerVersion(String),
}
pub fn m(a: Accion) -> Mensaje {
    Mensaje::DisenoMods(a)
}
fn ids(app: &App) -> Option<&BTreeSet<String>> {
    mods_reales::instancia(app).and_then(|i| app.servicios.mods.diseno.seleccion.get(&i.id))
}
pub fn cesta(app: &App) -> Option<&Cesta> {
    mods_reales::instancia(app).and_then(|i| app.servicios.mods.diseno.cestas.get(&i.id))
}
fn preparar(
    app: &mut App,
    agregar: Option<(String, Option<String>)>,
    quitar: Option<String>,
) -> Task<Mensaje> {
    if app.servicios.mods.busy.is_some() {
        return Task::none();
    }
    let Some(i) = mods_reales::instancia(app)
        .cloned()
        .filter(|i| i.loader != "vanilla")
    else {
        return Task::none();
    };
    let installed = mods_reales::lista(app).to_vec();
    let c = app
        .servicios
        .mods
        .diseno
        .cestas
        .entry(i.id.clone())
        .or_default();
    c.revision += 1;
    c.busy = true;
    c.error = None;
    let generation = c.revision;
    let mut roots = c.raices.clone();
    if let Some(id) = quitar {
        roots.retain(|v| v.project_id != id);
    }
    let id = i.id.clone();
    let source = app.servicios.catalog.clone();
    Task::perform(
        async move {
            let catalog = source.map_err(|e| e.to_string())?;
            if let Some((project, chosen)) = agregar {
                let mut available = if let Some(v) = chosen {
                    vec![catalog.version(&v).await.map_err(|e| e.to_string())?]
                } else {
                    catalog
                        .versions(&project, Some(&i.minecraft), Some(&i.loader))
                        .await
                        .map_err(|e| e.to_string())?
                };
                available.retain(|v| {
                    v.project_id == project && mods::compatible(v, &i.minecraft, &i.loader)
                });
                available.sort_by(|a, b| b.date_published.cmp(&a.date_published));
                let v = available
                    .iter()
                    .find(|v| v.version_type == "release")
                    .cloned()
                    .or_else(|| available.into_iter().next())
                    .ok_or("No hay una versión compatible para esta instancia.")?;
                roots.retain(|r| r.project_id != project);
                roots.push(v);
            }
            let plan = mods::resolver(&catalog, roots.clone(), &installed, &i.minecraft, &i.loader)
                .await
                .map_err(|e| e.to_string())?;
            let mut named = vec![];
            for v in plan {
                if installed.iter().any(|m| m.version_id == v.id && m.activo) {
                    continue;
                }
                let name = catalog
                    .project(&v.project_id)
                    .await
                    .map_err(|e| e.to_string())?
                    .title;
                named.push((v, name));
            }
            Ok((roots, named))
        },
        move |r| m(Accion::Plan(id.clone(), generation, r)),
    )
}
pub fn actualizar(app: &mut App, a: Accion) -> Task<Mensaje> {
    match a {
        Accion::MenuBarra => app.servicios.mods.diseno.menu_barra=!app.servicios.mods.diseno.menu_barra,
        Accion::ExportarLista => {
            app.servicios.mods.diseno.menu_barra=false;
            let selected=ids(app).is_some_and(|s|!s.is_empty());
            return crate::exportar_lista_mods::actualizar(app,crate::exportar_lista_mods::Accion::Abrir(selected));
        }
        Accion::VerificarArchivos => {
            app.servicios.mods.diseno.menu_barra=false;
            return crate::verificacion_real::actualizar(app,crate::verificacion_real::Accion::Abrir);
        }
        Accion::Agregar(p, v) => return preparar(app, Some((p, v)), None),
        Accion::Quitar(p) => return preparar(app, None, Some(p)),
        Accion::Vaciar => {
            if let Some(i) = mods_reales::instancia(app).cloned() {
                let c = app.servicios.mods.diseno.cestas.entry(i.id).or_default();
                c.raices.clear();
                c.plan.clear();
                c.busy = false;
                c.error = None;
                c.revision += 1;
            }
        }
        Accion::Plan(id, revision, result) => {
            let c = app.servicios.mods.diseno.cestas.entry(id).or_default();
            if c.revision == revision {
                c.busy = false;
                match result {
                    Ok((roots, plan)) => {
                        c.raices = roots;
                        c.plan = plan;
                    }
                    Err(e) => c.error = Some(e),
                }
            }
        }
        Accion::Instalar => {
            if app.servicios.mods.busy.is_some() || app.servicios.instancias.operando {
                return Task::none();
            }
            let Some(i) = mods_reales::instancia(app).cloned() else {
                return Task::none();
            };
            let Some(c) = cesta(app).filter(|c| !c.busy && c.error.is_none() && !c.plan.is_empty())
            else {
                return Task::none();
            };
            let roots = c.raices.clone();
            if roots.iter().any(|v| {
                mods_reales::lista(app)
                    .iter()
                    .any(|m| m.project_id == v.project_id && m.version_id != v.id)
            }) {
                app.servicios.mods.confirmacion = Some((i, roots));
            } else {
                return mods_reales::ejecutar(app, i, roots);
            }
        }
        Accion::Metadata => {
            let query: Vec<_> = mods_reales::lista(app)
                .iter()
                .flat_map(|m| {
                    std::iter::once(m.project_id.clone())
                        .chain(m.dependencias.iter().filter_map(|d| d.project_id.clone()))
                })
                .filter(|p| {
                    !p.is_empty()
                        && !app.servicios.mods.diseno.proyectos.contains_key(p)
                        && !app.servicios.mods.diseno.pendientes.contains(p)
                })
                .collect::<BTreeSet<_>>()
                .into_iter()
                .take(128)
                .collect();
            app.servicios
                .mods
                .diseno
                .pendientes
                .extend(query.iter().cloned());
            let source = app.servicios.catalog.clone();
            // Consultar proyectos en lotes y reutilizar miembros ya resueltos.
            return Task::run(
                iced::stream::channel(8, async move |mut output| {
                    let catalog = source.map_err(|e| e.to_string());
                    let projects = match &catalog {
                        Ok(catalog) => catalog.projects_by_ids(&query).await.map_err(|e|e.to_string()),
                        Err(e) => Err(e.clone()),
                    };
                    for id in query {
                        let r = match &catalog {
                            Ok(catalog) => {
                                async {
                                    let p = projects.as_ref().map_err(Clone::clone)?.get(&id)
                                        .cloned().ok_or("El proyecto ya no está disponible.".to_owned())?;
                                    let members = catalog.members(&id).await.unwrap_or_default();
                                    let author = members
                                        .iter()
                                        .find(|m| m.is_owner)
                                        .or_else(|| members.first())
                                        .map_or(String::new(), |m| m.user.username.clone());
                                    Ok((p, author))
                                }
                                .await
                            }
                            Err(e) => Err(e.clone()),
                        };
                        let _ = output.send(Accion::Metadatos(vec![(id, r)])).await;
                    }
                    if let Ok(catalog)=catalog {let _=catalog.guardar_metadata_cache().await;}
                }),
                m,
            );
        }
        Accion::Metadatos(items) => {
            let mut urls = vec![];
            for (id, result) in items {
                app.servicios.mods.diseno.pendientes.remove(&id);
                if let Ok((p, author)) = result {
                    urls.extend(p.icon_url.iter().cloned());
                    app.servicios.mods.diseno.proyectos.insert(id, (p, author));
                }
            }
            return servicios::cargar_imagenes(app, urls);
        }
        Accion::Categoria(k) => {
            if app.tienda_cat.contains(&k) {
                app.tienda_cat.retain(|v| *v != k);
            } else {
                app.tienda_cat.push(k);
            }
            return servicios::buscar(app, 0, false);
        }
        Accion::Lado(n) => {
            app.tienda_lado = if app.tienda_lado == n { 2 } else { n };
            return servicios::buscar(app, 0, false);
        }
        Accion::Seleccionar(f) => {
            if let Some(i) = mods_reales::instancia(app).cloned() {
                let set = app.servicios.mods.diseno.seleccion.entry(i.id).or_default();
                if !set.remove(&f) {
                    set.insert(f);
                }
            }
        }
        Accion::SeleccionarTodos => {
            if let Some(i) = mods_reales::instancia(app).cloned() {
                let todos = mods_reales::lista(app).iter().map(|m| m.archivo.clone()).collect();
                app.servicios.mods.diseno.seleccion.insert(i.id, todos);
            }
        }
        Accion::LimpiarSeleccion => {
            if let Some(i) = mods_reales::instancia(app).cloned() {
                app.servicios.mods.diseno.seleccion.remove(&i.id);
            }
        }
        Accion::Activar(enabled) => {
            if app.servicios.mods.busy.is_some() {
                return Task::none();
            }
            let files: Vec<_> = mods_reales::lista(app)
                .iter()
                .filter(|v| v.activo != enabled && ids(app).is_some_and(|s| s.contains(&v.archivo)))
                .map(|v| v.archivo.clone())
                .collect();
            for f in files {
                let _ = mods_reales::actualizar(app, mods_reales::Accion::Alternar(f));
            }
            return mods_reales::actualizar(app, mods_reales::Accion::Cargar);
        }
        Accion::ActualizarSeleccion => {
            let Some(i) = mods_reales::instancia(app).cloned() else {
                return Task::none();
            };
            let selected: Vec<_> = mods_reales::updates(app)
                .iter()
                .filter(|v| {
                    mods_reales::lista(app).iter().any(|m| {
                        m.project_id == v.project_id
                            && ids(app).is_some_and(|s| s.contains(&m.archivo))
                    })
                })
                .cloned()
                .collect();
            if !selected.is_empty() && app.servicios.mods.busy.is_none() {
                app.servicios.mods.confirmacion = Some((i, selected));
            }
        }
        Accion::Retirar => {
            if app.servicios.mods.busy.is_some() || app.servicios.instancias.operando {
                return Task::none();
            }
            let Some(i) = mods_reales::instancia(app).cloned() else {
                return Task::none();
            };
            let files = ids(app).cloned().unwrap_or_default();
            match mods::retirar(&app.servicios.instancias.almacen, &i.id, &files) {
                Ok(ticket) => {
                    app.servicios.mods.diseno.papelera = Some((i.id.clone(), ticket));
                    app.servicios.mods.diseno.seleccion.remove(&i.id);
                    mods_reales::cargar_local(app);
                    app.aviso = Some(
                        icaro_ui::avisos::Aviso::nuevo(
                            "Mods movidos a la papelera del sistema.".into(),
                            Some(m(Accion::Deshacer)),
                            std::time::Instant::now(),
                        )
                        .con_duracion(std::time::Duration::from_secs(10)),
                    );
                }
                Err(e) => app.avisar(e.to_string()),
            }
        }
        Accion::Deshacer => {
            if app.servicios.mods.busy.is_some() {
                return Task::none();
            }
            if let Some((id, ticket)) = app.servicios.mods.diseno.papelera.take() {
                if let Err(e) = mods::restaurar(&app.servicios.instancias.almacen, &id, &ticket) {
                    app.servicios.mods.diseno.papelera = Some((id, ticket));
                    app.avisar(e.to_string());
                } else {
                    app.aviso = None;
                    mods_reales::recargar(app, &id);
                }
            }
        }
        Accion::VerPagina => return servicios::ampliar_ficha(app),
        Accion::VerVersion(v) => {
            let page = servicios::ampliar_ficha(app);
            let change = servicios::actualizar(app, servicios::Accion::CambiosVersion(v));
            return Task::batch([page, change]);
        }
    }
    Task::none()
}
pub fn accion<'a>(
    app: &'a App,
    project: &str,
    version: Option<&str>,
    primary: bool,
) -> Element<'a, Mensaje> {
    let p = app.paleta();
    let installed = mods_reales::lista(app)
        .iter()
        .find(|m| m.project_id == project && version.is_none_or(|v| v == m.version_id));
    if let Some(item) = installed {
        return if item.activo {
            componentes::insignia(p, Senal::Exito, "Instalado")
        } else {
            componentes::insignia_contorno(p, "Desactivado")
        };
    }
    let en_lista = cesta(app).is_some_and(|c| {
        c.raices
            .iter()
            .any(|v| v.project_id == project && version.is_none_or(|id| v.id == id))
    });
    componentes::boton_con_icono(
        p,
        if en_lista { Icono::Check } else { Icono::Mas },
        if en_lista {
            "En la lista"
        } else if primary {
            "Agregar a la lista"
        } else {
            "Agregar"
        },
        if primary {
            Variante::Primario
        } else {
            Variante::Secundario
        },
        (app.servicios.mods.busy.is_none() && !cesta(app).is_some_and(|c| c.busy)).then(|| {
            if en_lista {
                m(Accion::Quitar(project.into()))
            } else {
                m(Accion::Agregar(project.into(), version.map(str::to_owned)))
            }
        }),
        !primary,
    )
}
pub fn regla<'a>(p: Paleta) -> Element<'a, Mensaje> {
    container(Space::new(Length::Fill, 1))
        .style(icaro_ui::estilo::bloque(p.border))
        .into()
}
pub fn tienda(app: &App) -> Element<'_, Mensaje> {
    let p = app.paleta();
    let s = &app.servicios;
    let modpack = app.seccion == icaro_ui::shell::Seccion::Modpacks && app.editor.is_none();
    let (mc, ld) = crate::juego_de_instancia(app);
    let instance = mods_reales::instancia(app).map_or("", |i| i.nombre.as_str());
    let tipo = match app.editor {Some(3)=>Some(mods::TipoPack::Recursos),Some(8)=>Some(mods::TipoPack::Shaders),_=>None};
    let titulo = if modpack { "MODPACKS" } else { match tipo {Some(mods::TipoPack::Recursos)=>"AGREGAR RECURSOS",Some(mods::TipoPack::Shaders)=>"AGREGAR SHADERS",_=>"AGREGAR MODS"} };
    let mut header = row![
        text(titulo)
            .font(fuentes::DISPLAY)
            .size(32)
            .color(p.text),
    ].spacing(12).align_y(Alignment::Center);
    if !modpack {
        header = header.push(componentes::insignia_contorno(p, &format!("{instance} · {mc} · {ld}")));
    }
    let header = header.push(Space::with_width(Length::Fill)).push(
        container(componentes::campo_con_icono(
            p,
            Icono::Buscar,
            if modpack { "Buscar modpacks" } else { match tipo {Some(mods::TipoPack::Recursos)=>"Buscar resourcepacks",Some(mods::TipoPack::Shaders)=>"Buscar shaders",_=>"Buscar mods"} },
            &s.query,
            |q| Mensaje::Servicio(servicios::Accion::Consulta(q))
        ))
        .width(320));
    let mut filters = column![
        componentes::etiqueta(p, "Fuente"),
        row![
            componentes::chip(p, "Modrinth", true, Mensaje::Nada),
            componentes::boton_compacto(p, "CurseForge", Variante::Secundario, None)
        ],
        regla(p),
        componentes::etiqueta(p, "Categorías")
    ]
    .spacing(12)
    .width(216);
    let categories: &[(&str,&str)] = if modpack {
        &[("adventure","Aventura"),("technology","Tecnología"),("magic","Magia"),("optimization","Optimización"),("quests","Misiones"),("kitchen-sink","Variado")]
    } else { match tipo {
        Some(mods::TipoPack::Recursos)=>&[("16x","16×"),("32x","32×"),("64x","64×"),("128x","128×"),("realistic","Realista"),("vanilla-like","Estilo Vanilla")],
        Some(mods::TipoPack::Shaders)=>&[("realistic","Realista"),("fantasy","Fantasía"),("vanilla-like","Estilo Vanilla"),("low","Ligero"),("medium","Medio"),("high","Alto")],
        _=>&[("optimization","Optimización"),("utility","Utilidad"),("worldgen","Mapas"),("decoration","Decoración"),("technology","Tecnología"),("adventure","Aventura")],
    } };
    for &(key,label) in categories {
        filters = filters.push(componentes::casilla(
            p,
            label,
            app.tienda_cat.iter().any(|k| k == key),
            move |_| m(Accion::Categoria(key.into())),
        ));
    }
    if modpack {
        let mut versions = vec!["Todas las versiones".to_owned()];
        let mut loaders = vec!["Todos los loaders".to_owned()];
        if let Some(tags) = &s.tags {
            versions.extend(tags.versions.iter().filter(|v| v.version_type == "release").map(|v| v.version.clone()));
            loaders.extend(tags.loaders.iter().filter(|l| l.supported_project_types.iter().any(|t| t == Kind::Modpack.as_str())).map(|l| crate::ficha::loader(&l.name)));
        }
        filters = filters.push(regla(p))
            .push(componentes::etiqueta(p, "Minecraft"))
            .push(componentes::selector(p, versions, Some(s.minecraft.clone()), |v| Mensaje::Servicio(servicios::Accion::Minecraft(v))))
            .push(componentes::etiqueta(p, "Loader"))
            .push(componentes::selector(p, loaders, Some(s.loader.clone()), |v| Mensaje::Servicio(servicios::Accion::Loader(v))))
            .push(componentes::boton_compacto(p, "Restablecer filtros", Variante::Fantasma, Some(Mensaje::Servicio(servicios::Accion::LimpiarFiltros))));
    } else if tipo.is_none() {
    filters = filters
        .push(regla(p))
        .push(componentes::etiqueta(p, "Lado"));
    for (i, label) in ["Cliente y servidor", "Solo cliente"].iter().enumerate() {
        filters = filters.push(
            button(
                row![
                    componentes::rombo(p, app.tienda_lado == i, false),
                    text(*label).size(texto::BODY_SM.0).color(p.text)
                ]
                .spacing(8)
                .align_y(Alignment::Center).wrap(),
            )
            .on_press(m(Accion::Lado(i)))
            .padding(0)
            .style(icaro_ui::estilo::sin_estilo(p.text)),
        );
    }
    }
    let mut results = column![row![
        text(
            s.results
                .as_ref()
                .map_or("Consultando Modrinth…".into(), |r| format!(
                    "{} resultados",
                    crate::catalogo::numero(r.total_hits as u64)
                ))
        )
        .size(texto::BODY_SM.0)
        .color(p.text_muted),
        Space::with_width(Length::Fill),
        container(componentes::selector(
            p,
            vec![
                Order::Downloads,
                Order::Relevance,
                Order::Updated,
                Order::Newest
            ],
            Some(s.order),
            |v| Mensaje::Servicio(servicios::Accion::Orden(v))
        ))
        .width(200)
    ]
    .align_y(Alignment::Center)]
    .spacing(12);
    if let Some(r) = &s.results {
        for h in &r.hits {
            let title = button(
                row![
                    crate::catalogo::texto_publico(&h.title)
                        .font(fuentes::TITULO)
                        .size(texto::HEADING.0)
                        .color(p.text),
                    crate::catalogo::texto_publico(&h.author)
                        .size(texto::BODY_SM.0)
                        .color(p.text_muted)
                ]
                .spacing(8)
                .align_y(Alignment::Center).wrap(),
            )
            .padding(0)
            .on_press(if modpack { Mensaje::Servicio(servicios::Accion::Proyecto(h.project_id.clone())) } else { Mensaje::PanelMod(Some(h.project_id.clone())) })
            .style(icaro_ui::estilo::sin_estilo(p.text));
            let mut tags = row![].spacing(6).align_y(Alignment::Center);
            for k in h
                .categories
                .iter()
                .filter(|k| !matches!(k.as_str(), "fabric" | "forge" | "neoforge" | "quilt"))
                .take(2)
            {
                tags = tags.push(componentes::insignia_contorno(p, categoria(k)));
            }
            tags = tags.push(
                text(format!(
                    "{} descargas",
                    crate::catalogo::numero(h.downloads)
                ))
                .font(fuentes::MONO)
                .size(texto::MONO_SM.0)
                .wrapping(iced::widget::text::Wrapping::None)
                .color(p.text_muted),
            );
            let selected = !modpack &&
                cesta(app).is_some_and(|c| c.raices.iter().any(|v| v.project_id == h.project_id));
            results = results.push(
                container(
                    row![
                        s.images.vista(p, h.icon_url.as_deref(), 48.),
                        column![
                            title,
                            crate::catalogo::texto_publico(
                                h.description.chars().take(180).collect::<String>()
                            )
                            .size(texto::BODY_SM.0)
                            .color(p.text_muted),
                            tags.wrap()
                        ]
                        .spacing(6)
                        .width(Length::Fill),
                        if modpack {crate::modpacks_reales::accion(app,&h.project_id,None)}else if let Some(t)=tipo {mods_reales::packs::accion_cesta(app,t,&h.project_id,None)}else{accion(app, &h.project_id, None, false)}
                    ]
                    .spacing(16)
                    .align_y(Alignment::Center),
                )
                .padding(16)
                .width(Length::Fill)
                .style(icaro_ui::estilo::tarjeta(p, selected)),
            );
        }
        if r.hits.is_empty() {
            results = results.push(crate::capt::vacio_busqueda(
                app,
                &s.query,
                Mensaje::Servicio(servicios::Accion::Consulta(String::new())),
            ));
        }
        let por_pagina = r.limit.max(1);
        let mut cantidades = row![text("Mostrar").size(texto::BODY_SM.0).color(p.text_muted)]
            .spacing(icaro_ui::tema::espacio::S2)
            .align_y(Alignment::Center);
        for n in servicios::POR_PAGINA {
            cantidades = cantidades.push(componentes::chip(
                p,
                &n.to_string(),
                s.por_pagina == n,
                Mensaje::Servicio(servicios::Accion::PorPagina(n)),
            ));
        }
        results = results.push(
            // Margen inferior: los botones quedan enteros al final de la lista.
            container(
                row![
                    componentes::boton_compacto(
                        p,
                        "Anterior",
                        Variante::Secundario,
                        (r.offset > 0).then(|| Mensaje::Servicio(servicios::Accion::Pagina(
                            r.offset.saturating_sub(por_pagina)
                        )))
                    ),
                    column![
                        text(format!(
                            "{} / {}",
                            r.offset / por_pagina + 1,
                            r.total_hits.div_ceil(por_pagina).max(1)
                        ))
                        .font(fuentes::MONO)
                        .color(p.text_muted),
                        cantidades,
                    ]
                    .spacing(icaro_ui::tema::espacio::S2)
                    .align_x(Alignment::Center)
                    .width(Length::Fill),
                    componentes::boton_compacto(
                        p,
                        "Siguiente",
                        Variante::Secundario,
                        (r.offset + r.limit < r.total_hits)
                            .then(|| Mensaje::Servicio(servicios::Accion::Pagina(r.offset + r.limit)))
                    )
                ]
                .align_y(Alignment::Center),
            )
            .padding(iced::Padding { bottom: 32.0, ..iced::Padding::ZERO }),
        );
    }
    if let Some(e) = &s.error {
        results = results.push(icaro_ui::superficies::banner(p,componentes::Estado::Aviso,Icono::Aviso,
            "No se pudo consultar el catálogo.",e,Some(("Reintentar",Mensaje::Servicio(servicios::Accion::Buscar)))));
    }
    let mut main = column![].spacing(24).height(Length::Fill).padding(24);
    if !modpack {
        main = main.push(componentes::boton_con_icono(
            p,
            Icono::Atras,
            match tipo {Some(mods::TipoPack::Recursos)=>"Volver a mis recursos",Some(mods::TipoPack::Shaders)=>"Volver a mis shaders",_=>"Volver a mis mods"},
            Variante::Fantasma,
            Some(Mensaje::VistaMods(0)),
            true
        ));
    }
    main = main.push(header);
    if modpack {
        main = main.push(text("Elige la versión del modpack. Se instalará en una instancia nueva con su Minecraft y loader.").size(texto::BODY_SM.0).color(p.text_muted));
    } else {
        main = main.push(mods_reales::estado_operacion(app));
    }
    // Cambiar de página o de cantidad vuelve arriba suavemente.
    let pagina = s.results.as_ref().map_or(0, |r| (u64::from(r.offset) << 32) | u64::from(r.limit));
    main = main.push(row![
            crate::desplazable_pub(filters.into()),
            icaro_ui::scroll::desplazable_con_clave(results, pagina)
        ]
        .spacing(32)
        .height(Length::Fill));
    if let Some(c) = cesta(app).filter(|_| !modpack) {
        if c.busy {
            main = main.push(text("Preparando la lista y sus dependencias…").color(p.text_muted));
        }
        if let Some(e) = &c.error {
            main = main.push(text(e).color(p.warning));
        }
    }
    column![main, if modpack {Space::with_height(0).into()}else if let Some(t)=tipo {mods_reales::packs::barra_cesta(app,t)}else{barra(app)}].height(Length::Fill).into()
}
fn barra(app: &App) -> Element<'_, Mensaje> {
    let p = app.paleta();
    let Some(c) = cesta(app).filter(|c| !c.raices.is_empty()) else {
        return Space::with_height(0).into();
    };
    let names = c
        .plan
        .iter()
        .map(|(v, n)| {
            if c.raices.iter().any(|r| r.project_id == v.project_id) {
                n.clone()
            } else {
                format!("+ {n} (dependencia)")
            }
        })
        .collect::<Vec<_>>()
        .join(", ");
    let bytes: u64 = c
        .plan
        .iter()
        .filter_map(|(v, _)| {
            v.files
                .iter()
                .find(|f| f.primary)
                .or_else(|| v.files.first())
        })
        .map(|f| f.size)
        .sum();
    let mut inverted = p;
    inverted.text = p.bg;
    inverted.bg = p.text;
    inverted.border_strong = p.bg;
    inverted.border = p.bg;
    inverted.accent = p.bg;
    inverted.on_accent = p.text;
    inverted.accent_hover = p.surface;
    inverted.accent_pressed = p.surface_hover;
    container(
        row![
            text(format!("{} en la lista", c.raices.len()))
                .font(fuentes::TITULO)
                .color(p.bg),
            text(format!("{names} · {:.1} MB", bytes as f64 / 1048576.))
                .size(texto::BODY_SM.0)
                .color(p.bg)
                .width(Length::Fill),
            componentes::boton_compacto(
                inverted,
                "Vaciar",
                Variante::Secundario,
                Some(m(Accion::Vaciar))
            ),
            componentes::boton_con_icono(
                inverted,
                Icono::Descargas,
                &format!("Instalar {}", c.plan.len()),
                Variante::Primario,
                (!c.busy
                    && c.error.is_none()
                    && app.servicios.mods.busy.is_none()
                    && !c.plan.is_empty())
                .then(|| m(Accion::Instalar)),
                true
            )
        ]
        .spacing(16)
        .align_y(Alignment::Center),
    )
    .padding(Padding::from([12., 16.]))
    .width(Length::Fill)
    .style(icaro_ui::estilo::bloque(p.text))
    .into()
}

fn menu_lista(app: &App, p: icaro_ui::tema::Paleta, activo: bool) -> Element<'_, Mensaje> {
    use icaro_ui::superficies::{menu_contextual, ElementoMenu};
    let boton = icaro_ui::descargas::boton_icono(p, Icono::Acciones, m(Accion::MenuBarra));
    let menu = menu_contextual(app.paleta(), vec![
        ElementoMenu::Opcion { icono: Icono::Subir, texto: "Exportar lista…".into(), atajo: None, peligro: false, mensaje: m(Accion::ExportarLista) },
        ElementoMenu::Opcion { icono: Icono::Aviso, texto: "Archivos sin verificar".into(), atajo: None, peligro: false, mensaje: m(Accion::VerificarArchivos) },
    ]);
    icaro_ui::menu_anclado::menu(boton, menu, activo && app.servicios.mods.diseno.menu_barra, m(Accion::MenuBarra))
}

pub fn lista(app: &App) -> Element<'_, Mensaje> {
    let p = app.paleta();
    let installed = mods_reales::lista(app);
    let selected = ids(app);
    let count = selected.map_or(0, BTreeSet::len);
    let busy = app.servicios.mods.busy.is_some();
    let mut toolbar = row![].spacing(8).align_y(Alignment::Center);
    for (n, label) in [
        format!("Todos {}", installed.len()),
        format!("Activos {}", installed.iter().filter(|m| m.activo).count()),
        format!("Con actualización {}", mods_reales::updates(app).len()),
        "Con problemas".into(),
    ]
    .iter()
    .enumerate()
    {
        toolbar = toolbar.push(componentes::chip(
            p,
            label,
            app.mods_filtro == n,
            Mensaje::FiltroMods(n),
        ));
    }
    toolbar = toolbar.push(componentes::boton_compacto(
        p, "Seleccionar todos", Variante::Secundario,
        (!installed.is_empty()).then(|| m(Accion::SeleccionarTodos)),
    ));
    toolbar=toolbar.push(menu_lista(app, p, count == 0));
    if !mods_reales::updates(app).is_empty() {
        toolbar = toolbar.push(componentes::boton_con_icono(
            p,
            Icono::Actualizar,
            "Actualizar todo",
            Variante::Secundario,
            (!busy).then(|| Mensaje::ModsReales(mods_reales::Accion::ActualizarTodos)),
            true,
        ));
    }
    toolbar = toolbar.push(componentes::boton_con_icono(
        p,
        Icono::Mas,
        "Agregar mods",
        Variante::Secundario,
        (!busy).then_some(Mensaje::VistaMods(1)),
        true,
    ));
    toolbar = toolbar.push(componentes::boton_con_icono(
        p,
        Icono::Subir,
        "Importar .jar",
        Variante::Secundario,
        (!busy).then_some(Mensaje::ModsReales(mods_reales::Accion::ImportarLocal)),
        true,
    ));
    let selected_update = mods_reales::updates(app).iter().any(|v| {
        installed.iter().any(|m| {
            m.project_id == v.project_id && selected.is_some_and(|s| s.contains(&m.archivo))
        })
    });
    let bar: Element<Mensaje> = if count > 0 {
        let mut inv = p;
        inv.text = p.bg;
        inv.bg = p.text;
        inv.border_strong = p.bg;
        container(
            row![
                text(format!("{count} seleccionados"))
                    .font(fuentes::TITULO)
                    .color(p.bg)
                    .width(200),
                componentes::boton_compacto(
                    inv, "Seleccionar todos", Variante::Secundario,
                    (count < installed.len()).then(|| m(Accion::SeleccionarTodos)),
                ),
                componentes::boton_compacto(
                    inv,
                    "Activar",
                    Variante::Secundario,
                    (!busy).then(|| m(Accion::Activar(true)))
                ),
                componentes::boton_compacto(
                    inv,
                    "Desactivar",
                    Variante::Secundario,
                    (!busy).then(|| m(Accion::Activar(false)))
                ),
                componentes::boton_con_icono(
                    inv,
                    Icono::Actualizar,
                    "Actualizar",
                    Variante::Secundario,
                    (!busy && selected_update).then(|| m(Accion::ActualizarSeleccion)),
                    true
                ),
                componentes::boton_con_icono(
                    inv,
                    Icono::Papelera,
                    "Quitar",
                    Variante::Secundario,
                    (!busy).then(|| m(Accion::Retirar)),
                    true
                ),
                menu_lista(app, inv, true),
                componentes::boton_con_icono(
                    inv,
                    Icono::Cerrar,
                    "",
                    Variante::Fantasma,
                    Some(m(Accion::LimpiarSeleccion)),
                    true
                )
            ]
            .spacing(8)
            .align_y(Alignment::Center)
            .wrap(),
        )
        .padding(12)
        .style(icaro_ui::estilo::bloque(p.text))
        .into()
    } else {
        toolbar.wrap().into()
    };
    let mut rows = column![].spacing(0);
    for item in installed.iter().filter(|v| {
        v.nombre
            .to_lowercase()
            .contains(&app.busqueda_mods.to_lowercase())
    }) {
        let update = mods_reales::updates(app)
            .iter()
            .find(|v| v.project_id == item.project_id);
        let missing = item.dependencias.iter().find(|d| {
            item.activo
                && d.dependency_type == "required"
                && (d.project_id.is_some() || d.version_id.is_some())
                && !installed.iter().any(|m| {
                    m.activo
                        && d.project_id.as_deref().is_none_or(|p| p == m.project_id)
                        && d.version_id.as_deref().is_none_or(|v| v == m.version_id)
                })
        });
        let conflict = item.dependencias.iter().find(|d| {
            item.activo
                && d.dependency_type == "incompatible"
                && installed.iter().any(|m| {
                    m.activo
                        && (d.project_id.is_some() || d.version_id.is_some())
                        && d.project_id.as_deref().is_none_or(|p| p == m.project_id)
                        && d.version_id.as_deref().is_none_or(|v| v == m.version_id)
                })
        });
        if (app.mods_filtro == 1 && !item.activo)
            || (app.mods_filtro == 2 && update.is_none())
            || (app.mods_filtro == 3 && missing.is_none() && conflict.is_none())
        {
            continue;
        }
        let meta = app.servicios.mods.diseno.proyectos.get(&item.project_id);
        let chosen = selected.is_some_and(|s| s.contains(&item.archivo));
        let ink = if item.activo {
            p.text
        } else {
            iced::Color { a: 0.45, ..p.text }
        };
        let description = conflict
            .and_then(|d| {
                installed.iter().find(|m| {
                    m.activo
                        && d.project_id.as_deref().is_none_or(|p| p == m.project_id)
                        && d.version_id.as_deref().is_none_or(|v| v == m.version_id)
                })
            })
            .map_or_else(
                || {
                    meta.map_or(item.archivo.as_str(), |(p, _)| p.description.as_str())
                        .to_owned()
                },
                |other| format!("Conflicto con {} {}.", other.nombre, other.version),
            );
        let title = button(
            column![
                row![
                    crate::catalogo::texto_publico(&item.nombre)
                        .font(fuentes::TITULO)
                        .size(texto::BODY.0)
                        .color(ink),
                    crate::catalogo::texto_publico(
                        meta.map_or(String::new(), |(_, a)| format!("· {a}"))
                    )
                    .size(texto::BODY_SM.0)
                    .color(p.text_muted)
                ]
                .spacing(4)
                .align_y(Alignment::Center),
                crate::catalogo::texto_publico(description)
                    .size(texto::BODY_SM.0)
                    .color(if conflict.is_some() {
                        p.error
                    } else if item.activo {
                        p.text_muted
                    } else {
                        p.text_disabled
                    })
            ]
            .spacing(2),
        )
        .padding(0)
        .width(Length::Fill)
        .on_press_maybe(
            (!item.project_id.is_empty()).then(|| Mensaje::PanelMod(Some(item.project_id.clone()))),
        )
        .style(icaro_ui::estilo::sin_estilo(ink));
        let status = if !item.activo {
            componentes::insignia_contorno(p, "Desactivado")
        } else if conflict.is_some() {
            componentes::insignia(p, Senal::Error, "Conflicto")
        } else if let Some(d) = missing {
            let name = d
                .project_id
                .as_ref()
                .and_then(|id| app.servicios.mods.diseno.proyectos.get(id))
                .map_or("dependencia", |(p, _)| p.title.as_str());
            componentes::insignia(p, Senal::Aviso, &format!("Requiere {name}"))
        } else if let Some(v) = update {
            componentes::insignia(p, Senal::Info, &format!("{} disponible", v.version_number))
        } else if item.project_id.is_empty() {
            componentes::insignia_contorno(p, "Archivo local")
        } else {
            componentes::insignia(p, Senal::Exito, "Compatible")
        };
        let mut line = row![
            componentes::casilla(p, "", chosen, {
                let f = item.archivo.clone();
                move |_| m(Accion::Seleccionar(f.clone()))
            }),
            componentes::interruptor(
                p,
                item.activo,
                if busy {
                    Mensaje::Nada
                } else {
                    Mensaje::ModsReales(mods_reales::Accion::Alternar(item.archivo.clone()))
                }
            ),
            app.servicios.images.vista_opacidad(
                p,
                meta.and_then(|(p, _)| p.icon_url.as_deref()),
                48.,
                if item.activo { 1. } else { 0.45 }
            ),
            title,
            status
        ]
        .spacing(16)
        .align_y(Alignment::Center);
        if update.is_some() {
            line = line.push(componentes::boton_compacto(
                p,
                "Actualizar",
                Variante::Secundario,
                (!busy).then(|| {
                    Mensaje::ModsReales(mods_reales::Accion::Actualizar(item.project_id.clone()))
                }),
            ));
        } else if missing.is_some() {
            line = line.push(componentes::boton_compacto(
                p,
                "Instalar",
                Variante::Secundario,
                (!busy).then(|| {
                    Mensaje::ModsReales(mods_reales::Accion::Instalar(
                        item.project_id.clone(),
                        Some(item.version_id.clone()),
                    ))
                }),
            ));
        }
        line = line.push(
            container(
                text(&item.version)
                    .font(fuentes::MONO)
                    .size(texto::MONO_SM.0)
                    .color(p.text_muted),
            )
            .width(80)
            .align_x(iced::alignment::Horizontal::Right),
        );
        rows = rows
            .push(
                container(line)
                    .padding(Padding::from([14., 16.]))
                    .width(Length::Fill)
                    .style(move |_| iced::widget::container::Style {
                        background: (chosen || conflict.is_some()).then_some(
                            iced::Background::Color(if conflict.is_some() {
                                p.error_soft
                            } else {
                                p.surface_hover
                            }),
                        ),
                        ..Default::default()
                    }),
            )
            .push(regla(p));
    }
    if installed.is_empty() {
        rows = rows.push(
            text("Esta instancia no tiene mods instalados. Pulsa Agregar mods para comenzar.")
                .color(p.text_muted),
        );
    }
    let bytes = mods_reales::instancia(app)
        .and_then(|i| app.servicios.mods.tamanos.get(&i.id))
        .copied()
        .unwrap_or(0);
    let body:Element<'_,Mensaje>=column![
        componentes::campo(p, "Buscar", &app.busqueda_mods, Mensaje::BuscarMods),
        bar,
        mods_reales::estado_operacion(app),
        regla(p),
        crate::desplazable_pub(rows.into()),
        text(format!(
            "{} mods · {} activos · {:.1} MB",
            installed.len(),
            installed.iter().filter(|m| m.activo).count(),
            bytes as f64 / 1048576.
        ))
        .font(fuentes::MONO)
        .size(texto::MONO_SM.0)
        .color(p.text_muted)
    ]
    .spacing(16)
    .padding(Padding::from([24., 48.]))
    .height(Length::Fill)
    .into();
    body
}

pub fn categoria(value: &str) -> &str {
    match value {
        "optimization" => "Optimización",
        "utility" => "Utilidad",
        "worldgen" => "Generación del mundo",
        "decoration" => "Decoración",
        "technology" => "Tecnología",
        "adventure" => "Aventura",
        "management" => "Gestión",
        "library" => "Biblioteca",
        "equipment" => "Equipamiento",
        "magic" => "Magia",
        "storage" => "Almacenamiento",
        "food" => "Comida",
        "transportation" => "Transporte",
        "game-mechanics" => "Mecánicas del juego",
        "social" => "Social",
        _ => value,
    }
}

#[cfg(test)]
pub fn comprobar_cesta(app: &mut App, v: Version) {
    let i = mods_reales::instancia(app).unwrap().clone();
    assert!(mods::inventario(&app.servicios.instancias.almacen, &i.id)
        .unwrap()
        .is_empty());
    let _ = actualizar(
        app,
        Accion::Agregar(v.project_id.clone(), Some(v.id.clone())),
    );
    let first = cesta(app).unwrap().revision;
    assert!(cesta(app).unwrap().busy && app.servicios.mods.busy.is_none());
    let _ = actualizar(app, Accion::Vaciar);
    let _ = actualizar(
        app,
        Accion::Plan(
            i.id.clone(),
            first,
            Ok((vec![v.clone()], vec![(v.clone(), "Mod".into())])),
        ),
    );
    assert!(cesta(app).unwrap().plan.is_empty());
    let _ = actualizar(
        app,
        Accion::Agregar(v.project_id.clone(), Some(v.id.clone())),
    );
    let next = cesta(app).unwrap().revision;
    let _ = actualizar(
        app,
        Accion::Plan(
            i.id.clone(),
            next,
            Ok((vec![v.clone()], vec![(v, "Mod".into())])),
        ),
    );
    assert_eq!(cesta(app).unwrap().plan.len(), 1);
    assert!(app.servicios.mods.busy.is_none());
    assert!(mods::inventario(&app.servicios.instancias.almacen, &i.id)
        .unwrap()
        .is_empty());
    let _ = actualizar(app, Accion::Vaciar);
}

pub fn ficha(app: &App) -> Element<'_, Mensaje> {
    let p = app.paleta();
    let s = &app.servicios;
    let Some((project, versions)) = &s.detail else {
        return container(
            column![
                componentes::boton_con_icono(
                    p,
                    Icono::Cerrar,
                    "",
                    Variante::Fantasma,
                    Some(Mensaje::PanelMod(None)),
                    true
                ),
                text(s.error.as_deref().unwrap_or("Obteniendo la ficha…")).color(p.text_muted)
            ]
            .spacing(16),
        )
        .padding(24)
        .width(640)
        .height(Length::Fill)
        .style(icaro_ui::estilo::flotante(p))
        .into();
    };
    let author = s
        .ficha
        .miembros
        .iter()
        .map(|m| m.user.username.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    let (mc, ld) = crate::juego_de_instancia(app);
    let pack = match project.project_type.as_str() {
        "resourcepack" => Some(mods::TipoPack::Recursos),
        "shader" => Some(mods::TipoPack::Shaders),
        _ => None,
    };
    let mut body = column![
        row![
            s.images.vista(p, project.icon_url.as_deref(), 56.),
            column![
                crate::catalogo::texto_publico(&project.title)
                    .font(fuentes::TITULO)
                    .size(texto::TITLE.0)
                    .color(p.text),
                text(format!(
                    "{author} · {} descargas",
                    crate::catalogo::numero(project.downloads)
                ))
                .size(texto::BODY_SM.0)
                .color(p.text_muted)
            ]
            .spacing(4)
            .width(Length::Fill),
            componentes::boton_con_icono(
                p,
                Icono::Cerrar,
                "",
                Variante::Fantasma,
                Some(Mensaje::PanelMod(None)),
                true
            )
        ]
        .spacing(16)
        .align_y(Alignment::Center),
        regla(p),
        row![
            if let Some(tipo) = pack {
                mods_reales::packs::accion_instalar(app, tipo, &project.id, None)
            } else {
                accion(app, &project.id, None, true)
            },
            componentes::boton_con_icono(
                p,
                Icono::Externo,
                "Ver página",
                Variante::Secundario,
                Some(m(Accion::VerPagina)),
                true
            )
        ]
        .spacing(8),
        crate::catalogo::texto_publico(&project.description)
            .size(texto::BODY_SM.0)
            .color(p.text)
    ]
    .spacing(20);
    let mut gallery = row![].spacing(8);
    for (n, g) in project.gallery.iter().take(3).enumerate() {
        gallery = gallery.push(
            button(
                container(s.images.portada_alta(&g.url, 100.).unwrap_or_else(|| {
                    container(Space::new(Length::Fill, 100))
                        .style(icaro_ui::estilo::marco(p))
                        .into()
                }))
                .width(Length::Fill)
                .height(100)
                .padding(1)
                .style(icaro_ui::estilo::marco(p)),
            )
            .padding(0)
            .width(Length::FillPortion(1))
            .on_press(Mensaje::Servicio(servicios::Accion::VisorAbrir(n)))
            .style(icaro_ui::estilo::sin_estilo(p.text)),
        );
    }
    if !project.gallery.is_empty() {
        body = body.push(gallery);
    }
    body = body.push(componentes::etiqueta(
        p,
        &match pack {
            Some(mods::TipoPack::Recursos) => "Versiones de resourcepack".into(),
            Some(mods::TipoPack::Shaders) => format!("Versiones para Minecraft {mc} · Shaders"),
            None => format!("Versiones para {mc} · {ld}"),
        },
    ));
    for v in versions
        .iter()
        .filter(|v| v.version_type == "release")
        .take(3)
    {
        body = body
            .push(
                row![
                    button(
                        text(&v.version_number)
                            .font(fuentes::MONO)
                            .size(texto::MONO.0)
                    )
                    .padding(0)
                    .on_press(m(Accion::VerVersion(v.id.clone())))
                    .style(icaro_ui::estilo::sin_estilo(p.text)),
                    componentes::insignia_contorno(p, "Estable"),
                    Space::with_width(Length::Fill),
                    text(format!(
                        "{} · {:.1} MB",
                        crate::ficha::fecha(&v.date_published),
                        v.files.iter().map(|f| f.size).sum::<u64>() as f64 / 1048576.
                    ))
                    .font(fuentes::MONO)
                    .size(texto::MONO_SM.0)
                    .color(p.text_muted)
                ]
                .spacing(8)
                .align_y(Alignment::Center),
            )
            .push(regla(p));
    }
    body = body.push(componentes::etiqueta(p, "Dependencias"));
    if let Some(v) = versions
        .iter()
        .find(|v| Some(&v.id) == s.ficha.seleccionada.as_ref())
    {
        for d in &v.dependencies {
            let dep = s
                .dependencias
                .get(&servicios::clave_dependencia(d))
                .and_then(|v| v.as_ref().ok());
            let name = dep.map_or("Obteniendo nombre…", |(_, n)| n.as_str());
            let installed = dep.is_some_and(|(id, _)| {
                mods_reales::lista(app)
                    .iter()
                    .any(|m| m.project_id == *id && m.activo)
                    || [mods::TipoPack::Recursos, mods::TipoPack::Shaders].iter().any(|tipo|
                        mods_reales::packs::lista(app, *tipo).iter().any(|m| m.project_id == *id && m.activo))
            });
            let en_lista = dep.is_some_and(|(id, _)| {
                pack.is_none() && cesta(app).is_some_and(|c| c.plan.iter().any(|(v, _)| &v.project_id == id))
            });
            let title = button(
                crate::catalogo::texto_publico(name)
                    .size(texto::BODY_SM.0)
                    .color(p.text),
            )
            .padding(0)
            .width(Length::Fill)
            .on_press_maybe(dep.map(|(id, _)| Mensaje::PanelMod(Some(id.clone()))))
            .style(icaro_ui::estilo::sin_estilo(p.text));
            let estado = if d.dependency_type == "optional" && !installed && pack.is_none() {
                if let Some((id, _)) = dep {
                    let id = id.clone();
                    let version = d.version_id.clone();
                    componentes::casilla(p, "Opcional", en_lista, move |yes| {
                        if yes {
                            m(Accion::Agregar(id.clone(), version.clone()))
                        } else {
                            m(Accion::Quitar(id.clone()))
                        }
                    })
                } else {
                    componentes::insignia_contorno(p, "Opcional")
                }
            } else {
                componentes::insignia(
                    p,
                    if installed && d.dependency_type != "incompatible" {
                        Senal::Exito
                    } else if d.dependency_type == "incompatible" {
                        Senal::Aviso
                    } else {
                        Senal::Info
                    },
                    match d.dependency_type.as_str() {
                        "incompatible" => "Incompatible",
                        "embedded" => "Incluida",
                        _ if installed => "Instalada",
                        "required" if pack.is_none() => "Se instalará",
                        "required" => "Obligatoria",
                        _ => "Opcional",
                    },
                )
            };
            body = body.push(
                row![icono(Icono::Mods, Tam::Base, p.text), title, estado]
                    .spacing(8)
                    .align_y(Alignment::Center),
            );
        }
    }
    body = body.push(crate::info_proyecto::vista(app, project, 592.));
    row![
        container(Space::new(2, Length::Fill)).style(icaro_ui::estilo::bloque(p.text)),
        container(crate::desplazable_pub(body.into()))
            .padding(24)
            .height(Length::Fill)
            .width(Length::Fill)
            .style(icaro_ui::estilo::bloque(p.surface))
    ]
    .width(640)
    .height(Length::Fill)
    .into()
}
