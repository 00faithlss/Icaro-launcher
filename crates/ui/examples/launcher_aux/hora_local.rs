// SPDX-License-Identifier: GPL-3.0-only
//! Fecha y hora locales para textos como «Jugaste el 05-11» o «Hasta mañana,
//! martes 10-11 a las 04:15». Windows da el desfase de su zona horaria; en
//! otros sistemas se usa UTC.
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Segundos que hay que sumar a UTC para obtener la hora local.
pub fn desfase() -> i64 {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Time::{GetTimeZoneInformation, TIME_ZONE_INFORMATION};
        let mut zona = TIME_ZONE_INFORMATION::default();
        // SAFETY: la función solo escribe en la estructura que le pasamos.
        let id = unsafe { GetTimeZoneInformation(&mut zona) };
        // 2 = TIME_ZONE_ID_DAYLIGHT; 0xFFFFFFFF = error.
        let extra = match id {
            2 => zona.DaylightBias,
            0xFFFF_FFFF => return 0,
            _ => zona.StandardBias,
        };
        -i64::from(zona.Bias + extra) * 60
    }
    #[cfg(not(windows))]
    {
        0
    }
}

/// Fecha civil local: año, mes, día, día de la semana (0 = domingo), hora y minuto.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Fecha {
    pub anio: i64,
    pub mes: u32,
    pub dia: u32,
    pub semana: u32,
    pub hora: u32,
    pub minuto: u32,
}

/// Convierte segundos locales desde la época en fecha civil (algoritmo de Howard Hinnant).
pub fn fecha_de(segundos_locales: i64) -> Fecha {
    let dias = segundos_locales.div_euclid(86_400);
    let resto = segundos_locales.rem_euclid(86_400);
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let dia = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let mes = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let anio = yoe + era * 400 + i64::from(mes <= 2);
    Fecha {
        anio,
        mes,
        dia,
        // 1970-01-01 fue jueves.
        semana: (dias + 4).rem_euclid(7) as u32,
        hora: (resto / 3600) as u32,
        minuto: ((resto % 3600) / 60) as u32,
    }
}

fn segundos(t: SystemTime) -> i64 {
    match t.duration_since(UNIX_EPOCH) {
        Ok(d) => d.as_secs() as i64,
        Err(e) => -(e.duration().as_secs() as i64),
    }
}

pub fn local(t: SystemTime) -> Fecha {
    fecha_de(segundos(t) + desfase())
}

const DIAS: [&str; 7] = [
    "domingo",
    "lunes",
    "martes",
    "miércoles",
    "jueves",
    "viernes",
    "sábado",
];

/// «Jugaste hace 2 h», «Jugaste ayer» o «Jugaste el 05-11».
pub fn jugaste(ultima: Option<SystemTime>, ahora: SystemTime) -> String {
    let Some(ultima) = ultima else {
        return "Sin jugar".into();
    };
    let pasado = ahora.duration_since(ultima).unwrap_or(Duration::ZERO);
    let (hoy, dia) = (local(ahora), local(ultima));
    let mismo_dia = (hoy.anio, hoy.mes, hoy.dia) == (dia.anio, dia.mes, dia.dia);
    let ayer = local(ahora - Duration::from_secs(86_400));
    if pasado < Duration::from_secs(60) {
        "Jugaste hace un momento".into()
    } else if pasado < Duration::from_secs(3600) {
        format!("Jugaste hace {} min", pasado.as_secs() / 60)
    } else if mismo_dia || pasado < Duration::from_secs(6 * 3600) {
        format!("Jugaste hace {} h", pasado.as_secs() / 3600)
    } else if (ayer.anio, ayer.mes, ayer.dia) == (dia.anio, dia.mes, dia.dia) {
        "Jugaste ayer".into()
    } else {
        format!("Jugaste el {:02}-{:02}", dia.dia, dia.mes)
    }
}

/// «Hasta mañana, martes 10-11 a las 04:15» o «Hasta el viernes 13-11 a las 04:15».
pub fn hasta(vence: SystemTime, ahora: SystemTime) -> String {
    let (v, hoy) = (local(vence), local(ahora));
    let manana = local(ahora + Duration::from_secs(86_400));
    let cuando = format!(
        "{} {:02}-{:02} a las {:02}:{:02}",
        DIAS[v.semana as usize], v.dia, v.mes, v.hora, v.minuto
    );
    if (v.anio, v.mes, v.dia) == (hoy.anio, hoy.mes, hoy.dia) {
        format!("Hasta hoy a las {:02}:{:02}", v.hora, v.minuto)
    } else if (v.anio, v.mes, v.dia) == (manana.anio, manana.mes, manana.dia) {
        format!("Hasta mañana, {cuando}")
    } else {
        format!("Hasta el {cuando}")
    }
}

/// «el martes 10-11 a las 04:15», para el mensaje con instrucciones.
pub fn dia_y_hora(t: SystemTime) -> String {
    let f = local(t);
    format!(
        "el {} {:02}-{:02} a las {:02}:{:02}",
        DIAS[f.semana as usize], f.dia, f.mes, f.hora, f.minuto
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_fecha_civil_coincide_con_dias_conocidos() {
        // 2026-11-09 04:15 UTC, lunes.
        let f = fecha_de(1_794_197_700);
        assert_eq!(
            (f.anio, f.mes, f.dia, f.semana, f.hora, f.minuto),
            (2026, 11, 9, 1, 4, 15)
        );
        assert_eq!(fecha_de(0).semana, 4);
        assert_eq!(
            (fecha_de(951_782_400).mes, fecha_de(951_782_400).dia),
            (2, 29)
        );
    }

    #[test]
    fn jugaste_usa_minutos_horas_ayer_y_fecha() {
        let ahora = UNIX_EPOCH + Duration::from_secs(1_794_197_700);
        assert_eq!(jugaste(None, ahora), "Sin jugar");
        assert_eq!(
            jugaste(Some(ahora - Duration::from_secs(600)), ahora),
            "Jugaste hace 10 min"
        );
        assert_eq!(
            jugaste(Some(ahora - Duration::from_secs(7200)), ahora),
            "Jugaste hace 2 h"
        );
        let viejo = jugaste(Some(ahora - Duration::from_secs(9 * 86_400)), ahora);
        assert!(viejo.starts_with("Jugaste el "), "{viejo}");
    }
}
