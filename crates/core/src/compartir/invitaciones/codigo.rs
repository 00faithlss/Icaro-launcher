// SPDX-License-Identifier: GPL-3.0-only
use crate::compartir::Error;

#[derive(Clone, PartialEq, Eq)]
pub struct Codigo(String);
impl std::fmt::Debug for Codigo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Codigo([oculto])")
    }
}
impl Codigo {
    pub fn parse(texto: &str) -> Result<Self, Error> {
        if texto.len() > 20 {
            return Err(Error::Datos("código de invitación"));
        }
        let mut codigo = String::new();
        for c in texto.chars() {
            let c = match c.to_ascii_uppercase() {
                ' ' | '-' => continue,
                'O' => '0',
                'I' | 'L' => '1',
                c => c,
            };
            if !"0123456789ABCDEFGHJKMNPQRSTVWXYZ".contains(c) {
                return Err(Error::Datos("código de invitación"));
            }
            codigo.push(c);
        }
        if codigo.len() != 12 {
            return Err(Error::Datos("código de invitación"));
        }
        Ok(Self(codigo))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn mostrar(&self) -> String {
        format!("{}-{}-{}", &self.0[..4], &self.0[4..8], &self.0[8..])
    }
}
