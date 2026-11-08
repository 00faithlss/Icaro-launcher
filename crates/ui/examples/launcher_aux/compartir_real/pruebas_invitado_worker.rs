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
    // Un fallo del perfil conserva un único destino incompleto, sin base aplicada.
    let fallida = invitado_instalacion::instalar_con(
        raiz.clone(),
        store.clone(),
        visita.clone(),
        "Compartida".into(),
        None,
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
    let instalada = invitado_instalacion::instalar_con(
        raiz.clone(),
        store.clone(),
        visita.clone(),
        "Compartida".into(),
        Some(destino.clone()),
        None,
        None,
        token,
        |_, _, _| async { Ok(()) },
    )
    .await;
    instalada.resultado.unwrap();
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
