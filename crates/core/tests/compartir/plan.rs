// SPDX-License-Identifier: GPL-3.0-only
use super::*;
#[test]
fn tres_vias_conserva_cambios_personales_y_exige_resolver_conflictos() {
    use icaro_core::compartir::plan::{planificar, BaseAplicada, Resolucion};
    let juego = Carpeta::nueva();
    juego.escribir("mods/ejemplo.jar", b"jar");
    juego.escribir("config/personal.toml", b"propio");
    let base = Manifiesto::leer(
        &serde_json::to_vec(&manifiesto()).unwrap(),
        &Limites::default(),
    )
    .unwrap();
    let mut remoto = base.clone();
    remoto.revision = 2;
    remoto.archivos[0].bytes = 5;
    remoto.archivos[0].sha512 = format!("{:x}", Sha512::digest(b"nuevo"));
    let inv =
        inventario::analizar(&juego.0, &CancellationToken::new(), &Limites::default()).unwrap();
    let aplicada = BaseAplicada {
        manifiesto: Some(base.clone()),
    };
    let plan = planificar(&remoto, &aplicada, &inv).unwrap();
    assert!(plan.puede_aplicar());
    assert_eq!(plan.cambios.len(), 1);
    assert!(plan
        .conservados
        .iter()
        .any(|r| r.as_str() == "config/personal.toml"));
    juego.escribir("mods/ejemplo.jar", b"modificado por invitado");
    let cambiado =
        inventario::analizar(&juego.0, &CancellationToken::new(), &Limites::default()).unwrap();
    let mut conflicto = planificar(&remoto, &aplicada, &cambiado).unwrap();
    assert!(!conflicto.puede_aplicar());
    assert_eq!(conflicto.conflictos.len(), 1);
    conflicto
        .resolver(&base.archivos[0].ruta, Resolucion::ConservarMia)
        .unwrap();
    assert!(conflicto.puede_aplicar());
    assert!(conflicto.cambios.is_empty());
    let sin_cambio_remoto = planificar(&base, &aplicada, &cambiado).unwrap();
    assert!(sin_cambio_remoto.puede_aplicar());
    assert!(sin_cambio_remoto.cambios.is_empty());
}

#[test]
fn retirada_modificada_y_colision_personal_requieren_confirmacion() {
    use icaro_core::compartir::plan::{planificar, BaseAplicada, Resolucion};
    let juego = Carpeta::nueva();
    juego.escribir("mods/ejemplo.jar", b"version personal");
    let base = Manifiesto::leer(
        &serde_json::to_vec(&manifiesto()).unwrap(),
        &Limites::default(),
    )
    .unwrap();
    let inv =
        inventario::analizar(&juego.0, &CancellationToken::new(), &Limites::default()).unwrap();
    let mut retirada = base.clone();
    retirada.revision = 2;
    retirada.archivos.clear();
    let mut plan = planificar(
        &retirada,
        &BaseAplicada {
            manifiesto: Some(base.clone()),
        },
        &inv,
    )
    .unwrap();
    assert_eq!(plan.conflictos.len(), 1);
    plan.resolver(&base.archivos[0].ruta, Resolucion::DelAnfitrion)
        .unwrap();
    assert_eq!(plan.cambios.len(), 1);
    assert!(plan.cambios[0].despues.is_none());
    let primera = planificar(&base, &BaseAplicada::default(), &inv).unwrap();
    assert_eq!(
        primera.conflictos.len(),
        1,
        "un archivo personal no debe pisarse"
    );
    assert!(primera.cambios.is_empty());
}

#[test]
fn mismo_contenido_final_no_crea_conflicto_y_otro_origen_se_rechaza() {
    use icaro_core::compartir::plan::{planificar, BaseAplicada};
    let juego = Carpeta::nueva();
    juego.escribir("mods/ejemplo.jar", b"nuevo");
    let base = Manifiesto::leer(
        &serde_json::to_vec(&manifiesto()).unwrap(),
        &Limites::default(),
    )
    .unwrap();
    let mut remoto = base.clone();
    remoto.revision = 2;
    remoto.archivos[0].bytes = 5;
    remoto.archivos[0].sha512 = format!("{:x}", Sha512::digest(b"nuevo"));
    let aplicada = BaseAplicada {
        manifiesto: Some(base),
    };
    let inv =
        inventario::analizar(&juego.0, &CancellationToken::new(), &Limites::default()).unwrap();
    let plan = planificar(&remoto, &aplicada, &inv).unwrap();
    assert!(plan.puede_aplicar());
    assert!(plan.cambios.is_empty());
    remoto.origen.anfitrion = "ef".repeat(32);
    assert!(planificar(&remoto, &aplicada, &inv).is_err());
}
