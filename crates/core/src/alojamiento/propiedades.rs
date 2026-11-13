// SPDX-License-Identifier: GPL-3.0-only
//! `server.properties` editable sin perder comentarios, orden ni claves que
//! Ícaro no conoce. Minecraft lo escribe en ISO-8859-1 con escapes `\uXXXX`;
//! aquí se lee y escribe con esos escapes para conservar acentos del MOTD.
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
enum Linea {
    Otra(String),
    Valor(String, String),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Propiedades {
    lineas: Vec<Linea>,
}

/// Valores con los que Ícaro crea un servidor nuevo. `server-ip` queda en
/// loopback: solo se entra por el túnel de Ícaro, no por la red local.
pub const INICIALES: &[(&str, &str)] = &[
    ("motd", "Servidor de Ícaro"),
    ("server-ip", "127.0.0.1"),
    ("online-mode", "true"),
    ("max-players", "10"),
    ("difficulty", "normal"),
    ("gamemode", "survival"),
    ("pvp", "true"),
    ("white-list", "false"),
    ("view-distance", "10"),
    ("simulation-distance", "10"),
    ("spawn-protection", "0"),
    ("allow-flight", "false"),
    ("hardcore", "false"),
    ("level-seed", ""),
    ("enable-command-block", "false"),
];

impl Propiedades {
    pub fn leer(texto: &str) -> Self {
        let mut lineas = vec![];
        let mut fisicas = texto.lines();
        while let Some(primera) = fisicas.next() {
            let mut logica = primera.to_owned();
            // Una barra final impar continúa en la siguiente línea.
            while continua(&logica) {
                logica.pop();
                match fisicas.next() {
                    Some(s) => logica.push_str(s.trim_start()),
                    None => break,
                }
            }
            let recortada = logica.trim_start();
            if recortada.is_empty() || recortada.starts_with('#') || recortada.starts_with('!') {
                lineas.push(Linea::Otra(logica));
                continue;
            }
            let (clave, valor) = separar(recortada);
            lineas.push(Linea::Valor(desescapar(&clave), desescapar(&valor)));
        }
        Self { lineas }
    }

    pub fn nuevas(puerto: u16) -> Self {
        let mut p = Self::default();
        p.lineas
            .push(Linea::Otra("#Minecraft server properties".into()));
        for (k, v) in INICIALES {
            p.fijar(k, v);
        }
        p.fijar("server-port", &puerto.to_string());
        p
    }

    pub fn valor(&self, clave: &str) -> Option<&str> {
        self.lineas.iter().rev().find_map(|l| match l {
            Linea::Valor(k, v) if k == clave => Some(v.as_str()),
            _ => None,
        })
    }

    pub fn bool(&self, clave: &str) -> bool {
        self.valor(clave)
            .is_some_and(|v| v.trim().eq_ignore_ascii_case("true"))
    }

    pub fn numero(&self, clave: &str) -> Option<i64> {
        self.valor(clave)?.trim().parse().ok()
    }

    /// Cambia el valor en su lugar o lo añade al final.
    pub fn fijar(&mut self, clave: &str, valor: &str) {
        let limpio: String = valor.chars().filter(|c| *c != '\n' && *c != '\r').collect();
        for l in self.lineas.iter_mut().rev() {
            if let Linea::Valor(k, v) = l {
                if k == clave {
                    *v = limpio;
                    return;
                }
            }
        }
        self.lineas.push(Linea::Valor(clave.into(), limpio));
    }

    pub fn claves(&self) -> impl Iterator<Item = (&str, &str)> {
        self.lineas.iter().filter_map(|l| match l {
            Linea::Valor(k, v) => Some((k.as_str(), v.as_str())),
            Linea::Otra(_) => None,
        })
    }

    pub fn escribir(&self) -> String {
        let mut s = String::new();
        for l in &self.lineas {
            match l {
                Linea::Otra(t) => s.push_str(t),
                Linea::Valor(k, v) => {
                    s.push_str(&escapar(k, true));
                    s.push('=');
                    s.push_str(&escapar(v, false));
                }
            }
            s.push('\n');
        }
        s
    }

    pub fn cargar(ruta: &Path) -> std::io::Result<Option<Self>> {
        match std::fs::read(ruta) {
            Ok(bytes) => Ok(Some(Self::leer(&latin1(&bytes)))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Escritura atómica: nunca deja un archivo a medias si falla el disco.
    pub fn guardar(&self, ruta: &Path) -> std::io::Result<()> {
        super::escribir_atomico(ruta, self.escribir().as_bytes())
    }
}

fn continua(linea: &str) -> bool {
    linea.chars().rev().take_while(|c| *c == '\\').count() % 2 == 1
}

fn separar(linea: &str) -> (String, String) {
    let mut clave = String::new();
    let mut chars = linea.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                clave.push('\\');
                if let Some(n) = chars.next() {
                    clave.push(n);
                }
            }
            '=' | ':' => break,
            c if c.is_whitespace() => {
                while chars.peek().is_some_and(|c| c.is_whitespace()) {
                    chars.next();
                }
                if chars.peek().is_some_and(|c| *c == '=' || *c == ':') {
                    chars.next();
                }
                break;
            }
            c => clave.push(c),
        }
    }
    let resto: String = chars.collect();
    (clave, resto.trim_start().to_owned())
}

fn desescapar(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('f') => out.push('\x0c'),
            Some('u') => {
                let hex: String = chars.by_ref().take(4).collect();
                match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                    Some(ch) => out.push(ch),
                    None => out.push_str(&hex),
                }
            }
            Some(o) => out.push(o),
            None => {}
        }
    }
    out
}

fn escapar(s: &str, clave: bool) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '=' | ':' if clave => {
                out.push('\\');
                out.push(c);
            }
            ' ' if clave || i == 0 => out.push_str("\\ "),
            '#' | '!' if i == 0 => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 || (c as u32) > 0x7e => {
                // Minecraft lee ISO-8859-1: todo lo que no es ASCII va escapado.
                let mut buf = [0u16; 2];
                for u in c.encode_utf16(&mut buf) {
                    out.push_str(&format!("\\u{:04X}", u));
                }
            }
            c => out.push(c),
        }
    }
    out
}

/// Java escribe estos archivos en ISO-8859-1; UTF-8 válido también se acepta.
fn latin1(bytes: &[u8]) -> String {
    match std::str::from_utf8(bytes) {
        Ok(s) => s.to_owned(),
        Err(_) => bytes.iter().map(|b| *b as char).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conserva_comentarios_orden_y_claves_desconocidas() {
        let texto = "#Minecraft server properties\n#Fri Nov 12\nmotd=Hola\nrcon.password=\nmi-clave = algo\nmax-players=20\n";
        let mut p = Propiedades::leer(texto);
        assert_eq!(p.valor("mi-clave"), Some("algo"));
        p.fijar("max-players", "8");
        let s = p.escribir();
        assert!(s.starts_with("#Minecraft server properties\n#Fri Nov 12\nmotd=Hola\n"));
        assert!(s.contains("mi-clave=algo\n"));
        assert!(s.contains("max-players=8\n"));
        assert_eq!(s.matches("max-players").count(), 1);
    }

    #[test]
    fn acentos_y_simbolos_sobreviven_ida_y_vuelta() {
        let mut p = Propiedades::default();
        p.fijar("motd", "Ícaro: año §anuevo\\ruta");
        let s = p.escribir();
        assert!(s.is_ascii());
        assert_eq!(
            Propiedades::leer(&s).valor("motd"),
            Some("Ícaro: año §anuevo\\ruta")
        );
    }

    #[test]
    fn un_valor_no_puede_inyectar_otra_clave() {
        let mut p = Propiedades::nuevas(25565);
        p.fijar("motd", "hola\nonline-mode=false");
        let leidas = Propiedades::leer(&p.escribir());
        assert!(leidas.bool("online-mode"));
        assert_eq!(leidas.valor("motd"), Some("holaonline-mode=false"));
    }

    #[test]
    fn nuevas_quedan_en_loopback_con_online_mode() {
        let p = Propiedades::nuevas(25570);
        assert_eq!(p.valor("server-ip"), Some("127.0.0.1"));
        assert!(p.bool("online-mode"));
        assert_eq!(p.numero("server-port"), Some(25570));
    }

    #[test]
    fn lee_continuaciones_y_separadores_de_java() {
        let p = Propiedades::leer("a : uno\\\n   dos\nb\\=c=3\n");
        assert_eq!(p.valor("a"), Some("unodos"));
        assert_eq!(p.valor("b=c"), Some("3"));
    }
}
