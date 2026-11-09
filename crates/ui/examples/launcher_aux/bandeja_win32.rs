// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Hilo Win32 de la bandeja: ventana oculta, `Shell_NotifyIconW` y menú.
//!
//! Todo objeto nativo nace y muere en este hilo. El contexto del procedimiento
//! es una `Box` de [`ejecutar`] publicada en una variable local del hilo, que
//! se retira y libera tras destruir la ventana y salir del bucle de mensajes.

use super::{
    coordenadas, evento_de_comando, interpretar_notificacion, texto_fijo, utf16, Evento,
    Notificacion, ID_ABRIR, ID_SALIR, TEXTO_ABRIR, TEXTO_AYUDA, TEXTO_SALIR,
};
use std::cell::Cell;
use std::ptr::{null, null_mut};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use windows_sys::Win32::Foundation::{
    GetLastError, HINSTANCE, HWND, LPARAM, LRESULT, POINT, WPARAM,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE,
    NIM_SETVERSION, NOTIFYICONDATAW, NOTIFYICON_VERSION_4,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, ChangeWindowMessageFilterEx, CreatePopupMenu, CreateWindowExW, DefWindowProcW,
    DestroyMenu, DestroyWindow, DispatchMessageW, GetCursorPos, GetMessageW, LoadIconW,
    PostMessageW, PostQuitMessage, RegisterClassW, RegisterWindowMessageW, SetForegroundWindow,
    SetMenuDefaultItem, TrackPopupMenuEx, TranslateMessage, UnregisterClassW, HICON, HMENU,
    MF_SEPARATOR, MF_STRING, MSG, MSGFLT_ALLOW, TPM_BOTTOMALIGN, TPM_NONOTIFY, TPM_RETURNCMD,
    TPM_RIGHTBUTTON, WM_APP, WM_CLOSE, WM_DESTROY, WM_NULL, WNDCLASSW, WS_EX_TOOLWINDOW,
    WS_OVERLAPPED,
};

/// Avisos del Explorador sobre el icono y petición de cierre de la aplicación.
const WM_BANDEJA: u32 = WM_APP + 1;
const WM_CERRAR: u32 = WM_APP + 2;
const ID_ICONO: u32 = 1;
/// Límite del arranque; incluye los reintentos de `NIM_ADD` al iniciar sesión.
const ESPERA_INICIO: Duration = Duration::from_secs(5);
const REINTENTOS_ICONO: u64 = 4;

static SIGUIENTE_CLASE: AtomicU32 = AtomicU32::new(0);

thread_local! {
    static CONTEXTO: Cell<*const Contexto> = const { Cell::new(null()) };
}

/// Estado del hilo nativo; sólo se accede por referencia compartida, así que
/// los bucles modales del menú pueden reentrar en el procedimiento sin riesgo.
struct Contexto {
    eventos: Sender<Evento>,
    icono: HICON,
    menu: Cell<HMENU>,
    taskbar_created: u32,
    agregado: Cell<bool>,
    version4: Cell<bool>,
    vivo: Arc<AtomicBool>,
}

/// Lado de la aplicación: sólo valores `Send + Sync`.
pub(super) struct Hilo {
    hwnd: usize,
    vivo: Arc<AtomicBool>,
    cierre_pedido: AtomicBool,
    terminado: Mutex<Receiver<()>>,
}

impl Hilo {
    pub(super) fn iniciar(eventos: Sender<Evento>) -> Result<Self, String> {
        // Canal de cita: si `iniciar` deja de esperar, el envío falla y el
        // hilo deshace lo creado en vez de dejar un icono huérfano.
        let (listo, confirmacion) = mpsc::sync_channel::<Result<usize, String>>(0);
        let (fin, terminado) = mpsc::channel();
        let vivo = Arc::new(AtomicBool::new(false));
        let vivo_hilo = Arc::clone(&vivo);
        std::thread::Builder::new()
            .name("icaro-bandeja".into())
            .spawn(move || ejecutar(eventos, listo, fin, vivo_hilo))
            .map_err(|error| format!("No se pudo crear el hilo de la bandeja: {error}"))?;
        match confirmacion.recv_timeout(ESPERA_INICIO) {
            Ok(Ok(hwnd)) => Ok(Self {
                hwnd,
                vivo,
                cierre_pedido: AtomicBool::new(false),
                terminado: Mutex::new(terminado),
            }),
            Ok(Err(error)) => Err(error),
            Err(RecvTimeoutError::Timeout) => {
                Err("La bandeja del sistema no respondió a tiempo".into())
            }
            Err(RecvTimeoutError::Disconnected) => {
                Err("El hilo de la bandeja terminó antes de crear el icono".into())
            }
        }
    }

    /// Pide el cierre una sola vez, sin esperar.
    pub(super) fn cerrar(&self) {
        if !self.cierre_pedido.swap(true, Ordering::AcqRel) && self.vivo.load(Ordering::Acquire) {
            // La ventana sólo se destruye al atender este mensaje: el HWND sigue
            // siendo suyo mientras `vivo` es verdadero.
            unsafe { PostMessageW(self.hwnd as HWND, WM_CERRAR, 0, 0) };
        }
    }

    pub(super) fn esperar_cierre(&self, espera: Duration) -> bool {
        let terminado = self
            .terminado
            .lock()
            .unwrap_or_else(|envenenado| envenenado.into_inner());
        !matches!(
            terminado.recv_timeout(espera),
            Err(RecvTimeoutError::Timeout)
        )
    }
}

fn ejecutar(
    eventos: Sender<Evento>,
    listo: SyncSender<Result<usize, String>>,
    fin: Sender<()>,
    vivo: Arc<AtomicBool>,
) {
    let instancia = unsafe { GetModuleHandleW(null()) };
    let clase = utf16(&format!(
        "IcaroBandeja-{}-{}",
        std::process::id(),
        SIGUIENTE_CLASE.fetch_add(1, Ordering::Relaxed)
    ));
    let taskbar_created = unsafe { RegisterWindowMessageW(utf16("TaskbarCreated").as_ptr()) };
    let icono = unsafe { cargar_icono(instancia) };
    if icono.is_null() {
        let _ = listo.send(Err(
            "No se pudo cargar el icono de Ícaro para la bandeja.".into()
        ));
        let _ = fin.send(());
        return;
    }
    let contexto = Box::new(Contexto {
        eventos,
        icono,
        menu: Cell::new(null_mut()),
        taskbar_created,
        agregado: Cell::new(false),
        version4: Cell::new(false),
        vivo: Arc::clone(&vivo),
    });
    CONTEXTO.with(|actual| actual.set(&*contexto));

    match unsafe { preparar(instancia, &clase, &contexto) } {
        Err(error) => {
            let _ = listo.send(Err(error));
        }
        Ok(hwnd) => {
            vivo.store(true, Ordering::Release);
            if listo.send(Ok(hwnd as usize)).is_err() {
                // `iniciar` ya devolvió un error por tiempo: deshacer.
                unsafe { DestroyWindow(hwnd) };
            }
            drop(listo);
            bucle_de_mensajes();
        }
    }

    // La ventana ya no existe (o nunca existió): ningún mensaje más llegará
    // al procedimiento, así que el contexto puede retirarse y liberarse.
    vivo.store(false, Ordering::Release);
    unsafe {
        let menu = contexto.menu.replace(null_mut());
        if !menu.is_null() {
            DestroyMenu(menu);
        }
        UnregisterClassW(clase.as_ptr(), instancia);
    }
    CONTEXTO.with(|actual| actual.set(null()));
    // El icono de `LoadIconW` es compartido por el sistema: no se destruye.
    drop(contexto);
    let _ = fin.send(());
}

fn bucle_de_mensajes() {
    let mut mensaje: MSG = unsafe { std::mem::zeroed() };
    // `GetMessageW` devuelve 0 con WM_QUIT y -1 si falla: ambos terminan.
    while unsafe { GetMessageW(&mut mensaje, null_mut(), 0, 0) } > 0 {
        unsafe {
            TranslateMessage(&mensaje);
            DispatchMessageW(&mensaje);
        }
    }
}

/// Registra la clase, crea el menú y la ventana y añade el icono. Si algo
/// falla deshace la ventana; el menú y la clase los libera [`ejecutar`].
unsafe fn preparar(
    instancia: HINSTANCE,
    clase: &[u16],
    contexto: &Contexto,
) -> Result<HWND, String> {
    let registro = WNDCLASSW {
        style: 0,
        lpfnWndProc: Some(procedimiento),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: instancia,
        hIcon: contexto.icono,
        hCursor: null_mut(),
        hbrBackground: null_mut(),
        lpszMenuName: null(),
        lpszClassName: clase.as_ptr(),
    };
    if RegisterClassW(&registro) == 0 {
        return Err(fallo("registrar la clase de la bandeja"));
    }
    contexto.menu.set(crear_menu()?);
    let titulo = utf16(TEXTO_AYUDA);
    // Ventana de nivel superior nunca mostrada (sin WS_VISIBLE): recibe el
    // difundido `TaskbarCreated` y sirve de dueña del menú contextual.
    let hwnd = CreateWindowExW(
        WS_EX_TOOLWINDOW,
        clase.as_ptr(),
        titulo.as_ptr(),
        WS_OVERLAPPED,
        0,
        0,
        0,
        0,
        null_mut(),
        null_mut(),
        instancia,
        null(),
    );
    if hwnd.is_null() {
        return Err(fallo("crear la ventana oculta de la bandeja"));
    }
    if contexto.taskbar_created != 0 {
        // Permite el aviso aunque el proceso tenga más integridad que el Explorador.
        ChangeWindowMessageFilterEx(hwnd, contexto.taskbar_created, MSGFLT_ALLOW, null_mut());
    }
    // Al iniciar sesión el Explorador puede no estar listo todavía.
    for intento in 0..REINTENTOS_ICONO {
        if agregar_icono(hwnd, contexto) {
            return Ok(hwnd);
        }
        if intento + 1 < REINTENTOS_ICONO {
            std::thread::sleep(Duration::from_millis(250 * (intento + 1)));
        }
    }
    DestroyWindow(hwnd);
    Err("No se pudo agregar el icono a la bandeja del sistema".into())
}

unsafe fn crear_menu() -> Result<HMENU, String> {
    let menu = CreatePopupMenu();
    if menu.is_null() {
        return Err(fallo("crear el menú de la bandeja"));
    }
    // `AppendMenuW` copia los textos: los búferes temporales bastan.
    let abrir = utf16(TEXTO_ABRIR);
    let salir = utf16(TEXTO_SALIR);
    let correcto = AppendMenuW(menu, MF_STRING, ID_ABRIR, abrir.as_ptr()) != 0
        && AppendMenuW(menu, MF_SEPARATOR, 0, null()) != 0
        && AppendMenuW(menu, MF_STRING, ID_SALIR, salir.as_ptr()) != 0;
    if !correcto {
        let error = fallo("crear el menú de la bandeja");
        DestroyMenu(menu);
        return Err(error);
    }
    SetMenuDefaultItem(menu, ID_ABRIR as u32, 0);
    Ok(menu)
}

/// Icono del ejecutable. `icaro.rc` declara `IDI_ICARO` sin `#define`, así
/// que según el compilador de recursos queda como entero 1 o como nombre.
/// Si no existe el recurso propio, el inicio falla y la ventana sigue accesible.
unsafe fn cargar_icono(instancia: HINSTANCE) -> HICON {
    let mut icono = LoadIconW(instancia, 1usize as *const u16);
    if icono.is_null() {
        let nombre = utf16("IDI_ICARO");
        icono = LoadIconW(instancia, nombre.as_ptr());
    }
    icono
}

fn datos_icono(hwnd: HWND) -> NOTIFYICONDATAW {
    // Estructura de datos planos: todo a cero es un valor válido.
    let mut datos: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    datos.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    datos.hWnd = hwnd;
    datos.uID = ID_ICONO;
    datos
}

unsafe fn agregar_icono(hwnd: HWND, contexto: &Contexto) -> bool {
    let mut datos = datos_icono(hwnd);
    datos.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP;
    datos.uCallbackMessage = WM_BANDEJA;
    datos.hIcon = contexto.icono;
    datos.szTip = texto_fijo(TEXTO_AYUDA);
    if Shell_NotifyIconW(NIM_ADD, &datos) == 0 {
        return false;
    }
    contexto.agregado.set(true);
    datos.Anonymous.uVersion = NOTIFYICON_VERSION_4;
    contexto
        .version4
        .set(Shell_NotifyIconW(NIM_SETVERSION, &datos) != 0);
    true
}

unsafe fn quitar_icono(hwnd: HWND, contexto: &Contexto) {
    if contexto.agregado.replace(false) {
        Shell_NotifyIconW(NIM_DELETE, &datos_icono(hwnd));
    }
}

unsafe fn mostrar_menu(hwnd: HWND, contexto: &Contexto, x: i32, y: i32) {
    let menu = contexto.menu.get();
    if menu.is_null() {
        return;
    }
    // Sin primer plano el menú no se cierra al hacer clic fuera; WM_NULL
    // tras cerrarlo es el arreglo documentado para el segundo clic.
    SetForegroundWindow(hwnd);
    let comando = TrackPopupMenuEx(
        menu,
        TPM_RETURNCMD | TPM_NONOTIFY | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN,
        x,
        y,
        hwnd,
        null(),
    );
    PostMessageW(hwnd, WM_NULL, 0, 0);
    if let Some(evento) = evento_de_comando(comando as usize) {
        emitir(contexto, evento);
    }
}

fn emitir(contexto: &Contexto, evento: Evento) {
    let _ = contexto.eventos.send(evento); // Sin receptor, ya no interesa.
}

fn fallo(accion: &str) -> String {
    let codigo = unsafe { GetLastError() };
    format!("No se pudo {accion} (código {codigo})")
}

unsafe extern "system" fn procedimiento(
    hwnd: HWND,
    mensaje: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // `try_with` evita pánicos (que abortarían aquí) durante la salida del hilo.
    let puntero = CONTEXTO.try_with(Cell::get).unwrap_or(null());
    if puntero.is_null() {
        return DefWindowProcW(hwnd, mensaje, wparam, lparam);
    }
    // Válido: `ejecutar` retira el puntero antes de liberar la `Box`.
    let contexto = &*puntero;
    match mensaje {
        WM_BANDEJA => {
            let version4 = contexto.version4.get();
            let evento = (lparam as usize & 0xFFFF) as u32;
            match interpretar_notificacion(evento, version4) {
                Notificacion::Mostrar => emitir(contexto, Evento::Mostrar),
                Notificacion::Menu => {
                    let (x, y) = if version4 {
                        coordenadas(wparam)
                    } else {
                        cursor()
                    };
                    mostrar_menu(hwnd, contexto, x, y);
                }
                Notificacion::Ninguna => {}
            }
            0
        }
        WM_CERRAR => {
            DestroyWindow(hwnd);
            0
        }
        // Sólo la aplicación cierra la bandeja, mediante WM_CERRAR.
        WM_CLOSE => 0,
        WM_DESTROY => {
            contexto.vivo.store(false, Ordering::Release);
            quitar_icono(hwnd, contexto);
            PostQuitMessage(0);
            0
        }
        // `taskbar_created` vale 0 si no pudo registrarse: no confundirlo con WM_NULL.
        _ if contexto.taskbar_created != 0 && mensaje == contexto.taskbar_created => {
            // El Explorador se reinició y olvidó el icono: volver a añadirlo.
            contexto.agregado.set(false);
            if !agregar_icono(hwnd, contexto) {
                let _ = contexto.eventos.send(Evento::Mostrar);
            }
            0
        }
        _ => DefWindowProcW(hwnd, mensaje, wparam, lparam),
    }
}

fn cursor() -> (i32, i32) {
    let mut punto = POINT { x: 0, y: 0 };
    unsafe { GetCursorPos(&mut punto) };
    (punto.x, punto.y)
}
