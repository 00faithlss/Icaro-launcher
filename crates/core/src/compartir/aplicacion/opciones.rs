// SPDX-License-Identifier: GPL-3.0-only
use super::{diario::Huella, estado, rutas, Error};
use std::path::Path;

pub(super) fn preparar(
    juego: &Path,
    anteriores: &[String],
    nuevos: &[String],
) -> Result<Option<(Option<Huella>, Vec<u8>)>, Error> {
    let path = rutas::resolver(juego, "options.txt", false)?;
    let original = estado::leer_acotado(&path, 4 * 1024 * 1024)?;
    let antes = original.as_ref().map(|b| estado::de_bytes(b));
    let bytes = original.unwrap_or_default();
    let texto = std::str::from_utf8(&bytes).map_err(|_| Error::Datos("options.txt no es UTF-8"))?;
    let mut lineas = texto
        .lines()
        .filter_map(|l| l.strip_prefix("resourcePacks:"));
    let actual: Vec<String> = match lineas.next() {
        Some(valor) => {
            serde_json::from_str(valor).map_err(|_| Error::Datos("selección de resourcepacks"))?
        }
        None => vec![],
    };
    if lineas.next().is_some() {
        return Err(Error::Datos("selección de packs duplicada"));
    }
    let orden = crate::compartir::packs::combinar_orden(&actual, anteriores, nuevos)?;
    if orden == actual {
        return Ok(None);
    }
    let valor = format!(
        "resourcePacks:{}",
        serde_json::to_string(&orden).map_err(|_| Error::Datos("selección de packs"))?
    );
    let mut salida = String::with_capacity(texto.len() + valor.len());
    let mut reemplazada = false;
    for linea in texto.split_inclusive('\n') {
        if linea.starts_with("resourcePacks:") {
            salida.push_str(&valor);
            if linea.ends_with("\r\n") {
                salida.push_str("\r\n");
            } else if linea.ends_with('\n') {
                salida.push('\n');
            }
            reemplazada = true;
        } else {
            salida.push_str(linea);
        }
    }
    if !reemplazada {
        let fin = if texto.contains("\r\n") { "\r\n" } else { "\n" };
        if !salida.is_empty() && !salida.ends_with('\n') {
            salida.push_str(fin);
        }
        salida.push_str(&valor);
        salida.push_str(fin);
    }
    Ok(Some((antes, salida.into_bytes())))
}
