// SPDX-License-Identifier: GPL-3.0-only
use super::Resultado;
use serde_json::{Map, Value};

pub struct Parametros {
    pub muestras: usize,
    pub archivos: usize,
    pub bytes: usize,
    pub modo: String,
}
impl Parametros {
    pub fn leer() -> Resultado<Self> {
        let mut parametros = Self {
            muestras: 7,
            archivos: 32,
            bytes: 256 * 1024,
            modo: "sin-cache".into(),
        };
        let mut args = std::env::args().skip(1);
        while let Some(opcion) = args.next() {
            let valor = args.next().ok_or("falta valor de argumento")?;
            match opcion.as_str() {
                "--muestras" => parametros.muestras = valor.parse()?,
                "--archivos" => parametros.archivos = valor.parse()?,
                "--bytes" => parametros.bytes = valor.parse()?,
                "--modo" => parametros.modo = valor,
                _ => return Err(format!("argumento desconocido: {opcion}").into()),
            }
        }
        if !(1..=50).contains(&parametros.muestras)
            || parametros.archivos == 0
            || parametros.bytes < 8
        {
            return Err("muestras: 1..=50; archivos > 0; bytes >= 8".into());
        }
        let payload = parametros
            .archivos
            .checked_mul(parametros.bytes)
            .ok_or("payload desbordado")?;
        if payload > 64 * 1024 * 1024 {
            return Err("payload máximo: 64 MiB".into());
        }
        if !matches!(
            parametros.modo.as_str(),
            "sin-cache" | "serial" | "frio" | "caliente"
        ) {
            return Err("modo válido: sin-cache, serial, frio, caliente".into());
        }
        Ok(parametros)
    }
}

/// Suma contadores entre muestras; conserva el máximo de los picos de concurrencia.
pub fn resumir_metricas(muestras: &[Value]) -> Value {
    let mut resumen = Map::new();
    for muestra in muestras {
        if let Some(metricas) = muestra["metricas_descargas"].as_object() {
            for (clave, valor) in metricas {
                if let Some(numero) = valor.as_u64() {
                    let previo = resumen.get(clave).and_then(Value::as_u64).unwrap_or(0);
                    let total = if clave.starts_with("pico_") {
                        previo.max(numero)
                    } else {
                        previo.saturating_add(numero)
                    };
                    resumen.insert(clave.clone(), Value::from(total));
                }
            }
        }
    }
    Value::Object(resumen)
}
