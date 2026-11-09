// SPDX-License-Identifier: GPL-3.0-only
//! Banco local aislado. Ejemplo: --muestras 7 --archivos 32 --bytes 262144.
//! No usa cuentas, APPDATA, catálogo, Worker ni conexiones fuera de loopback.
#[path = "banco_compartir/fixture.rs"]
mod fixture;
#[path = "banco_compartir/memoria.rs"]
mod memoria;
#[path = "banco_compartir/parametros.rs"]
mod parametros;

use icaro_core::compartir::{
    aplicacion::{self, ContextoDestino},
    cache::CacheContenido,
    descargas::{self, EventoDescarga},
    inventario::Inventario,
    plan,
    transporte::{Cliente, Modo, NodoAnfitrion},
    Limites,
};
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
use tokio_util::sync::CancellationToken;

type Resultado<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

fn main() -> Result<(), String> {
    // El futuro de iroh en debug necesita la misma pila amplia que el E2E.
    std::thread::Builder::new()
        .name("banco-compartir".into())
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .thread_stack_size(8 * 1024 * 1024)
                .enable_all()
                .build()
                .map_err(|e| e.to_string())?;
            let reporte = runtime.block_on(ejecutar()).map_err(|e| e.to_string())?;
            println!(
                "{}",
                serde_json::to_string_pretty(&reporte).map_err(|e| e.to_string())?
            );
            Ok(())
        })
        .map_err(|e| e.to_string())?
        .join()
        .map_err(|_| "el banco terminó con panic".to_string())?
}

async fn ejecutar() -> Resultado<Value> {
    let parametros = parametros::Parametros::leer()?;
    let fixture = fixture::Fixture::crear(parametros.archivos, parametros.bytes)?;
    let nodo = Arc::new(
        NodoAnfitrion::iniciar(
            fixture.host.clone(),
            fixture.revision.clone(),
            fixture.acl.clone(),
            Arc::new(fixture::Permiso),
            Modo::Local,
            Limites::default(),
        )
        .await?,
    );
    let conexion = Cliente::conectar(
        &fixture.guest,
        nodo.direccion(),
        &fixture.manifiesto.origen,
        Modo::Local,
        Limites::default(),
    )
    .await;
    let cliente = match conexion {
        Ok(c) => c,
        Err(e) => {
            nodo.parar().await;
            return Err(e.into());
        }
    };
    let resultado = medir(&fixture, &cliente, &nodo, &parametros).await;
    cliente.cerrar().await;
    nodo.parar().await;
    resultado
}

async fn medir(
    f: &fixture::Fixture,
    cliente: &Cliente,
    nodo: &NodoAnfitrion,
    parametros: &parametros::Parametros,
) -> Resultado<Value> {
    let remoto = cliente.consultar().await?;
    let muestras = parametros.muestras;
    let mut resultados = Vec::new();
    let mut tiempos = Vec::new();
    let caliente = CacheContenido::nueva(f.root.join("cache-caliente"));
    // La primera aplicación calienta objetos verificados sin entrar en las muestras.
    let calentamiento = usize::from(parametros.modo == "caliente");
    for indice in 0..muestras + calentamiento {
        let es_calentamiento = calentamiento == 1 && indice == 0;
        let mut opciones = descargas::OpcionesDescarga::default();
        opciones.cache = match parametros.modo.as_str() {
            "caliente" => Some(caliente.clone()),
            "frio" => Some(CacheContenido::nueva(
                f.root.join(format!("cache-fria-{indice}")),
            )),
            _ => None,
        };
        if parametros.modo == "serial" {
            opciones.limite_anfitrion = 1;
            opciones.limite_cdn = 1;
        }
        let destino = f.root.join(format!("destino-{indice}"));
        let staging = f.root.join(format!("staging-{indice}"));
        std::fs::create_dir(&destino)?;
        std::fs::create_dir(&staging)?;
        let contexto = ContextoDestino {
            juego: destino.clone(),
            limites: Limites::default(),
        };
        let plan = plan::planificar(&remoto, &Default::default(), &Inventario::default())?;
        let cancel = CancellationToken::new();
        let terminados = AtomicUsize::new(0);
        let fin = CancellationToken::new();
        let monitor = async {
            let mut maximo = 0;
            loop {
                maximo = maximo.max(nodo.contar_transferencias());
                tokio::select! {
                    _ = fin.cancelled() => break,
                    _ = tokio::time::sleep(Duration::from_millis(1)) => (),
                }
            }
            maximo
        };
        let inicio = Instant::now();
        let operacion = async {
            let preparado = descargas::preparar_configurado(
                &contexto,
                plan,
                cliente,
                None,
                &staging,
                &cancel,
                &opciones,
                |evento| {
                    if let EventoDescarga::Archivos { hechos, .. } = evento {
                        terminados.fetch_max(hechos, Ordering::Relaxed);
                    }
                },
            )
            .await;
            let descarga_ms = inicio.elapsed().as_secs_f64() * 1000.0;
            let resultado = preparado.and_then(|p| aplicacion::aplicar(p, &cancel, |_| {}));
            let total_ms = inicio.elapsed().as_secs_f64() * 1000.0;
            fin.cancel();
            (resultado, descarga_ms, total_ms)
        };
        let ((resultado, descarga_ms, total_ms), maximo) = tokio::join!(operacion, monitor);
        resultado?;
        // Integridad final fuera de la ventana de medida.
        f.verificar(&destino)?;
        let completados = terminados.load(Ordering::Relaxed);
        if completados != remoto.archivos.len() {
            return Err("progreso incompleto".into());
        }
        // Estas rutas se crean exclusivamente bajo la raíz privada del fixture.
        std::fs::remove_dir_all(&destino)?;
        std::fs::remove_dir_all(&staging)?;
        if parametros.modo == "frio" {
            std::fs::remove_dir_all(f.root.join(format!("cache-fria-{indice}")))?;
        }
        if es_calentamiento {
            continue;
        }
        tiempos.push(total_ms);
        resultados.push(
            json!({"indice": indice + 1 - calentamiento, "total_ms": total_ms,
            "preparar_ms": descarga_ms, "aplicar_ms": total_ms - descarga_ms,
            "archivos_verificados_progreso": completados,
            "payload_total_declared": remoto.archivos.iter().map(|a| a.bytes).sum::<u64>(),
            "max_transferencias_observadas_muestreo_1ms": maximo,
            "ram_peak_proceso_bytes": memoria::pico(),
            "metricas_descargas": opciones.metricas.lectura()}),
        );
    }
    tiempos.sort_by(f64::total_cmp);
    let mediana = if muestras % 2 == 0 {
        (tiempos[muestras / 2 - 1] + tiempos[muestras / 2]) / 2.0
    } else {
        tiempos[muestras / 2]
    };
    Ok(json!({"esquema": "icaro-banco-compartir/1",
        "modo": parametros.modo,
        "perfil": if cfg!(debug_assertions) { "debug-hashesopt" } else { "release" },
        "escenario": "instancia_nueva_staging_nuevo_conexion_reutilizada_iroh_loopback",
        "archivos": remoto.archivos.len(), "bytes_por_archivo": parametros.bytes,
        "payload_total_declared": remoto.archivos.iter().map(|a| a.bytes).sum::<u64>(),
        "wire_bytes_medidos": null, "mediana_ms": mediana,
        "p95_ms": tiempos[(muestras * 95).div_ceil(100).saturating_sub(1)],
        "resumen_metricas_descargas": parametros::resumir_metricas(&resultados),
        "muestras": resultados, "calentamientos_no_medidos": calentamiento,
        "limites": "caches del SO calientes; RAM pico de proceso completo acumulado; monitor 1ms puede omitir transferencias breves; no mide Internet ni partida real"}))
}
