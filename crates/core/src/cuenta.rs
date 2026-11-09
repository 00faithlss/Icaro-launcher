// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Perfil y skins oficiales. Los tokens permanecen dentro del núcleo.
use crate::auth::Session;
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::Arc, time::Duration};

const API: &str = "https://api.minecraftservices.com/minecraft/profile";
const MAX_PNG: usize = 1024 * 1024;
pub fn huella(png: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(png))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skin {
    pub id: String,
    pub state: String,
    pub url: String,
    pub variant: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cape {
    pub id: String,
    pub state: String,
    pub url: String,
    pub alias: String,
}
#[derive(Debug, Clone, Deserialize)]
pub struct Perfil {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub skins: Vec<Skin>,
    #[serde(default)]
    pub capes: Vec<Cape>,
}
#[derive(Debug, Clone)]
pub struct Textura {
    pub png: Arc<[u8]>,
    pub rgba: Arc<[u8]>,
}
#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("Vuelve a iniciar sesión para gestionar tu skin.")]
    Sesion,
    #[error("No se pudo consultar Minecraft. Revisa tu conexión y vuelve a intentarlo.")]
    Red,
    #[error("Minecraft rechazó el cambio de skin. Tu skin anterior se conserva.")]
    Rechazada,
    #[error("Solo se admiten imágenes PNG válidas de 64 × 64 o 64 × 32 píxeles y hasta 1 MB.")]
    Imagen,
    #[error("No se pudo leer o guardar la skin.")]
    Disco,
    #[error("El perfil recibido no corresponde a tu cuenta.")]
    Perfil,
}
fn cliente() -> Result<reqwest::Client, Error> {
    reqwest::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(45))
        .user_agent(crate::http::USER_AGENT)
        .build()
        .map_err(|_| Error::Red)
}
fn sesion(s: &Session) -> Result<(), Error> {
    if s.needs_refresh() || !s.has_minecraft_access() {
        Err(Error::Sesion)
    } else {
        Ok(())
    }
}
fn validar_perfil(p: &Perfil, id: &str) -> Result<(), Error> {
    if p.id != id || p.skins.len() > 64 || p.capes.len() > 64 {
        return Err(Error::Perfil);
    }
    for skin in &p.skins {
        textura_url(&skin.url)?;
    }
    for cape in &p.capes {
        textura_url(&cape.url)?;
    }
    Ok(())
}
pub async fn perfil(s: &Session) -> Result<Perfil, Error> {
    sesion(s)?;
    let response = cliente()?
        .get(API)
        .bearer_auth(s.minecraft_token())
        .send()
        .await
        .map_err(|_| Error::Red)?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(Error::Sesion);
    }
    if !response.status().is_success() {
        return Err(Error::Red);
    }
    let p: Perfil = crate::http::body_json(response, "Minecraft", MAX_PNG)
        .await
        .map_err(|_| Error::Perfil)?;
    validar_perfil(&p, &s.profile.id)?;
    Ok(p)
}
fn textura_url(value: &str) -> Result<reqwest::Url, Error> {
    let mut url = reqwest::Url::parse(value).map_err(|_| Error::Perfil)?;
    let parts: Vec<_> = url.path_segments().ok_or(Error::Perfil)?.collect();
    if !matches!(url.scheme(), "http" | "https")
        || url.host_str() != Some("textures.minecraft.net")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || parts.len() != 2
        || parts[0] != "texture"
        || parts[1].len() != 64
        || !parts[1].bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(Error::Perfil);
    }
    url.set_scheme("https").map_err(|_| Error::Perfil)?;
    Ok(url)
}
/// Dimensiones declaradas en la cabecera PNG, tras comprobar firma y tamaño.
fn dimensiones_png(bytes: &[u8]) -> Result<(u32, u32), Error> {
    if bytes.len() < 24
        || bytes.len() > MAX_PNG
        || &bytes[..8] != b"\x89PNG\r\n\x1a\n"
        || &bytes[12..16] != b"IHDR"
    {
        return Err(Error::Imagen);
    }
    Ok((
        u32::from_be_bytes(bytes[16..20].try_into().unwrap()),
        u32::from_be_bytes(bytes[20..24].try_into().unwrap()),
    ))
}

/// Skin de 64 × 64 o del formato antiguo de 64 × 32. Este último se expande a
/// 64 × 64 copiando en espejo el brazo y la pierna derechos a los izquierdos,
/// como hace Minecraft; el PNG original se conserva para subirlo.
pub fn decodificar(bytes: Vec<u8>) -> Result<Textura, Error> {
    let (ancho, alto) = dimensiones_png(&bytes)?;
    if ancho != 64 || !matches!(alto, 64 | 32) {
        return Err(Error::Imagen);
    }
    let image = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
        .map_err(|_| Error::Imagen)?
        .into_rgba8();
    let rgba = if alto == 32 { expandir_antigua(&image.into_raw()) } else { image.into_raw() };
    Ok(Textura {
        png: bytes.into(),
        rgba: rgba.into(),
    })
}

fn expandir_antigua(rgba: &[u8]) -> Vec<u8> {
    let mut salida = vec![0u8; 64 * 64 * 4];
    salida[..64 * 32 * 4].copy_from_slice(&rgba[..64 * 32 * 4]);
    // (origen x, origen y, ancho, alto, destino x, destino y), en espejo.
    let regiones = [
        (4, 16, 4, 4, 20, 48),
        (8, 16, 4, 4, 24, 48),
        (0, 20, 4, 12, 24, 52),
        (4, 20, 4, 12, 20, 52),
        (8, 20, 4, 12, 16, 52),
        (12, 20, 4, 12, 28, 52),
        (44, 16, 4, 4, 36, 48),
        (48, 16, 4, 4, 40, 48),
        (40, 20, 4, 12, 40, 52),
        (44, 20, 4, 12, 36, 52),
        (48, 20, 4, 12, 32, 52),
        (52, 20, 4, 12, 44, 52),
    ];
    for (ox, oy, w, h, dx, dy) in regiones {
        for y in 0..h {
            for x in 0..w {
                let o = ((oy + y) * 64 + ox + x) * 4;
                let d = ((dy + y) * 64 + dx + w - 1 - x) * 4;
                salida.copy_within(o..o + 4, d);
            }
        }
    }
    salida
}

/// Textura de una capa (cape): PNG de 64 × 32.
pub fn decodificar_capa(bytes: Vec<u8>) -> Result<Textura, Error> {
    if dimensiones_png(&bytes)? != (64, 32) {
        return Err(Error::Imagen);
    }
    let image = image::load_from_memory_with_format(&bytes, image::ImageFormat::Png)
        .map_err(|_| Error::Imagen)?
        .into_rgba8();
    Ok(Textura {
        png: bytes.into(),
        rgba: image.into_raw().into(),
    })
}
pub fn cargar(path: &Path) -> Result<Textura, Error> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|_| Error::Disco)?;
    let mut bytes = vec![];
    file.take((MAX_PNG + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Disco)?;
    decodificar(bytes)
}
/// PNG de `textures.minecraft.net`, sin credenciales y con tamaño acotado.
async fn bajar_textura(url: &str) -> Result<Vec<u8>, Error> {
    let mut response = cliente()?
        .get(textura_url(url)?)
        .send()
        .await
        .map_err(|_| Error::Red)?;
    if !response.status().is_success() {
        return Err(Error::Red);
    }
    let mut bytes = vec![];
    while let Some(chunk) = response.chunk().await.map_err(|_| Error::Red)? {
        if bytes.len() + chunk.len() > MAX_PNG {
            return Err(Error::Imagen);
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}
pub async fn descargar_skin(skin: &Skin) -> Result<Textura, Error> {
    let bytes = bajar_textura(&skin.url).await?;
    tokio::task::spawn_blocking(move || decodificar(bytes))
        .await
        .map_err(|_| Error::Imagen)?
}
pub async fn descargar_capa(capa: &Cape) -> Result<Textura, Error> {
    let bytes = bajar_textura(&capa.url).await?;
    tokio::task::spawn_blocking(move || decodificar_capa(bytes))
        .await
        .map_err(|_| Error::Imagen)?
}
/// Pone la capa `id` de la cuenta, o la quita con `None`. Devuelve el perfil
/// actualizado.
pub async fn poner_capa(s: &Session, id: Option<&str>) -> Result<Perfil, Error> {
    sesion(s)?;
    let url = format!("{API}/capes/active");
    let solicitud = match id {
        Some(id) => cliente()?
            .put(url)
            .json(&serde_json::json!({ "capeId": id })),
        None => cliente()?.delete(url),
    };
    let response = solicitud
        .bearer_auth(s.minecraft_token())
        .send()
        .await
        .map_err(|_| Error::Red)?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(Error::Sesion);
    }
    if !response.status().is_success() {
        return Err(Error::Rechazada);
    }
    perfil(s).await
}
pub async fn aplicar(s: &Session, textura: &Textura, delgada: bool) -> Result<Perfil, Error> {
    sesion(s)?;
    decodificar(textura.png.to_vec())?;
    let form = reqwest::multipart::Form::new()
        .text("variant", if delgada { "slim" } else { "classic" })
        .part(
            "file",
            reqwest::multipart::Part::bytes(textura.png.to_vec())
                .file_name("skin.png")
                .mime_str("image/png")
                .map_err(|_| Error::Imagen)?,
        );
    let response = cliente()?
        .post(format!("{API}/skins"))
        .bearer_auth(s.minecraft_token())
        .multipart(form)
        .send()
        .await
        .map_err(|_| Error::Red)?;
    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(Error::Sesion);
    }
    if !response.status().is_success() {
        return Err(Error::Rechazada);
    }
    perfil(s).await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn solo_descarga_texturas_oficiales_sin_credenciales_ni_redirecciones() {
        assert!(textura_url(&format!(
            "http://textures.minecraft.net/texture/{}",
            "ab".repeat(32)
        ))
        .unwrap()
        .as_str()
        .starts_with("https://"));
        for u in [
            "https://otro.example/texture/a",
            "https://textures.minecraft.net@localhost/texture/a",
            "file:///skin.png",
            "https://textures.minecraft.net/texture/a?token=secret",
        ] {
            assert!(textura_url(u).is_err());
        }
    }
    #[test]
    fn no_acepta_una_cabecera_sin_png_completo_ni_dimensiones_ajenas() {
        use image::ImageEncoder;
        let mut bytes = vec![];
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(&vec![200; 64 * 64 * 4], 64, 64, image::ColorType::Rgba8)
            .unwrap();
        let image = decodificar(bytes.clone()).unwrap();
        assert_eq!(image.rgba.len(), 64 * 64 * 4);
        assert!(decodificar(bytes[..24].to_vec()).is_err());
        bytes[19] = 63;
        assert!(decodificar(bytes).is_err());
        assert!(decodificar(vec![0; MAX_PNG + 1]).is_err());
    }
    fn png(ancho: u32, alto: u32, rgba: &[u8]) -> Vec<u8> {
        use image::ImageEncoder;
        let mut bytes = vec![];
        image::codecs::png::PngEncoder::new(&mut bytes)
            .write_image(rgba, ancho, alto, image::ColorType::Rgba8)
            .unwrap();
        bytes
    }
    #[test]
    fn una_skin_antigua_de_64x32_se_expande_con_las_extremidades_en_espejo() {
        let mut rgba = vec![0u8; 64 * 32 * 4];
        // Primer píxel del frente de la pierna derecha (4, 20) en rojo.
        let i = (20 * 64 + 4) * 4;
        rgba[i..i + 4].copy_from_slice(&[255, 0, 0, 255]);
        let t = decodificar(png(64, 32, &rgba)).unwrap();
        assert_eq!(t.rgba.len(), 64 * 64 * 4);
        // Frente de la pierna izquierda (20, 52), reflejado: último píxel de la fila.
        let d = (52 * 64 + 23) * 4;
        assert_eq!(&t.rgba[d..d + 4], &[255, 0, 0, 255]);
        assert_eq!(&t.png[..], &png(64, 32, &rgba)[..]);
    }
    #[test]
    fn la_capa_exige_64x32() {
        assert!(decodificar_capa(png(64, 32, &vec![9; 64 * 32 * 4])).is_ok());
        assert!(decodificar_capa(png(64, 64, &vec![9; 64 * 64 * 4])).is_err());
    }
}
