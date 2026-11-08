// SPDX-License-Identifier: GPL-3.0-only
use super::Estado;
use icaro_core::compartir::{
    identidad::{huella_publica, sello_publico},
    Manifiesto,
};
use icaro_ui::{compartir as v, tema::Paleta};
use iced::Element;
fn instancia(m: &Manifiesto) -> v::InstanciaCompartida {
    v::InstanciaCompartida {
        nombre: "Instancia compartida".into(),
        minecraft: m.perfil.minecraft.clone(),
        loader: match &m.perfil.loader_version {
            Some(version) => format!("{} {version}", m.perfil.loader),
            None => m.perfil.loader.clone(),
        },
        mods: m
            .archivos
            .iter()
            .filter(|a| a.ruta.as_str().starts_with("mods/"))
            .count(),
        ..Default::default()
    }
}
pub fn identidad<'a>(p: Paleta, s: &Estado) -> Option<Element<'a, v::Accion>> {
    let visita = s.invitado.visita.as_ref()?;
    let m = &visita.manifiesto;
    Some(v::identidad_anfitrion(
        p,
        &v::IdentidadAnfitrion {
            anfitrion: "Equipo anfitrión".into(),
            instancia: instancia(m),
            revision: m.revision,
            huella: huella_publica(&m.origen.anfitrion).ok()?,
            sello: sello_publico(&m.origen.anfitrion).ok()?,
            confianza: s.invitado.confianza.clone(),
            confirmar_nueva: s.invitado.confirmar_nueva,
        },
    ))
}
pub fn destino<'a>(p: Paleta, s: &Estado) -> Option<Element<'a, v::Accion>> {
    let m = &s.invitado.visita.as_ref()?.manifiesto;
    let i = instancia(m);
    Some(v::destino_nuevo(
        p,
        &v::DestinoNuevo {
            nombre: s.invitado.nombre.clone(),
            error_nombre: s.invitado.error.clone(),
            anfitrion: "Equipo anfitrión".into(),
            revision: m.revision,
            mods: i.mods.to_string(),
            instancia: i,
            carpetas: "mods · config · kubejs · resourcepacks".into(),
            descarga: format!(
                "{} bytes de contenido compartido",
                m.archivos.iter().map(|a| a.bytes).sum::<u64>()
            ),
        },
    ))
}
pub fn revision<'a>(p: Paleta, s: &Estado) -> Option<Element<'a, v::Accion>> {
    Some(v::revision_sincronizacion(
        p,
        &super::invitado_instalacion::revision(s.invitado.visita.as_ref()?, &s.invitado.nombre),
    ))
}
pub fn progreso<'a>(p: Paleta, s: &Estado) -> Option<Element<'a, v::Accion>> {
    let m = &s.invitado.visita.as_ref()?.manifiesto;
    if let Some(error) = &s.invitado.error {
        return Some(v::error_compartir(p,&v::ErrorCompartir {
            tipo:if s.invitado.revisar_nueva {v::TipoError::RevisionNueva}else{v::TipoError::Hash},titulo:"No se completó la instalación compartida".into(),
            explicacion:error.clone(),conservacion:"El destino y los archivos verificados se conservan para reintentar. La revisión no se marca aplicada si falla.".into(),detalle:s.invitado.ver_detalle.then(||format!("Minecraft {} · {} · revisión {}",m.perfil.minecraft,m.perfil.loader,m.revision)),
        }));
    }
    Some(v::progreso_sincronizacion(p,&v::ProgresoSincronizacion {
        instancia:s.invitado.nombre.clone(),anfitrion:"Equipo anfitrión".into(),revision:m.revision,
        estado:if s.invitado.completada {v::EstadoProgreso::Completado {resumen:"Perfil instalado y contenido verificado; no se ha iniciado el juego.".into()}} else {v::EstadoProgreso::Activo(v::Fase::Preparacion)},
        porcentaje:if s.invitado.completada {100.}else{0.},
        detalle:"Instalando el perfil y verificando el contenido compartido. Puedes cancelar y esperar a que finalice la operación.".into(),archivo:String::new(),
    }))
}
