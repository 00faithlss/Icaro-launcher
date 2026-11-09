// SPDX-License-Identifier: GPL-3.0-only
use super::{invitado, Estado, Etapa};
use icaro_ui::compartir as v;

#[test]
fn cambiar_codigo_descarta_validacion_y_cancelar_no_acepta_respuesta_tardia() {
    let mut s = Estado::default();
    invitado::abrir(&mut s);
    let anterior = s.solicitud;
    s.invitado.entrada = v::EstadoEntrada::Validando;
    invitado::cambiar_codigo(&mut s, "invalido".into());
    assert_ne!(anterior, s.solicitud);
    assert!(matches!(s.invitado.entrada, v::EstadoEntrada::Invalido));
    assert!(!s.red_ocupada);
    s.cerrar();
    invitado::resuelta(&mut s, anterior, Err("respuesta anterior".into()));
    assert_eq!(s.etapa, Etapa::Cerrada);
    assert!(s.error.is_none());
}

#[test]
fn huella_distinta_no_avanza_sin_confirmacion_explicita() {
    let mut s = Estado::default();
    invitado::abrir(&mut s);
    s.etapa = Etapa::Identidad;
    s.invitado.confianza = v::Confianza::Distinto {
        fecha: "anterior".into(),
    };
    assert!(!invitado::puede_confiar(&s));
    s.invitado.confirmar_nueva = true;
    assert!(invitado::puede_confiar(&s));
}

#[test]
fn atras_desde_revision_vuelve_al_nombre_sin_crear_instancia() {
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
    app.servicios.compartir.etapa = Etapa::RevisionInvitado;
    let _ = super::actualizar(&mut app, super::Accion::Vista(v::Accion::Atras));
    assert_eq!(app.servicios.compartir.etapa, Etapa::Destino);
    assert!(app.servicios.compartir.invitado.destino.is_none());
}

#[test]
fn error_validando_destino_anterior_no_pierde_su_identidad() {
    let (mut app, _) = crate::iniciar_con_guardado(
        icaro_ui::shell::Seccion::Servidores,
        true,
        None,
        false,
        crate::servicios::Estado::nuevo(true),
        None,
    );
    let instancia = icaro_core::instancias::Instancia {
        id: "ab".repeat(16),
        nombre: "Pendiente".into(),
        minecraft: "1.21.1".into(),
        loader: "vanilla".into(),
        portada: 0,
        creada: 1,
    };
    app.servicios.compartir.solicitud = 1;
    app.servicios.compartir.etapa = Etapa::InstalandoInvitado;
    app.servicios.compartir.invitado.destino = Some(instancia.clone());
    invitado::instalada(
        &mut app,
        1,
        super::invitado_instalacion::Resultado {
            instancia: None,
            tarjeta: None,
            resultado: Err("No se pudo validar el vínculo".into()),
            revisar_nueva: false,
        },
    );
    assert_eq!(
        app.servicios.compartir.invitado.destino.as_ref(),
        Some(&instancia)
    );
    assert!(!app.servicios.compartir.invitado.completada);
}

#[test]
fn repegar_codigo_detecta_perfil_distinto_del_destino_retenido() {
    let (mut app, _) = crate::iniciar_con_guardado(
        icaro_ui::shell::Seccion::Servidores,
        true,
        None,
        false,
        crate::servicios::Estado::nuevo(true),
        None,
    );
    super::invitado_diseno::preparar(&mut app, "compartir-controlador-invitado-identidad");
    let s = &mut app.servicios.compartir;
    let visita = s.invitado.visita.take().unwrap();
    let mut perfil = visita.manifiesto.perfil.clone();
    perfil.loader_version = Some("0.15.0".into());
    s.invitado.destino = Some(icaro_core::instancias::Instancia {
        id: "ab".repeat(16),
        nombre: "Anterior".into(),
        minecraft: perfil.minecraft.clone(),
        loader: perfil.loader.clone(),
        portada: 0,
        creada: 1,
    });
    s.invitado.destino_vinculo = Some(icaro_core::compartir::vinculos::Vinculo {
        origen: visita.manifiesto.origen.clone(),
        perfil,
    });
    s.etapa = Etapa::Entrada;
    invitado::cambiar_codigo(s, "7QKM4XPTR2C9".into());
    invitado::visitada(s, s.solicitud, Ok(visita));
    assert!(s.invitado.cambio_perfil);
    assert!(s.invitado.destino.is_none());
    assert!(s.invitado.nombre.ends_with("(nueva)"));
}
