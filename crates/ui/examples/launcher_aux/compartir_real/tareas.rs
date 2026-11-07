// SPDX-License-Identifier: GPL-3.0-only
//! Trabajo de disco/red del publicador, siempre fuera de update/view.
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
        let ruta = c::RutaCompartida::parse(ruta).map_err(|e| e.to_string())?;
        self.borrador
            .elegir(&ruta, eleccion)
            .map_err(|e| e.to_string())?;
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
    let cancel_disco = cancel.clone();
    let mut trabajo = tokio::task::spawn_blocking(move || {
        let identidad = Identidad::cargar_o_crear(&raiz).map_err(|e| e.to_string())?;
        let contexto =
            c::vinculos::publicacion(&raiz, &almacen, &instancia, &identidad.clave_publica())
                .map_err(|e| e.to_string())?;
        c::aplicacion::recuperar(&contexto.juego).map_err(|e| e.to_string())?;
        let anterior = revisiones::vigente(&contexto).map_err(|e| e.to_string())?;
        let inventario = c::inventario::analizar(&contexto.juego, &cancel_disco, &contexto.limites)
            .map_err(|e| e.to_string())?;
        let seleccion = Seleccion::cargar(&contexto.juego).map_err(|e| e.to_string())?;
        let borrador = Borrador::comparar(
            &inventario,
            anterior.as_ref().map(|r| &r.manifiesto),
            &seleccion,
        )
        .map_err(|e| e.to_string())?;
        let activos = icaro_core::mods::recursos_activos(&almacen, &instancia.id)
            .map_err(|e| e.to_string())?;
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
                                .map_err(|e| e.to_string())?;
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
    tokio::task::spawn_blocking(move || {
        trabajo
            .seleccion
            .guardar(&trabajo.contexto.juego)
            .map_err(|e| e.to_string())
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
    let url = std::env::var("ICARO_SERVICIO_INVITACIONES")
        .map_err(|_| "El servicio de invitaciones todavía no está configurado.".to_string())?;
    c::invitaciones::ClienteInvitaciones::nuevo(&url, trabajo.identidad.clone())
        .map_err(|e| e.to_string())?;
    let identidad = trabajo.identidad.clone();
    let acl = trabajo.contexto.almacen.join("accesos.json");
    let revision = tokio::task::spawn_blocking(move || {
        trabajo
            .seleccion
            .guardar(&trabajo.contexto.juego)
            .map_err(|e| e.to_string())?;
        let preparada =
            revisiones::preparar_revision(&trabajo.contexto, &trabajo.borrador, &cancel)
                .map_err(|e| e.to_string())?;
        revisiones::publicar_revision(preparada).map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| "No se pudo preparar la publicación.".to_string())??;
    let anfitrion = if let Some(anterior) = anterior {
        anterior
            .actualizar_revision(revision.clone())
            .map(|_| anterior)
            .map_err(|e| e.to_string())
    } else {
        let modo = if std::env::var("ICARO_COMPARTIR_LOCAL").as_deref() == Ok("1") {
            c::transporte::Modo::Local
        } else {
            c::transporte::Modo::Internet
        };
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
        .map_err(|e| e.to_string())
    };
    Ok(Publicada {
        revision,
        anfitrion,
    })
}
