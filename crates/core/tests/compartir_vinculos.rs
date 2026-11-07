// SPDX-License-Identifier: GPL-3.0-only
use icaro_core::{
    compartir::{self, vinculos, Limites, Manifiesto},
    instancias::Almacen,
};
use sha2::{Digest, Sha512};
use std::fs;
#[path = "compartir/comun.rs"]
mod comun;
use comun::Carpeta;

#[test]
fn publicacion_conserva_origen_y_rechaza_cambio_silencioso_de_identidad() {
    let root = Carpeta::nueva();
    root.escribir("dato-personal", b"conservar");
    let store = Almacen::nuevo(root.0.join("instancias"));
    let instancia = store.crear("Servidor", "1.21.1", 0).unwrap();
    let clave = "ab".repeat(32);
    let primera = vinculos::publicacion(&root.0, &store, &instancia, &clave).unwrap();
    let segunda = vinculos::publicacion(&root.0, &store, &instancia, &clave).unwrap();
    assert_eq!(primera.origen, segunda.origen);
    assert_ne!(primera.almacen, primera.juego);
    assert_eq!(
        fs::read(root.0.join("dato-personal")).unwrap(),
        b"conservar"
    );
    assert!(vinculos::publicacion(&root.0, &store, &instancia, &"cd".repeat(32)).is_err());
}

#[test]
fn invitacion_crea_destino_nuevo_fija_loader_y_no_marca_revision_aplicada() {
    let root = Carpeta::nueva();
    let store = Almacen::nuevo(root.0.join("instancias"));
    let existente = store.crear("Personal", "1.21.1", 0).unwrap();
    let (origen, _) = comun::origen_y_perfil();
    let manifiesto = Manifiesto {
        esquema: "icaro-compartir/1".into(),
        origen,
        revision: 1,
        perfil: compartir::Perfil {
            minecraft: "1.21.1".into(),
            loader: "fabric".into(),
            loader_version: Some("0.16.8".into()),
        },
        archivos: vec![],
        packs: vec![],
    };
    let nueva =
        vinculos::crear_destino(&store, "Invitada", &manifiesto, &Limites::default()).unwrap();
    assert_ne!(nueva.id, existente.id);
    assert_eq!(
        icaro_core::versiones_instancia::leer_loader(&store, &nueva).as_deref(),
        Some("0.16.8")
    );
    let juego = store.juego(&nueva.id).unwrap();
    let vinculo = vinculos::leer(&juego).unwrap().unwrap();
    assert_eq!(vinculo.origen, manifiesto.origen);
    assert_eq!(vinculo.perfil, manifiesto.perfil);
    assert!(!juego.join(".icaro-compartir-base.json").exists());
    assert!(vinculos::crear_destino(&store, "Personal", &manifiesto, &Limites::default()).is_err());
    assert!(vinculos::leer(&store.juego(&existente.id).unwrap())
        .unwrap()
        .is_none());
}
