// SPDX-License-Identifier: GPL-3.0-only
//! Recuperación de instalaciones compartidas interrumpidas tras reiniciar el almacén.
use icaro_core::{
    compartir::{
        aplicacion::{self as tx, ContextoDestino, FuenteArchivos},
        intentos, inventario,
        plan::planificar,
        vinculos, Artefacto, Error, Limites, Manifiesto, Procedencia, RutaCompartida,
    },
    instancias::{Almacen, Instancia},
};
use sha2::{Digest, Sha512};
use std::{fs, io::Read, path::PathBuf};
use tokio_util::sync::CancellationToken;
#[path = "compartir/comun.rs"]
mod comun;
use comun::Carpeta;

const MARCADOR: &str = ".icaro-compartir-instalacion.json";

fn remoto(revision: u64, archivos: &[(&str, &[u8])]) -> Manifiesto {
    let (origen, perfil) = comun::origen_y_perfil();
    Manifiesto {
        esquema: "icaro-compartir/1".into(),
        origen,
        perfil,
        revision,
        archivos: archivos
            .iter()
            .map(|(ruta, contenido)| Artefacto {
                ruta: RutaCompartida::parse(ruta).unwrap(),
                bytes: contenido.len() as u64,
                sha512: format!("{:x}", Sha512::digest(contenido)),
                procedencia: Procedencia::Propio,
            })
            .collect(),
        packs: vec![],
    }
}
fn otra_publicacion(m: &Manifiesto) -> Manifiesto {
    let mut otro = m.clone();
    otro.origen.publicacion = "ef".repeat(16);
    otro
}
fn otro_perfil(m: &Manifiesto) -> Manifiesto {
    let mut otro = m.clone();
    otro.perfil.loader_version = Some("0.16.9".into());
    otro
}
fn nueva(store: &Almacen, nombre: &str, m: &Manifiesto) -> Instancia {
    store
        .crear_con_loader(nombre, &m.perfil.minecraft, &m.perfil.loader, 0)
        .unwrap()
}
fn marcador(store: &Almacen, i: &Instancia) -> PathBuf {
    store.juego(&i.id).unwrap().join(MARCADOR)
}

struct FuenteLocal(PathBuf);
impl FuenteArchivos for FuenteLocal {
    fn abrir(&self, archivo: &Artefacto) -> Result<Box<dyn Read + Send>, Error> {
        Ok(Box::new(
            fs::File::open(self.0.join(archivo.ruta.as_str())).map_err(|_| Error::Disco)?,
        ))
    }
}
fn aplicar_base(store: &Almacen, i: &Instancia, m: &Manifiesto, fuente: &Carpeta) {
    let ctx = ContextoDestino {
        juego: store.juego(&i.id).unwrap(),
        limites: Limites::default(),
    };
    let cancel = CancellationToken::new();
    let inv = inventario::analizar(&ctx.juego, &cancel, &ctx.limites).unwrap();
    let plan = planificar(m, &tx::cargar_base(&ctx).unwrap(), &inv).unwrap();
    let preparada = tx::preparar(&ctx, plan, &FuenteLocal(fuente.0.clone()), &cancel).unwrap();
    tx::aplicar(preparada, &cancel, |_| {}).unwrap();
}

#[test]
fn registro_sobrevive_al_reinicio_y_reanuda_el_mismo_destino_sin_duplicar() {
    let root = Carpeta::nueva();
    let raiz = root.0.join("instancias");
    let m = remoto(1, &[]);
    let id = {
        let store = Almacen::nuevo(&raiz);
        let i = nueva(&store, "Recibida", &m);
        intentos::registrar(&store, &i, &m).unwrap();
        // Simula un fallo al preparar el perfil: no hay vínculo ni base.
        i.id
    };
    let store = Almacen::nuevo(&raiz);
    let encontrada = intentos::buscar(&store, &m).unwrap().unwrap();
    assert_eq!(encontrada.id, id);
    assert_eq!(store.cargar().unwrap().instancias.len(), 1);
    // Una revisión posterior del mismo origen y perfil vuelve a revisar ese destino.
    let siguiente = remoto(2, &[]);
    assert_eq!(
        intentos::buscar(&store, &siguiente).unwrap().unwrap().id,
        id
    );
    intentos::registrar(&store, &encontrada, &siguiente).unwrap();
    let texto = fs::read_to_string(marcador(&store, &encontrada)).unwrap();
    assert!(texto.contains(&id) && texto.contains("\"revision\": 2"));
    assert!(!texto.contains("http") && !texto.contains("codigo"));
    assert!(intentos::registrar(&store, &encontrada, &m).is_err());
}

#[test]
fn registro_previo_al_vinculo_permite_completar_solo_el_destino_propio() {
    let root = Carpeta::nueva();
    let store = Almacen::nuevo(root.0.join("instances"));
    let m = remoto(1, &[]);
    let instancia = nueva(&store, "Pendiente", &m);
    intentos::registrar(&store, &instancia, &m).unwrap();
    let temporal = store
        .juego(&instancia.id)
        .unwrap()
        .join(".icaro-state-123.tmp");
    fs::write(&temporal, b"preparacion interrumpida").unwrap();
    vinculos::preparar_destino(&store, &instancia, &m, &Limites::default()).unwrap();
    assert_eq!(fs::read(temporal).unwrap(), b"preparacion interrumpida");
    assert_eq!(
        vinculos::leer(&store.juego(&instancia.id).unwrap())
            .unwrap()
            .unwrap()
            .origen,
        m.origen
    );
}

#[test]
fn otro_origen_o_perfil_no_hereda_el_intento() {
    let root = Carpeta::nueva();
    let store = Almacen::nuevo(root.0.join("instancias"));
    let m = remoto(1, &[]);
    let i = nueva(&store, "Recibida", &m);
    intentos::registrar(&store, &i, &m).unwrap();
    let antes = fs::read(marcador(&store, &i)).unwrap();
    assert!(intentos::buscar(&store, &otra_publicacion(&m))
        .unwrap()
        .is_none());
    assert!(intentos::buscar(&store, &otro_perfil(&m))
        .unwrap()
        .is_none());
    // Registrar otra publicación sobre el mismo destino no pisa el marcador.
    assert!(intentos::registrar(&store, &i, &otra_publicacion(&m)).is_err());
    assert!(intentos::registrar(&store, &i, &otro_perfil(&m)).is_err());
    assert_eq!(fs::read(marcador(&store, &i)).unwrap(), antes);
}

#[test]
fn instancia_personal_con_el_mismo_nombre_nunca_se_adopta() {
    let root = Carpeta::nueva();
    let store = Almacen::nuevo(root.0.join("instancias"));
    let m = remoto(1, &[]);
    let personal = nueva(&store, "Recibida", &m);
    let juego = store.juego(&personal.id).unwrap();
    fs::create_dir_all(juego.join("saves/mundo")).unwrap();
    fs::write(juego.join("saves/mundo/level.dat"), b"mundo").unwrap();
    assert!(intentos::buscar(&store, &m).unwrap().is_none());
    assert!(intentos::registrar(&store, &personal, &m).is_err());
    assert!(!juego.join(MARCADOR).exists());
    assert_eq!(
        fs::read(juego.join("saves/mundo/level.dat")).unwrap(),
        b"mundo"
    );
}

#[test]
fn marcadores_corruptos_o_ajenos_son_errores_sin_modificar_nada() {
    let m = remoto(1, &[]);
    let casos: Vec<(&str, Box<dyn Fn(&Instancia) -> Vec<u8>>)> = vec![
        ("json", Box::new(|_: &Instancia| b"{no es json".to_vec())),
        (
            "tamano",
            Box::new(|_: &Instancia| vec![b' '; 16 * 1024 + 1]),
        ),
        (
            "esquema",
            Box::new(|i: &Instancia| marcador_json(2, &i.id, &remoto(1, &[]), None)),
        ),
        (
            "id",
            Box::new(|_: &Instancia| marcador_json(1, &"0f".repeat(16), &remoto(1, &[]), None)),
        ),
        (
            "campo",
            Box::new(|i: &Instancia| marcador_json(1, &i.id, &remoto(1, &[]), Some("codigo"))),
        ),
    ];
    for (caso, contenido) in casos {
        let root = Carpeta::nueva();
        let store = Almacen::nuevo(root.0.join("instancias"));
        let i = nueva(&store, "Recibida", &m);
        let ruta = marcador(&store, &i);
        let bytes = contenido(&i);
        fs::write(&ruta, &bytes).unwrap();
        assert!(intentos::buscar(&store, &m).is_err(), "buscar: {caso}");
        assert!(
            intentos::registrar(&store, &i, &m).is_err(),
            "registrar: {caso}"
        );
        assert!(
            intentos::completar(&store, &i, &m).is_err(),
            "completar: {caso}"
        );
        assert_eq!(fs::read(&ruta).unwrap(), bytes, "{caso}");
        assert_eq!(store.cargar().unwrap().instancias.len(), 1, "{caso}");
    }
    // Un marcador que es carpeta tampoco se trata como ausente.
    let root = Carpeta::nueva();
    let store = Almacen::nuevo(root.0.join("instancias"));
    let i = nueva(&store, "Recibida", &m);
    fs::create_dir(marcador(&store, &i)).unwrap();
    assert!(intentos::buscar(&store, &m).is_err());
    assert!(marcador(&store, &i).is_dir());
}
fn marcador_json(esquema: u32, id: &str, m: &Manifiesto, extra: Option<&str>) -> Vec<u8> {
    let mut valor = serde_json::json!({
        "esquema": esquema,
        "instancia_id": id,
        "origen": m.origen,
        "perfil": m.perfil,
        "revision": m.revision,
    });
    if let Some(campo) = extra {
        valor[campo] = "secreto".into();
    }
    serde_json::to_vec(&valor).unwrap()
}

#[test]
fn manifiesto_invalido_se_rechaza_sin_escribir() {
    let root = Carpeta::nueva();
    let store = Almacen::nuevo(root.0.join("instancias"));
    let m = remoto(1, &[]);
    let i = nueva(&store, "Recibida", &m);
    let mut invalido = m.clone();
    invalido.esquema = "icaro-compartir/9".into();
    assert!(intentos::registrar(&store, &i, &invalido).is_err());
    assert!(intentos::buscar(&store, &invalido).is_err());
    assert!(!marcador(&store, &i).exists());
}

#[test]
fn completar_sin_base_aplicada_conserva_el_marcador() {
    let root = Carpeta::nueva();
    let store = Almacen::nuevo(root.0.join("instancias"));
    let m = remoto(1, &[]);
    let i = nueva(&store, "Recibida", &m);
    vinculos::preparar_destino(&store, &i, &m, &Limites::default()).unwrap();
    intentos::registrar(&store, &i, &m).unwrap();
    assert!(intentos::completar(&store, &i, &m).is_err());
    assert!(marcador(&store, &i).exists());
    assert_eq!(intentos::buscar(&store, &m).unwrap().unwrap().id, i.id);
}

#[test]
fn base_real_permite_limpiar_solo_el_marcador_y_conserva_mundo_y_opciones() {
    let root = Carpeta::nueva();
    let fuente = Carpeta::nueva();
    fuente.escribir("mods/remoto.jar", b"mod");
    let store = Almacen::nuevo(root.0.join("instancias"));
    let m = remoto(1, &[("mods/remoto.jar", b"mod")]);
    let i = nueva(&store, "Recibida", &m);
    vinculos::preparar_destino(&store, &i, &m, &Limites::default()).unwrap();
    intentos::registrar(&store, &i, &m).unwrap();
    let juego = store.juego(&i.id).unwrap();
    fs::create_dir_all(juego.join("saves/mundo")).unwrap();
    fs::write(juego.join("saves/mundo/level.dat"), b"mundo").unwrap();
    fs::write(juego.join("options.txt"), b"fullscreen:true\n").unwrap();
    aplicar_base(&store, &i, &m, &fuente);
    // Otro manifiesto (revisión distinta) no basta para limpiar.
    assert!(intentos::completar(&store, &i, &remoto(2, &[("mods/remoto.jar", b"mod")])).is_err());
    assert!(juego.join(MARCADOR).exists());
    intentos::completar(&store, &i, &m).unwrap();
    assert!(!juego.join(MARCADOR).exists());
    assert_eq!(
        fs::read(juego.join("saves/mundo/level.dat")).unwrap(),
        b"mundo"
    );
    assert_eq!(
        fs::read(juego.join("options.txt")).unwrap(),
        b"fullscreen:true\n"
    );
    assert_eq!(fs::read(juego.join("mods/remoto.jar")).unwrap(), b"mod");
    assert!(vinculos::leer(&juego).unwrap().is_some());
    assert!(vinculos::comprobar_lista(&store, &i).is_ok());
    // Idempotente tras la limpieza.
    intentos::completar(&store, &i, &m).unwrap();
}

#[test]
fn corte_entre_base_y_limpieza_no_ofrece_reanudar_la_instancia_completa() {
    let root = Carpeta::nueva();
    let fuente = Carpeta::nueva();
    fuente.escribir("mods/remoto.jar", b"mod");
    let raiz = root.0.join("instancias");
    let m = remoto(1, &[("mods/remoto.jar", b"mod")]);
    let i = {
        let store = Almacen::nuevo(&raiz);
        let i = nueva(&store, "Recibida", &m);
        vinculos::preparar_destino(&store, &i, &m, &Limites::default()).unwrap();
        intentos::registrar(&store, &i, &m).unwrap();
        aplicar_base(&store, &i, &m, &fuente);
        i
    };
    let store = Almacen::nuevo(&raiz);
    let antes = fs::read(marcador(&store, &i)).unwrap();
    assert!(intentos::buscar(&store, &m).unwrap().is_none());
    assert!(intentos::buscar(&store, &remoto(2, &[])).unwrap().is_none());
    assert_eq!(fs::read(marcador(&store, &i)).unwrap(), antes);
}

#[test]
fn dos_intentos_incompletos_del_mismo_origen_piden_eleccion() {
    let root = Carpeta::nueva();
    let store = Almacen::nuevo(root.0.join("instancias"));
    let m = remoto(1, &[]);
    let a = nueva(&store, "Recibida", &m);
    let b = nueva(&store, "Recibida 2", &m);
    intentos::registrar(&store, &a, &m).unwrap();
    intentos::registrar(&store, &b, &m).unwrap();
    assert!(intentos::buscar(&store, &m).is_err());
    assert!(marcador(&store, &a).exists() && marcador(&store, &b).exists());
}

#[test]
fn marcador_copiado_a_otra_instancia_no_se_acepta() {
    let root = Carpeta::nueva();
    let store = Almacen::nuevo(root.0.join("instancias"));
    let m = remoto(1, &[]);
    let a = nueva(&store, "Recibida", &m);
    let b = nueva(&store, "Copia", &m);
    intentos::registrar(&store, &a, &m).unwrap();
    fs::copy(marcador(&store, &a), marcador(&store, &b)).unwrap();
    assert!(intentos::buscar(&store, &m).is_err());
    assert!(intentos::completar(&store, &b, &m).is_err());
    assert!(marcador(&store, &b).exists());
}
