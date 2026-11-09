// SPDX-License-Identifier: GPL-3.0-only
//! Frase de bienvenida de la banda de Instancias: cambia en cada inicio y,
//! cuando toca saludar, usa la hora local y el nombre de la cuenta.
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

/// Frases sin saludo horario; `{n}` se reemplaza por el nombre si hay cuenta.
const FRASES: [&str; 12] = [
    "¿Ganas de construir?",
    "¿Qué construimos hoy?",
    "El mundo te espera",
    "¿Una partida rápida?",
    "Hora de minar",
    "¿Seguimos donde lo dejaste?",
    "Que no se apague la antorcha",
    "Los creepers te extrañaban",
    "Qué bueno verte, {n}",
    "¿A explorar, {n}?",
    "Hoy toca aventura",
    "Tus mundos siguen ahí",
];

/// Semilla fija durante esta ejecución: la frase no cambia al redibujar.
fn semilla() -> u64 {
    static SEMILLA: OnceLock<u64> = OnceLock::new();
    *SEMILLA.get_or_init(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64)
    })
}

fn saludo_horario(hora: u32) -> &'static str {
    match hora {
        6..=11 => "Buenos días",
        12..=19 => "Buenas tardes",
        _ => "Buenas noches",
    }
}

/// Frase para una semilla, hora local (0–23) y nombre opcional. Un tercio de
/// las veces es el saludo horario; el resto, una frase de la lista.
pub fn frase_con(semilla: u64, hora: u32, nombre: Option<&str>) -> String {
    let nombre = nombre.map(str::trim).filter(|n| !n.is_empty());
    if semilla % 3 == 0 {
        return match nombre {
            Some(n) => format!("{}, {n}", saludo_horario(hora)),
            None => saludo_horario(hora).to_owned(),
        };
    }
    let candidatas: Vec<&str> = FRASES
        .iter()
        .copied()
        .filter(|f| nombre.is_some() || !f.contains("{n}"))
        .collect();
    let f = candidatas[(semilla / 3) as usize % candidatas.len()];
    match nombre {
        Some(n) => f.replace("{n}", n),
        None => f.to_owned(),
    }
}

/// Frase de esta ejecución con la hora actual y el nombre de la cuenta.
pub fn frase(nombre: Option<&str>) -> String {
    let hora = crate::hora_local::local(SystemTime::now()).hora;
    frase_con(semilla(), hora, nombre)
}

#[cfg(test)]
mod tests {
    use super::frase_con;

    #[test]
    fn el_saludo_horario_usa_la_hora_y_el_nombre() {
        assert_eq!(frase_con(0, 8, Some("Mineral_7")), "Buenos días, Mineral_7");
        assert_eq!(frase_con(3, 15, None), "Buenas tardes");
        assert_eq!(
            frase_con(6, 23, Some("Mineral_7")),
            "Buenas noches, Mineral_7"
        );
        assert_eq!(frase_con(9, 3, None), "Buenas noches");
    }

    #[test]
    fn sin_cuenta_no_quedan_marcas_de_nombre() {
        for s in 0..300 {
            let f = frase_con(s, 12, None);
            assert!(!f.contains("{n}") && !f.trim().is_empty(), "{f}");
        }
    }

    #[test]
    fn la_frase_cambia_con_la_semilla() {
        let distintas: std::collections::BTreeSet<_> =
            (0..60).map(|s| frase_con(s, 12, Some("Ana"))).collect();
        assert!(distintas.len() > 8);
    }
}
