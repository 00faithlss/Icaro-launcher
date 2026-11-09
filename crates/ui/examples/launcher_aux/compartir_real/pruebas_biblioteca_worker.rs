// SPDX-License-Identifier: GPL-3.0-only
//! Regresión con Worker e iroh reales, dentro del fixture aislado del publicador.
use super::*;
use icaro_core::compartir::{self as c, transporte::Cliente};
use std::{path::Path, sync::Arc};

pub(super) async fn comprobar_reconexion(
    url: &str,
    root: &Path,
    local: &tareas::PublicacionLocal,
    host: Arc<c::anfitrion::PublicacionActiva>,
    identidad_invitado: &Identidad,
    codigo_anterior: &Codigo,
) -> Arc<c::anfitrion::PublicacionActiva> {
    let invitado = ClienteInvitaciones::nuevo(url, identidad_invitado.clone()).unwrap();
    let origen = local.contexto.origen.clone();
    let revision = revisiones::vigente(&local.contexto).unwrap().unwrap();
    let encuentro = invitado.reencontrar(&origen).await.unwrap();
    let conexion = Cliente::conectar(
        identidad_invitado,
        encuentro
            .meeting
            .direccion(&origen.anfitrion, &origen.publicacion)
            .unwrap(),
        &origen,
        Modo::Local,
        c::Limites::default(),
    )
    .await
    .unwrap();
    assert_eq!(conexion.consultar().await.unwrap(), revision.manifiesto);
    conexion.cerrar().await;
    // La consulta anterior ha acreditado el acceso tanto remotamente como en ACL.
    let acl_path = local.contexto.almacen.join("accesos.json");
    let acl_antes = std::fs::read(&acl_path).unwrap();
    assert!(c::accesos::Accesos::abrir(acl_path.clone())
        .unwrap()
        .permitir(&identidad_invitado.clave_publica()));
    let accesos_antes = host.accesos().await.unwrap();
    host.cerrar().await;
    drop(host);

    // Reconstruye el contexto con la identidad inyectada; no carga claves del usuario.
    let recuperada = tareas::PublicacionLocal {
        instancia_id: local.instancia_id.clone(),
        contexto: local.contexto.clone(),
        identidad: local.identidad.clone(),
    };
    let (nuevo, creada) = crate::compartir_real::biblioteca_tareas::reconectar_con(
        recuperada,
        v::Caducidad::Dias2,
        3,
        Some(Servicio {
            url: url.into(),
            modo: Modo::Local,
        }),
    )
    .await
    .expect("reconectar sin revocar la publicación guardada");
    assert_ne!(
        Codigo::parse(&creada.code).unwrap().as_str(),
        codigo_anterior.as_str()
    );
    // Revocar el código impide nuevas aceptaciones; no expulsa a quien ya tenía
    // acceso. El invitado autorizado puede resolver incluso su código anterior.
    let sin_acceso = ClienteInvitaciones::nuevo(url, identidad()).unwrap();
    assert!(sin_acceso.resolver(codigo_anterior).await.is_err());
    assert!(invitado.resolver(codigo_anterior).await.unwrap().authorized);
    assert_eq!(nuevo.accesos().await.unwrap(), accesos_antes);
    assert_eq!(std::fs::read(&acl_path).unwrap(), acl_antes);
    assert_eq!(
        revisiones::vigente(&local.contexto)
            .unwrap()
            .unwrap()
            .manifiesto,
        revision.manifiesto
    );

    // /v1/reconnect funciona sin volver a aceptar ni conocer el código nuevo.
    let reencontrada = invitado
        .reencontrar(&origen)
        .await
        .expect("el invitado autorizado reencuentra la misma publicación tras el reinicio");
    assert_eq!(reencontrada.host, origen.anfitrion);
    assert_eq!(reencontrada.publication, origen.publicacion);
    let direccion = reencontrada
        .meeting
        .direccion(&origen.anfitrion, &origen.publicacion)
        .unwrap();
    let actual = nuevo
        .encuentro()
        .unwrap()
        .verificar(&origen.anfitrion, &origen.publicacion)
        .unwrap();
    let remoto = reencontrada
        .meeting
        .verificar(&origen.anfitrion, &origen.publicacion)
        .unwrap();
    assert_eq!(
        remoto.endpoint_hints, actual.endpoint_hints,
        "el Worker devuelve el nodo nuevo"
    );
    let conexion = Cliente::conectar(
        identidad_invitado,
        direccion.clone(),
        &origen,
        Modo::Local,
        c::Limites::default(),
    )
    .await
    .unwrap();
    let manifiesto = conexion.consultar().await.unwrap();
    assert_eq!(manifiesto, revision.manifiesto);
    let archivo = manifiesto
        .archivos
        .iter()
        .find(|a| a.ruta.as_str() == "mods/propio.jar")
        .unwrap();
    let descargado = conexion
        .descargar(
            &manifiesto,
            archivo,
            &root.join("reconexion-staging"),
            &tokio_util::sync::CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(
        std::fs::read(descargado).unwrap(),
        std::fs::read(
            local
                .contexto
                .almacen
                .join("contenido")
                .join(&archivo.sha512)
        )
        .unwrap()
    );
    conexion.cerrar().await;

    // Una dirección previamente válida tampoco permite saltarse la revocación.
    nuevo
        .revocar(&identidad_invitado.clave_publica())
        .await
        .unwrap();
    assert!(invitado.reencontrar(&origen).await.is_err());
    if let Ok(rechazada) = Cliente::conectar(
        identidad_invitado,
        direccion,
        &origen,
        Modo::Local,
        c::Limites::default(),
    )
    .await
    {
        assert!(
            rechazada.consultar().await.is_err(),
            "el transporte no admite al invitado revocado"
        );
        rechazada.cerrar().await;
    }
    assert!(!c::accesos::Accesos::abrir(acl_path)
        .unwrap()
        .permitir(&identidad_invitado.clave_publica()));
    nuevo
}
