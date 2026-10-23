// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Apertura mediante el sistema, incluida la activación de navegadores empaquetados.

use super::{Error, Url};

pub(super) fn open(url: &Url) -> Result<(), Error> {
    #[cfg(windows)]
    {
        use std::ffi::c_void;
        use windows::Win32::System::Com::{
            CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
        };

        #[link(name = "shell32")]
        extern "system" {
            fn ShellExecuteW(
                window: *mut c_void,
                operation: *const u16,
                file: *const u16,
                parameters: *const u16,
                directory: *const u16,
                show: i32,
            ) -> isize;
        }

        // No ejecutar el comando del registro: Dia usa DelegateExecute y puede
        // no incluir %1. El shell entrega la URL completa a la asociación HTTPS.
        let target = wide_url(url)?;
        let operation: Vec<u16> = "open".encode_utf16().chain(Some(0)).collect();
        // Las asociaciones con DelegateExecute pueden necesitar COM. El hilo
        // de spawn_blocking puede reutilizar un apartamento ya inicializado.
        // SAFETY: inicialización para el hilo actual, sin puntero reservado.
        let initialized =
            unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) }
                .is_ok();
        // SAFETY: cadenas UTF-16 terminadas en NUL, vigentes durante la llamada;
        // los parámetros opcionales son nulos. No interviene un intérprete.
        let result = unsafe {
            ShellExecuteW(
                std::ptr::null_mut(),
                operation.as_ptr(),
                target.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                1,
            )
        };
        if initialized {
            // SAFETY: equilibra solo nuestra inicialización COM satisfactoria.
            unsafe { CoUninitialize() };
        }
        if result > 32 {
            Ok(())
        } else {
            Err(Error::Browser)
        }
    }
    #[cfg(not(windows))]
    {
        webbrowser::open(url.as_str()).map_err(|_| Error::Browser)
    }
}

#[cfg(any(windows, test))]
fn wide_url(url: &Url) -> Result<Vec<u16>, Error> {
    if url.scheme() != "https" || url.as_str().contains('\0') {
        return Err(Error::Browser);
    }
    Ok(url.as_str().encode_utf16().chain(Some(0)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conserva_parametros_oauth_en_un_solo_enlace() {
        let url = Url::parse("https://login.microsoftonline.com/consumers/oauth2/v2.0/authorize?redirect_uri=icaro%3A%2F%2Foauth%2Fmicrosoft&state=prueba&code_challenge=abc-123&code_challenge_method=S256").unwrap();
        let wide = wide_url(&url).unwrap();
        assert_eq!(wide.last(), Some(&0));
        assert_eq!(
            String::from_utf16(&wide[..wide.len() - 1]).unwrap(),
            url.as_str()
        );
        assert!(!wide[..wide.len() - 1].contains(&0));
    }

    #[test]
    fn rechaza_protocolos_ajenos_al_login() {
        for value in [
            "file:///C:/Windows/notepad.exe",
            "icaro://oauth/microsoft",
            "http://example.com",
        ] {
            assert_eq!(wide_url(&Url::parse(value).unwrap()), Err(Error::Browser));
        }
    }
}
