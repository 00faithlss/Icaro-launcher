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
    s.revision = Some(revision_ejemplo("compartir-publicacion"));
}

/// Fila de ejemplo como las de la preview: marcadas las que ya se publicaban.
#[allow(clippy::too_many_arguments)]
fn fila(
    ruta: &str,
    nombre: &str,
    version: &str,
    bytes: u64,
    grupo: GrupoPublicacion,
    marca: Marca,
    cliente: bool,
    de_mod: Option<(&str, &str)>,
) -> CambioPublicacion {
    let (si, no) = match grupo {
        GrupoPublicacion::Nuevo | GrupoPublicacion::Privado => (Eleccion::Compartir, Eleccion::Privado),
        GrupoPublicacion::Retiradas => (Eleccion::Retirar, Eleccion::Posponer),
        _ => (Eleccion::Incluir, Eleccion::Posponer),
    };
    let publicada = version.split(" →").next().filter(|_| version.contains('→')).unwrap_or_default();
    CambioPublicacion {
        archivo: ArchivoCompartido {
            id: ruta.into(),
            nombre: nombre.into(),
            detalle: ruta.into(),
            tamano: if bytes == 0 { String::new() } else { crate::compartir_real::tamano(bytes) },
            carpeta: ruta.split('/').next().unwrap_or_default().into(),
            ..Default::default()
        },
        tipo: TipoContenido::de_ruta(ruta),
        grupo,
        marca,
        eleccion: if matches!(grupo, GrupoPublicacion::Nuevo | GrupoPublicacion::Privado) { no } else { si },
        si,
        no,
        version: version.into(),
        version_publicada: publicada.into(),
        de_mod: de_mod.map(|(id, n)| (id.into(), n.into())),
        propio: false,
        autorizado: true,
        solo_cliente: cliente,
        integrado: false,
        problema: None,
        estado: String::new(),
    }
}

/// Datos equivalentes a los de la preview de RevisionPublicacion (versión 32).
fn cambios_ejemplo() -> Vec<CambioPublicacion> {
    use GrupoPublicacion::*;
    let nuevo = |ruta: &str, nombre: &str, v: &str, b: u64, cliente: bool| fila(ruta, nombre, v, b, Nuevo, Marca::Alta, cliente, None);
    let mut c = vec![
        nuevo("mods/fabric-api-0.110.0+1.21.1.jar", "Fabric API", "0.110.0", 2_301_337, false),
        nuevo("mods/Clumps-fabric-1.21.1-19.0.0.1.jar", "Clumps", "19.0.0.1", 20_154, false),
        nuevo("mods/entityculling-fabric-1.7.2-mc1.21.jar", "Entity Culling", "1.7.2", 118_420, true),
        nuevo("mods/sodium-fabric-0.6.5+mc1.21.1.jar", "Sodium", "0.6.5", 1_183_002, true),
        nuevo("mods/iris-fabric-1.8.1+mc1.21.1.jar", "Iris Shaders", "1.8.1", 2_620_114, true),
        nuevo("mods/modmenu-11.0.3.jar", "Mod Menu", "11.0.3", 610_002, true),
        nuevo("mods/cloth-config-15.0.140-fabric.jar", "Cloth Config", "15.0.140", 1_040_221, false),
        nuevo("mods/ferritecore-7.0.2-fabric.jar", "FerriteCore", "7.0.2", 96_310, false),
        nuevo("mods/ImmediatelyFast-Fabric-1.3.4+1.21.1.jar", "ImmediatelyFast", "1.3.4", 380_660, true),
        nuevo("mods/modernfix-fabric-5.20.0+mc1.21.1.jar", "ModernFix", "5.20.0", 702_118, false),
        nuevo("mods/zoomify-2.14.1+1.21.1.jar", "Zoomify", "2.14.1", 1_402_993, true),
        nuevo("mods/continuity-3.0.0+1.21.jar", "Continuity", "3.0.0", 165_430, true),
        nuevo("mods/sound-physics-remastered-fabric-1.21.1-1.4.8.jar", "Sound Physics Remastered", "1.4.8", 312_440, true),
        nuevo("mods/appleskin-fabric-mc1.21-3.0.6.jar", "AppleSkin", "3.0.6", 89_120, false),
        nuevo("mods/Jade-1.21.1-Fabric-15.8.3.jar", "Jade", "15.8.3", 1_120_557, false),
        nuevo("mods/Xaeros_Minimap_24.6.1_Fabric_1.21.jar", "Xaero's Minimap", "24.6.1", 2_980_004, true),
        nuevo("mods/XaerosWorldMap_1.39.2_Fabric_1.21.jar", "Xaero's World Map", "1.39.2", 2_410_778, true),
        nuevo("mods/waystones-fabric-1.21.1-21.1.4.jar", "Waystones", "21.1.4", 640_305, false),
        nuevo("mods/balm-fabric-1.21.1-21.0.20.jar", "Balm", "21.0.20", 520_118, false),
        nuevo("mods/FarmersDelight-1.21.1-3.1.6+refabricated.jar", "Farmer's Delight", "3.1.6", 4_210_330, false),
        nuevo("mods/supplementaries-1.21-3.0.12-fabric.jar", "Supplementaries", "3.0.12", 6_820_440, false),
        nuevo("mods/moonlight-1.21-2.13.30-fabric.jar", "Moonlight Lib", "2.13.30", 1_610_402, false),
        nuevo("mods/architectury-13.0.8-fabric.jar", "Architectury API", "13.0.8", 488_210, false),
        nuevo("mods/dynamic-fps-3.7.7+minecraft-1.21.0-fabric.jar", "Dynamic FPS", "3.7.7", 240_118, true),
        nuevo("mods/emi-1.1.18+1.21.1+fabric.jar", "EMI", "1.1.18", 1_022_660, false),
        nuevo("mods/PresenceFootsteps-1.11.1+1.21.jar", "Presence Footsteps", "1.11.1", 690_014, true),
        nuevo("mods/fallingleaves-1.17.0+1.21.1.jar", "Falling Leaves", "1.17.0", 120_330, true),
        nuevo("mods/bobby-5.2.4+mc1.21.jar", "Bobby", "5.2.4", 160_002, true),
        nuevo("mods/BetterF3-11.0.3-Fabric-1.21.1.jar", "BetterF3", "11.0.3", 205_440, true),
        nuevo("mods/krypton-0.2.8.jar", "Krypton", "0.2.8", 88_402, false),
        nuevo("mods/comforts-fabric-9.0.2+1.21.1.jar", "Comforts", "9.0.2", 300_660, false),
        nuevo("mods/chipped-fabric-1.21.1-4.0.2.jar", "Chipped", "4.0.2", 9_402_100, false),
        nuevo("mods/boys-banners-1.0.2.jar", "boys-banners", "1.0.2", 1_402_118, false),
        fila("mods/lithium-fabric-0.14.5+mc1.21.1.jar", "Lithium", "0.14.3 → 0.14.5", 720_118, Actualizaciones, Marca::Actualizacion, false, None),
        fila("mods/create-fabric-0.5.1-j-build.1631+mc1.21.1.jar", "Create Fabric", "0.5.1-i → 0.5.1-j", 18_220_440, Actualizaciones, Marca::Actualizacion, false, None),
        fila("mods/bettercombat-fabric-2.0.3+1.21.1.jar", "Better Combat", "2.0.3", 0, Retiradas, Marca::Retirada, false, None),
        fila("mods/freecam-fabric-1.3.0+1.21.jar", "Freecam", "1.3.0", 0, Retiradas, Marca::Retirada, true, None),
        fila("mods/Debugify-1.21.1+1.0.jar", "Debugify", "1.0", 330_220, Privado, Marca::Conservado, true, None),
    ];
    for (archivo, de, mod_nombre, bytes) in [
        ("config/entityculling.json", "mods/entityculling-fabric-1.7.2-mc1.21.jar", "Entity Culling", 1_200),
        ("config/sodium-options.json", "mods/sodium-fabric-0.6.5+mc1.21.1.jar", "Sodium", 1_577),
        ("config/iris.properties", "mods/iris-fabric-1.8.1+mc1.21.1.jar", "Iris Shaders", 1_954),
        ("config/modmenu.json", "mods/modmenu-11.0.3.jar", "Mod Menu", 2_331),
        ("config/ferritecore.mixin.properties", "mods/ferritecore-7.0.2-fabric.jar", "FerriteCore", 2_708),
        ("config/immediatelyfast.json", "mods/ImmediatelyFast-Fabric-1.3.4+1.21.1.jar", "ImmediatelyFast", 3_085),
        ("config/modernfix-mixins.properties", "mods/modernfix-fabric-5.20.0+mc1.21.1.jar", "ModernFix", 3_462),
        ("config/zoomify.json", "mods/zoomify-2.14.1+1.21.1.jar", "Zoomify", 3_839),
        ("config/sound_physics_remastered/soundphysics.properties", "mods/sound-physics-remastered-fabric-1.21.1-1.4.8.jar", "Sound Physics Remastered", 4_216),
        ("config/jade/jade.json", "mods/Jade-1.21.1-Fabric-15.8.3.jar", "Jade", 4_593),
        ("config/xaerominimap.txt", "mods/Xaeros_Minimap_24.6.1_Fabric_1.21.jar", "Xaero's Minimap", 4_970),
        ("config/waystones-common.toml", "mods/waystones-fabric-1.21.1-21.1.4.jar", "Waystones", 5_347),
        ("config/farmersdelight-common.json", "mods/FarmersDelight-1.21.1-3.1.6+refabricated.jar", "Farmer's Delight", 5_724),
        ("config/supplementaries-common.json", "mods/supplementaries-1.21-3.0.12-fabric.jar", "Supplementaries", 6_101),
        ("config/dynamic_fps.json", "mods/dynamic-fps-3.7.7+minecraft-1.21.0-fabric.jar", "Dynamic FPS", 6_478),
        ("config/emi.css", "mods/emi-1.1.18+1.21.1+fabric.jar", "EMI", 6_855),
        ("config/fallingleaves.json", "mods/fallingleaves-1.17.0+1.21.1.jar", "Falling Leaves", 7_232),
    ] {
        let nombre = archivo.rsplit('/').next().unwrap_or_default();
        c.push(fila(archivo, nombre, "", bytes, Nuevo, Marca::Alta, false, Some((de, mod_nombre))));
    }
    c.extend([
        fila("config/lithium.properties", "lithium.properties", "Modificado", 7_609, Actualizaciones, Marca::Actualizacion, false, Some(("mods/lithium-fabric-0.14.5+mc1.21.1.jar", "Lithium"))),
        fila("config/fabric/indigo-renderer.properties", "indigo-renderer.properties", "", 900, Nuevo, Marca::Alta, false, None),
        fila("config/defaultoptions/options.txt", "options.txt", "", 1_310, Nuevo, Marca::Alta, false, None),
        fila("resourcepacks/FreshAnimations_v1.9.2.zip", "Fresh Animations", "Activado", 4_102_330, Cambios, Marca::Alta, false, None),
        fila("resourcepacks/Stay-True-1.21.zip", "Stay True", "Desactivado", 0, Cambios, Marca::Retirada, false, None),
        fila("@orden-resourcepacks", "Orden de prioridad", "Fresh Animations sube de 3 a 1", 0, Cambios, Marca::Actualizacion, false, None),
        fila("shaderpacks/ComplementaryReimagined_r5.3.zip", "Complementary Reimagined", "r5.3", 1_430_220, Nuevo, Marca::Alta, true, None),
        fila("kubejs/server_scripts/rieles.js", "rieles.js", "", 2_118, Nuevo, Marca::Alta, false, None),
        fila("kubejs/server_scripts/recetas.js", "recetas.js", "Modificado", 5_402, Actualizaciones, Marca::Actualizacion, false, None),
        fila("kubejs/client_scripts/tooltips.js", "tooltips.js", "", 1_330, Nuevo, Marca::Alta, true, None),
    ]);
    for x in &mut c {
        match x.archivo.nombre.as_str() {
            "Better Combat" => x.estado = "Lo eliminaste de tu instancia".into(),
            "Freecam" => x.estado = "Decidiste dejar de compartirlo".into(),
            "boys-banners" => {
                x.propio = true;
                x.autorizado = false;
            }
            "Supplementaries" => {
                x.problema = Some(Problema {
                    archivo: x.archivo.id.clone(),
                    titulo: "Dependencia ausente".into(),
                    detalle: "Necesita Moonlight Lib 2.17 o superior y tu instancia tiene la 2.13.".into(),
                    accion: Accion::MarcarPublicacion(x.archivo.id.clone(), false),
                    rotulo: "Dejar privado".into(),
                })
            }
            _ => {}
        }
    }
    c
}

/// Un estado de la preview por escena: `compartir-publicacion-<estado>`.
fn revision_ejemplo(escena: &str) -> RevisionPublicacion {
    let mut m = RevisionPublicacion {
        instancia: InstanciaCompartida {
            id: "fixture".into(),
            nombre: "Amigos".into(),
            minecraft: "1.21.1".into(),
            loader: "Fabric 0.16.9".into(),
            mods: 38,
            en_uso: false,
        },
        base: 7,
        nueva: 8,
        estado: EstadoRevisionPublicacion::Disponible,
        confirmar_cancelacion: false,
        guardando: false,
        vista: VistaRevision::default(),
        cambios: cambios_ejemplo(),
    };
    let atajo = |m: &mut RevisionPublicacion, a: AtajoPublicacion| {
        let (elecciones, fuera) = elecciones_de_atajo(m, a);
        for (id, e) in elecciones {
            if let Some(c) = m.cambios.iter_mut().find(|c| c.archivo.id == id) {
                c.eleccion = e;
            }
        }
        m.vista.atajo = Some(a);
        m.vista.aplicado = mensaje_atajo(m, a, &fuera);
    };
    let estado = escena
        .trim_end_matches("-angosto")
        .trim_end_matches("-piedra")
        .strip_prefix("compartir-publicacion-")
        .unwrap_or("disponibles");
    match estado {
        "analizando" => m.estado = EstadoRevisionPublicacion::Analizando { hechos: 612, total: 1204 },
        "sin-cambios" => {
            m.cambios.retain(|c| c.grupo == GrupoPublicacion::Privado);
            m.estado = EstadoRevisionPublicacion::SinCambios;
        }
        "atajo" => atajo(&mut m, AtajoPublicacion::Necesario),
        "bloqueada" => atajo(&mut m, AtajoPublicacion::Todo),
        "autorizacion" => {
            atajo(&mut m, AtajoPublicacion::Necesario);
            m.vista.atajo = None;
            m.vista.aplicado.clear();
        }
        "configuraciones" => {
            atajo(&mut m, AtajoPublicacion::Necesario);
            m.vista.abiertos = [TipoContenido::Config].into();
        }
        "buscar" => {
            m.vista.busqueda = "xaero".into();
        }
        "preparando" => {
            atajo(&mut m, AtajoPublicacion::Necesario);
            m.estado = EstadoRevisionPublicacion::Preparando { porcentaje: 46. };
        }
        "publicada" => {
            atajo(&mut m, AtajoPublicacion::Necesario);
            let resumen = frase_publicacion(&m);
            m.estado = EstadoRevisionPublicacion::Publicada { resumen };
        }
        "error" => {
            atajo(&mut m, AtajoPublicacion::Necesario);
            m.estado = EstadoRevisionPublicacion::Error(
                "boys-banners-1.0.2.jar cambió mientras se empaquetaba.".into(),
            );
        }
        "confirmar" => {
            atajo(&mut m, AtajoPublicacion::Necesario);
            m.confirmar_cancelacion = true;
        }
        _ => {}
    }
    m
}
pub fn vista<'a>(app: &'a App, escena: &str) -> Option<Element<'a, Mensaje>> {
    if !escena.starts_with("compartir-") {
        return None;
    }
    let p = app.paleta();
    let vista = match escena.trim_end_matches("-angosto") {
        "compartir-instancia" => compartir_instancia(p, &CompartirInstancia {
            instancias: vec![instancia()],
            seleccionada: None,
            estado: EstadoContenido::Bloqueado,
            incluidos: vec![],
            total_incluidos: 0,
            excluidos: vec![],
            propios: vec![],
            carpetas: "mods · config · kubejs · resourcepacks".into(),
            tamano_carpetas: String::new(),
            problemas: vec![],
        }),
        s if s.starts_with("compartir-listo") => {
            let migas = [
                Miga::enlace("Servidores", Accion::VerServidores),
                Miga::enlace("Instancias compartidas", Accion::VerCompartidas),
                Miga::actual("Compartida"),
            ];
            return Some(
                pagina(p, &migas, 700., compartido_listo(p, &listo_ejemplo(s)))
                    .map(|_| Mensaje::Nada),
            );
        }
        s if s.starts_with("compartir-selector") => {
            let migas = [
                Miga::enlace("Servidores", Accion::VerServidores),
                Miga::enlace("Instancias compartidas", Accion::VerCompartidas),
                Miga::actual("Compartir instancia"),
            ];
            return Some(
                pagina(p, &migas, 720., selector_instancia(p, &selector_ejemplo(s)))
                    .map(|_| Mensaje::Nada),
            );
        }
        s if s.starts_with("compartir-compartidas") => {
            return Some(
                instancias_compartidas(p, &compartidas_ejemplo(s))
                    .map(|_| Mensaje::Nada),
            )
        }
        s if s.starts_with("compartir-publicacion")
            || matches!(s, "compartir-privados" | "compartir-retirada" | "compartir-packs") =>
        {
            let mut m = revision_ejemplo(s);
            match s {
                "compartir-privados" => m.vista.filtro = FiltroPublicacion::NoSePublica,
                "compartir-packs" => m.vista.abiertos = [TipoContenido::Resourcepack].into(),
                _ => {}
            }
            revision_publicacion(p, &m)
        }
        "compartir-conflicto" => revision_sincronizacion(p, &RevisionSincronizacion {
            instancia:"Supervivencia compartida".into(),anfitrion:"Anfitrión conocido".into(),revision:3,estado:EstadoRevisionInvitado::Diferencias,conexion:None,
            diferencias:vec![
                Diferencia {archivo:archivo("mod.toml","config"),marca:Marca::Conflicto,del_anfitrion:true,resolucion:None},
                Diferencia {archivo:archivo("punchy.jar","mods"),marca:Marca::Alta,del_anfitrion:true,resolucion:None},
                Diferencia {archivo:archivo("personal.jar","mods"),marca:Marca::Conservado,del_anfitrion:false,resolucion:Some(Resolucion::ConservarMia)},
            ],
        }),
        "compartir-cupo" | "compartir-invitacion" | "compartir-cargando" => (if escena == "compartir-cargando" { |p, m: &PanelPublicacion| panel_publicacion_cargando(p, m, true) } else { panel_publicacion })(p, &PanelPublicacion {
            instancia:instancia(),revision:3,fecha_revision:"Hace unos minutos".into(),hash_revision:"a7f920cd82b1".into(),disponibilidad:"Disponible mientras Ícaro esté abierto".into(),transferencias:"Sin transferencias activas".into(),huella:"71ac 20f4 832d 5e90 b114 32fa".into(),contenido_privado:2,estado:EstadoPublicacion::Compartiendo,
            invitacion:Some(Invitacion {codigo:"7QKM-4XPT-R2C9".into(),estado:if escena=="compartir-cupo" {EstadoInvitacion::Completa} else {EstadoInvitacion::Vigente {restante:"23 h 41 min".into()}},cupo:10,aceptados:if escena=="compartir-cupo" {10} else {2},..Default::default()}),
            accesos:Accesos {personas:vec![Acceso {id:"fixture-invitado".into(),nombre:"Equipo invitado".into(),huella:"9e2a 7b14 08fc 621d".into(),estado:"Autorizado".into(),conectado:false,aceptado:"Hoy".into()}],confirmar:None,instancia:"Supervivencia".into()},
        }),
        "compartir-cierre" => cierre_transferencias(p,&CierreTransferencias {cerrar_icaro:true,instancia:"Supervivencia".into(),transferencias:vec![Transferencia {nombre:"texturas.zip".into(),fase:"Descargando".into(),porcentaje:64.}],accesos:2,opcion:OpcionCierre::Esperar}),
        "compartir-error" => error_compartir(p,&ErrorCompartir {tipo:TipoError::ArchivoCambiado,titulo:"El contenido cambió durante la revisión".into(),explicacion:"Un archivo se modificó después de comprobarlo. Revisa de nuevo antes de publicar.".into(),conservacion:"Tu revisión publicada y tus archivos personales se conservan.".into(),detalle:Some("config/mod.toml".into())}),
        "compartir-propios" => confirmar_propios(p,&ConfirmarPropios {instancia:"Supervivencia".into(),revision:3,archivos:vec![archivo("texturas.zip","resourcepacks"),archivo("inicio.js","kubejs")],autorizado:false}),
        "compartir-destino" => destino_nuevo(p,&DestinoNuevo {recuperando:false,nombre:"Supervivencia compartida".into(),error_nombre:None,anfitrion:"Anfitrión conocido".into(),instancia:instancia(),revision:3,mods:"24 mods".into(),carpetas:"config · kubejs · resourcepacks".into(),descarga:"48,2 MB".into()}),
        _ => entrada_invitacion(p,&EntradaInvitacion {codigo:"7QKM-4XPT-R2C9".into(),estado:EstadoEntrada::Valido {anfitrion:"Supervivencia".into(),disponibles:8}}),
    };
    Some(
        container(vista.map(|_| Mensaje::Nada))
            .center_x(Length::Fill)
            .padding(16)
            .into(),
    )
}

/// Datos de la preview de InstanciasCompartidas.
fn compartidas_ejemplo(escena: &str) -> InstanciasCompartidas {
    let fila = |id: &str, nombre: &str, estado, revision, personas, version: &str, codigo: Option<&str>, caduca: &str| {
        InstanciaQueCompartes {
            id: id.into(),
            nombre: nombre.into(),
            estado,
            revision,
            personas,
            version: version.into(),
            codigo: codigo.map(Into::into),
            caduca: caduca.into(),
            copiado: false,
        }
    };
    let recibida = |id: &str, nombre: &str, de: &str, estado, revision, cambios, version: &str| {
        InstanciaQueRecibes {
            id: id.into(),
            nombre: nombre.into(),
            anfitrion: de.into(),
            estado,
            revision: Some(revision),
            cambios: Some(cambios),
            version: version.into(),
        }
    };
    if escena.contains("vacio") {
        return InstanciasCompartidas {
            pestana: if escena.contains("recibes") { PestanaCompartidas::Recibes } else { PestanaCompartidas::Compartes },
            ..Default::default()
        };
    }
    InstanciasCompartidas {
        pestana: if escena.contains("recibes") { PestanaCompartidas::Recibes } else { PestanaCompartidas::Compartes },
        compartes: vec![
            fila("amigos", "Amigos", EstadoCompartida::Disponible, 4, 3, "1.21.1 · Fabric 0.16.9", Some("7QKM-4XPT-R2C9"), "caduca en 23 h"),
            fila("boys", "Boys SMP", EstadoCompartida::CambiosSinPublicar, 12, 5, "1.21.1 · NeoForge 21.1.77", Some("H3WD-8KQA-ZF47"), "caduca en 6 días"),
            fila("sky", "Skyblock", EstadoCompartida::EnPausa, 2, 1, "1.20.1 · Forge 47.3.0", None, ""),
        ],
        recibes: vec![
            recibida("s26", "Survival 26", "Equipo 9e2a 7b14", AnfitrionRecibida::Disponible, 9, 0, "1.21.1 · Fabric 0.16.9"),
            recibida("create", "Create Moderno", "Equipo 4c1d 0a77", AnfitrionRecibida::Disponible, 13, 3, "1.20.1 · Forge 47.3.0"),
            recibida("hard", "Hardcore", "Equipo b812 e3f0", AnfitrionRecibida::SinConexion, 2, 0, "1.21.4 · Vanilla"),
        ],
        menu: escena.contains("menu").then(|| "amigos".into()),
        confirmar_quitar: escena.contains("quitar").then(|| "sky".into()),
    }
}

/// Datos de la preview de SelectorInstanciaCompartir.
fn selector_ejemplo(escena: &str) -> SelectorInstancia {
    if escena.contains("vacio") {
        return SelectorInstancia::default();
    }
    let o = |n: &str, v: &str, m: &str, t: &str, j: &str, e: EstadoOpcion| OpcionInstancia {
        id: n.into(),
        nombre: n.into(),
        version: v.into(),
        mods: m.into(),
        tamano: t.into(),
        jugado: j.into(),
        estado: e,
    };
    SelectorInstancia {
        opciones: vec![
            o("Amigos", "1.21.1 · Fabric 0.16.9", "38 mods", "1,2 GB", "Jugaste hace 2 h", EstadoOpcion::Libre),
            o("Supervivencia", "1.21.4 · Fabric 0.16.9", "46 mods", "2,4 GB", "Jugaste ayer", EstadoOpcion::Libre),
            o("Boys SMP", "1.21.1 · NeoForge 21.1.77", "112 mods", "3,1 GB", "Jugaste el 05-11", EstadoOpcion::Compartida { revision: 12, personas: 3 }),
            o("Creativo", "1.21.1 · Vanilla", "Sin mods", "240 MB", "Jugaste el 28-10", EstadoOpcion::Vanilla),
            o("Better MC", "1.21.1 · NeoForge 21.1.77", "214 mods", "5,8 GB", "Jugando ahora", EstadoOpcion::EnUso),
            o("Skyblock", "1.20.1 · Forge 47.3.0", "88 mods", "2,0 GB", "Jugaste el 12-10", EstadoOpcion::Libre),
        ],
        elegida: Some("Amigos".into()),
        busqueda: if escena.contains("buscando") { "sup".into() } else { String::new() },
    }
}

/// Estados de la preview de CompartidoListo.
fn listo_ejemplo(escena: &str) -> CompartidoListo {
    let estado = escena.trim_end_matches("-angosto").trim_end_matches("-piedra");
    let personas = !estado.ends_with("-listo") && estado != "compartir-listo";
    let completo = estado.ends_with("completo");
    CompartidoListo {
        instancia: "Amigos".into(),
        resumen: "Revisión 1 · 21 mods, 9 configuraciones y 2 scripts".into(),
        codigo: if estado.ends_with("nuevo") { "B8RN-3WQF-K6TD" } else { "7QKM-4XPT-R2C9" }.into(),
        estado: if estado.ends_with("nuevo") {
            EstadoCodigo::Nuevo
        } else if completo {
            EstadoCodigo::Completo
        } else if estado.ends_with("vencido") {
            EstadoCodigo::Vencido
        } else {
            EstadoCodigo::Vigente
        },
        cupo: 10,
        aceptados: if completo { 10 } else if personas { 3 } else { 0 },
        personas: if personas {
            ["Equipo 9e2a 7b14", "Equipo 4c1d 0a77", "Equipo b812 e3f0"]
                .into_iter()
                .map(|n| PersonaUnida { nombre: n.into(), detalle: String::new() })
                .collect()
        } else {
            vec![]
        },
        caducidad: Caducidad::Horas24,
        vence: "Hasta mañana, martes 10-11 a las 04:15".into(),
        vencio: "Venció hoy a las 04:15.".into(),
        confirmar_cambio: estado.ends_with("cambiar"),
        copiado: false,
        copiado_mensaje: false,
        ocupado: false,
    }
}
