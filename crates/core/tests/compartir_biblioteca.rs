// SPDX-License-Identifier: GPL-3.0-only
//! Biblioteca de publicaciones propias: lectura sin efectos de lo ya persistido.
//! Nunca crea origen, identidad, revisión ni accesos; nunca adopta lo ajeno.
use icaro_core::{
    compartir::{
        self,
        accesos::Accesos,
        biblioteca::{self, EstadoGuardada},
        identidad::{AlmacenClave, Identidad, Zeroizing},
        revisiones::ContextoPublicacion,
        vinculos, Limites, Manifiesto,
    },
    instancias::{Almacen, Instancia},
};
use sha2::{Digest, Sha512};
use std::{
    cell::Cell,
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
#[path = "compartir/comun.rs"]
#[allow(dead_code)]
mod comun;
use comun::Carpeta;

fn clave() -> String {
    "ab".repeat(32)
}

/// Todos los archivos bajo `raiz` con su contenido: detecta cualquier escritura.
fn instantanea(raiz: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn recorrer(dir: &Path, salida: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entrada in fs::read_dir(dir).unwrap() {
            let ruta = entrada.unwrap().path();
            if ruta.is_dir() {
                salida.insert(ruta.clone(), Vec::new());
                recorrer(&ruta, salida);
            } else {
                salida.insert(ruta.clone(), fs::read(&ruta).unwrap());
            }
        }
    }
    let mut salida = BTreeMap::new();
    recorrer(raiz, &mut salida);
    salida
}

fn publicar(ctx: &ContextoPublicacion, revision: u64, minecraft: Option<&str>) {
    let mut perfil = serde_json::to_value(&ctx.perfil).unwrap();
    if let Some(version) = minecraft {
        perfil["minecraft"] = version.into();
    }
    let manifiesto = serde_json::json!({
        "esquema": "icaro-compartir/1",
        "origen": serde_json::to_value(&ctx.origen).unwrap(),
        "revision": revision,
        "perfil": perfil,
        "archivos": [{
            "ruta": "mods/ejemplo.jar",
            "bytes": 3,
            "sha512": format!("{:x}", Sha512::digest(b"jar")),
            "procedencia": {"tipo": "propio"}
        }],
        "packs": []
    });
    fs::create_dir_all(&ctx.almacen).unwrap();
    fs::write(
        ctx.almacen.join("vigente.json"),
        serde_json::to_vec(&manifiesto).unwrap(),
    )
    .unwrap();
}

fn entorno() -> (Carpeta, Almacen, Instancia) {
    let root = Carpeta::nueva();
    let store = Almacen::nuevo(root.0.join("instances"));
    let instancia = store.crear("Servidor de prueba", "1.21.1", 0).unwrap();
    (root, store, instancia)
}

#[test]
fn listar_no_crea_origen_identidad_ni_carpetas_de_publicacion() {
    let (root, store, instancia) = entorno();
    let antes = instantanea(&root.0);
    let lista = biblioteca::listar(&root.0, &store, &clave()).unwrap();
    assert!(lista.is_empty());
    assert_eq!(instantanea(&root.0), antes, "listar no debe escribir nada");
    let juego = store.juego(&instancia.id).unwrap();
    assert!(!juego.join(".icaro-compartir-publicacion.json").exists());
    assert!(!root.0.join("compartir").exists());
}

#[test]
fn listar_recupera_el_mismo_origen_y_revision_sin_escribir() {
    let (root, store, instancia) = entorno();
    let ctx = vinculos::publicacion(&root.0, &store, &instancia, &clave()).unwrap();
    publicar(&ctx, 3, None);
    let antes = instantanea(&root.0);
    let primera = biblioteca::listar(&root.0, &store, &clave()).unwrap();
    let segunda = biblioteca::listar(&root.0, &store, &clave()).unwrap();
    assert_eq!(instantanea(&root.0), antes, "listar no debe escribir nada");
    assert_eq!(primera.len(), 1);
    let p = &primera[0];
    assert_eq!(p.instancia, instancia.id);
    assert_eq!(p.origen.as_ref(), Some(&ctx.origen));
    assert_eq!(p.revision, Some(3));
    assert_eq!(p.archivos, 1);
    assert_eq!(p.autorizados, 0);
    assert_eq!(p.estado, EstadoGuardada::Lista);
    assert_eq!(segunda[0].origen, p.origen, "nunca otra publicación");
}

#[test]
fn origen_sin_revision_se_informa_como_sin_revision() {
    // Revisar una instancia ya crea su origen; eso no la convierte en publicación.
    let (root, store, instancia) = entorno();
    vinculos::publicacion(&root.0, &store, &instancia, &clave()).unwrap();
    let lista = biblioteca::listar(&root.0, &store, &clave()).unwrap();
    assert_eq!(lista.len(), 1);
    assert_eq!(lista[0].estado, EstadoGuardada::SinRevision);
    assert_eq!(lista[0].revision, None);
}

#[test]
fn ignora_publicaciones_de_otra_identidad_y_destinos_de_invitado() {
    let (root, store, instancia) = entorno();
    let ajena = vinculos::publicacion(&root.0, &store, &instancia, &"cd".repeat(32)).unwrap();
    publicar(&ajena, 1, None);
    let (origen, perfil) = comun::origen_y_perfil();
    let manifiesto = Manifiesto {
        esquema: "icaro-compartir/1".into(),
        origen,
        perfil,
        revision: 1,
        archivos: vec![],
        packs: vec![],
    };
    let invitada =
        vinculos::crear_destino(&store, "Invitada", &manifiesto, &Limites::default()).unwrap();
    // Incluso un origen propio residual no convierte un destino en publicador.
    vinculos::publicacion(&root.0, &store, &invitada, &clave()).unwrap();
    let antes = instantanea(&root.0);
    assert!(biblioteca::abrir(&root.0, &store, &invitada, &clave()).is_err());
    assert_eq!(instantanea(&root.0), antes);
    let lista = biblioteca::listar(&root.0, &store, &clave()).unwrap();
    assert!(
        lista.is_empty(),
        "ni otra clave ni un vínculo de invitado son publicaciones propias"
    );
}

#[test]
fn cuenta_autorizados_vigentes_y_refleja_publicacion_detenida() {
    let (root, store, instancia) = entorno();
    let ctx = vinculos::publicacion(&root.0, &store, &instancia, &clave()).unwrap();
    publicar(&ctx, 2, None);
    let accesos = Accesos::abrir(ctx.almacen.join("accesos.json")).unwrap();
    accesos.autorizar(&"11".repeat(32)).unwrap();
    accesos.autorizar(&"22".repeat(32)).unwrap();
    accesos.revocar(&"22".repeat(32)).unwrap();
    let lista = biblioteca::listar(&root.0, &store, &clave()).unwrap();
    assert_eq!(lista[0].autorizados, 1, "un revocado no cuenta");
    assert_eq!(lista[0].estado, EstadoGuardada::Lista);
    accesos.detener().unwrap();
    let antes = fs::read(ctx.almacen.join("accesos.json")).unwrap();
    let lista = biblioteca::listar(&root.0, &store, &clave()).unwrap();
    assert_eq!(lista[0].estado, EstadoGuardada::Detenida);
    assert_eq!(lista[0].revision, Some(2));
    assert_eq!(
        fs::read(ctx.almacen.join("accesos.json")).unwrap(),
        antes,
        "leer accesos no reinicia ni modifica la lista"
    );
}

#[test]
fn registro_ilegible_o_perfil_cambiado_no_impide_listar_las_demas() {
    let (root, store, rota) = entorno();
    let juego = store.juego(&rota.id).unwrap();
    fs::write(
        juego.join(".icaro-compartir-publicacion.json"),
        b"{no es json",
    )
    .unwrap();
    let cambiada = store.crear("Cambiada", "1.21.1", 0).unwrap();
    let ctx = vinculos::publicacion(&root.0, &store, &cambiada, &clave()).unwrap();
    publicar(&ctx, 4, Some("1.20.1"));
    let sana = store.crear("Sana", "1.21.1", 0).unwrap();
    let ctx_sana = vinculos::publicacion(&root.0, &store, &sana, &clave()).unwrap();
    publicar(&ctx_sana, 1, None);
    let lista = biblioteca::listar(&root.0, &store, &clave()).unwrap();
    let estado = |id: &str| {
        lista
            .iter()
            .find(|p| p.instancia == id)
            .map(|p| p.estado)
            .unwrap()
    };
    assert_eq!(estado(&rota.id), EstadoGuardada::Ilegible);
    assert_eq!(estado(&cambiada.id), EstadoGuardada::PerfilCambiado);
    assert_eq!(estado(&sana.id), EstadoGuardada::Lista);
    assert!(lista
        .iter()
        .find(|p| p.instancia == rota.id)
        .unwrap()
        .origen
        .is_none());
    assert_eq!(
        fs::read(juego.join(".icaro-compartir-publicacion.json")).unwrap(),
        b"{no es json",
        "un registro ilegible se conserva tal cual"
    );
}

#[test]
fn abrir_solo_reutiliza_un_origen_existente() {
    let (root, store, instancia) = entorno();
    let juego = store.juego(&instancia.id).unwrap();
    assert!(biblioteca::abrir(&root.0, &store, &instancia, &clave()).is_err());
    assert!(
        !juego.join(".icaro-compartir-publicacion.json").exists(),
        "abrir no crea otra publicación"
    );
    let ctx = vinculos::publicacion(&root.0, &store, &instancia, &clave()).unwrap();
    let abierta = biblioteca::abrir(&root.0, &store, &instancia, &clave()).unwrap();
    assert_eq!(abierta.origen, ctx.origen);
    assert_eq!(abierta.almacen, ctx.almacen);
    assert_eq!(abierta.juego, ctx.juego);
    assert!(biblioteca::abrir(&root.0, &store, &instancia, &"cd".repeat(32)).is_err());
}

#[test]
fn identidad_existente_no_genera_ni_guarda_claves() {
    struct Vacio<'a>(&'a Cell<bool>);
    impl AlmacenClave for Vacio<'_> {
        fn leer(&self) -> Result<Option<Zeroizing<Vec<u8>>>, compartir::Error> {
            Ok(None)
        }
        fn guardar(&self, _: &[u8]) -> Result<(), compartir::Error> {
            self.0.set(true);
            Ok(())
        }
    }
    struct Guardada;
    impl AlmacenClave for Guardada {
        fn leer(&self) -> Result<Option<Zeroizing<Vec<u8>>>, compartir::Error> {
            Ok(Some(Zeroizing::new(vec![7; 32])))
        }
        fn guardar(&self, _: &[u8]) -> Result<(), compartir::Error> {
            Err(compartir::Error::Credencial)
        }
    }
    let guardado = Cell::new(false);
    assert!(biblioteca::identidad_existente(&Vacio(&guardado))
        .unwrap()
        .is_none());
    assert!(!guardado.get(), "recuperar nunca crea otra identidad");
    let existente = biblioteca::identidad_existente(&Guardada).unwrap().unwrap();
    assert_eq!(
        existente.clave_publica(),
        Identidad::desde_almacen(&Guardada).unwrap().clave_publica()
    );
}

#[test]
fn anfitrion_invalido_se_rechaza_sin_leer_instancias() {
    let (root, store, _) = entorno();
    assert!(biblioteca::listar(&root.0, &store, "no-es-una-clave").is_err());
    assert!(biblioteca::listar(&root.0, &store, &"AB".repeat(32)).is_err());
}

#[test]
fn diagnostico_de_la_lista_no_incluye_nombres_de_instancia() {
    let (root, store, instancia) = entorno();
    let ctx = vinculos::publicacion(&root.0, &store, &instancia, &clave()).unwrap();
    publicar(&ctx, 1, None);
    let lista = biblioteca::listar(&root.0, &store, &clave()).unwrap();
    let texto = format!("{lista:?}");
    assert!(!texto.contains("Servidor de prueba"), "{texto}");
}

#[test]
fn eliminar_quita_solo_la_publicacion_propia_y_conserva_los_archivos_de_juego() {
    let (root, store, instancia) = entorno();
    let ctx = vinculos::publicacion(&root.0, &store, &instancia, &clave()).unwrap();
    publicar(&ctx, 2, None);
    let juego = store.juego(&instancia.id).unwrap();
    fs::create_dir_all(juego.join("mods")).unwrap();
    fs::write(juego.join("mods/propio.jar"), b"jar").unwrap();
    // Otra identidad no puede quitarla.
    assert!(biblioteca::eliminar(&root.0, &store, &instancia, &"cd".repeat(32)).is_err());
    assert_eq!(biblioteca::listar(&root.0, &store, &clave()).unwrap().len(), 1);
    biblioteca::eliminar(&root.0, &store, &instancia, &clave()).unwrap();
    assert!(biblioteca::listar(&root.0, &store, &clave()).unwrap().is_empty());
    assert!(!ctx.almacen.exists());
    assert!(!juego.join(".icaro-compartir-publicacion.json").exists());
    assert_eq!(fs::read(juego.join("mods/propio.jar")).unwrap(), b"jar");
    // Quitar otra vez no encuentra nada que borrar.
    assert!(biblioteca::eliminar(&root.0, &store, &instancia, &clave()).is_err());
}
