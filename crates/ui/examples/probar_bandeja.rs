// SPDX-License-Identifier: GPL-3.0-only
//! Comprobación nativa aislada: icono propio, restaurar, recrear y retirar.
#[allow(dead_code)]
#[path = "launcher_aux/bandeja.rs"]
mod bandeja;
#[cfg(windows)]
fn main() {
    use std::{
        ptr::null_mut,
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::{HWND, LPARAM},
        UI::{
            Shell::{Shell_NotifyIconW, NIM_DELETE, NOTIFYICONDATAW},
            WindowsAndMessaging::*,
        },
    };
    unsafe extern "system" fn buscar(hwnd: HWND, dato: LPARAM) -> i32 {
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, &mut pid);
        if pid == std::process::id() {
            let mut clase = [0u16; 128];
            let n = GetClassNameW(hwnd, clase.as_mut_ptr(), 128);
            if n > 0 && String::from_utf16_lossy(&clase[..n as usize]).starts_with("IcaroBandeja-")
            {
                *(dato as *mut HWND) = hwnd;
                return 0;
            }
        }
        1
    }
    fn esperar(b: &bandeja::Bandeja) {
        let fin = Instant::now() + Duration::from_secs(3);
        loop {
            if b.siguiente() == Some(bandeja::Evento::Mostrar) {
                return;
            }
            assert!(Instant::now() < fin, "No llegó restaurar desde la bandeja");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
    let b = bandeja::Bandeja::iniciar().expect("Iniciar bandeja con recurso del ejecutable");
    let mut hwnd: HWND = null_mut();
    unsafe {
        EnumWindows(Some(buscar), &mut hwnd as *mut HWND as LPARAM);
    }
    assert!(!hwnd.is_null(), "La ventana de bandeja debe existir");
    unsafe {
        PostMessageW(hwnd, WM_APP + 1, 0, ((1u32 << 16) | 1024) as LPARAM);
    }
    esperar(&b);
    // Reproducir únicamente la pérdida de ESTE icono; no reiniciar Explorer ni
    // tocar ventanas/iconos de otras aplicaciones de la sesión.
    let mut datos: NOTIFYICONDATAW = unsafe { std::mem::zeroed() };
    datos.cbSize = std::mem::size_of_val(&datos) as u32;
    datos.hWnd = hwnd;
    datos.uID = 1;
    assert_ne!(unsafe { Shell_NotifyIconW(NIM_DELETE, &datos) }, 0);
    let nombre: Vec<u16> = "TaskbarCreated\0".encode_utf16().collect();
    let mensaje = unsafe { RegisterWindowMessageW(nombre.as_ptr()) };
    assert_ne!(mensaje, 0);
    unsafe {
        SendMessageW(hwnd, mensaje, 0, 0);
        PostMessageW(hwnd, WM_APP + 1, 0, ((1u32 << 16) | 1024) as LPARAM);
    }
    esperar(&b);
    assert_ne!(
        unsafe { Shell_NotifyIconW(NIM_DELETE, &datos) },
        0,
        "El icono se volvió a registrar"
    );
    assert!(
        b.cerrar(Duration::from_secs(3)),
        "Retirar icono y terminar hilo"
    );
    assert_eq!(unsafe { IsWindow(hwnd) }, 0);
    println!("PASS bandeja Windows: recurso propio, restaurar, recreación de icono y cierre del hilo. No prueba clic físico ni reinicio real de Explorer.");
}
#[cfg(not(windows))]
fn main() {
    eprintln!("Esta comprobación nativa requiere Windows.");
}
