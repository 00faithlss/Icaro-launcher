// SPDX-License-Identifier: GPL-3.0-only
/// Pico residente acumulado del proceso, incluyendo fixture y runtime.
#[cfg(windows)]
pub fn pico() -> Option<u64> {
    #[repr(C)]
    struct Contadores {
        cb: u32,
        fallos: u32,
        pico: usize,
        trabajo: usize,
        cuota_paginada_pico: usize,
        cuota_paginada: usize,
        cuota_no_paginada_pico: usize,
        cuota_no_paginada: usize,
        pagina: usize,
        pagina_pico: usize,
    }
    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentProcess() -> *mut std::ffi::c_void;
    }
    #[link(name = "psapi")]
    extern "system" {
        fn GetProcessMemoryInfo(
            proceso: *mut std::ffi::c_void,
            datos: *mut Contadores,
            bytes: u32,
        ) -> i32;
    }
    // La estructura sigue PROCESS_MEMORY_COUNTERS de Win32, tamaño específico de arquitectura.
    unsafe {
        let mut datos: Contadores = std::mem::zeroed();
        datos.cb = std::mem::size_of::<Contadores>() as u32;
        if GetProcessMemoryInfo(
            GetCurrentProcess(),
            &mut datos,
            std::mem::size_of::<Contadores>() as u32,
        ) == 0
        {
            None
        } else {
            Some(datos.pico as u64)
        }
    }
}
#[cfg(target_os = "linux")]
pub fn pico() -> Option<u64> {
    std::fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find(|l| l.starts_with("VmHWM:"))?
        .split_whitespace()
        .nth(1)?
        .parse::<u64>()
        .ok()?
        .checked_mul(1024)
}
#[cfg(not(any(windows, target_os = "linux")))]
pub fn pico() -> Option<u64> {
    None
}
