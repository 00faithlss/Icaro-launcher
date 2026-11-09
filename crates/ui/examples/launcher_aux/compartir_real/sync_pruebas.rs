// SPDX-License-Identifier: GPL-3.0-only
use super::{sincronizar, Etapa};
use icaro_ui::compartir as v;

fn app() -> crate::App {
    let (mut app, _) = crate::iniciar_con_guardado(
        icaro_ui::shell::Seccion::Servidores,
        true,
        None,
        false,
        crate::servicios::Estado::nuevo(true),
        None,
    );
    app.servicios.demo = false;
    app.servicios.compartir.habilitado = true;
    app.servicios.instancias.registros = (1..=3)
        .map(|n| icaro_core::instancias::Instancia {
            id: format!("{n:032x}"),
            nombre: format!("Instancia {n}"),
            minecraft: "1.21.1".into(),
            loader: "fabric".into(),
            portada: 0,
            creada: 0,
        })
        .collect();
    app
}

#[test]
fn error_de_consulta_previa_a_jugar_no_encola_una_partida() {
    let mut app = app();
    let s = &mut app.servicios.compartir;
    s.etapa = Etapa::ConsultandoVinculo;
    s.solicitud = 7;
    s.sincronizar.jugar = true;
    let _ = sincronizar::recibida(&mut app, 7, Err("Anfitrión desconectado".into()));
    assert_eq!(app.servicios.compartir.etapa, Etapa::ErrorVinculo);
    assert!(app.servicios.juego.trabajos.is_empty());
    assert!(app.servicios.juego.procesos.is_empty());
}

#[test]
fn cerrar_aplicacion_espera_su_acuse_y_no_lanza_el_juego() {
    let mut app = app();
    let s = &mut app.servicios.compartir;
    s.etapa = Etapa::AplicandoVinculo;
    s.solicitud = 11;
    s.red_ocupada = true;
    s.sincronizar.jugar = true;
    s.cerrar();
    assert_eq!(s.etapa, Etapa::AplicandoVinculo);
    assert!(s.cerrar_al_terminar);
    assert!(s.cancel.is_cancelled());
    let _ = sincronizar::aplicada(&mut app, 11, Ok(()));
    assert_eq!(app.servicios.compartir.etapa, Etapa::Cerrada);
    assert!(!app.servicios.compartir.red_ocupada);
    assert!(app.servicios.juego.trabajos.is_empty());
}

#[test]
fn conflicto_permite_cambiar_de_eleccion_antes_de_aplicar() {
    let mut app = app();
    app.servicios.compartir.etapa = Etapa::RevisionVinculo;
    for e in [v::Resolucion::Anfitrion, v::Resolucion::ConservarMia] {
        let _ = sincronizar::accion(
            &mut app,
            v::Accion::ResolverConflicto("config/mod.toml".into(), e),
        );
        assert_eq!(
            app.servicios.compartir.sincronizar.elecciones["config/mod.toml"],
            e
        );
    }
    assert!(app.servicios.juego.trabajos.is_empty());
}

#[test]
fn jugar_tras_sincronizar_consulta_otra_vez_la_publicacion() {
    let mut app = app();
    let instancia = app.servicios.instancias.registros[0].clone();
    let s = &mut app.servicios.compartir;
    s.etapa = Etapa::RevisionVinculo;
    s.sincronizar.instancia = Some(instancia);
    s.sincronizar.completada = true;
    let _ = sincronizar::accion(&mut app, v::Accion::Jugar);
    assert_eq!(app.servicios.compartir.etapa, Etapa::ConsultandoVinculo);
    assert!(app.servicios.compartir.sincronizar.jugar);
    assert!(app.servicios.juego.trabajos.is_empty());
}

#[test]
fn cerrar_consulta_sync_espera_acuse_y_descarta_respuesta() {
    let mut app = app();
    let _ = sincronizar::abrir(&mut app, 0, true, None);
    let id = app.servicios.compartir.solicitud;
    app.servicios.compartir.cerrar();
    assert_eq!(app.servicios.compartir.etapa, Etapa::ConsultandoVinculo);
    assert!(app.servicios.compartir.red_ocupada);
    let _ = sincronizar::recibida(&mut app, id, Err("consulta cancelada".into()));
    assert_eq!(app.servicios.compartir.etapa, Etapa::Cerrada);
    assert!(app.servicios.compartir.sincronizar.error.is_none());
    assert!(app.servicios.juego.trabajos.is_empty());
    sincronizar::error(
        &mut app.servicios.compartir,
        id,
        "respuesta anterior".into(),
    );
    assert!(app.servicios.compartir.sincronizar.error.is_none());
}

#[test]
fn jugar_durante_instalacion_activa_o_en_cola_no_se_salta_la_revision() {
    for index in [0, 2] {
        let mut app = app();
        crate::juego_real::preparar_diseno(&mut app, "juego-descargas");
        assert!(sincronizar::antes_de_jugar(&mut app, index, true, None).is_some());
        let id = &app.servicios.instancias.registros[index].id;
        assert!(!app.servicios.juego.trabajos[id].jugar);
        assert_eq!(app.servicios.compartir.etapa, Etapa::Cerrada);
    }
}

#[test]
fn mods_en_otra_instancia_no_impiden_consultar_antes_de_jugar() {
    let mut app = app();
    app.servicios.mods.busy = Some(app.servicios.instancias.registros[1].id.clone());
    let _ = sincronizar::abrir(&mut app, 0, true, None);
    assert_eq!(app.servicios.compartir.etapa, Etapa::ConsultandoVinculo);
}

#[test]
fn resultado_revisado_no_se_fusiona_con_una_instalacion_que_aparecio_despues() {
    let mut app = app();
    crate::juego_real::preparar_diseno(&mut app, "juego-descargas");
    app.servicios.session = Some(
        icaro_core::auth::Session::local("PruebaSync")
            .unwrap()
            .into(),
    );
    let carpeta = std::env::temp_dir().join(format!(
        "icaro-guardia-handoff-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&carpeta).unwrap();
    let guardia =
        std::sync::Arc::new(icaro_core::actividad_instancia::Guardia::adquirir(&carpeta).unwrap());
    let _ = crate::juego_real::iniciar_revisado(&mut app, 0, true, None, Some(guardia.clone()));
    let id = &app.servicios.instancias.registros[0].id;
    let promovida = app.servicios.juego.trabajos[id].jugar;
    drop(guardia);
    std::fs::remove_file(carpeta.join(icaro_core::actividad_instancia::ARCHIVO)).unwrap();
    std::fs::remove_dir(carpeta).unwrap();
    assert!(
        !promovida,
        "el trabajo existente no posee la reserva revisada"
    );
}
