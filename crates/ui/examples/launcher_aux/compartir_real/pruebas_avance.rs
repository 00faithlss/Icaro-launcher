// SPDX-License-Identifier: GPL-3.0-only
use super::{
    avance::{self, Avance},
    Estado, Etapa,
};
use icaro_ui::compartir as v;

#[test]
fn progreso_pertenece_al_intento_activo_y_no_reabre_un_dialogo_cerrado() {
    let mut s = Estado::default();
    s.solicitud = 2;
    s.etapa = Etapa::InstalandoInvitado;
    let avance = Avance::nuevo(
        v::Fase::Descarga,
        2,
        5,
        "mods/uno.jar",
        "Archivos verificados",
    );
    avance::recibido(&mut s, 1, avance.clone());
    assert_eq!(s.invitado.avance.hechos, 0);
    avance::recibido(&mut s, 2, avance.clone());
    assert_eq!(s.invitado.avance.hechos, 2);
    assert_eq!(s.invitado.avance.porcentaje(), 40.);
    s.cerrar();
    avance::recibido(&mut s, 2, avance);
    assert_eq!(s.etapa, Etapa::Cerrada);
}

#[test]
fn progreso_sin_total_no_inventa_un_porcentaje_ni_divide_por_cero() {
    let avance = Avance::nuevo(v::Fase::Preparacion, 12, 0, "", "Comprobando perfil");
    assert_eq!(avance.porcentaje(), 0.);
    assert_eq!(
        Avance::nuevo(v::Fase::Aplicacion, 12, 3, "", "Movimientos durables").porcentaje(),
        100.
    );
}

#[test]
fn confianza_recupera_nombre_e_identidad_antes_de_revisar() {
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
    let i = icaro_core::instancias::Instancia {
        id: "cd".repeat(16),
        nombre: "Instalación pendiente".into(),
        minecraft: "1.21.1".into(),
        loader: "fabric".into(),
        portada: 0,
        creada: 1,
    };
    super::invitado::confiada(s, s.solicitud, Ok(Some(i.clone())));
    assert_eq!(s.etapa, Etapa::Destino);
    assert!(s.invitado.recuperando);
    assert_eq!(s.invitado.nombre, i.nombre);
    assert_eq!(s.invitado.destino, Some(i));
    assert!(s.invitado.destino_vinculo.is_some());
}
