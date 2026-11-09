// SPDX-License-Identifier: GPL-3.0-only
use super::{
    aplicacion::{contexto, preparar, remoto},
    *,
};
use icaro_core::compartir::aplicacion::{self as tx, EventoAplicacion};
use std::process::Command;

fn iniciar() -> (Carpeta, Carpeta, Manifiesto) {
    let juego = Carpeta::nueva();
    let fuente = Carpeta::nueva();
    fuente.escribir("mods/a.jar", b"mod1");
    fuente.escribir("config/a.toml", b"config1");
    let base = remoto(&[("mods/a.jar", b"mod1"), ("config/a.toml", b"config1")], 1);
    tx::aplicar(
        preparar(&contexto(&juego), &base, &fuente),
        &CancellationToken::new(),
        |_| {},
    )
    .unwrap();
    fuente.escribir("mods/a.jar", b"mod2");
    fuente.escribir("config/a.toml", b"config2");
    let nueva = remoto(&[("mods/a.jar", b"mod2"), ("config/a.toml", b"config2")], 2);
    fuente.escribir("revision.json", &serde_json::to_vec(&nueva).unwrap());
    (juego, fuente, base)
}
fn cortar(juego: &Carpeta, fuente: &Carpeta, movimiento: usize) {
    let salida = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "recuperacion::ayudante_corte", "--ignored"])
        .env("ICARO_TEST_JUEGO", &juego.0)
        .env("ICARO_TEST_FUENTE", &fuente.0)
        .env("ICARO_TEST_CORTE", movimiento.to_string())
        .output()
        .unwrap();
    assert_eq!(
        salida.status.code(),
        Some(83),
        "{}\n{}",
        String::from_utf8_lossy(&salida.stdout),
        String::from_utf8_lossy(&salida.stderr)
    );
}

#[test]
#[ignore = "Ayudante invocado por las pruebas de corte con sandboxes explícitos"]
fn ayudante_corte() {
    let Ok(juego) = std::env::var("ICARO_TEST_JUEGO") else {
        return;
    };
    let fuente = std::env::var("ICARO_TEST_FUENTE").unwrap();
    let corte: usize = std::env::var("ICARO_TEST_CORTE").unwrap().parse().unwrap();
    let ctx = tx::ContextoDestino {
        juego: juego.into(),
        limites: Limites::default(),
    };
    let revision = Manifiesto::leer(
        &fs::read(std::path::Path::new(&fuente).join("revision.json")).unwrap(),
        &ctx.limites,
    )
    .unwrap();
    let inv = inventario::analizar(&ctx.juego, &CancellationToken::new(), &ctx.limites).unwrap();
    let plan =
        icaro_core::compartir::plan::planificar(&revision, &tx::cargar_base(&ctx).unwrap(), &inv)
            .unwrap();
    let preparada = tx::preparar(
        &ctx,
        plan,
        &super::aplicacion::FuenteLocal(fuente.into()),
        &CancellationToken::new(),
    )
    .unwrap();
    tx::aplicar(preparada, &CancellationToken::new(), |evento| {
        if matches!(evento,EventoAplicacion::Movimiento{hechos,..} if hechos==corte) {
            std::process::exit(83);
        }
    })
    .unwrap();
}

#[test]
fn cierre_entre_cada_movimiento_recupera_mods_config_y_base() {
    for movimiento in 1..=6 {
        let (juego, fuente, base) = iniciar();
        cortar(&juego, &fuente, movimiento);
        tx::recuperar(&juego.0).unwrap();
        tx::recuperar(&juego.0).unwrap();
        assert_eq!(
            fs::read(juego.0.join("mods/a.jar")).unwrap(),
            b"mod1",
            "corte {movimiento}"
        );
        assert_eq!(
            fs::read(juego.0.join("config/a.toml")).unwrap(),
            b"config1",
            "corte {movimiento}"
        );
        assert_eq!(
            tx::cargar_base(&contexto(&juego)).unwrap().manifiesto,
            Some(base)
        );
    }
}

#[test]
fn recuperacion_con_cambio_externo_preserva_copia_personal_y_respaldo() {
    let (juego, fuente, base) = iniciar();
    cortar(&juego, &fuente, 2);
    juego.escribir("config/a.toml", b"copia personal despues del corte");
    assert!(tx::recuperar(&juego.0).is_err());
    assert_eq!(
        fs::read(juego.0.join("config/a.toml")).unwrap(),
        b"copia personal despues del corte"
    );
    assert_eq!(
        tx::cargar_base(&contexto(&juego)).unwrap().manifiesto,
        Some(base)
    );
    let pendiente = fs::read_dir(&juego.0)
        .unwrap()
        .flatten()
        .find(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with(".icaro-compartir-tx-")
        })
        .unwrap();
    assert_eq!(
        fs::read(pendiente.path().join("anteriores/0")).unwrap(),
        b"config1"
    );
}

#[test]
fn cambio_en_staging_durante_aplicacion_aborta_y_revierte() {
    let juego = Carpeta::nueva();
    let fuente = Carpeta::nueva();
    let ctx = contexto(&juego);
    fuente.escribir("config/a.toml", b"aaa");
    fuente.escribir("config/b.toml", b"bbb");
    let revision = remoto(&[("config/a.toml", b"aaa"), ("config/b.toml", b"bbb")], 1);
    let preparada = preparar(&ctx, &revision, &fuente);
    let resultado = tx::aplicar(preparada, &CancellationToken::new(), |evento| {
        if matches!(evento, EventoAplicacion::Movimiento { hechos: 1, .. }) {
            let dir = fs::read_dir(&juego.0)
                .unwrap()
                .flatten()
                .find(|e| {
                    e.file_name()
                        .to_string_lossy()
                        .starts_with(".icaro-compartir-tx-")
                })
                .unwrap();
            fs::write(dir.path().join("nuevos/1"), b"mal").unwrap();
        }
    });
    assert!(resultado.is_err());
    assert!(!juego.0.join("config/a.toml").exists());
    assert!(!juego.0.join("config/b.toml").exists());
    assert!(tx::cargar_base(&ctx).unwrap().manifiesto.is_none());
}

#[test]
fn modificar_slot_actual_tras_respaldar_no_publica_contenido_corrupto() {
    let (juego, fuente, base) = iniciar();
    let revision = Manifiesto::leer(
        &fs::read(fuente.0.join("revision.json")).unwrap(),
        &Limites::default(),
    )
    .unwrap();
    let ctx = contexto(&juego);
    let lista = preparar(&ctx, &revision, &fuente);
    let resultado = tx::aplicar(lista, &CancellationToken::new(), |evento| {
        if matches!(evento, EventoAplicacion::Movimiento { hechos: 1, .. }) {
            let dir = fs::read_dir(&juego.0)
                .unwrap()
                .flatten()
                .find(|e| {
                    e.file_name()
                        .to_string_lossy()
                        .starts_with(".icaro-compartir-tx-")
                })
                .unwrap();
            fs::write(dir.path().join("nuevos/0"), b"corrupto").unwrap();
        }
    });
    assert!(resultado.is_err());
    assert_eq!(fs::read(juego.0.join("config/a.toml")).unwrap(), b"config1");
    assert_eq!(tx::cargar_base(&ctx).unwrap().manifiesto, Some(base));
}

#[test]
fn corte_durante_limpieza_no_bloquea_recuperaciones_posteriores() {
    let (juego, fuente, base) = iniciar();
    cortar(&juego, &fuente, 1);
    let pendiente = fs::read_dir(&juego.0)
        .unwrap()
        .flatten()
        .find(|e| {
            e.file_name()
                .to_string_lossy()
                .starts_with(".icaro-compartir-tx-")
        })
        .unwrap()
        .path();
    // Estado tras restaurar originales y eliminar el primer grupo de staging.
    fs::rename(
        pendiente.join("anteriores/0"),
        juego.0.join("config/a.toml"),
    )
    .unwrap();
    for archivo in fs::read_dir(pendiente.join("nuevos")).unwrap().flatten() {
        fs::remove_file(archivo.path()).unwrap();
    }
    fs::remove_dir(pendiente.join("nuevos")).unwrap();
    tx::recuperar(&juego.0).unwrap();
    tx::recuperar(&juego.0).unwrap();
    assert_eq!(
        tx::cargar_base(&contexto(&juego)).unwrap().manifiesto,
        Some(base)
    );
    assert_eq!(fs::read(juego.0.join("config/a.toml")).unwrap(), b"config1");
}
