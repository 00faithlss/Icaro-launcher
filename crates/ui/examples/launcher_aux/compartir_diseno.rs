// SPDX-License-Identifier: GPL-3.0-only
//! Escenas de los componentes originales. Solo fixtures: sin red ni datos personales.
use crate::{App, Mensaje};
use icaro_ui::compartir::*;
use iced::{widget::container, Element, Length};

fn instancia() -> InstanciaCompartida {
    InstanciaCompartida {
        id: "fixture".into(),
        nombre: "Supervivencia".into(),
        minecraft: "1.21.1".into(),
        loader: "Fabric 0.16.8".into(),
        mods: 24,
        en_uso: false,
    }
}
fn archivo(nombre: &str, carpeta: &str) -> ArchivoCompartido {
    ArchivoCompartido {
        id: format!("{carpeta}/{nombre}"),
        nombre: nombre.into(),
        detalle: "Versión instalada · contenido exacto".into(),
        tamano: "1,2 MB".into(),
        procedencia: "Modrinth".into(),
        sha256: "ab12cd34ef56".repeat(5),
        carpeta: carpeta.into(),
    }
}

/// Recorre la capa real del controlador con datos visuales y acciones de demo.
pub fn preparar_controlador(app: &mut App) {
    crate::servidores_reales::preparar_diseno(app, "servidores");
    app.seccion = icaro_ui::shell::Seccion::Servidores;
    let s = &mut app.servicios.compartir;
    s.habilitado = true;
    s.etapa = crate::compartir_real::Etapa::Revision;
    s.revision = Some(RevisionPublicacion {
        instancia: instancia(),
        base: 2,
        nueva: 3,
        estado: EstadoRevisionPublicacion::Disponible,
        confirmar_cancelacion: false,
        cambios: vec![
            CambioPublicacion {
                archivo: archivo("punchy.jar", "mods"),
                grupo: GrupoPublicacion::Nuevo,
                marca: Marca::Alta,
                eleccion: Eleccion::Compartir,
                propio: false,
                autorizado: true,
                solo_cliente: false,
                integrado: false,
                problema: None,
                estado: String::new(),
            },
            CambioPublicacion {
                archivo: archivo("texturas.zip", "resourcepacks"),
                grupo: GrupoPublicacion::Resourcepacks,
                marca: Marca::Actualizacion,
                eleccion: Eleccion::Incluir,
                propio: true,
                autorizado: false,
                solo_cliente: false,
                integrado: false,
                problema: None,
                estado: String::new(),
            },
        ],
    });
}
pub fn vista<'a>(app: &'a App, escena: &str) -> Option<Element<'a, Mensaje>> {
    if !escena.starts_with("compartir-") {
        return None;
    }
    let p = app.paleta();
    let vista = match escena.trim_end_matches("-angosto") {
        "compartir-publicacion" | "compartir-privados" | "compartir-retirada" | "compartir-packs" => revision_publicacion(p, &RevisionPublicacion {
            instancia:instancia(),base:2,nueva:3,estado:EstadoRevisionPublicacion::Disponible,confirmar_cancelacion:false,
            cambios:vec![
                CambioPublicacion {archivo:archivo("punchy.jar","mods"),grupo:GrupoPublicacion::Nuevo,marca:Marca::Alta,eleccion:Eleccion::Compartir,propio:false,autorizado:true,solo_cliente:false,integrado:false,problema:None,estado:"Incluye su pack integrado".into()},
                CambioPublicacion {archivo:archivo("sodium.jar","mods"),grupo:GrupoPublicacion::Actualizaciones,marca:Marca::Actualizacion,eleccion:Eleccion::Posponer,propio:false,autorizado:true,solo_cliente:true,integrado:false,problema:None,estado:"Conserva la versión publicada".into()},
                CambioPublicacion {archivo:archivo("minimapa.jar","mods"),grupo:GrupoPublicacion::Retiradas,marca:Marca::Retirada,eleccion:Eleccion::Posponer,propio:false,autorizado:true,solo_cliente:true,integrado:false,problema:None,estado:"Retirada pospuesta".into()},
                CambioPublicacion {archivo:archivo("texturas.zip","resourcepacks"),grupo:GrupoPublicacion::Resourcepacks,marca:Marca::Actualizacion,eleccion:Eleccion::Incluir,propio:true,autorizado:false,solo_cliente:false,integrado:false,problema:None,estado:"Orden y activación compartidos".into()},
                CambioPublicacion {archivo:archivo("visual-personal.jar","mods"),grupo:GrupoPublicacion::Privado,marca:Marca::Conservado,eleccion:Eleccion::Privado,propio:false,autorizado:true,solo_cliente:true,integrado:false,problema:None,estado:"Permanece en este equipo".into()},
            ],
        }),
        "compartir-conflicto" => revision_sincronizacion(p, &RevisionSincronizacion {
            instancia:"Supervivencia compartida".into(),anfitrion:"Anfitrión conocido".into(),revision:3,estado:EstadoRevisionInvitado::Diferencias,conexion:None,
            diferencias:vec![
                Diferencia {archivo:archivo("mod.toml","config"),marca:Marca::Conflicto,del_anfitrion:true,resolucion:None},
                Diferencia {archivo:archivo("punchy.jar","mods"),marca:Marca::Alta,del_anfitrion:true,resolucion:None},
                Diferencia {archivo:archivo("personal.jar","mods"),marca:Marca::Conservado,del_anfitrion:false,resolucion:Some(Resolucion::ConservarMia)},
            ],
        }),
        "compartir-cupo" | "compartir-invitacion" => panel_publicacion(p, &PanelPublicacion {
            instancia:instancia(),revision:3,fecha_revision:"Hace unos minutos".into(),hash_revision:"a7f920cd82b1".into(),disponibilidad:"Disponible mientras Ícaro esté abierto".into(),transferencias:"Sin transferencias activas".into(),huella:"71ac 20f4 832d 5e90 b114 32fa".into(),contenido_privado:2,estado:EstadoPublicacion::Compartiendo,
            invitacion:Some(Invitacion {codigo:"7QKM-4XPT-R2C9".into(),estado:if escena=="compartir-cupo" {EstadoInvitacion::Completa} else {EstadoInvitacion::Vigente {restante:"23 h 41 min".into()}},cupo:10,aceptados:if escena=="compartir-cupo" {10} else {2},..Default::default()}),
            accesos:Accesos {personas:vec![Acceso {id:"fixture-invitado".into(),nombre:"Equipo invitado".into(),huella:"9e2a 7b14 08fc 621d".into(),estado:"Autorizado".into(),conectado:false,aceptado:"Hoy".into()}],confirmar:None,instancia:"Supervivencia".into()},
        }),
        "compartir-cierre" => cierre_transferencias(p,&CierreTransferencias {cerrar_icaro:true,instancia:"Supervivencia".into(),transferencias:vec![Transferencia {nombre:"texturas.zip".into(),fase:"Descargando".into(),porcentaje:64.}],accesos:2,opcion:OpcionCierre::Esperar}),
        "compartir-error" => error_compartir(p,&ErrorCompartir {tipo:TipoError::ArchivoCambiado,titulo:"El contenido cambió durante la revisión".into(),explicacion:"Un archivo se modificó después de comprobarlo. Revisa de nuevo antes de publicar.".into(),conservacion:"Tu revisión publicada y tus archivos personales se conservan.".into(),detalle:Some("config/mod.toml".into())}),
        "compartir-propios" => confirmar_propios(p,&ConfirmarPropios {instancia:"Supervivencia".into(),revision:3,archivos:vec![archivo("texturas.zip","resourcepacks"),archivo("inicio.js","kubejs")],autorizado:false}),
        "compartir-destino" => destino_nuevo(p,&DestinoNuevo {nombre:"Supervivencia compartida".into(),error_nombre:None,anfitrion:"Anfitrión conocido".into(),instancia:instancia(),revision:3,mods:"24 mods".into(),carpetas:"config · kubejs · resourcepacks".into(),descarga:"48,2 MB".into()}),
        _ => entrada_invitacion(p,&EntradaInvitacion {codigo:"7QKM-4XPT-R2C9".into(),estado:EstadoEntrada::Valido {anfitrion:"Supervivencia".into(),disponibles:8}}),
    };
    Some(
        container(vista.map(|_| Mensaje::Nada))
            .center_x(Length::Fill)
            .padding(16)
            .into(),
    )
}
