// SPDX-License-Identifier: GPL-3.0-only
//! SaludoInicio: la frase de Instancias. Las frases vienen en el código (no se
//! configuran): se filtran por momento del día y, tras 7 días o más sin jugar,
//! se usa una de regreso. Cambia solo al abrir Ícaro.
use crate::{hora_local, App, Mensaje};
use icaro_ui::saludo::{DatosSaludo, Trozo};
use iced::Task;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const MANANA: &[&str] = &[
    "Buenos días, {nickname}.",
    "Arriba, {nickname}. El sol ya salió en el Overworld.",
    "¿Café y un poco de minería, {nickname}?",
    "Buen día para empezar un mundo nuevo.",
    "Temprano y con antorchas de sobra.",
    "Los aldeanos ya abrieron sus puestos.",
    "La mañana es de los que madrugan a minar.",
    "{instancia} amaneció tranquila.",
    "Primero, revisar las granjas.",
    "Mañana de redstone y café, {nickname}.",
    "Buen momento para bajar al Nether.",
];
const TARDE: &[&str] = &[
    "Buenas tardes, {nickname}.",
    "¿Una vuelta rápida antes de la noche, {nickname}?",
    "{instancia} sigue donde la dejaste.",
    "¿Qué construimos hoy, {nickname}?",
    "Buena tarde para explorar.",
    "Hora de minar, {nickname}.",
    "Todavía queda luz para terminar la granja.",
    "¿Seguimos donde lo dejaste?",
    "Tarde perfecta para ir al End.",
    "Buena tarde para ampliar la granja.",
    "¿Un viaje rápido al Nether, {nickname}?",
];
const NOCHE: &[&str] = &[
    "Buenas noches, {nickname}.",
    "Es de noche: los zombis también juegan, {nickname}.",
    "Última partida y a dormir, {nickname}.",
    "Que no se apague la antorcha.",
    "Los creepers salen de noche. Tú también, {nickname}.",
    "Una cama cerca y a seguir construyendo.",
    "Noche tranquila en {instancia}.",
    "¿Una más antes de dormir, {nickname}?",
    "En el Nether no hay noche.",
    "De noche, la granja de mobs rinde más.",
    "Noche ideal para tu circuito de redstone.",
];
const CUALQUIERA: &[&str] = &[
    "¿Ganas de construir, {nickname}?",
    "El mundo te espera.",
    "¿Una partida rápida?",
    "Qué bueno verte, {nickname}.",
    "¿A explorar, {nickname}?",
    "Hoy toca aventura.",
    "Tus mundos siguen ahí.",
    "Los creepers te extrañaban.",
    // Nether
    "El Nether no se va a explorar solo.",
    "¿Portal al Nether, {nickname}?",
    "Trae oro: los piglins quieren negociar.",
    "Cuidado con los ghasts, {nickname}.",
    "La netherita no se mina sola.",
    "Las fortalezas del Nether guardan blazes.",
    // End
    "El dragón del End sigue esperando.",
    "¿Listo para el End, {nickname}?",
    "Las ciudades del End guardan elytras.",
    "Ojos de ender en mano y a buscar la fortaleza.",
    "Mejor no mirar a los endermen.",
    // Granjas técnicas y redstone
    "La granja de hierro no se arma sola.",
    "¿Terminamos la granja de mobs, {nickname}?",
    "Un poco de redstone nunca sobra.",
    "¿Otro circuito de redstone, {nickname}?",
    "Las tolvas no se llenan solas.",
    "Hoy toca optimizar la granja.",
    "Los aldeanos tienen buenas ofertas hoy.",
    // Profundidades
    "Sin ruido: el Warden escucha.",
    "Las ciudades antiguas guardan secretos.",
];
const REGRESO: &[&str] = &[
    "Cuánto tiempo, {nickname}.",
    "Volviste, {nickname}. Tus mundos te esperaban.",
    "Qué bueno tenerte de vuelta, {nickname}.",
    "{instancia} te extrañaba.",
    "Tus granjas te esperan, {nickname}.",
    "El Nether te extrañaba, {nickname}.",
];

/// Días sin jugar a partir de los que se usa una frase de regreso.
const DIAS_REGRESO: u64 = 7;

#[derive(Default)]
pub struct Estado {
    /// Última partida registrada: id de la instancia y fin en segundos Unix.
    pub ultima: Option<(String, u64)>,
}

#[derive(Debug, Clone)]
pub enum Accion {
    Ultima(Option<(String, u64)>),
}

/// Lo que la frase sabe del momento: hora local, nombre de la cuenta,
/// instancia de referencia y días sin jugar.
#[derive(Debug, Clone, Copy, Default)]
pub struct Contexto<'a> {
    pub hora: u32,
    pub nombre: Option<&'a str>,
    pub instancia: Option<&'a str>,
    pub ausencia: Option<Duration>,
}

fn regreso(ctx: &Contexto) -> bool {
    ctx.ausencia
        .is_some_and(|d| d.as_secs() >= DIAS_REGRESO * 86_400)
}

/// Frases posibles ahora: mañana 6–12, tarde 12–20, noche 20–6, más las de
/// cualquier hora; sin instancia conocida se descartan las que la nombran.
fn candidatas(ctx: &Contexto) -> Vec<&'static str> {
    let listas: &[&[&str]] = if regreso(ctx) {
        &[REGRESO]
    } else {
        match ctx.hora {
            6..=11 => &[MANANA, CUALQUIERA],
            12..=19 => &[TARDE, CUALQUIERA],
            _ => &[NOCHE, CUALQUIERA],
        }
    };
    listas
        .iter()
        .flat_map(|l| l.iter().copied())
        .filter(|f| ctx.instancia.is_some() || !f.contains("{instancia}"))
        .collect()
}

/// Rellena las variables. Sin cuenta, el nombre se quita con su coma.
fn trozos(frase: &str, ctx: &Contexto) -> Vec<Trozo> {
    let mut f = frase.replace("{instancia}", ctx.instancia.unwrap_or_default());
    let Some(nombre) = ctx.nombre else {
        f = f.replace(", {nickname}", "").replace("{nickname}, ", "");
        return vec![Trozo::Texto(f)];
    };
    let mut salida = Vec::new();
    let mut partes = f.split("{nickname}").peekable();
    while let Some(parte) = partes.next() {
        if !parte.is_empty() {
            salida.push(Trozo::Texto(parte.to_owned()));
        }
        if partes.peek().is_some() {
            salida.push(Trozo::Nombre(nombre.to_owned()));
        }
    }
    salida
}

/// Frase para una semilla (fija durante cada ejecución de Ícaro).
pub fn frase_con(semilla: u64, ctx: &Contexto) -> Vec<Trozo> {
    let lista = candidatas(ctx);
    trozos(lista[semilla as usize % lista.len()], ctx)
}

/// Segunda línea: «Supervivencia te espera. Jugaste hace 2 h.» o, tras días
/// sin jugar, «Hace 12 días que no abrías Supervivencia.». Si la frase ya
/// nombra la instancia (`nombrada`), no se repite: «Jugaste hace 2 h.».
pub fn sub(
    instancia: Option<&str>,
    ultima: Option<SystemTime>,
    ahora: SystemTime,
    nombrada: bool,
) -> Option<String> {
    let instancia = instancia?;
    let Some(ultima) = ultima else {
        return (!nombrada).then(|| format!("{instancia} te espera."));
    };
    let pasado = ahora.duration_since(ultima).unwrap_or(Duration::ZERO);
    let dias = pasado.as_secs() / 86_400;
    if dias >= DIAS_REGRESO {
        return Some(if nombrada {
            format!("Hace {dias} días que no jugabas.")
        } else {
            format!("Hace {dias} días que no abrías {instancia}.")
        });
    }
    let mut cuando = hora_local::jugaste(Some(ultima), ahora);
    if cuando == "Jugaste ayer" {
        let f = hora_local::local(ultima);
        cuando = format!("Jugaste ayer a las {:02}:{:02}", f.hora, f.minuto);
    }
    Some(if nombrada {
        format!("{cuando}.")
    } else {
        format!("{instancia} te espera. {cuando}.")
    })
}

/// Semilla fija durante esta ejecución: la frase no cambia al redibujar.
fn semilla() -> u64 {
    static SEMILLA: OnceLock<u64> = OnceLock::new();
    *SEMILLA.get_or_init(|| {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as u64 / 7)
    })
}

/// Datos del saludo para la vista.
pub fn datos(app: &App) -> DatosSaludo {
    let nombre = app.servicios.nombre(app.sesion.estado.nombre());
    let nombre = (!["Sin cuenta", "Invitado", "Recuperando sesión…", ""].contains(&nombre))
        .then_some(nombre);
    let ahora = SystemTime::now();
    // La instancia de la última partida; si no hay, la seleccionada.
    let (instancia, ultima) = match &app.saludo.ultima {
        Some((id, fin)) => (
            app.servicios
                .instancias
                .registros
                .iter()
                .find(|r| &r.id == id)
                .map(|r| r.nombre.clone()),
            Some(UNIX_EPOCH + Duration::from_secs(*fin)),
        ),
        None => (None, None),
    };
    let (instancia, ultima) = match instancia {
        Some(n) => (Some(n), ultima),
        None => (
            app.seleccionada
                .or(app.servicios.demo.then_some(0))
                .and_then(|i| crate::con_instancias(|l| l.get(i).map(|d| d.nombre.clone()))),
            // La muestra simula una partida de hace 2 h.
            app.servicios
                .demo
                .then(|| ahora - Duration::from_secs(2 * 3600)),
        ),
    };
    let ctx = Contexto {
        hora: hora_local::local(ahora).hora,
        nombre,
        instancia: instancia.as_deref(),
        ausencia: ultima.and_then(|u| ahora.duration_since(u).ok()),
    };
    let frase = frase_con(semilla(), &ctx);
    let nombrada = instancia
        .as_deref()
        .is_some_and(|n| frase.iter().any(|t| matches!(t, Trozo::Texto(s) if s.contains(n))));
    DatosSaludo {
        sub: sub(instancia.as_deref(), ultima, ahora, nombrada),
        frase,
    }
}

/// Lee la última partida de cada instancia fuera del hilo de la interfaz.
pub fn cargar(app: &App) -> Task<Mensaje> {
    if app.servicios.demo {
        return Task::none();
    }
    let almacen = app.servicios.instancias.almacen.clone();
    let ids: Vec<String> = app
        .servicios
        .instancias
        .registros
        .iter()
        .map(|r| r.id.clone())
        .collect();
    Task::perform(
        async move {
            tokio::task::spawn_blocking(move || {
                ids.into_iter()
                    .filter_map(|id| Some((almacen.ultima_partida(&id).ok()??, id)))
                    .max()
                    .map(|(fin, id)| (id, fin))
            })
            .await
            .unwrap_or_default()
        },
        |u| Mensaje::Saludo(Accion::Ultima(u)),
    )
}

/// Una partida acaba de terminar: pasa a ser la última.
pub fn jugada(app: &mut App, id: &str) {
    let fin = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    app.saludo.ultima = Some((id.to_owned(), fin));
}

pub fn actualizar(app: &mut App, accion: Accion) -> Task<Mensaje> {
    match accion {
        Accion::Ultima(u) => {
            // Una partida terminada mientras se leía es más reciente.
            if app.saludo.ultima.is_none() {
                app.saludo.ultima = u;
            }
        }
    }
    Task::none()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texto(t: &[Trozo]) -> String {
        t.iter()
            .map(|t| match t {
                Trozo::Texto(s) | Trozo::Nombre(s) => s.as_str(),
            })
            .collect()
    }

    fn ctx(hora: u32, nombre: Option<&'static str>) -> Contexto<'static> {
        Contexto {
            hora,
            nombre,
            instancia: Some("Supervivencia"),
            ausencia: None,
        }
    }

    #[test]
    fn hay_67_frases_y_respetan_la_hora() {
        assert_eq!(
            MANANA.len() + TARDE.len() + NOCHE.len() + CUALQUIERA.len() + REGRESO.len(),
            67
        );
        for s in 0..40 {
            let manana = texto(&frase_con(s, &ctx(8, Some("Ana"))));
            assert!(!manana.contains("noches") && !manana.contains("tardes"), "{manana}");
            let noche = texto(&frase_con(s, &ctx(23, Some("Ana"))));
            assert!(!noche.contains("días") && !noche.contains("tardes"), "{noche}");
        }
    }

    #[test]
    fn el_nombre_va_aparte_y_sin_cuenta_se_quita_con_su_coma() {
        let t = frase_con(0, &ctx(8, Some("Mineral_7")));
        assert_eq!(texto(&t), "Buenos días, Mineral_7.");
        assert!(t.contains(&Trozo::Nombre("Mineral_7".into())));
        for s in 0..40 {
            let f = texto(&frase_con(s, &ctx(15, None)));
            assert!(!f.contains('{') && !f.contains(", .") && !f.contains(" ,"), "{f}");
        }
    }

    #[test]
    fn cada_inicio_puede_traer_una_frase_distinta() {
        let c = ctx(15, Some("Ana"));
        let distintas: std::collections::BTreeSet<_> =
            (0..40).map(|s| texto(&frase_con(s, &c))).collect();
        assert!(distintas.len() > 8);
    }

    #[test]
    fn tras_una_semana_sin_jugar_se_usa_una_de_regreso() {
        let mut c = ctx(15, Some("Ana"));
        c.ausencia = Some(Duration::from_secs(12 * 86_400));
        for s in 0..8 {
            let f = texto(&frase_con(s, &c));
            assert!(REGRESO.iter().any(|r| texto(&trozos(r, &c)) == f), "{f}");
        }
        let ahora = UNIX_EPOCH + Duration::from_secs(1_794_197_700);
        let hace = |s| Some(ahora - Duration::from_secs(s));
        assert_eq!(
            sub(Some("Supervivencia"), hace(12 * 86_400), ahora, false).unwrap(),
            "Hace 12 días que no abrías Supervivencia."
        );
        assert_eq!(
            sub(Some("Supervivencia"), hace(20 * 60), ahora, false).unwrap(),
            "Supervivencia te espera. Jugaste hace 20 min."
        );
        assert_eq!(
            sub(Some("Supervivencia"), None, ahora, false).unwrap(),
            "Supervivencia te espera."
        );
        assert_eq!(sub(None, None, ahora, false), None);
    }

    #[test]
    fn si_la_frase_nombra_la_instancia_la_segunda_linea_no_la_repite() {
        let ahora = UNIX_EPOCH + Duration::from_secs(1_794_197_700);
        let hace = |s| Some(ahora - Duration::from_secs(s));
        assert_eq!(
            sub(Some("Supervivencia"), hace(2 * 3600), ahora, true).unwrap(),
            "Jugaste hace 2 h."
        );
        assert_eq!(
            sub(Some("Supervivencia"), hace(12 * 86_400), ahora, true).unwrap(),
            "Hace 12 días que no jugabas."
        );
        assert_eq!(sub(Some("Supervivencia"), None, ahora, true), None);
    }

    #[test]
    fn sin_instancia_no_quedan_variables() {
        let c = Contexto {
            hora: 22,
            nombre: Some("Ana"),
            instancia: None,
            ausencia: None,
        };
        for s in 0..40 {
            assert!(!texto(&frase_con(s, &c)).contains('{'));
        }
    }
}
