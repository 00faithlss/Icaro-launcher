// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss
//! Confirmación por mantener: reutiliza la tarjeta y las grietas del DS.
use crate::{App, Mensaje};
use icaro_core::instancias::Almacen;
use icaro_ui::{componentes, fuentes, iconos::{icono, Icono, Tam}, instancias::{self, AccionesTarjeta}, tema::{espacio, texto}};
use iced::{widget::{column, container, mouse_area, row, stack, text, Space}, Alignment, Element, Length, Padding, Task};
use std::{fs, path::{Path, PathBuf}};

#[derive(Debug, Clone, Default)]
pub struct Revision { pub mundos: usize, pub sin_respaldo: usize }
#[derive(Debug, Clone)]
pub enum Accion { Revisada(String, u64, Result<Revision, String>) }

/// La comprobación no bloquea ni el render ni el gesto de mantener.
pub fn revisar(app: &App, indice: usize, secuencia: u64) -> Task<Mensaje> {
    let Some(i) = app.servicios.instancias.registros.get(indice) else { return Task::none(); };
    let id = i.id.clone();
    let store = app.servicios.instancias.almacen.clone();
    Task::perform(async move {
        let target = id.clone();
        let r = tokio::task::spawn_blocking(move || revisar_mundos(&store, &target)).await
            .map_err(|_| "No se pudieron revisar los respaldos de los mundos.".to_owned()).and_then(|r| r);
        (id, r)
    }, move |(id, r)| Mensaje::Picar(Accion::Revisada(id, secuencia, r)))
}

fn leer_directorio(path: &Path) -> Result<Vec<fs::DirEntry>, String> {
    match fs::read_dir(path) {
        Ok(entries) => entries.collect::<Result<Vec<_>, _>>().map_err(|_| "No se pudieron leer los mundos o sus respaldos.".into()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(_) => Err("No se pudieron leer los mundos o sus respaldos.".into()),
    }
}

/// Solo considera vigente una copia que contiene todos los archivos actuales
/// y es posterior a su última modificación. ZIP externos se tratan con cautela.
fn copiar_vigente(origen: &Path, copia: &Path, archivos: &mut usize, profundidad: usize) -> bool {
    if profundidad > 32 { return false; }
    let Ok(listado) = leer_directorio(origen) else { return false; };
    for e in listado {
        *archivos += 1;
        if *archivos > 100_000 { return false; }
        let Ok(t) = e.file_type() else { return false; };
        if t.is_symlink() { return false; }
        let dst = copia.join(e.file_name());
        let Ok(md) = fs::symlink_metadata(&dst) else { return false; };
        if md.file_type().is_symlink() { return false; }
        if t.is_dir() {
            if !md.is_dir() || !copiar_vigente(&e.path(), &dst, archivos, profundidad + 1) { return false; }
        } else if t.is_file() {
            let Ok(original) = e.metadata() else { return false; };
            if !md.is_file() || original.len() != md.len() { return false; }
            let (Ok(modificado), Ok(respaldado)) = (original.modified(), md.modified()) else { return false; };
            if respaldado < modificado { return false; }
        } else { return false; }
    }
    true
}

fn revisar_mundos(store: &Almacen, id: &str) -> Result<Revision, String> {
    let game = store.juego(id).map_err(|e| e.to_string())?;
    let raiz = store.carpeta(id).map_err(|e| e.to_string())?;
    let mut copias: Vec<PathBuf> = Vec::new();
    for e in leer_directorio(&raiz.join(".icaro-backups"))?.into_iter().take(256) {
        if e.file_type().is_ok_and(|t| t.is_dir() && !t.is_symlink()) { copias.push(e.path().join("saves")); }
    }
    let mut revision = Revision::default();
    for mundo in leer_directorio(&game.join("saves"))? {
        let tipo = mundo.file_type().map_err(|_| "No se pudo revisar un mundo.".to_owned())?;
        if tipo.is_symlink() {
            revision.mundos += 1; revision.sin_respaldo += 1; continue;
        }
        if !tipo.is_dir() || !mundo.path().join("level.dat").is_file() { continue; }
        revision.mundos += 1;
        let seguro = copias.iter().any(|c| copiar_vigente(&mundo.path(), &c.join(mundo.file_name()), &mut 0, 0));
        if !seguro { revision.sin_respaldo += 1; }
    }
    Ok(revision)
}

pub fn vista(app: &App) -> Element<'_, Mensaje> {
    let p = app.paleta();
    let Some(i) = app.picar_objetivo else { return Space::new(0, 0).into(); };
    let Some(d) = crate::con_instancias(|l| l.get(i).cloned()) else { return Space::new(0, 0).into(); };
    let etapa = app.picar.as_ref().filter(|pi| pi.instancia == i)
        .map_or(0, |pi| crate::etapa_picar(pi, app.ahora));
    let ocupado = app.picar_revision.is_none() || app.servicios.instancias.operando || app.picar.as_ref().is_some_and(|pi| pi.rota.is_some());
    let tarjeta = instancias::tarjeta_instancia_con_icono(p, &d, AccionesTarjeta {
        seleccionar: Mensaje::Nada, principal: Mensaje::Nada, mas: Mensaje::Nada,
    }, Some(crate::icono_instancia::imagen(app,i,64.)));
    // El pie original queda cubierto por el control de mantener, con el mismo
    // alto y regla. Así la tarjeta y el sello procedimental permanecen iguales.
    let etiqueta = if app.picar_revision.is_none() {"Revisando mundos…"}else if ocupado { "Eliminando…" } else { "Mantén para eliminar" };
    let relleno = container(row![container(Space::new(30. * etapa.min(10) as f32, 44))
        .style(icaro_ui::estilo::bloque(iced::Color { a: if etapa == 0 { 0. } else { 0.22 }, ..p.error })),
        Space::with_width(Length::Fill)])
        .width(300).height(44);
    let pie = container(stack![relleno, container(row![icono(Icono::Papelera, Tam::Base, p.error),
        text(etiqueta).font(fuentes::ETIQUETA).size(texto::LABEL.0).color(p.error)]
        .spacing(8).align_y(Alignment::Center)).center_x(300).center_y(44)])
        .width(300).height(44).style(move |_| iced::widget::container::Style {
            background: Some(p.surface.into()), border: iced::Border { color: p.border, width: 1., ..Default::default() }, ..Default::default()
        });
    let pie: Element<'_, Mensaje> = if ocupado { pie.into() } else {
        mouse_area(pie).on_press(Mensaje::PicarEmpezar).on_release(Mensaje::PicarSoltar)
            .on_exit(Mensaje::PicarSoltar).interaction(iced::mouse::Interaction::Pointer).into()
    };
    let tarjeta = stack![tarjeta, container(pie).height(instancias::ALTO_TARJETA).align_bottom(instancias::ALTO_TARJETA)].into();
    let tarjeta = crate::con_efectos(app, i, &d.nombre, tarjeta);
    let mut bloques = row![].spacing(4);
    for n in 0..10 {
        bloques = bloques.push(container(Space::new(18, 18)).style(move |_| container::Style {
            background: Some(if n < etapa.min(10) { p.text.into() } else { p.surface.into() }),
            border: iced::Border { width: 1., color: p.border_strong, radius: 0.0.into() },
            ..Default::default()
        }));
    }
    let detalles = column![text(format!("¿Estás seguro de que quieres eliminar {}?", d.nombre)).font(fuentes::TITULO).size(28).color(p.text),
        text("Se quitarán de esta instancia tus mods, mundos, capturas y progreso. Exporta una copia antes de continuar. Puedes recuperarla desde la papelera mientras conserve sus archivos.")
            .size(texto::BODY.0).color(p.text_muted),
        text("Mantén presionado para eliminar. Si sueltas antes, se cancela.").size(texto::BODY_SM.0).color(p.text_muted),
        column![componentes::etiqueta(p, "Progreso"), bloques].spacing(8),
        row![componentes::insignia_contorno(p, "Supr"), text("Mantener con la tarjeta enfocada").size(texto::BODY_SM.0).color(p.text_muted)]
            .spacing(12).align_y(Alignment::Center),
    ].spacing(20);
    let cancelar=componentes::boton(p,"Cancelar",componentes::Variante::Secundario,
        (!app.servicios.instancias.operando && !app.picar.as_ref().is_some_and(|s|s.rota.is_some())).then_some(Mensaje::CerrarDialogo));
    let instrucciones=column![
        container(detalles).height(instancias::ALTO_TARJETA-44.),
        container(cancelar).height(44).align_y(Alignment::Center)
    ].width(Length::Fill).height(instancias::ALTO_TARJETA);
    container(row![tarjeta, instrucciones].spacing(48)).width((app.ventana.width-64.).clamp(700.,980.)).height(Length::Shrink)
        .padding(Padding::from([espacio::S8, espacio::S8])).style(move |_| container::Style {
            background: Some(p.bg.into()),
            border: iced::Border { width: 2., color: p.text, radius: 0.0.into() },
            ..Default::default()
        }).into()
}

/// Picar sin escombros: la tarjeta se funde con el lienzo en dur-fast (120 ms).
pub fn fundido<'a>(p: icaro_ui::tema::Paleta, tarjeta: Element<'a, Mensaje>, ms: f32) -> Element<'a, Mensaje> {
    let alfa = (ms / 120.).clamp(0., 1.);
    stack![tarjeta, container(Space::new(instancias::ANCHO_TARJETA, instancias::ALTO_TARJETA))
        .style(icaro_ui::estilo::bloque(iced::Color { a: alfa, ..p.bg }))].into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;
    #[test]
    fn mundos_sin_copia_y_archivos_nuevos_exigen_confirmacion() {
        let ruta = std::env::temp_dir().join(format!("icaro-picar-{}", SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&ruta).unwrap();
        let store = Almacen::nuevo(&ruta);
        let i = store.crear("Prueba", "1.21.4", 0).unwrap();
        let world = store.juego(&i.id).unwrap().join("saves/Prueba");
        fs::create_dir_all(&world).unwrap(); fs::write(world.join("level.dat"), b"mundo").unwrap();
        assert_eq!(revisar_mundos(&store, &i.id).unwrap().sin_respaldo, 1);
        let backup = store.carpeta(&i.id).unwrap().join(".icaro-backups/copia/saves/Prueba");
        fs::create_dir_all(&backup).unwrap(); fs::copy(world.join("level.dat"), backup.join("level.dat")).unwrap();
        assert_eq!(revisar_mundos(&store, &i.id).unwrap().sin_respaldo, 0);
        fs::write(world.join("nuevo.dat"), b"nuevo").unwrap();
        assert_eq!(revisar_mundos(&store, &i.id).unwrap().sin_respaldo, 1);
        fs::remove_dir_all(&ruta).unwrap();
    }
}
