// SPDX-License-Identifier: GPL-3.0-only
//! Trabajo de disco/red del publicador, siempre fuera de update/view.
use super::servicio::Servicio;
use icaro_core::{
    compartir::{
        self as c,
        anfitrion::PublicacionActiva,
        identidad::Identidad,
        inventario::Inventario,
        publicacion::{Borrador, Eleccion, Seleccion},
        revisiones::{self, ContextoPublicacion, Revision},
    },
    instancias::{Almacen, Instancia},
    modrinth::Catalog,
};
use std::{path::PathBuf, sync::Arc};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct PublicacionLocal {
    pub instancia_id: String,
    pub contexto: ContextoPublicacion,
    pub identidad: Identidad,
}
impl PublicacionLocal {
    pub fn desde(t: &Trabajo) -> Self {
        Self {
            instancia_id: t.instancia.id.clone(),
            contexto: t.contexto.clone(),
            identidad: t.identidad.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Trabajo {
    pub instancia: Instancia,
    pub contexto: ContextoPublicacion,
    pub identidad: Identidad,
    pub inventario: Inventario,
    pub borrador: Borrador,
    pub seleccion: Seleccion,
    pub anterior: Option<Revision>,
    pub activos: Vec<String>,
    pub aviso: Option<String>,
    pub orden_pospuesto: bool,
}
impl Trabajo {
    pub fn elegir(&mut self, ruta: &str, eleccion: Eleccion) -> Result<(), String> {
        if ruta == "@orden-resourcepacks" {
            self.orden_pospuesto = eleccion == Eleccion::Posponer;
            self.packs();
            return Ok(());
        }
        let ruta =
            c::RutaCompartida::parse(ruta).map_err(icaro_core::diagnostico_contexto::texto)?;
        self.borrador
            .elegir(&ruta, eleccion)
            .map_err(icaro_core::diagnostico_contexto::texto)?;
        self.seleccion
            .mantener_privado(ruta, eleccion == Eleccion::Privado);
        self.packs();
        Ok(())
    }
    fn packs(&mut self) {
        if self.orden_pospuesto {
            self.borrador.seleccionar_packs(
                self.anterior
                    .as_ref()
                    .map_or_else(Vec::new, |r| r.manifiesto.packs.clone()),
            );
            return;
        }
        let publicados: std::collections::BTreeSet<_> = self
            .borrador
            .archivos_publicados()
            .into_iter()
            .map(|a| a.ruta.as_str().to_owned())
            .collect();
        let activos = self
            .activos
            .iter()
            .filter(|p| match p.strip_prefix("file/") {
                None => true,
                Some(nombre) => {
                    let ruta = format!("resourcepacks/{nombre}");
                    let prefijo = format!("{ruta}/");
                    publicados.contains(&ruta)
                        || publicados
                            .range(prefijo.clone()..)
                            .next()
                            .is_some_and(|p| p.starts_with(&prefijo))
                }
            })
            .cloned()
            .collect();
        self.borrador.seleccionar_packs(activos);
    }
}

pub async fn analizar(
    raiz: PathBuf,
    almacen: Almacen,
    instancia: Instancia,
    catalogo: Option<Catalog>,
    cancel: CancellationToken,
) -> Result<Trabajo, String> {
    analizar_con(raiz, almacen, instancia, None, catalogo, cancel).await
}

/// Sin identidad usa la del almacén seguro del sistema; las pruebas inyectan una en memoria.
pub async fn analizar_con(
    raiz: PathBuf,
    almacen: Almacen,
    instancia: Instancia,
    identidad: Option<Identidad>,
    catalogo: Option<Catalog>,
    cancel: CancellationToken,
) -> Result<Trabajo, String> {
    let cancel_registro = cancel.clone();
    let instancia_registro = instancia.id.clone();
    icaro_core::diagnostico_contexto::observar(
        "compartir",
        "analizar_publicacion",
        Some(&instancia_registro),
        &cancel_registro,
        analizar_con_interno(raiz, almacen, instancia, identidad, catalogo, cancel),
    )
    .await
}

async fn analizar_con_interno(
    raiz: PathBuf,
    almacen: Almacen,
    instancia: Instancia,
    identidad: Option<Identidad>,
    catalogo: Option<Catalog>,
    cancel: CancellationToken,
) -> Result<Trabajo, String> {
    icaro_core::diagnostico_contexto::etapa("inventario_local");
    let cancel_disco = cancel.clone();
    let mut trabajo = icaro_core::diagnostico_contexto::bloqueante(move || {
        let identidad = match identidad {
            Some(identidad) => identidad,
            None => {
                Identidad::cargar_o_crear(&raiz).map_err(icaro_core::diagnostico_contexto::texto)?
            }
        };
        let contexto =
            c::vinculos::publicacion(&raiz, &almacen, &instancia, &identidad.clave_publica())
                .map_err(icaro_core::diagnostico_contexto::texto)?;
        c::aplicacion::recuperar(&contexto.juego)
            .map_err(icaro_core::diagnostico_contexto::texto)?;
        let anterior =
            revisiones::vigente(&contexto).map_err(icaro_core::diagnostico_contexto::texto)?;
        let inventario = c::inventario::analizar(&contexto.juego, &cancel_disco, &contexto.limites)
            .map_err(icaro_core::diagnostico_contexto::texto)?;
        let seleccion =
            Seleccion::cargar(&contexto.juego).map_err(icaro_core::diagnostico_contexto::texto)?;
        let borrador = Borrador::comparar(
            &inventario,
            anterior.as_ref().map(|r| &r.manifiesto),
            &seleccion,
        )
        .map_err(icaro_core::diagnostico_contexto::texto)?;
        let activos = icaro_core::mods::recursos_activos(&almacen, &instancia.id)
            .map_err(icaro_core::diagnostico_contexto::texto)?;
        Ok::<_, String>(Trabajo {
            instancia,
            contexto,
            identidad,
            inventario,
            borrador,
            seleccion,
            anterior,
            activos,
            aviso: None,
            orden_pospuesto: false,
        })
    })
    .await
    .map_err(|_| "No se pudo analizar la instancia.".to_string())??;
    if let Some(catalogo) = catalogo {
        icaro_core::diagnostico_contexto::etapa("catalogo");
        let archivos: Vec<_> = trabajo
            .inventario
            .archivos
            .iter()
            .filter(|a| {
                a.ruta.as_str().starts_with("mods/")
                    || a.ruta.as_str().starts_with("resourcepacks/")
            })
            .cloned()
            .collect();
        for lote in archivos.chunks(512) {
            let hashes: Vec<_> = lote.iter().map(|a| a.sha512.clone()).collect();
            let versiones = tokio::select! {
                _ = cancel.cancelled() => return Err("La revisión fue cancelada.".into()),
                r = catalogo.versions_from_hashes(&hashes) => r,
            };
            match versiones {
                Ok(versiones) => {
                    for archivo in lote {
                        if let Some(version) = versiones.get(&archivo.sha512) {
                            trabajo
                                .borrador
                                .reconocer_catalogo(&archivo.ruta, version)
                                .map_err(icaro_core::diagnostico_contexto::texto)?;
                        }
                    }
                }
                Err(_) => {
                    trabajo.aviso = Some("No se pudo reconocer todo el contenido en el catálogo. Los archivos sin origen comprobado requieren tu permiso.".into());
                    break;
                }
            }
        }
    } else {
        trabajo.aviso = Some(
            "El catálogo no está disponible. Los archivos propios requieren tu permiso.".into(),
        );
    }
    if cancel.is_cancelled() {
        return Err("La revisión fue cancelada.".into());
    }
    trabajo.packs();
    Ok(trabajo)
}

pub async fn guardar_seleccion(trabajo: Trabajo) -> Result<(), String> {
    let cancel_registro = tokio_util::sync::CancellationToken::new();
    let instancia_registro = trabajo.instancia.id.clone();
    icaro_core::diagnostico_contexto::observar(
        "compartir",
        "guardar_privados",
        Some(&instancia_registro),
        &cancel_registro,
        guardar_seleccion_interno(trabajo),
    )
    .await
}

async fn guardar_seleccion_interno(trabajo: Trabajo) -> Result<(), String> {
    icaro_core::diagnostico_contexto::bloqueante(move || {
        trabajo
            .seleccion
            .guardar(&trabajo.contexto.juego)
            .map_err(icaro_core::diagnostico_contexto::texto)
    })
    .await
    .map_err(|_| "No se pudo guardar la selección privada.".to_string())?
}

#[derive(Debug, Clone)]
pub struct Publicada {
    pub revision: Revision,
    pub anfitrion: Result<Arc<PublicacionActiva>, String>,
}
pub async fn publicar(
    trabajo: Trabajo,
    anterior: Option<Arc<PublicacionActiva>>,
    cancel: CancellationToken,
) -> Result<Publicada, String> {
    publicar_con(trabajo, anterior, Servicio::desde_entorno(), cancel).await
}

/// Valida el servicio antes de guardar la selección o escribir la revisión.
pub async fn publicar_con(
    trabajo: Trabajo,
    anterior: Option<Arc<PublicacionActiva>>,
    servicio: Option<Servicio>,
    cancel: CancellationToken,
) -> Result<Publicada, String> {
    let cancel_registro = cancel.clone();
    let instancia_registro = trabajo.instancia.id.clone();
    icaro_core::diagnostico_contexto::observar_valor(
        "compartir",
        "publicar_revision",
        Some(&instancia_registro),
        &cancel_registro,
        publicar_con_interno(trabajo, anterior, servicio, cancel),
        |r: &Result<Publicada, String>| r.as_ref().is_ok_and(|p| p.anfitrion.is_ok()),
    )
    .await
}

async fn publicar_con_interno(
    trabajo: Trabajo,
    anterior: Option<Arc<PublicacionActiva>>,
    servicio: Option<Servicio>,
    cancel: CancellationToken,
) -> Result<Publicada, String> {
    let Servicio { url, modo } = servicio
        .ok_or_else(|| "El servicio de invitaciones todavía no está configurado.".to_string())?;
    c::invitaciones::ClienteInvitaciones::nuevo(&url, trabajo.identidad.clone())
        .map_err(icaro_core::diagnostico_contexto::texto)?;
    let identidad = trabajo.identidad.clone();
    let acl = trabajo.contexto.almacen.join("accesos.json");
    icaro_core::diagnostico_contexto::etapa("snapshot_publicacion");
    let revision = icaro_core::diagnostico_contexto::bloqueante(move || {
        trabajo
            .seleccion
            .guardar(&trabajo.contexto.juego)
            .map_err(icaro_core::diagnostico_contexto::texto)?;
        let preparada =
            revisiones::preparar_revision(&trabajo.contexto, &trabajo.borrador, &cancel)
                .map_err(icaro_core::diagnostico_contexto::texto)?;
        revisiones::publicar_revision(preparada).map_err(icaro_core::diagnostico_contexto::texto)
    })
    .await
    .map_err(|_| "No se pudo preparar la publicación.".to_string())??;
    icaro_core::diagnostico_contexto::etapa("transporte_anfitrion");
    let anfitrion = if let Some(anterior) = anterior {
        anterior
            .actualizar_revision(revision.clone())
            .map(|_| anterior)
            .map_err(icaro_core::diagnostico_contexto::texto)
    } else {
        PublicacionActiva::iniciar(
            identidad,
            revision.clone(),
            &acl,
            &url,
            modo,
            c::Limites::default(),
        )
        .await
        .map(Arc::new)
        .map_err(icaro_core::diagnostico_contexto::texto)
    };
    Ok(Publicada {
        revision,
        anfitrion,
    })
}
