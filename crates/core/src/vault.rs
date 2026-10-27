// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Una cuenta recordada, protegida por el almacén del sistema. No usa archivos.

use crate::auth::{Session, CLIENT_ID};
use serde::{Deserialize, Serialize};
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Mutex,
};
use zeroize::Zeroizing;

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    #[error("No se pudo acceder al almacén de credenciales del sistema.")]
    Unavailable,
    #[error("La credencial supera el tamaño admitido por el almacén del sistema.")]
    TooLarge,
    #[error("La sesión cambió durante esta operación.")]
    Stale,
}

trait Store: Send + Sync {
    fn get(&self) -> Result<Option<Zeroizing<String>>, Error>;
    fn set(&self, value: &str) -> Result<(), Error>;
    fn delete(&self) -> Result<(), Error>;
}

struct SystemStore;
#[cfg(any(windows, target_os = "linux"))]
fn entry() -> Result<keyring::Entry, Error> {
    keyring::Entry::new(
        &format!("org.icaro.launcher.microsoft.{CLIENT_ID}"),
        "active-account",
    )
    .map_err(|_| Error::Unavailable)
}

impl Store for SystemStore {
    fn get(&self) -> Result<Option<Zeroizing<String>>, Error> {
        #[cfg(windows)]
        {
            read_windows(&entry()?)
        }
        #[cfg(target_os = "linux")]
        {
            match entry()?.get_password() {
                Ok(value) => Ok(Some(Zeroizing::new(value))),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(_) => Err(Error::Unavailable),
            }
        }
        #[cfg(not(any(windows, target_os = "linux")))]
        {
            Err(Error::Unavailable)
        }
    }
    fn set(&self, value: &str) -> Result<(), Error> {
        #[cfg(windows)]
        {
            write_windows(&entry()?, value)
        }
        #[cfg(target_os = "linux")]
        {
            entry()?.set_password(value).map_err(|_| Error::Unavailable)
        }
        #[cfg(not(any(windows, target_os = "linux")))]
        {
            let _ = value;
            Err(Error::Unavailable)
        }
    }
    fn delete(&self) -> Result<(), Error> {
        #[cfg(any(windows, target_os = "linux"))]
        {
            match entry()?.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(_) => Err(Error::Unavailable),
            }
        }
        #[cfg(not(any(windows, target_os = "linux")))]
        {
            Err(Error::Unavailable)
        }
    }
}

#[cfg(windows)]
fn read_windows(entry: &keyring::Entry) -> Result<Option<Zeroizing<String>>, Error> {
    let bytes = match entry.get_secret() {
        Ok(bytes) => Zeroizing::new(bytes),
        Err(keyring::Error::NoEntry) => return Ok(None),
        Err(_) => return Err(Error::Unavailable),
    };
    // Las versiones anteriores guardaban el JSON como una contraseña UTF-16.
    let value = if bytes.starts_with(b"{\0") && bytes.len() % 2 == 0 {
        let units = Zeroizing::new(
            bytes
                .chunks_exact(2)
                .map(|p| u16::from_le_bytes([p[0], p[1]]))
                .collect::<Vec<_>>(),
        );
        String::from_utf16(&units).map_err(|_| Error::Unavailable)?
    } else {
        std::str::from_utf8(&bytes)
            .map_err(|_| Error::Unavailable)?
            .to_owned()
    };
    Ok(Some(Zeroizing::new(value)))
}

#[cfg(windows)]
fn write_windows(entry: &keyring::Entry, value: &str) -> Result<(), Error> {
    // Credential Manager admite bytes: UTF-8 evita duplicar el tamaño del token.
    entry
        .set_secret(value.as_bytes())
        .map_err(|error| match error {
            keyring::Error::TooLong(_, _) => Error::TooLarge,
            _ => Error::Unavailable,
        })
}

#[derive(Deserialize)]
pub struct Remembered {
    client_id: String,
    pub profile_id: String,
    #[serde(default)]
    pub profile_name: Option<String>,
    pub refresh: Zeroizing<String>,
}
impl std::fmt::Debug for Remembered {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Remembered([REDACTED])")
    }
}

pub struct Vault {
    store: Box<dyn Store>,
    serial: Mutex<()>,
    generation: AtomicU64,
}
impl Default for Vault {
    fn default() -> Self {
        Self {
            store: Box::new(SystemStore),
            serial: Mutex::new(()),
            generation: AtomicU64::new(0),
        }
    }
}

impl Vault {
    /// Invalida las escrituras de intentos anteriores antes de cancelar o salir.
    pub fn invalidate(&self) -> u64 {
        self.generation.fetch_add(1, Ordering::SeqCst) + 1
    }
    pub fn load(&self) -> Result<Option<Remembered>, Error> {
        let _lock = self.serial.lock().map_err(|_| Error::Unavailable)?;
        let Some(value) = self.store.get()? else {
            return Ok(None);
        };
        let record: Remembered = serde_json::from_str(&value).map_err(|_| Error::Unavailable)?;
        if record.client_id != CLIENT_ID
            || record.profile_id.len() != 32
            || !record.profile_id.bytes().all(|b| b.is_ascii_hexdigit())
            || record.refresh.is_empty()
        {
            return Err(Error::Unavailable);
        }
        let mut record = record;
        if record.profile_name.as_ref().is_some_and(|name| name.is_empty() || name.len() > 16 || !name.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')) {
            record.profile_name = None;
        }
        Ok(Some(record))
    }
    pub fn remember(&self, generation: u64, session: &Session) -> Result<(), Error> {
        if session.es_local() { return Err(Error::Unavailable); }
        self.save_with_name(generation, &session.profile.id, &session.profile.name, &session.refresh)
    }
    #[cfg(test)]
    fn save(&self, generation: u64, profile_id: &str, refresh: &str) -> Result<(), Error> {
        self.save_with_name(generation, profile_id, "", refresh)
    }
    fn save_with_name(&self, generation: u64, profile_id: &str, profile_name: &str, refresh: &str) -> Result<(), Error> {
        let _lock = self.serial.lock().map_err(|_| Error::Unavailable)?;
        if self.generation.load(Ordering::SeqCst) != generation {
            return Err(Error::Stale);
        }
        #[derive(Serialize)]
        struct Record<'a> {
            client_id: &'a str,
            profile_id: &'a str,
            profile_name: &'a str,
            refresh: &'a str,
        }
        let value = Zeroizing::new(
            serde_json::to_string(&Record {
                client_id: CLIENT_ID,
                profile_id,
                profile_name,
                refresh,
            })
            .map_err(|_| Error::Unavailable)?,
        );
        self.store.set(&value)?;
        if self.generation.load(Ordering::SeqCst) != generation {
            self.store.delete()?;
            return Err(Error::Stale);
        }
        Ok(())
    }
    /// El llamador invalida primero; esta operación se serializa con las escrituras.
    pub fn forget(&self, generation: u64) -> Result<(), Error> {
        let _lock = self.serial.lock().map_err(|_| Error::Unavailable)?;
        if self.generation.load(Ordering::SeqCst) != generation {
            return Err(Error::Stale);
        }
        self.store.delete()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(windows)]
    #[test]
    #[ignore = "Usa y elimina una credencial de prueba aislada en Windows"]
    fn windows_reads_legacy_sessions_and_persists_long_utf8_credentials() {
        let mut nonce = [0; 16];
        getrandom::fill(&mut nonce).unwrap();
        let name: String = nonce.iter().map(|b| format!("{b:02x}")).collect();
        struct Cleanup(keyring::Entry);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = self.0.delete_credential();
            }
        }
        let entry = Cleanup(keyring::Entry::new("org.icaro.launcher.tests", &name).unwrap());
        let legacy = r#"{"refresh":"CANARY_LEGACY"}"#;
        entry.0.set_password(legacy).unwrap();
        assert_eq!(read_windows(&entry.0).unwrap().unwrap().as_str(), legacy);
        let long = Zeroizing::new(format!(r#"{{"refresh":"{}"}}"#, "X".repeat(2000)));
        assert!(matches!(
            entry.0.set_password(&long),
            Err(keyring::Error::TooLong(_, _))
        ));
        write_windows(&entry.0, &long).unwrap();
        assert_eq!(
            read_windows(&entry.0).unwrap().unwrap().as_str(),
            long.as_str()
        );
        assert_eq!(
            write_windows(&entry.0, &"X".repeat(6000)),
            Err(Error::TooLarge)
        );
        entry.0.delete_credential().unwrap();
        assert!(read_windows(&entry.0).unwrap().is_none());
    }
    struct MemoryStore(Mutex<Option<Zeroizing<String>>>);
    impl Store for MemoryStore {
        fn get(&self) -> Result<Option<Zeroizing<String>>, Error> {
            Ok(self.0.lock().unwrap().clone())
        }
        fn set(&self, value: &str) -> Result<(), Error> {
            *self.0.lock().unwrap() = Some(Zeroizing::new(value.into()));
            Ok(())
        }
        fn delete(&self) -> Result<(), Error> {
            self.0.lock().unwrap().take();
            Ok(())
        }
    }
    #[test]
    fn perfil_recordado_conserva_nombre_sin_exponer_credenciales() {
        let vault = Vault { store: Box::new(MemoryStore(Mutex::new(None))), serial: Mutex::new(()), generation: AtomicU64::new(0) };
        let id = "0123456789abcdef0123456789abcdef";
        vault.save_with_name(0, id, "Jugador", "CANARY_TOKEN").unwrap();
        let record = vault.load().unwrap().unwrap();
        assert_eq!(record.profile_name.as_deref(), Some("Jugador"));
        assert!(!format!("{record:?}").contains("CANARY_TOKEN"));
    }

    #[test]
    fn stale_attempt_cannot_restore_logged_out_credentials() {
        let vault = Vault {
            store: Box::new(MemoryStore(Mutex::new(None))),
            serial: Mutex::new(()),
            generation: AtomicU64::new(0),
        };
        let first = vault.invalidate();
        let id = "0123456789abcdef0123456789abcdef";
        vault.save(first, id, "CANARY_TOKEN").unwrap();
        let record = vault.load().unwrap().unwrap();
        assert_eq!(record.profile_id, id);
        assert_eq!(&*record.refresh, "CANARY_TOKEN");
        assert!(!format!("{record:?}").contains("CANARY_TOKEN"));
        let logout = vault.invalidate();
        vault.forget(logout).unwrap();
        assert!(matches!(
            vault.save(first, id, "OLD_RESPONSE"),
            Err(Error::Stale)
        ));
        assert!(vault.load().unwrap().is_none());
        let new_login = vault.invalidate();
        vault.save(new_login, id, "NEW_SESSION").unwrap();
        assert!(matches!(vault.forget(logout), Err(Error::Stale)));
        assert_eq!(&*vault.load().unwrap().unwrap().refresh, "NEW_SESSION");
    }
    #[test]
    fn cancellation_during_a_write_removes_the_new_secret() {
        use std::sync::{Arc, Barrier};
        struct SlowStore {
            value: MemoryStore,
            entered: Arc<Barrier>,
            resume: Arc<Barrier>,
        }
        impl Store for SlowStore {
            fn get(&self) -> Result<Option<Zeroizing<String>>, Error> {
                self.value.get()
            }
            fn set(&self, value: &str) -> Result<(), Error> {
                self.value.set(value)?;
                self.entered.wait();
                self.resume.wait();
                Ok(())
            }
            fn delete(&self) -> Result<(), Error> {
                self.value.delete()
            }
        }
        let entered = Arc::new(Barrier::new(2));
        let resume = Arc::new(Barrier::new(2));
        let vault = Arc::new(Vault {
            store: Box::new(SlowStore {
                value: MemoryStore(Mutex::new(None)),
                entered: entered.clone(),
                resume: resume.clone(),
            }),
            serial: Mutex::new(()),
            generation: AtomicU64::new(0),
        });
        let generation = vault.invalidate();
        let writer = vault.clone();
        let thread = std::thread::spawn(move || {
            writer.save(
                generation,
                "0123456789abcdef0123456789abcdef",
                "CANCELLED_SECRET",
            )
        });
        entered.wait();
        vault.invalidate();
        resume.wait();
        assert_eq!(thread.join().unwrap(), Err(Error::Stale));
        assert!(vault.load().unwrap().is_none());
    }
}
