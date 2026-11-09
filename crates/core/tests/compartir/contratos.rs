// SPDX-License-Identifier: GPL-3.0-only
use super::*;
#[test]
fn rechaza_rutas_fuera_de_raices_y_nombres_no_portables() {
    for ruta in [
        "../mods/a.jar",
        "/mods/a.jar",
        "C:/mods/a.jar",
        "mods\\a.jar",
        "mods/../config/a",
        "saves/mundo/level.dat",
        "options.txt",
        "config//a",
        "config/CON.json",
        "config/a.",
        "config/a ",
        "config/a:b",
        "config/a\n",
        "mods/sub/a.jar",
    ] {
        assert!(RutaCompartida::parse(ruta).is_err(), "aceptó {ruta:?}");
    }
    for ruta in [
        "mods/a.jar",
        "config/ejemplo/client.toml",
        "kubejs/server_scripts/receta.js",
        "resourcepacks/arte/assets/a.png",
    ] {
        assert_eq!(RutaCompartida::parse(ruta).unwrap().as_str(), ruta);
    }
}

#[test]
fn rechaza_archivos_internos_y_secretos_evidentes() {
    for ruta in [
        "mods/.icaro-mods.json",
        "config/.env",
        "config/secreto.pem",
        "config/accounts/token.json",
        "kubejs/.git/config",
    ] {
        assert!(RutaCompartida::parse(ruta).is_err(), "aceptó {ruta}");
    }
    assert!(serde_json::from_str::<RutaCompartida>("\"../fuera\"").is_err());
}

#[test]
fn manifiesto_valida_perfil_hash_y_colisiones_windows() {
    let base = manifiesto();
    let bytes = serde_json::to_vec(&base).unwrap();
    assert!(Manifiesto::leer(&bytes, &Limites::default()).is_ok());
    let mut colision = base.clone();
    let mut otro = colision["archivos"][0].clone();
    otro["ruta"] = "mods/EJEMPLO.jar".into();
    colision["archivos"].as_array_mut().unwrap().push(otro);
    assert!(
        Manifiesto::leer(&serde_json::to_vec(&colision).unwrap(), &Limites::default()).is_err()
    );
    for (campo, valor) in [("sha512", "invalido"), ("ruta", "../fuera")] {
        let mut mal = base.clone();
        mal["archivos"][0][campo] = valor.into();
        assert!(Manifiesto::leer(&serde_json::to_vec(&mal).unwrap(), &Limites::default()).is_err());
    }
    let mut mal = base;
    mal["perfil"]["loader_version"] = serde_json::Value::Null;
    assert!(Manifiesto::leer(&serde_json::to_vec(&mal).unwrap(), &Limites::default()).is_err());
}

#[test]
fn limita_manifesto_archivo_y_total_antes_de_publicar() {
    let bytes = serde_json::to_vec(&manifiesto()).unwrap();
    for limites in [
        Limites {
            manifiesto_bytes: bytes.len() - 1,
            ..Limites::default()
        },
        Limites {
            archivos: 0,
            ..Limites::default()
        },
        Limites {
            archivo_bytes: 2,
            ..Limites::default()
        },
        Limites {
            revision_bytes: 2,
            ..Limites::default()
        },
    ] {
        assert!(Manifiesto::leer(&bytes, &limites).is_err());
    }
}

#[test]
fn manifiesto_admite_vanilla_y_packs_integrados_sin_archivo_externo() {
    let mut value = manifiesto();
    value["perfil"]["loader"] = "vanilla".into();
    value["perfil"]["loader_version"] = serde_json::Value::Null;
    value["packs"] = serde_json::json!(["vanilla", "punchy:default"]);
    assert!(Manifiesto::leer(&serde_json::to_vec(&value).unwrap(), &Limites::default()).is_ok());
    value["packs"] = serde_json::json!(["file/ausente.zip"]);
    assert!(Manifiesto::leer(&serde_json::to_vec(&value).unwrap(), &Limites::default()).is_err());
}

#[test]
fn manifiesto_rechaza_archivo_que_tambien_es_carpeta() {
    let mut value = manifiesto();
    value["archivos"][0]["ruta"] = "config/ejemplo".into();
    let mut hijo = value["archivos"][0].clone();
    hijo["ruta"] = "config/EJEMPLO/hijo.toml".into();
    value["archivos"].as_array_mut().unwrap().push(hijo);
    assert!(Manifiesto::leer(&serde_json::to_vec(&value).unwrap(), &Limites::default()).is_err());
}

#[test]
fn el_tamano_validado_del_manifiesto_es_el_que_se_guarda() {
    use icaro_core::compartir::{
        publicacion::{Borrador, Eleccion, Seleccion},
        revisiones::{preparar_revision, publicar_revision, vigente, ContextoPublicacion},
    };
    let juego = Carpeta::nueva();
    let almacen = Carpeta::nueva();
    juego.escribir("config/a.toml", b"valor=1");
    let cancel = CancellationToken::new();
    let (origen, perfil) = origen_y_perfil();
    let inv = inventario::analizar(&juego.0, &cancel, &Limites::default()).unwrap();
    let mut borrador = Borrador::comparar(&inv, None, &Seleccion::default()).unwrap();
    borrador
        .elegir(
            &RutaCompartida::parse("config/a.toml").unwrap(),
            Eleccion::Compartir,
        )
        .unwrap();
    let esperado = Manifiesto {
        esquema: "icaro-compartir/1".into(),
        origen: origen.clone(),
        perfil: perfil.clone(),
        revision: 1,
        archivos: borrador.archivos_publicados(),
        packs: vec![],
    };
    let compactos = serde_json::to_vec(&esperado).unwrap().len();
    let ctx = ContextoPublicacion {
        juego: juego.0.clone(),
        almacen: almacen.0.clone(),
        origen,
        perfil,
        limites: Limites {
            manifiesto_bytes: compactos,
            ..Limites::default()
        },
    };
    publicar_revision(preparar_revision(&ctx, &borrador, &cancel).unwrap()).unwrap();
    assert_eq!(vigente(&ctx).unwrap().unwrap().manifiesto, esperado);
    assert_eq!(
        fs::metadata(almacen.0.join("vigente.json")).unwrap().len(),
        compactos as u64
    );
}
