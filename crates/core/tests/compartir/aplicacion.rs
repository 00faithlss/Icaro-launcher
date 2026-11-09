// SPDX-License-Identifier: GPL-3.0-only
use super::*;
use icaro_core::compartir::{
    aplicacion::{self as tx, ContextoDestino, FuenteArchivos},
    plan::{planificar, BaseAplicada},
    Artefacto, Error, Procedencia,
};
use std::{io::Read, path::PathBuf};

pub(super) struct FuenteLocal(pub PathBuf);
impl FuenteArchivos for FuenteLocal {
    fn abrir(&self, archivo: &Artefacto) -> Result<Box<dyn Read + Send>, Error> {
        Ok(Box::new(
            fs::File::open(self.0.join(archivo.ruta.as_str())).map_err(|_| Error::Disco)?,
        ))
    }
}
pub(super) fn remoto(archivos: &[(&str, &[u8])], revision: u64) -> Manifiesto {
    let (origen, perfil) = origen_y_perfil();
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
pub(super) fn contexto(juego: &Carpeta) -> ContextoDestino {
    ContextoDestino {
        juego: juego.0.clone(),
        limites: Limites::default(),
    }
}
pub(super) fn preparar(
    ctx: &ContextoDestino,
    revision: &Manifiesto,
    fuente: &Carpeta,
) -> tx::PlanPreparado {
    let inv = inventario::analizar(&ctx.juego, &CancellationToken::new(), &ctx.limites).unwrap();
    let base = tx::cargar_base(ctx).unwrap();
    let plan = planificar(revision, &base, &inv).unwrap();
    tx::preparar(
        ctx,
        plan,
        &FuenteLocal(fuente.0.clone()),
        &CancellationToken::new(),
    )
    .unwrap()
}

#[test]
fn aplica_revision_completa_y_conserva_datos_personales() {
    let juego = Carpeta::nueva();
    let fuente = Carpeta::nueva();
    let ctx = contexto(&juego);
    juego.escribir("mods/personal.jar", b"personal");
    juego.escribir("saves/mundo/level.dat", b"mundo");
    fuente.escribir("mods/remoto.jar", b"mod");
    fuente.escribir("config/mod.toml", b"valor=1");
    let revision = remoto(
        &[("mods/remoto.jar", b"mod"), ("config/mod.toml", b"valor=1")],
        1,
    );
    let lista = preparar(&ctx, &revision, &fuente);
    assert!(
        !juego.0.join("mods/remoto.jar").exists(),
        "preparar no debe aplicar"
    );
    let base = tx::aplicar(lista, &CancellationToken::new(), |_| {}).unwrap();
    assert_eq!(base.manifiesto, Some(revision.clone()));
    assert_eq!(tx::cargar_base(&ctx).unwrap().manifiesto, Some(revision));
    assert_eq!(
        fs::read(juego.0.join("config/mod.toml")).unwrap(),
        b"valor=1"
    );
    assert_eq!(
        fs::read(juego.0.join("mods/personal.jar")).unwrap(),
        b"personal"
    );
    assert_eq!(
        fs::read(juego.0.join("saves/mundo/level.dat")).unwrap(),
        b"mundo"
    );
}

#[test]
fn descarga_corrupta_y_cancelacion_no_modifican_el_destino() {
    let juego = Carpeta::nueva();
    let fuente = Carpeta::nueva();
    let ctx = contexto(&juego);
    fuente.escribir("config/mod.toml", b"corrupto");
    let revision = remoto(&[("config/mod.toml", b"correcto")], 1);
    let inv = inventario::analizar(&juego.0, &CancellationToken::new(), &ctx.limites).unwrap();
    let plan = planificar(&revision, &BaseAplicada::default(), &inv).unwrap();
    assert!(tx::preparar(
        &ctx,
        plan,
        &FuenteLocal(fuente.0.clone()),
        &CancellationToken::new()
    )
    .is_err());
    assert!(!juego.0.join("config/mod.toml").exists());
    fuente.escribir("config/mod.toml", b"correcto");
    let lista = preparar(&ctx, &revision, &fuente);
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(tx::aplicar(lista, &cancel, |_| {}).is_err());
    assert!(tx::cargar_base(&ctx).unwrap().manifiesto.is_none());
    assert!(!juego.0.join("config/mod.toml").exists());
}

#[test]
fn cambio_externo_despues_de_revisar_no_se_sobrescribe() {
    let juego = Carpeta::nueva();
    let fuente = Carpeta::nueva();
    let ctx = contexto(&juego);
    fuente.escribir("config/mod.toml", b"primero");
    let base = remoto(&[("config/mod.toml", b"primero")], 1);
    tx::aplicar(
        preparar(&ctx, &base, &fuente),
        &CancellationToken::new(),
        |_| {},
    )
    .unwrap();
    fuente.escribir("config/mod.toml", b"segundo");
    let nueva = remoto(&[("config/mod.toml", b"segundo")], 2);
    let lista = preparar(&ctx, &nueva, &fuente);
    juego.escribir("config/mod.toml", b"cambio externo");
    assert!(tx::aplicar(lista, &CancellationToken::new(), |_| {}).is_err());
    assert_eq!(
        fs::read(juego.0.join("config/mod.toml")).unwrap(),
        b"cambio externo"
    );
    assert_eq!(tx::cargar_base(&ctx).unwrap().manifiesto, Some(base));
}

#[test]
fn cinco_retiradas_no_copian_ni_reescriben_mods_ajenos() {
    let juego = Carpeta::nueva();
    let fuente = Carpeta::nueva();
    let ctx = contexto(&juego);
    let nombres: Vec<_> = (0..5).map(|i| format!("mods/{i}.jar")).collect();
    for nombre in &nombres {
        fuente.escribir(nombre, b"mod");
    }
    let archivos: Vec<_> = nombres
        .iter()
        .map(|n| (n.as_str(), b"mod".as_slice()))
        .collect();
    let base = remoto(&archivos, 1);
    tx::aplicar(
        preparar(&ctx, &base, &fuente),
        &CancellationToken::new(),
        |_| {},
    )
    .unwrap();
    juego.escribir("mods/personal.jar", b"personal");
    let path = juego.0.join("mods/personal.jar");
    let antes = fs::metadata(&path).unwrap().modified().unwrap();
    let nueva = remoto(&[], 2);
    tx::aplicar(
        preparar(&ctx, &nueva, &fuente),
        &CancellationToken::new(),
        |_| {},
    )
    .unwrap();
    assert!(nombres.iter().all(|n| !juego.0.join(n).exists()));
    assert_eq!(fs::read(&path).unwrap(), b"personal");
    assert_eq!(fs::metadata(&path).unwrap().modified().unwrap(), antes);
}

#[test]
fn seleccion_de_packs_solo_modifica_su_linea_de_options_y_respeta_crlf() {
    let juego = Carpeta::nueva();
    let fuente = Carpeta::nueva();
    let ctx = contexto(&juego);
    juego.escribir(
        "options.txt",
        b"lang:es_cl\r\nresourcePacks:[\"vanilla\",\"file/personal.zip\"]\r\nrenderDistance:8\r\n",
    );
    juego.escribir("resourcepacks/personal.zip", b"personal");
    fuente.escribir("resourcepacks/remoto.zip", b"pack");
    let mut revision = remoto(&[("resourcepacks/remoto.zip", b"pack")], 1);
    revision.packs = vec!["file/remoto.zip".into()];
    tx::aplicar(
        preparar(&ctx, &revision, &fuente),
        &CancellationToken::new(),
        |_| {},
    )
    .unwrap();
    assert_eq!(fs::read_to_string(juego.0.join("options.txt")).unwrap(),
        "lang:es_cl\r\nresourcePacks:[\"vanilla\",\"file/personal.zip\",\"file/remoto.zip\"]\r\nrenderDistance:8\r\n");
}

#[test]
fn base_cercana_al_limite_del_manifiesto_se_puede_volver_a_leer() {
    let juego = Carpeta::nueva();
    let fuente = Carpeta::nueva();
    fuente.escribir("config/a.toml", b"dato");
    let revision = remoto(&[("config/a.toml", b"dato")], 1);
    let mut ctx = contexto(&juego);
    ctx.limites.manifiesto_bytes = serde_json::to_vec(&revision).unwrap().len();
    tx::aplicar(
        preparar(&ctx, &revision, &fuente),
        &CancellationToken::new(),
        |_| {},
    )
    .unwrap();
    assert_eq!(tx::cargar_base(&ctx).unwrap().manifiesto, Some(revision));
}
