// SPDX-License-Identifier: GPL-3.0-only
//! FilaMod y ModsInstancia, compartidos por paquetes ZIP reales.
use crate::{mods_reales, App, Mensaje};
use icaro_core::{
    instancias::Instancia,
    modrinth::{Project, Version},
    mods::{self, Instalado, TipoPack},
};
use icaro_ui::{
    componentes::{self, Estado as Senal, Variante},
    fuentes,
    iconos::Icono,
    tema::texto,
};
use iced::{
    widget::{button, column, container, row, text},
    Alignment, Element, Length, Padding, Task,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub struct Estado {
    listas: BTreeMap<(String, TipoPack), Vec<Instalado>>,
    tamanos: BTreeMap<(String, TipoPack), u64>,
    updates: BTreeMap<(String, TipoPack), Vec<Version>>,
    seleccion: BTreeMap<(String, TipoPack), BTreeSet<String>>,
    pub confirmacion: Option<(Instancia, TipoPack, Vec<Version>, bool)>,
    generation: u64,
    consultas: BTreeMap<(String, TipoPack), u64>,
    operacion: Option<u64>,
    iris: BTreeMap<String, bool>,
    cestas: BTreeMap<(String, TipoPack), Vec<Version>>,
    pendientes_cesta: BTreeMap<(String, TipoPack, String), u64>,
}
#[derive(Debug, Clone)]
pub enum Accion {
    AgregarCesta(TipoPack, String, Option<String>),
    CestaPreparada(Instancia, TipoPack, String, u64, Result<Version, String>),
    QuitarCesta(TipoPack, String),
    VaciarCesta(TipoPack),
    InstalarCesta(TipoPack),
    Revisada(
        String,
        TipoPack,
        u64,
        Result<(Vec<Version>, Vec<(Project, String)>), String>,
    ),
    Instalar(TipoPack, String, Option<String>),
    Preparada(Instancia, TipoPack, u64, Result<Version, String>),
    Seleccionar(TipoPack, String),
    Todos(TipoPack),
    Limpiar(TipoPack),
    Alternar(TipoPack, String),
    Activar(TipoPack, bool),
    Quitar(TipoPack),
    Actualizar(TipoPack, Option<String>),
    ActualizarSeleccion(TipoPack),
    Confirmar,
    Cancelar,
    Terminada(String, TipoPack, u64, Result<(), String>),
}
fn m(a: Accion) -> Mensaje {
    Mensaje::PacksReales(a)
}
fn key(app: &App, tipo: TipoPack) -> Option<(String, TipoPack)> {
    mods_reales::instancia(app).map(|i| (i.id.clone(), tipo))
}
pub fn lista(app: &App, tipo: TipoPack) -> &[Instalado] {
    key(app, tipo)
        .and_then(|k| app.servicios.mods.packs.listas.get(&k))
        .map_or(&[], Vec::as_slice)
}
fn novedades(app: &App, tipo: TipoPack) -> &[Version] {
    key(app, tipo)
        .and_then(|k| app.servicios.mods.packs.updates.get(&k))
        .map_or(&[], Vec::as_slice)
}
fn seleccion(app: &App, tipo: TipoPack) -> BTreeSet<String> {
    key(app, tipo)
        .and_then(|k| app.servicios.mods.packs.seleccion.get(&k))
        .cloned()
        .unwrap_or_default()
}
fn recargar(app: &mut App, id: &str, tipo: TipoPack) {
    match mods::inventario_packs(&app.servicios.instancias.almacen, id, tipo) {
        Ok(list) => {
            let bytes = app
                .servicios
                .instancias
                .almacen
                .juego(id)
                .ok()
                .map_or(0, |game| {
                    list.iter()
                        .filter_map(|item| {
                            std::fs::metadata(game.join(tipo.carpeta()).join(&item.archivo)).ok()
                        })
                        .map(|meta| meta.len())
                        .sum()
                });
            app.servicios
                .mods
                .packs
                .tamanos
                .insert((id.into(), tipo), bytes);
            if tipo == TipoPack::Recursos {
                app.servicios.mods.recursos.insert(id.into(), list.clone());
            }
            app.servicios
                .mods
                .packs
                .listas
                .insert((id.into(), tipo), list);
        }
        Err(e) => app.avisar_error(e.to_string()),
    }
}
pub fn cargar(app: &mut App, tipo: TipoPack) -> Task<Mensaje> {
    let Some(i) = mods_reales::instancia(app).cloned() else {
        return Task::none();
    };
    app.servicios.mods.packs.iris.insert(
        i.id.clone(),
        app.servicios
            .instancias
            .almacen
            .juego(&i.id)
            .ok()
            .is_some_and(|game| game.join("config/iris.properties").is_file()),
    );
    recargar(app, &i.id, tipo);
    let installed = lista(app, tipo).to_vec();
    let state = &mut app.servicios.mods.packs;
    state.generation += 1;
    let generation = state.generation;
    state.consultas.insert((i.id.clone(), tipo), generation);
    let source = app.servicios.catalog.clone();
    Task::perform(
        async move {
            use iced::futures::stream::{self, StreamExt};
            let cat = source.map_err(|e| e.to_string())?;
            let minecraft = i.minecraft;
            let items: Vec<_> = installed
                .into_iter()
                .filter(|m| !m.project_id.is_empty())
                .take(128)
                .collect();
            // Una consulta por lotes de 50 fichas en lugar de una por paquete;
            // si falla, los paquetes se revisan igual sin ficha.
            let ids: Vec<String> = items
                .iter()
                .map(|m| m.project_id.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            let fichas = cat.projects_by_ids(&ids).await.unwrap_or_default();
            let items: Vec<_> = items
                .into_iter()
                .map(|item| {
                    let ficha = fichas.get(&item.project_id).cloned();
                    (item, ficha)
                })
                .collect();
            // Autor y versiones siguen siendo por proyecto: hasta 4 en curso,
            // con el orden original y la cuota compartida del catálogo.
            let revisados: Vec<Result<_, String>> = stream::iter(items)
                .map(|(item, ficha)| {
                    let (cat, minecraft) = (cat.clone(), minecraft.clone());
                    async move {
                        let minecraft = minecraft.as_str();
                        let ficha = match ficha {
                            Some(p) => {
                                let author = cat
                                    .members(&item.project_id)
                                    .await
                                    .unwrap_or_default()
                                    .into_iter()
                                    .find(|m| m.is_owner)
                                    .map_or(String::new(), |m| m.user.username);
                                Some((p, author))
                            }
                            None => None,
                        };
                        let mut versions = cat
                            .versions(&item.project_id, Some(minecraft), None)
                            .await
                            .map_err(|e| e.to_string())?;
                        versions.retain(|v| {
                            v.project_id == item.project_id
                                && mods::recurso_compatible(v, minecraft)
                                && v.version_type == "release"
                        });
                        versions.sort_by(|a, b| b.date_published.cmp(&a.date_published));
                        let update = versions.into_iter().next().filter(|v| {
                            v.id != item.version_id && v.date_published > item.publicada
                        });
                        Ok((ficha, update))
                    }
                })
                .buffered(4)
                .collect()
                .await;
            let mut updates = vec![];
            let mut projects = vec![];
            for revisado in revisados {
                let (ficha, update) = revisado?;
                projects.extend(ficha);
                updates.extend(update);
            }
            Ok((updates, projects))
        },
        move |r| m(Accion::Revisada(i.id.clone(), tipo, generation, r)),
    )
}
fn disponible(app: &mut App) -> bool {
    if app.servicios.mods.busy.is_some() || app.servicios.instancias.operando {
        return false;
    }
    if mods_reales::instancia(app).is_some_and(|i| app.servicios.juego.procesos.contains_key(&i.id))
    {
        app.avisar("Cierra Minecraft antes de modificar los paquetes de esta instancia.".into());
        return false;
    }
    true
}
fn ejecutar(app: &mut App, i: Instancia, tipo: TipoPack, versions: Vec<Version>) -> Task<Mensaje> {
    if !disponible(app) {
        return Task::none();
    }
    let state = &mut app.servicios.mods.packs;
    state.generation += 1;
    let generation = state.generation;
    state.operacion = Some(generation);
    app.servicios.mods.busy = Some(i.id.clone());
    app.servicios.mods.progreso = format!("Instalando {}…", tipo.etiqueta());
    let store = app.servicios.instancias.almacen.clone();
    let id = i.id.clone();
    let source = app.servicios.catalog.clone();
    Task::perform(
        async move {
            let cat = source.map_err(|e| e.to_string())?;
            let result = mods::instalar_packs(&cat, &store, &id, versions, tipo, |_| {})
                .await
                .map(|_| ())
                .map_err(|e| e.to_string());
            let _ = cat.guardar_metadata_cache().await;
            result
        },
        move |r| m(Accion::Terminada(i.id.clone(), tipo, generation, r)),
    )
}
fn operar(
    app: &mut App,
    tipo: TipoPack,
    files: BTreeSet<String>,
    activar: Option<bool>,
) -> Task<Mensaje> {
    if files.is_empty() || !disponible(app) {
        return Task::none();
    }
    let Some(i) = mods_reales::instancia(app).cloned() else {
        return Task::none();
    };
    let state = &mut app.servicios.mods.packs;
    state.generation += 1;
    let generation = state.generation;
    state.operacion = Some(generation);
    app.servicios.mods.busy = Some(i.id.clone());
    app.servicios.mods.progreso = "Guardando cambios de paquetes…".into();
    let store = app.servicios.instancias.almacen.clone();
    let id = i.id.clone();
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || match activar {
                Some(a) => mods::activar_packs(&store, &id, tipo, &files, a),
                None => mods::quitar_packs(&store, &id, tipo, &files),
            })
            .await
            .map_err(|_| "No se pudo completar la operación de paquetes.".to_owned())?
            .map_err(|e| e.to_string())
        },
        move |r| m(Accion::Terminada(i.id.clone(), tipo, generation, r)),
    )
}
pub fn actualizar(app: &mut App, a: Accion) -> Task<Mensaje> {
    if app.servicios.demo {
        return Task::none();
    }
    match a {
        Accion::AgregarCesta(t, project, version) => {
            if !disponible(app) { return Task::none(); }
            let Some(i) = mods_reales::instancia(app).cloned() else { return Task::none(); };
            let state = &mut app.servicios.mods.packs;
            state.generation += 1;
            let g = state.generation;
            state.pendientes_cesta.insert((i.id.clone(), t, project.clone()), g);
            let requested = project.clone();
            let source = app.servicios.catalog.clone();
            let minecraft = i.minecraft.clone();
            return Task::perform(async move {
                let catalog = source.map_err(|e| e.to_string())?;
                let p = catalog.project(&requested).await.map_err(|e|e.to_string())?;
                if p.project_type != if t == TipoPack::Recursos { "resourcepack" } else { "shader" } { return Err("El proyecto no corresponde a esta tienda.".into()); }
                let mut versions = catalog.versions(&requested, None, None).await.map_err(|e|e.to_string())?;
                versions.sort_by(|a,b|b.date_published.cmp(&a.date_published));
                elegir_version(&versions, version.as_deref(), &minecraft)
                    .cloned().ok_or_else(||"No hay un archivo instalable para este proyecto.".to_owned())
            }, move |r|m(Accion::CestaPreparada(i.clone(),t,project.clone(),g,r)));
        }
        Accion::CestaPreparada(i,t,project,g,r) => {
            let pending = (i.id.clone(),t,project.clone());
            if app.servicios.mods.packs.pendientes_cesta.get(&pending)!=Some(&g) {return Task::none();}
            app.servicios.mods.packs.pendientes_cesta.remove(&pending);
            match r {
                Ok(v) if v.project_id==project => {
                    let basket=app.servicios.mods.packs.cestas.entry((i.id,t)).or_default();
                    basket.retain(|old|old.project_id!=project); basket.push(v);
                }
                Ok(_) => app.avisar("La versión recibida corresponde a otro proyecto.".into()),
                Err(e) => app.avisar_error(e),
            }
        }
        Accion::QuitarCesta(t,project) => {
            if let Some(k)=key(app,t) {
                app.servicios.mods.packs.pendientes_cesta.remove(&(k.0.clone(),t,project.clone()));
                if let Some(b)=app.servicios.mods.packs.cestas.get_mut(&k){b.retain(|v|v.project_id!=project);}
            }
        }
        Accion::VaciarCesta(t) => {
            if let Some(k)=key(app,t){app.servicios.mods.packs.cestas.remove(&k);app.servicios.mods.packs.pendientes_cesta.retain(|(id,kind,_),_|id!=&k.0||*kind!=t);}
        }
        Accion::InstalarCesta(t) => {
            if !disponible(app) {return Task::none();}
            if let Some(i)=mods_reales::instancia(app).cloned(){
                let versions=cesta(app,t).to_vec();
                if !versions.is_empty(){app.servicios.mods.packs.confirmacion=Some((i,t,versions,false));}
            }
        }
        Accion::Revisada(id, t, g, r) => {
            if app.servicios.mods.packs.consultas.get(&(id.clone(), t)) != Some(&g) {
                return Task::none();
            }
            app.servicios.mods.packs.consultas.remove(&(id.clone(), t));
            match r {
                Ok((u, p)) => {
                    app.servicios.mods.packs.updates.insert((id, t), u);
                    let mut urls = vec![];
                    for (project, author) in p {
                        urls.extend(project.icon_url.iter().cloned());
                        app.servicios
                            .mods
                            .diseno
                            .proyectos
                            .insert(project.id.clone(), (project, author));
                    }
                    return crate::servicios::cargar_imagenes(app, urls);
                }
                Err(e) => app.avisar_error(e),
            }
        }
        Accion::Seleccionar(t, f) => {
            if let Some(k) = key(app, t) {
                let s = app.servicios.mods.packs.seleccion.entry(k).or_default();
                if !s.insert(f.clone()) {
                    s.remove(&f);
                }
            }
        }
        Accion::Todos(t) => {
            let names = lista(app, t).iter().map(|m| m.archivo.clone()).collect();
            if let Some(k) = key(app, t) {
                app.servicios.mods.packs.seleccion.insert(k, names);
            }
        }
        Accion::Limpiar(t) => {
            if let Some(k) = key(app, t) {
                app.servicios.mods.packs.seleccion.remove(&k);
            }
        }
        Accion::Alternar(t, f) => {
            let enabled = lista(app, t)
                .iter()
                .find(|m| m.archivo == f)
                .is_some_and(|m| m.activo);
            return operar(app, t, BTreeSet::from([f]), Some(!enabled));
        }
        Accion::Activar(t, enabled) => return operar(app, t, seleccion(app, t), Some(enabled)),
        Accion::Quitar(t) => return operar(app, t, seleccion(app, t), None),
        Accion::Instalar(t, p, v) => {
            if !disponible(app) {
                return Task::none();
            }
            let Some(i) = mods_reales::instancia(app).cloned() else {
                return Task::none();
            };
            let state = &mut app.servicios.mods.packs;
            state.generation += 1;
            let g = state.generation;
            state.operacion = Some(g);
            app.servicios.mods.busy = Some(i.id.clone());
            app.servicios.mods.progreso = "Buscando versión del paquete…".into();
            let record = i.clone();
            let source = app.servicios.catalog.clone();
            return Task::perform(
                async move {
                    let cat = source.map_err(|e| e.to_string())?;
                    let project = cat.project(&p).await.map_err(|e| e.to_string())?;
                    if project.project_type
                        != if t == TipoPack::Recursos {
                            "resourcepack"
                        } else {
                            "shader"
                        }
                    {
                        return Err("El proyecto no pertenece a este tipo de paquete.".into());
                    }
                    let mut versions = if let Some(v) = v {
                        vec![cat.version(&v).await.map_err(|e| e.to_string())?]
                    } else {
                        cat.versions(&p, None, None)
                            .await
                            .map_err(|e| e.to_string())?
                    };
                    versions.retain(|v| v.project_id == p);
                    versions.sort_by(|a, b| b.date_published.cmp(&a.date_published));
                    versions
                        .iter()
                        .find(|v| {
                            mods::recurso_compatible(v, &i.minecraft) && v.version_type == "release"
                        })
                        .cloned()
                        .or_else(|| {
                            versions
                                .iter()
                                .find(|v| mods::recurso_compatible(v, &i.minecraft))
                                .cloned()
                        })
                        .or_else(|| versions.into_iter().next())
                        .ok_or("No hay archivos disponibles.".into())
                },
                move |r| m(Accion::Preparada(record.clone(), t, g, r)),
            );
        }
        Accion::Preparada(i, t, g, r) => {
            if app.servicios.mods.packs.operacion != Some(g) {
                return Task::none();
            }
            app.servicios.mods.busy = None;
            app.servicios.mods.packs.operacion = None;
            match r {
                Ok(v) => {
                    if !mods::recurso_compatible(&v, &i.minecraft) {
                        app.servicios.mods.packs.confirmacion = Some((i, t, vec![v], false));
                    } else {
                        return ejecutar(app, i, t, vec![v]);
                    }
                }
                Err(e) => app.avisar_error(e),
            }
        }
        Accion::Actualizar(t, p) => {
            if let Some(i) = mods_reales::instancia(app).cloned() {
                let versions = novedades(app, t)
                    .iter()
                    .filter(|v| p.as_ref().is_none_or(|p| p == &v.project_id))
                    .cloned()
                    .collect::<Vec<_>>();
                if !versions.is_empty() {
                    app.servicios.mods.packs.confirmacion = Some((i, t, versions, true));
                }
            }
        }
        Accion::ActualizarSeleccion(t) => {
            if let Some(i) = mods_reales::instancia(app).cloned() {
                let selected = seleccion(app, t);
                let versions = novedades(app, t)
                    .iter()
                    .filter(|v| {
                        lista(app, t)
                            .iter()
                            .any(|m| m.project_id == v.project_id && selected.contains(&m.archivo))
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                if !versions.is_empty() {
                    app.servicios.mods.packs.confirmacion = Some((i, t, versions, true));
                }
            }
        }
        Accion::Confirmar => {
            if let Some((i, t, v, _)) = app.servicios.mods.packs.confirmacion.take() {
                return ejecutar(app, i, t, v);
            }
        }
        Accion::Cancelar => app.servicios.mods.packs.confirmacion = None,
        Accion::Terminada(id, t, g, r) => {
            if app.servicios.mods.packs.operacion != Some(g) {
                return Task::none();
            }
            app.servicios.mods.packs.operacion = None;
            app.servicios.mods.busy = None;
            recargar(app, &id, t);
            let existing = app
                .servicios
                .mods
                .packs
                .listas
                .get(&(id.clone(), t))
                .map(|list| {
                    list.iter()
                        .map(|i| i.archivo.clone())
                        .collect::<BTreeSet<_>>()
                })
                .unwrap_or_default();
            if let Some(selected) = app.servicios.mods.packs.seleccion.get_mut(&(id.clone(), t)) {
                selected.retain(|f| existing.contains(f));
            }
            match r {
                Ok(()) => {
                    app.servicios.mods.packs.cestas.remove(&(id.clone(),t));
                    app.avisar(format!("Cambios de {} guardados.", t.etiqueta()));
                }
                Err(e) => app.avisar_error(e),
            }
            if mods_reales::instancia(app).is_some_and(|i| i.id == id) {
                return cargar(app, t);
            }
        }
    }
    Task::none()
}
pub fn accion_instalar<'a>(
    app: &'a App,
    tipo: TipoPack,
    project: &str,
    version: Option<&str>,
) -> Element<'a, Mensaje> {
    let installed = lista(app, tipo)
        .iter()
        .any(|m| m.project_id == project && version.is_none_or(|v| v == m.version_id));
    componentes::boton(
        app.paleta(),
        if installed { "Instalado" } else { "Instalar" },
        Variante::Primario,
        (!installed && app.servicios.mods.busy.is_none()).then(|| {
            m(Accion::Instalar(
                tipo,
                project.into(),
                version.map(str::to_owned),
            ))
        }),
    )
}
// La compatibilidad orienta la elección; no impide probar un ZIP de otra versión.
fn elegir_version<'a>(versions: &'a [Version], requested: Option<&str>, minecraft: &str) -> Option<&'a Version> {
    let available = || versions.iter().filter(|v| v.files.iter().any(|f| f.filename.ends_with(".zip")));
    if let Some(id) = requested { return available().find(|v| v.id == id); }
    available().find(|v| v.version_type == "release" && mods::recurso_compatible(v, minecraft))
        .or_else(|| available().find(|v| mods::recurso_compatible(v, minecraft)))
        .or_else(|| available().find(|v| v.version_type == "release"))
        .or_else(|| available().next())
}

pub fn cesta(app: &App, tipo: TipoPack) -> &[Version] {
    key(app,tipo).and_then(|k|app.servicios.mods.packs.cestas.get(&k)).map_or(&[],Vec::as_slice)
}
pub fn accion_cesta<'a>(app:&'a App,tipo:TipoPack,project:&str,version:Option<&str>)->Element<'a,Mensaje>{
    let p=app.paleta();
    if lista(app,tipo).iter().any(|i|i.project_id==project && version.is_none_or(|v|i.version_id==v)){return componentes::insignia(p,Senal::Exito,"Instalado");}
    let selected=cesta(app,tipo).iter().any(|v|v.project_id==project && version.is_none_or(|id|v.id==id));
    let pending=key(app,tipo).is_some_and(|k|app.servicios.mods.packs.pendientes_cesta.contains_key(&(k.0,tipo,project.into())));
    componentes::boton_con_icono(p,if selected{Icono::Check}else{Icono::Mas},if pending{"Preparando"}else if selected{"En la lista"}else{"Agregar"},Variante::Secundario,
        (!pending&&app.servicios.mods.busy.is_none()).then(||if selected{m(Accion::QuitarCesta(tipo,project.into()))}else{m(Accion::AgregarCesta(tipo,project.into(),version.map(str::to_owned)))}),true)
}
pub fn barra_cesta(app:&App,tipo:TipoPack)->Element<'_,Mensaje>{
    let p=app.paleta();let basket=cesta(app,tipo);
    if basket.is_empty(){return iced::widget::Space::with_height(0).into();}
    let bytes:u64=basket.iter().filter_map(|v|v.files.iter().find(|f|f.primary).or_else(||v.files.first())).map(|f|f.size).sum();
    let names=basket.iter().map(|v|app.servicios.mods.diseno.proyectos.get(&v.project_id).map(|(p,_)|p.title.clone()).or_else(||app.servicios.results.as_ref().and_then(|r|r.hits.iter().find(|h|h.project_id==v.project_id).map(|h|h.title.clone()))).unwrap_or_else(||v.name.clone())).collect::<Vec<_>>().join(", ");
    let mut inv=p;inv.bg=p.text;inv.text=p.bg;inv.border=p.bg;inv.border_strong=p.bg;inv.accent=p.bg;inv.on_accent=p.text;
    container(row![text(format!("{} en la lista",basket.len())).font(fuentes::TITULO).color(p.bg),text(format!("{names} · {:.1} MB",bytes as f64/1048576.)).size(12).color(p.bg).width(Length::Fill),componentes::boton_compacto(inv,"Vaciar",Variante::Secundario,Some(m(Accion::VaciarCesta(tipo)))),componentes::boton_con_icono(inv,icaro_ui::iconos::Icono::Descargas,&format!("Instalar {}",basket.len()),Variante::Primario,(app.servicios.mods.busy.is_none()).then(||m(Accion::InstalarCesta(tipo))),true)].spacing(16).align_y(Alignment::Center)).padding([12,24]).width(Length::Fill).style(icaro_ui::estilo::bloque(p.text)).into()
}
pub fn vista(app: &App, tipo: TipoPack) -> Element<'_, Mensaje> {
    let p = app.paleta();
    let installed = lista(app, tipo);
    let selected = seleccion(app, tipo);
    let busy = app.servicios.mods.busy.is_some();
    let updates = novedades(app, tipo);
    let mut toolbar = row![].spacing(8).align_y(Alignment::Center);
    for (n, label) in [
        format!("Todos {}", installed.len()),
        format!("Activos {}", installed.iter().filter(|r| r.activo).count()),
        format!("Con actualización {}", updates.len()),
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
        p,
        "Seleccionar todos",
        Variante::Secundario,
        (!installed.is_empty()).then(|| m(Accion::Todos(tipo))),
    ));
    if !updates.is_empty() {
        toolbar = toolbar.push(componentes::boton_con_icono(
            p,
            Icono::Actualizar,
            "Actualizar todo",
            Variante::Secundario,
            (!busy).then(|| m(Accion::Actualizar(tipo, None))),
            true,
        ));
    }
    toolbar = toolbar.push(componentes::boton_con_icono(
        p,
        Icono::Mas,
        &format!("Agregar {}", tipo.etiqueta()),
        Variante::Secundario,
        Some(Mensaje::VistaMods(1)),
        true,
    ));
    let count = selected.len();
    let bar: Element<Mensaje> = if count == 0 {
        toolbar.wrap().into()
    } else {
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
                    inv,
                    "Seleccionar todos",
                    Variante::Secundario,
                    (count < installed.len()).then(|| m(Accion::Todos(tipo)))
                ),
                componentes::boton_compacto(
                    inv,
                    "Activar",
                    Variante::Secundario,
                    (!busy).then(|| m(Accion::Activar(tipo, true)))
                ),
                componentes::boton_compacto(
                    inv,
                    "Desactivar",
                    Variante::Secundario,
                    (!busy).then(|| m(Accion::Activar(tipo, false)))
                ),
                componentes::boton_con_icono(
                    inv,
                    Icono::Actualizar,
                    "Actualizar",
                    Variante::Secundario,
                    (!busy
                        && updates.iter().any(|v| installed.iter().any(|i| i.project_id
                            == v.project_id
                            && selected.contains(&i.archivo))))
                    .then(|| m(Accion::ActualizarSeleccion(tipo))),
                    true
                ),
                componentes::boton_con_icono(
                    inv,
                    Icono::Papelera,
                    "Quitar",
                    Variante::Secundario,
                    (!busy).then(|| m(Accion::Quitar(tipo))),
                    true
                ),
                componentes::boton_con_icono(
                    inv,
                    Icono::Cerrar,
                    "",
                    Variante::Fantasma,
                    Some(m(Accion::Limpiar(tipo))),
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
    };
    let mut rows = column![].spacing(0);
    for item in installed.iter().filter(|i| {
        i.nombre
            .to_lowercase()
            .contains(&app.busqueda_mods.to_lowercase())
    }) {
        let update = updates.iter().find(|v| v.project_id == item.project_id);
        let meta = app.servicios.mods.diseno.proyectos.get(&item.project_id);
        let problema = tipo == TipoPack::Shaders && !soporta_iris(app);
        if (app.mods_filtro == 1 && !item.activo)
            || (app.mods_filtro == 2 && update.is_none())
            || (app.mods_filtro == 3 && !problema)
        {
            continue;
        }
        let chosen = selected.contains(&item.archivo);
        let ink = if item.activo {
            p.text
        } else {
            iced::Color { a: 0.45, ..p.text }
        };
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
                crate::catalogo::texto_publico(
                    meta.map_or(item.archivo.as_str(), |(p, _)| p.description.as_str())
                )
                .size(texto::BODY_SM.0)
                .color(if item.activo {
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
        let status = if problema {
            componentes::insignia(p, Senal::Aviso, "Requiere Iris")
        } else if !item.activo {
            componentes::insignia_contorno(p, "Desactivado")
        } else if let Some(v) = update {
            componentes::insignia(p, Senal::Info, &format!("{} disponible", v.version_number))
        } else if item.project_id.is_empty() {
            componentes::insignia_contorno(p, "Archivo local")
        } else {
            componentes::insignia(p, Senal::Exito, "Activo")
        };
        let mut line = row![
            componentes::casilla(p, "", chosen, {
                let f = item.archivo.clone();
                move |_| m(Accion::Seleccionar(tipo, f.clone()))
            }),
            componentes::interruptor(
                p,
                item.activo,
                if busy {
                    Mensaje::Nada
                } else {
                    m(Accion::Alternar(tipo, item.archivo.clone()))
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
                (!busy).then(|| m(Accion::Actualizar(tipo, Some(item.project_id.clone())))),
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
                        background: chosen.then_some(iced::Background::Color(p.surface_hover)),
                        ..Default::default()
                    }),
            )
            .push(crate::diseno_mods::regla(p));
    }
    if installed.is_empty() {
        rows = rows.push(
            text(format!(
                "Esta instancia no tiene {} instalados. Pulsa Agregar para comenzar.",
                tipo.etiqueta()
            ))
            .color(p.text_muted),
        );
    }
    let mut body = column![
        componentes::campo(p, "Buscar", &app.busqueda_mods, Mensaje::BuscarMods),
        bar,
        mods_reales::estado_operacion(app),
        crate::diseno_mods::regla(p),
        crate::desplazable_pub(rows.into()),
        text(format!(
            "{} {} · {} activos · {:.1} MB",
            installed.len(),
            tipo.etiqueta(),
            installed.iter().filter(|r| r.activo).count(),
            key(app, tipo)
                .and_then(|key| app.servicios.mods.packs.tamanos.get(&key))
                .copied()
                .unwrap_or(0) as f64
                / 1048576.
        ))
        .font(fuentes::MONO)
        .size(texto::MONO_SM.0)
        .color(p.text_muted)
    ]
    .spacing(16)
    .padding(Padding::from([24., 48.]))
    .height(Length::Fill);
    if tipo == TipoPack::Shaders && !soporta_iris(app) {
        body=body.push(text("Los ZIP se guardan en shaderpacks. Para activarlos necesitas instalar e iniciar Iris; Minecraft Vanilla no renderiza shaders.").size(texto::BODY_SM.0).color(p.text_muted));
    }
    body.into()
}
fn soporta_iris(app: &App) -> bool {
    mods_reales::instancia(app)
        .is_some_and(|i| app.servicios.mods.packs.iris.get(&i.id) == Some(&true))
}

pub fn capa<'a>(app: &'a App, window: Element<'a, Mensaje>) -> Element<'a, Mensaje> {
    let Some((i, t, versions, update)) = &app.servicios.mods.packs.confirmacion else {
        return window;
    };
    let p = app.paleta();
    let body=column![text(if *update{format!("Se actualizarán {} {} de {}.",versions.len(),t.etiqueta(),i.nombre)}else{format!("Se instalarán {} {} en {}. {} no declaran Minecraft {}.",versions.len(),t.etiqueta(),i.nombre,versions.iter().filter(|v|!mods::recurso_compatible(v,&i.minecraft)).count(),i.minecraft)}).color(p.text),text(if *update{"Las nuevas versiones pueden cambiar el resultado o tener incompatibilidades. ¿Quieres continuar?"}else{"Puedes probarlo igualmente y desactivarlo si presenta problemas."}).color(p.warning),text("Los archivos anteriores se conservan si la descarga o la verificación falla.").color(p.text_muted)].spacing(16);
    crate::con_velo_losa(
        p,
        window,
        crate::dialogo(
            p,
            if *update {
                "¿Actualizar paquetes?"
            } else {
                "¿Instalar paquete?"
            },
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
                    if *update {
                        "Actualizar"
                    } else {
                        "Instalar igualmente"
                    },
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
    if app.servicios.mods.packs.confirmacion.is_none() {
        return iced::Subscription::none();
    }
    iced::keyboard::on_key_press(|k, _| {
        matches!(
            k,
            iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape)
        )
        .then(|| m(Accion::Cancelar))
    })
}
pub fn cargar_local(app: &mut App, tipo: TipoPack) {
    if let Some(i) = mods_reales::instancia(app).cloned() {
        recargar(app, &i.id, tipo);
    }
}
pub fn preparar_diseno(app: &mut App, scene: &str, i: &Instancia, project: &Project, v: &Version) {
    let tipo = if scene.starts_with("shaders") {
        TipoPack::Shaders
    } else {
        TipoPack::Recursos
    };
    let key = (i.id.clone(), tipo);
    app.servicios
        .mods
        .packs
        .tamanos
        .insert(key.clone(), 18 * 1048576);
    if tipo == TipoPack::Shaders {
        app.editor = Some(8);
    }
    let items = if tipo == TipoPack::Recursos {
        app.servicios
            .mods
            .recursos
            .get(&i.id)
            .cloned()
            .unwrap_or_default()
    } else {
        vec![Instalado {
            project_id: project.id.clone(),
            nombre: "Complementary Reimagined".into(),
            version_id: v.id.clone(),
            version: v.version_number.clone(),
            publicada: v.date_published.clone(),
            archivo: "ComplementaryReimagined.zip".into(),
            sha512: String::new(),
            activo: true,
            dependencias: vec![],
        }]
    };
    app.servicios.mods.packs.iris.insert(i.id.clone(), true);
    app.servicios
        .mods
        .packs
        .listas
        .insert(key.clone(), items.clone());
    app.servicios
        .mods
        .diseno
        .proyectos
        .insert(project.id.clone(), (project.clone(), "FreshLX".into()));
    if scene.contains("seleccion") {
        app.servicios.mods.packs.seleccion.insert(
            key.clone(),
            items.iter().map(|m| m.archivo.clone()).collect(),
        );
    }
    if scene.contains("actualizacion") {
        let mut next = v.clone();
        next.version_number = "1.11".into();
        next.id = "packqa-next".into();
        app.servicios.mods.packs.updates.insert(key, vec![next]);
    }
    if scene.contains("confirmacion") {
        app.servicios.mods.packs.confirmacion = Some((i.clone(), tipo, vec![v.clone()], false));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn version(id: &str, tipo: &str, mc: &str) -> Version {
        serde_json::from_value(serde_json::json!({"id":id,"project_id":"pack","name":id,"version_number":id,"version_type":tipo,"game_versions":[mc],"loaders":["minecraft"],"files":[{"filename":"pack.zip","url":"https://cdn.modrinth.com/pack.zip","size":1,"primary":true,"hashes":{"sha512":"00"}}],"dependencies":[],"changelog":null})).unwrap()
    }
    #[test]
    fn agregar_acepta_beta_y_otra_version_sin_confundir_la_eleccion_explicita() {
        let versiones = vec![version("beta", "beta", "1.21.1"), version("estable", "release", "1.20.1")];
        assert_eq!(elegir_version(&versiones, None, "1.21.1").unwrap().id, "beta");
        assert_eq!(elegir_version(&versiones[..1], None, "26.3").unwrap().id, "beta");
        assert_eq!(elegir_version(&versiones, Some("estable"), "1.21.1").unwrap().id, "estable");
        assert!(elegir_version(&versiones, Some("ausente"), "1.21.1").is_none());
        let mut sin_archivo = versiones;
        for v in &mut sin_archivo { v.files.clear(); }
        assert!(elegir_version(&sin_archivo, None, "1.21.1").is_none());
    }
}
