// SPDX-License-Identifier: GPL-3.0-only
//! Regresiones del anexado, la validación de carpetas y el cierre con emisores
//! concurrentes. Solo usan carpetas temporales propias; las pruebas con enlaces
//! se omiten si el sistema no permite crearlos (Windows sin modo desarrollador).
use super::{
    archivos,
    escritor::{Compuerta, Config, Mensaje},
    iniciar_con,
    pruebas::{lineas, Temporal, PLAZO},
    Campo, Causa, ErrorRegistro, Resultado, ARCHIVO,
};
use std::{
    fs,
    io::Seek,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread,
    time::{Duration, Instant},
};

#[cfg(unix)]
fn enlazar(destino: &Path, enlace: &Path, _carpeta: bool) -> bool {
    std::os::unix::fs::symlink(destino, enlace).is_ok()
}

#[cfg(windows)]
fn enlazar(destino: &Path, enlace: &Path, carpeta: bool) -> bool {
    use std::os::windows::fs::{symlink_dir, symlink_file};
    if carpeta {
        symlink_dir(destino, enlace).is_ok()
    } else {
        symlink_file(destino, enlace).is_ok()
    }
}

fn logs_creada(t: &Temporal) -> std::path::PathBuf {
    let logs = t.logs();
    fs::create_dir(&logs).unwrap();
    logs
}

#[test]
fn anexar_recorta_un_fragmento_previo_sin_concatenarlo() {
    let t = Temporal::nueva();
    let ruta = logs_creada(&t).join(ARCHIVO);
    fs::write(&ruta, b"{\"a\":1}\n{\"frag").unwrap();
    archivos::anexar(&ruta, 14, b"{\"b\":2}\n").unwrap();
    assert_eq!(fs::read(&ruta).unwrap(), b"{\"a\":1}\n{\"b\":2}\n");

    fs::write(&ruta, b"{\"solo_fragmento").unwrap();
    archivos::anexar(&ruta, 16, b"{\"c\":3}\n").unwrap();
    assert_eq!(fs::read(&ruta).unwrap(), b"{\"c\":3}\n");
}

#[test]
fn anexar_no_escribe_si_el_archivo_crecio_fuera_del_bloqueo() {
    let t = Temporal::nueva();
    let ruta = logs_creada(&t).join(ARCHIVO);
    fs::write(&ruta, b"{}\n{}\n").unwrap();
    assert_eq!(
        archivos::anexar(&ruta, 3, b"{\"x\":1}\n"),
        Err(archivos::TAMANO_AJENO)
    );
    assert_eq!(fs::read(&ruta).unwrap(), b"{}\n{}\n");
}

#[test]
fn abrir_un_archivo_existente_no_lo_trunca_y_el_anexo_queda_al_final() {
    let t = Temporal::nueva();
    let logs = logs_creada(&t);
    let cerrojo = logs.join("diagnostico.lock");
    fs::write(&cerrojo, b"contenido").unwrap();
    drop(archivos::abrir_bloqueo(&cerrojo).unwrap());
    assert_eq!(fs::read(&cerrojo).unwrap(), b"contenido");

    let ruta = logs.join(ARCHIVO);
    fs::write(&ruta, b"{}\n").unwrap();
    let mut archivo = archivos::abrir_anexo(&ruta).unwrap();
    assert_eq!(archivo.stream_position().unwrap(), 3);
    drop(archivo);
    assert_eq!(fs::read(&ruta).unwrap(), b"{}\n");

    let nuevo = logs.join("nuevo.jsonl");
    drop(archivos::abrir_anexo(&nuevo).unwrap());
    assert_eq!(fs::metadata(&nuevo).unwrap().len(), 0);
}

#[test]
fn enlace_roto_en_el_registro_no_se_sigue_ni_crea_su_destino() {
    let t = Temporal::nueva();
    let logs = logs_creada(&t);
    let destino = t.0.join("fuera.jsonl");
    let enlace = logs.join(ARCHIVO);
    if !enlazar(&destino, &enlace, false) {
        return;
    }
    assert!(archivos::anexar(&enlace, 0, b"{}\n").is_err());
    assert!(archivos::abrir_bloqueo(&enlace).is_err());
    assert!(!destino.exists(), "no se crea el destino del enlace");
    assert!(fs::symlink_metadata(&enlace)
        .unwrap()
        .file_type()
        .is_symlink());
}

#[test]
fn carpeta_logs_enlazada_no_recibe_escrituras_ni_rotaciones() {
    let t = Temporal::nueva();
    let ajena = t.0.join("ajena");
    fs::create_dir(&ajena).unwrap();
    fs::write(ajena.join(ARCHIVO), b"ajeno\n").unwrap();
    let logs = t.logs();
    if !enlazar(&ajena, &logs, true) {
        return;
    }
    let base = logs.join(ARCHIVO);
    assert_eq!(
        archivos::anexar(&base, 6, b"{}\n"),
        Err(archivos::NO_REGULAR)
    );
    assert_eq!(archivos::rotar(&base, 3), Err(archivos::NO_REGULAR));
    assert!(archivos::abrir_bloqueo(&logs.join("diagnostico.lock")).is_err());
    assert_eq!(fs::read(ajena.join(ARCHIVO)).unwrap(), b"ajeno\n");
    assert_eq!(
        fs::read_dir(&ajena).unwrap().count(),
        1,
        "no se crea nada en la carpeta ajena"
    );
}

#[test]
fn sustituir_logs_por_un_enlace_tras_iniciar_no_redirige_el_registro() {
    let t = Temporal::nueva();
    let mut sesion = iniciar_con(&t.0, "prueba", "0", Config::default()).unwrap();
    let r = sesion.registro();
    r.medir("prueba", "antes").terminar(Resultado::Exito);
    r.vaciar(PLAZO).unwrap();
    let real = t.0.join("logs-real");
    let ajena = t.0.join("ajena");
    fs::create_dir(&ajena).unwrap();
    fs::write(ajena.join("propio.txt"), b"ajeno").unwrap();
    // Con el cerrojo abierto, Windows puede impedir renombrar la carpeta.
    if fs::rename(t.logs(), &real).is_err() {
        sesion.cerrar(PLAZO).unwrap();
        return;
    }
    if !enlazar(&ajena, &t.logs(), true) {
        let _ = fs::rename(&real, t.logs());
        let _ = sesion.cerrar(PLAZO);
        return;
    }
    r.medir("prueba", "despues").terminar(Resultado::Exito);
    assert_eq!(
        r.vaciar(PLAZO),
        Err(ErrorRegistro::Escritura(archivos::NO_REGULAR))
    );
    let _ = sesion.cerrar(PLAZO);
    assert_eq!(
        fs::read_dir(&ajena).unwrap().count(),
        1,
        "nada se escribe a través del enlace"
    );
    assert_eq!(fs::read(ajena.join("propio.txt")).unwrap(), b"ajeno");
    let estado = r.estado();
    assert_eq!(
        (estado.escritos, estado.perdidos, estado.pendientes()),
        (2, 2, 0)
    );
    let previas = lineas(&real.join(ARCHIVO));
    assert_eq!(previas.iter().filter(|l| l["accion"] == "antes").count(), 2);
}

#[test]
fn vaciado_y_eventos_detras_del_cierre_reciben_acuse_y_se_escriben() {
    let t = Temporal::nueva();
    let compuerta = Arc::new(Compuerta::default());
    let config = Config {
        compuerta: Some(compuerta.clone()),
        ..Config::default()
    };
    let mut sesion = iniciar_con(&t.0, "prueba", "0", config).unwrap();
    let r = sesion.registro();
    let (aviso_cierre, acuse_cierre) = mpsc::sync_channel(1);
    assert!(r.c.tx.try_send(Mensaje::Cerrar(aviso_cierre)).is_ok());
    r.medir("prueba", "tardio").terminar(Resultado::Exito);
    let (aviso_vaciado, acuse_vaciado) = mpsc::sync_channel(1);
    assert!(r.c.tx.try_send(Mensaje::Vaciar(aviso_vaciado)).is_ok());

    compuerta.abrir();
    assert_eq!(acuse_cierre.recv_timeout(PLAZO).unwrap(), Ok(()));
    assert_eq!(acuse_vaciado.recv_timeout(PLAZO).unwrap(), Ok(()));
    let estado = r.estado();
    assert!(estado.cerrado);
    assert_eq!(
        (estado.enviados, estado.escritos, estado.pendientes()),
        (2, 2, 0)
    );

    r.medir("prueba", "rechazado").terminar(Resultado::Exito);
    let estado = r.estado();
    assert_eq!(
        (estado.enviados, estado.descartados),
        (2, 2),
        "tras cerrar no se acepta nada"
    );
    assert_eq!(sesion.cerrar(PLAZO), Err(ErrorRegistro::Cerrado));
    let ls = lineas(&t.logs().join(ARCHIVO));
    assert_eq!(ls.iter().filter(|l| l["accion"] == "tardio").count(), 2);
}

#[test]
fn productores_concurrentes_y_cierre_no_dejan_eventos_pendientes() {
    let t = Temporal::nueva();
    let config = Config {
        capacidad: 64,
        limite: 64 * 1024 * 1024,
        ..Config::default()
    };
    let mut sesion = iniciar_con(&t.0, "prueba", "0", config).unwrap();
    let r = sesion.registro();
    let parar = Arc::new(AtomicBool::new(false));
    let hilos: Vec<_> = (0..4)
        .map(|_| {
            let (r, parar) = (r.clone(), parar.clone());
            thread::spawn(move || {
                for _ in 0..2000 {
                    if parar.load(Ordering::Relaxed) {
                        break;
                    }
                    r.medir("prueba", "concurrente").terminar(Resultado::Exito);
                }
            })
        })
        .collect();
    thread::sleep(Duration::from_millis(20));
    let inicio = Instant::now();
    let cierre = sesion.cerrar(PLAZO);
    assert!(inicio.elapsed() < PLAZO);
    parar.store(true, Ordering::Relaxed);
    for hilo in hilos {
        hilo.join().unwrap();
    }
    assert_eq!(cierre, Ok(()));

    let estado = r.estado();
    assert!(estado.cerrado);
    assert_eq!(
        estado.pendientes(),
        0,
        "todo lo aceptado está escrito o contado"
    );
    assert_eq!((estado.escritos, estado.perdidos), (estado.enviados, 0));
    let ls = lineas(&t.logs().join(ARCHIVO));
    let externas = ls.iter().filter(|l| l["evento"] != "descartes").count() as u64;
    assert_eq!(
        externas, estado.escritos,
        "el archivo coincide con los contadores"
    );
    for (i, linea) in ls.iter().enumerate() {
        assert_eq!(linea["seq"], i as u64);
    }
}

#[test]
fn linea_sobredimensionada_no_impide_escribir_las_validas_del_mismo_lote() {
    let t = Temporal::nueva();
    let compuerta = Arc::new(Compuerta::default());
    let limite = 8192;
    let config = Config {
        limite,
        compuerta: Some(compuerta.clone()),
        ..Config::default()
    };
    let mut sesion = iniciar_con(&t.0, "prueba", "0", config).unwrap();
    let r = sesion.registro();
    let op = r.medir("prueba", "grande");
    let etiqueta: &'static str = Box::leak("x".repeat(16 * 1024).into_boxed_str());
    op.dato(Campo::Etiqueta("muestra", etiqueta));
    op.terminar(Resultado::Exito);
    r.medir("prueba", "siguiente").terminar(Resultado::Exito);
    assert_eq!(r.estado().enviados, 5);

    compuerta.abrir();
    let grande = Causa::Validacion("linea_demasiado_grande");
    assert_eq!(r.vaciar(PLAZO), Err(ErrorRegistro::Escritura(grande)));
    let estado = r.estado();
    assert_eq!(
        (estado.escritos, estado.perdidos, estado.ultimo_error),
        (4, 1, Some(grande))
    );
    sesion.cerrar(PLAZO).unwrap();

    let base = t.logs().join(ARCHIVO);
    assert!(fs::metadata(&base).unwrap().len() <= limite);
    let ls = lineas(&base);
    assert_eq!(ls.iter().filter(|l| l["accion"] == "grande").count(), 2);
    assert_eq!(ls.iter().filter(|l| l["accion"] == "siguiente").count(), 2);
    let informe = ls
        .iter()
        .find(|l| l["evento"] == "descartes")
        .expect("informe de la pérdida");
    assert_eq!(informe["datos"]["perdidos"], 1);
}
