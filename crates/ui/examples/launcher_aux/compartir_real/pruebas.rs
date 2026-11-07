// SPDX-License-Identifier: GPL-3.0-only
use super::{Estado, Etapa};
use icaro_ui::compartir as v;

fn app() -> crate::App {
    let (mut a, _) = crate::iniciar_con_guardado(
        icaro_ui::shell::Seccion::Servidores,
        true,
        None,
        false,
        crate::servicios::Estado::nuevo(true),
        None,
    );
    a.servicios.demo = false;
    a.servicios.compartir.habilitado = true;
    a
}
fn panel() -> v::PanelPublicacion {
    v::PanelPublicacion {
        instancia: Default::default(),
        revision: 1,
        fecha_revision: String::new(),
        hash_revision: String::new(),
        disponibilidad: String::new(),
        transferencias: String::new(),
        huella: String::new(),
        contenido_privado: 0,
        estado: v::EstadoPublicacion::Compartiendo,
        invitacion: Some(v::Invitacion {
            cupo: 10,
            aceptados: 5,
            estado: v::EstadoInvitacion::Vigente {
                restante: "1 hora".into(),
            },
            ..Default::default()
        }),
        accesos: Default::default(),
    }
}

#[test]
fn detener_sin_red_no_muestra_la_publicacion_como_activa() {
    let mut s = Estado::default();
    let id = s.comenzar("instancia".into());
    s.panel = Some(panel());
    s.etapa = Etapa::Panel;
    s.red_ocupada = true;
    super::resultados::operada(&mut s, id, super::Operacion::Detener, Err("Sin red".into()));
    assert!(matches!(
        s.panel.as_ref().unwrap().estado,
        v::EstadoPublicacion::Detenida
    ));
    assert!(s.anfitrion.is_none());
}

#[test]
fn cupo_no_puede_bajar_de_las_aceptaciones_confirmadas() {
    let mut a = app();
    a.servicios.compartir.panel = Some(panel());
    a.servicios.compartir.etapa = Etapa::Panel;
    let _ = super::actualizar(&mut a, super::Accion::Vista(v::Accion::Cupo(3)));
    assert_eq!(a.servicios.compartir.cupo, 10);
}

#[test]
fn error_guardando_privados_no_cierra_ni_descarta_la_revision() {
    let mut a = app();
    let id = a.servicios.compartir.comenzar("instancia".into());
    a.servicios.compartir.etapa = Etapa::Guardando;
    a.servicios.compartir.cerrar_al_terminar = true;
    a.servicios.compartir.revision = Some(v::RevisionPublicacion {
        instancia: Default::default(),
        base: 0,
        nueva: 1,
        cambios: vec![],
        estado: v::EstadoRevisionPublicacion::Disponible,
        confirmar_cancelacion: false,
    });
    let _ = super::actualizar(
        &mut a,
        super::Accion::Guardada(id, true, Err("Disco no disponible".into())),
    );
    assert_eq!(a.servicios.compartir.etapa, Etapa::Revision);
    assert!(matches!(
        a.servicios.compartir.revision.as_ref().unwrap().estado,
        v::EstadoRevisionPublicacion::Error(_)
    ));
}

#[test]
fn modo_demo_y_debug_no_disparan_el_servicio_ni_exponen_codigo() {
    let mut a = app();
    a.servicios.demo = true;
    let _ = super::actualizar(&mut a, super::Accion::Vista(v::Accion::AbrirCompartir));
    assert_eq!(a.servicios.compartir.etapa, Etapa::Cerrada);
    let mensaje = super::Accion::Vista(v::Accion::Codigo("7QKM4XPTR2C9".into()));
    assert!(!format!("{mensaje:?}").contains("7QKM"));
}

#[test]
fn cerrar_panel_conserva_datos_para_editar_invitacion_y_reabrir_publicacion() {
    let mut s = Estado::default();
    s.panel = Some(panel());
    let t = trabajo();
    s.publicacion = Some(super::tareas::PublicacionLocal::desde(&t));
    s.trabajo = Some(t);
    s.etapa = Etapa::Panel;
    s.cerrar();
    assert_eq!(s.etapa, Etapa::Cerrada);
    assert!(s.publicacion.is_some());
}

#[test]
fn modal_bloquea_navegacion_y_escape_cierra_solo_el_controlador() {
    let mut a = app();
    a.servicios.compartir.etapa = Etapa::Selector;
    let _ = crate::actualizar_estado(
        &mut a,
        crate::Mensaje::Ir(icaro_ui::shell::Seccion::Instancias),
    );
    assert_eq!(a.seccion, icaro_ui::shell::Seccion::Servidores);
    let _ = crate::actualizar_estado(&mut a, crate::Mensaje::Escape);
    assert_eq!(a.servicios.compartir.etapa, Etapa::Cerrada);
}

#[test]
fn una_eleccion_privada_espera_el_guardado_antes_de_decir_que_quedo_lista() {
    let mut a = app();
    let t = trabajo();
    let id = a.servicios.compartir.comenzar(t.instancia.id.clone());
    let _ = super::actualizar(&mut a, super::Accion::Analizada(id, Ok(t)));
    let _ = super::actualizar(
        &mut a,
        super::Accion::Vista(v::Accion::EleccionPublicacion(
            "mods/propio.jar".into(),
            v::Eleccion::Privado,
        )),
    );
    assert_eq!(a.servicios.compartir.etapa, Etapa::Guardando);
}

#[test]
fn cancelar_visible_durante_preparacion_cancela_y_espera_confirmacion() {
    let mut a = app();
    let id = a.servicios.compartir.comenzar("instancia".into());
    a.servicios.compartir.etapa = Etapa::Publicando;
    let cancel = a.servicios.compartir.cancel.clone();
    let _ = super::actualizar(
        &mut a,
        super::Accion::Vista(v::Accion::ConfirmarCancelarPublicacion),
    );
    assert!(cancel.is_cancelled());
    assert!(a.servicios.compartir.vigente(id));
    assert!(a.servicios.compartir.cerrar_al_terminar);
}

#[test]
fn fallo_al_reabrir_respeta_el_cierre_que_estaba_esperando_la_respuesta() {
    let mut a = app();
    let id = a.servicios.compartir.comenzar("instancia".into());
    a.servicios.compartir.etapa = Etapa::Panel;
    a.servicios.compartir.red_ocupada = true;
    a.servicios.compartir.cerrar();
    let _ = super::actualizar(&mut a, super::Accion::Reanudada(id, Err("Sin red".into())));
    assert_eq!(a.servicios.compartir.etapa, Etapa::Cerrada);
}

#[test]
fn panel_detenido_se_puede_reabrir_despues_de_descartar_otro_borrador() {
    use icaro_core::compartir::{revisiones, Manifiesto};
    let mut a = app();
    let mut primera = trabajo();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let raiz = std::env::temp_dir().join(format!("icaro-panel-{}-{nonce}", std::process::id()));
    std::fs::create_dir(&raiz).unwrap();
    primera.contexto.almacen = raiz.clone();
    let manifiesto = Manifiesto {
        esquema: "icaro-compartir/1".into(),
        origen: primera.contexto.origen.clone(),
        perfil: primera.contexto.perfil.clone(),
        revision: 1,
        archivos: vec![primera
            .borrador
            .archivo_actual(&primera.inventario.archivos[0].ruta)
            .unwrap()
            .clone()],
        packs: vec![],
    };
    std::fs::write(
        raiz.join("vigente.json"),
        serde_json::to_vec(&manifiesto).unwrap(),
    )
    .unwrap();
    let revision = revisiones::vigente(&primera.contexto).unwrap().unwrap();
    let id = a.servicios.compartir.comenzar(primera.instancia.id.clone());
    a.servicios.compartir.trabajo = Some(primera.clone());
    a.servicios.compartir.etapa = Etapa::Publicando;
    let _ = super::actualizar(
        &mut a,
        super::Accion::Publicada(
            id,
            Ok(super::tareas::Publicada {
                revision,
                anfitrion: Err("Detenida para probar".into()),
            }),
        ),
    );
    let _ = super::actualizar(&mut a, super::Accion::Vista(v::Accion::Cerrar));
    let mut segunda = trabajo();
    segunda.instancia.id = "ef".repeat(16);
    segunda.instancia.nombre = "Otra instancia".into();
    let id = a.servicios.compartir.comenzar(segunda.instancia.id.clone());
    let _ = super::actualizar(&mut a, super::Accion::Analizada(id, Ok(segunda)));
    let _ = super::actualizar(&mut a, super::Accion::Vista(v::Accion::CerrarSinPublicar));
    let _ = super::actualizar(&mut a, super::Accion::Vista(v::Accion::VerPublicacion));
    let _ = super::actualizar(&mut a, super::Accion::Vista(v::Accion::VolverACompartir));
    assert!(
        a.servicios.compartir.red_ocupada,
        "Debe poder reabrir la publicación A, independiente del borrador B"
    );
    assert_eq!(
        a.servicios.compartir.panel.as_ref().unwrap().instancia.id,
        primera.instancia.id
    );
    std::fs::remove_file(raiz.join("vigente.json")).unwrap();
    std::fs::remove_dir(raiz).unwrap();
}

#[test]
fn cerrar_y_reabrir_descarta_respuestas_de_la_revision_anterior() {
    let mut estado = Estado::default();
    let anterior = estado.comenzar("anterior".into());
    let cancel = estado.cancel.clone();
    estado.cerrar();
    assert!(cancel.is_cancelled());
    assert!(!estado.vigente(anterior));
    let nueva = estado.comenzar("nueva".into());
    assert!(!estado.vigente(anterior));
    assert!(estado.vigente(nueva));
    assert_eq!(estado.seleccionada.as_deref(), Some("nueva"));
}

#[test]
fn cerrar_publicacion_espera_resultado_y_no_permite_otro_intento_simultaneo() {
    let mut estado = Estado::default();
    let solicitud = estado.comenzar("instancia".into());
    estado.etapa = Etapa::Publicando;
    estado.cerrar();
    assert!(estado.cancel.is_cancelled());
    assert!(estado.cerrar_al_terminar);
    assert!(estado.vigente(solicitud));
    assert!(matches!(estado.etapa, Etapa::Publicando));
    assert_eq!(estado.comenzar("otra".into()), solicitud);
    assert_eq!(estado.seleccionada.as_deref(), Some("instancia"));
}

fn trabajo() -> super::tareas::Trabajo {
    use icaro_core::{
        compartir::{
            self as c,
            identidad::{AlmacenClave, Identidad, Zeroizing},
            publicacion::{Borrador, Seleccion},
        },
        instancias::Instancia,
    };
    struct Clave;
    impl AlmacenClave for Clave {
        fn leer(&self) -> Result<Option<Zeroizing<Vec<u8>>>, c::Error> {
            Ok(Some(Zeroizing::new(vec![19; 32])))
        }
        fn guardar(&self, _: &[u8]) -> Result<(), c::Error> {
            Err(c::Error::Credencial)
        }
    }
    let identidad = Identidad::desde_almacen(&Clave).unwrap();
    let inventario = c::inventario::Inventario {
        archivos: vec![c::inventario::ArchivoInventario {
            ruta: c::RutaCompartida::parse("mods/propio.jar").unwrap(),
            bytes: 3,
            sha512: "ab".repeat(64),
            sha256: "cd".repeat(32),
        }],
        bytes: 3,
        bloqueados: vec![],
    };
    let seleccion = Seleccion::default();
    let borrador = Borrador::comparar(&inventario, None, &seleccion).unwrap();
    super::tareas::Trabajo {
        instancia: Instancia {
            id: "ab".repeat(16),
            nombre: "Fixture".into(),
            minecraft: "1.21.1".into(),
            loader: "vanilla".into(),
            portada: 0,
            creada: 0,
        },
        contexto: c::revisiones::ContextoPublicacion {
            juego: "fixture/juego".into(),
            almacen: "fixture/publicacion".into(),
            origen: c::Origen {
                anfitrion: identidad.clave_publica(),
                publicacion: "cd".repeat(16),
            },
            perfil: c::Perfil {
                minecraft: "1.21.1".into(),
                loader: "vanilla".into(),
                loader_version: None,
            },
            limites: Default::default(),
        },
        identidad,
        inventario,
        borrador,
        seleccion,
        anterior: None,
        activos: vec![],
        aviso: None,
        orden_pospuesto: false,
    }
}

#[test]
fn propios_necesitan_permiso_expreso_y_una_respuesta_vieja_no_lo_transfiere() {
    let mut a = app();
    let t = trabajo();
    let id = a.servicios.compartir.comenzar(t.instancia.id.clone());
    let _ = super::actualizar(&mut a, super::Accion::Analizada(id, Ok(t.clone())));
    let _ = super::actualizar(
        &mut a,
        super::Accion::Vista(v::Accion::EleccionPublicacion(
            "mods/propio.jar".into(),
            v::Eleccion::Compartir,
        )),
    );
    let _ = super::actualizar(&mut a, super::Accion::Guardada(id, false, Ok(())));
    let _ = super::actualizar(&mut a, super::Accion::Vista(v::Accion::PublicarRevision));
    assert_eq!(a.servicios.compartir.etapa, Etapa::Propios);
    let _ = super::actualizar(&mut a, super::Accion::Vista(v::Accion::AutorizarPropios));
    assert_eq!(
        a.servicios
            .compartir
            .trabajo
            .as_ref()
            .unwrap()
            .borrador
            .autorizacion_pendiente()
            .len(),
        1
    );
    let _ = super::actualizar(
        &mut a,
        super::Accion::Vista(v::Accion::PermisoPropios(true)),
    );
    let _ = super::actualizar(&mut a, super::Accion::Vista(v::Accion::AutorizarPropios));
    assert!(a
        .servicios
        .compartir
        .trabajo
        .as_ref()
        .unwrap()
        .borrador
        .autorizacion_pendiente()
        .is_empty());
    let nuevo = a.servicios.compartir.comenzar(t.instancia.id.clone());
    let _ = super::actualizar(&mut a, super::Accion::Analizada(id, Ok(t.clone())));
    assert!(a.servicios.compartir.trabajo.is_none());
    let _ = super::actualizar(&mut a, super::Accion::Analizada(nuevo, Ok(t)));
    assert!(!a.servicios.compartir.permiso_propios);
}

#[tokio::test]
async fn guardar_privados_del_controlador_persiste_sin_publicar_revision() {
    use icaro_core::compartir::{
        publicacion::{Eleccion, Seleccion},
        RutaCompartida,
    };
    let mut t = trabajo();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let raiz =
        std::env::temp_dir().join(format!("icaro-controlador-{}-{nonce}", std::process::id()));
    std::fs::create_dir(&raiz).unwrap();
    t.contexto.juego = raiz.clone();
    t.elegir("mods/propio.jar", Eleccion::Privado).unwrap();
    super::tareas::guardar_seleccion(t).await.unwrap();
    assert!(Seleccion::cargar(&raiz)
        .unwrap()
        .es_privado(&RutaCompartida::parse("mods/propio.jar").unwrap()));
    assert!(!raiz.join(".icaro-compartir-base.json").exists());
    std::fs::remove_file(raiz.join(".icaro-compartir-privado.json")).unwrap();
    std::fs::remove_dir(raiz).unwrap();
}
