// SPDX-License-Identifier: GPL-3.0-only
use super::*;
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
    app
}
#[test]
fn cerrar_sin_transferencias_no_muestra_dialogo_y_espera_acuse_de_instalacion() {
    let mut app = app();
    app.servicios.compartir.red_ocupada = true;
    app.servicios.compartir.etapa = compartir_real::Etapa::InstalandoInvitado;
    assert!(solicitar(&mut app).is_some());
    assert!(!app.cierre.solicitado);
    assert!(app.servicios.compartir.cancel.is_cancelled());
    assert!(!app.cierre.aprobado);
    let _ = actualizar(&mut app, Accion::Tic);
    assert!(!app.cierre.aprobado);
    app.servicios.compartir.red_ocupada = false;
    app.servicios.compartir.etapa = compartir_real::Etapa::ResultadoInvitado;
    let _ = actualizar(&mut app, Accion::Tic);
    let _ = actualizar(&mut app, Accion::Tic);
    assert!(app.cierre.aprobado);
}
#[test]
fn cancelar_dialogo_no_cancela_transferencias() {
    let mut app = app();
    app.cierre.solicitado = true;
    app.cierre.transferencias = 2;
    let _ = cancelar(&mut app);
    assert!(!app.cierre.solicitado);
    assert!(!app.servicios.compartir.cancel.is_cancelled());
    assert!(!app.cierre.esperando);
}
#[test]
fn cierre_no_se_completa_hasta_acuse_de_compartir_y_no_se_repite_drenaje() {
    let mut e = Estado::default();
    e.solicitado = true;
    e.esperando = true;
    assert!(!e.puede_terminar(true));
    e.host_cerrado = true;
    assert!(!e.puede_terminar(true));
    assert!(e.puede_terminar(false));
}
#[test]
fn error_de_bandeja_no_autoriza_ocultar_ventana() {
    let e = Estado::default();
    assert!(e.bandeja.is_none());
    assert!(!e.oculta);
    assert!(!e.aprobado);
}

#[test]
fn sin_partida_cerrar_no_espera_en_la_bandeja() {
    let mut app = app();
    assert!(ocultar_por_juego(&mut app).is_none());
    assert!(!app.cierre.por_juego);
}

#[test]
fn al_terminar_la_partida_en_la_bandeja_se_completa_el_cierre() {
    let mut app = app();
    // Ícaro esperaba en la bandeja y Minecraft ya se cerró.
    app.cierre.por_juego = true;
    let _ = actualizar(&mut app, Accion::Tic);
    assert!(!app.cierre.por_juego);
    assert!(app.cierre.forzar, "el cierre siguiente no vuelve a esperar");
    // Con `forzar`, otra solicitud no vuelve a ocultar la ventana.
    assert!(ocultar_por_juego(&mut app).is_none());
}

#[test]
fn sin_bandeja_se_minimiza_y_no_se_cierra_sola_despues() {
    let mut app = app();
    app.cierre.por_juego = true;
    let _ = actualizar(&mut app, Accion::Bandeja(Err("sin bandeja".into())));
    assert!(!app.cierre.por_juego);
    assert!(!app.cierre.forzar);
}
