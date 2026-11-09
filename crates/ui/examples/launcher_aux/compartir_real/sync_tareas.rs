// SPDX-License-Identifier: GPL-3.0-only
//! Consultar una publicación ya aceptada, sin instalar ni descargar antes de revisar.
use super::{
    avance::{Avance, Emitir},
    servicio::Servicio,
};
use icaro_core::{
    actividad_instancia::Guardia,
    compartir::{
        self as c, aplicacion::ContextoDestino, identidad::Identidad, plan::PlanCompartido,
    },
    instancias::{Almacen, Instancia},
    modrinth::Catalog,
};
use icaro_ui::compartir as v;
use std::{path::PathBuf, sync::Arc};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct Revision {
    pub instancia: Instancia,
    pub contexto: ContextoDestino,
    pub plan: PlanCompartido,
    pub identidad: Identidad,
    pub encuentro: c::invitaciones::Reencuentro,
    pub modo: c::transporte::Modo,
    pub actividad: Arc<Guardia>,
    pub al_dia: bool,
}
impl Revision {
    pub fn sin_cambios(&self) -> bool {
        self.al_dia && self.plan.cambios.is_empty() && self.plan.conflictos.is_empty()
    }
}

pub async fn consultar(
    raiz: PathBuf,
    store: Almacen,
    instancia: Instancia,
    servicio: Option<Servicio>,
    host: Option<Arc<c::anfitrion::PublicacionActiva>>,
    cancel: CancellationToken,
) -> Result<Option<Revision>, String> {
    let raiz_identidad = raiz.clone();
    consultar_con(store, instancia, servicio, host, cancel, move || {
        Identidad::cargar_o_crear(&raiz_identidad).map_err(icaro_core::diagnostico_contexto::texto)
    })
    .await
}
pub async fn consultar_con<F>(
    store: Almacen,
    instancia: Instancia,
    servicio: Option<Servicio>,
    host: Option<Arc<c::anfitrion::PublicacionActiva>>,
    cancel: CancellationToken,
    identidad: F,
) -> Result<Option<Revision>, String>
where
    F: FnOnce() -> Result<Identidad, String> + Send + 'static,
{
    let cancel_registro = cancel.clone();
    let instancia_registro = instancia.id.clone();
    icaro_core::diagnostico_contexto::observar(
        "compartir",
        "consultar_revision",
        Some(&instancia_registro),
        &cancel_registro,
        consultar_con_interno(store, instancia, servicio, host, cancel, identidad),
    )
    .await
}

async fn consultar_con_interno<F>(
    store: Almacen,
    instancia: Instancia,
    servicio: Option<Servicio>,
    host: Option<Arc<c::anfitrion::PublicacionActiva>>,
    cancel: CancellationToken,
    identidad: F,
) -> Result<Option<Revision>, String>
where
    F: FnOnce() -> Result<Identidad, String> + Send + 'static,
{
    let datos = icaro_core::diagnostico_contexto::bloqueante(move || {
        let juego = store
            .juego(&instancia.id)
            .map_err(icaro_core::diagnostico_contexto::texto)?;
        let Some(vinculo) =
            c::vinculos::leer(&juego).map_err(icaro_core::diagnostico_contexto::texto)?
        else {
            return Ok(None);
        };
        let actividad = Arc::new(
            Guardia::adquirir_instancia(&store, &instancia.id)
                .map_err(icaro_core::diagnostico_contexto::texto)?,
        );
        c::aplicacion::recuperar(&juego).map_err(icaro_core::diagnostico_contexto::texto)?;
        c::vinculos::comprobar_lista(&store, &instancia)
            .map_err(icaro_core::diagnostico_contexto::texto)?;
        let contexto = ContextoDestino {
            juego,
            limites: c::Limites::default(),
        };
        let base = c::aplicacion::cargar_base(&contexto)
            .map_err(icaro_core::diagnostico_contexto::texto)?;
        let packs = icaro_core::mods::recursos_activos(&store, &instancia.id)
            .map_err(icaro_core::diagnostico_contexto::texto)?;
        Ok::<_, String>(Some((
            instancia,
            vinculo,
            contexto,
            base,
            actividad,
            identidad()?,
            packs,
        )))
    })
    .await
    .map_err(|_| "No se pudo revisar el vínculo de esta instancia.".to_string())??;
    let Some((instancia, vinculo, contexto, base, actividad, identidad, packs)) = datos else {
        return Ok(None);
    };
    let servicio =
        servicio.ok_or_else(|| "El servicio de invitaciones no está configurado.".to_string())?;
    icaro_core::diagnostico_contexto::etapa("reconexion_autorizada");
    let cliente = c::invitaciones::ClienteInvitaciones::nuevo(&servicio.url, identidad.clone())
        .map_err(icaro_core::diagnostico_contexto::texto)?;
    let encuentro = tokio::select! {_=cancel.cancelled()=>return Err("Consulta cancelada.".into()),r=cliente.reencontrar(&vinculo.origen)=>r.map_err(icaro_core::diagnostico_contexto::texto)?};
    icaro_core::diagnostico_contexto::etapa("manifiesto_anfitrion");
    let conexion = conectar(&identidad, &encuentro, servicio.modo, host, &cancel).await?;
    let remoto = tokio::select! {_=cancel.cancelled()=>Err("Consulta cancelada.".into()),r=conexion.consultar()=>r.map_err(icaro_core::diagnostico_contexto::texto)};
    conexion.cerrar().await;
    let remoto = remoto?;
    if remoto.perfil != vinculo.perfil {
        return Err(
            "El perfil de la publicación cambió. Acepta una invitación para crear otro destino."
                .into(),
        );
    }
    let ctx = contexto.clone();
    let token = cancel.clone();
    let base_packs = base
        .manifiesto
        .as_ref()
        .map_or_else(Vec::new, |m| m.packs.clone());
    let orden_packs = c::packs::RevisionOrden::nueva(&packs, &base_packs, &remoto.packs)
        .map_err(icaro_core::diagnostico_contexto::texto)?;
    let al_dia = base.manifiesto.as_ref() == Some(&remoto);
    icaro_core::diagnostico_contexto::etapa("comparacion_local");
    let mut plan = icaro_core::diagnostico_contexto::bloqueante(move || {
        let inventario = c::inventario::analizar(&ctx.juego, &token, &ctx.limites)
            .map_err(icaro_core::diagnostico_contexto::texto)?;
        c::plan::planificar(&remoto, &base, &inventario)
            .map_err(icaro_core::diagnostico_contexto::texto)
    })
    .await
    .map_err(|_| "No se pudo comparar la revisión.".to_string())??;
    plan.orden_packs = Some(orden_packs);
    if cancel.is_cancelled() {
        return Err("Consulta cancelada.".into());
    }
    Ok(Some(Revision {
        instancia,
        contexto,
        plan,
        identidad,
        encuentro,
        modo: servicio.modo,
        actividad,
        al_dia,
    }))
}
pub async fn aplicar(
    raiz: PathBuf,
    revision: Arc<Revision>,
    catalogo: Option<Catalog>,
    host: Option<Arc<c::anfitrion::PublicacionActiva>>,
    cancel: CancellationToken,
    emitir: Emitir,
) -> Result<(), String> {
    let cancel_registro = cancel.clone();
    let instancia_registro = revision.instancia.id.clone();
    icaro_core::diagnostico_contexto::observar(
        "compartir",
        "sincronizar_revision",
        Some(&instancia_registro),
        &cancel_registro,
        aplicar_interno(raiz, revision, catalogo, host, cancel, emitir),
    )
    .await
}

async fn aplicar_interno(
    raiz: PathBuf,
    revision: Arc<Revision>,
    catalogo: Option<Catalog>,
    host: Option<Arc<c::anfitrion::PublicacionActiva>>,
    cancel: CancellationToken,
    emitir: Emitir,
) -> Result<(), String> {
    let conexion = conectar(
        &revision.identidad,
        &revision.encuentro,
        revision.modo,
        host,
        &cancel,
    )
    .await?;
    let r = async {
        let actual = tokio::select! {
            _ = cancel.cancelled() => return Err("Sincronización cancelada.".into()),
            r = conexion.consultar() => r.map_err(icaro_core::diagnostico_contexto::texto)?,
        };
        if actual != revision.plan.revision {
            return Err("La publicación cambió. Vuelve a revisar antes de aplicar.".into());
        }
        icaro_core::diagnostico_contexto::etapa("descarga_verificacion");
        let progreso = emitir.clone();
        let opciones = c::descargas::OpcionesDescarga {
            cache: Some(c::cache::CacheContenido::nueva(
                raiz.join("compartir/cache-contenido"),
            )),
            ..Default::default()
        };
        let preparada = c::descargas::preparar_configurado(
            &revision.contexto,
            revision.plan.clone(),
            &conexion,
            catalogo.as_ref(),
            &raiz
                .join("compartir/descargas")
                .join(&revision.instancia.id),
            &cancel,
            &opciones,
            move |e| match e {
                c::descargas::EventoDescarga::Archivos {
                    ruta,
                    hechos,
                    total,
                } => progreso(Avance::nuevo(
                    v::Fase::Descarga,
                    hechos as u64,
                    total as u64,
                    &ruta,
                    "Archivos descargados y verificados",
                )),
                c::descargas::EventoDescarga::Verificacion { total } => progreso(Avance::nuevo(
                    v::Fase::Verificacion,
                    0,
                    total as u64,
                    "",
                    "Preparando la transacción",
                )),
            },
        )
        .await
        .map_err(icaro_core::diagnostico_contexto::texto)?;
        emitir(Avance::nuevo(
            v::Fase::Aplicacion,
            0,
            0,
            "",
            "Aplicando cambios revisados",
        ));
        icaro_core::diagnostico_contexto::etapa("transaccion");
        let guardia = revision.actividad.clone();
        icaro_core::diagnostico_contexto::bloqueante(move || {
            let _actividad = guardia;
            c::aplicacion::aplicar(preparada, &cancel, move |e| {
                if let c::aplicacion::EventoAplicacion::Movimiento { hechos, total } = e {
                    emitir(Avance::nuevo(
                        v::Fase::Aplicacion,
                        hechos as u64,
                        total as u64,
                        "",
                        "Movimientos guardados",
                    ));
                }
            })
            .map(|_| ())
            .map_err(icaro_core::diagnostico_contexto::texto)
        })
        .await
        .map_err(|_| "No se pudo aplicar la revisión.".to_string())?
    }
    .await;
    conexion.cerrar().await;
    r
}
async fn conectar(
    identidad: &Identidad,
    r: &c::invitaciones::Reencuentro,
    modo: c::transporte::Modo,
    host: Option<Arc<c::anfitrion::PublicacionActiva>>,
    cancel: &CancellationToken,
) -> Result<c::transporte::Cliente, String> {
    let origen = c::Origen {
        anfitrion: r.host.clone(),
        publicacion: r.publication.clone(),
    };
    let direccion = r
        .meeting
        .direccion(&r.host, &r.publication)
        .map_err(icaro_core::diagnostico_contexto::texto)?;
    tokio::select! {_=cancel.cancelled()=>Err("Conexión cancelada.".into()),r=async {match host {Some(h)=>h.conectar(&r.meeting,&origen).await,None=>c::transporte::Cliente::conectar(identidad,direccion,&origen,modo,c::Limites::default()).await}}=>r.map_err(icaro_core::diagnostico_contexto::texto)}
}
