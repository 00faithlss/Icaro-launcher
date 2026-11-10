// SPDX-License-Identifier: GPL-3.0-only
//! Registro legible `logs/latest.log`, al estilo del de Minecraft:
//!
//! ```text
//! [2026-11-10 02:16:06] [launcher/INFO]: Ícaro 0.4.1 (compilación 42757294569) en windows
//! [2026-11-10 02:16:07] [compartir/INFO]: listar_publicaciones: completado en 1 ms
//! [2026-11-10 02:16:09] [juego/ERROR]: preparar: falló en «descarga» tras 812 ms · red (tiempo agotado) · juego_real.rs:640
//! ```
//!
//! Cada línea sale del mismo evento que `diagnostico.jsonl`, así que tampoco
//! lleva rutas, URL ni textos de errores externos: solo etiquetas del código,
//! números y causas tipadas. Al iniciar, el `latest.log` anterior pasa a
//! `AAAA-MM-DD-N.log` y se conservan los últimos [`ARCHIVADOS`].
use super::modelos::{civil, Cabecera, Campo, Causa, Evento, Nivel, TipoEvento};
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicI64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

/// Nombre del registro legible dentro de `logs/`.
pub const ARCHIVO: &str = "latest.log";
/// Registros de sesiones anteriores que se conservan.
pub const ARCHIVADOS: usize = 10;
/// Tamaño a partir del cual se deja de escribir en la sesión actual.
const LIMITE: u64 = 8 * 1024 * 1024;

static DESFASE: AtomicI64 = AtomicI64::new(0);

/// Segundos que hay que sumar a UTC para la hora local. La aplicación lo fija
/// al iniciar (el núcleo no consulta la zona horaria del sistema).
pub fn usar_desfase_local(segundos: i64) {
    DESFASE.store(segundos, Ordering::Relaxed);
}

/// `AAAA-MM-DD HH:MM:SS` en hora local.
pub fn fecha_local(fecha: SystemTime) -> String {
    let segundos = fecha
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
        + DESFASE.load(Ordering::Relaxed);
    let (anio, mes, dia) = civil(segundos.div_euclid(86_400));
    let resto = segundos.rem_euclid(86_400);
    format!(
        "{anio:04}-{mes:02}-{dia:02} {:02}:{:02}:{:02}",
        resto / 3_600,
        resto % 3_600 / 60,
        resto % 60
    )
}

fn nivel(n: Nivel) -> &'static str {
    match n {
        Nivel::Info => "INFO",
        Nivel::Aviso => "WARN",
        Nivel::Error => "ERROR",
    }
}

/// Causa en palabras: «disco (permiso denegado, código 5)», «red (sin conexión)».
fn causa(c: &Causa) -> String {
    let json = c.json();
    let mut partes: Vec<String> = Vec::new();
    if let Some(m) = json.as_object() {
        for (k, v) in m {
            if k == "categoria" || v.is_null() || v == &serde_json::Value::Bool(false) {
                continue;
            }
            partes.push(match v {
                serde_json::Value::Bool(true) => k.clone(),
                serde_json::Value::String(s) => format!("{k}={s}"),
                otro => format!("{k}={otro}"),
            });
        }
    }
    if partes.is_empty() {
        c.categoria().to_owned()
    } else {
        format!("{} ({})", c.categoria(), partes.join(", "))
    }
}

fn datos(d: &[Campo]) -> String {
    d.iter()
        .map(|c| match *c {
            Campo::Numero(n, v) => format!("{n}={v}"),
            Campo::Etiqueta(n, v) => format!("{n}={v}"),
            Campo::CodigoSalida(Some(c)) => format!("código de salida {c}"),
            Campo::CodigoSalida(None) => "terminó sin código de salida".to_owned(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// Una línea del registro legible, terminada en salto de línea.
pub(crate) fn linea(e: &Evento) -> String {
    let mut texto = match e.tipo {
        TipoEvento::Inicio => format!("{}: empieza", e.accion),
        TipoEvento::Etapa => format!("{} › {}", e.accion, e.etapa.unwrap_or("etapa")),
        TipoEvento::Dato => format!("{}: {}", e.accion, datos(&e.datos)),
        TipoEvento::Fin => {
            let duracion = e.duracion_ms.map(|ms| format!(" en {ms} ms")).unwrap_or_default();
            match (e.resultado, &e.causa) {
                (_, Some(c)) => {
                    let donde = e.etapa.map(|s| format!(" en «{s}»")).unwrap_or_default();
                    let tras = e.duracion_ms.map(|ms| format!(" tras {ms} ms")).unwrap_or_default();
                    format!("{}: falló{donde}{tras} · {}", e.accion, causa(c))
                }
                (Some("cancelado"), None) => format!("{}: cancelado{duracion}", e.accion),
                _ => format!("{}: completado{duracion}", e.accion),
            }
        }
        TipoEvento::Abandono => format!("{}: quedó sin terminar", e.accion),
        TipoEvento::Descartes => format!("se perdieron registros ({})", datos(&e.datos)),
        TipoEvento::Panico => format!("{}: cierre inesperado", e.accion),
    };
    if e.tipo != TipoEvento::Dato && e.tipo != TipoEvento::Descartes && !e.datos.is_empty() {
        texto.push_str(&format!(" · {}", datos(&e.datos)));
    }
    if let Some(alias) = &e.instancia {
        texto.push_str(&format!(" · instancia {alias}"));
    }
    if e.nivel != Nivel::Info {
        if let Some(u) = e.ubicacion {
            texto.push_str(&format!(" · {}:{}", u.archivo(), u.linea()));
        }
    }
    format!(
        "[{}] [{}/{}]: {}\n",
        fecha_local(e.fecha),
        e.componente,
        nivel(e.nivel),
        texto.replace(['\r', '\n'], " ")
    )
}

/// Archiva el `latest.log` anterior, poda los archivados y escribe la
/// cabecera de esta sesión. Si algo falla, el registro legible queda
/// desactivado (devuelve `None`) sin afectar a `diagnostico.jsonl`.
pub(crate) fn preparar(logs: &Path, cab: &Cabecera) -> Option<PathBuf> {
    let ruta = logs.join(ARCHIVO);
    if let Ok(meta) = fs::symlink_metadata(&ruta) {
        if !meta.is_file() {
            return None;
        }
        let fecha = meta.modified().unwrap_or_else(|_| SystemTime::now());
        let dia = fecha_local(fecha)[..10].to_owned();
        let destino = (1..1000)
            .map(|n| logs.join(format!("{dia}-{n}.log")))
            .find(|p| !p.exists())?;
        fs::rename(&ruta, destino).ok()?;
    }
    podar(logs);
    let cabecera = format!(
        "[{}] [launcher/INFO]: Ícaro {} (compilación {}) en {} · sesión {}\n",
        fecha_local(SystemTime::now()),
        cab.version,
        cab.compilacion,
        std::env::consts::OS,
        cab.sesion
    );
    let mut f = OpenOptions::new().create_new(true).write(true).open(&ruta).ok()?;
    f.write_all(cabecera.as_bytes()).ok()?;
    Some(ruta)
}

/// `AAAA-MM-DD-N.log`
fn es_archivado(nombre: &str) -> bool {
    let Some(base) = nombre.strip_suffix(".log") else {
        return false;
    };
    let b = base.as_bytes();
    b.len() >= 12
        && b[..10].iter().enumerate().all(|(i, c)| if i == 4 || i == 7 { *c == b'-' } else { c.is_ascii_digit() })
        && b[10] == b'-'
        && b[11..].iter().all(u8::is_ascii_digit)
}

fn podar(logs: &Path) {
    let Ok(entradas) = fs::read_dir(logs) else { return };
    let mut archivados: Vec<(SystemTime, PathBuf)> = entradas
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .filter(|e| e.file_name().to_str().is_some_and(es_archivado))
        .map(|e| (e.metadata().and_then(|m| m.modified()).unwrap_or(UNIX_EPOCH), e.path()))
        .collect();
    if archivados.len() < ARCHIVADOS {
        return;
    }
    archivados.sort();
    // Se deja sitio para el que se archivará en la próxima sesión.
    for (_, ruta) in &archivados[..archivados.len() + 1 - ARCHIVADOS] {
        let _ = fs::remove_file(ruta);
    }
}

/// Añade líneas al registro legible; sin efecto si superó el límite. Se
/// llama desde el hilo escritor, nunca desde la interfaz.
pub(crate) fn anexar(ruta: &Path, texto: &str) {
    let Ok(meta) = fs::symlink_metadata(ruta) else { return };
    if !meta.is_file() || meta.len() > LIMITE {
        return;
    }
    if let Ok(mut f) = OpenOptions::new().append(true).open(ruta) {
        let _ = f.write_all(texto.as_bytes());
        if meta.len() + texto.len() as u64 > LIMITE {
            let _ = f.write_all(
                format!(
                    "[{}] [diagnostico/WARN]: el registro llegó a su tamaño máximo; el resto de la sesión queda en diagnostico.jsonl\n",
                    fecha_local(SystemTime::now())
                )
                .as_bytes(),
            );
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn la_linea_lleva_fecha_componente_nivel_y_causa() {
        usar_desfase_local(-3 * 3600);
        let mut e = Evento::nuevo(TipoEvento::Fin, Nivel::Error, "juego", "preparar");
        e.fecha = UNIX_EPOCH + std::time::Duration::from_secs(1_794_197_766);
        e.etapa = Some("descarga");
        e.duracion_ms = Some(812);
        e.resultado = Some("error");
        e.causa = Some(Causa::Red { timeout: true, conexion: false, estado: None });
        let l = linea(&e);
        assert!(l.starts_with("[2026-11-09 01:16:06] [juego/ERROR]: preparar: falló en «descarga» tras 812 ms · red"), "{l}");
        assert!(l.contains("timeout") && l.ends_with('\n'), "{l}");
        let mut ok = Evento::nuevo(TipoEvento::Fin, Nivel::Info, "compartir", "listar_publicaciones");
        ok.duracion_ms = Some(1);
        ok.resultado = Some("exito");
        assert!(linea(&ok).ends_with("[compartir/INFO]: listar_publicaciones: completado en 1 ms\n"));
        usar_desfase_local(0);
    }

    #[test]
    fn al_iniciar_archiva_el_anterior_y_conserva_los_ultimos() {
        let dir = std::env::temp_dir().join(format!("icaro-legible-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let cab = Cabecera { sesion: "abc".into(), version: "0.4.1", compilacion: "7" };
        for _ in 0..(ARCHIVADOS + 3) {
            let ruta = preparar(&dir, &cab).unwrap();
            anexar(&ruta, "[x] [y/INFO]: z\n");
        }
        let nombres: Vec<String> = fs::read_dir(&dir)
            .unwrap()
            .flatten()
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(nombres.contains(&ARCHIVO.to_owned()));
        assert_eq!(nombres.iter().filter(|n| es_archivado(n)).count(), ARCHIVADOS - 1);
        let actual = fs::read_to_string(dir.join(ARCHIVO)).unwrap();
        assert!(actual.contains("[launcher/INFO]: Ícaro 0.4.1 (compilación 7)") && actual.contains("[y/INFO]: z"));
        assert!(!es_archivado("latest.log") && !es_archivado("2026-11-10.log") && es_archivado("2026-11-10-12.log"));
        fs::remove_dir_all(&dir).unwrap();
    }
}
