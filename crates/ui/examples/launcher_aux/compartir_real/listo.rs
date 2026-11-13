// SPDX-License-Identifier: GPL-3.0-only
//! CompartidoListo: la fase justo después de publicar, con el código, quién se
//! unió y los ajustes del código. Todo vive también en la publicación.
use super::{acciones, Estado, Etapa};
use crate::Mensaje;
use icaro_ui::compartir as v;
use iced::Task;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[derive(Debug, Default)]
pub struct Listo {
    /// «Revisión 1 · 21 mods, 9 configuraciones y 2 scripts».
    pub resumen: String,
    pub vence: Option<SystemTime>,
    pub confirmar_cambio: bool,
    /// Tras anular, se genera otro código (Cambiar código).
    pub regenerar: bool,
    pub codigo_nuevo: bool,
    pub copiado_mensaje: bool,
}

/// Resumen de lo publicado a partir de la frase de la revisión.
pub fn resumen(revision: u64, frase: &str) -> String {
    let lista = frase
        .strip_prefix("Se publican ")
        .and_then(|f| f.split('.').next())
        .unwrap_or_default();
    if lista.is_empty() {
        format!("Revisión {revision}")
    } else {
        format!("Revisión {revision} · {lista}")
    }
}

pub fn vence_desde(segundos: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(segundos)
}

/// Modelo de la fase con lo que hay en memoria.
pub fn modelo(s: &Estado) -> Option<v::CompartidoListo> {
    let p = s.panel.as_ref()?;
    let ahora = SystemTime::now();
    let inv = p.invitacion.as_ref();
    let vencido = s.listo.vence.is_some_and(|v| v <= ahora);
    let estado = match inv.map(|i| &i.estado) {
        None | Some(v::EstadoInvitacion::Generando) => v::EstadoCodigo::Generando,
        Some(v::EstadoInvitacion::Revocada) => v::EstadoCodigo::Anulado,
        Some(v::EstadoInvitacion::Vencida { .. }) => v::EstadoCodigo::Vencido,
        Some(_) if vencido => v::EstadoCodigo::Vencido,
        Some(v::EstadoInvitacion::Completa) => v::EstadoCodigo::Completo,
        Some(_) if inv.is_some_and(|i| i.cupo > 0 && i.aceptados >= i.cupo) => {
            v::EstadoCodigo::Completo
        }
        Some(_) if s.listo.codigo_nuevo => v::EstadoCodigo::Nuevo,
        Some(_) => v::EstadoCodigo::Vigente,
    };
    let vence = s
        .listo
        .vence
        .map(|v| crate::hora_local::hasta(v, ahora))
        .unwrap_or_default();
    let vencio = s
        .listo
        .vence
        .map(|v| {
            let f = crate::hora_local::local(v);
            format!("Venció a las {:02}:{:02}.", f.hora, f.minuto)
        })
        .unwrap_or_else(|| "Venció.".into());
    Some(v::CompartidoListo {
        instancia: p.instancia.nombre.clone(),
        resumen: s.listo.resumen.clone(),
        codigo: inv.map(|i| i.codigo.clone()).unwrap_or_default(),
        estado,
        cupo: inv.map_or(s.cupo, |i| i.cupo),
        aceptados: inv.map_or(0, |i| i.aceptados),
        personas: p
            .accesos
            .personas
            .iter()
            .map(|a| v::PersonaUnida {
                nombre: a.nombre.clone(),
                detalle: a.aceptado.clone(),
            })
            .collect(),
        caducidad: inv.map_or(s.caducidad, |i| i.caducidad),
        vence,
        vencio,
        confirmar_cambio: s.listo.confirmar_cambio,
        copiado: inv.is_some_and(|i| i.copiado),
        copiado_mensaje: s.listo.copiado_mensaje,
        ocupado: s.red_ocupada,
    })
}

/// «Únete a mi instancia Amigos en Ícaro: …», con la fecha de vencimiento.
pub fn mensaje(instancia: &str, codigo: &str, vence: Option<SystemTime>) -> String {
    let mut m = format!(
        "Únete a mi instancia {instancia} en Ícaro: abre Servidores › Unirme con un código y pega {codigo}."
    );
    if let Some(v) = vence {
        m.push_str(&format!(" Vence {}.", crate::hora_local::dia_y_hora(v)));
    }
    m
}

/// Acciones propias de la fase; `None` deja pasar las demás.
pub fn accion(s: &mut Estado, a: &v::Accion) -> Option<Task<Mensaje>> {
    if s.etapa != Etapa::Listo {
        return None;
    }
    match a {
        v::Accion::CopiarMensaje => {
            let p = s.panel.as_ref()?;
            let codigo = p.invitacion.as_ref()?.codigo.clone();
            let texto = mensaje(&p.instancia.nombre, &codigo, s.listo.vence);
            s.listo.copiado_mensaje = true;
            Some(iced::clipboard::write(texto))
        }
        v::Accion::CopiarCodigo => {
            s.listo.copiado_mensaje = false;
            None
        }
        v::Accion::PedirCambioCodigo => {
            s.listo.confirmar_cambio = true;
            Some(Task::none())
        }
        v::Accion::CancelarCambioCodigo => {
            s.listo.confirmar_cambio = false;
            Some(Task::none())
        }
        v::Accion::CambiarCodigo => {
            s.listo.confirmar_cambio = false;
            s.listo.regenerar = true;
            Some(acciones::revocar_codigo(s))
        }
        v::Accion::RevocarCodigo | v::Accion::GenerarInvitacion => {
            s.listo.codigo_nuevo = false;
            None
        }
        v::Accion::Cerrar | v::Accion::Cancelar => {
            // «Listo»: vuelve a Instancias compartidas; la publicación sigue activa.
            s.cerrar();
            s.compartidas = Some(v::PestanaCompartidas::Compartes);
            Some(Task::none())
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn el_resumen_toma_la_lista_de_la_frase() {
        assert_eq!(
            super::resumen(
                1,
                "Se publican 21 mods, 9 configuraciones y 2 scripts. 30 quedan privados."
            ),
            "Revisión 1 · 21 mods, 9 configuraciones y 2 scripts"
        );
        assert_eq!(super::resumen(3, ""), "Revisión 3");
    }

    #[test]
    fn el_mensaje_lleva_codigo_y_pasos() {
        let m = super::mensaje("Amigos", "7QKM-4XPT-R2C9", None);
        assert_eq!(
            m,
            "Únete a mi instancia Amigos en Ícaro: abre Servidores › Unirme con un código y pega 7QKM-4XPT-R2C9."
        );
    }
}
