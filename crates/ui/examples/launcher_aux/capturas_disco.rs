// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Lee exclusivamente los originales de las carpetas de captura de instancias.

use icaro_core::instancias::{Almacen, Instancia};
use icaro_ui::{
    capturas::{Captura, OriginalCaptura},
    laminas::Lamina,
};
use iced::widget::image::Handle;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::SystemTime,
};

/// Lado mayor de las miniaturas de la cuadrícula. Basta para las columnas
/// más anchas sin ocupar en memoria lo que ocupa el original.
const MINIATURA: (u32, u32) = (480, 270);
/// Decodificaciones simultáneas: cada PNG grande ocupa decenas de MB.
const HILOS_MINIATURAS: usize = 4;

/// Identifica una versión concreta del original: ruta, tamaño y fecha.
pub type Clave = (PathBuf, u64, Option<SystemTime>);

fn enlace(m: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        m.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        m.file_type().is_symlink()
    }
}

pub fn carpeta(store: &Almacen, id: &str) -> Result<PathBuf, String> {
    let carpeta = store
        .juego(id)
        .map_err(|e| e.to_string())?
        .join("screenshots");
    match fs::symlink_metadata(&carpeta) {
        Ok(m) if m.is_dir() && !enlace(&m) => Ok(carpeta),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(carpeta),
        _ => Err("La carpeta de capturas no es una carpeta local válida.".into()),
    }
}

pub fn original_valido(path: &Path) -> Result<(), String> {
    let parent = path.parent().ok_or("No se pudo localizar la captura.")?;
    let folder =
        fs::symlink_metadata(parent).map_err(|_| "No se pudo leer la carpeta de capturas.")?;
    let file = fs::symlink_metadata(path).map_err(|_| "La captura ya no está en su carpeta.")?;
    if !folder.is_dir() || enlace(&folder) || !file.is_file() || enlace(&file) {
        return Err("La captura no es un archivo local válido.".into());
    }
    Ok(())
}

fn fecha(nombre: &str) -> String {
    let s = nombre.as_bytes();
    if s.len() >= 19
        && [4, 7, 10, 13, 16]
            .into_iter()
            .zip([b'-', b'-', b'_', b'.', b'.'])
            .all(|(i, c)| s[i] == c)
        && s[..19]
            .iter()
            .enumerate()
            .all(|(i, c)| [4, 7, 10, 13, 16].contains(&i) || c.is_ascii_digit())
    {
        format!(
            "{}-{}-{} {}:{}",
            &nombre[8..10],
            &nombre[5..7],
            &nombre[..4],
            &nombre[11..13],
            &nombre[14..16]
        )
    } else {
        "Fecha no disponible".into()
    }
}

pub fn listar(store: &Almacen, instancias: &[Instancia]) -> Result<Vec<Captura>, String> {
    let mut capturas = Vec::new();
    for instancia in instancias {
        let carpeta = carpeta(store, &instancia.id)?;
        let entradas = match fs::read_dir(&carpeta) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => {
                return Err(format!(
                    "No se pudieron leer las capturas de {}.",
                    instancia.nombre
                ));
            }
        };
        for entrada in entradas {
            let entrada = entrada.map_err(|_| "No se pudo leer una captura.")?;
            let ruta = entrada.path();
            if !ruta
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("png"))
            {
                continue;
            }
            let m = fs::symlink_metadata(&ruta).map_err(|_| "No se pudo leer una captura.")?;
            if !m.is_file() || enlace(&m) || m.len() == 0 {
                continue;
            }
            // La imagen se conserva en su resolución original. No se vuelve a
            // codificar ni se usa una miniatura ampliada para el visor.
            let archivo = entrada.file_name().to_string_lossy().into_owned();
            capturas.push(Captura {
                fecha: fecha(&archivo),
                archivo,
                instancia: instancia.nombre.clone(),
                bytes: m.len(),
                imagen: Lamina::Umbral,
                original: Some(OriginalCaptura {
                    imagen: Handle::from_path(&ruta),
                    miniatura: None,
                    ruta,
                    instancia_id: instancia.id.clone(),
                    modificada: m.modified().ok(),
                }),
            });
        }
    }
    capturas.sort_by(|a, b| {
        b.original
            .as_ref()
            .and_then(|o| o.modificada)
            .cmp(&a.original.as_ref().and_then(|o| o.modificada))
            .then_with(|| b.archivo.cmp(&a.archivo))
            .then_with(|| a.instancia.cmp(&b.instancia))
    });
    Ok(capturas)
}

fn decodificar(path: &Path) -> Result<image::DynamicImage, String> {
    original_valido(path)?;
    let mut reader = image::io::Reader::open(path)
        .map_err(|_| "No se pudo abrir la captura.")?
        .with_guessed_format()
        .map_err(|_| "No se pudo reconocer la captura.")?;
    let mut limits = image::io::Limits::default();
    limits.max_image_width = Some(16_384);
    limits.max_image_height = Some(16_384);
    limits.max_alloc = Some(256 * 1024 * 1024);
    reader.limits(limits);
    reader
        .decode()
        .map_err(|_| "No se pudo leer la imagen de la captura.".into())
}

/// Miniatura RGBA del original, sin modificar el archivo.
pub fn miniatura(path: &Path) -> Result<Handle, String> {
    let reducida = decodificar(path)?
        .thumbnail(MINIATURA.0, MINIATURA.1)
        .into_rgba8();
    let (ancho, alto) = reducida.dimensions();
    Ok(Handle::from_rgba(ancho, alto, reducida.into_raw()))
}

/// Genera las miniaturas en orden con concurrencia acotada y entrega cada
/// una en cuanto está lista. Se detiene si nadie espera los resultados.
pub fn miniaturas(
    claves: Vec<Clave>,
    salida: tokio::sync::mpsc::Sender<(Clave, Result<Handle, String>)>,
) {
    let siguiente = AtomicUsize::new(0);
    let hilos = std::thread::available_parallelism()
        .map_or(1, |n| n.get())
        .clamp(1, HILOS_MINIATURAS)
        .min(claves.len());
    std::thread::scope(|s| {
        for _ in 0..hilos {
            s.spawn(|| {
                while let Some(clave) = claves.get(siguiente.fetch_add(1, Ordering::Relaxed)) {
                    let resultado = miniatura(&clave.0);
                    if salida.blocking_send((clave.clone(), resultado)).is_err() {
                        break;
                    }
                }
            });
        }
    });
}

/// Copia la imagen, no el nombre del archivo, al portapapeles de Windows.
#[cfg(windows)]
pub fn copiar(path: &Path) -> Result<(), String> {
    let image = decodificar(path)?.into_rgba8();
    let (ancho, alto) = image.dimensions();
    let longitud = (ancho as usize)
        .checked_mul(alto as usize)
        .and_then(|n| n.checked_mul(4))
        .ok_or("La imagen es demasiado grande para copiarla.")?;
    // BITMAPV5HEADER con máscaras RGBA y filas de arriba a abajo.
    let mut dib = vec![0u8; 124 + longitud];
    let u32_en = |dib: &mut [u8], i: usize, n: u32| dib[i..i + 4].copy_from_slice(&n.to_le_bytes());
    u32_en(&mut dib, 0, 124);
    u32_en(&mut dib, 4, ancho);
    u32_en(&mut dib, 8, (-(alto as i32)) as u32);
    dib[12..14].copy_from_slice(&1u16.to_le_bytes());
    dib[14..16].copy_from_slice(&32u16.to_le_bytes());
    u32_en(&mut dib, 16, 3); // BI_BITFIELDS
    u32_en(&mut dib, 20, longitud as u32);
    for (i, mask) in [
        (40, 0x00ff0000),
        (44, 0x0000ff00),
        (48, 0x000000ff),
        (52, 0xff000000),
        (56, 0x73524742),
    ] {
        u32_en(&mut dib, i, mask);
    }
    for (destino, pixel) in dib[124..]
        .chunks_exact_mut(4)
        .zip(image.as_raw().chunks_exact(4))
    {
        destino.copy_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
    }
    portapapeles(17, &dib) // CF_DIBV5
}

#[cfg(windows)]
pub fn copiar_archivos(paths: &[PathBuf]) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    for path in paths {
        original_valido(path)?;
    }
    // DROPFILES Unicode: las capturas se pueden pegar como archivos en Explorer.
    let mut datos = vec![0u8; 20];
    datos[..4].copy_from_slice(&20u32.to_le_bytes());
    datos[16..20].copy_from_slice(&1u32.to_le_bytes());
    for path in paths {
        for c in path.as_os_str().encode_wide().chain(std::iter::once(0)) {
            datos.extend_from_slice(&c.to_le_bytes());
        }
    }
    datos.extend_from_slice(&0u16.to_le_bytes());
    portapapeles(15, &datos) // CF_HDROP
}

#[cfg(windows)]
fn portapapeles(formato: u32, datos: &[u8]) -> Result<(), String> {
    use std::ffi::c_void;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GlobalAlloc(flags: u32, bytes: usize) -> *mut c_void;
        fn GlobalLock(handle: *mut c_void) -> *mut c_void;
        fn GlobalUnlock(handle: *mut c_void) -> i32;
        fn GlobalFree(handle: *mut c_void) -> *mut c_void;
    }
    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetForegroundWindow() -> *mut c_void;
        fn GetDesktopWindow() -> *mut c_void;
        fn OpenClipboard(window: *mut c_void) -> i32;
        fn EmptyClipboard() -> i32;
        fn SetClipboardData(format: u32, data: *mut c_void) -> *mut c_void;
        fn CloseClipboard() -> i32;
    }
    // La memoria pasa a ser propiedad del sistema solo tras SetClipboardData.
    unsafe {
        let handle = GlobalAlloc(0x0002, datos.len());
        if handle.is_null() {
            return Err("No hay memoria suficiente para copiar la captura.".into());
        }
        let memoria = GlobalLock(handle);
        if memoria.is_null() {
            GlobalFree(handle);
            return Err("No se pudo preparar la captura para copiarla.".into());
        }
        std::ptr::copy_nonoverlapping(datos.as_ptr(), memoria.cast::<u8>(), datos.len());
        GlobalUnlock(handle);
        let window = GetForegroundWindow();
        let window = if window.is_null() {
            GetDesktopWindow()
        } else {
            window
        };
        if OpenClipboard(window) == 0 {
            GlobalFree(handle);
            return Err("El portapapeles está ocupado. Intenta copiar de nuevo.".into());
        }
        let copiada = EmptyClipboard() != 0 && !SetClipboardData(formato, handle).is_null();
        CloseClipboard();
        if !copiada {
            GlobalFree(handle);
            return Err("No se pudo copiar la captura al portapapeles.".into());
        }
    }
    Ok(())
}

#[cfg(not(windows))]
pub fn copiar(_path: &Path) -> Result<(), String> {
    Err("La copia de imágenes al portapapeles aún no está disponible en este sistema.".into())
}

#[cfg(not(windows))]
pub fn copiar_archivos(_paths: &[PathBuf]) -> Result<(), String> {
    Err("La copia de capturas al portapapeles aún no está disponible en este sistema.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lee_originales_aislados_y_sin_recorrer_otras_carpetas() {
        let raiz = std::env::temp_dir().join(format!(
            "icaro-capturas-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store = Almacen::nuevo(&raiz);
        let a = store.crear("Primera", "1.21.4", 0).unwrap();
        let b = store.crear("Segunda", "1.21.4", 0).unwrap();
        let folder = carpeta(&store, &a.id).unwrap();
        fs::create_dir(&folder).unwrap();
        let ruta = folder.join("2026-10-11_17.02.43.png");
        image::RgbaImage::from_pixel(1920, 1080, image::Rgba([10, 20, 30, 255]))
            .save(&ruta)
            .unwrap();
        fs::create_dir(folder.join("anidada")).unwrap();
        fs::write(folder.join("ignorar.txt"), "texto").unwrap();
        fs::write(folder.join("incompleta.png"), "").unwrap();
        let capturas = listar(&store, &[a.clone(), b]).unwrap();
        assert_eq!(capturas.len(), 1);
        assert_eq!(capturas[0].fecha, "11-10-2026 17:02");
        let original = capturas[0].original.as_ref().unwrap();
        assert_eq!(original.ruta, ruta);
        assert_eq!(original.instancia_id, a.id);
        assert_eq!(image::image_dimensions(&ruta).unwrap(), (1920, 1080));
        original_valido(&ruta).unwrap();
        assert!(original.miniatura.is_none());
        assert!(original_valido(&folder.join("anidada")).is_err());

        // Las miniaturas se reducen sin tocar el original y un PNG dañado
        // solo produce un error para esa captura.
        let danada = folder.join("danada.png");
        fs::write(&danada, "no es un png").unwrap();
        let (tx, mut rx) = tokio::sync::mpsc::channel(4);
        miniaturas(
            vec![(ruta.clone(), 0, None), (danada.clone(), 0, None)],
            tx,
        );
        let mut resultados = Vec::new();
        while let Ok(r) = rx.try_recv() {
            resultados.push(r);
        }
        resultados.sort_by(|a, b| a.0.0.cmp(&b.0.0));
        assert_eq!(resultados.len(), 2);
        assert!(resultados[0].1.is_ok() && resultados[0].0.0 == ruta);
        assert!(resultados[1].1.is_err());
        match resultados[0].1.as_ref().unwrap() {
            Handle::Rgba { width, height, .. } => assert_eq!((*width, *height), (480, 270)),
            otro => panic!("miniatura inesperada: {otro:?}"),
        }
        assert_eq!(image::image_dimensions(&ruta).unwrap(), (1920, 1080));
        fs::remove_dir_all(&raiz).unwrap();
    }

    #[test]
    fn nombres_ajenos_no_rompen_la_fecha() {
        assert_eq!(fecha("á-é-💛_foto.png"), "Fecha no disponible");
        assert_eq!(fecha("2026-10-11_17.02.43.png"), "11-10-2026 17:02");
    }
}
