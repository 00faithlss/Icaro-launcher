// SPDX-License-Identifier: GPL-3.0-only
//! Contenido reutilizable; la autorización/manifiesto se consulta en el controlador.
use icaro_core::compartir::{
    self as c,
    aplicacion::{self, ContextoDestino},
    descargas::{self, OpcionesDescarga},
    inventario::Inventario,
    plan,
    transporte::{Cliente, Modo, NodoAnfitrion},
    Limites,
};
use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;
type Resultado<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
#[path = "../examples/banco_compartir/fixture.rs"]
#[allow(dead_code)]
mod fixture;
use fixture::Fixture;
async fn iniciar(f: &Fixture) -> (NodoAnfitrion, Cliente) {
    let n = NodoAnfitrion::iniciar(
        f.host.clone(),
        f.revision.clone(),
        f.acl.clone(),
        Arc::new(fixture::Permiso),
        Modo::Local,
        Limites::default(),
    )
    .await
    .unwrap();
    let cliente = Cliente::conectar(
        &f.guest,
        n.direccion(),
        &f.manifiesto.origen,
        Modo::Local,
        Limites::default(),
    )
    .await
    .unwrap();
    (n, cliente)
}
async fn instalar(
    f: &Fixture,
    m: &c::Manifiesto,
    cliente: &Cliente,
    opciones: &OpcionesDescarga,
    nombre: &str,
) -> Result<(), c::Error> {
    let juego = f.root.join(nombre);
    std::fs::create_dir_all(&juego).unwrap();
    let ctx = ContextoDestino {
        juego,
        limites: Default::default(),
    };
    let plan = plan::planificar(m, &Default::default(), &Inventario::default())?;
    let progreso = Mutex::new(Vec::new());
    let p = descargas::preparar_configurado(
        &ctx,
        plan,
        cliente,
        None,
        &f.root.join(format!("staging-{nombre}")),
        &CancellationToken::new(),
        opciones,
        |e| {
            if let descargas::EventoDescarga::Archivos { hechos, .. } = e {
                progreso.lock().unwrap().push(hechos);
            }
        },
    )
    .await?;
    let avances = progreso.into_inner().unwrap();
    assert!(avances.windows(2).all(|p| p[0] <= p[1]));
    assert_eq!(avances.last(), Some(&m.archivos.len()));
    aplicacion::aplicar(p, &CancellationToken::new(), |_| {})?;
    Ok(())
}
fn opciones(f: &Fixture) -> OpcionesDescarga {
    OpcionesDescarga {
        cache: Some(c::cache::CacheContenido::nueva(f.root.join("cache"))),
        ..Default::default()
    }
}
fn reescribir(f: &Fixture, m: &c::Manifiesto) {
    let ctx = c::revisiones::ContextoPublicacion {
        juego: f.root.join("juego"),
        almacen: f.root.join("publicacion"),
        origen: m.origen.clone(),
        perfil: m.perfil.clone(),
        limites: Default::default(),
    };
    std::fs::write(
        ctx.almacen.join("vigente.json"),
        serde_json::to_vec(m).unwrap(),
    )
    .unwrap();
    *f.revision.write().unwrap() = c::revisiones::vigente(&ctx).unwrap().unwrap();
}
#[tokio::test]
async fn deduplica_dos_rutas_y_segunda_instancia_no_descarga() {
    let mut f = Fixture::crear(2, 64 * 1024).unwrap();
    let mut copia = f.manifiesto.archivos[0].clone();
    copia.ruta = c::RutaCompartida::parse("config/copia.bin").unwrap();
    f.manifiesto.archivos.push(copia);
    reescribir(&f, &f.manifiesto);
    let (n, cliente) = iniciar(&f).await;
    let frio = opciones(&f);
    instalar(&f, &f.manifiesto, &cliente, &frio, "uno")
        .await
        .unwrap();
    f.verificar(&f.root.join("uno")).unwrap();
    let e = frio.metricas.lectura();
    assert_eq!((e.archivos, e.objetos, e.duplicados_evitados), (3, 2, 1));
    assert_eq!(e.descargas_propias, 2);
    assert_eq!(e.payload_red_verificado, 128 * 1024);
    n.parar().await;
    // Sin servidor de archivos ni catálogo: estos bytes ya están verificados.
    // El controlador normal sigue exigiendo autorización remota antes de entrar.
    let caliente = opciones(&f);
    instalar(&f, &f.manifiesto, &cliente, &caliente, "dos")
        .await
        .unwrap();
    f.verificar(&f.root.join("dos")).unwrap();
    let e = caliente.metricas.lectura();
    assert_eq!(e.aciertos_cache, 2);
    assert_eq!(e.payload_red_verificado, 0);
    assert_eq!(e.descargas_propias, 0);
    assert_eq!(e.lotes_catalogo, 0);
    // Copias independientes: modificar una instancia no altera CAS ni la otra.
    std::fs::write(f.root.join("uno/config/copia.bin"), b"personal").unwrap();
    f.verificar(&f.root.join("dos")).unwrap();
    cliente.cerrar().await;
}
#[tokio::test]
async fn cache_corrupta_se_repara_sin_tocar_mundo_personal() {
    let f = Fixture::crear(1, 64 * 1024).unwrap();
    let (n, cliente) = iniciar(&f).await;
    instalar(&f, &f.manifiesto, &cliente, &opciones(&f), "uno")
        .await
        .unwrap();
    let objeto = f
        .root
        .join("cache/objetos")
        .join(&f.manifiesto.archivos[0].sha512);
    std::fs::write(&objeto, vec![0u8; 64 * 1024]).unwrap();
    std::fs::create_dir_all(f.root.join("dos/worlds/mio")).unwrap();
    std::fs::write(f.root.join("dos/worlds/mio/nivel"), b"conservar").unwrap();
    let reparacion = opciones(&f);
    instalar(&f, &f.manifiesto, &cliente, &reparacion, "dos")
        .await
        .unwrap();
    assert_eq!(reparacion.metricas.lectura().descargas_propias, 1);
    f.verificar(&f.root.join("dos")).unwrap();
    assert_eq!(
        std::fs::read(f.root.join("dos/worlds/mio/nivel")).unwrap(),
        b"conservar"
    );
    cliente.cerrar().await;
    n.parar().await;
}
#[tokio::test]
async fn origen_propio_no_acredita_modrinth_y_binding_confirmado_evita_catalogo() {
    let f = Fixture::crear(1, 64 * 1024).unwrap();
    let (n, cliente) = iniciar(&f).await;
    instalar(&f, &f.manifiesto, &cliente, &opciones(&f), "uno")
        .await
        .unwrap();
    let mut remoto = f.manifiesto.clone();
    remoto.archivos[0].procedencia = c::Procedencia::Modrinth {
        proyecto: "proyecto1".into(),
        version: "version1".into(),
    };
    let fallo = instalar(&f, &remoto, &cliente, &opciones(&f), "dos").await;
    assert!(matches!(
        fallo,
        Err(c::Error::Datos("catálogo no disponible"))
    ));
    // Simula la aprobación del catálogo; este test no acredita un servicio oficial.
    let cache = opciones(&f).cache.unwrap();
    let cancel = CancellationToken::new();
    let uso = cache.sesion(&cancel).await.unwrap();
    let a = &remoto.archivos[0];
    let reserva = cache
        .reservar(&a.sha512, a.bytes, uso, &cancel)
        .await
        .unwrap();
    assert!(reserva.buscar(&cancel).await.unwrap().is_some());
    reserva
        .confirmar_procedencia(&a.procedencia, &cancel)
        .await
        .unwrap();
    drop(reserva);
    let aprobado = opciones(&f);
    instalar(&f, &remoto, &cliente, &aprobado, "tres")
        .await
        .unwrap();
    assert_eq!(aprobado.metricas.lectura().lotes_catalogo, 0);
    assert_eq!(aprobado.metricas.lectura().payload_red_verificado, 0);
    f.verificar(&f.root.join("tres")).unwrap();
    cliente.cerrar().await;
    n.parar().await;
}
#[tokio::test]
async fn dos_operaciones_comparten_productor_y_progreso_es_monotono() {
    let archivos = 32;
    let f = Fixture::crear(archivos, 128 * 1024).unwrap();
    let (n, cliente) = iniciar(&f).await;
    let a = opciones(&f);
    let b = opciones(&f);
    let (ra, rb) = tokio::join!(
        instalar(&f, &f.manifiesto, &cliente, &a, "uno"),
        instalar(&f, &f.manifiesto, &cliente, &b, "dos")
    );
    ra.unwrap();
    rb.unwrap();
    assert_eq!(
        a.metricas.lectura().descargas_propias + b.metricas.lectura().descargas_propias,
        archivos as u64
    );
    f.verificar(&f.root.join("uno")).unwrap();
    f.verificar(&f.root.join("dos")).unwrap();
    assert!(a.metricas.lectura().pico_propias <= 2 && b.metricas.lectura().pico_propias <= 2);
    cliente.cerrar().await;
    n.parar().await;
}

#[tokio::test]
async fn fallo_de_un_archivo_no_aplica_los_otros_ni_modifica_mundos() {
    let f = Fixture::crear(3, 512 * 1024).unwrap();
    let (n, cliente) = iniciar(&f).await;
    std::fs::remove_file(
        f.root
            .join("publicacion/contenido")
            .join(&f.manifiesto.archivos[0].sha512),
    )
    .unwrap();
    let juego = f.root.join("fallo");
    std::fs::create_dir_all(juego.join("worlds/mio")).unwrap();
    std::fs::write(juego.join("worlds/mio/nivel"), b"personal").unwrap();
    let r = instalar(&f, &f.manifiesto, &cliente, &opciones(&f), "fallo").await;
    assert!(r.is_err());
    assert_eq!(
        std::fs::read(juego.join("worlds/mio/nivel")).unwrap(),
        b"personal"
    );
    assert!(!juego.join("config").exists());
    assert!(aplicacion::cargar_base(&ContextoDestino {
        juego,
        limites: Default::default()
    })
    .unwrap()
    .manifiesto
    .is_none());
    cliente.cerrar().await;
    n.parar().await;
}
