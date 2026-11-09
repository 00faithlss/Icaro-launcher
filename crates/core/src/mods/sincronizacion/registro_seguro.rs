// SPDX-License-Identifier: GPL-3.0-only
//! Compatibilidad del registro anterior: solo métricas y etiquetas conocidas.
use crate::diagnostico::{Campo, Causa, Medicion, Resultado};
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

pub(super) fn alias(valor: &str) -> String {
    static SAL: OnceLock<Option<[u8; 32]>> = OnceLock::new();
    let Some(sal) = SAL.get_or_init(|| {
        let mut bytes = [0; 32];
        getrandom::fill(&mut bytes).ok().map(|_| bytes)
    }) else {
        return "omitida".into();
    };
    let mut hash = Sha256::new();
    hash.update(sal);
    hash.update(valor);
    format!("{:x}", hash.finalize())[..16].to_string()
}

const EVENTOS: &[&str] = &[
    "revision-local",
    "error",
    "aviso",
    "plan",
    "aplicar-inicio",
    "aplicar-fin",
    "al-dia",
    "descarga",
    "commit",
    "rollback",
    "recuperacion",
    "indice",
];
const ETIQUETAS: &[&str] = &[
    "ok",
    "error",
    "true",
    "false",
    "modrinth",
    "anfitrion",
    "deshecha",
    "limpieza-confirmada",
    "descartar-descargas",
    "revision-local",
    "plan",
    "indice",
    "aplicar",
    "recuperacion",
    "inventario",
    "verificado",
    "plan-limite-90s",
    "anfitrion-cambio",
];
fn etiqueta(valor: &str) -> Option<&'static str> {
    ETIQUETAS.iter().copied().find(|v| *v == valor)
}
fn numero(clave: &str) -> Option<&'static str> {
    [
        "archivos",
        "bytes",
        "releidos",
        "reutilizados",
        "ms",
        "agregar",
        "actualizar",
        "quitar",
        "conflictos",
        "reemplazos",
        "descarga_bytes",
        "movimientos",
        "mods",
        "archivos_propios",
        "avisos",
    ]
    .into_iter()
    .find(|v| *v == clave)
}
pub(super) fn linea(mensaje: &str) -> String {
    // El texto del error y todo lo que le siga son deliberadamente opacos.
    let mut partes = mensaje
        .split("mensaje=")
        .next()
        .unwrap_or_default()
        .split_whitespace();
    let evento = partes
        .next()
        .and_then(|e| EVENTOS.iter().copied().find(|v| *v == e))
        .unwrap_or("evento");
    let op = Medicion::global("sincronizacion_antigua", evento);
    let mut salida = vec![evento.to_string()];
    for parte in partes {
        let Some((clave, valor)) = parte.split_once('=') else {
            continue;
        };
        match clave {
            "instancia" | "carpeta" | "tx" => {
                salida.push(format!("{clave}={}", alias(valor)));
                if clave == "instancia" {
                    op.instancia(valor);
                }
            }
            "fase" | "fuente" | "resultado" | "accion" | "sin_red" => {
                if let Some(valor) = etiqueta(valor) {
                    salida.push(format!("{clave}={valor}"));
                    if clave == "fase" {
                        op.etapa(valor);
                    }
                    if clave == "fuente" {
                        op.dato(Campo::Etiqueta("fuente", valor));
                    }
                }
            }
            "revision" if valor.len() == 12 && valor.bytes().all(|b| b.is_ascii_hexdigit()) => {
                salida.push(format!("revision={valor}"));
            }
            _ => {
                if let (Some(clave), Ok(valor)) = (numero(clave), valor.parse::<u64>()) {
                    salida.push(format!("{clave}={valor}"));
                    op.dato(Campo::Numero(clave, valor));
                }
            }
        }
    }
    op.terminar(if evento == "error" {
        Resultado::Error(Causa::Interno)
    } else {
        Resultado::Exito
    });
    salida.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn el_registro_anterior_no_guarda_direcciones_nombres_ni_texto_de_errores() {
        let texto = linea("error fase=plan instancia=0123456789abcdef0123456789abcdef origen=https://privado.example/token carpeta=MundoPrivado archivo=secreto.jar ms=123 mensaje=credencial C:\\Users\\PersonaPrivada\\mundo bytes=999");
        assert!(texto.contains("fase=plan"));
        assert!(texto.contains("ms=123"));
        for secreto in [
            "0123456789abcdef0123456789abcdef",
            "privado.example",
            "token",
            "MundoPrivado",
            "secreto.jar",
            "credencial",
            "PersonaPrivada",
            "999",
        ] {
            assert!(!texto.contains(secreto), "{secreto}");
        }
    }
    #[test]
    fn se_conservan_metricas_de_recuperacion_y_no_su_ruta() {
        let texto = linea("recuperacion carpeta=Supervivencia tx=.icaro-mods-tx-personal accion=deshecha movimientos=1");
        assert!(texto.contains("accion=deshecha movimientos=1"));
        assert!(!texto.contains("Supervivencia"));
        assert!(!texto.contains("personal"));
    }
}
