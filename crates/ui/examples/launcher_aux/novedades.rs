// SPDX-License-Identifier: GPL-3.0-only
//! NovedadesVersion con las notas reales de `docs/releases/<versión>.json`,
//! incluidas al compilar, y las fechas del CHANGELOG. Se abre sola la primera
//! vez que se entra a una versión nueva y siempre está en Ajustes › Acerca de.
use crate::{pantallas, App, Mensaje, Seccion};
use icaro_ui::novedades::{self as v, Funcion, Tipo, Version};
use iced::{Element, Task};
use serde::Deserialize;
use std::sync::OnceLock;
use std::time::SystemTime;

/// Notas de cada versión. Al publicar una versión nueva, agregar su archivo
/// aquí (una prueba lo exige).
const NOTAS: &[&str] = &[
    include_str!("../../../../docs/releases/0.1.0.json"),
    include_str!("../../../../docs/releases/0.2.0.json"),
    include_str!("../../../../docs/releases/0.2.1.json"),
    include_str!("../../../../docs/releases/0.2.2.json"),
    include_str!("../../../../docs/releases/0.2.3.json"),
    include_str!("../../../../docs/releases/0.2.4.json"),
    include_str!("../../../../docs/releases/0.2.5.json"),
    include_str!("../../../../docs/releases/0.3.0.json"),
    include_str!("../../../../docs/releases/0.3.1.json"),
    include_str!("../../../../docs/releases/0.3.2.json"),
    include_str!("../../../../docs/releases/0.3.3.json"),
    include_str!("../../../../docs/releases/0.3.4.json"),
    include_str!("../../../../docs/releases/0.4.0.json"),
    include_str!("../../../../docs/releases/0.4.1.json"),
    include_str!("../../../../docs/releases/0.4.2.json"),
    include_str!("../../../../docs/releases/0.4.3.json"),
];
const CHANGELOG: &str = include_str!("../../../../CHANGELOG.md");
const ACTUAL: &str = env!("CARGO_PKG_VERSION");

#[derive(Deserialize)]
struct Archivo {
    version: String,
    notas: Vec<Nota>,
}

#[derive(Deserialize)]
struct Nota {
    tipo: String,
    texto: String,
    #[serde(default)]
    titulo: Option<String>,
    #[serde(default)]
    accion: Option<String>,
    /// Qué pasa con lo guardado si se vuelve a una versión anterior.
    #[serde(default)]
    al_bajar: Option<String>,
}

/// Funciones nuevas de una versión con lo que pasa al volver a Estable
/// (CanalActualizaciones): título y consecuencia.
pub fn al_volver(version: &str) -> Vec<(String, String)> {
    NOTAS
        .iter()
        .filter_map(|j| serde_json::from_str::<Archivo>(j).ok())
        .filter(|a| a.version == version)
        .flat_map(|a| a.notas.into_iter())
        .filter(|n| n.tipo == "nuevo")
        .map(|n| {
            let f = funcion(&n);
            let consecuencia = n
                .al_bajar
                .clone()
                .unwrap_or_else(|| "Deja de estar disponible; lo guardado se conserva.".into());
            (f.titulo, consecuencia)
        })
        .collect()
}

/// Una versión con el destino de cada acción de sus funciones nuevas.
struct Registro {
    version: Version,
    destinos: Vec<Option<String>>,
}

#[derive(Default)]
pub struct Estado {
    pub abierta: bool,
    pub elegida: usize,
    pub filtro: Option<Tipo>,
    pub recien: bool,
    /// Sección desde la que se abrió (si no fue Ajustes), para volver a ella.
    pub volver: Option<Seccion>,
    /// Versiones que se muestran: la disponible (si se abrió desde el aviso
    /// de actualización) seguida de las incluidas en el launcher.
    pub versiones: Vec<Version>,
    /// 1 si la primera es la versión disponible, aún sin instalar.
    desfase: usize,
}

#[derive(Debug, Clone)]
pub enum Accion {
    /// Abre en la versión instalada; `recien` tras actualizar.
    Abrir {
        recien: bool,
    },
    Elegir(usize),
    Filtrar(Option<Tipo>),
    /// Acción de la función nueva `i` de la versión elegida.
    Ir(usize),
    Github,
    Cerrar,
}

fn numero(version: &str) -> (u32, u32, u32) {
    let mut partes = version.split(['.', '-']).map(|p| p.parse().unwrap_or(0));
    (
        partes.next().unwrap_or(0),
        partes.next().unwrap_or(0),
        partes.next().unwrap_or(0),
    )
}

/// Días desde la época para una fecha civil (algoritmo de Howard Hinnant).
fn dias_civiles(anio: i64, mes: u32, dia: u32) -> i64 {
    let y = if mes <= 2 { anio - 1 } else { anio };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(mes);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(dia) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Fecha de cada versión según el CHANGELOG («## [0.4.0] - 2026-11-09»).
fn fecha_de(version: &str) -> Option<(i64, u32, u32)> {
    let cabeza = format!("## [{version}] - ");
    let linea = CHANGELOG
        .lines()
        .find_map(|l| l.strip_prefix(cabeza.as_str()))?;
    let mut p = linea.trim().split('-');
    Some((
        p.next()?.parse().ok()?,
        p.next()?.parse().ok()?,
        p.next()?.parse().ok()?,
    ))
}

/// «Hoy», «Ayer», «Hace 12 días», «Hace 4 semanas» o «Hace 2 meses».
pub fn hace(dias: i64) -> String {
    match dias {
        i64::MIN..=0 => "Hoy".into(),
        1 => "Ayer".into(),
        2..=13 => format!("Hace {dias} días"),
        14..=59 => format!("Hace {} semanas", dias / 7),
        _ => format!("Hace {} meses", dias / 30),
    }
}

fn rotulo(destino: &str) -> Option<&'static str> {
    match destino {
        "servidores" => Some("Abrir Servidores"),
        "cuenta" => Some("Ver tu cuenta"),
        "tienda" => Some("Abrir la tienda"),
        "apariencia" => Some("Elegir transición"),
        "canal" => Some("Ver el canal"),
        "instancias" => Some("Ir a Instancias"),
        _ => None,
    }
}

/// Título y frase de una función nueva. Sin título en la nota, se toma lo
/// que va antes de «: » si es corto; si no, toda la nota es el título.
fn funcion(n: &Nota) -> Funcion {
    let (titulo, texto) = match &n.titulo {
        Some(t) => {
            let resto = n
                .texto
                .strip_prefix(t.as_str())
                .and_then(|r| r.strip_prefix(": "));
            (t.clone(), resto.unwrap_or(&n.texto).to_owned())
        }
        None => match n.texto.split_once(": ") {
            Some((t, r)) if t.chars().count() <= 48 => (t.to_owned(), r.to_owned()),
            _ => (n.texto.clone(), String::new()),
        },
    };
    let mut letras = texto.chars();
    let texto = match letras.next() {
        Some(c) => c.to_uppercase().chain(letras).collect(),
        None => texto,
    };
    Funcion {
        titulo,
        texto,
        accion: n.accion.as_deref().and_then(rotulo).map(str::to_owned),
    }
}

fn registros_con(hoy: i64) -> Vec<Registro> {
    let mut lista: Vec<Registro> = NOTAS
        .iter()
        .filter_map(|j| serde_json::from_str::<Archivo>(j).ok())
        // Las notas de versiones futuras (aún sin publicar) no se muestran.
        .filter(|a| numero(&a.version) <= numero(ACTUAL))
        .map(|a| {
            let fecha = fecha_de(&a.version);
            let mut version = Version {
                fecha: fecha.map_or_else(
                    || "Sin fecha".into(),
                    |(y, m, d)| format!("{d:02}-{m:02}-{y}"),
                ),
                hace: fecha.map_or_else(String::new, |(y, m, d)| hace(hoy - dias_civiles(y, m, d))),
                version: a.version,
                nuevo: Vec::new(),
                mejoras: Vec::new(),
                correcciones: Vec::new(),
            };
            let mut destinos = Vec::new();
            for n in &a.notas {
                match n.tipo.as_str() {
                    "nuevo" => {
                        version.nuevo.push(funcion(n));
                        destinos.push(n.accion.clone());
                    }
                    "mejora" => version.mejoras.push(n.texto.clone()),
                    _ => version.correcciones.push(n.texto.clone()),
                }
            }
            Registro { version, destinos }
        })
        .collect();
    lista.sort_by(|a, b| numero(&b.version.version).cmp(&numero(&a.version.version)));
    lista
}

fn registros() -> &'static [Registro] {
    static LISTA: OnceLock<Vec<Registro>> = OnceLock::new();
    LISTA.get_or_init(|| {
        let f = crate::hora_local::local(SystemTime::now());
        registros_con(dias_civiles(f.anio, f.mes, f.dia))
    })
}

fn instalada() -> Option<usize> {
    registros().iter().position(|r| r.version.version == ACTUAL)
}

/// Las notas de una versión publicada que todavía no está instalada.
fn version_disponible(d: &icaro_core::actualizacion::Disponible) -> Version {
    let mut v = Version {
        version: d.version.clone(),
        fecha: "Disponible".into(),
        hace: String::new(),
        nuevo: Vec::new(),
        mejoras: Vec::new(),
        correcciones: Vec::new(),
    };
    for n in &d.notas {
        match n.tipo.as_str() {
            "nuevo" => v.nuevo.push(funcion(&Nota {
                tipo: n.tipo.clone(),
                texto: n.texto.clone(),
                titulo: n.titulo.clone(),
                accion: None,
                al_bajar: None,
            })),
            "mejora" => v.mejoras.push(n.texto.clone()),
            _ => v.correcciones.push(n.texto.clone()),
        }
    }
    v
}

/// Al iniciar: abre las novedades si el estado guardado viene de otra
/// versión. Una instalación nueva (sin estado) no las abre.
pub fn al_iniciar(app: &mut App, vistas: Option<Option<String>>) {
    if let Some(vistas) = vistas {
        if vistas.as_deref() != Some(ACTUAL) && instalada().is_some() {
            abrir(app, true);
        }
    }
}

fn abrir(app: &mut App, recien: bool) {
    abrir_con(app, recien, None);
}

/// Abre la página en Ajustes; al cerrarla se vuelve a la sección de origen.
/// Con `disponible`, la versión nueva va primero y queda elegida.
pub fn abrir_con(
    app: &mut App,
    recien: bool,
    disponible: Option<&icaro_core::actualizacion::Disponible>,
) {
    let volver = (app.seccion != Seccion::Ajustes).then_some(app.seccion);
    let extra = disponible
        .filter(|d| numero(&d.version) > numero(ACTUAL))
        .map(version_disponible);
    let desfase = usize::from(extra.is_some());
    let versiones: Vec<Version> = extra
        .into_iter()
        .chain(registros().iter().map(|r| r.version.clone()))
        .collect();
    app.novedades = Estado {
        abierta: true,
        elegida: if desfase == 1 { 0 } else { instalada().unwrap_or(0) },
        filtro: None,
        recien,
        volver,
        versiones,
        desfase,
    };
    app.seccion = Seccion::Ajustes;
}

pub fn actualizar(app: &mut App, accion: Accion) -> Task<Mensaje> {
    match accion {
        Accion::Abrir { recien } => abrir(app, recien),
        Accion::Elegir(i) => {
            app.novedades.elegida = i.min(app.novedades.versiones.len().saturating_sub(1));
            app.novedades.filtro = None;
        }
        Accion::Filtrar(f) => app.novedades.filtro = f,
        Accion::Ir(i) => {
            let destino = app
                .novedades
                .elegida
                .checked_sub(app.novedades.desfase)
                .and_then(|n| registros().get(n))
                .and_then(|r| r.destinos.get(i).cloned().flatten());
            app.novedades.abierta = false;
            return match destino.as_deref() {
                Some("servidores") => Task::done(Mensaje::Ir(Seccion::Servidores)),
                Some("tienda") => Task::done(Mensaje::Ir(Seccion::Modpacks)),
                Some("instancias") => Task::done(Mensaje::Ir(Seccion::Instancias)),
                // Ajustes › Apariencia y Ajustes › Acerca de (canal).
                Some(d @ ("apariencia" | "canal")) => {
                    app.ajuste = if d == "apariencia" { 1 } else { 7 };
                    Task::done(Mensaje::Ir(Seccion::Ajustes))
                }
                Some("cuenta") => Task::done(Mensaje::Pant(pantallas::Accion::Abrir(
                    pantallas::Pantalla::Cuenta,
                ))),
                _ => Task::none(),
            };
        }
        Accion::Github => {
            return crate::servicios::actualizar(
                app,
                crate::servicios::Accion::Enlace(format!(
                    "{}/releases",
                    env!("CARGO_PKG_REPOSITORY")
                )),
            );
        }
        Accion::Cerrar => {
            app.novedades.abierta = false;
            if let Some(s) = app.novedades.volver.take() {
                return Task::done(Mensaje::Ir(s));
            }
        }
    }
    Task::none()
}

/// La página de Novedades dentro de Ajustes: migas «Ajustes › Novedades» y el
/// bloque centrado que llena el alto, igual que Crear instancia.
pub fn pagina(app: &App) -> Element<'_, Mensaje> {
    use icaro_ui::tema::espacio;
    use iced::widget::{column, container};
    use iced::{Length, Padding};
    let p = app.paleta();
    let migas = icaro_ui::migas::migas(
        p,
        &[
            (
                "Ajustes".to_string(),
                Some(Mensaje::Novedades(Accion::Cerrar)),
            ),
            ("Novedades".to_string(), None),
        ],
    );
    container(
        column![migas, bloque(app)]
            .spacing(espacio::S4)
            .width(Length::Fill)
            .max_width(v::ANCHO)
            .height(Length::Fill),
    )
    .center_x(Length::Fill)
    .height(Length::Fill)
    .padding(Padding {
        top: espacio::S6,
        right: espacio::S6,
        bottom: espacio::S6,
        left: espacio::S6,
    })
    .style(icaro_ui::estilo::bloque(p.bg))
    .into()
}

fn bloque(app: &App) -> Element<'_, Mensaje> {
    let m = |a| Mensaje::Novedades(a);
    let versiones = &app.novedades.versiones;
    let instalada = versiones.iter().position(|v| v.version == ACTUAL);
    let pie = if app.novedades.recien {
        "Esto vuelve a estar en Ajustes › Acerca de.".to_owned()
    } else if app.novedades.desfase == 1 {
        format!("Tienes la {ACTUAL}; la {} está disponible.", versiones[0].version)
    } else if instalada == Some(0) {
        format!("Tienes la {ACTUAL}, la más reciente.")
    } else {
        format!("Tienes la {ACTUAL}.")
    };
    v::novedades(
        app.paleta(),
        v::Datos {
            versiones,
            elegida: app.novedades.elegida,
            instalada,
            filtro: app.novedades.filtro,
            recien: app.novedades.recien,
            pie,
        },
        v::Mensajes {
            elegir: Box::new(move |i| m(Accion::Elegir(i))),
            filtrar: Box::new(move |f| m(Accion::Filtrar(f))),
            accion: Box::new(move |i| m(Accion::Ir(i))),
            github: m(Accion::Github),
            cerrar: m(Accion::Cerrar),
        },
    )
}

/// Escenas: `novedades` (desde Acerca de), `novedades-recien` y
/// `novedades-mantenimiento` (una versión solo con correcciones).
pub fn preparar_diseno(app: &mut App, escena: &str) {
    abrir(app, escena.ends_with("-recien"));
    if escena.ends_with("-mantenimiento") {
        if let Some(i) = app.novedades.versiones.iter().position(|v| v.nuevo.is_empty()) {
            app.novedades.elegida = i;
        }
    }
    if let Some(f) = escena.strip_prefix("novedades-filtro-") {
        app.novedades.filtro = match f {
            "nuevo" => Some(Tipo::Nuevo),
            "mejoras" => Some(Tipo::Mejora),
            _ => Some(Tipo::Correccion),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cada_archivo_de_notas_esta_incluido_y_es_valido() {
        let carpeta = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/releases");
        let archivos = std::fs::read_dir(carpeta)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
            .count();
        assert_eq!(archivos, NOTAS.len(), "agrega el JSON nuevo a NOTAS");
        for j in NOTAS {
            let a: Archivo = serde_json::from_str(j).unwrap();
            assert!(a
                .notas
                .iter()
                .all(|n| matches!(n.tipo.as_str(), "nuevo" | "mejora" | "correccion")));
            assert!(a
                .notas
                .iter()
                .all(|n| n.accion.as_deref().is_none_or(|d| rotulo(d).is_some())));
        }
    }

    #[test]
    fn la_version_instalada_va_primero_y_tiene_fecha() {
        let lista = registros_con(dias_civiles(2026, 11, 9));
        assert_eq!(lista[0].version.version, ACTUAL);
        assert!(lista
            .windows(2)
            .all(|w| numero(&w[0].version.version) > numero(&w[1].version.version)));
        let v040 = lista.iter().find(|r| r.version.version == "0.4.0").unwrap();
        assert_eq!(v040.version.fecha, "09-11-2026");
        assert_eq!(v040.version.hace, "Hoy");
        assert_eq!(
            v040.version.nuevo[0].titulo,
            "Compartir instancias (experimental)"
        );
        assert!(v040.version.nuevo[0].texto.starts_with("Publica"));
        assert_eq!(
            v040.version.nuevo[0].accion.as_deref(),
            Some("Abrir Servidores")
        );
    }

    #[test]
    fn los_textos_de_tiempo_siguen_el_diseno() {
        assert_eq!(hace(0), "Hoy");
        assert_eq!(hace(1), "Ayer");
        assert_eq!(hace(12), "Hace 12 días");
        assert_eq!(hace(28), "Hace 4 semanas");
        assert_eq!(dias_civiles(1970, 1, 1), 0);
        assert_eq!(dias_civiles(2026, 11, 9) - dias_civiles(2026, 10, 28), 12);
    }

    #[test]
    fn una_funcion_sin_titulo_usa_lo_que_va_antes_de_los_dos_puntos() {
        let n = Nota {
            tipo: "nuevo".into(),
            texto: "Skin local por cuenta: carga un PNG.".into(),
            titulo: None,
            accion: None,
            al_bajar: None,
        };
        let f = funcion(&n);
        assert_eq!(
            (f.titulo.as_str(), f.texto.as_str()),
            ("Skin local por cuenta", "Carga un PNG.")
        );
    }
}
