// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Estructura de la ventana. Ejecutar con `cargo run -p icaro-ui --example shell`.

use icaro_ui::componentes::{self, Variante};
use icaro_ui::estilo;
use icaro_ui::fuentes;
use icaro_ui::laminas::{banda, Lamina};
use icaro_ui::movimiento::{losa, Preferencia};
use icaro_ui::shell::{app_shell, Cuenta, DescargasActivas, MensajesShell, Seccion};
use icaro_ui::tema::duracion;
use icaro_ui::tema::{espacio, texto, Modo};
use iced::widget::{column, container, stack, text};
use iced::{window, Element, Length, Padding, Size, Subscription, Task};
use std::time::Instant;

#[derive(Default)]
struct App {
    modo: Modo,
    seccion: Option<Seccion>,
    colapsada: bool,
    /// Instante en que apareció el aviso, si está a la vista.
    aviso: Option<Instant>,
    ahora: Option<Instant>,
}

#[derive(Debug, Clone)]
enum Mensaje {
    Ir(Seccion),
    CambiarTema,
    Colapsar,
    Aviso,
    Cuadro(Instant),
    Arrastrar,
    Minimizar,
    Maximizar,
    Cerrar,
    Nada,
    ImagenesActualizadas,
}

fn actualizar(app: &mut App, m: Mensaje) -> Task<Mensaje> {
    match m {
        Mensaje::Ir(s) => app.seccion = Some(s),
        Mensaje::CambiarTema => {
            app.modo = match app.modo {
                Modo::Tinta => Modo::Piedra,
                Modo::Piedra => Modo::Tinta,
            }
        }
        Mensaje::Colapsar => app.colapsada = !app.colapsada,
        Mensaje::Aviso => {
            let ahora = Instant::now();
            app.ahora = Some(ahora);
            app.aviso = Some(ahora);
        }
        Mensaje::Cuadro(ahora) => app.ahora = Some(ahora),
        Mensaje::Arrastrar => return window::get_oldest().and_then(window::drag),
        Mensaje::Minimizar => {
            return window::get_oldest().and_then(|id| window::minimize(id, true))
        }
        Mensaje::Maximizar => return window::get_oldest().and_then(window::toggle_maximize),
        Mensaje::Cerrar => return window::get_oldest().and_then(window::close),
        Mensaje::Nada | Mensaje::ImagenesActualizadas => {}
    }
    Task::none()
}

fn suscripcion(app: &App) -> Subscription<Mensaje> {
    let animacion = match (app.aviso, app.ahora) {
        (Some(inicio), Some(ahora)) if progreso(inicio, ahora) < 1.0 => {
            window::frames().map(Mensaje::Cuadro)
        }
        _ => Subscription::none(),
    };
    Subscription::batch([animacion,
        icaro_ui::laminas::suscripcion().map(|()| Mensaje::ImagenesActualizadas)])
}

fn progreso(inicio: Instant, ahora: Instant) -> f32 {
    let total = Preferencia::default().duracion(duracion::LOSA);
    (ahora.saturating_duration_since(inicio).as_secs_f32() / total.as_secs_f32()).min(1.0)
}

fn vista(app: &App) -> Element<'_, Mensaje> {
    let p = app.modo.paleta();
    let activa = app.seccion.unwrap_or(Seccion::Instancias);
    let cuerpo = column![
        componentes::boton(
            p,
            "Cambiar tema",
            Variante::Secundario,
            Some(Mensaje::CambiarTema)
        ),
        componentes::boton(
            p,
            "Colapsar barra",
            Variante::Secundario,
            Some(Mensaje::Colapsar)
        ),
        componentes::boton(
            p,
            "Mostrar aviso",
            Variante::Secundario,
            Some(Mensaje::Aviso)
        ),
    ]
    .spacing(espacio::S4)
    .padding(espacio::S12);
    let contenido = column![
        banda(p, Lamina::CaidaCielo, activa.nombre(), Some(6)),
        cuerpo
    ];
    let pie = componentes::boton(p, "Jugar", Variante::Primario, Some(Mensaje::Nada));
    let ventana = app_shell(
        p,
        activa,
        app.colapsada,
        Cuenta {
            nombre: "Mineral_7",
            proveedor: "Microsoft",
            estado: icaro_ui::cuenta::EstadoSesion::Activa,
            textura: None,
        },
        Some(DescargasActivas {
            cantidad: 2,
            velocidad: "6,1 MB/s".into(),
            avance: 0.58,
        }),
        contenido.into(),
        Some(pie),
        MensajesShell {
            ir_a: Box::new(Mensaje::Ir),
            arrastrar: Mensaje::Arrastrar,
            buscar: Mensaje::Nada,
            alternar_barra: Mensaje::Colapsar,
            cuenta: Mensaje::Nada,
            minimizar: Mensaje::Minimizar,
            maximizar: Mensaje::Maximizar,
            cerrar: Mensaje::Cerrar,
        },
    );
    let Some(inicio) = app.aviso else {
        return ventana;
    };
    let t = progreso(inicio, app.ahora.unwrap_or(inicio));
    // La losa cae desde arriba: el desplazamiento se resta del margen superior.
    let arriba = 48.0 - losa(t);
    let tarjeta = container(
        text("Instancia exportada")
            .font(fuentes::CUERPO_NEGRITA)
            .size(texto::BODY.0),
    )
    .padding(espacio::S4)
    .style(estilo::flotante(p));
    stack![
        ventana,
        container(tarjeta)
            .width(Length::Fill)
            .align_x(iced::alignment::Horizontal::Center)
            .padding(Padding::default().top(arriba))
    ]
    .into()
}

fn main() -> iced::Result {
    let mut app = iced::application("Ícaro", actualizar, vista)
        .subscription(suscripcion)
        .theme(|_| iced::Theme::Dark)
        .default_font(fuentes::CUERPO)
        .window(window::Settings {
            decorations: false,
            size: Size::new(1200.0, 760.0),
            min_size: Some(Size::new(940.0, 600.0)),
            ..Default::default()
        });
    for f in fuentes::BYTES {
        app = app.font(f);
    }
    app.run()
}
