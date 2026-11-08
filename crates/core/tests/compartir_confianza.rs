// SPDX-License-Identifier: GPL-3.0-only
//! Confianza persistente del invitado en la identidad del anfitrión.
//! Identidades en memoria y raíces temporales: no usa keyring, APPDATA ni cuentas.
use icaro_core::compartir::{
    confianza::{self, EstadoConfianza},
    identidad::{self, AlmacenClave, Identidad, Zeroizing},
    Error, Limites, Manifiesto, Origen,
};
// `comun` usa estos nombres mediante `super::*`.
#[allow(unused_imports)]
use sha2::{Digest, Sha512};
use std::{fs, sync::Mutex};
#[path = "compartir/comun.rs"]
mod comun;
use comun::Carpeta;

struct Memoria(Mutex<Option<Zeroizing<Vec<u8>>>>);
impl AlmacenClave for Memoria {
    fn leer(&self) -> Result<Option<Zeroizing<Vec<u8>>>, Error> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn guardar(&self, bytes: &[u8]) -> Result<(), Error> {
        *self.0.lock().unwrap() = Some(Zeroizing::new(bytes.to_vec()));
        Ok(())
    }
}
fn identidad() -> Identidad {
    Identidad::desde_almacen(&Memoria(Mutex::new(None))).unwrap()
}
fn origen(anfitrion: &Identidad, publicacion: &str) -> Origen {
    Origen {
        anfitrion: anfitrion.clave_publica(),
        publicacion: publicacion.into(),
    }
}
fn archivo(root: &Carpeta) -> std::path::PathBuf {
    root.0.join("compartir").join("confianza.json")
}
fn fecha(root: &Carpeta, origen: &Origen) -> u64 {
    match confianza::consultar(&root.0, origen).unwrap() {
        EstadoConfianza::Conocido { confirmado_en } => confirmado_en,
        otro => panic!("se esperaba Conocido: {otro:?}"),
    }
}
/// Primera clave de 64 hex que no es un punto Ed25519 válido.
fn clave_no_ed25519() -> String {
    (1u8..=64)
        .map(|i| {
            let mut bytes = [0u8; 32];
            bytes[0] = i;
            bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
        })
        .find(|clave| identidad::huella_publica(clave).is_err())
        .expect("alguna codificación debe quedar fuera de la curva")
}

#[test]
fn primera_confianza_se_persiste_y_reabrir_la_reconoce() {
    let root = Carpeta::nueva();
    let host = identidad();
    let o = origen(&host, &"ab".repeat(16));
    assert_eq!(
        confianza::consultar(&root.0, &o).unwrap(),
        EstadoConfianza::PrimeraVez
    );
    assert!(!archivo(&root).exists(), "consultar no debe escribir");
    confianza::confirmar(&root.0, &o, false).unwrap();
    assert!(archivo(&root).is_file());
    // Reabrir: cada consulta vuelve a leer el archivo desde disco.
    let confirmado = fecha(&root, &o);
    assert!(confirmado > 0);
    let texto = fs::read_to_string(archivo(&root)).unwrap();
    assert!(texto.contains(&o.anfitrion), "clave completa del anfitrión");
    assert!(texto.contains(&o.publicacion));
}

#[test]
fn conocido_no_reescribe_ni_cambia_la_fecha() {
    let root = Carpeta::nueva();
    let host = identidad();
    let o = origen(&host, &"ab".repeat(16));
    confianza::confirmar(&root.0, &o, false).unwrap();
    let antes = fs::read(archivo(&root)).unwrap();
    let fecha_antes = fecha(&root, &o);
    std::thread::sleep(std::time::Duration::from_millis(1100));
    confianza::confirmar(&root.0, &o, false).unwrap();
    confianza::confirmar(&root.0, &o, true).unwrap();
    assert_eq!(fs::read(archivo(&root)).unwrap(), antes);
    assert_eq!(fecha(&root, &o), fecha_antes);
}

#[test]
fn clave_distinta_se_detecta_y_se_rechaza_sin_modificar_el_archivo() {
    let root = Carpeta::nueva();
    let publicacion = "ab".repeat(16);
    let original = origen(&identidad(), &publicacion);
    let suplantada = origen(&identidad(), &publicacion);
    confianza::confirmar(&root.0, &original, false).unwrap();
    let confirmado = fecha(&root, &original);
    let antes = fs::read(archivo(&root)).unwrap();
    assert_eq!(
        confianza::consultar(&root.0, &suplantada).unwrap(),
        EstadoConfianza::Distinto {
            confirmado_en: confirmado
        }
    );
    assert!(confianza::confirmar(&root.0, &suplantada, false).is_err());
    assert_eq!(fs::read(archivo(&root)).unwrap(), antes);
    assert_eq!(fecha(&root, &original), confirmado);
}

#[test]
fn cambio_confirmado_reemplaza_la_clave_y_conserva_otras_publicaciones() {
    let root = Carpeta::nueva();
    let otra = origen(&identidad(), &"cd".repeat(16));
    let original = origen(&identidad(), &"ab".repeat(16));
    let nueva = origen(&identidad(), &"ab".repeat(16));
    confianza::confirmar(&root.0, &otra, false).unwrap();
    confianza::confirmar(&root.0, &original, false).unwrap();
    let fecha_otra = fecha(&root, &otra);
    confianza::confirmar(&root.0, &nueva, true).unwrap();
    fecha(&root, &nueva);
    assert!(matches!(
        confianza::consultar(&root.0, &original).unwrap(),
        EstadoConfianza::Distinto { .. }
    ));
    assert_eq!(fecha(&root, &otra), fecha_otra);
}

#[test]
fn otra_clave_con_igual_nombre_no_hereda_la_confianza() {
    let root = Carpeta::nueva();
    // Mismo nombre visible de instancia/anfitrión; la confianza solo depende de
    // la publicación y de la clave completa.
    let (base, _) = comun::origen_y_perfil();
    let confiado = Origen {
        anfitrion: identidad().clave_publica(),
        ..base.clone()
    };
    confianza::confirmar(&root.0, &confiado, false).unwrap();
    let otro_equipo = identidad();
    let otra_publicacion = origen(&otro_equipo, &"ef".repeat(16));
    assert_eq!(
        confianza::consultar(&root.0, &otra_publicacion).unwrap(),
        EstadoConfianza::PrimeraVez
    );
    let misma_publicacion = origen(&otro_equipo, &base.publicacion);
    assert!(matches!(
        confianza::consultar(&root.0, &misma_publicacion).unwrap(),
        EstadoConfianza::Distinto { .. }
    ));
}

#[test]
fn confianza_corrupta_es_error_y_no_se_pierde_el_archivo() {
    let host = identidad();
    let o = origen(&host, &"ab".repeat(16));
    let valido = |extra: &str| {
        format!(
            r#"{{"esquema":1,"registros":[{{"publicacion":"{}","anfitrion":"{}","confirmado_en":5{extra}}}]}}"#,
            o.publicacion, o.anfitrion
        )
    };
    let duplicado = format!(
        r#"{{"esquema":1,"registros":[{{"publicacion":"{p}","anfitrion":"{a}","confirmado_en":5}},{{"publicacion":"{p}","anfitrion":"{a}","confirmado_en":6}}]}}"#,
        p = o.publicacion,
        a = o.anfitrion
    );
    let clave_mal = format!(
        r#"{{"esquema":1,"registros":[{{"publicacion":"{}","anfitrion":"{}","confirmado_en":5}}]}}"#,
        o.publicacion,
        o.anfitrion.to_uppercase()
    );
    let casos: Vec<Vec<u8>> = vec![
        b"{no es json".to_vec(),
        b"".to_vec(),
        valido(r#","futuro":true"#).into_bytes(),
        duplicado.into_bytes(),
        clave_mal.into_bytes(),
        br#"{"esquema":2,"registros":[]}"#.to_vec(),
    ];
    for bytes in casos {
        let root = Carpeta::nueva();
        root.escribir("compartir/confianza.json", &bytes);
        assert!(
            confianza::consultar(&root.0, &o).is_err(),
            "no es PrimeraVez"
        );
        assert!(confianza::confirmar(&root.0, &o, false).is_err());
        assert!(confianza::confirmar(&root.0, &o, true).is_err());
        assert_eq!(fs::read(archivo(&root)).unwrap(), bytes);
    }
    // Control: el mismo formato sin campos extra sí se acepta.
    let root = Carpeta::nueva();
    root.escribir("compartir/confianza.json", valido("").as_bytes());
    assert_eq!(fecha(&root, &o), 5);
}

#[test]
fn confianza_rechaza_tamano_excesivo_demasiados_registros_y_no_archivos() {
    let host = identidad();
    let o = origen(&host, &"ab".repeat(16));
    let root = Carpeta::nueva();
    let grande = vec![b' '; 256 * 1024 + 1];
    root.escribir("compartir/confianza.json", &grande);
    assert!(matches!(
        confianza::consultar(&root.0, &o),
        Err(Error::Limite(_))
    ));
    assert!(confianza::confirmar(&root.0, &o, false).is_err());
    assert_eq!(fs::read(archivo(&root)).unwrap(), grande);

    let root = Carpeta::nueva();
    let registros: Vec<String> = (0..1001u32)
        .map(|i| {
            format!(
                r#"{{"publicacion":"{i:032x}","anfitrion":"{}","confirmado_en":1}}"#,
                o.anfitrion
            )
        })
        .collect();
    let muchos = format!(r#"{{"esquema":1,"registros":[{}]}}"#, registros.join(","));
    assert!(muchos.len() <= 256 * 1024);
    root.escribir("compartir/confianza.json", muchos.as_bytes());
    assert!(confianza::consultar(&root.0, &o).is_err());
    assert_eq!(fs::read(archivo(&root)).unwrap(), muchos.as_bytes());

    let root = Carpeta::nueva();
    fs::create_dir_all(archivo(&root)).unwrap();
    assert!(confianza::consultar(&root.0, &o).is_err());
    assert!(confianza::confirmar(&root.0, &o, false).is_err());
    assert!(archivo(&root).is_dir());
}

#[test]
fn confirmar_no_supera_el_limite_de_registros() {
    let host = identidad();
    let root = Carpeta::nueva();
    let registros: Vec<String> = (0..1000u32)
        .map(|i| {
            format!(
                r#"{{"publicacion":"{i:032x}","anfitrion":"{}","confirmado_en":1}}"#,
                host.clave_publica()
            )
        })
        .collect();
    let lleno = format!(r#"{{"esquema":1,"registros":[{}]}}"#, registros.join(","));
    root.escribir("compartir/confianza.json", lleno.as_bytes());
    let nueva = origen(&host, &"ff".repeat(16));
    assert_eq!(
        confianza::consultar(&root.0, &nueva).unwrap(),
        EstadoConfianza::PrimeraVez
    );
    assert!(confianza::confirmar(&root.0, &nueva, false).is_err());
    assert_eq!(fs::read(archivo(&root)).unwrap(), lleno.as_bytes());
    // Las publicaciones ya confirmadas siguen siendo consultables.
    fecha(&root, &origen(&host, &format!("{:032x}", 7)));
}

#[test]
fn huella_y_sello_publicos_coinciden_con_la_identidad() {
    for _ in 0..4 {
        let id = identidad();
        let publica = id.clave_publica();
        assert_eq!(identidad::huella_publica(&publica).unwrap(), id.huella());
        assert_eq!(identidad::sello_publico(&publica).unwrap(), id.sello());
    }
}

#[test]
fn clave_invalida_se_rechaza_en_huella_sello_y_confianza() {
    let root = Carpeta::nueva();
    let valida = identidad().clave_publica();
    let invalidas = [
        String::new(),
        valida[..62].to_owned(),
        format!("{valida}00"),
        valida.to_uppercase(),
        "zz".repeat(32),
        format!(" {}", &valida[1..]),
        clave_no_ed25519(),
    ];
    for clave in invalidas {
        assert!(identidad::huella_publica(&clave).is_err(), "{clave}");
        assert!(identidad::sello_publico(&clave).is_err(), "{clave}");
        let o = Origen {
            anfitrion: clave.clone(),
            publicacion: "ab".repeat(16),
        };
        assert!(confianza::consultar(&root.0, &o).is_err(), "{clave}");
        assert!(confianza::confirmar(&root.0, &o, true).is_err(), "{clave}");
    }
    let mala_publicacion = Origen {
        anfitrion: valida,
        publicacion: "../x".into(),
    };
    assert!(confianza::consultar(&root.0, &mala_publicacion).is_err());
    assert!(confianza::confirmar(&root.0, &mala_publicacion, false).is_err());
    assert!(!archivo(&root).exists());
}

#[test]
fn raiz_inexistente_o_archivo_no_se_crea_implicitamente() {
    let root = Carpeta::nueva();
    let o = origen(&identidad(), &"ab".repeat(16));
    let falta = root.0.join("no-existe");
    assert!(confianza::confirmar(&falta, &o, false).is_err());
    assert!(!falta.exists());
    root.escribir("archivo", b"x");
    assert!(confianza::confirmar(&root.0.join("archivo"), &o, false).is_err());
    assert_eq!(fs::read(root.0.join("archivo")).unwrap(), b"x");
}

#[test]
fn bloqueo_de_otro_proceso_no_pierde_ni_sobrescribe_confianza() {
    let root = Carpeta::nueva();
    let o = origen(&identidad(), &"ab".repeat(16));
    confianza::confirmar(&root.0, &o, false).unwrap();
    let antes = fs::read(archivo(&root)).unwrap();
    let bloqueo = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(root.0.join("compartir/confianza.lock"))
        .unwrap();
    bloqueo.try_lock().unwrap();
    let otra = origen(&identidad(), &"cd".repeat(16));
    assert!(confianza::confirmar(&root.0, &otra, false).is_err());
    assert_eq!(fs::read(archivo(&root)).unwrap(), antes);
}
