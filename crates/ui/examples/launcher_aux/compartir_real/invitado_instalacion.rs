// SPDX-License-Identifier: GPL-3.0-only
//! Primer destino aislado: instalar perfil exacto, verificar revisión y aplicar.
use super::invitado_tareas::{self, Visita};
use icaro_core::{
    compartir::{self as c, aplicacion::ContextoDestino},
    instancias::{Almacen, Instancia},
    modrinth::Catalog,
};
use icaro_ui::compartir as v;
use std::{path::PathBuf, sync::Arc};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct Resultado {
    pub instancia: Option<Instancia>,
    pub tarjeta: Option<icaro_ui::instancias::DatosInstancia>,
    pub resultado: Result<(), String>,
    pub revisar_nueva: bool,
}
pub fn revision(visita: &Visita, nombre: &str) -> v::RevisionSincronizacion {
    v::RevisionSincronizacion {
        instancia: nombre.into(),
        anfitrion: "Equipo anfitrión".into(),
        revision: visita.manifiesto.revision,
        estado: v::EstadoRevisionInvitado::Diferencias,
        conexion: None,
        diferencias: visita
            .manifiesto
            .archivos
            .iter()
            .map(|a| v::Diferencia {
                archivo: v::ArchivoCompartido {
                    id: a.ruta.as_str().into(),
                    nombre: a.ruta.as_str().into(),
                    detalle: a.ruta.as_str().into(),
                    tamano: format!("{} bytes", a.bytes),
                    procedencia: match a.procedencia {
                        c::Procedencia::Propio => "Archivo propio",
                        _ => "Modrinth",
                    }
                    .into(),
                    carpeta: a.ruta.as_str().split('/').next().unwrap_or_default().into(),
                    ..Default::default()
                },
                marca: v::Marca::Alta,
                del_anfitrion: matches!(a.procedencia, c::Procedencia::Propio),
                resolucion: None,
            })
            .collect(),
    }
}
pub async fn instalar(
    raiz: PathBuf,
    store: Almacen,
    visita: Visita,
    nombre: String,
    anterior: Option<Instancia>,
    catalogo: Option<Catalog>,
    host: Option<Arc<c::anfitrion::PublicacionActiva>>,
    cancel: CancellationToken,
) -> Resultado {
    instalar_con(
        raiz,
        store,
        visita,
        nombre,
        anterior,
        catalogo,
        host,
        cancel,
        |store, instancia, cancel| async move {
            // El vínculo evita sugerir Fabric API u otros mods fuera del manifiesto.
            icaro_core::minecraft::instalar(&store, &instancia, cancel, |_| {})
                .await
                .map(|_| ())
                .map_err(|e| e.to_string())
        },
    )
    .await
}
pub async fn instalar_con<F, Fut>(
    raiz: PathBuf,
    store: Almacen,
    visita: Visita,
    nombre: String,
    anterior: Option<Instancia>,
    catalogo: Option<Catalog>,
    host: Option<Arc<c::anfitrion::PublicacionActiva>>,
    cancel: CancellationToken,
    perfil: F,
) -> Resultado
where
    F: FnOnce(Almacen, Instancia, CancellationToken) -> Fut,
    Fut: std::future::Future<Output = Result<(), String>>,
{
    let crear_store = store.clone();
    let destino_anterior = anterior.clone();
    let manifiesto = visita.manifiesto.clone();
    let raiz_confianza = raiz.clone();
    let creada = tokio::task::spawn_blocking(move || {
        if !matches!(
            c::confianza::consultar(&raiz_confianza, &manifiesto.origen)
                .map_err(|e| e.to_string())?,
            c::confianza::EstadoConfianza::Conocido { .. }
        ) {
            return Err("Confirma la huella del anfitrión antes de crear el destino.".into());
        }
        if let Some(i) = anterior {
            let juego = crear_store.juego(&i.id).map_err(|e| e.to_string())?;
            if let Some(vinculo) = c::vinculos::leer(&juego).map_err(|e| e.to_string())? {
                if vinculo.origen != manifiesto.origen || vinculo.perfil != manifiesto.perfil {
                    return Err("El destino pertenece a otra publicación.".into());
                }
            }
            Ok(i)
        } else {
            crear_store
                .crear_con_loader(
                    &nombre,
                    &manifiesto.perfil.minecraft,
                    &manifiesto.perfil.loader,
                    0,
                )
                .map_err(|e| e.to_string())
        }
    })
    .await
    .map_err(|_| "No se pudo crear el destino.".to_string())
    .and_then(|r| r);
    let instancia = match creada {
        Ok(i) => i,
        Err(e) => {
            return Resultado {
                instancia: destino_anterior,
                tarjeta: None,
                resultado: Err(e),
                revisar_nueva: false,
            }
        }
    };
    let mut revisar_nueva = false;
    let resultado = async {
        if cancel.is_cancelled() {
            return Err("Instalación cancelada; el destino se conserva para reintentar.".into());
        }
        let preparar_store = store.clone();
        let preparar_instancia = instancia.clone();
        let manifiesto = visita.manifiesto.clone();
        tokio::task::spawn_blocking(move || {
            c::vinculos::preparar_destino(
                &preparar_store,
                &preparar_instancia,
                &manifiesto,
                &c::Limites::default(),
            )
            .map_err(|e| e.to_string())
        })
        .await
        .map_err(|_| "No se pudo completar el vínculo del destino.".to_string())??;
        perfil(store.clone(), instancia.clone(), cancel.clone()).await?;
        let conexion = invitado_tareas::conectar(&visita.invitacion, host, &cancel).await?;
        let resultado = async {
            let actual = conexion.consultar().await.map_err(|e| e.to_string())?;
            if actual != visita.manifiesto {
                revisar_nueva = true;
                return Err(
                    "La publicación cambió. Vuelve a revisar la invitación antes de instalar."
                        .into(),
                );
            }
            let juego = store.juego(&instancia.id).map_err(|e| e.to_string())?;
            let ctx = ContextoDestino {
                juego,
                limites: c::Limites::default(),
            };
            let contexto = ctx.clone();
            let token = cancel.clone();
            let plan = tokio::task::spawn_blocking(move || {
                c::aplicacion::recuperar(&contexto.juego).map_err(|e| e.to_string())?;
                let base = c::aplicacion::cargar_base(&contexto).map_err(|e| e.to_string())?;
                let inv = c::inventario::analizar(&contexto.juego, &token, &contexto.limites)
                    .map_err(|e| e.to_string())?;
                c::plan::planificar(&actual, &base, &inv).map_err(|e| e.to_string())
            })
            .await
            .map_err(|_| "No se pudo revisar el destino.".to_string())??;
            if !plan.conflictos.is_empty() {
                return Err(
                    "El destino fue modificado. No se sobrescribieron los conflictos.".into(),
                );
            }
            let staging = raiz.join("compartir/descargas").join(&instancia.id);
            let preparada =
                c::descargas::preparar(&ctx, plan, &conexion, catalogo.as_ref(), &staging, &cancel)
                    .await
                    .map_err(|e| e.to_string())?;
            let token = cancel.clone();
            tokio::task::spawn_blocking(move || {
                c::aplicacion::aplicar(preparada, &token, |_| {})
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            })
            .await
            .map_err(|_| "No se pudo aplicar la revisión.".to_string())?
        }
        .await;
        conexion.cerrar().await;
        resultado
    }
    .await;
    let listar = store.clone();
    let registro = instancia.clone();
    let exito = resultado.is_ok();
    let tarjeta = tokio::task::spawn_blocking(move || {
        let mut datos = crate::instancias_reales::Estado::datos(&listar, &registro);
        if !exito {
            datos.estado = icaro_ui::instancias::EstadoInstancia::Error;
            datos.detalle =
                "Instalación compartida incompleta; vuelve a intentar desde la invitación.".into();
        }
        datos
    })
    .await
    .ok();
    Resultado {
        instancia: Some(instancia),
        tarjeta,
        resultado,
        revisar_nueva,
    }
}
