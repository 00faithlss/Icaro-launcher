// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Icono nativo de Ícaro en la bandeja del sistema.
//!
//! Módulo autocontenido: no conoce `App` ni `Mensaje`. La aplicación llama a
//! [`Bandeja::iniciar`] dentro de `spawn_blocking` (espera, con un límite, a que
//! el hilo nativo confirme el registro de la clase, la ventana y el icono) y
//! consulta [`Bandeja::siguiente`] sin bloquear. La bandeja nunca cierra el
//! proceso: «Salir» sólo emite [`Evento::Salir`] y la aplicación decide.
//!
//! En Windows un hilo propio mantiene una ventana oculta de nivel superior
//! (no `HWND_MESSAGE`: las ventanas sólo de mensajes no reciben el difundido
//! `TaskbarCreated`, que hace falta para volver a añadir el icono cuando el
//! Explorador se reinicia). El icono es el recurso del ejecutable.
//! En otras plataformas [`Bandeja::iniciar`] devuelve un error explícito: la
//! aplicación debe minimizar la ventana visible en lugar de ocultarla.
//!
//! Funciones de `windows-sys` 0.61 que necesita (dev-dependency de Windows):
//! `Win32_Foundation`, `Win32_Graphics_Gdi` (`WNDCLASSW`, `RegisterClassW`),
//! `Win32_System_LibraryLoader` (`GetModuleHandleW`), `Win32_UI_Shell`
//! (`Shell_NotifyIconW`, `NOTIFYICONDATAW`) y `Win32_UI_WindowsAndMessaging`.

#![cfg_attr(not(windows), allow(dead_code))]

use std::sync::mpsc::{Receiver, TryRecvError};
use std::sync::Mutex;
use std::time::Duration;

#[cfg(windows)]
#[path = "bandeja_win32.rs"]
mod win32;

/// Lo que la persona pidió desde la bandeja.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evento {
    /// El hilo dejó de responder; restaurar y retirar esta bandeja.
    NoDisponible,
    /// Restaurar y enfocar la ventana (clic en el icono o «Abrir Ícaro»).
    Mostrar,
    /// Cerrar Ícaro de forma ordenada; la aplicación decide cómo.
    Salir,
}

/// Icono activo en la bandeja. Al soltarlo se retira el icono sin esperar al
/// hilo nativo. Es `Send + Sync`, así que puede viajar en un `Arc`.
pub struct Bandeja {
    eventos: Mutex<Receiver<Evento>>,
    #[cfg(windows)]
    hilo: Option<win32::Hilo>,
}

impl Bandeja {
    /// Crea el icono. Bloquea como mucho unos segundos: llamarla desde
    /// `spawn_blocking`, nunca desde `update` o `view`.
    pub fn iniciar() -> Result<Self, String> {
        #[cfg(windows)]
        {
            let (emisor, receptor) = std::sync::mpsc::channel();
            let hilo = win32::Hilo::iniciar(emisor)?;
            Ok(Self {
                eventos: Mutex::new(receptor),
                hilo: Some(hilo),
            })
        }
        #[cfg(not(windows))]
        {
            Err(String::from(
                "La bandeja del sistema no está disponible en esta plataforma",
            ))
        }
    }

    /// Siguiente evento pendiente, sin bloquear.
    pub fn siguiente(&self) -> Option<Evento> {
        let eventos = self
            .eventos
            .lock()
            .unwrap_or_else(|envenenado| envenenado.into_inner());
        match eventos.try_recv() {
            Ok(evento) => Some(evento),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Evento::NoDisponible),
        }
    }

    /// Retira el icono y espera como mucho `espera` a que el hilo nativo lo
    /// confirme. Opcional: útil en `spawn_blocking` justo antes de terminar el
    /// proceso, para no dejar un icono huérfano. Devuelve si terminó a tiempo.
    #[allow(dead_code)]
    pub fn cerrar(self, espera: Duration) -> bool {
        #[cfg(windows)]
        if let Some(hilo) = &self.hilo {
            hilo.cerrar();
            return hilo.esperar_cierre(espera);
        }
        let _ = espera;
        true
    }

    #[cfg(test)]
    fn desde_canal(eventos: Receiver<Evento>) -> Self {
        Self {
            eventos: Mutex::new(eventos),
            #[cfg(windows)]
            hilo: None,
        }
    }
}

impl Drop for Bandeja {
    fn drop(&mut self) {
        // Sólo avisa al hilo nativo: él retira el icono, destruye la ventana y
        // el menú y libera su contexto. No hay `join` que bloquee la interfaz.
        #[cfg(windows)]
        if let Some(hilo) = &self.hilo {
            hilo.cerrar();
        }
    }
}

/// Identificadores de las órdenes del menú contextual.
const ID_ABRIR: usize = 1;
const ID_SALIR: usize = 2;

const TEXTO_ABRIR: &str = "Abrir Ícaro";
const TEXTO_SALIR: &str = "Salir";
const TEXTO_AYUDA: &str = "Ícaro";

/// Notificaciones del icono (palabra baja de `lParam`), copiadas del SDK para
/// probar la interpretación en cualquier plataforma.
const NIN_SELECT: u32 = 0x0400;
const NIN_KEYSELECT: u32 = 0x0401;
const WM_CONTEXTMENU: u32 = 0x007B;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_RBUTTONUP: u32 = 0x0205;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Notificacion {
    Mostrar,
    Menu,
    Ninguna,
}

/// Con `NOTIFYICON_VERSION_4` llegan `NIN_SELECT`/`NIN_KEYSELECT` y
/// `WM_CONTEXTMENU` junto con los mensajes de ratón; sin ella, sólo estos.
/// Interpretar únicamente una de las dos familias evita acciones dobles.
fn interpretar_notificacion(evento: u32, version4: bool) -> Notificacion {
    match (evento, version4) {
        (NIN_SELECT | NIN_KEYSELECT, true) | (WM_LBUTTONUP, false) => Notificacion::Mostrar,
        (WM_CONTEXTMENU, true) | (WM_RBUTTONUP, false) => Notificacion::Menu,
        _ => Notificacion::Ninguna,
    }
}

fn evento_de_comando(comando: usize) -> Option<Evento> {
    match comando {
        ID_ABRIR => Some(Evento::Mostrar),
        ID_SALIR => Some(Evento::Salir),
        _ => None,
    }
}

/// Punto de anclaje de la versión 4 (`wParam`), con signo para monitores a
/// la izquierda o encima del principal.
fn coordenadas(wparam: usize) -> (i32, i32) {
    let x = (wparam & 0xFFFF) as u16 as i16 as i32;
    let y = ((wparam >> 16) & 0xFFFF) as u16 as i16 as i32;
    (x, y)
}

/// Texto UTF-16 terminado en cero para las API `W`.
fn utf16(texto: &str) -> Vec<u16> {
    texto.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Copia `texto` en un búfer fijo terminado en cero, recortando por
/// caracteres completos (nunca deja medio par sustituto).
fn texto_fijo<const N: usize>(texto: &str) -> [u16; N] {
    let mut destino = [0u16; N];
    let mut usado = 0;
    let mut unidad = [0u16; 2];
    for caracter in texto.chars() {
        let codificado = caracter.encode_utf16(&mut unidad);
        if usado + codificado.len() >= N {
            break;
        }
        destino[usado..usado + codificado.len()].copy_from_slice(codificado);
        usado += codificado.len();
    }
    destino
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn siguiente_entrega_eventos_en_orden_y_no_bloquea_sin_eventos() {
        let (emisor, receptor) = mpsc::channel();
        let bandeja = Bandeja::desde_canal(receptor);
        assert_eq!(bandeja.siguiente(), None);
        emisor.send(Evento::Mostrar).unwrap();
        emisor.send(Evento::Salir).unwrap();
        assert_eq!(bandeja.siguiente(), Some(Evento::Mostrar));
        assert_eq!(bandeja.siguiente(), Some(Evento::Salir));
        assert_eq!(bandeja.siguiente(), None);
        drop(emisor);
        assert_eq!(bandeja.siguiente(), Some(Evento::NoDisponible));
    }

    #[test]
    fn soltar_o_cerrar_sin_hilo_nativo_no_falla() {
        let (_emisor, receptor) = mpsc::channel();
        drop(Bandeja::desde_canal(receptor));
        let (_emisor, receptor) = mpsc::channel();
        assert!(Bandeja::desde_canal(receptor).cerrar(Duration::ZERO));
    }

    #[test]
    fn bandeja_puede_compartirse_entre_hilos() {
        fn compartible<T: Send + Sync>() {}
        compartible::<Bandeja>();
    }

    #[test]
    fn version4_distingue_seleccion_menu_y_raton() {
        assert_eq!(
            interpretar_notificacion(NIN_SELECT, true),
            Notificacion::Mostrar
        );
        assert_eq!(
            interpretar_notificacion(NIN_KEYSELECT, true),
            Notificacion::Mostrar
        );
        assert_eq!(
            interpretar_notificacion(WM_CONTEXTMENU, true),
            Notificacion::Menu
        );
        // En la versión 4 los mensajes de ratón acompañan a los anteriores.
        assert_eq!(
            interpretar_notificacion(WM_LBUTTONUP, true),
            Notificacion::Ninguna
        );
        assert_eq!(
            interpretar_notificacion(WM_RBUTTONUP, true),
            Notificacion::Ninguna
        );
    }

    #[test]
    fn modo_clasico_usa_los_mensajes_de_raton() {
        assert_eq!(
            interpretar_notificacion(WM_LBUTTONUP, false),
            Notificacion::Mostrar
        );
        assert_eq!(
            interpretar_notificacion(WM_RBUTTONUP, false),
            Notificacion::Menu
        );
        assert_eq!(
            interpretar_notificacion(NIN_SELECT, false),
            Notificacion::Ninguna
        );
        assert_eq!(
            interpretar_notificacion(WM_CONTEXTMENU, false),
            Notificacion::Ninguna
        );
    }

    #[cfg(windows)]
    #[test]
    fn constantes_locales_coinciden_con_el_sdk() {
        use windows_sys::Win32::UI::{Shell, WindowsAndMessaging as wam};
        assert_eq!(NIN_SELECT, Shell::NIN_SELECT);
        // NIN_KEYSELECT es la macro NIN_SELECT | NINF_KEY en el SDK;
        // windows-sys no exporta esa macro compuesta.
        assert_eq!(NIN_KEYSELECT, Shell::NIN_SELECT | 1);
        assert_eq!(WM_CONTEXTMENU, wam::WM_CONTEXTMENU);
        assert_eq!(WM_LBUTTONUP, wam::WM_LBUTTONUP);
        assert_eq!(WM_RBUTTONUP, wam::WM_RBUTTONUP);
    }

    #[test]
    fn el_menu_solo_emite_eventos_y_cancelar_no_hace_nada() {
        assert_eq!(evento_de_comando(ID_ABRIR), Some(Evento::Mostrar));
        assert_eq!(evento_de_comando(ID_SALIR), Some(Evento::Salir));
        assert_eq!(evento_de_comando(0), None);
        assert_eq!(evento_de_comando(99), None);
    }

    #[test]
    fn coordenadas_conservan_el_signo_en_otros_monitores() {
        assert_eq!(coordenadas(0x0064_00C8), (200, 100));
        let negativas = ((-20i16 as u16 as usize) << 16) | (-1500i16 as u16 as usize);
        assert_eq!(coordenadas(negativas), (-1500, -20));
    }

    #[test]
    fn textos_utf16_terminan_en_cero_y_recortan_sin_partir_caracteres() {
        assert_eq!(
            utf16("Ícaro"),
            vec![0xCD, 'c' as u16, 'a' as u16, 'r' as u16, 'o' as u16, 0]
        );
        let ayuda: [u16; 128] = texto_fijo(TEXTO_AYUDA);
        assert_eq!(&ayuda[..6], &utf16(TEXTO_AYUDA)[..]);
        let corto: [u16; 4] = texto_fijo("abcdef");
        assert_eq!(corto, [b'a' as u16, b'b' as u16, b'c' as u16, 0]);
        // Un emoji ocupa dos unidades: no cabe entero tras «a» con 3 libres.
        let par: [u16; 3] = texto_fijo("a😀");
        assert_eq!(par, [b'a' as u16, 0, 0]);
        let vacio: [u16; 1] = texto_fijo("Ícaro");
        assert_eq!(vacio, [0]);
    }
}
