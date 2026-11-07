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
    pub fn desde_almacen(almacen: &impl AlmacenClave) -> Result<Self, Error> {
        let mut semilla = Zeroizing::new([0u8; 32]);
        match almacen.leer()? {
            Some(valor) => {
                if valor.len() != 32 {
                    return Err(Error::Credencial);
                }
                semilla.copy_from_slice(&valor);
            }
            None => {
                getrandom::fill(semilla.as_mut()).map_err(|_| Error::Credencial)?;
                almacen.guardar(semilla.as_ref())?;
            }
        }
        Ok(Self {
            clave: iroh::SecretKey::from_bytes(&semilla),
        })
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
        let clave = iroh::PublicKey::from_bytes(&decodificar_hex::<32>(publica)?)
            .map_err(|_| Error::Datos("clave pública"))?;
        let firma = iroh::Signature::from_bytes(&decodificar_hex::<64>(firma)?);
        clave
            .verify(mensaje, &firma)
            .map_err(|_| Error::Datos("firma o identidad incorrecta"))
    }
    pub fn huella(&self) -> String {
        let hash = Sha256::digest(self.clave.public().as_bytes());
        hash[..12]
            .chunks(2)
            .map(|parte| parte.iter().map(|b| format!("{b:02X}")).collect::<String>())
            .collect::<Vec<_>>()
            .join(" ")
    }
    pub fn sello(&self) -> [[bool; 8]; 8] {
        let hash = Sha256::digest(self.clave.public().as_bytes());
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
