// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss
//! Datos del preview del System Design, exclusivamente para capturas QA.
use crate::{App, Mensaje};
use icaro_ui::{componentes::{self, Estado}, fuentes, iconos::Icono, progreso, superficies};
use iced::{widget::{column, container, row, text, Space}, Element, Length};

pub fn vista<'a>(app: &'a App, scene: &str) -> Option<Element<'a, Mensaje>> {
    if scene.starts_with("compartir-controlador") {return None;}
    if scene.starts_with("compartir-") { return crate::compartir_diseno::vista(app, scene); }
    let p = app.paleta();
    let vista: Element<'a, Mensaje> = match scene {
        "picar" => return None,
        "apariencia" => componentes::apariencia(p, app.tema, app.aj.metal, app.aj.escala,
            Mensaje::Tema, |i| Mensaje::Pant(crate::pantallas::Accion::Metal(i)),
            |v| Mensaje::Pant(crate::pantallas::Accion::Escala(v))),
        "insignia" => column![row![componentes::insignia_solida(p, "Nueva"),
            componentes::insignia_contorno(p, "Cliente"),
            componentes::insignia(p, Estado::Exito, "Compatible"),
            componentes::insignia(p, Estado::Aviso, "Desactualizada"),
            componentes::insignia(p, Estado::Error, "Conflicto"),
            componentes::insignia(p, Estado::Info, "Beta")].spacing(8),
            row![componentes::aviso_detallado(p, Estado::Aviso, "Hay una versión nueva de Sodium",
                "0.6.5 corrige un cierre al cambiar de dimensión. Actualiza desde la pestaña Mods."),
                componentes::aviso_detallado(p, Estado::Error, "Java 17 no sirve para 1.21",
                "Instala Java 21 desde Ajustes > Java o deja que Icaro Launcher lo descargue.")].spacing(16),
            row![componentes::aviso_detallado(p, Estado::Exito, "Integridad verificada", "1.482 archivos revisados. Ninguno dañado."),
                componentes::aviso_detallado(p, Estado::Info, "Esta instancia usa una snapshot",
                "Las snapshots pueden dañar mundos. Haz una copia antes de jugar.")].spacing(16)
        ].spacing(16).into(),
        "progreso" | "progreso-animado" => {
            let reducir = scene != "progreso-animado";
            let rotulo = |titulo: &str, dato: &str| row![text(titulo.to_owned()).size(13).color(p.text),
                Space::with_width(Length::Fill), text(dato.to_owned()).font(fuentes::MONO).size(11).color(p.text_muted)];
            column![row![
                column![rotulo("Instalando Fabric 0.16.9", "112 de 186"), progreso::bloques(p, 186, 112, reducir)].spacing(8).width(Length::Fill),
                column![rotulo("Descargando assets", "412,8 de 687,0 MB · 6,1 MB/s"), progreso::continuo(p, 0.6, reducir)].spacing(8).width(Length::Fill)
            ].spacing(48),
                row![column![rotulo("Resolviendo dependencias", ""), progreso::indeterminado(p, reducir)].spacing(8).width(Length::Fill),
                    row![progreso::spinner(p, reducir), text("Consultando Modrinth").size(13).color(p.text_muted)].spacing(12).width(Length::Fill)
                ].spacing(48),
                row![progreso::esqueleto(p, 48, 48, reducir),
                    column![progreso::esqueleto(p, 320, 14, reducir), progreso::esqueleto(p, Length::Fill, 12, reducir)]
                    .spacing(8).width(Length::Fill)].spacing(16)
            ].spacing(32).into()
        }
        "toast" => column![superficies::banner(p, Estado::Aviso, Icono::Aviso, "Sin conexión.",
            "Puedes jugar instancias ya instaladas; la tienda y las descargas esperarán.", Some(("Reintentar", Mensaje::Nada))),
            superficies::toast(p, Icono::Check, "Se quitaron 3 mods de Supervivencia.", Some(("Deshacer", Mensaje::Nada)), Some(Mensaje::Nada), false),
            superficies::toast(p, Icono::Alerta, "No se pudo descargar Iris 1.8.1: Modrinth no responde.", Some(("Ver detalle", Mensaje::Nada)), None, true)
        ].spacing(16).into(),
        _ => return None,
    };
    Some(container(vista).width(Length::Fill).padding(32).into())
}

pub fn preparar(app: &mut App, scene: &str) {
    if scene == "picar" {
        app.picar_objetivo = Some(0);
        app.picar_revision=Some(crate::picar_real::Revision::default());
        app.picar = Some(crate::Picar { instancia: 0, inicio: app.ahora - std::time::Duration::from_millis(480),
            soltada: None, rota: None });
        app.reducir = false;
    }
}
