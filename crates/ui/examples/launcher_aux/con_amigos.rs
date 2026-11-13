// SPDX-License-Identifier: GPL-3.0-only
//! PantallaServidores › Con amigos: maestro y detalle. A la izquierda,
//! Compartir una instancia, Unirme con un código y las listas Compartes y
//! Recibes; a la derecha, el detalle de lo elegido o el flujo en curso. Las
//! acciones reales son las de `compartir_real`: esta vista solo las ordena.
use crate::compartir_real::{self as cr, Etapa};
use crate::servidores_vista::pixel;
use crate::{App, Mensaje};
use icaro_ui::{
    compartir as v,
    componentes::{self, Variante},

    iconos::Icono,
    pantalla_servidores::{self as sv, Tono},
    tema::borde,
};
use iced::widget::{column, container, row, Space};
use iced::{Alignment, Element, Length, Padding, Task};
use std::time::Instant;

/// Lo elegido en Con amigos que no vive en `compartir_real`.
#[derive(Debug, Default)]
pub struct Estado {
    /// Instancia recibida elegida (las compartidas se abren como publicación).
    pub recibida: Option<String>,
    pub confirmar: Option<Confirmacion>,
    pub copiado: Option<(&'static str, Instant)>,
    pub saliendo: bool,
    /// Compartir › «Elegir a mano» y la revisión completa abierta.
    pub mano: bool,
    pub completa: bool,
    /// Unirme: los tres grupos de 4 del código.
    pub grupos: [String; 3],
    /// Escenas de diseño: listas de muestra en vez de las reales.
    pub muestra: Option<v::InstanciasCompartidas>,
}

/// Compartes y Recibes: las reales o, en una escena de diseño, las de muestra.
pub fn modelo(app: &App) -> v::InstanciasCompartidas {
    app.servicios.servidores.amigos.muestra.clone().unwrap_or_else(|| cr::modelo_compartidas(app))
}

/// Lo que pide confirmar en el mismo lugar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Confirmacion {
    CambiarCodigo,
    Persona(String),
    Dejar,
    Salir(String),
    /// Quitar de Compartes una publicación en pausa o detenida.
    Quitar(String),
}

#[derive(Debug, Clone)]
pub enum Accion {
    Compartida(String),
    Recibida(String),
    Compartir,
    Unirme,
    CerrarFlujo,
    CopiarCodigo,
    CopiarMensaje,
    Pedir(Confirmacion),
    Cancelar,
    Confirmar,
    Caducidad(v::Caducidad),
    Cupo(u32),
    Pausar(String),
    Reanudar(String),
    Publicar(String),
    Sincronizar(String),
    Jugar(String),
    Salido(String, Result<(), String>),
    Activar,
    // Compartir una instancia
    Opcion(String),
    Continuar,
    Atajo(v::AtajoPublicacion),
    Mano,
    RevisionCompleta(bool),
    FCaducidad(v::Caducidad),
    FCupo(u32),
    Atras,
    VerNueva,
    // Unirme con un código
    Grupo(usize, String),
    /// «Pegar»: lee el portapapeles y lo reparte en los tres grupos.
    Pegar,
    Pegado(String),
    /// «Tengo un código» y las celdas grandes: llevan al campo de la izquierda.
    EnfocarCodigo,
    UnirmeDescargar,
    ConfirmarHuella(bool),
}

pub(crate) fn m(a: Accion) -> Mensaje {
    Mensaje::Servidores(crate::servidores_reales::Accion::Amigos(a))
}

fn vista_compartir(a: v::Accion) -> Mensaje {
    Mensaje::Compartir(cr::Accion::Vista(a))
}

fn despachar(app: &mut App, a: v::Accion) -> Task<Mensaje> {
    cr::actualizar(app, cr::Accion::Vista(a))
}

/// Al entrar a la pestaña se leen las publicaciones y las recibidas.
pub fn entrar(app: &mut App) -> Task<Mensaje> {
    if app.servicios.demo || !app.servicios.compartir.habilitado {
        return Task::none();
    }
    despachar(app, v::Accion::VerCompartidas)
}

/// Contador de la pestaña: lo que compartes más lo que recibes.
pub fn cantidad(app: &App) -> usize {
    if !app.servicios.compartir.habilitado {
        return 0;
    }
    let modelo = modelo(app);
    modelo.compartes.len() + modelo.recibes.len()
}

pub fn actualizar(app: &mut App, a: Accion) -> Task<Mensaje> {
    let estado = &mut app.servicios.servidores.amigos;
    match a {
        Accion::Compartida(id) => {
            estado.recibida = None;
            estado.confirmar = None;
            let misma = app.servicios.compartir.panel.as_ref().is_some_and(|p| p.instancia.id == id)
                && app.servicios.compartir.etapa == Etapa::Panel;
            if !misma && !soltar_flujo(app) {
                return Task::none();
            }
            let aj = &mut app.servicios.servidores.alojar;
            if aj.editor.as_ref().is_some_and(|e| e.instancia != id) {
                aj.editor = None;
            }
            despachar(app, v::Accion::VerPublicacionDe(id))
        }
        Accion::Recibida(id) => {
            estado.recibida = Some(id.clone());
            estado.confirmar = None;
            app.servicios.servidores.alojar.editor = None;
            // Se pregunta al anfitrión si tiene su servidor abierto.
            let servidor = crate::alojamiento_real::actualizar(app, crate::alojamiento_real::Accion::Consultar(id));
            // Lo abierto a la derecha se cierra para mostrar la recibida.
            // Si algo no se puede interrumpir, se queda a la vista con su aviso.
            let _ = soltar_flujo(app);
            servidor
        }
        Accion::Compartir => {
            estado.confirmar = None;
            estado.recibida = None;
            if crate::con_amigos_flujos::paso(app.servicios.compartir.etapa).is_none() && !soltar_flujo(app) {
                return Task::none();
            }
            despachar(app, v::Accion::AbrirCompartir)
        }
        Accion::Unirme => {
            estado.confirmar = None;
            estado.recibida = None;
            if !es_unirme(app.servicios.compartir.etapa) {
                if !soltar_flujo(app) {
                    return Task::none();
                }
                app.servicios.servidores.amigos.grupos = Default::default();
            }
            despachar(app, v::Accion::AceptarInvitacion)
        }
        Accion::CerrarFlujo => despachar(app, v::Accion::Cancelar),
        Accion::CopiarCodigo => {
            estado.copiado = Some(("codigo", Instant::now()));
            app.ahora = Instant::now();
            despachar(app, v::Accion::CopiarCodigo)
        }
        Accion::CopiarMensaje => {
            let Some(texto_m) = cr::mensaje_codigo(&app.servicios.compartir) else {
                return Task::none();
            };
            app.servicios.servidores.amigos.copiado = Some(("mensaje", Instant::now()));
            app.ahora = Instant::now();
            iced::clipboard::write(texto_m)
        }
        Accion::Pedir(c) => {
            estado.confirmar = Some(c);
            Task::none()
        }
        Accion::Cancelar => {
            estado.confirmar = None;
            Task::none()
        }
        Accion::Confirmar => match estado.confirmar.take() {
            Some(Confirmacion::CambiarCodigo) => cr::cambiar_codigo(&mut app.servicios.compartir),
            Some(Confirmacion::Persona(id)) => despachar(app, v::Accion::RevocarAcceso(id)),
            Some(Confirmacion::Dejar) => despachar(app, v::Accion::DejarDeCompartir),
            Some(Confirmacion::Quitar(id)) => {
                // Se cierra el detalle: la publicación deja de existir.
                let _ = soltar_flujo(app);
                let s = &mut app.servicios.compartir;
                if s.publicacion.as_ref().is_some_and(|p| p.instancia_id == id) {
                    s.publicacion = None;
                    s.panel = None;
                }
                despachar(app, v::Accion::ConfirmarQuitarDe(id))
            }
            Some(Confirmacion::Salir(id)) => {
                let Ok(juego) = app.servicios.instancias.almacen.juego(&id) else {
                    return Task::none();
                };
                app.servicios.servidores.amigos.saliendo = true;
                Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            icaro_core::compartir::vinculos::desvincular(&juego)
                                .map_err(icaro_core::diagnostico_contexto::texto)
                        })
                        .await
                        .unwrap_or_else(|_| Err("No se pudo salir de la instancia.".into()))
                    },
                    move |r| m(Accion::Salido(id.clone(), r)),
                )
            }
            None => Task::none(),
        },
        Accion::Salido(id, r) => {
            estado.saliendo = false;
            match r {
                Ok(()) => {
                    if estado.recibida.as_deref() == Some(id.as_str()) {
                        estado.recibida = None;
                    }
                    app.servicios.compartir.recibidas.retain(|r| r.fila.id != id);
                    app.avisar("Saliste de la instancia. Queda en tu equipo como una normal.".into());
                    entrar(app)
                }
                Err(e) => {
                    app.avisar_error(format!("No se pudo salir de la instancia: {e}"));
                    Task::none()
                }
            }
        }
        Accion::Caducidad(c) => despachar(app, v::Accion::Caducidad(c)),
        Accion::Cupo(n) => despachar(app, v::Accion::Cupo(n)),
        Accion::Pausar(id) => despachar(app, v::Accion::PausarDe(id)),
        Accion::Reanudar(id) => despachar(app, v::Accion::ReanudarDe(id)),
        Accion::Publicar(id) => despachar(app, v::Accion::RevisarCambiosDe(id)),
        Accion::Sincronizar(id) => despachar(app, v::Accion::SincronizarRecibida(id)),
        Accion::Jugar(id) => despachar(app, v::Accion::JugarRecibida(id)),
        Accion::Activar => cr::actualizar(app, cr::Accion::ActivarExperimental),
        Accion::Opcion(id) => despachar(app, v::Accion::SeleccionarInstancia(id)),
        Accion::Continuar => {
            estado.mano = false;
            estado.completa = false;
            despachar(app, v::Accion::Continuar)
        }
        Accion::Atajo(a) => {
            estado.mano = false;
            estado.completa = false;
            despachar(app, v::Accion::AtajoPublicacion(a))
        }
        Accion::Mano => {
            estado.mano = true;
            Task::none()
        }
        Accion::RevisionCompleta(b) => {
            estado.completa = b;
            Task::none()
        }
        Accion::FCaducidad(c) => {
            app.servicios.compartir.caducidad = c;
            Task::none()
        }
        Accion::FCupo(n) => {
            app.servicios.compartir.cupo = n.clamp(1, 50);
            Task::none()
        }
        Accion::Atras => {
            // Vuelve a elegir la instancia, con la misma marcada.
            estado.mano = false;
            estado.completa = false;
            let elegida = app.servicios.compartir.seleccionada.clone();
            app.servicios.compartir.cerrar();
            let t = despachar(app, v::Accion::AbrirCompartir);
            app.servicios.compartir.seleccionada = elegida;
            t
        }
        Accion::VerNueva => {
            estado.recibida = None;
            despachar(app, v::Accion::VerPublicacion)
        }
        Accion::Pegar => iced::clipboard::read().map(|c| m(Accion::Pegado(c.unwrap_or_default()))),
        Accion::Pegado(texto_p) => {
            estado.grupos = Default::default();
            actualizar(app, Accion::Grupo(0, texto_p))
        }
        Accion::EnfocarCodigo => {
            let abrir = if es_unirme(app.servicios.compartir.etapa) {
                Task::none()
            } else {
                actualizar(app, Accion::Unirme)
            };
            let id = iced::widget::text_input::Id::new(crate::con_amigos_flujos::ID_GRUPO[0]);
            Task::batch([abrir, iced::widget::text_input::focus(id)])
        }
        Accion::Grupo(i, texto_g) => {
            // El campo vive en la columna izquierda: escribir abre Unirme.
            if !es_unirme(app.servicios.compartir.etapa) {
                if !soltar_flujo(app) {
                    return Task::none();
                }
                app.servicios.servidores.amigos.recibida = None;
                let _ = despachar(app, v::Accion::AceptarInvitacion);
            }
            let estado = &mut app.servicios.servidores.amigos;
            crate::con_amigos_flujos::repartir(&mut estado.grupos, i, &texto_g);
            let grupos = estado.grupos.clone();
            let s = &mut app.servicios.compartir;
            // Escribir limpia el error.
            if !matches!(s.invitado.entrada, v::EstadoEntrada::Vacio) {
                s.invitado.entrada = v::EstadoEntrada::Vacio;
                s.invitado.resuelta = None;
            }
            use iced::widget::text_input::{focus, move_cursor_to_end, Id};
            let id_de = |k: usize| Id::new(crate::con_amigos_flujos::ID_GRUPO[k]);
            // El campo se muestra con el valor ya limpio (sin guiones ni
            // espacios): su cursor vuelve al final para no quedar fuera del
            // texto y desplazarlo a la izquierda.
            let cursor = move_cursor_to_end(id_de(i));
            if grupos.iter().all(|g| g.len() == 4) {
                // Completo: se valida solo.
                s.invitado.auto = true;
                let validar = despachar(app, v::Accion::Codigo(grupos.join("-")));
                return Task::batch([cursor, validar]);
            }
            // Un guion o un espacio tras un grupo completo pasa al siguiente.
            let da_paso = texto_g.contains(['-', ' ']) && grupos[i].len() == 4;
            let siguiente = grupos.iter().position(|g| g.len() < 4);
            match siguiente {
                Some(k) if k != i => Task::batch([cursor, focus(id_de(k)), move_cursor_to_end(id_de(k))]),
                Some(k) if da_paso && i < 2 && k == i => Task::batch([cursor, focus(id_de(i + 1))]),
                _ => cursor,
            }
        }
        Accion::UnirmeDescargar => {
            app.servicios.compartir.invitado.auto = true;
            despachar(app, v::Accion::ConfiarAnfitrion)
        }
        Accion::ConfirmarHuella(b) => despachar(app, v::Accion::ConfirmarHuella(b)),
    }
}

/// Deja el flujo abierto para mostrar otra cosa del maestro. Lo que no se
/// puede cortar a la mitad (publicar, guardar, instalar o aplicar cambios)
/// avisa y se queda; lo demás se cierra sin preguntar.
fn soltar_flujo(app: &mut App) -> bool {
    let s = &mut app.servicios.compartir;
    match s.etapa {
        Etapa::Cerrada => true,
        Etapa::Publicando | Etapa::Guardando | Etapa::InstalandoInvitado | Etapa::AplicandoVinculo => {
            app.avisar("Espera a que termine lo que está en curso.".into());
            false
        }
        _ => {
            s.red_ocupada = false;
            s.cerrar_al_terminar = false;
            s.cerrar();
            true
        }
    }
}

/// Qué muestra el panel de detalle.
enum Detalle<'a> {
    Inactivo,
    Compartir(usize),
    Unirme,
    Sincronizando(Option<&'a v::InstanciaQueRecibes>),
    Publicacion,
    Recibida(&'a v::InstanciaQueRecibes),
    Vacio,
}

fn paso_compartir(e: Etapa) -> Option<usize> {
    crate::con_amigos_flujos::paso(e)
}

fn es_unirme(e: Etapa) -> bool {
    matches!(
        e,
        Etapa::Entrada | Etapa::Identidad | Etapa::Destino | Etapa::RevisionInvitado | Etapa::InstalandoInvitado | Etapa::ResultadoInvitado
    )
}

fn es_sincronizar(e: Etapa) -> bool {
    matches!(e, Etapa::ConsultandoVinculo | Etapa::RevisionVinculo | Etapa::AplicandoVinculo | Etapa::ErrorVinculo)
}

/// Clave del panel: cambia al elegir otra cosa y el panel entra con fundido.
fn clave(d: &Detalle, id: Option<&str>) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    std::mem::discriminant(d).hash(&mut h);
    id.hash(&mut h);
    if let Detalle::Compartir(p) = d {
        p.hash(&mut h);
    }
    h.finish()
}

pub fn vista(app: &App) -> Element<'_, Mensaje> {
    let p = app.paleta();
    let s = &app.servicios.compartir;
    let a = &app.servicios.servidores.amigos;
    let modelo = modelo(app);
    let recibes = &modelo.recibes;
    let publicacion = s.panel.as_ref().map(|panel| panel.instancia.id.clone());
    let detalle = if !s.habilitado {
        Detalle::Inactivo
    } else if let Some(paso) = paso_compartir(s.etapa) {
        Detalle::Compartir(paso)
    } else if es_unirme(s.etapa) {
        Detalle::Unirme
    } else if es_sincronizar(s.etapa) {
        Detalle::Sincronizando(a.recibida.as_ref().and_then(|id| recibes.iter().find(|r| &r.id == id)))
    } else if matches!(s.etapa, Etapa::Panel | Etapa::Cierre) && s.panel.is_some() {
        Detalle::Publicacion
    } else if let Some(r) = a.recibida.as_ref().and_then(|id| recibes.iter().find(|r| &r.id == id)) {
        Detalle::Recibida(r)
    } else {
        Detalle::Vacio
    };
    let actual_compartida = matches!(detalle, Detalle::Publicacion).then_some(publicacion.clone()).flatten();
    let actual_recibida = match &detalle {
        Detalle::Recibida(r) => Some(r.id.clone()),
        Detalle::Sincronizando(Some(r)) => Some(r.id.clone()),
        _ => None,
    };

    // Columna izquierda (300 px).
    let mut maestro = column![column![
        sv::accion_grande(
            p,
            Icono::Subir,
            "Compartir una instancia",
            "Tus amigos reciben los mismos mods",
            matches!(detalle, Detalle::Compartir(_)),
            m(Accion::Compartir),
        ),
        // El código siempre a la vista: no se confunde con agregar un servidor.
        crate::con_amigos_flujos::bloque_codigo(app, matches!(detalle, Detalle::Unirme)),
    ]
    .spacing(8)]
    .spacing(10);
    let grupo = |nombre: &str, n: usize| -> Element<'_, Mensaje> {
        column![
            container(row![sv::rotulo(p, nombre), Space::with_width(Length::Fill), sv::mono_sm(n.to_string(), p.text_muted)])
                .padding(Padding { top: 18.0, right: 4.0, bottom: 4.0, left: 4.0 }),
            sv::regla(borde::FINO, p.border),
        ]
        .into()
    };
    maestro = maestro.push(grupo("Compartes", modelo.compartes.len()));
    let mut items = column![].spacing(2);
    if modelo.compartes.is_empty() {
        items = items.push(container(sv::cuerpo_sm("No compartes ninguna", p.text_muted)).padding(Padding::from([10.0, 12.0])));
    }
    for c in &modelo.compartes {
        let marca = match c.estado {
            v::EstadoCompartida::Disponible => p.success,
            v::EstadoCompartida::CambiosSinPublicar => p.info,
            v::EstadoCompartida::EnPausa => p.warning,
        };
        let linea = format!(
            "Revisión {} · {} {}",
            c.revision,
            c.personas,
            if c.personas == 1 { "persona" } else { "personas" }
        );
        items = items.push(sv::item_lista(
            p,
            &c.nombre,
            icaro_core::icono_instancia::patron(&c.nombre, 0),
            &linea,
            Some(marca),
            actual_compartida.as_deref() == Some(c.id.as_str()),
            m(Accion::Compartida(c.id.clone())),
        ));
    }
    maestro = maestro.push(items);
    maestro = maestro.push(grupo("Recibes", recibes.len()));
    let mut items = column![].spacing(2);
    if recibes.is_empty() {
        items = items.push(container(sv::cuerpo_sm("No recibes ninguna", p.text_muted)).padding(Padding::from([10.0, 12.0])));
    }
    for r in recibes {
        let nueva = r.cambios.is_some_and(|n| n > 0);
        // Con el servidor del anfitrión abierto, la fila lo dice en verde.
        let abierto = app.servicios.servidores.alojar.remotos.get(&r.id) == Some(&crate::alojamiento_real::Remoto::Abierto);
        items = items.push(sv::item_lista(
            p,
            &r.nombre,
            icaro_core::icono_instancia::patron(&r.nombre, 0),
            &if abierto { format!("Servidor abierto · de {}", r.anfitrion) } else { format!("de {}", r.anfitrion) },
            if abierto { Some(p.success) } else { nueva.then_some(p.info) },
            actual_recibida.as_deref() == Some(r.id.as_str()),
            m(Accion::Recibida(r.id.clone())),
        ));
    }
    maestro = maestro.push(items);

    let id_clave = actual_compartida.clone().or(actual_recibida.clone());
    let k = clave(&detalle, id_clave.as_deref());
    let interior: Element<'_, Mensaje> = match detalle {
        Detalle::Inactivo => vacio(
            app,
            "Juega con amigos sin servidor",
            "Compartir instancias está en fase experimental. Actívalo para compartir una instancia o unirte con un código.",
            Some(componentes::boton(p, "Activar Compartir experimental", Variante::Primario, Some(m(Accion::Activar)))),
        ),
        Detalle::Vacio => icaro_ui::scroll::desplazable(portada(app, &modelo)),
        Detalle::Compartir(paso) => {
            let cab = sv::cabecera_detalle(
                p,
                sv::marca_grande(p, Icono::Subir),
                "Compartir una instancia",
                "Tus amigos tendrán una copia con los mismos mods y podrán entrar contigo.",
                Some(componentes::boton_compacto(p, "Cancelar", Variante::Fantasma, Some(m(Accion::CerrarFlujo)))),
            );
            column![
                cab,
                sv::pasos(p, paso, (paso as f32 - 1.0) / 2.0),
                container(crate::con_amigos_flujos::compartir(app)).height(Length::Fill),
            ]
            .spacing(22)
            .height(Length::Fill)
            .into()
        }
        Detalle::Unirme => {
            let cab = sv::cabecera_detalle(
                p,
                sv::marca_grande(p, Icono::Llave),
                "Unirme con un código",
                "Pega el código que te envió tu amigo.",
                Some(componentes::boton_compacto(p, "Cancelar", Variante::Fantasma, Some(m(Accion::CerrarFlujo)))),
            );
            let mut c = column![cab].spacing(22).height(Length::Fill);
            if let Some(u) = crate::con_amigos_flujos::unirme(app) {
                c = c.push(container(u).height(Length::Fill));
            } else if let Some(f) = cr::fase_pura(app) {
                c = c.push(container(f.map(vista_compartir)).height(Length::Fill));
            }
            c.into()
        }
        Detalle::Sincronizando(r) => {
            let mut c = column![].spacing(22).height(Length::Fill);
            if let Some(r) = r {
                c = c.push(cabecera_recibida(app, r));
            }
            if let Some(f) = cr::fase_pura(app) {
                c = c.push(container(f.map(vista_compartir)).height(Length::Fill));
            }
            c.into()
        }
        Detalle::Publicacion if s.etapa == Etapa::Cierre => publicacion_detalle(app, &modelo),
        Detalle::Publicacion
            if app.servicios.servidores.alojar.editor.as_ref().map(|e| &e.instancia) == publicacion.as_ref() =>
        {
            let e = app.servicios.servidores.alojar.editor.as_ref().expect("editor abierto");
            let nombre = s.panel.as_ref().map_or("", |p| p.instancia.nombre.as_str());
            icaro_ui::scroll::desplazable(crate::alojamiento_vista::ajustes(app, e, nombre))
        }
        Detalle::Publicacion => icaro_ui::scroll::desplazable(publicacion_detalle(app, &modelo)),
        Detalle::Recibida(r) => icaro_ui::scroll::desplazable(recibida_detalle(app, r)),
    };
    // En ventanas anchas el detalle no se estira: se centra con un ancho de lectura.
    let interior = container(container(interior).max_width(ANCHO_DETALLE).width(Length::Fill).height(Length::Fill))
        .center_x(Length::Fill)
        .height(Length::Fill);
    let panel = container(icaro_ui::transicion::subir(p.surface, k, 6.0, 260, interior))
        .padding(Padding { top: 28.0, right: 32.0, bottom: 32.0, left: 32.0 })
        .width(Length::Fill)
        .height(Length::Fill)
        .style(sv::caja(Some(p.surface), p.border_strong, borde::FINO));
    row![container(icaro_ui::scroll::desplazable(maestro)).width(300).height(Length::Fill), panel]
        .spacing(24)
        .height(Length::Fill)
        .align_y(Alignment::Start)
        .into()
}

/// Ancho máximo del detalle; lo demás queda como margen a los lados.
const ANCHO_DETALLE: f32 = 820.0;

/// Portada de Con amigos: grabado, cómo funciona en tres pasos, las dos
/// acciones y, si ya compartes o recibes algo, su resumen.
fn portada<'a>(app: &'a App, modelo: &v::InstanciasCompartidas) -> Element<'a, Mensaje> {
    let p = app.paleta();
    let aloja = &app.servicios.servidores.alojar;
    let lamina = container(icaro_ui::laminas::grabado(p, icaro_ui::laminas::Lamina::Alas, Length::Fill, 190))
        .padding(borde::MEDIO)
        .width(Length::Fill)
        .style(icaro_ui::estilo::marco(p));
    let paso = |n: usize, glifo: Icono, titulo_p: &str, linea: &str| -> Element<'a, Mensaje> {
        container(
            column![
                row![
                    sv::mono_sm(format!("0{n}"), p.text_muted),
                    Space::with_width(Length::Fill),
                    icaro_ui::iconos::icono(glifo, icaro_ui::iconos::Tam::Base, p.text),
                ]
                .align_y(Alignment::Center),
                sv::subtitulo(titulo_p.to_owned(), p.text),
                sv::cuerpo_sm(linea.to_owned(), p.text_muted),
            ]
            .spacing(8),
        )
        .padding(16)
        .width(Length::FillPortion(1))
        .height(132)
        .style(sv::caja(Some(p.surface_sunken), p.border, borde::FINO))
        .into()
    };
    let mut c = column![
        lamina,
        column![
            sv::titulo("Juega con amigos sin servidor", p.text),
            sv::cuerpo_sm(
                "Comparte una instancia y tus amigos la reciben con los mismos mods. Si quieres, abre un servidor en tu equipo y entran sin abrir puertos.",
                p.text_muted,
            ),
        ]
        .spacing(8),
        row![
            paso(1, Icono::Subir, "Comparte", "Eliges la instancia y qué mods y ajustes van."),
            paso(2, Icono::Copiar, "Envía el código", "12 letras que caducan cuando tú quieras."),
            paso(3, Icono::Jugar, "Jueguen juntos", "Abre un servidor o únanse donde quieran."),
        ]
        .spacing(12),
        row![
            sv::accion_grande(p, Icono::Subir, "Compartir una instancia", "Tus amigos reciben los mismos mods", false, m(Accion::Compartir)),
            sv::accion_grande(p, Icono::Llave, "Tengo un código", "Escríbelo en la columna de la izquierda", false, m(Accion::EnfocarCodigo)),
        ]
        .spacing(12),
    ]
    .spacing(22);
    // Primero, los servidores abiertos de tus amigos, con Entrar.
    if let Some(amigos) = crate::alojamiento_vista::servidores_amigos(app, &modelo.recibes) {
        c = c.push(amigos);
    }
    // Resumen: lo que compartes y lo que recibes, con su estado y un acceso.
    let fila = |glifo_n: &str, linea: String, estado_w: Element<'a, Mensaje>, al: Mensaje| -> Element<'a, Mensaje> {
        container(
            row![
                pixel(p, glifo_n, 32.0),
                column![negrita_sm(glifo_n.to_owned(), p.text), sv::mono_sm(linea, p.text_muted)].spacing(2).width(Length::Fill),
                estado_w,
                componentes::boton_compacto(p, "Ver", Variante::Secundario, Some(al)),
            ]
            .spacing(14)
            .align_y(Alignment::Center),
        )
        .padding(Padding::from([10.0, 14.0]))
        .width(Length::Fill)
        .style(sv::caja(Some(p.surface), p.border, borde::FINO))
        .into()
    };
    if !modelo.compartes.is_empty() {
        let mut lista = column![sv::subtitulo("Lo que compartes", p.text)].spacing(8);
        for cmp in &modelo.compartes {
            let servidor = aloja.de(&cmp.id).filter(|f| **f == crate::alojamiento_real::Fase::Abierto).map(|_| {
                if aloja.jugadores.is_empty() { " · servidor abierto".to_owned() } else { format!(" · servidor abierto · {} jugando", aloja.jugadores.len()) }
            });
            let linea = format!(
                "Revisión {} · {} {}{}",
                cmp.revision,
                cmp.personas,
                if cmp.personas == 1 { "persona" } else { "personas" },
                servidor.unwrap_or_default()
            );
            let estado_w = match cmp.estado {
                v::EstadoCompartida::Disponible => componentes::insignia(p, componentes::Estado::Exito, "Disponible"),
                v::EstadoCompartida::CambiosSinPublicar => componentes::insignia(p, componentes::Estado::Info, "Cambios"),
                v::EstadoCompartida::EnPausa => componentes::insignia(p, componentes::Estado::Aviso, "En pausa"),
            };
            lista = lista.push(fila(&cmp.nombre, linea, estado_w, m(Accion::Compartida(cmp.id.clone()))));
        }
        c = c.push(lista);
    }
    if !modelo.recibes.is_empty() {
        let mut lista = column![sv::subtitulo("Lo que recibes", p.text)].spacing(8);
        for r in &modelo.recibes {
            let estado_w = if r.cambios.is_some_and(|n| n > 0) {
                componentes::insignia(p, componentes::Estado::Info, "Cambios nuevos")
            } else {
                componentes::insignia_contorno(p, &format!("de {}", r.anfitrion))
            };
            lista = lista.push(fila(&r.nombre, r.version.clone(), estado_w, m(Accion::Recibida(r.id.clone()))));
        }
        c = c.push(lista);
    }
    c.into()
}

fn negrita_sm<'a>(s: String, c: iced::Color) -> Element<'a, Mensaje> {
    iced::widget::text(s).font(icaro_ui::fuentes::CUERPO_NEGRITA).size(icaro_ui::tema::texto::BODY_SM.0).color(c).into()
}

fn vacio<'a>(app: &'a App, titulo: &str, linea: &str, accion: Option<Element<'a, Mensaje>>) -> Element<'a, Mensaje> {
    let p = app.paleta();
    let mut c = column![sv::titulo(titulo.to_owned(), p.text), sv::cuerpo_sm(linea.to_owned(), p.text_muted)].spacing(8);
    if let Some(a) = accion {
        c = c.push(Space::with_height(8)).push(a);
    }
    container(c).padding(Padding::from([48.0, 32.0])).width(Length::Fill).into()
}

fn bloque<'a>(contenido: impl Into<Element<'a, Mensaje>>) -> Element<'a, Mensaje> {
    container(contenido).width(Length::Fill).into()
}

fn copiado(app: &App, que: &str) -> bool {
    app.servicios
        .servidores
        .amigos
        .copiado
        .as_ref()
        .is_some_and(|c| c.0 == que && app.ahora.saturating_duration_since(c.1).as_millis() < crate::servidores_reales::MS_COPIADO as u128)
}

/// Lo que compartes: código, caducidad, cupo, personas, pausa y dejar.
fn publicacion_detalle<'a>(app: &'a App, modelo: &v::InstanciasCompartidas) -> Element<'a, Mensaje> {
    let p = app.paleta();
    let s = &app.servicios.compartir;
    let a = &app.servicios.servidores.amigos;
    let Some(panel) = s.panel.as_ref() else {
        return Space::new(0, 0).into();
    };
    let id = panel.instancia.id.clone();
    let entrada = modelo.compartes.iter().find(|c| c.id == id);
    let estado = entrada.map_or(v::EstadoCompartida::Disponible, |c| c.estado);
    let en_pausa = estado == v::EstadoCompartida::EnPausa || (s.anfitrion.is_none() && !app.servicios.demo);
    let version = format!("{} · {}", panel.instancia.minecraft, panel.instancia.loader);
    let (insignia, palabra) = match (estado, en_pausa) {
        (_, true) => (componentes::Estado::Aviso, "En pausa"),
        (v::EstadoCompartida::CambiosSinPublicar, _) => (componentes::Estado::Info, "Cambios sin publicar"),
        _ => (componentes::Estado::Exito, "Disponible"),
    };
    let mut c = column![sv::cabecera_detalle(
        p,
        pixel(p, &panel.instancia.nombre, 56.0),
        &panel.instancia.nombre,
        &format!("{version} · revisión {}", panel.revision),
        Some(componentes::insignia(p, insignia, palabra)),
    )]
    .spacing(22);
    if matches!(s.etapa, Etapa::Cierre) {
        if let Some(f) = cr::fase_pura(app) {
            return c.push(f.map(vista_compartir)).into();
        }
    }
    // Recuperando, reconectando o reanudando la publicación: también tras una
    // pausa, para que Reanudar muestre que está trabajando.
    if s.red_ocupada && s.anfitrion.is_none() {
        let texto_r = if estado == v::EstadoCompartida::EnPausa { "Reanudando la publicación…" } else { "Abriendo la publicación…" };
        return c
            .push(row![icaro_ui::progreso::spinner(p, app.reducir), sv::cuerpo_sm(texto_r, p.text_muted)].spacing(10))
            .into();
    }
    // Un fallo al reconectar o reanudar se muestra con su causa.
    if let v::EstadoPublicacion::Error(e) = &panel.estado {
        c = c.push(sv::nota(
            p,
            Tono::Error,
            Icono::Alerta,
            sv::cuerpo_sm(format!("No se pudo reanudar: {e}"), p.text),
            Some(componentes::boton_compacto(p, "Reintentar", Variante::Secundario, Some(m(Accion::Reanudar(id.clone()))))),
        ));
    }
    if estado == v::EstadoCompartida::CambiosSinPublicar {
        c = c.push(sv::nota(
            p,
            Tono::Info,
            Icono::InfoCaja,
            sv::cuerpo_sm(
                format!("Cambiaste cosas en la instancia. Tus amigos siguen con la revisión {} hasta que publiques.", panel.revision),
                p.text,
            ),
            Some(componentes::boton_compacto(p, "Revisar y publicar", Variante::Primario, Some(m(Accion::Publicar(id.clone()))))),
        ));
    }
    if en_pausa {
        c = c.push(sv::nota(
            p,
            Tono::Alerta,
            Icono::Pausa,
            sv::cuerpo_sm("En pausa: nadie puede unirse ni descargar. Quienes ya están conservan lo que tienen.", p.text),
            None,
        ));
    }
    // Código.
    let invitacion = panel.invitacion.as_ref();
    let caduca = match invitacion.map(|i| &i.estado) {
        _ if en_pausa => "En pausa".to_owned(),
        Some(v::EstadoInvitacion::Vigente { restante }) => format!("Caduca en {restante}"),
        Some(v::EstadoInvitacion::Generando) => "Generando…".to_owned(),
        Some(v::EstadoInvitacion::Completa) => "Cupo completo".to_owned(),
        Some(v::EstadoInvitacion::Vencida { fecha }) => format!("Venció {fecha}"),
        Some(v::EstadoInvitacion::Revocada) => "Anulado".to_owned(),
        None => "Sin código".to_owned(),
    };
    let codigo_txt = invitacion.map(|i| i.codigo.clone()).filter(|c| !c.is_empty()).unwrap_or_else(|| "----------------".into());
    let vigente = matches!(invitacion.map(|i| &i.estado), Some(v::EstadoInvitacion::Vigente { .. })) && !en_pausa;
    let zona_codigo: Element<'a, Mensaje> = if a.confirmar == Some(Confirmacion::CambiarCodigo) {
        sv::confirmar(
            p,
            "¿Cambiar el código?",
            &format!("{codigo_txt} deja de servir ahora. Quienes ya se unieron no pierden su acceso."),
            "Cambiar código",
            m(Accion::Confirmar),
            m(Accion::Cancelar),
        )
    } else {
        row![
            componentes::boton_con_icono(
                p,
                if copiado(app, "codigo") { Icono::Check } else { Icono::Copiar },
                if copiado(app, "codigo") { "Copiado" } else { "Copiar código" },
                Variante::Primario,
                vigente.then(|| m(Accion::CopiarCodigo)),
                true,
            ),
            componentes::boton_con_icono(
                p,
                if copiado(app, "mensaje") { Icono::Check } else { Icono::Documento },
                if copiado(app, "mensaje") { "Copiado" } else { "Copiar mensaje" },
                Variante::Secundario,
                vigente.then(|| m(Accion::CopiarMensaje)),
                true,
            ),
            componentes::boton_con_icono(
                p,
                Icono::Refrescar,
                "Cambiar código",
                Variante::Fantasma,
                (!en_pausa && !s.red_ocupada).then(|| m(Accion::Pedir(Confirmacion::CambiarCodigo))),
                true,
            ),
        ]
        .spacing(8)
        .into()
    };
    let celdas: Element<'a, Mensaje> = v::celdas(p, &codigo_txt, !vigente);
    c = c.push(
        container(
            column![
                row![sv::rotulo(p, "Código para tus amigos"), Space::with_width(Length::Fill), sv::mono_sm(caduca, p.text_muted)],
                celdas,
                zona_codigo,
            ]
            .spacing(12),
        )
        .padding(20)
        .width(Length::Fill)
        .style(sv::caja(Some(p.surface_sunken), p.border, borde::FINO)),
    );
    // Caducidad y límite de personas.
    let editable = invitacion.is_some_and(|i| i.ajustes_editables) && !en_pausa && !s.red_ocupada;
    let caducidad = invitacion.map_or(s.caducidad, |i| i.caducidad);
    let cupo = invitacion.map_or(s.cupo, |i| i.cupo);
    let personas = &panel.accesos.personas;
    let activa = v::Caducidad::TODAS.iter().position(|c| *c == caducidad).unwrap_or(0);
    let seg = componentes::segmentado_compacto(p, &["24 horas", "2 días", "1 semana"], activa, |i| {
        if editable {
            m(Accion::Caducidad(v::Caducidad::TODAS[i]))
        } else {
            Mensaje::Nada
        }
    });
    let minimo = personas.len().max(1) as u32;
    let contador = componentes::contador(
        p,
        cupo as i32,
        if editable && cupo > minimo { m(Accion::Cupo(cupo - 1)) } else { Mensaje::Nada },
        if editable && cupo < 50 { m(Accion::Cupo(cupo + 1)) } else { Mensaje::Nada },
    );
    c = c.push(
        row![
            bloque(column![sv::rotulo(p, "Caduca en"), seg].spacing(12)),
            bloque(
                column![
                    sv::rotulo(p, "Pueden unirse"),
                    row![contador, sv::cuerpo_sm("personas", p.text_muted)].spacing(16).align_y(Alignment::Center),
                ]
                .spacing(12)
            ),
        ]
        .spacing(24),
    );
    // Personas.
    let mut lista = column![];
    for (i, persona) in personas.iter().enumerate() {
        if i > 0 {
            lista = lista.push(sv::regla(borde::FINO, p.border));
        }
        let fila: Element<'a, Mensaje> = if a.confirmar.as_ref() == Some(&Confirmacion::Persona(persona.id.clone())) {
            container(sv::confirmar(
                p,
                &format!("¿Quitar a {}?", persona.nombre),
                "Su copia queda como está, pero deja de recibir cambios.",
                "Quitar",
                m(Accion::Confirmar),
                m(Accion::Cancelar),
            ))
            .padding(Padding::from([8.0, 0.0]))
            .into()
        } else {
            sv::persona(
                p,
                &persona.nombre,
                icaro_core::icono_instancia::patron(&persona.huella, 0),
                Some(persona.huella.as_str()),
                persona.conectado,
                if persona.conectado { "en línea" } else { &persona.estado },
                Some(componentes::boton_compacto(
                    p,
                    "Quitar",
                    Variante::Fantasma,
                    (!s.red_ocupada).then(|| m(Accion::Pedir(Confirmacion::Persona(persona.id.clone())))),
                )),
            )
        };
        lista = lista.push(fila);
    }
    if personas.is_empty() {
        lista = lista.push(container(sv::cuerpo_sm("Todavía no se une nadie. Envía el código a tus amigos.", p.text_muted)).padding(Padding::from([8.0, 0.0])));
    }
    c = c.push(
        column![
            row![
                sv::subtitulo("Personas", p.text),
                Space::with_width(Length::Fill),
                sv::mono_sm(format!("{} de {}", personas.len(), cupo), p.text),
            ],
            sv::cupo(p, personas.len(), cupo as usize),
            lista,
        ]
        .spacing(12),
    );
    // Servidor alojado en este equipo.
    c = c.push(crate::alojamiento_vista::seccion_anfitrion(app, &id, en_pausa));
    // Pausar o Reanudar y Dejar de compartir.
    let zona: Element<'a, Mensaje> = if a.confirmar == Some(Confirmacion::Dejar) {
        sv::confirmar(
            p,
            &format!("¿Dejar de compartir {}?", panel.instancia.nombre),
            "El código deja de servir y nadie recibe más cambios. Tus amigos conservan su copia.",
            "Dejar de compartir",
            m(Accion::Confirmar),
            m(Accion::Cancelar),
        )
    } else if a.confirmar.as_ref() == Some(&Confirmacion::Quitar(id.clone())) {
        sv::confirmar(
            p,
            &format!("¿Quitar {} de lo que compartes?", panel.instancia.nombre),
            "Tus amigos dejan de recibir cambios y no pueden volver a entrar. Su copia y la tuya no se tocan.",
            "Quitar",
            m(Accion::Confirmar),
            m(Accion::Cancelar),
        )
    } else {
        let mut fila = row![
            if en_pausa {
                componentes::boton_con_icono(p, Icono::Jugar, "Reanudar", Variante::Secundario, (!s.red_ocupada).then(|| m(Accion::Reanudar(id.clone()))), true)
            } else {
                componentes::boton_con_icono(p, Icono::Pausa, "Pausar", Variante::Secundario, (!s.red_ocupada).then(|| m(Accion::Pausar(id.clone()))), true)
            },
        ]
        .spacing(8);
        // Activa se deja de compartir; en pausa ya no hay nodo: se puede quitar.
        fila = fila.push(if en_pausa {
            componentes::boton_con_icono(
                p,
                Icono::Papelera,
                "Quitar de compartidas",
                Variante::Peligro,
                (!s.red_ocupada).then(|| m(Accion::Pedir(Confirmacion::Quitar(id.clone())))),
                true,
            )
        } else {
            componentes::boton_compacto(p, "Dejar de compartir", Variante::Peligro, (!s.red_ocupada).then(|| m(Accion::Pedir(Confirmacion::Dejar))))
        });
        fila.into()
    };
    c.push(sv::pie(p, zona)).into()
}

fn cabecera_recibida<'a>(app: &'a App, r: &v::InstanciaQueRecibes) -> Element<'a, Mensaje> {
    let p = app.paleta();
    let nueva = r.cambios.is_some_and(|n| n > 0);
    sv::cabecera_detalle(
        p,
        pixel(p, &r.nombre, 56.0),
        &r.nombre,
        &format!("de {} · {}", r.anfitrion, r.version),
        Some(if nueva {
            componentes::insignia(p, componentes::Estado::Info, "Cambios nuevos")
        } else if r.cambios == Some(0) {
            componentes::insignia(p, componentes::Estado::Exito, "Al día")
        } else {
            componentes::insignia_contorno(p, "Comprobando")
        }),
    )
}

/// Lo que recibes: anfitrión, tu copia, qué cambia y Sincronizar y jugar.
fn recibida_detalle<'a>(app: &'a App, r: &v::InstanciaQueRecibes) -> Element<'a, Mensaje> {
    let p = app.paleta();
    let a = &app.servicios.servidores.amigos;
    let nueva = r.cambios.is_some_and(|n| n > 0);
    let (en_linea, anfitrion) = match r.estado {
        v::AnfitrionRecibida::Disponible => (true, format!("{} está compartiendo", r.anfitrion)),
        v::AnfitrionRecibida::SinConexion => (false, format!("{} no está conectado ahora", r.anfitrion)),
        v::AnfitrionRecibida::Comprobando => (false, "Comprobando…".to_owned()),
    };
    let copia = match (r.revision, r.cambios) {
        (Some(rev), Some(n)) if n > 0 => format!("Revisión {rev} · hay una nueva"),
        (Some(rev), Some(_)) => format!("Revisión {rev} · al día"),
        (Some(rev), None) => format!("Revisión {rev}"),
        (None, _) => "Sin revisión aplicada".to_owned(),
    };
    let mut c = column![
        cabecera_recibida(app, r),
        // Lo primero: el servidor del amigo y cómo entrar.
        crate::alojamiento_vista::seccion_invitado(app, r),
        row![
            bloque(
                column![
                    sv::rotulo(p, "Anfitrión"),
                    row![sv::marca(if en_linea { p.success } else { p.text_disabled }), sv::cuerpo_sm(anfitrion, p.text)]
                        .spacing(8)
                        .align_y(Alignment::Center),
                ]
                .spacing(12)
            ),
            bloque(column![sv::rotulo(p, "Tu copia"), sv::cuerpo_sm(copia, p.text)].spacing(12)),
        ]
        .spacing(24),
    ]
    .spacing(22);
    if nueva {
        let n = r.cambios.unwrap_or(0);
        c = c.push(
            column![
                sv::subtitulo("Qué cambia en la revisión nueva", p.text),
                sv::nota(
                    p,
                    Tono::Info,
                    Icono::InfoCaja,
                    sv::cuerpo_sm(
                        format!(
                            "{n} {} por revisar. Al sincronizar ves cada uno antes de aplicarlo.",
                            if n == 1 { "cambio" } else { "cambios" }
                        ),
                        p.text,
                    ),
                    None,
                ),
                sv::cuerpo_sm("Tus mundos y ajustes no se tocan.", p.text_muted),
            ]
            .spacing(12),
        );
    }
    let zona: Element<'a, Mensaje> = if a.confirmar.as_ref() == Some(&Confirmacion::Salir(r.id.clone())) {
        sv::confirmar(
            p,
            &format!("¿Salir de {}?", r.nombre),
            &format!("Dejas de recibir cambios de {}. La instancia queda en tu equipo como una normal.", r.anfitrion),
            "Salir",
            m(Accion::Confirmar),
            m(Accion::Cancelar),
        )
    } else {
        row![
            if nueva {
                componentes::boton_con_icono(p, Icono::Refrescar, "Sincronizar y jugar", Variante::Primario, Some(m(Accion::Sincronizar(r.id.clone()))), false)
            } else {
                componentes::boton_con_icono(p, Icono::Jugar, "Jugar", Variante::Primario, Some(m(Accion::Jugar(r.id.clone()))), false)
            },
            componentes::boton(p, "Salir de esta instancia", Variante::Peligro, (!a.saliendo).then(|| m(Accion::Pedir(Confirmacion::Salir(r.id.clone()))))),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
    };
    c.push(sv::pie(p, zona)).into()
}

/// Escena de diseño: listas y publicación de muestra para Con amigos.
/// `servidores-amigos` abre la publicación «Amigos»; `-recibida`, una recibida
/// con cambios nuevos; `-vacio`, sin nada; `-compartir`, el primer paso.
pub fn preparar_diseno(app: &mut App, scene: &str) {
    app.servicios.compartir.habilitado = true;
    let compartida = |id: &str, nombre: &str, estado, revision, personas| v::InstanciaQueCompartes {
        id: id.into(),
        nombre: nombre.into(),
        estado,
        revision,
        personas,
        version: "1.21.1 · Fabric 0.16.9".into(),
        codigo: None,
        caduca: String::new(),
        copiado: false,
    };
    let recibida = |id: &str, nombre: &str, de: &str, revision, cambios| v::InstanciaQueRecibes {
        id: id.into(),
        nombre: nombre.into(),
        anfitrion: de.into(),
        estado: v::AnfitrionRecibida::Disponible,
        revision: Some(revision),
        cambios: Some(cambios),
        version: "1.20.1 · Forge 47.3.0".into(),
    };
    let vacio = scene.ends_with("-vacio");
    app.servicios.servidores.amigos.muestra = Some(v::InstanciasCompartidas {
        compartes: if vacio {
            vec![]
        } else {
            vec![
                compartida("amigos", "Amigos", v::EstadoCompartida::Disponible, 4, 3),
                compartida("boys", "Boys SMP", v::EstadoCompartida::CambiosSinPublicar, 12, 5),
            ]
        },
        recibes: if vacio {
            vec![]
        } else {
            vec![
                recibida("survival", "Survival 26", "Forjador", 9, 0),
                recibida("create", "Create Moderno", "Pato_Lucas", 12, 3),
            ]
        },
        ..Default::default()
    });
    // `-inicio`: la portada con el resumen, sin nada elegido.
    if scene.contains("-inicio") {
        return;
    }
    if scene.contains("-unirme-escrito") {
        app.servicios.compartir.etapa = Etapa::Entrada;
        app.servicios.servidores.amigos.grupos = ["7QKM".into(), "4X".into(), String::new()];
        return;
    }
    if scene.ends_with("-recibida") || scene.ends_with("-recibida-servidor") {
        app.servicios.servidores.amigos.recibida = Some("create".into());
        return;
    }
    if scene.ends_with("-compartir") {
        app.servicios.compartir.etapa = Etapa::Selector;
        return;
    }
    if scene.ends_with("-compartir-2") {
        let fila = |n: usize, tipo: v::TipoContenido, marcado: bool| v::CambioPublicacion {
            archivo: v::ArchivoCompartido { id: format!("a{n}"), nombre: format!("Archivo {n}"), ..Default::default() },
            tipo,
            grupo: v::GrupoPublicacion::Nuevo,
            marca: v::Marca::Alta,
            eleccion: if marcado { v::Eleccion::Compartir } else { v::Eleccion::Privado },
            si: v::Eleccion::Compartir,
            no: v::Eleccion::Privado,
            version: String::new(),
            version_publicada: String::new(),
            de_mod: None,
            propio: false,
            autorizado: true,
            solo_cliente: false,
            integrado: false,
            problema: None,
            estado: String::new(),
        };
        let mut cambios = vec![];
        for (tipo, total, marcados) in [
            (v::TipoContenido::Mod, 46, 21),
            (v::TipoContenido::Config, 20, 9),
            (v::TipoContenido::Resourcepack, 3, 0),
            (v::TipoContenido::Shader, 1, 0),
            (v::TipoContenido::Script, 3, 2),
        ] {
            for n in 0..total {
                cambios.push(fila(cambios.len(), tipo, n < marcados));
            }
        }
        let s = &mut app.servicios.compartir;
        s.etapa = Etapa::Revision;
        s.seleccionada = Some("qa-supervivencia".into());
        let mut vista = v::VistaRevision::default();
        vista.atajo = Some(v::AtajoPublicacion::Necesario);
        s.revision = Some(v::RevisionPublicacion {
            instancia: v::InstanciaCompartida { id: "qa-supervivencia".into(), nombre: "Supervivencia".into(), ..Default::default() },
            base: 0,
            nueva: 1,
            cambios,
            estado: v::EstadoRevisionPublicacion::Disponible,
            confirmar_cancelacion: false,
            guardando: false,
            vista,
        });
        return;
    }
    if scene.ends_with("-unirme") || scene.ends_with("-unirme-error") {
        app.servicios.compartir.etapa = Etapa::Entrada;
        if scene.ends_with("-error") {
            app.servicios.servidores.amigos.grupos = ["AAAA".into(), "AAAA".into(), "AAAA".into()];
            app.servicios.compartir.invitado.entrada = v::EstadoEntrada::Invalido;
        }
        return;
    }
    if vacio {
        return;
    }
    let persona = |nombre: &str, conectado: bool, estado: &str| v::Acceso {
        id: nombre.to_lowercase(),
        nombre: nombre.into(),
        huella: nombre.into(),
        estado: estado.into(),
        conectado,
        aceptado: String::new(),
    };
    let s = &mut app.servicios.compartir;
    s.etapa = Etapa::Panel;
    s.panel = Some(v::PanelPublicacion {
        instancia: v::InstanciaCompartida {
            id: "amigos".into(),
            nombre: "Amigos".into(),
            minecraft: "1.21.1".into(),
            loader: "Fabric 0.16.9".into(),
            mods: 38,
            en_uso: false,
        },
        revision: 4,
        fecha_revision: String::new(),
        hash_revision: String::new(),
        disponibilidad: String::new(),
        transferencias: String::new(),
        huella: String::new(),
        contenido_privado: 0,
        estado: v::EstadoPublicacion::Compartiendo,
        invitacion: Some(v::Invitacion {
            codigo: "7QKM-4XPT-R2C9".into(),
            estado: v::EstadoInvitacion::Vigente { restante: "23 h".into() },
            caducidad: v::Caducidad::Horas24,
            cupo: 10,
            aceptados: 3,
            copiado: false,
            ajustes_editables: true,
        }),
        accesos: v::Accesos {
            personas: vec![
                persona("Forjador", true, "en línea"),
                persona("Pato_Lucas", true, "en línea"),
                persona("Sombra", false, "ayer, 22:10"),
            ],
            confirmar: None,
            instancia: "amigos".into(),
        },
    });
    if scene.ends_with("-compartir-3") {
        app.servicios.compartir.etapa = Etapa::Listo;
    }
    if scene.ends_with("-confirmar") {
        app.servicios.servidores.amigos.confirmar = Some(Confirmacion::Dejar);
    }
    // `-pausa`: la publicación en pausa, con Reanudar y Quitar de compartidas;
    // `-pausa-quitar`: con la confirmación de quitar abierta.
    if scene.contains("-pausa") {
        if let Some(m) = &mut app.servicios.servidores.amigos.muestra {
            if let Some(c) = m.compartes.iter_mut().find(|c| c.id == "amigos") {
                c.estado = v::EstadoCompartida::EnPausa;
            }
        }
        if scene.ends_with("-quitar") {
            app.servicios.servidores.amigos.confirmar = Some(Confirmacion::Quitar("amigos".into()));
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn app() -> App {
        let (mut a, _) = crate::iniciar_con_guardado(
            icaro_ui::shell::Seccion::Servidores,
            true,
            None,
            false,
            crate::servicios::Estado::nuevo(true),
            None,
        );
        a.servicios.demo = false;
        a.servicios.compartir.habilitado = true;
        a
    }

    #[test]
    fn escribir_el_codigo_a_la_izquierda_abre_unirme_y_cierra_lo_demas() {
        let mut a = app();
        a.servicios.servidores.amigos.recibida = Some("x".into());
        let _ = actualizar(&mut a, Accion::Grupo(0, "7qk".into()));
        assert_eq!(a.servicios.compartir.etapa, Etapa::Entrada);
        assert_eq!(a.servicios.servidores.amigos.grupos[0], "7QK");
        assert!(a.servicios.servidores.amigos.recibida.is_none());
        // Seguir escribiendo no reinicia lo escrito.
        let _ = actualizar(&mut a, Accion::Grupo(0, "7QKM".into()));
        assert_eq!(a.servicios.servidores.amigos.grupos[0], "7QKM");
    }

    #[test]
    fn lo_pegado_llena_los_tres_grupos_aunque_hubiera_algo_escrito() {
        let mut a = app();
        a.servicios.servidores.amigos.grupos = ["ZZZZ".into(), "Z".into(), String::new()];
        let _ = actualizar(&mut a, Accion::Pegado("7qkm-4xpt-r2c9".into()));
        assert_eq!(
            a.servicios.servidores.amigos.grupos,
            ["7QKM".to_owned(), "4XPT".into(), "R2C9".into()]
        );
        assert_eq!(a.servicios.compartir.etapa, Etapa::Entrada);
    }
}
