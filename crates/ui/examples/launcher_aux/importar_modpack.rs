// SPDX-License-Identifier: GPL-3.0-only
//! ImportarModpack y ZonaSoltar del diseño entregado; los estados proceden del instalador.
use super::{m, Accion};
use crate::{App, Mensaje};
use icaro_ui::{
    componentes::{self as c, Variante},
    estilo, fuentes,
    iconos::{icono, Icono, Tam},
    laminas::{self, Lamina},
    tema::Paleta,
};
use iced::{
    widget::{
        button, column, container, pick_list, row, scrollable, stack, text, text_input, Space,
    },
    Alignment, Element, Length,
};

fn mono<'a>(p: Paleta, s: impl Into<String>) -> Element<'a, Mensaje> {
    text(s.into())
        .font(fuentes::MONO)
        .size(12)
        .color(p.text_muted)
        .into()
}
fn caja(p: Paleta) -> impl Fn(&iced::Theme) -> container::Style {
    move |_| container::Style {
        background: Some(p.surface_sunken.into()),
        border: iced::Border {
            color: p.border,
            width: 1.,
            ..Default::default()
        },
        ..Default::default()
    }
}
fn regla<'a>(p: Paleta, fuerte: bool) -> Element<'a, Mensaje> {
    container(Space::new(Length::Fill, if fuerte { 2. } else { 1. }))
        .style(estilo::bloque(if fuerte { p.text } else { p.border }))
        .into()
}
fn dato<'a>(p: Paleta, glifo: Icono, label: String, valor: String) -> Element<'a, Mensaje> {
    column![
        container(
            row![
                icono(glifo, Tam::Sm, p.text),
                text(label)
                    .size(13)
                    .color(p.text)
                    .width(Length::FillPortion(1)),
                container(mono(p, valor))
                    .width(Length::FillPortion(1))
                    .align_right(Length::Fill)
            ]
            .spacing(12)
            .align_y(Alignment::Center)
        )
        .padding([10, 0]),
        regla(p, false)
    ]
    .into()
}
fn bytes(n: u64) -> String {
    let valor = if n >= 1024 * 1024 * 1024 {
        format!("{:.1} GB", n as f64 / (1024. * 1024. * 1024.))
    } else if n >= 1024 * 1024 {
        format!("{:.1} MB", n as f64 / (1024. * 1024.))
    } else {
        format!("{:.1} KB", n as f64 / 1024.)
    };
    valor.replace('.', ",")
}
fn ruta<'a>(app: &'a App) -> Element<'a, Mensaje> {
    let s = &app.servicios.modpacks;
    let p = app.paleta();
    let disponible = !s.busy && !s.creando && !s.eligiendo;
    let entrada = text_input("C:\\Modpacks\\mi-modpack.mrpack", &s.ruta)
        .on_input_maybe(disponible.then_some(|v| m(Accion::Ruta(v))))
        .on_submit_maybe(disponible.then(|| m(Accion::AbrirRuta)))
        .font(fuentes::CUERPO)
        .size(14)
        .padding(0)
        .style(move |theme, status| {
            let mut v = estilo::campo(p)(theme, status);
            v.border.width = 0.;
            v
        });
    column![
        c::etiqueta(p, "Archivo del modpack"),
        row![
            container(
                row![icono(Icono::Carpeta, Tam::Base, p.text_muted), entrada]
                    .spacing(10)
                    .align_y(Alignment::Center)
            )
            .padding([8, 12])
            .width(Length::Fill)
            .style(move |_| container::Style {
                background: Some(p.surface_sunken.into()),
                border: iced::Border {
                    color: p.border_strong,
                    width: 2.,
                    ..Default::default()
                },
                ..Default::default()
            }),
            c::boton(
                p,
                "Examinar",
                Variante::Secundario,
                disponible.then(|| m(Accion::Examinar))
            )
        ]
        .spacing(8)
        .align_y(Alignment::Center),
        text("Pega una ruta y pulsa Enter, o elige el archivo con Examinar.")
            .size(12)
            .color(p.text_muted)
    ]
    .spacing(8)
    .into()
}

pub(super) fn vista(app: &App) -> Element<'_, Mensaje> {
    let s = &app.servicios.modpacks;
    let p = app.paleta();
    let ancho = (app.ventana.width - 48.).clamp(280., 760.);
    let compacto = ancho < 640.;
    let margen = if compacto { 20. } else { 32. };
    let posterior = s.archivo.is_some() && (!s.busy || s.lectura_visible);
    let listo = s.plan.is_some() && s.error.is_none();
    let disponible = !s.busy && !s.creando && !s.eligiendo;
    let subtitulo = if s.busy {
        "Leyendo el archivo. Puedes cancelar cuando quieras."
    } else if s.error.is_some() {
        "No se puede importar este archivo."
    } else if listo {
        "Revisa lo que trae antes de instalar."
    } else {
        "Elige un archivo .mrpack o .zip para crear una instancia."
    };
    let cerrar = button(icono(Icono::Cerrar, Tam::Base, p.text))
        .padding(4)
        .on_press_maybe((!s.creando).then(|| m(Accion::Cerrar)))
        .style(estilo::boton_fantasma(p));
    let cabeza = container(
        row![
            column![
                text("Importar modpack")
                    .font(fuentes::TITULO)
                    .size(24)
                    .color(p.text),
                text(subtitulo).size(13).color(p.text_muted)
            ]
            .spacing(6)
            .width(Length::Fill),
            cerrar
        ]
        .spacing(12),
    )
    .padding(iced::Padding {
        top: 28.,
        right: margen,
        bottom: 0.,
        left: margen,
    });
    let mut cuerpo = column![].spacing(28);
    if !posterior {
        cuerpo = cuerpo
            .push(icaro_ui::zona_soltar::zona_soltar_estado(
                p,
                s.arrastrando,
                s.sobre.clone(),
                (s.sobre.is_some() && !s.arrastrando)
                    .then(|| "Ícaro no reconoce este archivo. Usa .mrpack o .zip.".into()),
                disponible.then(|| m(Accion::Examinar)),
            ))
            .push(ruta(app));
        if let Some(error) = &s.error {
            cuerpo = cuerpo.push(c::aviso_detallado(
                p,
                c::Estado::Error,
                "Archivo no admitido",
                error,
            ));
        }
    } else if s.busy {
        cuerpo = cuerpo.push(archivo(app));
        let titulos = [
            "Abrir el archivo",
            "Reconocer el formato",
            "Leer la lista de mods",
            "Verificar archivos y orígenes",
            "Preparar la copia verificada",
        ];
        let mut pasos = column![regla(p, true)];
        for (i, titulo) in titulos.iter().enumerate() {
            let hecho = i < s.paso;
            let actual = i == s.paso;
            let marca: Element<'_, Mensaje> = if hecho {
                icono(Icono::Check, Tam::Sm, p.bg).into()
            } else if actual {
                icaro_ui::progreso::spinner(p, app.reducir)
            } else {
                mono(p, (i + 1).to_string())
            };
            let marca =
                container(marca)
                    .center_x(24)
                    .center_y(24)
                    .style(move |_| container::Style {
                        background: hecho.then(|| p.text.into()),
                        border: iced::Border {
                            color: if hecho || actual {
                                p.text
                            } else {
                                p.border_strong
                            },
                            width: 1.,
                            ..Default::default()
                        },
                        ..Default::default()
                    });
            let contenido = row![
                marca,
                text(*titulo)
                    .size(13)
                    .font(if actual {
                        fuentes::CUERPO_NEGRITA
                    } else {
                        fuentes::CUERPO
                    })
                    .color(if actual || hecho {
                        p.text
                    } else {
                        p.text_muted
                    })
                    .width(Length::Fill),
                mono(p, s.detalles[i].clone())
            ]
            .spacing(16)
            .align_y(Alignment::Center);
            pasos = pasos
                .push(container(contenido).padding([12, 0]).height(48))
                .push(regla(p, false));
        }
        cuerpo = cuerpo.push(pasos).push(
            column![
                row![
                    text(titulos[s.paso.min(4)])
                        .size(13)
                        .color(p.text)
                        .width(Length::Fill),
                    mono(p, format!("Paso {} de 5", s.paso + 1))
                ],
                icaro_ui::progreso::continuo(p, s.paso as f32 / 5., app.reducir)
            ]
            .spacing(10),
        );
    } else if listo {
        let plan = s.plan.as_ref().unwrap();
        let (descarga, copia, carpetas, mods) = plan.resumen();
        let resumen = column![
            text(format!("{} {}", plan.nombre, plan.version))
                .font(fuentes::CUERPO_NEGRITA)
                .size(16)
                .color(p.text),
            mono(
                p,
                format!(
                    "{} · {} {} · {} mods · {}",
                    plan.minecraft,
                    crate::ficha::loader(&plan.loader),
                    plan.loader_version.as_deref().unwrap_or(""),
                    mods,
                    plan.formato()
                )
            )
        ]
        .spacing(4)
        .width(Length::Fill);
        cuerpo = cuerpo.push(
            container(
                row![
                    crate::icono_instancia::procedural(p, &plan.nombre, 0, 48.),
                    resumen,
                    c::boton_compacto(
                        p,
                        "Cambiar archivo",
                        Variante::Fantasma,
                        disponible.then(|| m(Accion::Examinar))
                    )
                ]
                .spacing(20)
                .align_y(Alignment::Center),
            )
            .padding([20, 24])
            .style(move |tema| {
                let mut v = caja(p)(tema);
                v.background = Some(p.surface.into());
                v
            }),
        );
        let nombre = column![
            c::etiqueta(p, "Nombre"),
            text_input("Nombre de la instancia", &s.nombre)
                .id("importar-nombre")
                .on_input_maybe((!s.creando).then_some(|v| m(Accion::Nombre(v))))
                .padding([8, 12])
                .size(14)
                .style(estilo::campo(p))
        ]
        .spacing(8)
        .width(Length::Fill);
        let mut grupos = vec!["Sin grupo".to_owned()];
        grupos.extend(app.lista_grupos.iter().cloned());
        let grupo = column![
            c::etiqueta(p, "Grupo"),
            stack![
                pick_list(
                    grupos,
                    Some(if s.grupo.is_empty() {
                        "Sin grupo".into()
                    } else {
                        s.grupo.clone()
                    }),
                    |v| m(Accion::Grupo(if v == "Sin grupo" {
                        String::new()
                    } else {
                        v
                    }))
                )
                .width(Length::Fill)
                .font(fuentes::CUERPO)
                .text_size(14)
                .padding(iced::Padding {
                    top: 8.,
                    right: 40.,
                    bottom: 8.,
                    left: 12.
                })
                .handle(iced::widget::pick_list::Handle::None)
                .style(estilo::selector(p))
                .menu_style(estilo::menu_selector(p)),
                container(icono(Icono::ChevronAbajo, Tam::Base, p.text))
                    .padding([0, 12])
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_x(iced::alignment::Horizontal::Right)
                    .align_y(iced::alignment::Vertical::Center)
            ]
        ]
        .spacing(8);
        let campos: Element<'_, Mensaje> = if compacto {
            column![nombre, grupo].spacing(20).into()
        } else {
            row![nombre, container(grupo).width(220)].spacing(20).into()
        };
        cuerpo = cuerpo.push(campos).push(
            column![
                c::etiqueta(p, "Al instalar"),
                regla(p, true),
                dato(
                    p,
                    Icono::Descargas,
                    if plan.archivos_por_descargar() == 1 {
                        "Descargar 1 archivo referenciado".into()
                    } else {
                        format!(
                            "Descargar {} archivos referenciados",
                            plan.archivos_por_descargar()
                        )
                    },
                    bytes(descarga)
                ),
                dato(
                    p,
                    Icono::Archivo,
                    "Copiar desde el archivo".into(),
                    format!("{} · {}", carpetas.join(", "), bytes(copia))
                ),
                dato(
                    p,
                    Icono::Cpu,
                    "Java compatible con la instancia".into(),
                    "Se comprobará al instalar".into()
                )
            ]
            .spacing(0),
        );
        if plan.archivos_opcionales() > 0 {
            cuerpo = cuerpo.push(c::casilla(
                p,
                &format!("Incluir {} archivos opcionales", plan.archivos_opcionales()),
                plan.opcionales,
                |v| m(Accion::Opcionales(v)),
            ));
        }
    } else {
        cuerpo = cuerpo.push(archivo(app));
        if let Some(error) = &s.error {
            cuerpo = cuerpo.push(c::aviso_detallado(
                p,
                c::Estado::Error,
                "No se pudo revisar el archivo",
                error,
            ));
        }
        // Los ZIP completos sin manifiesto siguen admitiendo el perfil manual.
        if s.error.as_ref().is_some_and(|e| {
            e.contains("Escribe su versión") || e.contains("Indica la versión del loader")
        }) {
            cuerpo = cuerpo
                .push(
                    text("Indica el perfil que utiliza el ZIP y vuelve a revisarlo.")
                        .size(13)
                        .color(p.text_muted),
                )
                .push(
                    column![
                        c::etiqueta(p, "Minecraft"),
                        c::campo(p, "1.21.1", &s.minecraft, |v| m(Accion::Minecraft(v)))
                    ]
                    .spacing(8),
                )
                .push(
                    column![
                        c::etiqueta(p, "Loader"),
                        c::selector(
                            p,
                            vec![
                                "vanilla".into(),
                                "fabric".into(),
                                "forge".into(),
                                "neoforge".into()
                            ],
                            Some(s.loader.clone()),
                            |v| m(Accion::Loader(v))
                        )
                    ]
                    .spacing(8),
                );
            if s.loader != "vanilla" {
                cuerpo = cuerpo.push(
                    column![
                        c::etiqueta(p, "Versión del loader"),
                        c::campo(p, "Versión exacta", &s.loader_version, |v| m(
                            Accion::LoaderVersion(v)
                        ))
                    ]
                    .spacing(8),
                );
            }
            cuerpo = cuerpo.push(c::boton(
                p,
                "Volver a revisar",
                Variante::Secundario,
                disponible.then(|| m(Accion::RevisarLocal)),
            ));
        }
    }
    let informacion = if listo {
        format!("instancias\\{}", s.nombre.trim())
    } else if s.busy {
        "Cancelar no deja nada a medias".into()
    } else {
        String::new()
    };
    let mut botones = row![c::boton(
        p,
        "Cancelar",
        Variante::Secundario,
        (!s.creando).then(|| m(Accion::Cerrar))
    )]
    .spacing(12);
    if posterior && s.error.is_some() {
        botones = botones.push(c::boton_con_icono(
            p,
            Icono::Carpeta,
            "Elegir otro archivo",
            Variante::Secundario,
            disponible.then(|| m(Accion::Examinar)),
            false,
        ));
    } else if posterior {
        botones = botones.push(c::boton_con_icono(
            p,
            Icono::Descargas,
            if s.creando {
                "Creando…"
            } else {
                "Instalar en nueva instancia"
            },
            Variante::Primario,
            (disponible && listo && !s.nombre.trim().is_empty()).then(|| m(Accion::Crear)),
            false,
        ));
    }
    let pie: Element<'_, Mensaje> = if compacto {
        column![mono(p, informacion), botones].spacing(12).into()
    } else {
        row![container(mono(p, informacion)).width(Length::Fill), botones]
            .spacing(12)
            .align_y(Alignment::Center)
            .into()
    };
    let pie = column![
        Space::new(0, 8),
        regla(p, false),
        container(pie).padding([20., margen])
    ];
    let mut ventana = column![];
    if posterior {
        let credito = container(
            text("Todo cruza el puente antes de entrar")
                .font(fuentes::MONO)
                .size(12)
                .color(p.text),
        )
        .padding([4, 8])
        .style(move |_| container::Style {
            background: Some(p.bg.into()),
            border: iced::Border {
                color: p.border_strong,
                width: 1.,
                ..Default::default()
            },
            ..Default::default()
        });
        ventana = ventana
            .push(stack![
                laminas::grabado_posicion(
                    p,
                    Lamina::CastilloPasarela,
                    Length::Fill,
                    128.,
                    (0.5, 0.4)
                ),
                container(credito)
                    .padding(iced::Padding {
                        top: 0.,
                        right: 16.,
                        bottom: 12.,
                        left: 16.
                    })
                    .width(Length::Fill)
                    .height(128)
                    .align_y(iced::alignment::Vertical::Bottom)
            ])
            .push(regla(p, true));
    }
    let maximo = (app.ventana.height - 48.).max(280.);
    let alto_pie = if compacto { 112. } else { 80. };
    let alto_cuerpo =
        (maximo - 4. - if posterior { 128. + 2. } else { 0. } - 86. - alto_pie).max(100.);
    ventana = ventana
        .push(cabeza)
        .push(
            container(
                scrollable(container(cuerpo).padding(iced::Padding {
                    top: 28.,
                    right: margen,
                    bottom: 24.,
                    left: margen,
                }))
                .height(Length::Shrink),
            )
            .max_height(alto_cuerpo),
        )
        .push(pie);
    // Limitar solo el cuerpo mantiene Cancelar y la acción principal accesibles.
    // El contenedor de la ventana distribuye el espacio restante al scroll.

    container(ventana)
        .padding(2)
        .width(ancho)
        .max_height(maximo)
        .style(estilo::flotante(p))
        .into()
}

fn archivo(app: &App) -> Element<'_, Mensaje> {
    let p = app.paleta();
    let s = &app.servicios.modpacks;
    let nombre = s
        .archivo
        .as_ref()
        .and_then(|p| p.file_name())
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    container(
        row![
            icono(Icono::Paquete, Tam::Base, p.text),
            text(nombre)
                .font(fuentes::CUERPO_NEGRITA)
                .size(16)
                .color(p.text)
                .width(Length::Fill),
            mono(p, s.peso_archivo.map(bytes).unwrap_or_default())
        ]
        .spacing(14)
        .align_y(Alignment::Center),
    )
    .padding([16, 20])
    .style(caja(p))
    .into()
}
