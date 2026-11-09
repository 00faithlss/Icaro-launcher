// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Pruebas de la guardia de actividad de instancias. Solo usan carpetas
//! temporales propias; nunca `APPDATA` ni datos reales del usuario.

use std::fmt::Debug;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use icaro_core::actividad_instancia::{Error, Guardia, ARCHIVO};

const VARIABLE_AYUDANTE: &str = "ICARO_PRUEBA_ACTIVIDAD_DIR";
const ESPERA_MAXIMA: Duration = Duration::from_secs(9);

/// Carpeta temporal única que se borra al terminar la prueba.
struct Temporal(PathBuf);

impl Temporal {
    fn nueva(nombre: &str) -> Self {
        static CONTADOR: AtomicU32 = AtomicU32::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let ruta = std::env::temp_dir().join(format!(
            "icaro-actividad-{}-{}-{}-{}",
            std::process::id(),
            nombre,
            nanos,
            CONTADOR.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&ruta).unwrap();
        Temporal(ruta)
    }

    fn juego(&self, nombre: &str) -> PathBuf {
        let juego = self.0.join(nombre);
        fs::create_dir_all(&juego).unwrap();
        juego
    }
}

impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn ocupada(resultado: Result<Guardia, Error>) -> bool {
    matches!(resultado, Err(Error::Ocupada))
}

#[test]
fn guardia_es_debug_send_y_sync() {
    fn afirmar<T: Debug + Send + Sync>() {}
    afirmar::<Guardia>();
    afirmar::<Arc<Guardia>>();
    afirmar::<Error>();
}

#[test]
fn renombrar_instancia_conserva_la_reserva_por_id() {
    let temp = Temporal::nueva("renombrado");
    let raiz = temp.0.join("instances");
    let store = icaro_core::instancias::Almacen::nuevo(&raiz);
    let mut instancia = store.crear("Original", "1.21.1", 0).unwrap();
    let guardia = Guardia::adquirir_instancia(&store, &instancia.id).unwrap();
    instancia.nombre = "Recuperada".into();
    store.guardar(&instancia).unwrap();
    let recargado = icaro_core::instancias::Almacen::nuevo(&raiz);
    assert_eq!(
        guardia.ruta(),
        Guardia::ruta_instancia(&recargado, &instancia.id).unwrap()
    );
    assert!(ocupada(Guardia::adquirir_instancia(
        &recargado,
        &instancia.id
    )));
    drop(guardia);
    drop(Guardia::adquirir_instancia(&recargado, &instancia.id).unwrap());
}

#[test]
fn crea_el_archivo_de_bloqueo_si_falta() {
    let tmp = Temporal::nueva("crea");
    let juego = tmp.juego("juego");
    let guardia = Guardia::adquirir(&juego).unwrap();
    assert_eq!(guardia.ruta(), juego.join(ARCHIVO));
    assert!(fs::symlink_metadata(juego.join(ARCHIVO)).unwrap().is_file());
}

#[test]
fn segundo_acceso_queda_ocupado() {
    let tmp = Temporal::nueva("segundo");
    let juego = tmp.juego("juego");
    let _guardia = Guardia::adquirir(&juego).unwrap();
    assert!(ocupada(Guardia::adquirir(&juego)));
}

#[test]
fn otro_juego_es_independiente() {
    let tmp = Temporal::nueva("independiente");
    let uno = tmp.juego("uno");
    let dos = tmp.juego("dos");
    let _a = Guardia::adquirir(&uno).unwrap();
    let _b = Guardia::adquirir(&dos).unwrap();
}

#[test]
fn clones_arc_mantienen_el_bloqueo_hasta_el_ultimo() {
    let tmp = Temporal::nueva("arc");
    let juego = tmp.juego("juego");
    let guardia = Arc::new(Guardia::adquirir(&juego).unwrap());
    let clon = Arc::clone(&guardia);
    let en_hilo = Arc::clone(&guardia);
    drop(guardia);
    assert!(ocupada(Guardia::adquirir(&juego)));
    std::thread::spawn(move || drop(en_hilo)).join().unwrap();
    assert!(ocupada(Guardia::adquirir(&juego)));
    drop(clon);
    let _de_nuevo = Guardia::adquirir(&juego).unwrap();
}

#[test]
fn soltar_permite_adquirir_otra_vez() {
    let tmp = Temporal::nueva("soltar");
    let juego = tmp.juego("juego");
    drop(Guardia::adquirir(&juego).unwrap());
    let _otra = Guardia::adquirir(&juego).unwrap();
}

#[test]
fn conserva_el_contenido_y_el_archivo_existente() {
    let tmp = Temporal::nueva("conserva");
    let juego = tmp.juego("juego");
    fs::write(juego.join(ARCHIVO), b"contenido previo").unwrap();
    drop(Guardia::adquirir(&juego).unwrap());
    assert_eq!(fs::read(juego.join(ARCHIVO)).unwrap(), b"contenido previo");
}

#[test]
fn rechaza_juego_que_es_archivo() {
    let tmp = Temporal::nueva("juego-archivo");
    let juego = tmp.0.join("juego");
    fs::write(&juego, b"no soy carpeta").unwrap();
    assert!(matches!(
        Guardia::adquirir(&juego),
        Err(Error::NoRegular { .. })
    ));
    assert_eq!(fs::read(&juego).unwrap(), b"no soy carpeta");
}

#[test]
fn no_crea_un_juego_inexistente() {
    let tmp = Temporal::nueva("inexistente");
    let juego = tmp.0.join("falta");
    assert!(matches!(
        Guardia::adquirir(&juego),
        Err(Error::Disco { .. })
    ));
    assert!(!juego.exists());
}

#[test]
fn rechaza_bloqueo_que_es_directorio() {
    let tmp = Temporal::nueva("lock-dir");
    let juego = tmp.juego("juego");
    fs::create_dir(juego.join(ARCHIVO)).unwrap();
    assert!(matches!(
        Guardia::adquirir(&juego),
        Err(Error::NoRegular { .. })
    ));
    assert!(juego.join(ARCHIVO).is_dir());
}

#[test]
fn rechaza_enlaces_simbolicos() {
    let tmp = Temporal::nueva("enlaces");
    let real = tmp.juego("real");
    let destino = tmp.0.join("destino.lock");
    fs::write(&destino, b"x").unwrap();
    let juego = tmp.juego("juego");

    // Bloqueo que es un enlace a un archivo regular.
    if !crear_enlace(&destino, &juego.join(ARCHIVO), false) {
        eprintln!("sin permiso para crear enlaces simbólicos; se omite la prueba");
        return;
    }
    assert!(matches!(
        Guardia::adquirir(&juego),
        Err(Error::NoRegular { .. })
    ));
    assert_eq!(fs::read(&destino).unwrap(), b"x");

    // Carpeta de juego que es un enlace a una carpeta real.
    let enlace = tmp.0.join("juego-enlace");
    assert!(crear_enlace(&real, &enlace, true));
    assert!(matches!(
        Guardia::adquirir(&enlace),
        Err(Error::NoRegular { .. })
    ));
    assert!(!real.join(ARCHIVO).exists());
}

#[cfg(unix)]
fn crear_enlace(destino: &Path, enlace: &Path, _carpeta: bool) -> bool {
    std::os::unix::fs::symlink(destino, enlace).is_ok()
}

#[cfg(windows)]
fn crear_enlace(destino: &Path, enlace: &Path, carpeta: bool) -> bool {
    use std::os::windows::fs::{symlink_dir, symlink_file};
    if carpeta {
        symlink_dir(destino, enlace).is_ok()
    } else {
        symlink_file(destino, enlace).is_ok()
    }
}

// --- Conflicto entre procesos -------------------------------------------

/// Proceso hijo creado por la prueba; se mata y espera al soltarlo para no
/// dejarlo vivo si la prueba falla. Solo actúa sobre el hijo que creó.
struct Hijo(Child);

impl Drop for Hijo {
    fn drop(&mut self) {
        if let Ok(None) = self.0.try_wait() {
            let _ = self.0.kill();
        }
        let _ = self.0.wait();
    }
}

fn lanzar_ayudante(senales: &Path) -> Hijo {
    let exe = std::env::current_exe().unwrap();
    let hijo = Command::new(exe)
        .args([
            "ayudante_subproceso_mantiene_guardia",
            "--exact",
            "--ignored",
            "--test-threads=1",
            "--nocapture",
        ])
        .env(VARIABLE_AYUDANTE, senales)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let hijo = Hijo(hijo);
    let listo = senales.join("READY");
    let inicio = Instant::now();
    while !listo.exists() {
        assert!(
            inicio.elapsed() < ESPERA_MAXIMA,
            "el ayudante no avisó READY"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    hijo
}

fn esperar_salida(hijo: &mut Hijo) {
    let inicio = Instant::now();
    while hijo.0.try_wait().unwrap().is_none() {
        assert!(inicio.elapsed() < ESPERA_MAXIMA, "el ayudante no terminó");
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// El sistema puede tardar un instante en soltar el bloqueo de un proceso
/// muerto: reintentar con un límite acotado.
fn adquirir_con_espera(juego: &Path) -> Guardia {
    let inicio = Instant::now();
    loop {
        match Guardia::adquirir(juego) {
            Ok(guardia) => return guardia,
            Err(Error::Ocupada) if inicio.elapsed() < ESPERA_MAXIMA => {
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(e) => panic!("no se liberó el bloqueo: {e}"),
        }
    }
}

#[test]
fn otro_proceso_ocupa_y_matarlo_libera() {
    let tmp = Temporal::nueva("proceso-matar");
    let juego = tmp.juego("juego");
    let mut hijo = lanzar_ayudante(&tmp.0);
    assert!(ocupada(Guardia::adquirir(&juego)));
    hijo.0.kill().unwrap();
    esperar_salida(&mut hijo);
    let _guardia = adquirir_con_espera(&juego);
}

#[test]
fn otro_proceso_ocupa_y_terminar_libera() {
    let tmp = Temporal::nueva("proceso-terminar");
    let juego = tmp.juego("juego");
    let mut hijo = lanzar_ayudante(&tmp.0);
    assert!(ocupada(Guardia::adquirir(&juego)));
    fs::write(tmp.0.join("STOP"), b"").unwrap();
    esperar_salida(&mut hijo);
    let _guardia = adquirir_con_espera(&juego);
}

/// Ayudante para las pruebas entre procesos: no hace nada si se ejecuta
/// directamente. Adquiere la guardia de `<dir>/juego`, avisa con `READY` y
/// sale con `process::exit` sin soltarla (al ver `STOP` o tras un límite),
/// para comprobar que es el sistema quien libera el bloqueo.
#[test]
#[ignore]
fn ayudante_subproceso_mantiene_guardia() {
    let Some(senales) = std::env::var_os(VARIABLE_AYUDANTE).map(PathBuf::from) else {
        return;
    };
    let guardia = Guardia::adquirir(&senales.join("juego")).unwrap();
    fs::write(senales.join("READY"), b"").unwrap();
    let inicio = Instant::now();
    while !senales.join("STOP").exists() && inicio.elapsed() < Duration::from_secs(20) {
        std::thread::sleep(Duration::from_millis(25));
    }
    std::mem::forget(guardia);
    std::process::exit(0);
}
