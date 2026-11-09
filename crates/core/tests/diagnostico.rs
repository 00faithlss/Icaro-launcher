// SPDX-License-Identifier: GPL-3.0-only
//! Diagnóstico persistente con raíces temporales propias, sin datos de usuario.
use icaro_core::diagnostico::{self, Campo, Causa, Resultado};
use std::{
    fs,
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

struct Temporal(PathBuf);
impl Temporal {
    fn nueva() -> Self {
        static SIGUIENTE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let raiz = std::env::temp_dir().join(format!(
            "icaro-diagnostico-{}-{}-{}",
            std::process::id(),
            SIGUIENTE.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&raiz).unwrap();
        Self(raiz)
    }
    fn lineas(&self) -> Vec<serde_json::Value> {
        fs::read_to_string(self.0.join("logs/diagnostico.jsonl"))
            .unwrap()
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }
}
impl Drop for Temporal {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn fallo_conserva_operacion_etapa_y_causa_sin_mensaje_privado() {
    let t = Temporal::nueva();
    let mut sesion = diagnostico::iniciar(&t.0, "prueba", "0").unwrap();
    let registro = sesion.registro();
    let op = registro.medir("compartir", "consultar");
    op.etapa("red");
    op.dato(Campo::Numero("revision", 3));
    op.terminar(Resultado::Error(Causa::de_io(
        &std::io::Error::from_raw_os_error(5),
    )));
    sesion.cerrar(Duration::from_secs(3)).unwrap();
    let lineas = t.lineas();
    assert!(lineas.len() >= 3);
    assert!(lineas
        .iter()
        .all(|l| l["operacion"] == lineas[0]["operacion"]));
    let fin = lineas.last().unwrap();
    assert_eq!(fin["etapa"], "red");
    assert_eq!(fin["resultado"], "error");
    assert_eq!(fin["causa"]["codigo_os"], 5);
    assert!(fin["duracion_ms"].is_u64());
    assert!(!fs::read_to_string(t.0.join("logs/diagnostico.jsonl"))
        .unwrap()
        .contains(t.0.to_str().unwrap()));
}

#[test]
fn cancelar_y_descartar_un_futuro_son_resultados_distintos() {
    let t = Temporal::nueva();
    let mut sesion = diagnostico::iniciar(&t.0, "prueba", "0").unwrap();
    let r = sesion.registro();
    r.medir("compartir", "cancelada")
        .terminar(Resultado::Cancelado);
    drop(r.medir("compartir", "abandonada"));
    sesion.cerrar(Duration::from_secs(3)).unwrap();
    let ls = t.lineas();
    assert!(ls.iter().any(|l| l["resultado"] == "cancelado"));
    assert!(ls.iter().any(|l| l["resultado"] == "abandonado"));
}

// ---- Pruebas ampliadas (raíces temporales propias; global solo en subproceso) ----

use icaro_core::diagnostico::{ErrorRegistro, Medicion};
use serde_json::Value;
use std::{
    collections::HashMap,
    process::Command,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    thread,
};

const PLAZO: Duration = Duration::from_secs(5);
const NO_REGULAR: Causa = Causa::Validacion("no_regular");
const HIJO_PANICO: &str = "ICARO_DIAGNOSTICO_HIJO_PANICO";

/// Carpeta temporal única aunque dos pruebas paralelas coincidan en el reloj.
fn temporal() -> Temporal {
    static CONTADOR: AtomicUsize = AtomicUsize::new(0);
    let n = CONTADOR.fetch_add(1, Ordering::Relaxed);
    let marca = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let raiz = std::env::temp_dir().join(format!(
        "icaro-diagnostico-{}-{marca}-{n}",
        std::process::id()
    ));
    fs::create_dir(&raiz).unwrap();
    Temporal(raiz)
}

#[test]
fn ocho_productores_conservan_json_correlacion_y_secuencia() {
    let t = temporal();
    let mut sesion = diagnostico::iniciar(&t.0, "prueba", "0").unwrap();
    let r = sesion.registro();
    // 8 × 20 × 3 = 480 eventos: caben en la cola aunque el escritor se retrase.
    let hilos: Vec<_> = (0..8)
        .map(|_| {
            let r = sesion.registro();
            thread::spawn(move || {
                for _ in 0..20 {
                    let op = r.medir("prueba", "concurrente");
                    op.etapa("trabajo");
                    op.terminar(Resultado::Exito);
                }
            })
        })
        .collect();
    for hilo in hilos {
        hilo.join().unwrap();
    }
    sesion.cerrar(PLAZO).unwrap();

    let ls = t.lineas();
    assert_eq!(ls.len(), 480);
    let claves = [
        "esquema",
        "fecha",
        "nivel",
        "sesion",
        "seq",
        "version",
        "compilacion",
        "componente",
        "accion",
        "operacion",
        "etapa",
        "evento",
        "resultado",
        "causa",
        "duracion_ms",
    ];
    let mut por_operacion: HashMap<u64, Vec<&Value>> = HashMap::new();
    for (i, l) in ls.iter().enumerate() {
        let objeto = l.as_object().unwrap();
        assert!(
            claves.iter().all(|c| objeto.contains_key(*c)),
            "faltan claves en {l}"
        );
        assert_eq!(l["seq"], i as u64);
        assert_eq!(l["esquema"], 1);
        assert_eq!(l["sesion"], ls[0]["sesion"]);
        assert_eq!(
            (l["version"].as_str(), l["compilacion"].as_str()),
            (Some("prueba"), Some("0"))
        );
        let fecha = l["fecha"].as_str().unwrap();
        assert!(
            fecha.len() == 24
                && &fecha[10..11] == "T"
                && &fecha[19..20] == "."
                && fecha.ends_with('Z')
        );
        por_operacion
            .entry(l["operacion"].as_u64().unwrap())
            .or_default()
            .push(l);
    }
    assert_eq!(por_operacion.len(), 160);
    for lineas in por_operacion.values() {
        let eventos: Vec<_> = lineas
            .iter()
            .map(|l| l["evento"].as_str().unwrap())
            .collect();
        assert_eq!(eventos, ["inicio", "etapa", "fin"]);
        let fin = lineas[2];
        assert_eq!(
            (fin["etapa"].as_str(), fin["resultado"].as_str()),
            (Some("trabajo"), Some("exito"))
        );
        assert!(fin["duracion_ms"].is_u64());
        assert_eq!(fin["ubicacion"]["archivo"], "diagnostico.rs");
    }
    let estado = r.estado();
    assert_eq!(
        (
            estado.enviados,
            estado.escritos,
            estado.descartados,
            estado.perdidos
        ),
        (480, 480, 0, 0)
    );
    assert!(estado.cerrado);
}

#[test]
fn iniciar_rechaza_destinos_no_regulares_sin_tocarlos() {
    let t = temporal();
    let destino = t.0.join("logs").join("diagnostico.jsonl");
    fs::create_dir_all(&destino).unwrap();
    fs::write(destino.join("ajeno.txt"), b"ajeno").unwrap();
    assert_eq!(
        diagnostico::iniciar(&t.0, "prueba", "0").unwrap_err(),
        ErrorRegistro::Archivo(NO_REGULAR)
    );
    assert_eq!(fs::read(destino.join("ajeno.txt")).unwrap(), b"ajeno");

    let t = temporal();
    fs::write(t.0.join("logs"), b"archivo").unwrap();
    assert!(matches!(
        diagnostico::iniciar(&t.0, "prueba", "0"),
        Err(ErrorRegistro::Archivo(_))
    ));
    assert_eq!(fs::read(t.0.join("logs")).unwrap(), b"archivo");
}

#[test]
fn fallo_de_escritura_se_refleja_en_la_salud_y_se_informa_al_recuperarse() {
    let t = temporal();
    let mut sesion = diagnostico::iniciar(&t.0, "prueba", "0").unwrap();
    let r = sesion.registro();
    let destino = t.0.join("logs").join("diagnostico.jsonl");
    fs::create_dir(&destino).unwrap();
    r.medir("prueba", "fallida").terminar(Resultado::Exito);
    assert_eq!(r.vaciar(PLAZO), Err(ErrorRegistro::Escritura(NO_REGULAR)));
    let estado = r.estado();
    assert!(estado.errores_escritura >= 1);
    assert_eq!((estado.escritos, estado.perdidos), (0, 2));
    assert_eq!(estado.ultimo_error, Some(NO_REGULAR));
    assert!(destino.is_dir(), "la entrada ajena no se modifica");

    fs::remove_dir(&destino).unwrap();
    r.medir("prueba", "recuperada").terminar(Resultado::Exito);
    r.vaciar(PLAZO).unwrap();
    sesion.cerrar(PLAZO).unwrap();
    let ls = t.lineas();
    let informe = ls
        .iter()
        .find(|l| l["evento"] == "descartes")
        .expect("informe de pérdidas");
    assert_eq!(informe["datos"]["perdidos_total"], 2);
    assert_eq!(ls.iter().filter(|l| l["accion"] == "recuperada").count(), 2);
    assert!(!ls.iter().any(|l| l["accion"] == "fallida"));
}

#[test]
fn instancia_se_registra_por_alias_y_nunca_por_su_id_o_texto() {
    let t = temporal();
    let id = "0123456789abcdef0123456789abcdef";
    let mut sesion = diagnostico::iniciar(&t.0, "prueba", "0").unwrap();
    let r = sesion.registro();
    let op = r.medir("compartir", "instalar");
    op.instancia(id);
    op.etapa("descargas");
    op.terminar(Resultado::Exito);
    let otra = r.medir("compartir", "instalar");
    otra.instancia("Mi Mundo Secreto");
    otra.terminar(Resultado::Cancelado);
    sesion.cerrar(PLAZO).unwrap();

    let texto = fs::read_to_string(t.0.join("logs/diagnostico.jsonl")).unwrap();
    assert!(!texto.to_ascii_lowercase().contains(id));
    assert!(!texto.contains("Mi Mundo Secreto"));
    let ls = t.lineas();
    let alias = ls
        .iter()
        .find(|l| l["evento"] == "fin" && l["resultado"] == "exito")
        .unwrap()["instancia"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(alias.len(), 16);
    assert!(alias.bytes().all(|b| b.is_ascii_hexdigit()));
    assert!(ls.iter().any(|l| l["datos"]["instancia"] == "invalida"));
}

#[test]
fn causa_de_servicio_solo_admite_codigos_conocidos() {
    assert_eq!(
        Causa::servicio(410, "invitation_expired"),
        Causa::Servicio {
            estado: 410,
            codigo: "invitation_expired"
        }
    );
    assert_eq!(
        Causa::servicio(400, "ABCD-1234 https://ejemplo.invalid/x"),
        Causa::Servicio {
            estado: 400,
            codigo: "desconocido"
        }
    );
}

#[test]
fn medicion_global_sin_registro_no_hace_nada_y_es_compartible() {
    fn compartible<T: Send + Sync>() {}
    compartible::<Medicion>();
    compartible::<diagnostico::Registro>();
    // Este binario de pruebas nunca instala un registro global.
    let medicion = Arc::new(Medicion::global("prueba", "sin_global"));
    let copia = medicion.clone();
    thread::spawn(move || {
        copia.etapa("hilo");
        copia.dato(Campo::CodigoSalida(None));
    })
    .join()
    .unwrap();
    medicion.terminar(Resultado::Exito);
}

#[test]
fn terminar_es_idempotente_y_lo_tardio_queda_contabilizado() {
    let t = temporal();
    let mut sesion = diagnostico::iniciar(&t.0, "prueba", "0").unwrap();
    let r = sesion.registro();
    let op = r.medir("prueba", "doble");
    op.terminar(Resultado::Exito);
    op.terminar(Resultado::Cancelado);
    op.etapa("tarde");
    drop(op);
    sesion.cerrar(PLAZO).unwrap();
    sesion.cerrar(PLAZO).unwrap();
    r.medir("prueba", "tardia").terminar(Resultado::Exito);

    let estado = r.estado();
    assert!(estado.cerrado);
    assert_eq!((estado.escritos, estado.descartados), (2, 2));
    let resultados: Vec<_> = t.lineas().iter().map(|l| l["resultado"].clone()).collect();
    assert_eq!(resultados, [Value::Null, Value::from("exito")]);
}

#[test]
fn panico_deja_evidencia_antes_del_hook_previo_que_aborta() {
    if let Some(raiz) = std::env::var_os(HIJO_PANICO) {
        hijo_que_aborta(PathBuf::from(raiz));
    }
    let t = temporal();
    let salida = Command::new(std::env::current_exe().unwrap())
        .args([
            "panico_deja_evidencia_antes_del_hook_previo_que_aborta",
            "--exact",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(HIJO_PANICO, &t.0)
        .output()
        .unwrap();
    assert!(!salida.status.success(), "el subproceso debe abortar");
    assert_eq!(
        fs::read_to_string(t.0.join("previo.txt")).unwrap(),
        "despues",
        "el hook previo debe ejecutarse después de la escritura de emergencia"
    );
    let texto = fs::read_to_string(t.0.join("logs/panico.jsonl")).unwrap();
    let ls: Vec<Value> = texto
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert_eq!(ls.len(), 1);
    let l = &ls[0];
    assert_eq!(
        (l["evento"].as_str(), l["nivel"].as_str()),
        (Some("panico"), Some("error"))
    );
    assert_eq!(l["causa"]["categoria"], "panico");
    assert_eq!(l["ubicacion"]["archivo"], "diagnostico.rs");
    assert!(l["ubicacion"]["linea"].is_u64());
    assert!(l["carga"].is_string());
    assert!(!texto.contains("carga-secreta"), "sin payload");
    assert!(!texto.contains(t.0.to_str().unwrap()), "sin rutas");
    assert!(
        !texto.contains("panico_deja_evidencia"),
        "sin nombre de hilo"
    );
    assert!(!t.0.join("logs/diagnostico.jsonl").exists() || t.lineas().is_empty());
}

/// Cuerpo del subproceso: el hook previo aborta de inmediato, como en release.
fn hijo_que_aborta(raiz: PathBuf) -> ! {
    let marca = raiz.join("previo.txt");
    let emergencia = raiz.join("logs").join("panico.jsonl");
    std::panic::set_hook(Box::new(move |_| {
        let orden = if emergencia.is_file() {
            "despues"
        } else {
            "antes"
        };
        let _ = fs::write(&marca, orden);
        std::process::abort();
    }));
    let sesion = diagnostico::iniciar(&raiz, "prueba", "0").unwrap();
    sesion.instalar_panico();
    panic!("carga-secreta {}", raiz.display());
}

#[test]
fn primera_instalacion_crea_la_raiz_de_logs_sin_datos_previos() {
    let t = temporal();
    let raiz = t.0.join("datos").join("icaro");
    let mut sesion = diagnostico::iniciar(&raiz, "prueba", "0").unwrap();
    sesion
        .registro()
        .medir("launcher", "inicio")
        .terminar(Resultado::Exito);
    sesion.cerrar(PLAZO).unwrap();
    assert!(raiz.join("logs/diagnostico.jsonl").is_file());
}
