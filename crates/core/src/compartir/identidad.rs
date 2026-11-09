// SPDX-License-Identifier: GPL-3.0-only
//! Identidad Ed25519 independiente de cuentas Microsoft, sin persistir claves en disco.
mod sistema;
use super::{manifiesto::hexadecimal, Error};
use sha2::{Digest, Sha256};
use std::path::Path;
pub use zeroize::Zeroizing;

pub trait AlmacenClave {
    fn leer(&self) -> Result<Option<Zeroizing<Vec<u8>>>, Error>;
    fn guardar(&self, valor: &[u8]) -> Result<(), Error>;
}

#[derive(Clone)]
pub struct Identidad {
    clave: iroh::SecretKey,
}
impl std::fmt::Debug for Identidad {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Identidad")
            .field("publica", &self.clave_publica())
            .finish_non_exhaustive()
    }
}
impl Identidad {
    pub fn cargar_o_crear(raiz: &Path) -> Result<Self, Error> {
        sistema::cargar(raiz)
    }
    /// Solo lee la identidad guardada del sistema: nunca genera ni guarda una clave.
    pub fn cargar_existente(raiz: &Path) -> Result<Option<Self>, Error> {
        sistema::existente(raiz)
    }
    /// Lee una identidad ya guardada; `None` si no existe. Nunca llama a `guardar`.
    pub fn existente(almacen: &impl AlmacenClave) -> Result<Option<Self>, Error> {
        almacen.leer()?.map(|valor| Self::desde_semilla(&valor)).transpose()
    }
    fn desde_semilla(valor: &[u8]) -> Result<Self, Error> {
        if valor.len() != 32 {
            return Err(Error::Credencial);
        }
        let mut semilla = Zeroizing::new([0u8; 32]);
        semilla.copy_from_slice(valor);
        Ok(Self {
            clave: iroh::SecretKey::from_bytes(&semilla),
        })
    }
    pub fn desde_almacen(almacen: &impl AlmacenClave) -> Result<Self, Error> {
        if let Some(identidad) = Self::existente(almacen)? {
            return Ok(identidad);
        }
        let mut semilla = Zeroizing::new([0u8; 32]);
        getrandom::fill(semilla.as_mut()).map_err(|_| Error::Credencial)?;
        almacen.guardar(semilla.as_ref())?;
        Self::desde_semilla(semilla.as_ref())
    }
    pub fn clave_publica(&self) -> String {
        self.clave.public().to_string()
    }
    pub(crate) fn clave_endpoint(&self) -> iroh::SecretKey {
        self.clave.clone()
    }
    pub fn firmar(&self, mensaje: &[u8]) -> String {
        self.clave
            .sign(mensaje)
            .to_bytes()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }
    pub fn verificar(publica: &str, mensaje: &[u8], firma: &str) -> Result<(), Error> {
        let clave = publica_validada(publica)?;
        let firma = iroh::Signature::from_bytes(&decodificar_hex::<64>(firma)?);
        clave
            .verify(mensaje, &firma)
            .map_err(|_| Error::Datos("firma o identidad incorrecta"))
    }
    pub fn huella(&self) -> String {
        huella_de(self.clave.public().as_bytes())
    }
    pub fn sello(&self) -> [[bool; 8]; 8] {
        sello_de(self.clave.public().as_bytes())
    }
}

/// Clave pública Ed25519 completa: 64 caracteres hexadecimales en minúscula que
/// además deben ser un punto válido de la curva.
pub(crate) fn publica_validada(publica: &str) -> Result<iroh::PublicKey, Error> {
    iroh::PublicKey::from_bytes(&decodificar_hex::<32>(publica)?)
        .map_err(|_| Error::Datos("clave pública"))
}
/// Huella de otra identidad, con el mismo algoritmo que [`Identidad::huella`].
pub fn huella_publica(publica: &str) -> Result<String, Error> {
    Ok(huella_de(publica_validada(publica)?.as_bytes()))
}
/// Sello de otra identidad, con el mismo algoritmo que [`Identidad::sello`].
pub fn sello_publico(publica: &str) -> Result<[[bool; 8]; 8], Error> {
    Ok(sello_de(publica_validada(publica)?.as_bytes()))
}
fn huella_de(publica: &[u8]) -> String {
    let hash = Sha256::digest(publica);
    hash[..12]
        .chunks(2)
        .map(|parte| parte.iter().map(|b| format!("{b:02X}")).collect::<String>())
        .collect::<Vec<_>>()
        .join(" ")
}
fn sello_de(publica: &[u8]) -> [[bool; 8]; 8] {
    let hash = Sha256::digest(publica);
    let mut sello = [[false; 8]; 8];
    for (y, fila) in sello.iter_mut().enumerate() {
        for x in 0..4 {
            let valor = (hash[y] & (1 << x)) != 0;
            fila[x] = valor;
            fila[7 - x] = valor;
        }
    }
    sello
}
pub(crate) fn decodificar_hex<const N: usize>(valor: &str) -> Result<[u8; N], Error> {
    if !hexadecimal(valor, N * 2) {
        return Err(Error::Datos("valor hexadecimal"));
    }
    let mut bytes = [0u8; N];
    for (i, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&valor[i * 2..i * 2 + 2], 16)
            .map_err(|_| Error::Datos("valor hexadecimal"))?;
    }
    Ok(bytes)
}
