// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! IconoInstancia: imágenes acotadas, recorte y cambio reversible por sesión.
use crate::instancias::Almacen;
use image::{imageops::FilterType, ImageFormat, RgbaImage};
use std::{
    fs,
    io::{Cursor, Read},
    path::{Path, PathBuf},
    sync::Arc,
};
const MAX_ARCHIVO: u64 = 10 * 1024 * 1024;
#[derive(Debug, Clone)]
pub struct Fuente {
    pub nombre: String,
    pub imagen: Arc<RgbaImage>,
}
#[derive(Debug, Clone, Default)]
pub struct Anterior {
    pub imagen: Option<Vec<u8>>,
    pub variacion: Option<Vec<u8>>,
}
fn regular(path: &Path) -> Result<fs::Metadata, String> {
    let m = fs::symlink_metadata(path).map_err(|_| "No se pudo abrir la imagen.")?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if m.file_attributes() & 0x400 != 0 {
            return Err("Elige una imagen local, sin enlaces a otras carpetas.".into());
        }
    }
    if !m.is_file() || m.file_type().is_symlink() {
        return Err("Elige un archivo de imagen local.".into());
    }
    Ok(m)
}
fn decodificar(bytes: &[u8]) -> Result<RgbaImage, String> {
    let mut reader = image::io::Reader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| "No se reconoce el formato de la imagen.")?;
    if !matches!(
        reader.format(),
        Some(ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP | ImageFormat::Gif)
    ) {
        return Err("Usa una imagen PNG, JPG, WebP o GIF.".into());
    }
    let mut limits = image::io::Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(128 * 1024 * 1024);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|_| "La imagen está dañada o supera las dimensiones admitidas.")?;
    if image.width() as u64 * image.height() as u64 > 16_777_216 {
        return Err("La imagen supera los 16 millones de píxeles admitidos.".into());
    }
    Ok(image.into_rgba8())
}
pub fn abrir(path: &Path) -> Result<Fuente, String> {
    let m = regular(path)?;
    if m.len() > MAX_ARCHIVO {
        return Err("La imagen pesa más de 10 MB.".into());
    }
    let mut bytes = vec![];
    fs::File::open(path)
        .map_err(|_| "No se pudo abrir la imagen.")?
        .take(MAX_ARCHIVO + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "No se pudo leer la imagen.")?;
    if bytes.len() as u64 > MAX_ARCHIVO {
        return Err("La imagen pesa más de 10 MB.".into());
    }
    Ok(Fuente {
        nombre: path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        imagen: Arc::new(decodificar(&bytes)?),
    })
}
pub fn recortar(f: &Fuente, zoom: u32, x: f32, y: f32, estilo: u32) -> Result<Vec<u8>, String> {
    if matches!(estilo, 16 | 32) {
        // Mismos bloques finales, sin copiar y reducir millones de píxeles.
        return png(vista_previa(f, zoom, x, y, estilo)?);
    }
    let (left, top, size) = region(f, zoom, x, y)?;
    let cropped = image::imageops::crop_imm(f.imagen.as_ref(), left, top, size, size).to_image();
    png(image::imageops::resize(
        &cropped,
        256,
        256,
        FilterType::Lanczos3,
    ))
}
/// Vista interactiva: trabajo acotado a 256² muestras, sin volver a codificar PNG.
/// El archivo definitivo conserva el recorte Lanczos de la imagen original.
pub fn vista_previa(
    f: &Fuente,
    zoom: u32,
    x: f32,
    y: f32,
    estilo: u32,
) -> Result<RgbaImage, String> {
    let (left, top, size) = region(f, zoom, x, y)?;
    let img = f.imagen.as_ref();
    let grid = if matches!(estilo, 16 | 32) {
        estilo
    } else {
        256
    };
    Ok(RgbaImage::from_fn(256, 256, |ox, oy| {
        let gx = ox * grid / 256;
        let gy = oy * grid / 256;
        let sx = ((gx as f32 + 0.5) * size as f32 / grid as f32 - 0.5).clamp(0., (size - 1) as f32);
        let sy = ((gy as f32 + 0.5) * size as f32 / grid as f32 - 0.5).clamp(0., (size - 1) as f32);
        if grid != 256 {
            return *img.get_pixel(left + sx.round() as u32, top + sy.round() as u32);
        }
        let (ix, iy) = (sx.floor() as u32, sy.floor() as u32);
        let (nx, ny) = ((ix + 1).min(size - 1), (iy + 1).min(size - 1));
        let (tx, ty) = (sx - ix as f32, sy - iy as f32);
        let (a, b, c, d) = (
            img.get_pixel(left + ix, top + iy),
            img.get_pixel(left + nx, top + iy),
            img.get_pixel(left + ix, top + ny),
            img.get_pixel(left + nx, top + ny),
        );
        image::Rgba(std::array::from_fn(|i| {
            ((a[i] as f32 * (1. - tx) + b[i] as f32 * tx) * (1. - ty)
                + (c[i] as f32 * (1. - tx) + d[i] as f32 * tx) * ty)
                .round() as u8
        }))
    }))
}
fn region(f: &Fuente, zoom: u32, x: f32, y: f32) -> Result<(u32, u32, u32), String> {
    let img = f.imagen.as_ref();
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Err("La imagen está vacía.".into());
    }
    let side = (w.min(h) as f32 / (zoom.clamp(100, 400) as f32 / 100.)).max(1.);
    let sx = (w as f32 - side) / 2. - x * side / 280.;
    let sy = (h as f32 - side) / 2. - y * side / 280.;
    let left = sx.clamp(0., w as f32 - side).round() as u32;
    let top = sy.clamp(0., h as f32 - side).round() as u32;
    let size = (side.round() as u32).min(w - left).min(h - top).max(1);
    Ok((left, top, size))
}
fn png(img: RgbaImage) -> Result<Vec<u8>, String> {
    let mut out = Cursor::new(vec![]);
    image::DynamicImage::ImageRgba8(img)
        .write_to(&mut out, ImageFormat::Png)
        .map_err(|_| "No se pudo preparar el icono.")?;
    Ok(out.into_inner())
}
/// Misma FNV-1a sobre UTF-16 y xorshift que bundle.js del System Design.
pub fn patron(nombre: &str, variacion: u32) -> [u8; 64] {
    let name = if variacion == 0 {
        nombre.to_owned()
    } else {
        format!("{nombre} · {variacion}")
    };
    let mut r = 2_166_136_261u32;
    for c in name.encode_utf16() {
        r = (r ^ u32::from(c)).wrapping_mul(16_777_619);
    }
    if r == 0 {
        r = 1;
    }
    let mut result = [0; 64];
    for y in 0..8 {
        for x in 0..4 {
            r ^= r << 13;
            r ^= r >> 17;
            r ^= r << 5;
            let n = r % 9;
            let mut t = if n < 3 {
                0
            } else if n < 5 {
                1
            } else if n < 7 {
                2
            } else if n < 8 {
                3
            } else {
                4
            };
            if (x == 0 || x == 3) && (y == 0 || y == 7) {
                t = 0;
            }
            result[y * 8 + x] = t;
            result[y * 8 + 7 - x] = t;
        }
    }
    result
}
fn leer_opt(path: &Path, max: u64) -> Result<Option<Vec<u8>>, String> {
    if !path
        .try_exists()
        .map_err(|_| "No se pudo leer el icono actual.")?
    {
        return Ok(None);
    }
    let m = regular(path)?;
    if m.len() > max {
        return Err("El icono guardado no es válido.".into());
    }
    let bytes = fs::read(path).map_err(|_| "No se pudo leer el icono actual.")?;
    if bytes.len() as u64 > max {
        return Err("El icono guardado no es válido.".into());
    }
    Ok(Some(bytes))
}
fn publicar(path: &Path, bytes: Option<&[u8]>) -> Result<(), String> {
    if path.exists() {
        regular(path)?;
    }
    if let Some(bytes) = bytes {
        let mut nonce = [0u8; 8];
        getrandom::fill(&mut nonce).map_err(|_| "No se pudo preparar el icono.")?;
        let temp = path.with_extension(format!("icaro-{}.tmp", u64::from_le_bytes(nonce)));
        let result = (|| {
            use std::io::Write;
            let mut f = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temp)
                .map_err(|_| "No se pudo guardar el icono.")?;
            f.write_all(bytes)
                .map_err(|_| "No se pudo guardar el icono.")?;
            f.sync_all().map_err(|_| "No se pudo guardar el icono.")?;
            fs::rename(&temp, path).map_err(|_| "No se pudo reemplazar el icono.")?;
            Ok(())
        })();
        let _ = fs::remove_file(temp);
        result
    } else {
        if path.exists() {
            fs::remove_file(path).map_err(|_| "No se pudo volver al icono generado.")?;
        }
        Ok(())
    }
}
pub fn leer(store: &Almacen, id: &str) -> Result<(Option<Vec<u8>>, u32), String> {
    let root = store.carpeta(id).map_err(|e| e.to_string())?;
    let img = leer_opt(&root.join("icon.png"), MAX_ARCHIVO)?;
    if let Some(b) = &img {
        let decoded = decodificar(b)?;
        if decoded.width() != 256 || decoded.height() != 256 {
            return Err("El icono guardado debe medir 256 × 256.".into());
        }
    }
    let v = leer_opt(&root.join("icono.json"), 4096)?
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| v.get("variacion")?.as_u64())
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(0);
    Ok((img, v))
}
pub fn guardar(
    store: &Almacen,
    id: &str,
    img: Option<Vec<u8>>,
    variacion: u32,
) -> Result<Anterior, String> {
    if let Some(b) = &img {
        if b.len() as u64 > MAX_ARCHIVO {
            return Err("El icono pesa más de 10 MB.".into());
        }
        let decoded = decodificar(b)?;
        if decoded.dimensions() != (256, 256) {
            return Err("El icono debe medir 256 × 256.".into());
        }
    }
    let root = store.carpeta(id).map_err(|e| e.to_string())?;
    let old = Anterior {
        imagen: leer_opt(&root.join("icon.png"), MAX_ARCHIVO)?,
        variacion: leer_opt(&root.join("icono.json"), 4096)?,
    };
    let meta = serde_json::json!({"variacion":variacion})
        .to_string()
        .into_bytes();
    publicar(&root.join("icon.png"), img.as_deref())?;
    if let Err(e) = publicar(&root.join("icono.json"), Some(&meta)) {
        let _ = publicar(&root.join("icon.png"), old.imagen.as_deref());
        return Err(e);
    }
    Ok(old)
}
pub fn deshacer(store: &Almacen, id: &str, old: &Anterior) -> Result<(), String> {
    let root = store.carpeta(id).map_err(|e| e.to_string())?;
    let now = Anterior {
        imagen: leer_opt(&root.join("icon.png"), MAX_ARCHIVO)?,
        variacion: leer_opt(&root.join("icono.json"), 4096)?,
    };
    publicar(&root.join("icon.png"), old.imagen.as_deref())?;
    if let Err(e) = publicar(&root.join("icono.json"), old.variacion.as_deref()) {
        let _ = publicar(&root.join("icon.png"), now.imagen.as_deref());
        return Err(e);
    }
    Ok(())
}
/// Tamaño de una portada propia: la proporción de la portada de la tarjeta.
pub const PORTADA: (u32, u32) = (600, 262);

/// Recorta al centro y escala la imagen a [`PORTADA`], como `object-fit: cover`.
pub fn recortar_portada(f: &Fuente) -> Result<Vec<u8>, String> {
    let img = f.imagen.as_ref();
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Err("La imagen está vacía.".into());
    }
    let (pw, ph) = PORTADA;
    // Lado que manda: el que deja cubrir toda la portada sin bandas.
    let (cw, ch) = if u64::from(w) * u64::from(ph) > u64::from(h) * u64::from(pw) {
        ((u64::from(h) * u64::from(pw) / u64::from(ph)) as u32, h)
    } else {
        (w, (u64::from(w) * u64::from(ph) / u64::from(pw)) as u32)
    };
    let (cw, ch) = (cw.max(1), ch.max(1));
    let recorte = image::imageops::crop_imm(img, (w - cw) / 2, (h - ch) / 2, cw, ch).to_image();
    png(image::imageops::resize(&recorte, pw, ph, FilterType::Lanczos3))
}

/// Portada propia guardada en la instancia, si tiene.
pub fn leer_portada(store: &Almacen, id: &str) -> Result<Option<Vec<u8>>, String> {
    let root = store.carpeta(id).map_err(|e| e.to_string())?;
    let img = leer_opt(&root.join("portada.png"), MAX_ARCHIVO)?;
    if let Some(b) = &img {
        if decodificar(b)?.dimensions() != PORTADA {
            return Err("La portada guardada no tiene el tamaño esperado.".into());
        }
    }
    Ok(img)
}

/// Guarda (o quita, con `None`) la portada propia de forma atómica.
pub fn guardar_portada(store: &Almacen, id: &str, img: Option<&[u8]>) -> Result<(), String> {
    if let Some(b) = img {
        if b.len() as u64 > MAX_ARCHIVO || decodificar(b)?.dimensions() != PORTADA {
            return Err("La portada no es válida.".into());
        }
    }
    let root = store.carpeta(id).map_err(|e| e.to_string())?;
    publicar(&root.join("portada.png"), img)
}

pub fn capturas(store: &Almacen, id: &str) -> Result<Vec<PathBuf>, String> {
    let game = store.juego(id).map_err(|e| e.to_string())?;
    let path = game.join("screenshots");
    if !path.exists() {
        return Ok(vec![]);
    }
    let m = fs::symlink_metadata(&path).map_err(|_| "No se pudo leer las capturas.")?;
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if m.file_attributes() & 0x400 != 0 {
            return Err("Las capturas deben estar en una carpeta local.".into());
        }
    }
    if !m.is_dir() || m.file_type().is_symlink() {
        return Err("Las capturas deben estar en una carpeta local.".into());
    }
    let mut files = vec![];
    for e in fs::read_dir(path).map_err(|_| "No se pudieron leer las capturas.")? {
        let path = e.map_err(|_| "No se pudo leer una captura.")?.path();
        if regular(&path).is_ok_and(|m| m.len() <= MAX_ARCHIVO)
            && path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
                matches!(
                    e.to_ascii_lowercase().as_str(),
                    "png" | "jpg" | "jpeg" | "webp" | "gif"
                )
            })
        {
            files.push(path);
        }
    }
    files.sort();
    files.reverse();
    files.truncate(120);
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preview_cambia_estilo_y_recorte_sin_mutar_la_fuente() {
        let source = Fuente {
            nombre: "gradiente".into(),
            imagen: Arc::new(RgbaImage::from_fn(2000, 2236, |x, y| {
                image::Rgba([(x % 255) as u8, (y % 255) as u8, ((x + y) % 255) as u8, 255])
            })),
        };
        let original = vista_previa(&source, 100, 0., 0., 0).unwrap();
        let pixels = vista_previa(&source, 100, 0., 0., 16).unwrap();
        assert_ne!(original, pixels);
        assert_eq!(
            pixels,
            decodificar(&recortar(&source, 100, 0., 0., 16).unwrap()).unwrap()
        );
        assert_eq!(original.dimensions(), (256, 256));
        assert_ne!(original, vista_previa(&source, 200, 16., 0., 0).unwrap());
        for y in 0..256 {
            for x in 0..256 {
                assert_eq!(
                    pixels.get_pixel(x, y),
                    pixels.get_pixel(x / 16 * 16, y / 16 * 16)
                );
            }
        }
        assert_eq!(source.imagen.dimensions(), (2000, 2236));
    }
    #[test]
    fn patron_es_simetrico_estable_y_variaciones_distintas() {
        let a = patron("Supervivencia", 0);
        assert_eq!(a, patron("Supervivencia", 0));
        assert_ne!(a, patron("Supervivencia", 1));
        for y in 0..8 {
            for x in 0..4 {
                assert_eq!(a[y * 8 + x], a[y * 8 + 7 - x]);
            }
        }
    }
    #[test]
    fn recorte_cambio_a_generado_y_deshacer_conservan_icono() {
        let root = std::env::temp_dir().join(format!(
            "icaro-icon-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store = Almacen::nuevo(root.join("instances"));
        let i = store.crear("Prueba", "1.21.1", 0).unwrap();
        let img = RgbaImage::from_fn(400, 200, |x, y| image::Rgba([x as u8, y as u8, 0, 255]));
        let source = Fuente {
            nombre: "foto".into(),
            imagen: Arc::new(img),
        };
        let bytes = recortar(&source, 100, 0., 0., 32).unwrap();
        assert_eq!(decodificar(&bytes).unwrap().dimensions(), (256, 256));
        let old = guardar(&store, &i.id, Some(bytes.clone()), 0).unwrap();
        assert!(old.imagen.is_none());
        assert_eq!(leer(&store, &i.id).unwrap().0, Some(bytes));
        let old = guardar(&store, &i.id, None, 12).unwrap();
        assert!(leer(&store, &i.id).unwrap().0.is_none());
        assert_eq!(leer(&store, &i.id).unwrap().1, 12);
        deshacer(&store, &i.id, &old).unwrap();
        assert!(leer(&store, &i.id).unwrap().0.is_some());
        assert!(guardar(&store, &i.id, Some(b"no imagen".to_vec()), 0).is_err());
        assert!(leer(&store, &i.id).unwrap().0.is_some());
        assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn la_portada_propia_cubre_la_proporcion_y_se_guarda_y_quita() {
        let root = std::env::temp_dir().join(format!(
            "icaro-portada-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store = Almacen::nuevo(root.join("instances"));
        let i = store.crear("Prueba", "1.21.1", 0).unwrap();
        for (w, h) in [(1920, 1080), (300, 900), (601, 262)] {
            let source = Fuente {
                nombre: "foto".into(),
                imagen: Arc::new(RgbaImage::from_pixel(w, h, image::Rgba([9, 9, 9, 255]))),
            };
            let bytes = recortar_portada(&source).unwrap();
            assert_eq!(decodificar(&bytes).unwrap().dimensions(), PORTADA);
            guardar_portada(&store, &i.id, Some(&bytes)).unwrap();
            assert_eq!(leer_portada(&store, &i.id).unwrap(), Some(bytes));
        }
        assert!(guardar_portada(&store, &i.id, Some(b"no imagen")).is_err());
        assert!(leer_portada(&store, &i.id).unwrap().is_some());
        guardar_portada(&store, &i.id, None).unwrap();
        assert!(leer_portada(&store, &i.id).unwrap().is_none());
        assert_eq!(root.parent(), Some(std::env::temp_dir().as_path()));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn imagenes_invalidas_y_mayores_a_diez_mb_se_rechazan_y_recorte_respeta_pixels() {
        let root = std::env::temp_dir().join(format!(
            "icaro-icon-limits-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let invalid = root.join("falsa.png");
        fs::write(&invalid, b"texto").unwrap();
        assert!(abrir(&invalid).is_err());
        let big = root.join("grande.png");
        fs::File::create(&big)
            .unwrap()
            .set_len(MAX_ARCHIVO + 1)
            .unwrap();
        assert!(abrir(&big).unwrap_err().contains("10 MB"));
        let source = Fuente {
            nombre: "patron".into(),
            imagen: Arc::new(RgbaImage::from_fn(512, 256, |x, y| {
                image::Rgba([(x % 255) as u8, (y % 255) as u8, 0, 255])
            })),
        };
        let a = decodificar(&recortar(&source, 100, 0., 0., 32).unwrap()).unwrap();
        let b = decodificar(&recortar(&source, 200, 8., 0., 32).unwrap()).unwrap();
        assert_ne!(a, b);
        for y in 0..256 {
            for x in 0..256 {
                assert_eq!(a.get_pixel(x, y), a.get_pixel(x / 8 * 8, y / 8 * 8));
            }
        }
        fs::remove_dir_all(root).unwrap();
    }
}
