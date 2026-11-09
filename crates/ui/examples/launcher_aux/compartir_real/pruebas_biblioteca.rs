// SPDX-License-Identifier: GPL-3.0-only
//! Biblioteca del publicador, recuperación tras reiniciar, reconexión y contadores.
//! Contrato previo a la implementación: ver `publicador-alcance.md` del informe.
use super::{biblioteca, biblioteca_tareas, contadores, tareas::PublicacionLocal, Estado, Etapa};
use icaro_core::{
    compartir::{
        self as c,
        biblioteca::{EstadoGuardada, PublicacionGuardada},
        identidad::{AlmacenClave, Identidad, Zeroizing},
        revisiones::{self, ContextoPublicacion, Revision},
    },
    instancias::{Almacen, Instancia},
};
use icaro_ui::compartir as v;
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

static SECUENCIA: AtomicU64 = AtomicU64::new(0);
struct Temporal(PathBuf);
impl Temporal {
    fn nuevo() -> Self {
        let ruta = std::env::temp_dir().join(format!(
            "icaro-biblioteca-test-{}-{}",
            std::process::id(),
            SECUENCIA.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&ruta).unwrap();
        Self(ruta)
    }
}
impl Drop for Temporal {
    fn drop(&mut self) {
        if self.0.parent() == Some(std::env::temp_dir().as_path())
            && self
                .0
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("icaro-biblioteca-test-")
        {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

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
fn identidad() -> Identidad {
    struct Clave;
    impl AlmacenClave for Clave {
        fn leer(&self) -> Result<Option<Zeroizing<Vec<u8>>>, c::Error> {
            Ok(Some(Zeroizing::new(vec![23; 32])))
        }
        fn guardar(&self, _: &[u8]) -> Result<(), c::Error> {
            Err(c::Error::Credencial)
        }
    }
    Identidad::desde_almacen(&Clave).unwrap()
}
fn instancia(id: &str, nombre: &str) -> Instancia {
    Instancia {
        id: id.into(),
        nombre: nombre.into(),
        minecraft: "1.21.1".into(),
        loader: "vanilla".into(),
        portada: 0,
        creada: 0,
    }
}
fn entrada(id: &str, revision: Option<u64>, estado: EstadoGuardada) -> PublicacionGuardada {
    PublicacionGuardada {
        instancia: id.into(),
        origen: Some(c::Origen {
            anfitrion: identidad().clave_publica(),
            publicacion: format!("{:0>32}", id.len()),
        }),
        revision,
        archivos: 1,
        autorizados: 2,
        estado,
    }
}
fn panel(id: &str) -> v::PanelPublicacion {
    v::PanelPublicacion {
        instancia: v::InstanciaCompartida {
            id: id.into(),
            nombre: "Activa".into(),
            ..Default::default()
        },
        revision: 4,
        fecha_revision: String::new(),
        hash_revision: String::new(),
        disponibilidad: String::new(),
        transferencias: String::new(),
        huella: String::new(),
        contenido_privado: 0,
        estado: v::EstadoPublicacion::Compartiendo,
        invitacion: Some(v::Invitacion {
            codigo: "7QKM-4XPT-R2C9".into(),
            cupo: 10,
            aceptados: 1,
            estado: v::EstadoInvitacion::Vigente {
                restante: "24 horas".into(),
            },
            ..Default::default()
        }),
        accesos: v::Accesos {
            personas: vec![acceso(&"11".repeat(32))],
            ..Default::default()
        },
    }
}
fn acceso(k: &str) -> v::Acceso {
    v::Acceso {
        id: k.into(),
        nombre: "Equipo 1".into(),
        huella: format!("Clave {}", &k[..12]),
        estado: "Autorizado".into(),
        conectado: false,
        aceptado: String::new(),
    }
}
/// Contexto real en disco con una revisión vigente escrita como la publica el núcleo.
fn contexto(raiz: &Path, numero: u64) -> (ContextoPublicacion, Revision) {
    let ctx = ContextoPublicacion {
        juego: raiz.join("juego"),
        almacen: raiz.join("publicacion"),
        origen: c::Origen {
            anfitrion: identidad().clave_publica(),
            publicacion: "cd".repeat(16),
        },
        perfil: c::Perfil {
            minecraft: "1.21.1".into(),
            loader: "vanilla".into(),
            loader_version: None,
        },
        limites: Default::default(),
    };
    escribir_vigente(&ctx, numero);
    let revision = revisiones::vigente(&ctx).unwrap().unwrap();
    (ctx, revision)
}
fn escribir_vigente(ctx: &ContextoPublicacion, numero: u64) {
    std::fs::create_dir_all(&ctx.juego).unwrap();
    std::fs::create_dir_all(&ctx.almacen).unwrap();
    let manifiesto = serde_json::json!({
        "esquema": "icaro-compartir/1",
        "origen": serde_json::to_value(&ctx.origen).unwrap(),
        "revision": numero,
        "perfil": serde_json::to_value(&ctx.perfil).unwrap(),
        "archivos": [{
            "ruta": "mods/ejemplo.jar",
            "bytes": 3,
            "sha512": "483365784455682e7e83c6e150e56dbe3f387a06c9d407b8253e4bdf8e0bd1fac8234e3973d4e59ff8c01cb969f25c0d08c5dc5bb967744b1a5813afe3c58fcc",
            "procedencia": {"tipo": "propio"}
        }],
        "packs": []
    });
    std::fs::write(
        ctx.almacen.join("vigente.json"),
        serde_json::to_vec(&manifiesto).unwrap(),
    )
    .unwrap();
}
fn local(id: &str, ctx: ContextoPublicacion) -> PublicacionLocal {
    PublicacionLocal {
        instancia_id: id.into(),
        contexto: ctx,
        identidad: identidad(),
    }
}
fn contenido(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut v: Vec<_> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| {
            let p = e.unwrap().path();
            let bytes = if p.is_file() {
                std::fs::read(&p).unwrap()
            } else {
                vec![]
            };
            (p, bytes)
        })
        .collect();
    v.sort();
    v
}

// --- Lista persistida -------------------------------------------------------

#[test]
fn respuesta_vieja_de_la_lista_se_descarta_y_un_error_conserva_la_anterior() {
    let mut s = Estado::default();
    let vieja = biblioteca::pedir(&mut s);
    let nueva = biblioteca::pedir(&mut s);
    assert_ne!(vieja, nueva);
    biblioteca::recibida(
        &mut s,
        vieja,
        Ok(vec![entrada("aa", Some(1), EstadoGuardada::Lista)]),
    );
    assert!(s.biblioteca.entradas.is_empty());
    assert!(s.biblioteca.cargando);
    biblioteca::recibida(
        &mut s,
        nueva,
        Ok(vec![entrada("aa", Some(1), EstadoGuardada::Lista)]),
    );
    assert_eq!(s.biblioteca.entradas.len(), 1);
    assert!(!s.biblioteca.cargando);
    let fallida = biblioteca::pedir(&mut s);
    biblioteca::recibida(&mut s, fallida, Err("Disco no disponible".into()));
    assert_eq!(s.biblioteca.entradas.len(), 1, "un fallo no vacía la lista");
    assert!(s.biblioteca.error.is_some());
    assert!(!s.biblioteca.cargando);
}

#[test]
fn cerrar_el_controlador_conserva_la_biblioteca() {
    let mut s = Estado::default();
    let g = biblioteca::pedir(&mut s);
    biblioteca::recibida(
        &mut s,
        g,
        Ok(vec![entrada("aa", Some(1), EstadoGuardada::Lista)]),
    );
    s.etapa = Etapa::Panel;
    s.cerrar();
    assert_eq!(s.biblioteca.entradas.len(), 1);
}

#[test]
fn resumenes_muestran_solo_publicaciones_reales_y_prefieren_datos_en_memoria() {
    let mut s = Estado::default();
    let g = biblioteca::pedir(&mut s);
    biblioteca::recibida(
        &mut s,
        g,
        Ok(vec![
            entrada("aa", Some(3), EstadoGuardada::Lista),
            entrada("bb", None, EstadoGuardada::SinRevision),
            entrada("cc", Some(1), EstadoGuardada::Detenida),
            PublicacionGuardada {
                origen: None,
                revision: None,
                ..entrada("dd", None, EstadoGuardada::Ilegible)
            },
            entrada("ee", Some(5), EstadoGuardada::Lista),
            entrada("ff", Some(2), EstadoGuardada::PerfilCambiado),
        ]),
    );
    let registros = [
        instancia("aa", "Alfa"),
        instancia("bb", "Beta"),
        instancia("cc", "Gamma"),
        instancia("dd", "Delta"),
        instancia("ff", "Zeta"),
        instancia("gg", "Nueva"),
    ];
    let r = biblioteca::resumenes(&s, &registros);
    let nombres: Vec<_> = r.iter().map(|r| r.instancia.as_str()).collect();
    assert_eq!(r.len(), 3, "{nombres:?}");
    for esperado in ["Alfa", "Gamma", "Zeta"] {
        assert!(nombres.contains(&esperado), "{nombres:?}");
    }
    let alfa = r.iter().find(|r| r.instancia == "Alfa").unwrap();
    assert_eq!(
        (alfa.revision, alfa.sesiones, alfa.transferencias),
        (3, 2, 0)
    );

    // La publicación abierta en esta sesión usa los datos vivos del panel y no se duplica.
    let mut p = panel("aa");
    p.instancia.nombre = "Alfa".into();
    s.panel = Some(p);
    let r = biblioteca::resumenes(&s, &registros);
    let alfas: Vec<_> = r.iter().filter(|r| r.instancia == "Alfa").collect();
    assert_eq!(alfas.len(), 1);
    assert_eq!((alfas[0].revision, alfas[0].sesiones), (4, 1));

    // Una publicación recién creada aparece aunque la lista aún no se haya releído.
    let mut p = panel("gg");
    p.instancia.nombre = "Nueva".into();
    s.panel = Some(p);
    let r = biblioteca::resumenes(&s, &registros);
    assert!(r.iter().any(|r| r.instancia == "Nueva"));
    assert_eq!(r.iter().filter(|r| r.instancia == "Alfa").count(), 1);
}

#[test]
fn diagnostico_de_la_biblioteca_no_expone_nombres_ni_codigos() {
    let mut a = app();
    a.servicios
        .compartir
        .biblioteca
        .entradas
        .push(entrada("aa", Some(1), EstadoGuardada::Lista));
    a.servicios
        .instancias
        .registros
        .push(instancia("aa", "Nombre privado"));
    let _ = biblioteca::abrir(&mut a, "aa".into());
    let texto = format!("{:?}", a.servicios.compartir.biblioteca);
    assert!(!texto.contains("Nombre privado"), "{texto}");
    assert!(!texto.contains("7QKM"), "{texto}");
}

// --- Abrir una publicación guardada ----------------------------------------

#[test]
fn abrir_sin_revision_o_ilegible_no_abre_panel_ni_inicia_trabajo() {
    let mut a = app();
    let s = &mut a.servicios.compartir;
    s.biblioteca
        .entradas
        .push(entrada("bb", None, EstadoGuardada::SinRevision));
    s.biblioteca.entradas.push(PublicacionGuardada {
        origen: None,
        ..entrada("dd", None, EstadoGuardada::Ilegible)
    });
    a.servicios
        .instancias
        .registros
        .push(instancia("bb", "Beta"));
    a.servicios
        .instancias
        .registros
        .push(instancia("dd", "Delta"));
    for id in ["bb", "dd", "zz"] {
        let _ = biblioteca::abrir(&mut a, id.into());
        assert_eq!(a.servicios.compartir.etapa, Etapa::Cerrada, "{id}");
        assert!(!a.servicios.compartir.red_ocupada, "{id}");
    }
}

#[test]
fn abrir_durante_otra_operacion_no_mezcla_publicaciones() {
    let mut a = app();
    a.servicios
        .instancias
        .registros
        .push(instancia("aa", "Alfa"));
    let s = &mut a.servicios.compartir;
    s.biblioteca
        .entradas
        .push(entrada("aa", Some(3), EstadoGuardada::Lista));
    s.panel = Some(panel("otra"));
    s.etapa = Etapa::Panel;
    s.red_ocupada = true;
    let _ = biblioteca::abrir(&mut a, "aa".into());
    let s = &a.servicios.compartir;
    assert_eq!(s.panel.as_ref().unwrap().instancia.id, "otra");
    assert_ne!(s.seleccionada.as_deref(), Some("aa"));
}

#[test]
fn abrir_la_publicacion_ya_cargada_reutiliza_memoria_sin_tareas() {
    let raiz = Temporal::nuevo();
    let (ctx, _) = contexto(&raiz.0, 4);
    let mut a = app();
    a.servicios
        .instancias
        .registros
        .push(instancia("aa", "Alfa"));
    let s = &mut a.servicios.compartir;
    s.biblioteca
        .entradas
        .push(entrada("aa", Some(4), EstadoGuardada::Lista));
    s.panel = Some(panel("aa"));
    s.publicacion = Some(local("aa", ctx));
    let _ = biblioteca::abrir(&mut a, "aa".into());
    let s = &a.servicios.compartir;
    assert_eq!(s.etapa, Etapa::Panel);
    assert!(!s.red_ocupada, "no hace falta leer disco ni reconectar");
    assert_eq!(s.panel.as_ref().unwrap().accesos.personas.len(), 1);
}

#[test]
fn abrir_otra_publicacion_muestra_su_panel_mientras_recupera_los_datos() {
    let mut a = app();
    a.servicios
        .instancias
        .registros
        .push(instancia("aa", "Alfa"));
    a.servicios.compartir.biblioteca.entradas.extend([
        entrada("aa", Some(3), EstadoGuardada::Lista),
        entrada("cc", Some(1), EstadoGuardada::Detenida),
    ]);
    a.servicios
        .instancias
        .registros
        .push(instancia("cc", "Gamma"));
    let _ = biblioteca::abrir(&mut a, "aa".into());
    let s = &a.servicios.compartir;
    assert_eq!(s.etapa, Etapa::Panel);
    assert!(
        s.red_ocupada,
        "bloquea acciones hasta tener identidad y contexto"
    );
    assert_eq!(s.seleccionada.as_deref(), Some("aa"));
    let p = s.panel.as_ref().unwrap();
    assert_eq!(
        (p.instancia.id.as_str(), p.instancia.nombre.as_str()),
        ("aa", "Alfa")
    );
    assert_eq!(p.revision, 3);
    assert!(p.invitacion.is_none(), "nunca se muestra un código antiguo");
    assert!(!matches!(p.estado, v::EstadoPublicacion::Detenida));

    let mut a = app();
    a.servicios
        .instancias
        .registros
        .push(instancia("cc", "Gamma"));
    a.servicios.compartir.biblioteca.entradas.push(entrada(
        "cc",
        Some(1),
        EstadoGuardada::Detenida,
    ));
    let _ = biblioteca::abrir(&mut a, "cc".into());
    let p = a.servicios.compartir.panel.as_ref().unwrap();
    assert!(matches!(p.estado, v::EstadoPublicacion::Detenida));
}

// --- Recuperación desde disco ---------------------------------------------

#[test]
fn recuperada_activa_pide_reconectar_y_detenida_solo_permite_volver_a_compartir() {
    let raiz = Temporal::nuevo();
    let (ctx, revision) = contexto(&raiz.0, 3);
    for detenida in [false, true] {
        let mut a = app();
        a.servicios
            .instancias
            .registros
            .push(instancia("aa", "Alfa"));
        let estado = if detenida {
            EstadoGuardada::Detenida
        } else {
            EstadoGuardada::Lista
        };
        a.servicios
            .compartir
            .biblioteca
            .entradas
            .push(entrada("aa", Some(3), estado));
        let _ = biblioteca::abrir(&mut a, "aa".into());
        let s = &mut a.servicios.compartir;
        let id = s.solicitud;
        let reconectar = biblioteca::recuperada(
            s,
            id,
            Ok(biblioteca_tareas::Recuperada {
                local: local("aa", ctx.clone()),
                revision: revision.clone(),
                detenida,
            }),
        );
        assert_eq!(reconectar, !detenida);
        assert_eq!(s.red_ocupada, !detenida);
        assert_eq!(
            s.publicacion.as_ref().map(|p| p.instancia_id.as_str()),
            Some("aa")
        );
        let p = s.panel.as_ref().unwrap();
        assert_eq!(p.revision, 3);
        assert_eq!(p.huella, identidad().huella());
        assert_eq!(matches!(p.estado, v::EstadoPublicacion::Detenida), detenida);
        assert!(s.anfitrion.is_none());
    }
}

#[test]
fn recuperacion_vieja_o_fallida_no_cambia_otra_publicacion() {
    let raiz = Temporal::nuevo();
    let (ctx, revision) = contexto(&raiz.0, 3);
    let mut a = app();
    a.servicios
        .instancias
        .registros
        .push(instancia("aa", "Alfa"));
    a.servicios
        .compartir
        .biblioteca
        .entradas
        .push(entrada("aa", Some(3), EstadoGuardada::Lista));
    let _ = biblioteca::abrir(&mut a, "aa".into());
    let s = &mut a.servicios.compartir;
    let vieja = s.solicitud.wrapping_sub(1);
    assert!(!biblioteca::recuperada(
        s,
        vieja,
        Ok(biblioteca_tareas::Recuperada {
            local: local("aa", ctx.clone()),
            revision: revision.clone(),
            detenida: false,
        }),
    ));
    assert!(s.publicacion.is_none());
    assert!(s.red_ocupada, "la recuperación vigente sigue pendiente");

    // Cerrar durante la lectura espera su resultado y después no reconecta.
    s.cerrar();
    assert!(s.cerrar_al_terminar);
    let id = s.solicitud;
    assert!(!biblioteca::recuperada(
        s,
        id,
        Ok(biblioteca_tareas::Recuperada {
            local: local("aa", ctx),
            revision,
            detenida: false,
        }),
    ));
    assert_eq!(s.etapa, Etapa::Cerrada);
    assert!(!s.red_ocupada);

    let _ = biblioteca::abrir(&mut a, "aa".into());
    let s = &mut a.servicios.compartir;
    let id = s.solicitud;
    assert!(!biblioteca::recuperada(
        s,
        id,
        Err("Credencial no disponible".into())
    ));
    assert!(!s.red_ocupada);
    assert!(matches!(
        s.panel.as_ref().unwrap().estado,
        v::EstadoPublicacion::Error(_)
    ));
}

#[tokio::test]
async fn recuperar_reutiliza_el_origen_persistido_y_nunca_crea_otro() {
    let raiz = Temporal::nuevo();
    let almacen = Almacen::nuevo(raiz.0.join("instances"));
    let identidad = identidad();
    let con = almacen.crear("Con publicación", "1.21.1", 0).unwrap();
    let sin = almacen.crear("Sin publicación", "1.21.1", 0).unwrap();
    let ctx =
        c::vinculos::publicacion(&raiz.0, &almacen, &con, &identidad.clave_publica()).unwrap();
    escribir_vigente(&ctx, 2);
    let r = biblioteca_tareas::recuperar_con(
        raiz.0.clone(),
        almacen.clone(),
        con.clone(),
        Some(identidad.clone()),
    )
    .await
    .unwrap();
    assert_eq!(r.local.contexto.origen, ctx.origen);
    assert_eq!(r.local.instancia_id, con.id);
    assert_eq!(r.revision.manifiesto.revision, 2);
    assert!(!r.detenida);

    let juego = almacen.juego(&sin.id).unwrap();
    let antes = contenido(&juego);
    let error =
        biblioteca_tareas::recuperar_con(raiz.0.clone(), almacen.clone(), sin, Some(identidad))
            .await;
    assert!(error.is_err());
    assert_eq!(contenido(&juego), antes, "no se crea otra publicación");
}

// --- Reconexión tras reiniciar --------------------------------------------

#[test]
fn reabrir_tras_cerrar_durante_recuperacion_reconecta_la_publicacion_guardada() {
    let raiz = Temporal::nuevo();
    let (ctx, revision) = contexto(&raiz.0, 3);
    let origen = ctx.origen.clone();
    let mut a = app();
    a.servicios
        .instancias
        .registros
        .push(instancia("aa", "Alfa"));
    a.servicios
        .compartir
        .biblioteca
        .entradas
        .push(entrada("aa", Some(3), EstadoGuardada::Lista));
    let _ = biblioteca::abrir(&mut a, "aa".into());
    let s = &mut a.servicios.compartir;
    let id = s.solicitud;
    s.cerrar();
    assert!(!biblioteca::recuperada(
        s,
        id,
        Ok(biblioteca_tareas::Recuperada {
            local: local("aa", ctx),
            revision,
            detenida: false,
        })
    ));
    assert_eq!(s.etapa, Etapa::Cerrada);
    assert!(!s.red_ocupada);

    let _ = biblioteca::abrir(&mut a, "aa".into());
    let s = &a.servicios.compartir;
    assert_eq!(s.etapa, Etapa::Panel);
    assert!(
        s.red_ocupada,
        "la recuperación aplazada se reconecta al reabrir"
    );
    assert!(
        s.trabajo.is_none(),
        "no vuelve a analizar ni crea otro origen"
    );
    assert_eq!(s.publicacion.as_ref().unwrap().contexto.origen, origen);
    assert!(s.panel.as_ref().unwrap().invitacion.is_none());
}

#[tokio::test]
async fn reconectar_sin_servicio_o_detenida_no_toca_accesos_ni_revoca() {
    use super::servicio::Servicio;
    use icaro_core::compartir::{accesos::Accesos, transporte::Modo};
    let raiz = Temporal::nuevo();
    let (ctx, _) = contexto(&raiz.0, 2);
    let antes = contenido(&ctx.almacen);
    let r = biblioteca_tareas::reconectar_con(
        local("aa", ctx.clone()),
        v::Caducidad::Horas24,
        10,
        None,
    )
    .await;
    assert!(r.err().unwrap().contains("no está configurado"));
    assert_eq!(contenido(&ctx.almacen), antes);

    let accesos = Accesos::abrir(ctx.almacen.join("accesos.json")).unwrap();
    accesos.autorizar(&"11".repeat(32)).unwrap();
    accesos.detener().unwrap();
    let antes = contenido(&ctx.almacen);
    let r = biblioteca_tareas::reconectar_con(
        local("aa", ctx.clone()),
        v::Caducidad::Horas24,
        10,
        Some(Servicio {
            url: "http://127.0.0.1:9/".into(),
            modo: Modo::Local,
        }),
    )
    .await;
    let error = r.err().expect("una publicación detenida no se reconecta");
    assert!(error.contains("Volver a compartir"), "{error}");
    assert_eq!(
        contenido(&ctx.almacen),
        antes,
        "reconectar nunca reinicia ni borra accesos"
    );
}

#[test]
fn fallo_al_reconectar_conserva_accesos_y_permite_reintentar() {
    let raiz = Temporal::nuevo();
    let (ctx, _) = contexto(&raiz.0, 4);
    let mut a = app();
    a.servicios
        .instancias
        .registros
        .push(instancia("aa", "Alfa"));
    let s = &mut a.servicios.compartir;
    s.biblioteca
        .entradas
        .push(entrada("aa", Some(4), EstadoGuardada::Lista));
    let id = s.comenzar("aa".into());
    s.etapa = Etapa::Panel;
    s.panel = Some(panel("aa"));
    s.publicacion = Some(local("aa", ctx));
    s.red_ocupada = true;
    biblioteca::reconectada(s, id, Err("Sin red".into()));
    assert!(!s.red_ocupada);
    assert!(s.anfitrion.is_none());
    let p = s.panel.as_ref().unwrap();
    assert!(matches!(p.estado, v::EstadoPublicacion::Error(_)));
    assert_eq!(
        p.accesos.personas.len(),
        1,
        "no se borran accesos aceptados"
    );

    // Reintentar en este panel reconecta; no vuelve a analizar la instancia.
    let _ = super::actualizar(&mut a, super::Accion::Vista(v::Accion::Reintentar));
    let s = &a.servicios.compartir;
    assert_eq!(s.etapa, Etapa::Panel);
    assert!(s.red_ocupada);
    assert!(s.trabajo.is_none());
}

#[test]
fn fallo_al_reconectar_respeta_un_cierre_pendiente() {
    let mut s = Estado::default();
    let id = s.comenzar("aa".into());
    s.etapa = Etapa::Panel;
    s.panel = Some(panel("aa"));
    s.red_ocupada = true;
    s.cerrar();
    assert!(s.cerrar_al_terminar);
    biblioteca::reconectada(&mut s, id, Err("Sin red".into()));
    assert_eq!(s.etapa, Etapa::Cerrada);
}

// --- Contadores del panel -------------------------------------------------

fn panel_con_contadores() -> (Estado, u64, u64) {
    let mut s = Estado::default();
    let id = s.comenzar("aa".into());
    s.etapa = Etapa::Panel;
    s.panel = Some(panel("aa"));
    s.biblioteca.contadores = 7;
    (s, id, 7)
}

#[test]
fn contadores_actualizan_transferencias_accesos_e_invitacion() {
    let (mut s, id, g) = panel_con_contadores();
    s.panel.as_mut().unwrap().accesos.confirmar = Some("11".repeat(32));
    contadores::recibidos(
        &mut s,
        id,
        g,
        contadores::Contadores {
            transferencias: 2,
            accesos: Ok(vec!["11".repeat(32), "22".repeat(32)]),
            invitacion: Some(Ok((3, 10))),
        },
    );
    let p = s.panel.as_ref().unwrap();
    assert!(p.transferencias.contains('2'), "{}", p.transferencias);
    assert_eq!(p.accesos.personas.len(), 2);
    assert_eq!(
        p.accesos.confirmar.as_deref(),
        Some("11".repeat(32).as_str()),
        "una confirmación abierta se conserva si el acceso sigue"
    );
    let i = p.invitacion.as_ref().unwrap();
    assert_eq!((i.aceptados, i.cupo), (3, 10));
    assert!(matches!(i.estado, v::EstadoInvitacion::Vigente { .. }));

    contadores::recibidos(
        &mut s,
        id,
        g,
        contadores::Contadores {
            transferencias: 0,
            accesos: Ok(vec!["22".repeat(32)]),
            invitacion: Some(Ok((10, 10))),
        },
    );
    let p = s.panel.as_ref().unwrap();
    assert_eq!(p.transferencias, "Sin transferencias activas");
    assert!(p.accesos.confirmar.is_none(), "el acceso ya no existe");
    assert!(matches!(
        p.invitacion.as_ref().unwrap().estado,
        v::EstadoInvitacion::Completa
    ));
}

#[test]
fn contadores_viejos_o_fallidos_no_borran_datos_ni_liberan_operaciones() {
    let (mut s, id, g) = panel_con_contadores();
    s.red_ocupada = true;
    contadores::recibidos(
        &mut s,
        id,
        g - 1,
        contadores::Contadores {
            transferencias: 9,
            accesos: Ok(vec![]),
            invitacion: None,
        },
    );
    assert_eq!(s.panel.as_ref().unwrap().accesos.personas.len(), 1);
    contadores::recibidos(
        &mut s,
        id.wrapping_add(1),
        g,
        contadores::Contadores {
            transferencias: 9,
            accesos: Ok(vec![]),
            invitacion: None,
        },
    );
    assert_eq!(s.panel.as_ref().unwrap().accesos.personas.len(), 1);
    contadores::recibidos(
        &mut s,
        id,
        g,
        contadores::Contadores {
            transferencias: 1,
            accesos: Err("Sin red".into()),
            invitacion: Some(Err("Sin red".into())),
        },
    );
    let p = s.panel.as_ref().unwrap();
    assert_eq!(
        p.accesos.personas.len(),
        1,
        "un fallo de red no vacía la lista"
    );
    assert_eq!(p.invitacion.as_ref().unwrap().aceptados, 1);
    assert!(
        s.red_ocupada,
        "los contadores no liberan otra operación en curso"
    );
}

#[test]
fn sin_anfitrion_activo_no_se_piden_contadores() {
    let (mut s, _, _) = panel_con_contadores();
    assert!(contadores::pedir(&mut s).is_none());
    s.panel.as_mut().unwrap().estado = v::EstadoPublicacion::Detenida;
    assert!(contadores::pedir(&mut s).is_none());
}
