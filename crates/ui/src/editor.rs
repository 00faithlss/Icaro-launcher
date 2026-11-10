// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Editor de instancia: encabezado con datos clave, navegación entre pestañas
//! y controles de General, Java, Recursos, Mundos y Archivos.

use iced::widget::{column, container, row, text, Space};
use iced::{Alignment, Element, Length, Padding};

use crate::componentes::{
    self, boton, boton_jugar, campo, casilla, etiqueta, insignia, interruptor, migas, pestanas,
    segmentado, selector, senal, Estado, Variante,
};
use crate::estilo;
use crate::fuentes;
use crate::iconos::{icono, Icono, Tam};
use crate::laminas::{banda_con, Lamina};
use crate::tema::{borde, espacio, texto, Paleta};

/// Datos del encabezado del editor.
#[derive(Debug, Clone)]
pub struct DatosEditor {
    pub nombre: String,
    pub version: String,
    pub jugado: String,
    pub mods: String,
    pub tamano: String,
    pub memoria: String,
    pub portada: Lamina,
}

/// Mensajes del encabezado.
#[derive(Debug, Clone)]
pub struct MensajesEncabezado<M> {
    pub volver: M,
    pub abrir_carpeta: M,
    pub mas: M,
    pub jugar: M,
}

/// Pestañas del editor, en su orden. Mods lleva su conteo.
pub fn pestanas_editor<'a, M: Clone + 'a>(
    p: Paleta,
    activa: usize,
    mods: usize,
    mundos: usize,
    al_elegir: impl Fn(usize) -> M,
) -> Element<'a, M> {
    pestanas_editor_con_mods(p, activa, mods, mundos, true, al_elegir)
}

/// Vanilla conserva el orden de navegación, sin la pestaña de mods.
pub fn pestanas_editor_con_mods<'a, M: Clone + 'a>(
    p: Paleta,
    activa: usize,
    mods: usize,
    mundos: usize,
    mostrar_mods: bool,
    al_elegir: impl Fn(usize) -> M,
) -> Element<'a, M> {
    let mut opciones = vec![("General", None), ("Java", None)];
    let mut indices = vec![0, 1];
    if mostrar_mods { opciones.push(("Mods", Some(mods))); indices.push(2); }
    opciones.extend([("Recursos", None), ("Shaders", None), ("Mundos", Some(mundos)),
        ("Capturas", None), ("Registros", None), ("Archivos", None)]);
    indices.extend([3, 8, 4, 5, 6, 7]);
    let elegida = indices.iter().position(|i| *i == activa).unwrap_or(0);
    pestanas(p, &opciones, elegida, move |i| al_elegir(indices[i]))
}

fn dato<'a, M: 'a>(p: Paleta, nombre: &str, valor: &str) -> Element<'a, M> {
    column![
        etiqueta(p, nombre),
        text(valor.to_owned())
            .font(fuentes::MONO)
            .size(texto::MONO.0)
            .color(p.text),
    ]
    .spacing(espacio::S1)
    .into()
}

/// Encabezado: banda con la portada, migas, datos clave y acciones.
pub fn encabezado_editor<'a, M: Clone + 'a>(
    p: Paleta,
    d: &DatosEditor,
    m: MensajesEncabezado<M>,
) -> Element<'a, M> {
    encabezado_editor_con_mods(p, d, m, true)
}
pub fn encabezado_editor_con_mods<'a, M: Clone + 'a>(
    p: Paleta,
    d: &DatosEditor,
    m: MensajesEncabezado<M>,
    mostrar_mods: bool,
) -> Element<'a, M> {
    let jugar = boton_jugar(p, "Jugar", Some(&d.memoria), m.jugar.clone());
    encabezado_editor_accion(p, d, m, mostrar_mods, jugar)
}

pub fn encabezado_editor_accion<'a, M: Clone + 'a>(
    p: Paleta,
    d: &DatosEditor,
    m: MensajesEncabezado<M>,
    mostrar_mods: bool,
    jugar: Element<'a, M>,
) -> Element<'a, M> {
    encabezado_editor_icono(p,d,m,mostrar_mods,jugar,None)
}

pub fn encabezado_editor_icono<'a,M:Clone+'a>(p:Paleta,d:&DatosEditor,m:MensajesEncabezado<M>,mostrar_mods:bool,jugar:Element<'a,M>,imagen:Option<Element<'a,M>>)->Element<'a,M>{
    let acciones = row![
        crate::descargas::boton_icono(p, Icono::Carpeta, m.abrir_carpeta),
        crate::descargas::boton_icono(p, Icono::Acciones, m.mas),
        jugar,
    ]
    .spacing(espacio::S2)
    .align_y(Alignment::Center);
    let mut datos =
        row![dato(p, "Versión", &d.version), dato(p, "Jugado", &d.jugado),].spacing(espacio::S8);
    if mostrar_mods {
        datos = datos.push(dato(p, "Mods", &d.mods));
    }
    datos = datos.push(dato(p, "Tamaño", &d.tamano));
    column![
        banda_con(p, d.portada, &d.nombre, None, 150.0, texto::DISPLAY),
        container(
            column![
                migas(p, &["Instancias", &d.nombre], move |_| m.volver.clone()),
                row![
                    imagen.unwrap_or_else(||container(icono(Icono::Instancias,Tam::Lg,p.text)).padding(espacio::S3).style(estilo::marco(p)).into()),
                    datos,
                    Space::with_width(Length::Fill),
                    acciones,
                ]
                .spacing(espacio::S6)
                .align_y(Alignment::Center),
            ]
            .spacing(espacio::S4)
        )
        .padding(Padding::from([espacio::S4, espacio::S12])),
    ]
    .into()
}

fn pagina<'a, M: 'a>(contenido: Element<'a, M>) -> Element<'a, M> {
    crate::scroll::desplazable(
        container(contenido).padding(Padding::from([espacio::S6, espacio::S12])),
    )
}

fn campo_rotulado<'a, M: 'a>(p: Paleta, nombre: &str, control: Element<'a, M>) -> Element<'a, M> {
    column![etiqueta(p, nombre), control]
        .spacing(espacio::S2)
        .width(Length::Fill)
        .into()
}

/// Estado de la pestaña General.
#[derive(Debug, Clone)]
pub struct EstadoGeneral {
    pub nombre: String,
    pub grupo: Option<String>,
    /// Grupos disponibles, con "Sin grupo" al final.
    pub grupos: Vec<String>,
    pub ancho: String,
    pub alto: String,
    pub pantalla_completa: bool,
    pub servidor: Option<&'static str>,
}

/// Mensajes de la pestaña General.
pub struct MensajesGeneral<M> {
    pub nombre: fn(String) -> M,
    pub grupo: fn(String) -> M,
    pub ancho: fn(String) -> M,
    pub alto: fn(String) -> M,
    pub pantalla_completa: fn(bool) -> M,
    pub servidor: fn(&'static str) -> M,
}

/// Pestaña General: nombre, grupo, ventana y servidor.
pub fn pestana_general<'a, M: Clone + 'a>(
    p: Paleta,
    e: &EstadoGeneral,
    m: &MensajesGeneral<M>,
) -> Element<'a, M> {
    let ventana = row![
        container(campo(p, "Ancho", &e.ancho, m.ancho)).width(100),
        text("×").color(p.text_muted),
        container(campo(p, "Alto", &e.alto, m.alto)).width(100),
        casilla(
            p,
            "Pantalla completa",
            e.pantalla_completa,
            m.pantalla_completa
        ),
    ]
    .spacing(espacio::S3)
    .align_y(Alignment::Center);
    let rejilla = column![
        row![
            campo_rotulado(p, "Nombre", campo(p, "Nombre", &e.nombre, m.nombre)),
            campo_rotulado(
                p,
                "Grupo",
                selector(
                    p,
                    e.grupos.clone(),
                    Some(e.grupo.clone().unwrap_or_else(|| "Sin grupo".to_owned())),
                    m.grupo
                )
            ),
        ]
        .spacing(espacio::S8),
        row![
            campo_rotulado(p, "Ventana del juego", ventana.into()),
            campo_rotulado(
                p,
                "Al jugar, unirse a",
                selector(
                    p,
                    vec!["Amigos · 10.147.17.1", "Ninguno"],
                    e.servidor,
                    m.servidor
                )
            ),
        ]
        .spacing(espacio::S8),
    ]
    .spacing(espacio::S6);
    pagina(rejilla.into())
}

/// Presentación de rutas Windows; el ejecutable conserva su ruta original.
pub fn ruta_java_visible(ruta: &str) -> String {
    if let Some(unc) = ruta.strip_prefix(r"\\?\UNC\") {
        return format!(r"\\{unc}");
    }
    if let Some(local) = ruta.strip_prefix(r"\\?\") {
        let bytes = local.as_bytes();
        if bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'\\' {
            return local.to_owned();
        }
    }
    ruta.to_owned()
}

/// Instalación de Java detectada.
#[derive(Debug, Clone)]
pub struct InstalacionJava {
    pub version: String,
    pub proveedor: String,
    pub gestionada: bool,
    pub ruta: String,
    pub usado_por: String,
    pub existe: bool,
}

/// Tabla de instalaciones de Java detectadas y sus acciones.
fn tabla_instalaciones<'a, M: Clone + 'a>(
    p: Paleta,
    instalaciones: &[InstalacionJava],
    m: &MensajesJava<M>,
) -> Element<'a, M> {
    let mut tabla = column![container(row![
        columna_titulo(p, "Java", 1),
        columna_titulo(p, "Proveedor", 2),
        columna_titulo(p, "Ruta", 3),
        columna_titulo(p, "Usado por", 2),
        Space::with_width(90),
    ])
    .padding(Padding::from([espacio::S2, 0.0]))];
    for i in instalaciones {
        let tono = if i.existe { p.text } else { p.error };
        let tenue = if i.existe { p.text_muted } else { p.error };
        let mut proveedor = row![text(i.proveedor.clone()).size(texto::BODY.0).color(tono)]
            .spacing(espacio::S2)
            .align_y(Alignment::Center);
        if i.gestionada {
            proveedor = proveedor.push(insignia(p, Estado::Info, "Icaro Launcher"));
        }
        let accion = if i.existe {
            boton(
                p,
                "Probar",
                Variante::Secundario,
                Some((m.probar)(i.ruta.clone())),
            )
        } else {
            boton(p, "Quitar", Variante::Peligro, None::<M>)
        };
        tabla = tabla
            .push(crate::descargas::regla(borde::FINO, p.border))
            .push(
                container(
                    row![
                        text(i.version.clone())
                            .font(fuentes::MONO)
                            .size(texto::MONO.0)
                            .color(tono)
                            .width(Length::FillPortion(1)),
                        container(proveedor).width(Length::FillPortion(2)),
                        text(ruta_java_visible(&i.ruta))
                            .font(fuentes::MONO)
                            .size(texto::MONO_SM.0)
                            .color(tenue)
                            .width(Length::FillPortion(3)),
                        text(i.usado_por.clone())
                            .size(texto::BODY_SM.0)
                            .color(tenue)
                            .width(Length::FillPortion(2)),
                        container(accion).width(90),
                    ]
                    .align_y(Alignment::Center),
                )
                .padding(Padding::from([espacio::S2, 0.0])),
            );
    }
    column![
        tabla,
        row![
            boton(p, "Descargar Java", Variante::Secundario, None::<M>),
            boton(
                p,
                "Detectar instalaciones",
                Variante::Secundario,
                Some(m.detectar.clone())
            ),
            boton(
                p,
                "Agregar ruta",
                Variante::Fantasma,
                Some(m.agregar.clone())
            ),
        ]
        .spacing(espacio::S2),
    ]
    .spacing(espacio::S4)
    .into()
}

fn columna_titulo<'a, M: 'a>(p: Paleta, nombre: &str, partes: u16) -> Element<'a, M> {
    container(etiqueta(p, nombre))
        .width(Length::FillPortion(partes))
        .into()
}

/// Paquete de recursos o shader.
#[derive(Debug, Clone)]
pub struct Paquete {
    pub nombre: String,
    pub autor: String,
    pub resolucion: String,
    pub activo: bool,
    pub compat: CompatPaquete,
    /// Fijo al pie, sin asa (paquetes del juego).
    pub fijo: bool,
}

/// Compatibilidad de un paquete con la versión de la instancia.
#[derive(Debug, Clone, PartialEq)]
pub enum CompatPaquete {
    Compatible,
    /// Hecho para otra versión; el juego lo carga pero puede verse mal.
    HechoPara(String),
    Incompatible,
}

/// Pestaña Recursos: paquetes ordenados por prioridad.
pub fn pestana_recursos<'a, M: Clone + 'a>(
    p: Paleta,
    vista: usize,
    al_elegir_vista: impl Fn(usize) -> M + 'a,
    paquetes: &[Paquete],
    alternar: impl Fn(usize) -> M,
) -> Element<'a, M> {
    let mut lista = column![crate::descargas::regla(borde::FINO, p.border)];
    for (n, q) in paquetes.iter().enumerate() {
        let (estado, palabra) = match &q.compat {
            CompatPaquete::Compatible => (Estado::Exito, "Compatible".to_owned()),
            CompatPaquete::HechoPara(v) => (Estado::Aviso, format!("Hecho para {v}")),
            CompatPaquete::Incompatible => (Estado::Error, "Incompatible".to_owned()),
        };
        let asa: Element<M> = if q.fijo {
            Space::with_width(Tam::Base.px()).into()
        } else {
            icono(Icono::Arrastrar, Tam::Base, p.text_muted).into()
        };
        lista = lista
            .push(
                container(
                    row![
                        asa,
                        interruptor(p, q.activo, alternar(n)),
                        column![
                            text(q.nombre.clone())
                                .font(fuentes::TITULO)
                                .size(texto::BODY.0)
                                .color(p.text),
                            text(q.autor.clone())
                                .size(texto::BODY_SM.0)
                                .color(p.text_muted),
                        ]
                        .width(Length::Fill),
                        text(q.resolucion.clone())
                            .font(fuentes::MONO)
                            .size(texto::MONO_SM.0)
                            .color(p.text_muted),
                        insignia(p, estado, &palabra),
                    ]
                    .spacing(espacio::S4)
                    .align_y(Alignment::Center),
                )
                .padding(Padding::from([espacio::S3, 0.0])),
            )
            .push(crate::descargas::regla(borde::FINO, p.border));
    }
    pagina(
        column![
            row![
                segmentado(p, &["Recursos", "Shaders"], vista, al_elegir_vista),
                Space::with_width(Length::Fill),
                text("Arriba tiene prioridad")
                    .size(texto::BODY_SM.0)
                    .color(p.text_muted),
            ]
            .align_y(Alignment::Center),
            lista,
        ]
        .spacing(espacio::S4)
        .into(),
    )
}

/// Mundo guardado.
#[derive(Debug, Clone)]
pub struct Mundo {
    pub nombre: String,
    pub modo: String,
    pub hardcore: bool,
    pub ultima_vez: String,
    pub tamano: String,
    pub copias: usize,
}

/// Copia de seguridad de un mundo.
#[derive(Debug, Clone)]
pub struct Copia {
    pub fecha: String,
    pub tamano: String,
    pub motivo: String,
}

/// Pestaña Mundos: lista a la izquierda y copias del mundo elegido.
pub fn pestana_mundos<'a, M: Clone + 'a>(
    p: Paleta,
    mundos: &[Mundo],
    elegido: usize,
    al_elegir: impl Fn(usize) -> M + 'a,
    copias: &[Copia],
) -> Element<'a, M> {
    let mut lista = column![].spacing(espacio::S3).width(Length::FillPortion(3));
    for (i, w) in mundos.iter().enumerate() {
        let modo = if w.hardcore {
            text(w.modo.to_uppercase())
                .font(fuentes::ETIQUETA)
                .size(texto::LABEL.0)
                .color(p.error)
        } else {
            text(w.modo.to_uppercase())
                .font(fuentes::ETIQUETA)
                .size(texto::LABEL.0)
                .color(p.text_muted)
        };
        let contenido = row![
            icono(Icono::Instancias, Tam::Md, p.text),
            column![
                text(w.nombre.clone())
                    .font(fuentes::TITULO)
                    .size(texto::HEADING.0)
                    .color(p.text),
                row![
                    modo,
                    text(format!("{} · {}", w.ultima_vez, w.tamano))
                        .size(texto::BODY_SM.0)
                        .color(p.text_muted),
                ]
                .spacing(espacio::S3),
            ]
            .width(Length::Fill)
            .spacing(espacio::S1),
            text(if w.copias == 0 {
                "Sin copias".to_owned()
            } else {
                format!("{} copias", w.copias)
            })
            .font(fuentes::MONO)
            .size(texto::MONO_SM.0)
            .color(p.text_muted),
        ]
        .spacing(espacio::S4)
        .align_y(Alignment::Center);
        lista = lista.push(
            iced::widget::button(
                container(contenido)
                    .padding(espacio::S4)
                    .width(Length::Fill)
                    .style(estilo::tarjeta(p, i == elegido)),
            )
            .padding(0)
            .width(Length::Fill)
            .on_press(al_elegir(i))
            .style(estilo::sin_estilo(p.text)),
        );
    }
    let mut panel = column![text("COPIAS DE SEGURIDAD")
        .font(fuentes::TITULO)
        .size(texto::HEADING.0)
        .color(p.text),]
    .spacing(espacio::S3);
    for c in copias {
        panel = panel
            .push(crate::descargas::regla(borde::FINO, p.border))
            .push(
                row![
                    column![
                        text(c.fecha.clone())
                            .font(fuentes::MONO)
                            .size(texto::MONO_SM.0)
                            .color(p.text),
                        text(c.motivo.clone())
                            .size(texto::BODY_SM.0)
                            .color(p.text_muted),
                    ]
                    .width(Length::Fill),
                    text(c.tamano.clone())
                        .font(fuentes::MONO)
                        .size(texto::MONO_SM.0)
                        .color(p.text_muted),
                    boton(p, "Restaurar", Variante::Secundario, None::<M>),
                ]
                .spacing(espacio::S3)
                .align_y(Alignment::Center),
            );
    }
    pagina(
        row![
            lista,
            container(panel)
                .padding(espacio::S6)
                .width(Length::FillPortion(2))
                .style(estilo::tarjeta(p, false)),
        ]
        .spacing(espacio::S6)
        .into(),
    )
}

/// Señal de impacto de un shader: cuatro barras invertidas (más barras,
/// menos impacto).
pub fn impacto<'a, M: 'a>(p: Paleta, nivel: usize) -> Element<'a, M> {
    senal(p, 4usize.saturating_sub(nivel))
}

/// Configuración de Java de una instancia. Cada instancia tiene la suya.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigJava {
    /// Selección propia; `None` usa Automático cuando no se heredan ajustes globales.
    pub java: Option<String>,
    pub heredar: bool,
    pub heredar_memoria: bool,
    pub java_requiere_seleccion: bool,
    pub descarga: bool,
    /// Memoria inicial de la JVM (`-Xms`), en MB.
    pub memoria_min: u32,
    /// Memoria máxima de la JVM (`-Xmx`), en MB.
    pub memoria_max: u32,
    /// Memoria para clases y metadatos (`-XX:MaxMetaspaceSize`), en MB.
    pub metaspace: u32,
    /// Argumentos adicionales de la JVM, separados por espacios.
    pub argumentos: String,
}

impl ConfigJava {
    /// Argumentos finales que se pasarían a la JVM.
    pub fn linea_de_comando(&self) -> String {
        let mut partes = vec![
            format!("-Xms{}M", self.memoria_min),
            format!("-Xmx{}M", self.memoria_max),
            format!("-XX:MaxMetaspaceSize={}M", self.metaspace),
        ];
        if !self.argumentos.trim().is_empty() {
            partes.push(self.argumentos.trim().to_owned());
        }
        partes.join(" ")
    }
}

/// Mensajes de la pestaña Java.
pub struct MensajesJava<M> {
    pub java: fn(Option<String>) -> M,
    pub heredar: fn(bool) -> M,
    pub heredar_memoria: fn(bool) -> M,
    pub descarga: fn(bool) -> M,
    pub detectar: M,
    pub agregar: M,
    pub probar: fn(String) -> M,
    pub memoria_min: fn(u32) -> M,
    pub memoria_max: fn(u32) -> M,
    pub metaspace: fn(u32) -> M,
    pub argumentos: fn(String) -> M,
}

/// Texto de la opción de Java automático en el selector.
pub const JAVA_AUTOMATICO: &str = "Automático (recomendado)";

/// Pestaña Java de una instancia: versión de Java, memoria mínima, máxima y
/// de metadatos, argumentos de la JVM y las instalaciones detectadas.
pub fn pestana_java<'a, M: Clone + 'a>(
    p: Paleta,
    cfg: &ConfigJava,
    m: &MensajesJava<M>,
    ram_total: u32,
    instalaciones: &[InstalacionJava],
    estado_java: &str,
) -> Element<'a, M> {
    let mut opciones = vec![JAVA_AUTOMATICO.to_owned()];
    opciones.extend(
        instalaciones
            .iter()
            .filter(|i| i.existe)
            .map(|i| format!("Java {} · {} · {}", i.version, i.proveedor, ruta_java_visible(&i.ruta))),
    );
    let elegida = cfg
        .java
        .as_ref()
        .map(|path| {
            instalaciones
                .iter()
                .find(|i| &i.ruta == path)
                .map(|i| format!("Java {} · {} · {}", i.version, i.proveedor, ruta_java_visible(&i.ruta)))
                .unwrap_or_else(|| ruta_java_visible(path))
        })
        .unwrap_or_else(|| JAVA_AUTOMATICO.to_owned());
    let rutas: Vec<_> = instalaciones
        .iter()
        .filter(|i| i.existe)
        .map(|i| {
            (
                format!("Java {} · {} · {}", i.version, i.proveedor, ruta_java_visible(&i.ruta)),
                i.ruta.clone(),
            )
        })
        .collect();
    let al_elegir = m.java;
    let ruta_manual = m.java;
    let version = selector(p, opciones, Some(elegida), move |o| {
        al_elegir(if o == JAVA_AUTOMATICO {
            None
        } else {
            rutas
                .iter()
                .find(|(label, _)| label == &o)
                .map(|(_, path)| path.clone())
        })
    });

    let tope = ram_total.saturating_sub(1024).max(1024);
    let memoria = column![
        campo_rotulado(
            p,
            "Memoria mínima",
            componentes::deslizador_valor(p, cfg.memoria_min, 256, tope, 256, m.memoria_min),
        ),
        campo_rotulado(
            p,
            "Memoria máxima",
            componentes::deslizador_valor(p, cfg.memoria_max, 512, tope, 256, m.memoria_max),
        ),
        campo_rotulado(
            p,
            "Memoria de metadatos (Metaspace)",
            componentes::deslizador_valor(p, cfg.metaspace, 128, 2048, 128, m.metaspace),
        ),
    ]
    .spacing(espacio::S4)
    .width(Length::Fill);

    pagina(
        column![
            campo_rotulado(p, "Heredar memoria y argumentos globales", componentes::interruptor(p, cfg.heredar_memoria, (m.heredar_memoria)(!cfg.heredar_memoria))),
            campo_rotulado(p, "Heredar Java de los ajustes globales", componentes::interruptor(p, cfg.heredar, (m.heredar)(!cfg.heredar))),
            campo_rotulado(p, "Versión de Java de esta instancia", version),
            campo_rotulado(p, "Ruta manual de Java", campo(p, "Automático", &ruta_java_visible(cfg.java.as_deref().unwrap_or("")), move |v: String| ruta_manual((!v.trim().is_empty()).then_some(v)))),
            campo_rotulado(p, "Permitir descarga automática (al no heredar)", componentes::interruptor(p, cfg.descarga, (m.descarga)(!cfg.descarga))),
            text(estado_java.to_owned()).size(texto::BODY_SM.0).color(p.text_muted),
            text("Automático con descargas usa el runtime administrado del perfil. Sin descargas detecta Java compatible disponible. Probar comprueba también la memoria y los argumentos. La memoria es propia de cada instancia.")
                .size(texto::BODY_SM.0)
                .color(p.text_muted),
            memoria,
            campo_rotulado(
                p,
                "Argumentos de la JVM",
                campo(p, "-XX:+UseG1GC -XX:MaxGCPauseMillis=50", &cfg.argumentos, m.argumentos),
            ),
            campo_rotulado(
                p,
                "Línea resultante",
                container(
                    text(cfg.linea_de_comando())
                        .font(fuentes::MONO)
                        .size(texto::MONO_SM.0)
                        .color(p.text),
                )
                .padding(espacio::S3)
                .width(Length::Fill)
                .style(estilo::marco(p))
                .into(),
            ),
            campo_rotulado(p, "Instalaciones detectadas", tabla_instalaciones(p, instalaciones, m)),
        ]
        .spacing(espacio::S6)
        .into(),
    )
}

/// Entrada visible del árbol de archivos.
#[derive(Debug, Clone)]
pub struct Nodo {
    pub nombre: String,
    pub profundidad: usize,
    pub carpeta: bool,
    pub abierta: bool,
    /// Gestionado por Icaro Launcher: se muestra con candado y no se modifica aquí.
    pub bloqueado: bool,
    pub tamano: String,
}

/// Acciones de la pestaña Archivos.
#[derive(Debug, Clone, PartialEq)]
pub enum AccionArchivo {
    Abrir,
    AbrirCarpeta,
    Renombrar,
    ConfirmarNombre,
    CancelarNombre,
    Duplicar,
    Papelera,
    CopiarRuta,
    Guardar,
    Descartar,
}

/// Estado que la pestaña Archivos necesita para dibujarse.
pub struct VistaArchivos<'a> {
    pub ruta: Option<String>,
    pub arbol: &'a [Nodo],
    pub elegido: Option<usize>,
    /// Contenido del archivo de texto abierto; `None` si no hay o no es texto.
    pub contenido: Option<&'a iced::widget::text_editor::Content>,
    pub modificado: bool,
    /// Mensaje junto al editor: error de lectura o de formato.
    pub nota: Option<(Estado, String)>,
    /// Nombre en edición al renombrar.
    pub renombrando: Option<&'a str>,
}

/// Mensajes de la pestaña Archivos.
pub struct MensajesArchivos<M> {
    pub elegir: fn(usize) -> M,
    pub accion: fn(AccionArchivo) -> M,
    pub editar: fn(iced::widget::text_editor::Action) -> M,
    pub nombre: fn(String) -> M,
}

/// Pestaña Archivos: árbol de carpetas que se abren y cierran, acciones sobre
/// el elemento elegido y vista previa de solo lectura.
pub fn pestana_archivos<'a, M: Clone + 'a>(
    p: Paleta,
    v: &VistaArchivos<'a>,
    m: &MensajesArchivos<M>,
) -> Element<'a, M> {
    let nodo = v.elegido.and_then(|i| v.arbol.get(i));
    let hay = nodo.is_some();
    let libre = nodo.is_some_and(|n| !n.bloqueado);
    let barra: Element<'a, M> = row![
        text(v.ruta.clone().unwrap_or("Selecciona un archivo".into()))
            .font(fuentes::MONO)
            .size(texto::MONO_SM.0)
            .color(p.text_muted),
        crate::componentes::boton_con_icono(
            p,
            Icono::Externo,
            "Abrir",
            Variante::Secundario,
            hay.then(|| (m.accion)(AccionArchivo::Abrir)),
            true
        ),
        crate::componentes::boton_con_icono(
            p,
            Icono::Editar,
            "Renombrar",
            Variante::Secundario,
            libre.then(|| (m.accion)(AccionArchivo::Renombrar)),
            true
        ),
        crate::componentes::boton_con_icono(
            p,
            Icono::Copiar,
            "Copiar ruta",
            Variante::Secundario,
            hay.then(|| (m.accion)(AccionArchivo::CopiarRuta)),
            true
        ),
        crate::componentes::boton_con_icono(
            p,
            Icono::Carpeta,
            "",
            Variante::Fantasma,
            hay.then(|| (m.accion)(AccionArchivo::AbrirCarpeta)),
            true
        ),
        Space::with_width(Length::Fill),
        crate::componentes::boton_con_icono(
            p,
            Icono::Papelera,
            "Eliminar",
            Variante::Peligro,
            libre.then(|| (m.accion)(AccionArchivo::Papelera)),
            true
        ),
    ]
    .spacing(espacio::S2)
    .align_y(Alignment::Center)
    .into();

    let mut nodos = column![];
    for (i, n) in v.arbol.iter().enumerate() {
        let actual = v.elegido == Some(i);
        let tinta = if actual { p.bg } else { p.text };
        let expansor: Element<'a, M> = if n.carpeta {
            icono(
                if n.abierta {
                    Icono::ChevronAbajo
                } else {
                    Icono::ChevronDerecha
                },
                Tam::Base,
                tinta,
            )
            .into()
        } else {
            Space::with_width(Tam::Base.px()).into()
        };
        let glifo = if n.carpeta {
            Icono::Carpeta
        } else {
            Icono::Copiar
        };
        let nombre: Element<'a, M> = if actual && v.renombrando.is_some() {
            iced::widget::text_input("Nuevo nombre", v.renombrando.unwrap())
                .id("archivo-nombre")
                .on_input(m.nombre)
                .on_submit((m.accion)(AccionArchivo::ConfirmarNombre))
                .font(fuentes::MONO)
                .size(texto::MONO_SM.0)
                .padding(4)
                .style(estilo::campo(p))
                .into()
        } else {
            text(n.nombre.clone())
                .font(fuentes::MONO)
                .size(texto::MONO_SM.0)
                .color(tinta)
                .width(Length::Fill)
                .into()
        };
        let mut fila = row![
            Space::with_width(n.profundidad as f32 * espacio::S5),
            expansor,
            icono(glifo, Tam::Base, tinta),
            nombre,
        ]
        .spacing(espacio::S3)
        .align_y(Alignment::Center);
        fila = fila.push(
            text(&n.tamano)
                .font(fuentes::MONO)
                .size(texto::MONO_SM.0)
                .color(if actual { p.bg } else { p.text_muted }),
        );
        if n.bloqueado {
            fila = fila.push(icono(Icono::Candado, Tam::Base, tinta));
        }
        if actual && v.renombrando.is_some() {
            nodos = nodos.push(
                container(fila)
                    .height(32)
                    .width(Length::Fill)
                    .padding(Padding::from([0., espacio::S4])),
            );
            continue;
        }
        nodos = nodos.push(
            iced::widget::button(container(fila).center_y(Length::Fill))
                .width(Length::Fill)
                .height(32)
                .padding(Padding::from([0.0, espacio::S4]))
                .on_press((m.elegir)(i))
                .style(move |_, estado| iced::widget::button::Style {
                    background: (actual || matches!(estado, iced::widget::button::Status::Hovered))
                        .then_some(iced::Background::Color(if actual {
                            p.text
                        } else {
                            p.surface_hover
                        })),
                    text_color: tinta,
                    ..Default::default()
                }),
        );
    }

    let preview: Element<'a, M> = match v.contenido {
        Some(c) => crate::scroll::desplazable(
            container(
                text(c.text())
                    .font(fuentes::MONO)
                    .size(texto::MONO_SM.0)
                    .color(p.text),
            )
            .padding(16),
        )
        .into(),
        None => container(
            text(v.nota.as_ref().map_or(
                "Elige un archivo de texto para verlo.".to_owned(),
                |(_, n)| n.clone(),
            ))
            .size(texto::BODY_SM.0)
            .color(p.text_muted),
        )
        .padding(16)
        .into(),
    };
    let derecha = column![
        container(
            text("Vista previa · solo lectura")
                .font(fuentes::MONO)
                .size(texto::MONO_SM.0)
                .color(p.text_muted)
        )
        .padding(12),
        container(Space::new(Length::Fill, 1)).style(estilo::bloque(p.border)),
        preview
    ]
    .height(Length::Fill);

    // El panel ocupa todo el alto de la ventana: el árbol y la vista previa
    // se desplazan por dentro y la página no se mueve.
    container(
        column![
            barra,
            container(
                row![
                    container(crate::scroll::desplazable(nodos))
                        .padding(borde::MEDIO)
                        .width(Length::FillPortion(2))
                        .height(Length::Fill),
                    container(Space::new(1, Length::Fill)).style(estilo::bloque(p.border)),
                    container(derecha).width(Length::FillPortion(3)).height(Length::Fill),
                ]
                .height(Length::Fill)
            )
            .height(Length::Fill)
            .style(estilo::marco(p)),
        ]
        .spacing(espacio::S4)
        .height(Length::Fill),
    )
    .padding(Padding::from([espacio::S6, espacio::S12]))
    .height(Length::Fill)
    .into()
}
