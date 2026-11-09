// SPDX-License-Identifier: GPL-3.0-only
//! Pruebas colocadas: límites pequeños y una compuerta que detiene al escritor
//! de forma determinista. Solo usan carpetas temporales propias.
use super::{
    archivos,
    escritor::{Compuerta, Config},
    iniciar_con, modelos, ErrorRegistro, Resultado, ARCHIVO,
};
use serde_json::Value;
use std::{
    fs,
    panic::Location,
    path::{Path, PathBuf},
    sync::Arc,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub(super) const PLAZO: Duration = Duration::from_secs(5);

pub(super) struct Temporal(pub(super) PathBuf);

impl Temporal {
    pub(super) fn nueva() -> Self {
        static SIGUIENTE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let marca = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let raiz = std::env::temp_dir().join(format!(
            "icaro-diag-unidad-{}-{}-{marca}",
            std::process::id(),
            SIGUIENTE.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir(&raiz).unwrap();
        Self(raiz)
    }

    pub(super) fn logs(&self) -> PathBuf {
        self.0.join("logs")
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub(super) fn lineas(ruta: &Path) -> Vec<Value> {
    fs::read_to_string(ruta)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect()
}

fn esperar_cierre(registro: &super::Registro) {
    let fin = Instant::now() + PLAZO;
    while !registro.estado().cerrado && Instant::now() < fin {
        thread::sleep(Duration::from_millis(5));
    }
    assert!(registro.estado().cerrado);
}

#[test]
fn fecha_civil_y_formato_iso_conocidos() {
    assert_eq!(modelos::civil(0), (1970, 1, 1));
    assert_eq!(modelos::civil(-1), (1969, 12, 31));
    assert_eq!(modelos::civil(951_868_800 / 86_400), (2000, 3, 1));
    assert_eq!(modelos::civil(1_709_164_800 / 86_400), (2024, 2, 29));
    let fecha = UNIX_EPOCH + Duration::from_millis(1_709_164_800_123);
    assert_eq!(modelos::fecha_iso(fecha), "2024-02-29T00:00:00.123Z");
}

#[test]
fn cola_saturada_no_bloquea_y_deja_constancia_al_recuperarse() {
    let t = Temporal::nueva();
    let compuerta = Arc::new(Compuerta::default());
    let config = Config {
        capacidad: 4,
        compuerta: Some(compuerta.clone()),
        ..Config::default()
    };
    let mut sesion = iniciar_con(&t.0, "prueba", "0", config).unwrap();
    let r = sesion.registro();

    let inicio = Instant::now();
    for _ in 0..10 {
        r.medir("prueba", "saturar").terminar(Resultado::Exito);
    }
    assert!(
        inicio.elapsed() < Duration::from_secs(1),
        "los emisores no deben esperar"
    );
    let estado = r.estado();
    assert_eq!((estado.enviados, estado.descartados), (4, 16));

    compuerta.abrir();
    r.vaciar(PLAZO).unwrap();
    let estado = r.estado();
    assert_eq!(
        (
            estado.escritos,
            estado.pendientes(),
            estado.errores_escritura
        ),
        (4, 0, 0)
    );
    sesion.cerrar(PLAZO).unwrap();

    let ls = lineas(&t.logs().join(ARCHIVO));
    assert_eq!(ls.len(), 5);
    for (i, linea) in ls.iter().enumerate() {
        assert_eq!(linea["seq"], i as u64);
    }
    let informe = ls
        .iter()
        .find(|l| l["evento"] == "descartes")
        .expect("informe de descartes");
    assert_eq!(informe["datos"]["descartados"], 16);
    assert_eq!(informe["datos"]["descartados_total"], 16);
    assert_eq!(informe["nivel"], "aviso");
}

#[test]
fn rotacion_respeta_el_limite_y_tres_respaldos_sin_tocar_ajenos() {
    let t = Temporal::nueva();
    let logs = t.logs();
    fs::create_dir(&logs).unwrap();
    fs::write(logs.join("diagnostico.jsonl.otro"), b"ajeno").unwrap();
    fs::write(logs.join("sincronizacion.log"), b"ajeno").unwrap();
    let limite = 4096;
    let config = Config {
        limite,
        ..Config::default()
    };
    let mut sesion = iniciar_con(&t.0, "prueba", "0", config).unwrap();
    let r = sesion.registro();
    for _ in 0..40 {
        for _ in 0..10 {
            r.medir("prueba", "rotar").terminar(Resultado::Exito);
        }
        r.vaciar(PLAZO).unwrap();
    }
    sesion.cerrar(PLAZO).unwrap();

    let base = logs.join(ARCHIVO);
    let orden = [3, 2, 1].map(|n| archivos::respaldo(&base, n));
    let mut seqs = Vec::new();
    for ruta in orden.iter().chain(std::iter::once(&base)) {
        let tamano = fs::metadata(ruta).unwrap().len();
        assert!(
            tamano > 0 && tamano <= limite,
            "tamaño fuera de cota: {tamano}"
        );
        seqs.extend(lineas(ruta).iter().map(|l| l["seq"].as_u64().unwrap()));
    }
    assert!(
        seqs.windows(2).all(|par| par[1] == par[0] + 1),
        "secuencia contigua entre respaldos"
    );
    assert!(!archivos::respaldo(&base, 4).exists());
    assert_eq!(
        fs::read(logs.join("diagnostico.jsonl.otro")).unwrap(),
        b"ajeno"
    );
    assert_eq!(fs::read(logs.join("sincronizacion.log")).unwrap(), b"ajeno");
    let estado = r.estado();
    assert_eq!(
        (estado.escritos, estado.errores_escritura, estado.perdidos),
        (800, 0, 0)
    );
}

#[test]
fn rotacion_bloqueada_no_hace_crecer_el_registro_ni_toca_lo_ajeno() {
    let t = Temporal::nueva();
    let logs = t.logs();
    fs::create_dir(&logs).unwrap();
    let ajeno = logs.join("diagnostico.jsonl.2");
    fs::create_dir(&ajeno).unwrap();
    fs::write(ajeno.join("ajeno.txt"), b"ajeno").unwrap();
    let limite = 4096;
    let config = Config {
        limite,
        ..Config::default()
    };
    let mut sesion = iniciar_con(&t.0, "prueba", "0", config).unwrap();
    let r = sesion.registro();
    let mut fallos = 0;
    for _ in 0..10 {
        for _ in 0..10 {
            r.medir("prueba", "rotar").terminar(Resultado::Exito);
        }
        if r.vaciar(PLAZO).is_err() {
            fallos += 1;
        }
    }
    let _ = sesion.cerrar(PLAZO);

    let base = logs.join(ARCHIVO);
    assert!(fallos > 0, "el vaciado debe informar la pérdida");
    assert!(fs::metadata(&base).unwrap().len() <= limite);
    assert!(
        !archivos::respaldo(&base, 1).exists(),
        "no se rota a medias"
    );
    assert!(ajeno.is_dir());
    assert_eq!(fs::read(ajeno.join("ajeno.txt")).unwrap(), b"ajeno");
    let estado = r.estado();
    assert!(estado.errores_escritura > 0 && estado.perdidos > 0);
    assert_eq!(estado.ultimo_error, Some(archivos::NO_REGULAR));
    assert_eq!(estado.escritos + estado.perdidos, estado.enviados);
}

#[test]
fn cierre_con_escritor_atascado_respeta_el_plazo() {
    let t = Temporal::nueva();
    let compuerta = Arc::new(Compuerta::default());
    let config = Config {
        compuerta: Some(compuerta.clone()),
        ..Config::default()
    };
    let mut sesion = iniciar_con(&t.0, "prueba", "0", config).unwrap();
    let r = sesion.registro();
    r.medir("prueba", "atascada").terminar(Resultado::Exito);

    let inicio = Instant::now();
    assert_eq!(
        sesion.cerrar(Duration::from_millis(200)),
        Err(ErrorRegistro::Plazo)
    );
    assert!(inicio.elapsed() < Duration::from_secs(2));
    assert_eq!(
        sesion.cerrar(Duration::from_millis(200)),
        Ok(()),
        "cerrar es idempotente"
    );
    assert_eq!(
        r.vaciar(Duration::from_millis(50)),
        Err(ErrorRegistro::Plazo)
    );
    assert_eq!(
        r.estado().pendientes(),
        2,
        "la pérdida potencial queda visible"
    );

    compuerta.abrir();
    esperar_cierre(&r);
    assert_eq!(r.estado().escritos, 2);
}

#[test]
fn descartar_una_sesion_atascada_tiene_plazo_acotado() {
    let t = Temporal::nueva();
    let compuerta = Arc::new(Compuerta::default());
    let config = Config {
        compuerta: Some(compuerta.clone()),
        plazo_drop: Duration::from_millis(100),
        ..Config::default()
    };
    let sesion = iniciar_con(&t.0, "prueba", "0", config).unwrap();
    let r = sesion.registro();
    let inicio = Instant::now();
    drop(sesion);
    assert!(inicio.elapsed() < Duration::from_secs(2));
    compuerta.abrir();
    esperar_cierre(&r);
}

#[test]
fn linea_de_panico_solo_lleva_ubicacion_y_clase() {
    let cab = modelos::Cabecera {
        sesion: "0011223344556677".into(),
        version: "prueba",
        compilacion: "0",
    };
    let bytes = super::panico::linea_panico(&cab, Some(Location::caller()), "texto_dinamico");
    let texto = String::from_utf8(bytes).unwrap();
    assert!(texto.ends_with('\n'));
    let linea: Value = serde_json::from_str(texto.trim_end()).unwrap();
    assert_eq!(linea["ubicacion"]["archivo"], "pruebas.rs");
    assert!(linea["ubicacion"]["linea"].is_u64());
    assert_eq!(linea["causa"]["categoria"], "panico");
    assert_eq!(linea["carga"], "texto_dinamico");
    assert!(linea["seq"].is_null());
    assert!(
        !texto.contains('/') && !texto.contains('\\'),
        "sin rutas de código"
    );
}

#[test]
fn alias_de_instancia_depende_de_la_sesion_y_rechaza_texto() {
    let t = Temporal::nueva();
    let id = "0123456789abcdef0123456789ABCDEF";
    let mut a = iniciar_con(&t.0, "prueba", "0", Config::default()).unwrap();
    let mut b = iniciar_con(&t.0, "prueba", "0", Config::default()).unwrap();
    let alias_a = a.registro().alias_instancia(id).unwrap();
    assert_eq!(alias_a.len(), 16);
    assert_eq!(
        Some(alias_a.clone()),
        a.registro().alias_instancia(&id.to_ascii_lowercase())
    );
    assert_ne!(Some(alias_a.clone()), b.registro().alias_instancia(id));
    assert!(!id.to_ascii_lowercase().contains(&*alias_a));
    assert_eq!(a.registro().alias_instancia("mi-mundo"), None);
    assert_eq!(a.registro().alias_instancia(&"g".repeat(32)), None);
    a.cerrar(PLAZO).unwrap();
    b.cerrar(PLAZO).unwrap();
}

#[test]
fn linea_que_supera_la_cota_no_desborda_el_archivo_y_se_notifica() {
    let t = Temporal::nueva();
    let config = Config {
        limite: 1024,
        ..Config::default()
    };
    let mut sesion = iniciar_con(&t.0, "prueba", "0", config).unwrap();
    let r = sesion.registro();
    let op = r.medir("prueba", "linea_grande");
    let etiqueta: &'static str = Box::leak("x".repeat(4096).into_boxed_str());
    op.dato(super::Campo::Etiqueta("muestra", etiqueta));
    op.terminar(Resultado::Exito);
    assert!(sesion.cerrar(PLAZO).is_err());
    if let Ok(meta) = fs::metadata(t.logs().join(ARCHIVO)) {
        assert!(meta.len() <= 1024);
    }
    assert!(r.estado().perdidos > 0);
}

#[test]
fn archivo_de_anexo_puede_revertir_una_escritura_parcial_en_windows() {
    use std::io::Write;
    let t = Temporal::nueva();
    fs::create_dir(t.logs()).unwrap();
    let ruta = t.logs().join(ARCHIVO);
    fs::write(&ruta, b"original\n").unwrap();
    let mut archivo = archivos::abrir_anexo(&ruta).unwrap();
    archivo.write_all(b"parcial").unwrap();
    let revertida = archivo.set_len(9);
    drop(archivo);
    assert!(
        revertida.is_ok(),
        "el descriptor necesita permiso para truncar: {revertida:?}"
    );
    assert_eq!(fs::read(ruta).unwrap(), b"original\n");
}

#[test]
fn eventos_aceptados_tras_el_control_de_cierre_no_desaparecen() {
    use super::escritor::Mensaje;
    let t = Temporal::nueva();
    let compuerta = Arc::new(Compuerta::default());
    let config = Config {
        compuerta: Some(compuerta.clone()),
        ..Config::default()
    };
    let mut sesion = iniciar_con(&t.0, "prueba", "0", config).unwrap();
    let r = sesion.registro();
    let (aviso, acuse) = std::sync::mpsc::sync_channel(1);
    // Reproduce determinísticamente el orden de la carrera entre un emisor y Cerrar.
    assert!(r.c.tx.try_send(Mensaje::Cerrar(aviso)).is_ok());
    r.medir("prueba", "tardio").terminar(Resultado::Exito);
    assert_eq!(r.estado().enviados, 2);
    compuerta.abrir();
    acuse.recv_timeout(PLAZO).unwrap().unwrap();
    let _ = sesion.cerrar(PLAZO);
    assert_eq!(
        r.estado().pendientes(),
        0,
        "todo aceptado debe estar escrito o contabilizado perdido"
    );
}
