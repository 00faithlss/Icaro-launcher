// SPDX-License-Identifier: GPL-3.0-only
use super::EncuentroFirmado;
use serde::Deserialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Caducidad {
    Horas24,
    Dias2,
    Semana,
}
impl Caducidad {
    pub fn horas(self) -> u32 {
        match self {
            Self::Horas24 => 24,
            Self::Dias2 => 48,
            Self::Semana => 168,
        }
    }
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvitacionCreada {
    pub code: String,
    pub expires_at: u64,
    pub capacity: u32,
    pub accepted: u32,
}
impl std::fmt::Debug for InvitacionCreada {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InvitacionCreada")
            .field("code", &"[oculto]")
            .field("capacity", &self.capacity)
            .finish_non_exhaustive()
    }
}
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InvitacionResuelta {
    pub host: String,
    pub publication: String,
    pub meeting: EncuentroFirmado,
    pub expires_at: u64,
    pub capacity: u32,
    pub accepted: u32,
    pub authorized: bool,
}
/// Encuentro vigente de una publicación ya aceptada, obtenido sin código.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reencuentro {
    pub host: String,
    pub publication: String,
    pub meeting: EncuentroFirmado,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PaginaAccesos {
    pub publication: String,
    pub active: bool,
    pub guests: Vec<String>,
    pub next: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Confirmacion {
    pub ok: bool,
}
