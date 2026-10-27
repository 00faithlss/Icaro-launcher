// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

use image::codecs::webp::WebPEncoder;
use image::{ColorType, ImageEncoder};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};

#[path = "src/derivadas.rs"]
mod derivadas;
#[path = "src/originales.rs"]
mod originales;
mod preparar_imagenes;

mod laminas {
    macro_rules! unica {
        () => {
            false
        };
        ($nombre:ident) => {
            true
        };
    }
    macro_rules! laminas {
        ($($variante:ident => $archivo:literal $(@ $unica:ident)?),* $(,)?) => {
            #[derive(Clone, Copy, PartialEq)]
            pub enum Lamina { $($variante),* }
            pub const TODAS: &[(Lamina, &str, bool)] = &[
                $((Lamina::$variante, $archivo, unica!($($unica)?))),*
            ];
        };
    }
    include!("src/catalogo_laminas.rs");
}

fn preparar(
    lamina: laminas::Lamina,
    nombre: &str,
    unica: bool,
    tinta: bool,
    salida: &Path,
    codigo: &str,
    cache: Option<&Path>,
) {
    use laminas::Lamina;
    let (fuente, recorte, invertir) = match lamina {
        Lamina::Faeton => ("assets/originales/faeton.webp".to_owned(), None, tinta),
        Lamina::HermesHilos => (
            "assets/originales/hermes-hilos.webp".to_owned(),
            None,
            false,
        ),
        _ => match originales::recorte(lamina) {
            Some((fuente, recorte)) => (fuente.to_owned(), Some(recorte), tinta),
            None => (
                format!(
                    "assets/laminas/{nombre}{}.webp",
                    if tinta && !unica { "-negativo" } else { "" }
                ),
                // El grabado de Apolo trae un margen de papel irregular.
                (lamina == Lamina::ApoloBelvedere).then_some([0.045, 0.018, 0.965, 0.98]),
                false,
            ),
        },
    };
    let tema = if tinta { "tinta" } else { "piedra" };
    let webp = salida.join(format!("{nombre}-{tema}.webp"));
    let rgba = salida.join(format!("{nombre}-{tema}.rgba"));
    // Una segunda copia en Tinta que solo invierte Piedra, o que la repite,
    // duplicaría el ejecutable: se deja una marca y se deriva al decodificar.
    let derivada = if !tinta {
        None
    } else if invertir {
        Some(derivadas::INVERTIDA)
    } else if unica || lamina == Lamina::HermesHilos {
        Some(derivadas::IGUAL)
    } else {
        None
    };
    if let Some(marca) = derivada {
        escribir_si_cambia(&webp, marca);
        escribir_si_cambia(&rgba, marca);
        return;
    }
    let fuente = fs::read(&fuente).expect("leer original del grabado");
    // Se compara el contenido y no la fecha: un checkout nuevo renueva todas
    // las fechas y obligaba a recodificar cada grabado en CI.
    let clave = clave_contenido(&[
        codigo.as_bytes(),
        &fuente,
        format!("{nombre}|{tema}|{unica}|{invertir}|{recorte:?}").as_bytes(),
    ]);
    let registro = salida.join(format!("{nombre}-{tema}.clave"));
    if fs::read_to_string(&registro).is_ok_and(|c| c == clave)
        && webp.is_file()
        && rgba.is_file()
    {
        return;
    }
    let en_cache = cache.map(|c| {
        (
            c.join(format!("{clave}.webp")),
            c.join(format!("{clave}.rgba")),
        )
    });
    if let Some((cache_webp, cache_rgba)) = &en_cache {
        if cache_webp.is_file() && cache_rgba.is_file() {
            fs::copy(cache_webp, &webp).unwrap();
            fs::copy(cache_rgba, &rgba).unwrap();
            fs::write(&registro, &clave).unwrap();
            return;
        }
    }
    let imagen = preparar_imagenes::preparar(&fuente, recorte, invertir, lamina == Lamina::Faeton)
        .unwrap_or_else(|e| panic!("Preparar {nombre}: {e}"));
    let dimensiones = image::io::Reader::new(std::io::Cursor::new(&fuente))
        .with_guessed_format()
        .ok()
        .and_then(|r| r.into_dimensions().ok());
    let intacta = recorte.is_none()
        && !invertir
        && lamina != Lamina::Faeton
        && dimensiones == Some(imagen.dimensions());
    if intacta && fuente.starts_with(b"RIFF") {
        // Sin recorte, inversión ni escala: el WebP original decodifica los
        // mismos píxeles y ocupa una fracción de volver a codificarlo sin pérdida.
        fs::write(&webp, &fuente).unwrap();
    } else {
        // VP8L conserva cada canal RGBA y comprime los grabados sin reducir su
        // resolución. La decodificación sigue en los trabajadores de imágenes.
        WebPEncoder::new_lossless(File::create(&webp).unwrap())
        .write_image(
            imagen.as_raw(),
            imagen.width(),
            imagen.height(),
            ColorType::Rgba8,
        )
        .expect("guardar grabado preparado");
    }
    // Píxeles pequeños listos para el primer cuadro: sin decodificar ni esperar
    // al trabajador de alta resolución y sin archivos externos a la app.
    let previa = image::DynamicImage::ImageRgba8(imagen)
        .thumbnail(320, 320)
        .into_rgba8();
    let mut archivo = File::create(&rgba).unwrap();
    archivo.write_all(&previa.width().to_le_bytes()).unwrap();
    archivo.write_all(&previa.height().to_le_bytes()).unwrap();
    archivo.write_all(previa.as_raw()).unwrap();
    drop(archivo);
    fs::write(&registro, &clave).unwrap();
    if let Some((cache_webp, cache_rgba)) = en_cache {
        publicar_en_cache(&webp, &cache_webp);
        publicar_en_cache(&rgba, &cache_rgba);
    }
}

/// Identifica entradas por contenido con 128 bits de dos SipHash con dominios
/// distintos. Solo selecciona salidas de compilación propias; no verifica datos
/// de terceros.
fn clave_contenido(partes: &[&[u8]]) -> String {
    use std::hash::Hasher;
    let resumen = |dominio: u8| {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        h.write_u8(dominio);
        for parte in partes {
            h.write_usize(parte.len());
            h.write(parte);
        }
        h.finish()
    };
    format!("{:016x}{:016x}", resumen(1), resumen(2))
}

/// Evita renovar la fecha de salidas idénticas, que forzaría a recompilar la
/// interfaz por sus `include_bytes!`.
fn escribir_si_cambia(ruta: &Path, contenido: &[u8]) {
    if fs::read(ruta).is_ok_and(|actual| actual == contenido) {
        return;
    }
    fs::write(ruta, contenido).unwrap();
}

/// Copia mediante un temporal y un renombrado: otro hilo o compilación nunca
/// lee un archivo de caché a medias. Un destino existente se conserva.
fn publicar_en_cache(origen: &Path, destino: &Path) {
    if destino.is_file() {
        return;
    }
    let temporal = destino.with_extension(format!("tmp-{}", std::process::id()));
    fs::copy(origen, &temporal).unwrap();
    if fs::rename(&temporal, destino).is_err() {
        let _ = fs::remove_file(&temporal);
    }
}

fn main() {
    println!("cargo:rerun-if-env-changed=ICARO_COMPILACION");
    let compilacion = std::env::var("ICARO_COMPILACION").unwrap_or_else(|_| "0".into());
    compilacion.parse::<u64>().expect("Número de compilación de Ícaro");
    println!("cargo:rustc-env=ICARO_COMPILACION={compilacion}");
    // Canal de publicación: el workflow compila `develop` como beta.
    println!("cargo:rerun-if-env-changed=ICARO_CANAL");
    let canal = std::env::var("ICARO_CANAL").unwrap_or_else(|_| "estable".into());
    assert!(matches!(canal.as_str(), "estable" | "beta"), "Canal de Ícaro desconocido");
    println!("cargo:rustc-env=ICARO_CANAL={canal}");
    let fuentes = [
        "build.rs",
        "preparar_imagenes.rs",
        "src/catalogo_laminas.rs",
        "src/derivadas.rs",
        "src/originales.rs",
    ];
    // El lockfile fija la versión del codificador: otra versión puede producir
    // bytes distintos con las mismas entradas.
    let mut entradas = Vec::new();
    for fuente in fuentes.iter().chain(&["../../Cargo.lock"]) {
        println!("cargo:rerun-if-changed={fuente}");
        entradas.push(fs::read(fuente).unwrap());
    }
    let entradas: Vec<&[u8]> = entradas.iter().map(Vec::as_slice).collect();
    let codigo = clave_contenido(&entradas);
    println!("cargo:rerun-if-changed=assets/laminas");
    println!("cargo:rerun-if-changed=assets/originales");
    let salida = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("imagenes");
    fs::create_dir_all(&salida).unwrap();
    // Carpeta opcional compartida entre compilaciones (el CI la conserva con
    // actions/cache). Sin ella, cada OUT_DIR prepara sus propias imágenes.
    println!("cargo:rerun-if-env-changed=ICARO_CACHE_IMAGENES");
    let cache = std::env::var_os("ICARO_CACHE_IMAGENES").map(std::path::PathBuf::from);
    if let Some(cache) = &cache {
        fs::create_dir_all(cache).unwrap();
    }
    let siguiente = AtomicUsize::new(0);
    let hilos = std::thread::available_parallelism().map_or(2, |n| n.get().min(4));
    std::thread::scope(|scope| {
        for _ in 0..hilos {
            scope.spawn(|| loop {
                let indice = siguiente.fetch_add(1, Ordering::Relaxed);
                let Some(&(lamina, nombre, unica)) = laminas::TODAS.get(indice) else {
                    break;
                };
                for tinta in [false, true] {
                    preparar(lamina, nombre, unica, tinta, &salida, &codigo, cache.as_deref());
                }
            });
        }
    });
    println!("cargo:rerun-if-changed=assets/icono/icaro.ico");
    println!("cargo:rerun-if-changed=assets/icono/icaro.rc");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let version = std::env::var("CARGO_PKG_VERSION").unwrap();
        let mut partes: Vec<_> = version
            .split(['.', '-', '+'])
            .take(3)
            .map(|n| {
                n.parse::<u16>()
                    .expect("Versión numérica de Windows")
                    .to_string()
            })
            .collect();
        partes.push("0".into());
        let icono =
            Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("assets/icono/icaro.ico");
        let codigo = fs::read_to_string("assets/icono/icaro.rc")
            .unwrap()
            .replace("@ICARO_VERSION@", &version)
            .replace("@ICARO_NUMERIC@", &partes.join(","))
            .replace("@ICARO_ICON@", &icono.to_string_lossy().replace('\\', "/"));
        let recurso =
            std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("icaro.rc");
        fs::write(&recurso, codigo).unwrap();
        embed_resource::compile_for_examples(&recurso, embed_resource::NONE)
            .manifest_optional()
            .expect("Compilar icono y versión de Windows");
    }
}
