// SPDX-License-Identifier: GPL-3.0-only
#[test]
fn conservar_pack_local_ausente_no_lo_activa_al_publicarse_como_activo() {
    use super::{aplicacion as ayuda, Carpeta};
    use icaro_core::compartir::{
        aplicacion as tx, inventario,
        plan::{planificar, Resolucion},
        Limites,
    };
    use tokio_util::sync::CancellationToken;
    for (ruta, pack) in [
        ("resourcepacks/b.zip", "file/b.zip"),
        ("resourcepacks/b/pack.mcmeta", "file/b"),
    ] {
        let juego = Carpeta::nueva();
        let fuente = Carpeta::nueva();
        let ctx = ayuda::contexto(&juego);
        fuente.escribir(ruta, b"b1");
        let anterior = ayuda::remoto(&[(ruta, b"b1")], 1);
        tx::aplicar(
            ayuda::preparar(&ctx, &anterior, &fuente),
            &CancellationToken::new(),
            |_| {},
        )
        .unwrap();
        std::fs::remove_file(juego.0.join(ruta)).unwrap();
        let mut nuevo = ayuda::remoto(&[(ruta, b"b2")], 2);
        nuevo.packs = vec![pack.into()];
        let inv =
            inventario::analizar(&juego.0, &CancellationToken::new(), &Limites::default()).unwrap();
        let mut plan = planificar(&nuevo, &tx::cargar_base(&ctx).unwrap(), &inv).unwrap();
        plan.resolver(&plan.conflictos[0].ruta.clone(), Resolucion::ConservarMia)
            .unwrap();
        let p = tx::preparar(
            &ctx,
            plan,
            &ayuda::FuenteLocal(fuente.0.clone()),
            &CancellationToken::new(),
        )
        .unwrap();
        tx::aplicar(p, &CancellationToken::new(), |_| {}).unwrap();
        let opciones = std::fs::read_to_string(juego.0.join("options.txt")).unwrap_or_default();
        assert!(!opciones.contains(pack));
    }
}
#[test]
fn seleccion_revisada_rechaza_cambios_externos_antes_de_descargar_y_aplicar() {
    use super::{aplicacion as ayuda, Carpeta};
    use icaro_core::compartir::{
        aplicacion as tx, inventario,
        packs::RevisionOrden,
        plan::{planificar, BaseAplicada},
        Limites,
    };
    use tokio_util::sync::CancellationToken;
    let juego = Carpeta::nueva();
    let fuente = Carpeta::nueva();
    let ctx = ayuda::contexto(&juego);
    juego.escribir(
        "options.txt",
        b"lang:es_es\nresourcePacks:[\"vanilla\"]\ngamma:0.8\n",
    );
    let mut remoto = ayuda::remoto(&[], 1);
    remoto.packs = vec!["vanilla".into(), "punchy:integrado".into()];
    let inv =
        inventario::analizar(&juego.0, &CancellationToken::new(), &Limites::default()).unwrap();
    let mut plan = planificar(&remoto, &BaseAplicada::default(), &inv).unwrap();
    plan.orden_packs = Some(RevisionOrden::nueva(&["vanilla".into()], &[], &remoto.packs).unwrap());
    tx::comprobar_opciones(&ctx, &plan).unwrap();
    juego.escribir(
        "options.txt",
        b"resourcePacks:[\"vanilla\",\"personal:pack\"]\n",
    );
    assert!(tx::comprobar_opciones(&ctx, &plan).is_err());
    juego.escribir(
        "options.txt",
        b"lang:es_es\nresourcePacks:[\"vanilla\"]\ngamma:0.8\n",
    );
    let preparado = tx::preparar(
        &ctx,
        plan,
        &ayuda::FuenteLocal(fuente.0.clone()),
        &CancellationToken::new(),
    )
    .unwrap();
    let personal = b"lang:es_cl\nresourcePacks:[\"vanilla\"]\ngamma:0.5\n";
    juego.escribir("options.txt", personal);
    assert!(tx::aplicar(preparado, &CancellationToken::new(), |_| {}).is_err());
    assert_eq!(
        std::fs::read(juego.0.join("options.txt")).unwrap(),
        personal
    );
    assert!(tx::cargar_base(&ctx).unwrap().manifiesto.is_none());
}
#[test]
fn resolver_orden_compartido_conserva_personales_y_rechaza_revision_local_distinta() {
    use icaro_core::compartir::{packs::RevisionOrden, plan::Resolucion};
    let actual = vec![
        "vanilla".into(),
        "file/b.zip".into(),
        "file/personal.zip".into(),
        "file/a.zip".into(),
    ];
    let anterior = vec!["file/a.zip".into(), "file/b.zip".into()];
    let nuevo = vec!["file/a.zip".into()];
    let mut revision = RevisionOrden::nueva(&actual, &anterior, &nuevo).unwrap();
    assert!(revision.pendiente());
    assert!(revision.aplicar(&actual).is_err());
    revision.resolver(Resolucion::DelAnfitrion).unwrap();
    assert_eq!(
        revision.aplicar(&actual).unwrap(),
        vec!["vanilla", "file/a.zip", "file/personal.zip"]
    );
    assert!(revision.aplicar(&["vanilla".into()]).is_err());
}

#[test]
fn conservar_seleccion_local_retiro_no_deja_pack_retirado_activado() {
    use icaro_core::compartir::{packs::RevisionOrden, plan::Resolucion};
    let actual = vec![
        "vanilla".into(),
        "file/b.zip".into(),
        "file/personal.zip".into(),
        "file/a.zip".into(),
    ];
    let anterior = vec!["file/a.zip".into(), "file/b.zip".into()];
    let nuevo = vec!["file/a.zip".into(), "file/c.zip".into()];
    let mut revision = RevisionOrden::nueva(&actual, &anterior, &nuevo).unwrap();
    revision.resolver(Resolucion::ConservarMia).unwrap();
    assert_eq!(
        revision.aplicar(&actual).unwrap(),
        vec!["vanilla", "file/personal.zip", "file/a.zip"]
    );
}
#[test]
fn primera_revision_no_confunde_vanilla_con_un_zip_personal() {
    use icaro_core::compartir::packs::combinar_orden;
    let actual = vec!["vanilla".into(), "file/personal.zip".into()];
    let nuevo = vec!["vanilla".into(), "file/compartido.zip".into()];
    assert_eq!(
        combinar_orden(&actual, &[], &nuevo).unwrap(),
        vec!["vanilla", "file/personal.zip", "file/compartido.zip"]
    );
}
#[test]
fn orden_compartido_conserva_prioridad_relativa_de_packs_personales() {
    use icaro_core::compartir::packs::combinar_orden;
    let actual = vec![
        "vanilla".into(),
        "file/personal.zip".into(),
        "file/a.zip".into(),
        "file/b.zip".into(),
    ];
    let anterior = vec!["file/a.zip".into(), "file/b.zip".into()];
    let nuevo = vec!["file/b.zip".into(), "file/a.zip".into()];
    assert_eq!(
        combinar_orden(&actual, &anterior, &nuevo).unwrap(),
        vec!["vanilla", "file/personal.zip", "file/b.zip", "file/a.zip"]
    );
}

#[test]
fn cambios_simultaneos_de_orden_o_pack_personal_colisionado_no_se_pisan() {
    use icaro_core::compartir::packs::combinar_orden;
    let anterior = vec!["file/a.zip".into(), "file/b.zip".into()];
    let local = vec![
        "file/b.zip".into(),
        "file/a.zip".into(),
        "file/personal.zip".into(),
    ];
    assert_eq!(combinar_orden(&local, &anterior, &anterior).unwrap(), local);
    let remoto = vec!["file/b.zip".into()];
    assert!(combinar_orden(&local, &anterior, &remoto).is_err());
    let colision = vec!["file/personal.zip".into()];
    assert!(combinar_orden(&local, &anterior, &colision).is_err());
}
