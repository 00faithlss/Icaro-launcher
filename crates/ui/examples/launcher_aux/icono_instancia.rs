// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! IconoInstancia del ZIP: imagen, generación y capturas con recorte real.
use crate::{App, Mensaje};
use icaro_core::icono_instancia::{self as core, Anterior, Fuente};
use icaro_ui::{
    componentes::{self, Variante},
    fuentes,
    iconos::{Icono, Tam},
    tema::{texto, Paleta},
};
use iced::{
    widget::{button, column, container, image, mouse_area, row, scrollable, slider, text, Space},
    Alignment, Element, Length, Padding, Point, Task,
};
use std::{collections::HashMap, path::PathBuf};

#[derive(Debug, Clone, Default)]
pub struct Guardado {
    pub png: Option<Vec<u8>>,
    pub variacion: u32,
    pub handle: Option<image::Handle>,
}
#[derive(Default)]
pub struct Estado {
    pub dialogo: Option<Dialogo>,
    pub cache: HashMap<String, Guardado>,
    /// Portadas propias por id de instancia.
    pub portadas: HashMap<String, image::Handle>,
    undo: Option<(u64, String, Anterior)>,
    secuencia: u64,
}
pub struct Dialogo {
    pub id: String,
    pub nombre: String,
    pub version: String,
    pub tab: usize,
    pub zoom: u32,
    pub estilo: usize,
    pub x: f32,
    pub y: f32,
    pub variacion: u32,
    pub tanda: u32,
    pub fuente: Option<Fuente>,
    pub png: Option<Vec<u8>>,
    pub handle: Option<image::Handle>,
    pub capturas: Vec<PathBuf>,
    pub error: Option<String>,
    pub ocupada: bool,
    generacion: u64,
    sesion: u64,
    arrastre: Option<(Point, f32, f32)>,
    cursor: Point,
}
#[derive(Debug, Clone)]
pub enum Accion {
    Abrir(usize),
    Cerrar,
    Tab(usize),
    ElegirImagen,
    Archivo(PathBuf),
    Cargada(u64, Result<Fuente, String>),
    Capturas(u64, Result<Vec<PathBuf>, String>),
    Zoom(u32),
    Estilo(usize),
    Reset,
    Cursor(Point),
    Pulsar,
    Soltar,
    Mover(f32, f32),
    Generado(u32),
    OtraTanda,
    VolverGenerado,
    Guardar,
    Guardado(String, Guardado, Result<Anterior, String>),
    /// Icono y portada propia de cada instancia, leídos al iniciar.
    Cargados(Vec<(String, Result<(Option<Vec<u8>>, u32), String>, Option<Vec<u8>>)>),
    Deshacer(u64),
    Deshecho(String, Result<(), String>),
}
pub fn m(a: Accion) -> Mensaje {
    Mensaje::Icono(a)
}
pub fn cargar(app: &App) -> Task<Mensaje> {
    if app.servicios.demo {
        return Task::none();
    }
    let store = app.servicios.instancias.almacen.clone();
    let ids: Vec<_> = app
        .servicios
        .instancias
        .registros
        .iter()
        .map(|i| i.id.clone())
        .collect();
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                ids.into_iter()
                    .map(|id| {
                        let r = core::leer(&store, &id);
                        // Una portada dañada se ignora: queda el grabado elegido.
                        let portada = core::leer_portada(&store, &id).ok().flatten();
                        (id, r, portada)
                    })
                    .collect()
            })
            .await
            .unwrap_or_default()
        },
        |r| m(Accion::Cargados(r)),
    )
}
fn render(app: &mut App) -> Task<Mensaje> {
    if let Some(d) = app.iconos.dialogo.as_mut() {
        if let Some(f) = d.fuente.as_ref() {
            match core::vista_previa(f, d.zoom, d.x, d.y, [0, 32, 16][d.estilo.min(2)]) {
                Ok(pixels) => {
                    d.handle = Some(image::Handle::from_rgba(256, 256, pixels.into_raw()));
                    d.png = None; // Solo el resultado definitivo se escribe al guardar.
                }
                Err(e) => d.error = Some(e),
            }
        }
    }
    Task::none()
}
pub fn actualizar(app: &mut App, a: Accion) -> Task<Mensaje> {
    match a {
        Accion::Abrir(index) => {
            let cards = crate::instancias();
            let Some(card) = cards.get(index) else {
                return Task::none();
            };
            let id = app
                .servicios
                .instancias
                .registros
                .get(index)
                .map(|i| i.id.clone())
                .unwrap_or_else(|| format!("demo-{index}"));
            let saved = app.iconos.cache.get(&id).cloned().unwrap_or_default();
            app.iconos.secuencia = app.iconos.secuencia.wrapping_add(1);
            let seq = app.iconos.secuencia;
            app.iconos.dialogo = Some(Dialogo {
                id: id.clone(),
                nombre: card.nombre.clone(),
                version: card.version.clone(),
                tab: usize::from(saved.png.is_none()),
                zoom: 100,
                estilo: 1,
                x: 0.,
                y: 0.,
                variacion: saved.variacion,
                tanda: saved.variacion / 12,
                fuente: None,
                png: saved.png,
                handle: saved.handle,
                capturas: vec![],
                error: None,
                ocupada: false,
                generacion: seq,
                sesion: seq,
                arrastre: None,
                cursor: Point::ORIGIN,
            });
            app.menu = None;
            if !app.servicios.demo {
                let store = app.servicios.instancias.almacen.clone();
                return Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || core::capturas(&store, &id))
                            .await
                            .unwrap_or_else(|_| Err("No se pudo leer las capturas.".into()))
                    },
                    move |r| m(Accion::Capturas(seq, r)),
                );
            }
        }
        Accion::Cerrar => {
            if !app.iconos.dialogo.as_ref().is_some_and(|d| d.ocupada) {
                app.iconos.dialogo = None;
            }
        }
        Accion::Tab(tab) => {
            if let Some(d) = app.iconos.dialogo.as_mut() {
                if !d.ocupada {
                    d.tab = tab.min(2);
                    d.error = None;
                }
            }
        }
        Accion::ElegirImagen => {
            return Task::perform(
                async {
                    rfd::AsyncFileDialog::new()
                        .add_filter("Imágenes", &["png", "jpg", "jpeg", "webp", "gif"])
                        .pick_file()
                        .await
                        .map(|f| f.path().to_path_buf())
                },
                |r| r.map_or(Mensaje::Nada, |p| m(Accion::Archivo(p))),
            );
        }
        Accion::Archivo(path) => {
            if let Some(d) = app.iconos.dialogo.as_mut().filter(|d| !d.ocupada) {
                app.iconos.secuencia = app.iconos.secuencia.wrapping_add(1);
                let seq = app.iconos.secuencia;
                d.generacion = seq;
                d.fuente = None;
                d.handle = None;
                d.png = None;
                d.error = None;
                return Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || core::abrir(&path))
                            .await
                            .unwrap_or_else(|_| Err("No se pudo abrir la imagen.".into()))
                    },
                    move |r| m(Accion::Cargada(seq, r)),
                );
            }
        }
        Accion::Cargada(seq, result) => {
            if let Some(d) = app.iconos.dialogo.as_mut().filter(|d| d.generacion == seq) {
                match result {
                    Ok(f) => {
                        d.fuente = Some(f);
                        d.tab = 0;
                        d.zoom = 100;
                        d.x = 0.;
                        d.y = 0.;
                        return render(app);
                    }
                    Err(e) => d.error = Some(e),
                }
            }
        }
        Accion::Capturas(seq, r) => {
            if let Some(d) = app.iconos.dialogo.as_mut().filter(|d| d.sesion == seq) {
                match r {
                    Ok(v) => d.capturas = v,
                    Err(e) => d.error = Some(e),
                }
            }
        }
        Accion::Zoom(z) => {
            if let Some(d) = app.iconos.dialogo.as_mut().filter(|d| !d.ocupada) {
                d.zoom = z.clamp(100, 400);
                return render(app);
            }
        }
        Accion::Estilo(i) => {
            if let Some(d) = app.iconos.dialogo.as_mut().filter(|d| !d.ocupada) {
                d.estilo = i.min(2);
                return render(app);
            }
        }
        Accion::Reset => {
            if let Some(d) = app.iconos.dialogo.as_mut().filter(|d| !d.ocupada) {
                d.zoom = 100;
                d.x = 0.;
                d.y = 0.;
                return render(app);
            }
        }
        Accion::Cursor(point) => {
            if let Some(d) = app.iconos.dialogo.as_mut() {
                d.cursor = point;
                if let Some((origin, x, y)) = d.arrastre {
                    d.x = x + point.x - origin.x;
                    d.y = y + point.y - origin.y;
                    return render(app);
                }
            }
        }
        Accion::Pulsar => {
            if let Some(d) = app.iconos.dialogo.as_mut().filter(|d| !d.ocupada) {
                d.arrastre = Some((d.cursor, d.x, d.y));
            }
        }
        Accion::Soltar => {
            if let Some(d) = app.iconos.dialogo.as_mut() {
                d.arrastre = None;
            }
        }
        Accion::Mover(x, y) => {
            if let Some(d) = app.iconos.dialogo.as_mut().filter(|d| !d.ocupada) {
                d.x += x;
                d.y += y;
                return render(app);
            }
        }
        Accion::Generado(n) => {
            if let Some(d) = app.iconos.dialogo.as_mut().filter(|d| !d.ocupada) {
                d.variacion = n;
                d.tab = 1;
            }
        }
        Accion::OtraTanda => {
            if let Some(d) = app.iconos.dialogo.as_mut().filter(|d| !d.ocupada) {
                d.tanda = d.tanda.wrapping_add(1);
                d.variacion = d.tanda.saturating_mul(12);
            }
        }
        Accion::VolverGenerado => {
            if let Some(d) = app.iconos.dialogo.as_mut().filter(|d| !d.ocupada) {
                d.tab = 1;
                d.variacion = 0;
                d.tanda = 0;
            }
        }
        Accion::Guardar => {
            let Some(d) = app.iconos.dialogo.as_mut().filter(|d| !d.ocupada) else {
                return Task::none();
            };
            if d.tab != 1 && d.fuente.is_none() && d.png.is_none() {
                d.error = Some("Elige una imagen o un icono generado.".into());
                return Task::none();
            }
            let recorte = if d.tab == 1 {
                None
            } else {
                d.fuente
                    .clone()
                    .map(|f| (f, d.zoom, d.x, d.y, [0, 32, 16][d.estilo.min(2)]))
            };
            let mut value = Guardado {
                png: if d.tab == 1 { None } else { d.png.clone() },
                variacion: d.variacion,
                handle: if d.tab == 1 { None } else { d.handle.clone() },
            };
            if app.servicios.demo {
                if let Some((f, z, x, y, style)) = recorte {
                    match core::recortar(&f, z, x, y, style) {
                        Ok(png) => value.png = Some(png),
                        Err(e) => {
                            d.error = Some(e);
                            return Task::none();
                        }
                    }
                }
                app.iconos.cache.insert(d.id.clone(), value);
                app.iconos.dialogo = None;
                return Task::none();
            }
            let id = d.id.clone();
            let store = app.servicios.instancias.almacen.clone();
            d.ocupada = true;
            return Task::perform(
                async move {
                    let workid = id.clone();
                    tokio::task::spawn_blocking(move || {
                        let result = (|| {
                            if let Some((f, z, x, y, style)) = recorte {
                                let png = core::recortar(&f, z, x, y, style)?;
                                value.handle = Some(image::Handle::from_bytes(png.clone()));
                                value.png = Some(png);
                            }
                            core::guardar(&store, &workid, value.png.clone(), value.variacion)
                        })();
                        (id, value, result)
                    })
                    .await
                    .unwrap_or_else(|_| {
                        (
                            String::new(),
                            Guardado::default(),
                            Err("No se pudo guardar el icono.".into()),
                        )
                    })
                },
                |(id, v, r)| m(Accion::Guardado(id, v, r)),
            );
        }
        Accion::Guardado(id, value, r) => match r {
            Ok(old) => {
                app.iconos.cache.insert(id.clone(), value);
                app.iconos.secuencia = app.iconos.secuencia.wrapping_add(1);
                let seq = app.iconos.secuencia;
                app.iconos.undo = Some((seq, id, old));
                app.iconos.dialogo = None;
                app.avisar_con(
                    "Icono de instancia actualizado.".into(),
                    "Deshacer",
                    m(Accion::Deshacer(seq)),
                );
            }
            Err(e) => {
                if let Some(d) = app.iconos.dialogo.as_mut() {
                    d.ocupada = false;
                    d.error = Some(e);
                }
            }
        },
        Accion::Cargados(v) => {
            for (id, r, portada) in v {
                if let Some(png) = portada {
                    app.iconos
                        .portadas
                        .entry(id.clone())
                        .or_insert_with(|| image::Handle::from_bytes(png));
                }
                if let Ok((png, variacion)) = r {
                    let handle = png.as_ref().map(|p| image::Handle::from_bytes(p.clone()));
                    app.iconos.cache.entry(id).or_insert(Guardado {
                        png,
                        variacion,
                        handle,
                    });
                }
            }
        }
        Accion::Deshacer(seq) => {
            if app.iconos.undo.as_ref().is_some_and(|(n, _, _)| *n == seq) {
                if let Some((_, id, old)) = app.iconos.undo.take() {
                    let store = app.servicios.instancias.almacen.clone();
                    return Task::perform(
                        async move {
                            let workid = id.clone();
                            let r = tokio::task::spawn_blocking(move || {
                                core::deshacer(&store, &workid, &old)
                            })
                            .await
                            .unwrap_or_else(|_| Err("No se pudo restaurar el icono.".into()));
                            (id, r)
                        },
                        |(id, r)| m(Accion::Deshecho(id, r)),
                    );
                }
            }
        }
        Accion::Deshecho(id, r) => match r {
            Ok(()) => {
                app.iconos.cache.remove(&id);
                app.avisar("Icono anterior restaurado.".into());
                return cargar(app);
            }
            Err(e) => app.avisar_error(e),
        },
    }
    Task::none()
}
pub fn evento(app: &App, e: &iced::Event) -> Option<Accion> {
    let d = app.iconos.dialogo.as_ref()?;
    if d.ocupada {
        return None;
    }
    match e {
        iced::Event::Window(iced::window::Event::FileDropped(path)) => {
            Some(Accion::Archivo(path.clone()))
        }
        iced::Event::Keyboard(iced::keyboard::Event::KeyPressed { key, modifiers, .. }) => {
            use iced::keyboard::{key::Named, Key};
            let step = if modifiers.shift() { 1. } else { 8. };
            match key {
                Key::Named(Named::Escape) => Some(Accion::Cerrar),
                Key::Named(Named::ArrowLeft) => Some(Accion::Mover(-step, 0.)),
                Key::Named(Named::ArrowRight) => Some(Accion::Mover(step, 0.)),
                Key::Named(Named::ArrowUp) => Some(Accion::Mover(0., -step)),
                Key::Named(Named::ArrowDown) => Some(Accion::Mover(0., step)),
                _ => None,
            }
        }
        iced::Event::Mouse(iced::mouse::Event::WheelScrolled { delta }) if d.tab == 0 => {
            let y = match delta {
                iced::mouse::ScrollDelta::Lines { y, .. }
                | iced::mouse::ScrollDelta::Pixels { y, .. } => *y,
            };
            Some(Accion::Zoom(
                (d.zoom as i32 + (y.signum() * 10.) as i32).clamp(100, 400) as u32,
            ))
        }
        iced::Event::Mouse(iced::mouse::Event::ButtonReleased(iced::mouse::Button::Left)) => {
            Some(Accion::Soltar)
        }
        _ => None,
    }
}
pub(super) fn procedural<'a>(p: Paleta, name: &str, var: u32, size: f32) -> Element<'a, Mensaje> {
    let bits = core::patron(name, var);
    let colors = [
        p.surface_sunken,
        p.border,
        p.border_strong,
        p.text_muted,
        p.text,
    ];
    let mut rows = column![];
    for y in 0..8 {
        let mut r = row![];
        for x in 0..8 {
            r = r.push(
                container(Space::new(size / 8., size / 8.))
                    .style(icaro_ui::estilo::bloque(colors[bits[y * 8 + x] as usize])),
            );
        }
        rows = rows.push(r);
    }
    container(rows).width(size).height(size).into()
}
fn display<'a>(
    p: Paleta,
    name: &str,
    png: Option<&image::Handle>,
    var: u32,
    size: f32,
) -> Element<'a, Mensaje> {
    if let Some(png) = png {
        image(png.clone())
            .width(size)
            .height(size)
            .filter_method(image::FilterMethod::Nearest)
            .into()
    } else {
        procedural(p, name, var, size)
    }
}
struct Registro(Paleta);
impl iced::widget::canvas::Program<Mensaje> for Registro {
    type State = ();
    fn draw(
        &self,
        _: &(),
        renderer: &iced::Renderer,
        _: &iced::Theme,
        bounds: iced::Rectangle,
        _: iced::mouse::Cursor,
    ) -> Vec<iced::widget::canvas::Geometry> {
        use iced::widget::canvas::{Frame, Path, Stroke};
        let mut frame = Frame::new(renderer, bounds.size());
        for (x, y, sx, sy) in [
            (6., 6., 1., 1.),
            (bounds.width - 6., 6., -1., 1.),
            (6., bounds.height - 6., 1., -1.),
            (bounds.width - 6., bounds.height - 6., -1., -1.),
        ] {
            let path = Path::new(|b| {
                b.move_to(Point::new(x, y + 14. * sy));
                b.line_to(Point::new(x, y));
                b.line_to(Point::new(x + 14. * sx, y));
            });
            frame.stroke(
                &path,
                Stroke::default().with_width(2.).with_color(self.0.text),
            );
        }
        vec![frame.into_geometry()]
    }
}
pub fn imagen<'a>(app: &'a App, index: usize, size: f32) -> Element<'a, Mensaje> {
    let cards = crate::instancias();
    let name = cards
        .get(index)
        .map(|c| c.nombre.as_str())
        .unwrap_or("Instancia");
    let id = app
        .servicios
        .instancias
        .registros
        .get(index)
        .map(|i| i.id.clone())
        .unwrap_or_else(|| format!("demo-{index}"));
    let v = app.iconos.cache.get(&id);
    display(
        app.paleta(),
        name,
        v.and_then(|v| v.handle.as_ref()),
        v.map_or(0, |v| v.variacion),
        size,
    )
}
pub fn preparar_diseno(app: &mut App, scene: &str) {
    let _ = actualizar(app, Accion::Abrir(0));
    if scene == "icono-imagen" {
        if let Some(d) = app.iconos.dialogo.as_mut() {
            let img =
                ::image::load_from_memory(include_bytes!("../../assets/laminas/valle-rocas.webp"))
                    .unwrap()
                    .into_rgba8();
            let source = Fuente {
                nombre: "imagen-de-ejemplo.webp".into(),
                imagen: std::sync::Arc::new(img),
            };
            let png = core::recortar(&source, 160, 0., 0., 32).unwrap();
            d.tab = 0;
            d.zoom = 160;
            d.handle = Some(image::Handle::from_bytes(png.clone()));
            d.png = Some(png);
            d.fuente = Some(source);
        }
    }
}
pub fn vista<'a>(app: &'a App) -> Option<Element<'a, Mensaje>> {
    let d = app.iconos.dialogo.as_ref()?;
    let p = app.paleta();
    let mut tabs = row![];
    for (i, (label, ico)) in [
        ("Imagen", Icono::Imagen),
        ("Generado", Icono::Cuadricula),
        ("Capturas", Icono::Capturas),
    ]
    .into_iter()
    .enumerate()
    {
        tabs = tabs.push(componentes::boton_con_icono(
            p,
            ico,
            label,
            if d.tab == i {
                Variante::Primario
            } else {
                Variante::Secundario
            },
            Some(m(Accion::Tab(i))),
            false,
        ));
    }
    let mut left = column![tabs].spacing(20);
    match d.tab {
        0 => {
            let area: Element<'a, Mensaje> = if let Some(png) = d.handle.as_ref() {
                display(p, &d.nombre, Some(png), 0, 280.)
            } else {
                container(
                    column![
                        icaro_ui::iconos::icono(Icono::Imagen, Tam::Md, p.text_muted),
                        text("Elegir o soltar imagen")
                            .size(texto::BODY_SM.0)
                            .color(p.text_muted)
                    ]
                    .spacing(12)
                    .align_x(Alignment::Center),
                )
                .center(280)
                .style(icaro_ui::estilo::marco(p))
                .into()
            };
            let visor = mouse_area(
                container(iced::widget::stack![
                    area,
                    iced::widget::canvas(Registro(p)).width(280).height(280)
                ])
                .width(280)
                .height(280)
                .style(icaro_ui::estilo::marco(p)),
            )
            .on_move(|p| m(Accion::Cursor(p)))
            .on_press(m(Accion::Pulsar))
            .on_release(m(Accion::Soltar));
            let (name, dim) = d
                .fuente
                .as_ref()
                .map(|f| {
                    (
                        f.nombre.clone(),
                        format!("{} × {}", f.imagen.width(), f.imagen.height()),
                    )
                })
                .unwrap_or_else(|| ("PNG, JPG, WebP o GIF".into(), "Hasta 10 MB".into()));
            left = left.push(
                row![
                    visor,
                    column![
                        text(name).font(fuentes::TITULO).size(14).color(p.text),
                        text(dim)
                            .font(fuentes::MONO)
                            .size(texto::MONO_SM.0)
                            .color(p.text_muted),
                        componentes::etiqueta(p, &format!("Zoom {} %", d.zoom)),
                        slider(100..=400, d.zoom, |v| m(Accion::Zoom(v)))
                            .step(1u32)
                            .style(move |_, _| iced::widget::slider::Style {
                                rail: iced::widget::slider::Rail {
                                    backgrounds: (p.border_strong.into(), p.border_strong.into()),
                                    width: 4.,
                                    border: iced::Border::default()
                                },
                                handle: iced::widget::slider::Handle {
                                    shape: iced::widget::slider::HandleShape::Rectangle {
                                        width: 16,
                                        border_radius: 0.into()
                                    },
                                    background: p.text.into(),
                                    border_width: 2.,
                                    border_color: p.bg
                                }
                            }),
                        componentes::boton_con_icono(
                            p,
                            Icono::Carpeta,
                            "Elegir otra imagen",
                            Variante::Secundario,
                            Some(m(Accion::ElegirImagen)),
                            false
                        ),
                        componentes::boton_con_icono(
                            p,
                            Icono::Refrescar,
                            "Restablecer",
                            Variante::Fantasma,
                            Some(m(Accion::Reset)),
                            false
                        ),
                        text("También puedes soltar una imagen sobre el recuadro.")
                            .font(fuentes::MONO)
                            .size(texto::MONO_SM.0)
                            .color(p.text_muted)
                    ]
                    .spacing(14)
                    .width(Length::Fill)
                ]
                .spacing(20),
            );
            left=left.push(componentes::etiqueta(p,"Estilo")).push(componentes::segmentado(p,&["Original","Píxeles 32","Píxeles 16"],d.estilo,|i|m(Accion::Estilo(i)))).push(text("Píxeles reduce la imagen sin suavizar para que combine con el estilo de Minecraft.").size(texto::BODY_SM.0).color(p.text_muted));
        }
        1 => {
            let cols = if app.ventana.width >= 1050. { 6 } else { 3 };
            let mut grid = column![].spacing(16);
            for y in 0..(12 / cols) {
                let mut line = row![].spacing(16);
                for x in 0..cols {
                    let n = d.tanda * 12 + y * cols + x;
                    line = line.push(
                        button(
                            container(procedural(p, &d.nombre, n, 64.))
                                .padding(4)
                                .style(icaro_ui::estilo::tarjeta(p, n == d.variacion)),
                        )
                        .padding(0)
                        .on_press(m(Accion::Generado(n)))
                        .style(icaro_ui::estilo::boton_fantasma(p)),
                    );
                }
                grid = grid.push(line);
            }
            left = left.push(grid).push(componentes::boton_con_icono(
                p,
                Icono::Refrescar,
                "Otra tanda",
                Variante::Secundario,
                Some(m(Accion::OtraTanda)),
                false,
            ));
        }
        _ => {
            let mut captures = column![].spacing(12);
            if d.capturas.is_empty() {
                captures = captures.push(
                    text("Esta instancia todavía no tiene capturas.")
                        .size(texto::BODY_SM.0)
                        .color(p.text_muted),
                );
            }
            for chunk in d.capturas.chunks(3) {
                let mut line = row![].spacing(12);
                for path in chunk {
                    line = line.push(
                        button(
                            image(image::Handle::from_path(path))
                                .width(140)
                                .height(90)
                                .content_fit(iced::ContentFit::Cover),
                        )
                        .padding(0)
                        .on_press(m(Accion::Archivo(path.clone())))
                        .style(icaro_ui::estilo::boton_fantasma(p)),
                    );
                }
                captures = captures.push(line);
            }
            left = left.push(scrollable(captures).height(Length::Fill)).push(
                text(format!(
                    "Capturas de {}. Al elegir una pasa al recorte.",
                    d.nombre
                ))
                .font(fuentes::MONO)
                .size(texto::MONO_SM.0)
                .color(p.text_muted),
            );
        }
    }
    if let Some(e) = &d.error {
        left = left.push(componentes::aviso(p, componentes::Estado::Error, e));
    }
    let preview_png = if d.tab == 1 { None } else { d.handle.as_ref() };
    let mut previews = row![].spacing(20).align_y(Alignment::End);
    for size in [64., 48., 24.] {
        previews = previews.push(
            column![
                display(p, &d.nombre, preview_png, d.variacion, size),
                text(format!("{}", size as u32))
                    .font(fuentes::MONO)
                    .size(texto::MONO_SM.0)
                    .color(p.text_muted)
            ]
            .spacing(8)
            .align_x(Alignment::Center),
        );
    }
    let right = column![
        componentes::etiqueta(p, "Vista previa"),
        previews,
        container(Space::new(Length::Fill, 1)).style(icaro_ui::estilo::bloque(p.border)),
        componentes::etiqueta(p, "Así se verá"),
        container(
            row![
                display(p, &d.nombre, preview_png, d.variacion, 48.),
                column![
                    text(d.nombre.clone())
                        .font(fuentes::TITULO)
                        .size(15)
                        .color(p.text),
                    text(d.version.clone())
                        .font(fuentes::MONO)
                        .size(texto::MONO_SM.0)
                        .color(p.text_muted)
                ]
            ]
            .spacing(12)
            .align_y(Alignment::Center)
        )
        .padding(12)
        .style(icaro_ui::estilo::marco(p)),
        container(
            row![
                display(p, &d.nombre, preview_png, d.variacion, 24.),
                text(d.nombre.clone()).size(texto::BODY_SM.0).color(p.bg)
            ]
            .spacing(10)
            .align_y(Alignment::Center)
        )
        .padding(12)
        .style(icaro_ui::estilo::bloque(p.text)),
        Space::with_height(Length::Fill),
        componentes::boton_con_icono(
            p,
            Icono::Deshacer,
            "Volver al icono generado",
            Variante::Fantasma,
            Some(m(Accion::VolverGenerado)),
            false
        )
    ]
    .spacing(22);
    let body = row![
        container(left)
            .width(Length::Fill)
            .padding(Padding::from([8., 0.])),
        container(Space::new(1, Length::Fill)).style(icaro_ui::estilo::bloque(p.border)),
        container(right)
            .width(272)
            .padding(Padding::from([8., 14.]))
    ]
    .spacing(20);
    let footer = row![
        Space::with_width(Length::Fill),
        componentes::boton(
            p,
            "Cancelar",
            Variante::Secundario,
            (!d.ocupada).then(|| m(Accion::Cerrar))
        ),
        componentes::boton_con_icono(
            p,
            Icono::Check,
            if d.ocupada {
                "Guardando…"
            } else {
                "Usar icono"
            },
            Variante::Primario,
            (!d.ocupada).then(|| m(Accion::Guardar)),
            false
        )
    ]
    .spacing(8)
    .align_y(Alignment::Center);
    Some(crate::pantallas::formulario_icono(
        p,
        &format!("Icono de {}", d.nombre),
        body.into(),
        footer.into(),
        (app.ventana.width - 48.).clamp(700., 1000.),
        (app.ventana.height - 80.).clamp(480., 700.),
        m(Accion::Cerrar),
    ))
}
