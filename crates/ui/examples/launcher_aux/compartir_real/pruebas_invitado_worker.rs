// SPDX-License-Identifier: GPL-3.0-only
//! Extensión del recorrido Worker: perfil inyectado, archivos y aplicación reales.
use super::*;
use crate::compartir_real::{invitado_instalacion, invitado_tareas};

pub(super) async fn comprobar(url: &str, codigo: &Codigo, root: &std::path::Path) {
    use icaro_core::compartir::{
        self as c,
        confianza::{self, EstadoConfianza},
    };
    let raiz = root.join("invitado");
    std::fs::create_dir(&raiz).unwrap();
    let store = Almacen::nuevo(raiz.join("instances"));
    let personal = store.crear("Personal", "1.21.1", 0).unwrap();
    let personal_juego = store.juego(&personal.id).unwrap();
    std::fs::create_dir_all(personal_juego.join("worlds/mi-mundo")).unwrap();
    std::fs::write(personal_juego.join("worlds/mi-mundo/nivel"), b"conservar").unwrap();
    let token = tokio_util::sync::CancellationToken::new();
    let resuelta = invitado_tareas::resolver_con(
        identidad(),
        codigo.clone(),
        Servicio {
            url: url.into(),
            modo: Modo::Local,
        },
        token.clone(),
    )
    .await
    .unwrap();
    assert!(!resuelta.respuesta.authorized);
    let visita = invitado_tareas::consultar(raiz.clone(), resuelta, None, token.clone())
        .await
        .unwrap();
    assert!(matches!(visita.confianza, EstadoConfianza::PrimeraVez));
    assert_eq!(
        store.cargar().unwrap().instancias.len(),
        1,
        "consultar no crea un destino"
    );
    // Sin confirmar la clave, ni siquiera se ejecuta el instalador del perfil.
    let no_confiada = invitado_instalacion::instalar_con(
        raiz.clone(),
        store.clone(),
        visita.clone(),
        "Bloqueada".into(),
        None,
        None,
        None,
        token.clone(),
        |_, _, _| async {
            panic!("no instalar sin confianza");
            #[allow(unreachable_code)]
            Ok(())
        },
    )
    .await;
    assert!(no_confiada.resultado.is_err());
    assert!(no_confiada.instancia.is_none());
    assert_eq!(store.cargar().unwrap().instancias.len(), 1);
    invitado_tareas::confiar(raiz.clone(), visita.clone(), false)
        .await
        .unwrap();
    assert!(matches!(
        confianza::consultar(&raiz, &visita.manifiesto.origen).unwrap(),
        EstadoConfianza::Conocido { .. }
    ));
    // Cancelar justo después de crear conserva el registro para volver a abrir Ícaro.
    let cancelar = tokio_util::sync::CancellationToken::new();
    let al_crear = cancelar.clone();
    let cancelada = invitado_instalacion::instalar_con_avance(
        raiz.clone(),
        store.clone(),
        visita.clone(),
        "Compartida".into(),
        None,
        None,
        None,
        cancelar,
        std::sync::Arc::new(move |e| {
            if e.archivo == "Compartida" {
                al_crear.cancel();
            }
        }),
        |_, _, _, _| async {
            panic!("no instalar un perfil después de cancelar");
            #[allow(unreachable_code)]
            Ok(())
        },
    )
    .await;
    assert!(cancelada.resultado.is_err());
    let cancelada = cancelada.instancia.unwrap();
    assert_eq!(
        c::intentos::buscar(&Almacen::nuevo(raiz.join("instances")), &visita.manifiesto)
            .unwrap()
            .unwrap()
            .id,
        cancelada.id
    );
    // Un fallo del perfil conserva ese mismo destino incompleto, sin base aplicada.
    let fallida = invitado_instalacion::instalar_con(
        raiz.clone(),
        store.clone(),
        visita.clone(),
        "Compartida".into(),
        Some(cancelada),
        None,
        None,
        token.clone(),
        |_, _, _| async { Err("Fallo de perfil inyectado".into()) },
    )
    .await;
    assert!(fallida.resultado.is_err());
    let destino = fallida.instancia.unwrap();
    assert_ne!(destino.id, personal.id);
    let juego = store.juego(&destino.id).unwrap();
    let contexto = c::aplicacion::ContextoDestino {
        juego: juego.clone(),
        limites: c::Limites::default(),
    };
    assert!(c::aplicacion::cargar_base(&contexto)
        .unwrap()
        .manifiesto
        .is_none());
    // Otro Almacen representa volver a abrir Ícaro: no conservamos el destino en memoria.
    let store = Almacen::nuevo(raiz.join("instances"));
    let recuperada =
        invitado_tareas::confiar_y_recuperar(raiz.clone(), store.clone(), visita.clone(), false)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(recuperada.id, destino.id);
    let eventos = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let guardar_eventos = eventos.clone();
    let instalada = invitado_instalacion::instalar_con_avance(
        raiz.clone(),
        store.clone(),
        visita.clone(),
        "Compartida recuperada".into(),
        Some(recuperada),
        None,
        None,
        token,
        std::sync::Arc::new(move |e| guardar_eventos.lock().unwrap().push(e)),
        |_, _, _, emitir| async move {
            emitir(crate::compartir_real::avance::Avance::nuevo(
                v::Fase::Preparacion,
                1,
                2,
                "",
                "Perfil de prueba",
            ));
            Ok(())
        },
    )
    .await;
    instalada.resultado.unwrap();
    let eventos = eventos.lock().unwrap();
    for fase in [
        v::Fase::Preparacion,
        v::Fase::Descarga,
        v::Fase::Verificacion,
        v::Fase::Aplicacion,
    ] {
        assert!(
            eventos.iter().any(|e| e.fase == fase),
            "falta la etapa {fase:?}"
        );
    }
    assert!(eventos
        .iter()
        .any(|e| e.fase == v::Fase::Descarga && e.total > 0 && e.hechos == e.total));
    drop(eventos);
    assert!(c::intentos::buscar(&store, &visita.manifiesto)
        .unwrap()
        .is_none());
    let juego = store.juego(&destino.id).unwrap();
    assert!(!juego.join(".icaro-compartir-instalacion.json").exists());
    assert_eq!(
        store
            .cargar()
            .unwrap()
            .instancias
            .into_iter()
            .find(|i| i.id == destino.id)
            .unwrap()
            .nombre,
        "Compartida recuperada"
    );
    let contexto = c::aplicacion::ContextoDestino {
        juego: juego.clone(),
        limites: c::Limites::default(),
    };
    c::vinculos::comprobar_lista(&store, &destino).unwrap();
    assert_eq!(
        instalada.instancia.unwrap().id,
        destino.id,
        "el reintento no duplica el destino"
    );
    assert_eq!(store.cargar().unwrap().instancias.len(), 2);
    assert_eq!(
        c::aplicacion::cargar_base(&contexto).unwrap().manifiesto,
        Some(visita.manifiesto)
    );
    assert!(juego.join("mods/propio.jar").exists());
    assert!(juego.join("resourcepacks/texturas.zip").exists());
    assert!(!juego.join("config/privada.toml").exists());
    assert!(std::fs::read_to_string(juego.join("options.txt"))
        .unwrap()
        .contains("file/texturas.zip"));
    assert_eq!(
        std::fs::read(personal_juego.join("worlds/mi-mundo/nivel")).unwrap(),
        b"conservar"
    );
}
