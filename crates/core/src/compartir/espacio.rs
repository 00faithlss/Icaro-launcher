// SPDX-License-Identifier: GPL-3.0-only
//! Presupuesto conservador; consultar desde un servicio bloqueante, nunca iced.
use super::Error;
use std::path::Path;

// Diario (hasta 32 MiB), opciones, manifiesto y margen. La comprobación no
// reserva bloques: otros procesos todavía pueden consumir el espacio después.
const RESERVA: u64 = 64 * 1024 * 1024;
pub fn presupuesto(archivos: &[u64]) -> Result<u64, Error> {
    archivos.iter().try_fold(RESERVA, |total, bytes| {
        bytes
            .checked_mul(2)
            .and_then(|b| b.checked_add(16 * 1024))
            .and_then(|b| total.checked_add(b))
            .ok_or(Error::Limite("presupuesto de disco"))
    })
}
pub fn comprobar_disponible(necesarios: u64, disponibles: u64) -> Result<(), Error> {
    crate::diagnostico_contexto::dato(crate::diagnostico::Campo::Numero(
        "disco_necesarios",
        necesarios,
    ));
    crate::diagnostico_contexto::dato(crate::diagnostico::Campo::Numero(
        "disco_disponibles",
        disponibles,
    ));
    if disponibles < necesarios {
        Err(Error::Datos(
            "espacio libre insuficiente para preparar la revisión",
        ))
    } else {
        Ok(())
    }
}
pub fn comprobar(ruta: &Path, archivos: &[u64]) -> Result<(), Error> {
    let existente = ruta.ancestors().find(|p| p.exists()).ok_or(Error::Disco)?;
    comprobar_disponible(presupuesto(archivos)?, disponible(existente)?)
}
#[cfg(windows)]
pub fn disponible(ruta: &Path) -> Result<u64, Error> {
    use std::os::windows::ffi::OsStrExt;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetDiskFreeSpaceExW(
            path: *const u16,
            libres: *mut u64,
            total: *mut u64,
            total_libres: *mut u64,
        ) -> i32;
    }
    let ruta = ruta.canonicalize().map_err(super::diagnostico::disco)?;
    let nombre: Vec<_> = ruta.as_os_str().encode_wide().chain(Some(0)).collect();
    let mut libres = 0;
    if unsafe {
        GetDiskFreeSpaceExW(
            nombre.as_ptr(),
            &mut libres,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    } == 0
    {
        return Err(super::diagnostico::disco(std::io::Error::last_os_error()));
    }
    Ok(libres)
}
#[cfg(target_os = "linux")]
pub fn disponible(ruta: &Path) -> Result<u64, Error> {
    // GNU df viene con coreutils en los sistemas Linux admitidos. Sin shell ni
    // interpolación: una ruta nunca se interpreta como comando ni como opción.
    let salida = std::process::Command::new("df")
        .env("LC_ALL", "C")
        .arg("-Pk")
        .arg("--")
        .arg(ruta)
        .output()
        .map_err(super::diagnostico::disco)?;
    if !salida.status.success() {
        return Err(Error::Disco);
    }
    std::str::from_utf8(&salida.stdout)
        .ok()
        .and_then(|s| s.lines().last())
        .and_then(|l| l.split_whitespace().nth(3))
        .and_then(|v| v.parse::<u64>().ok())
        .and_then(|b| b.checked_mul(1024))
        .ok_or(Error::Disco)
}
#[cfg(not(any(windows, target_os = "linux")))]
pub fn disponible(_: &Path) -> Result<u64, Error> {
    Err(Error::Datos(
        "consulta de espacio no disponible en esta plataforma",
    ))
}
