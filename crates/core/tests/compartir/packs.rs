// SPDX-License-Identifier: GPL-3.0-only
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
