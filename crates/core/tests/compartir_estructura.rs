// SPDX-License-Identifier: GPL-3.0-only
use icaro_core::compartir::aplicacion::{self as tx, EventoAplicacion};
use icaro_core::compartir::{inventario, Limites, Manifiesto, RutaCompartida};
use sha2::{Digest, Sha512};
use std::{fs, process::Command};
use tokio_util::sync::CancellationToken;
#[path = "compartir/comun.rs"]
mod comun;
use comun::{origen_y_perfil, Carpeta};
#[path = "compartir/aplicacion.rs"]
mod aplicacion;
use aplicacion::{contexto, preparar, remoto};

fn iniciar(inversa: bool) -> (Carpeta, Carpeta, Manifiesto, Manifiesto) {
    let juego = Carpeta::nueva();
    let fuente = Carpeta::nueva();
    let (antes, despues) = if inversa {
        ("config/a/b", "config/a")
    } else {
        ("config/a", "config/a/b")
    };
    fuente.escribir(antes, b"original");
    let base = remoto(&[(antes, b"original")], 1);
    tx::aplicar(
        preparar(&contexto(&juego), &base, &fuente),
        &CancellationToken::new(),
        |_| {},
    )
    .unwrap();
    // La fuente de cada revisión es independiente de la estructura anterior.
    let nueva_fuente = Carpeta::nueva();
    nueva_fuente.escribir(despues, b"nuevo");
    let nueva = remoto(&[(despues, b"nuevo")], 2);
    nueva_fuente.escribir("revision.json", &serde_json::to_vec(&nueva).unwrap());
    (juego, nueva_fuente, base, nueva)
}
#[test]
fn convierte_archivo_y_carpeta_en_ambas_direcciones() {
    for inversa in [false, true] {
        let (juego, fuente, _, nueva) = iniciar(inversa);
        tx::aplicar(
            preparar(&contexto(&juego), &nueva, &fuente),
            &CancellationToken::new(),
            |_| {},
        )
        .unwrap();
        let ruta = if inversa { "config/a" } else { "config/a/b" };
        assert_eq!(fs::read(juego.0.join(ruta)).unwrap(), b"nuevo");
        assert_eq!(
            tx::cargar_base(&contexto(&juego)).unwrap().manifiesto,
            Some(nueva)
        );
    }
}
#[test]
fn archivo_personal_impide_retirar_su_carpeta() {
    let (juego, fuente, base, nueva) = iniciar(true);
    juego.escribir("config/a/personal", b"personal");
    let ctx = contexto(&juego);
    let inv = inventario::analizar(&juego.0, &CancellationToken::new(), &ctx.limites).unwrap();
    let plan =
        icaro_core::compartir::plan::planificar(&nueva, &tx::cargar_base(&ctx).unwrap(), &inv)
            .unwrap();
    assert!(tx::preparar(
        &ctx,
        plan,
        &aplicacion::FuenteLocal(fuente.0.clone()),
        &CancellationToken::new()
    )
    .is_err());
    assert_eq!(
        fs::read(juego.0.join("config/a/personal")).unwrap(),
        b"personal"
    );
    assert_eq!(fs::read(juego.0.join("config/a/b")).unwrap(), b"original");
    assert_eq!(tx::cargar_base(&ctx).unwrap().manifiesto, Some(base));
}
#[test]
#[ignore = "Ayudante de cortes en sandbox"]
fn ayudante_corte_estructura() {
    let Ok(juego) = std::env::var("ICARO_ESTRUCTURA_JUEGO") else {
        return;
    };
    let fuente = Carpeta(std::env::var("ICARO_ESTRUCTURA_FUENTE").unwrap().into());
    let ctx = tx::ContextoDestino {
        juego: juego.into(),
        limites: Limites::default(),
    };
    let nueva = Manifiesto::leer(
        &fs::read(fuente.0.join("revision.json")).unwrap(),
        &ctx.limites,
    )
    .unwrap();
    let corte: usize = std::env::var("ICARO_ESTRUCTURA_CORTE")
        .unwrap()
        .parse()
        .unwrap();
    tx::aplicar(
        preparar(&ctx, &nueva, &fuente),
        &CancellationToken::new(),
        |e| {
            if matches!(e, EventoAplicacion::Movimiento { hechos, .. } if hechos == corte) {
                std::process::exit(83);
            }
        },
    )
    .unwrap();
}
#[test]
fn recupera_cortes_estructurales_y_repite_sin_efectos() {
    for inversa in [false, true] {
        for corte in 1..=4 {
            let (juego, fuente, base, _) = iniciar(inversa);
            let salida = Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "ayudante_corte_estructura", "--ignored"])
                .env("ICARO_ESTRUCTURA_JUEGO", &juego.0)
                .env("ICARO_ESTRUCTURA_FUENTE", &fuente.0)
                .env("ICARO_ESTRUCTURA_CORTE", corte.to_string())
                .output()
                .unwrap();
            assert_eq!(
                salida.status.code(),
                Some(83),
                "{}",
                String::from_utf8_lossy(&salida.stderr)
            );
            tx::recuperar(&juego.0).unwrap();
            tx::recuperar(&juego.0).unwrap();
            let ruta = if inversa { "config/a/b" } else { "config/a" };
            assert_eq!(fs::read(juego.0.join(ruta)).unwrap(), b"original");
            assert_eq!(
                tx::cargar_base(&contexto(&juego)).unwrap().manifiesto,
                Some(base)
            );
        }
    }
}

#[test]
fn rollback_estructural_no_publica_staging_alterado() {
    for inversa in [false, true] {
        let (juego, fuente, base, nueva) = iniciar(inversa);
        let lista = preparar(&contexto(&juego), &nueva, &fuente);
        let resultado = tx::aplicar(lista, &CancellationToken::new(), |evento| {
            if matches!(evento, EventoAplicacion::Movimiento { hechos: 1, .. }) {
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
                for slot in fs::read_dir(pendiente.join("nuevos")).unwrap().flatten() {
                    fs::write(slot.path(), b"alterado").unwrap();
                }
            }
        });
        assert!(resultado.is_err());
        tx::recuperar(&juego.0).unwrap();
        let ruta = if inversa { "config/a/b" } else { "config/a" };
        assert_eq!(fs::read(juego.0.join(ruta)).unwrap(), b"original");
        assert_eq!(
            tx::cargar_base(&contexto(&juego)).unwrap().manifiesto,
            Some(base)
        );
    }
}

#[test]
fn diario_estructural_no_autoriza_directorios_ajenos() {
    let (juego, fuente, _, nueva) = iniciar(false);
    let lista = preparar(&contexto(&juego), &nueva, &fuente);
    drop(lista);
    juego.escribir("saves/mundo/dato", b"personal");
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
    let path = pendiente.join("diario.json");
    let mut diario: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    diario["estructura"] = serde_json::json!({"creados":["saves/mundo"],"retirados":[]});
    fs::write(path, serde_json::to_vec(&diario).unwrap()).unwrap();
    assert!(tx::recuperar(&juego.0).is_err());
    assert_eq!(
        fs::read(juego.0.join("saves/mundo/dato")).unwrap(),
        b"personal"
    );
    assert_eq!(fs::read(juego.0.join("config/a")).unwrap(), b"original");
}

#[test]
fn cambio_externo_estructural_conserva_personal_y_original_en_respaldo() {
    let (juego, fuente, _, nueva) = iniciar(false);
    let lista = preparar(&contexto(&juego), &nueva, &fuente);
    let resultado = tx::aplicar(lista, &CancellationToken::new(), |evento| {
        if matches!(evento, EventoAplicacion::Movimiento { hechos: 1, .. }) {
            juego.escribir("config/a/personal", b"personal");
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
            for slot in fs::read_dir(pendiente.join("nuevos")).unwrap().flatten() {
                fs::write(slot.path(), b"alterado").unwrap();
            }
        }
    });
    assert!(resultado.is_err());
    assert!(tx::recuperar(&juego.0).is_err());
    assert_eq!(
        fs::read(juego.0.join("config/a/personal")).unwrap(),
        b"personal"
    );
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
    assert!(fs::read_dir(pendiente.join("anteriores"))
        .unwrap()
        .flatten()
        .any(|e| fs::read(e.path()).unwrap() == b"original"));
}
