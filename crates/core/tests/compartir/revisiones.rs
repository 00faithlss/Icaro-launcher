// SPDX-License-Identifier: GPL-3.0-only
use super::*;
#[test]
fn contenido_alterado_despues_de_preparar_no_se_publica() {
    use icaro_core::compartir::{
        publicacion::{Borrador, Eleccion, Seleccion},
        revisiones::{preparar_revision, publicar_revision, vigente, ContextoPublicacion},
    };
    let juego = Carpeta::nueva();
    let almacen = Carpeta::nueva();
    juego.escribir("config/a.toml", b"valor=1");
    let cancel = CancellationToken::new();
    let (origen, perfil) = origen_y_perfil();
    let ctx = ContextoPublicacion {
        juego: juego.0.clone(),
        almacen: almacen.0.clone(),
        origen,
        perfil,
        limites: Limites::default(),
    };
    let inv = inventario::analizar(&juego.0, &cancel, &ctx.limites).unwrap();
    let mut borrador = Borrador::comparar(&inv, None, &Seleccion::default()).unwrap();
    let ruta = RutaCompartida::parse("config/a.toml").unwrap();
    borrador.elegir(&ruta, Eleccion::Compartir).unwrap();
    let preparada = preparar_revision(&ctx, &borrador, &cancel).unwrap();
    let hash = &borrador.archivos_publicados()[0].sha512;
    fs::write(almacen.0.join("contenido").join(hash), b"valor=2").unwrap();
    assert!(publicar_revision(preparada).is_err());
    assert!(vigente(&ctx).unwrap().is_none());
}

#[test]
fn cancelar_despues_de_preparar_conserva_la_revision_vigente() {
    use icaro_core::compartir::{
        publicacion::{Borrador, Eleccion, Seleccion},
        revisiones::{preparar_revision, publicar_revision, vigente, ContextoPublicacion},
    };
    let juego = Carpeta::nueva();
    let almacen = Carpeta::nueva();
    juego.escribir("config/a.toml", b"valor=1");
    let cancel = CancellationToken::new();
    let (origen, perfil) = origen_y_perfil();
    let ctx = ContextoPublicacion {
        juego: juego.0.clone(),
        almacen: almacen.0.clone(),
        origen,
        perfil,
        limites: Limites::default(),
    };
    let inv = inventario::analizar(&juego.0, &cancel, &ctx.limites).unwrap();
    let mut borrador = Borrador::comparar(&inv, None, &Seleccion::default()).unwrap();
    borrador
        .elegir(
            &RutaCompartida::parse("config/a.toml").unwrap(),
            Eleccion::Compartir,
        )
        .unwrap();
    let preparada = preparar_revision(&ctx, &borrador, &cancel).unwrap();
    cancel.cancel();
    assert!(publicar_revision(preparada).is_err());
    assert!(vigente(&ctx).unwrap().is_none());
}

#[test]
fn cambiar_solo_el_orden_de_packs_publica_y_bloquea_otra_preparacion() {
    use icaro_core::compartir::{
        publicacion::{Borrador, Eleccion, Seleccion},
        revisiones::{preparar_revision, publicar_revision, ContextoPublicacion},
    };
    let juego = Carpeta::nueva();
    let almacen = Carpeta::nueva();
    juego.escribir("config/a.toml", b"valor=1");
    let cancel = CancellationToken::new();
    let (origen, perfil) = origen_y_perfil();
    let ctx = ContextoPublicacion {
        juego: juego.0.clone(),
        almacen: almacen.0.clone(),
        origen,
        perfil,
        limites: Limites::default(),
    };
    let inv = inventario::analizar(&juego.0, &cancel, &ctx.limites).unwrap();
    let mut borrador = Borrador::comparar(&inv, None, &Seleccion::default()).unwrap();
    borrador
        .elegir(
            &RutaCompartida::parse("config/a.toml").unwrap(),
            Eleccion::Compartir,
        )
        .unwrap();
    borrador.seleccionar_packs(vec!["vanilla".into(), "punchy:default".into()]);
    let preparada = preparar_revision(&ctx, &borrador, &cancel).unwrap();
    assert!(preparar_revision(&ctx, &borrador, &cancel).is_err());
    let primera = publicar_revision(preparada).unwrap();
    let mut nuevo =
        Borrador::comparar(&inv, Some(&primera.manifiesto), &Seleccion::default()).unwrap();
    assert!(
        preparar_revision(&ctx, &nuevo, &cancel).is_err(),
        "sin cambios no debe publicar"
    );
    nuevo.seleccionar_packs(vec!["punchy:default".into(), "vanilla".into()]);
    let segunda = publicar_revision(preparar_revision(&ctx, &nuevo, &cancel).unwrap()).unwrap();
    assert_eq!(segunda.manifiesto.revision, 2);
    assert_eq!(segunda.manifiesto.packs, vec!["punchy:default", "vanilla"]);
    assert!(
        preparar_revision(&ctx, &nuevo, &cancel).is_err(),
        "el borrador anterior quedo obsoleto"
    );
}

#[test]
fn publicacion_conserva_snapshot_independiente_y_errores_no_reemplazan_vigente() {
    use icaro_core::compartir::{
        publicacion::{Borrador, Eleccion, Seleccion},
        revisiones::{preparar_revision, publicar_revision, vigente, ContextoPublicacion},
    };
    let juego = Carpeta::nueva();
    let almacen = Carpeta::nueva();
    juego.escribir("mods/a.jar", b"primero");
    let limites = Limites::default();
    let cancel = CancellationToken::new();
    let inv = inventario::analizar(&juego.0, &cancel, &limites).unwrap();
    let mut borrador = Borrador::comparar(&inv, None, &Seleccion::default()).unwrap();
    let ruta = RutaCompartida::parse("mods/a.jar").unwrap();
    borrador.elegir(&ruta, Eleccion::Compartir).unwrap();
    let (origen, perfil) = origen_y_perfil();
    let ctx = ContextoPublicacion {
        juego: juego.0.clone(),
        almacen: almacen.0.clone(),
        origen,
        perfil,
        limites,
    };
    assert!(
        preparar_revision(&ctx, &borrador, &cancel).is_err(),
        "falta autorizar el JAR propio"
    );
    borrador.autorizar_propios();
    let lista = preparar_revision(&ctx, &borrador, &cancel).unwrap();
    assert!(
        vigente(&ctx).unwrap().is_none(),
        "preparar no debe publicar"
    );
    let revision = publicar_revision(lista).unwrap();
    assert_eq!(revision.manifiesto.revision, 1);
    juego.escribir("mods/a.jar", b"segundo");
    assert_eq!(
        fs::read(revision.archivo(&ruta).unwrap()).unwrap(),
        b"primero"
    );
    let inv2 = inventario::analizar(&juego.0, &cancel, &ctx.limites).unwrap();
    let mut borrador2 =
        Borrador::comparar(&inv2, Some(&revision.manifiesto), &Seleccion::default()).unwrap();
    borrador2.autorizar_propios();
    juego.escribir("mods/a.jar", b"tercero");
    assert!(preparar_revision(&ctx, &borrador2, &cancel).is_err());
    assert_eq!(vigente(&ctx).unwrap().unwrap().manifiesto.revision, 1);
    assert_eq!(fs::read(juego.0.join("mods/a.jar")).unwrap(), b"tercero");
}

#[test]
fn cancelar_preparacion_no_publica_y_retirada_pospuesta_reutiliza_snapshot() {
    use icaro_core::compartir::{
        publicacion::{Borrador, Eleccion, Seleccion},
        revisiones::{preparar_revision, publicar_revision, vigente, ContextoPublicacion},
    };
    let juego = Carpeta::nueva();
    let almacen = Carpeta::nueva();
    juego.escribir("config/mod.toml", b"valor=1");
    let cancel = CancellationToken::new();
    let (origen, perfil) = origen_y_perfil();
    let ctx = ContextoPublicacion {
        juego: juego.0.clone(),
        almacen: almacen.0.clone(),
        origen,
        perfil,
        limites: Limites::default(),
    };
    let ruta = RutaCompartida::parse("config/mod.toml").unwrap();
    let inv = inventario::analizar(&juego.0, &cancel, &ctx.limites).unwrap();
    let mut borrador = Borrador::comparar(&inv, None, &Seleccion::default()).unwrap();
    borrador.elegir(&ruta, Eleccion::Compartir).unwrap();
    let cancelada = CancellationToken::new();
    cancelada.cancel();
    assert!(preparar_revision(&ctx, &borrador, &cancelada).is_err());
    assert!(vigente(&ctx).unwrap().is_none());
    let primera = publicar_revision(preparar_revision(&ctx, &borrador, &cancel).unwrap()).unwrap();
    fs::remove_file(juego.0.join("config/mod.toml")).unwrap();
    let inv2 = inventario::analizar(&juego.0, &cancel, &ctx.limites).unwrap();
    let mut segundo =
        Borrador::comparar(&inv2, Some(&primera.manifiesto), &Seleccion::default()).unwrap();
    segundo.elegir(&ruta, Eleccion::Posponer).unwrap();
    // Agregar otra configuración permite publicar una revisión efectiva.
    juego.escribir("config/otro.toml", b"otro=2");
    let inv3 = inventario::analizar(&juego.0, &cancel, &ctx.limites).unwrap();
    segundo = Borrador::comparar(&inv3, Some(&primera.manifiesto), &Seleccion::default()).unwrap();
    segundo.elegir(&ruta, Eleccion::Posponer).unwrap();
    segundo
        .elegir(
            &RutaCompartida::parse("config/otro.toml").unwrap(),
            Eleccion::Compartir,
        )
        .unwrap();
    let nueva = publicar_revision(preparar_revision(&ctx, &segundo, &cancel).unwrap()).unwrap();
    assert_eq!(nueva.manifiesto.revision, 2);
    assert_eq!(fs::read(nueva.archivo(&ruta).unwrap()).unwrap(), b"valor=1");
}
