// SPDX-License-Identifier: GPL-3.0-only
//! Recorrido local: revisión sin descargas, conflictos explícitos y exclusión.
use crate::compartir_real::{invitado_tareas::Visita, servicio::Servicio, sync_tareas};
use icaro_core::{
    actividad_instancia::Guardia,
    compartir::{
        self as c,
        publicacion::{Borrador, Seleccion},
        revisiones::{self, ContextoPublicacion},
    },
    instancias::{Almacen, Instancia},
};
use std::{path::Path, sync::Arc};
use tokio_util::sync::CancellationToken;

async fn revisar(url: &str, store: &Almacen, i: &Instancia, v: &Visita) -> sync_tareas::Revision {
    let identidad = v.invitacion.identidad.clone();
    sync_tareas::consultar_con(
        store.clone(),
        i.clone(),
        Some(Servicio {
            url: url.into(),
            modo: c::transporte::Modo::Local,
        }),
        None,
        CancellationToken::new(),
        move || Ok(identidad),
    )
    .await
    .unwrap()
    .unwrap()
}
fn publicar(ctx: &ContextoPublicacion, host: &c::anfitrion::PublicacionActiva, contenido: &[u8]) {
    std::fs::write(ctx.juego.join("config/compartida.toml"), contenido).unwrap();
    let inv = c::inventario::analizar(&ctx.juego, &CancellationToken::new(), &ctx.limites).unwrap();
    let anterior = revisiones::vigente(ctx).unwrap().unwrap();
    let mut borrador = Borrador::comparar(
        &inv,
        Some(&anterior.manifiesto),
        &Seleccion::cargar(&ctx.juego).unwrap(),
    )
    .unwrap();
    borrador.autorizar_propios();
    let nueva = revisiones::publicar_revision(
        revisiones::preparar_revision(ctx, &borrador, &CancellationToken::new()).unwrap(),
    )
    .unwrap();
    host.actualizar_revision(nueva).unwrap();
}
pub async fn comprobar(
    url: &str,
    raiz: &Path,
    store: &Almacen,
    i: &Instancia,
    v: &Visita,
    host: &Arc<c::anfitrion::PublicacionActiva>,
    ctx: &ContextoPublicacion,
) {
    let juego = store.juego(&i.id).unwrap();
    let intacta = revisar(url, store, i, v).await;
    assert!(intacta.sin_cambios());
    assert!(
        Guardia::adquirir_instancia(store, &i.id).is_err(),
        "la revisión conserva la reserva"
    );
    let transferida = crate::juego_real::actividad::adquirir(
        store.clone(),
        i.clone(),
        Some(intacta.actividad.clone()),
    )
    .await
    .unwrap();
    assert!(Arc::ptr_eq(&transferida, &intacta.actividad));
    drop(intacta);
    assert!(
        Guardia::adquirir_instancia(store, &i.id).is_err(),
        "Jugar conserva la misma reserva"
    );
    drop(transferida);
    drop(Guardia::adquirir_instancia(store, &i.id).unwrap());
    std::fs::write(juego.join("config/solo_mio.toml"), b"personal").unwrap();
    for (eleccion, local, remoto) in [
        (
            c::plan::Resolucion::DelAnfitrion,
            b"mi cambio 2".as_slice(),
            b"host 2".as_slice(),
        ),
        (
            c::plan::Resolucion::ConservarMia,
            b"mi cambio 3".as_slice(),
            b"host 3".as_slice(),
        ),
    ] {
        std::fs::write(juego.join("config/compartida.toml"), local).unwrap();
        publicar(ctx, host, remoto);
        let mut revision = revisar(url, store, i, v).await;
        assert_eq!(revision.plan.conflictos.len(), 1);
        assert!(!revision.sin_cambios());
        assert_eq!(
            std::fs::read(juego.join("config/compartida.toml")).unwrap(),
            local,
            "consultar no aplica cambios"
        );
        revision
            .plan
            .resolver(
                &c::RutaCompartida::parse("config/compartida.toml").unwrap(),
                eleccion,
            )
            .unwrap();
        sync_tareas::aplicar(
            raiz.to_owned(),
            Arc::new(revision),
            None,
            None,
            CancellationToken::new(),
            Arc::new(|_| {}),
        )
        .await
        .unwrap();
        assert_eq!(
            std::fs::read(juego.join("config/compartida.toml")).unwrap(),
            if matches!(eleccion, c::plan::Resolucion::DelAnfitrion) {
                remoto
            } else {
                local
            }
        );
        assert_eq!(
            std::fs::read(juego.join("config/solo_mio.toml")).unwrap(),
            b"personal"
        );
    }
    assert!(revisar(url, store, i, v).await.sin_cambios());
    host.revocar(&v.invitacion.identidad.clave_publica())
        .await
        .unwrap();
    let identidad = v.invitacion.identidad.clone();
    assert!(sync_tareas::consultar_con(
        store.clone(),
        i.clone(),
        Some(Servicio {
            url: url.into(),
            modo: c::transporte::Modo::Local
        }),
        None,
        CancellationToken::new(),
        move || Ok(identidad)
    )
    .await
    .is_err());
    drop(Guardia::adquirir_instancia(store, &i.id).unwrap());
}
