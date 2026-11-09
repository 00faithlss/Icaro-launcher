// SPDX-License-Identifier: GPL-3.0-only
//! Escenas del controlador real, con claves en memoria y sin tareas de red/disco.
use super::{
    invitado_tareas::{Resuelta, Visita},
    servicio::Servicio,
    Etapa,
};
use icaro_core::compartir::{
    self as c,
    identidad::{AlmacenClave, Identidad, Zeroizing},
    invitaciones::{Codigo, EncuentroFirmado, InvitacionResuelta},
};
pub fn preparar(app: &mut crate::App, escena: &str) {
    struct Memoria;
    impl AlmacenClave for Memoria {
        fn leer(&self) -> Result<Option<Zeroizing<Vec<u8>>>, c::Error> {
            Ok(Some(Zeroizing::new(vec![23; 32])))
        }
        fn guardar(&self, _: &[u8]) -> Result<(), c::Error> {
            unreachable!("fixture con clave fija")
        }
    }
    crate::servidores_reales::preparar_diseno(app, "servidores");
    app.seccion = icaro_ui::shell::Seccion::Servidores;
    let identidad = Identidad::desde_almacen(&Memoria).unwrap();
    let origen = c::Origen {
        anfitrion: identidad.clave_publica(),
        publicacion: "ab".repeat(16),
    };
    let manifiesto = c::Manifiesto {
        esquema: "icaro-compartir/1".into(),
        origen: origen.clone(),
        revision: 3,
        perfil: c::Perfil {
            minecraft: "1.21.1".into(),
            loader: "fabric".into(),
            loader_version: Some("0.16.8".into()),
        },
        archivos: [
            "mods/punchy.jar",
            "resourcepacks/texturas.zip",
            "config/mod.toml",
        ]
        .into_iter()
        .map(|ruta| c::Artefacto {
            ruta: c::RutaCompartida::parse(ruta).unwrap(),
            bytes: 1234,
            sha512: "ab".repeat(64),
            procedencia: c::Procedencia::Propio,
        })
        .collect(),
        packs: vec!["vanilla".into(), "file/texturas.zip".into()],
    };
    let respuesta = InvitacionResuelta {
        host: origen.anfitrion.clone(),
        publication: origen.publicacion.clone(),
        meeting: EncuentroFirmado::crear(
            &identidad,
            &origen.publicacion,
            vec!["fixture-sin-conexion".into()],
        )
        .unwrap(),
        expires_at: 1,
        capacity: 10,
        accepted: 1,
        authorized: true,
    };
    let s = &mut app.servicios.compartir;
    s.habilitado = true;
    s.invitado.visita = Some(Visita {
        invitacion: Resuelta {
            identidad,
            servicio: Servicio {
                url: "http://127.0.0.1:1".into(),
                modo: c::transporte::Modo::Local,
            },
            codigo: Codigo::parse("7QKM4XPTR2C9").unwrap(),
            respuesta,
        },
        manifiesto,
        confianza: c::confianza::EstadoConfianza::PrimeraVez,
    });
    s.invitado.nombre = "Supervivencia compartida".into();
    if escena.contains("sync-error") {
        s.sincronizar.instancia = Some(icaro_core::instancias::Instancia {
            id: "ab".repeat(16),
            nombre: "Supervivencia compartida".into(),
            minecraft: "1.21.1".into(),
            loader: "fabric".into(),
            portada: 0,
            creada: 0,
        });
        s.sincronizar.error = Some("El equipo anfitrión está desconectado o el acceso dejó de estar vigente. Vuelve a comprobar la publicación antes de jugar.".into());
        s.sincronizar.detalle = true;
        s.etapa = Etapa::ErrorVinculo;
        return;
    }
    s.invitado.recuperando = escena.contains("recuperacion");
    s.etapa = if escena.contains("destino") || escena.contains("recuperacion") {
        Etapa::Destino
    } else if escena.contains("progreso") {
        s.invitado.avance = super::avance::Avance::nuevo(
            icaro_ui::compartir::Fase::Descarga,
            2,
            5,
            "resourcepacks/texturas.zip",
            "Archivos descargados y verificados",
        );
        Etapa::InstalandoInvitado
    } else if escena.contains("revision") {
        Etapa::RevisionInvitado
    } else {
        Etapa::Identidad
    };
}
