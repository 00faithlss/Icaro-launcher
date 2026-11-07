// SPDX-License-Identifier: GPL-3.0-only
use super::*;
#[test]
fn catalogo_reconoce_solo_los_bytes_exactos_y_no_cambia_una_revision_pospuesta() {
    use icaro_core::compartir::publicacion::{Borrador, Eleccion, Seleccion};
    let d = Carpeta::nueva();
    d.escribir("mods/visual.jar", b"visual");
    let inv = inventario::analizar(&d.0, &CancellationToken::new(), &Limites::default()).unwrap();
    let archivo = &inv.archivos[0];
    let mut borrador = Borrador::comparar(&inv, None, &Seleccion::default()).unwrap();
    borrador.elegir(&archivo.ruta, Eleccion::Compartir).unwrap();
    let mut version: icaro_core::modrinth::Version = serde_json::from_value(serde_json::json!({
        "id":"version1","project_id":"project1","name":"Visual","version_number":"1","version_type":"release",
        "game_versions":["1.21.1"],"loaders":["fabric"],"files":[{"filename":"visual.jar","url":"https://cdn.modrinth.com/visual.jar","size":archivo.bytes+1,"primary":true,"hashes":{"sha512":archivo.sha512}}],"dependencies":[],"changelog":null
    })).unwrap();
    assert!(borrador
        .reconocer_catalogo(&archivo.ruta, &version)
        .is_err());
    assert_eq!(borrador.autorizacion_pendiente().len(), 1);
    version.files[0].size = archivo.bytes;
    borrador
        .reconocer_catalogo(&archivo.ruta, &version)
        .unwrap();
    assert!(borrador.autorizacion_pendiente().is_empty());
    assert!(
        matches!(&borrador.archivos_publicados()[0].procedencia, icaro_core::compartir::Procedencia::Modrinth {proyecto,version} if proyecto=="project1" && version=="version1")
    );
    assert!(!borrador.archivos_publicados().is_empty());
}
#[test]
fn contenido_nuevo_es_privado_y_seleccion_privada_persiste() {
    use icaro_core::compartir::publicacion::{Borrador, Eleccion, Seleccion};
    let d = Carpeta::nueva();
    d.escribir("mods/visual.jar", b"solo para mi");
    let inv = inventario::analizar(&d.0, &CancellationToken::new(), &Limites::default()).unwrap();
    let mut seleccion = Seleccion::default();
    let mut borrador = Borrador::comparar(&inv, None, &seleccion).unwrap();
    assert!(borrador.archivos_publicados().is_empty());
    let ruta = RutaCompartida::parse("mods/visual.jar").unwrap();
    borrador.elegir(&ruta, Eleccion::Compartir).unwrap();
    assert_eq!(borrador.archivos_publicados().len(), 1);
    seleccion.mantener_privado(ruta.clone(), true);
    seleccion.guardar(&d.0).unwrap();
    let guardada = Seleccion::cargar(&d.0).unwrap();
    assert!(guardada.es_privado(&ruta));
    assert!(Borrador::comparar(&inv, None, &guardada)
        .unwrap()
        .archivos_publicados()
        .is_empty());
}

#[test]
fn posponer_actualizacion_y_retirada_conserva_revision_publicada() {
    use icaro_core::compartir::publicacion::{Borrador, Eleccion, Seleccion};
    let d = Carpeta::nueva();
    d.escribir("mods/ejemplo.jar", b"nuevo");
    let inv = inventario::analizar(&d.0, &CancellationToken::new(), &Limites::default()).unwrap();
    let base = Manifiesto::leer(
        &serde_json::to_vec(&manifiesto()).unwrap(),
        &Limites::default(),
    )
    .unwrap();
    let mut borrador = Borrador::comparar(&inv, Some(&base), &Seleccion::default()).unwrap();
    assert_ne!(
        borrador.archivos_publicados()[0].sha512,
        base.archivos[0].sha512
    );
    borrador
        .elegir(&base.archivos[0].ruta, Eleccion::Posponer)
        .unwrap();
    assert_eq!(borrador.archivos_publicados(), base.archivos);
    fs::remove_file(d.0.join("mods/ejemplo.jar")).unwrap();
    let vacio = inventario::analizar(&d.0, &CancellationToken::new(), &Limites::default()).unwrap();
    let mut borrador = Borrador::comparar(&vacio, Some(&base), &Seleccion::default()).unwrap();
    assert!(borrador.archivos_publicados().is_empty());
    borrador
        .elegir(&base.archivos[0].ruta, Eleccion::Posponer)
        .unwrap();
    assert_eq!(borrador.archivos_publicados(), base.archivos);
}
