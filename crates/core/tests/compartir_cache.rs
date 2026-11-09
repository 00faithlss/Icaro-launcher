// SPDX-License-Identifier: GPL-3.0-only
//! El CAS es una optimización: nunca sustituye la comprobación del contenido.
use icaro_core::compartir::{cache::CacheContenido, Error, Limites, Manifiesto, Procedencia};
use sha2::{Digest, Sha512};
use std::{fs, path::PathBuf, time::Duration};
use tokio_util::sync::CancellationToken;
#[path = "compartir/comun.rs"]
#[allow(dead_code)]
mod comun;
use comun::Carpeta;

fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha512::digest(bytes))
}

async fn publicar(cache: &CacheContenido, bytes: &[u8]) -> PathBuf {
    let cancel = CancellationToken::new();
    let uso = cache.sesion(&cancel).await.unwrap();
    let reserva = cache
        .reservar(&sha(bytes), bytes.len() as u64, uso, &cancel)
        .await
        .unwrap();
    fs::create_dir_all(reserva.staging()).unwrap();
    fs::write(
        reserva.staging().join(format!("{}.part", sha(bytes))),
        bytes,
    )
    .unwrap();
    reserva.publicar(&cancel).await.unwrap()
}

#[tokio::test]
async fn nueva_no_crea_directorios_y_publicar_reutiliza_contenido_verificado() {
    let root = Carpeta::nueva();
    let carpeta = root.0.join("cache-contenido");
    let cache = CacheContenido::nueva(carpeta.clone());
    assert!(!carpeta.exists());
    let bytes = b"contenido compartido";
    let path = publicar(&cache, bytes).await;
    assert_eq!(
        path.canonicalize().unwrap(),
        carpeta
            .join("objetos")
            .join(sha(bytes))
            .canonicalize()
            .unwrap()
    );
    assert_eq!(fs::read(&path).unwrap(), bytes);
    let cancel = CancellationToken::new();
    let uso = cache.sesion(&cancel).await.unwrap();
    let reserva = cache
        .reservar(&sha(bytes), bytes.len() as u64, uso, &cancel)
        .await
        .unwrap();
    assert_eq!(reserva.buscar(&cancel).await.unwrap(), Some(path));
    assert!(!reserva
        .staging()
        .join(format!("{}.part", sha(bytes)))
        .exists());
}

#[tokio::test]
async fn publicar_rechaza_hash_o_tamano_incorrectos_sin_crear_objeto() {
    let root = Carpeta::nueva();
    let carpeta = root.0.join("cache");
    let cache = CacheContenido::nueva(carpeta.clone());
    let cancel = CancellationToken::new();
    for incorrecto in [b"otro".as_slice(), b"incompleto".as_slice()] {
        let uso = cache.sesion(&cancel).await.unwrap();
        let hash = sha(b"bien");
        let reserva = cache.reservar(&hash, 4, uso, &cancel).await.unwrap();
        fs::create_dir_all(reserva.staging()).unwrap();
        fs::write(reserva.staging().join(format!("{hash}.part")), incorrecto).unwrap();
        assert!(reserva.publicar(&cancel).await.is_err());
        assert!(!carpeta.join("objetos").join(hash).exists());
    }
}

#[tokio::test]
async fn corrupcion_de_igual_tamano_no_es_un_acierto_de_cache() {
    let root = Carpeta::nueva();
    let cache = CacheContenido::nueva(root.0.join("cache"));
    let path = publicar(&cache, b"bien").await;
    fs::write(path, b"otro").unwrap();
    let cancel = CancellationToken::new();
    let uso = cache.sesion(&cancel).await.unwrap();
    let reserva = cache
        .reservar(&sha(b"bien"), 4, uso, &cancel)
        .await
        .unwrap();
    assert_eq!(reserva.buscar(&cancel).await.unwrap(), None);
}

#[tokio::test]
async fn esperar_hash_ocupado_se_puede_cancelar_y_el_productor_se_conserva() {
    let root = Carpeta::nueva();
    let cache = CacheContenido::nueva(root.0.join("cache"));
    let cancel = CancellationToken::new();
    let uso = cache.sesion(&cancel).await.unwrap();
    let hash = sha(b"bien");
    let productor = cache
        .reservar(&hash, 4, uso.clone(), &cancel)
        .await
        .unwrap();
    let otro = CacheContenido::nueva(root.0.join("cache"));
    let espera = CancellationToken::new();
    let token = espera.clone();
    let esperando = otro.reservar(&hash, 4, uso, &espera);
    tokio::pin!(esperando);
    tokio::select! {
        _ = &mut esperando => panic!("el segundo productor no debe saltarse el bloqueo"),
        _ = tokio::time::sleep(Duration::from_millis(40)) => token.cancel(),
    }
    let resultado = tokio::time::timeout(Duration::from_secs(2), esperando)
        .await
        .unwrap();
    assert!(matches!(resultado, Err(Error::Cancelada)));
    fs::create_dir_all(productor.staging()).unwrap();
    fs::write(productor.staging().join(format!("{hash}.part")), b"bien").unwrap();
    assert_eq!(
        fs::read(productor.publicar(&cancel).await.unwrap()).unwrap(),
        b"bien"
    );
}

#[tokio::test]
async fn segundo_productor_reutiliza_lo_publicado_al_liberarse_el_hash() {
    let root = Carpeta::nueva();
    let cache = CacheContenido::nueva(root.0.join("cache"));
    let cancel = CancellationToken::new();
    let uso = cache.sesion(&cancel).await.unwrap();
    let hash = sha(b"bien");
    let productor = cache
        .reservar(&hash, 4, uso.clone(), &cancel)
        .await
        .unwrap();
    let otro = CacheContenido::nueva(root.0.join("cache"));
    let esperando = otro.reservar(&hash, 4, uso, &cancel);
    tokio::pin!(esperando);
    tokio::select! {
        _ = &mut esperando => panic!("el hash todavía tiene un productor"),
        _ = tokio::time::sleep(Duration::from_millis(40)) => {},
    }
    fs::create_dir_all(productor.staging()).unwrap();
    fs::write(productor.staging().join(format!("{hash}.part")), b"bien").unwrap();
    let path = productor.publicar(&cancel).await.unwrap();
    drop(productor);
    let reserva = tokio::time::timeout(Duration::from_secs(2), esperando)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reserva.buscar(&cancel).await.unwrap(), Some(path));
}

#[tokio::test]
async fn lease_activa_impide_eviction_hasta_terminar_la_preparacion() {
    let root = Carpeta::nueva();
    let carpeta = root.0.join("cache");
    let cache = CacheContenido::nueva(carpeta.clone());
    let path = publicar(&cache, b"bien").await;
    let cancel = CancellationToken::new();
    let uso = cache.sesion(&cancel).await.unwrap();
    let limpiador = CacheContenido::nueva(carpeta).con_limites(0, Duration::ZERO);
    tokio::time::timeout(Duration::from_secs(2), limpiador.limpiar(&cancel))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fs::read(&path).unwrap(), b"bien");
    drop(uso);
    limpiador.limpiar(&cancel).await.unwrap();
    assert!(!path.exists());
}

#[tokio::test]
async fn cuota_retira_el_menos_usado_y_ttl_retira_el_restante() {
    let root = Carpeta::nueva();
    let carpeta = root.0.join("cache");
    let cache = CacheContenido::nueva(carpeta.clone());
    let primero = publicar(&cache, b"uno!").await;
    tokio::time::sleep(Duration::from_millis(1100)).await;
    let segundo = publicar(&cache, b"dos!").await;
    tokio::time::sleep(Duration::from_millis(1100)).await;
    let cancel = CancellationToken::new();
    {
        let uso = cache.sesion(&cancel).await.unwrap();
        let reserva = cache
            .reservar(&sha(b"uno!"), 4, uso, &cancel)
            .await
            .unwrap();
        assert_eq!(
            reserva.buscar(&cancel).await.unwrap(),
            Some(primero.clone())
        );
    }
    CacheContenido::nueva(carpeta.clone())
        .con_limites(4, Duration::from_secs(7 * 86400))
        .limpiar(&cancel)
        .await
        .unwrap();
    assert!(primero.exists());
    assert!(!segundo.exists());
    CacheContenido::nueva(carpeta)
        .con_limites(u64::MAX, Duration::ZERO)
        .limpiar(&cancel)
        .await
        .unwrap();
    assert!(!primero.exists());
}

#[tokio::test]
async fn limpieza_no_toca_publicaciones_instancias_ni_archivos_desconocidos() {
    let root = Carpeta::nueva();
    let carpeta = root.0.join("compartir/cache-contenido");
    let cache = CacheContenido::nueva(carpeta.clone());
    let path = publicar(&cache, b"bien").await;
    let hash = sha(b"bien");
    let ajenos = [
        root.0.join("instances/real/minecraft/mods").join(&hash),
        root.0
            .join("compartir/publicaciones/vigente/contenido")
            .join(&hash),
        carpeta.join("personal.txt"),
        carpeta.join("objetos/no-es-un-hash"),
        carpeta.join("parciales/ajeno.part"),
    ];
    for ajeno in &ajenos {
        fs::create_dir_all(ajeno.parent().unwrap()).unwrap();
        fs::write(ajeno, b"conservar").unwrap();
    }
    CacheContenido::nueva(carpeta)
        .con_limites(0, Duration::ZERO)
        .limpiar(&CancellationToken::new())
        .await
        .unwrap();
    assert!(!path.exists());
    for ajeno in ajenos {
        assert_eq!(fs::read(ajeno).unwrap(), b"conservar");
    }
}

#[tokio::test]
async fn procedencia_modrinth_exige_confirmacion_exacta_y_persistente() {
    let root = Carpeta::nueva();
    let carpeta = root.0.join("cache");
    let cache = CacheContenido::nueva(carpeta.clone());
    publicar(&cache, b"bien").await;
    let cancel = CancellationToken::new();
    let procedencia = Procedencia::Modrinth {
        proyecto: "proyecto".into(),
        version: "version".into(),
    };
    {
        let uso = cache.sesion(&cancel).await.unwrap();
        let reserva = cache
            .reservar(&sha(b"bien"), 4, uso, &cancel)
            .await
            .unwrap();
        assert!(reserva
            .procedencia_confirmada(&Procedencia::Propio, &cancel)
            .await
            .unwrap());
        assert!(!reserva
            .procedencia_confirmada(&procedencia, &cancel)
            .await
            .unwrap());
        reserva
            .confirmar_procedencia(&procedencia, &cancel)
            .await
            .unwrap();
    }
    let cache = CacheContenido::nueva(carpeta);
    let uso = cache.sesion(&cancel).await.unwrap();
    let reserva = cache
        .reservar(&sha(b"bien"), 4, uso, &cancel)
        .await
        .unwrap();
    assert!(reserva
        .procedencia_confirmada(&procedencia, &cancel)
        .await
        .unwrap());
    for diferente in [
        Procedencia::Modrinth {
            proyecto: "otro".into(),
            version: "version".into(),
        },
        Procedencia::Modrinth {
            proyecto: "proyecto".into(),
            version: "otra".into(),
        },
    ] {
        assert!(!reserva
            .procedencia_confirmada(&diferente, &cancel)
            .await
            .unwrap());
    }
    drop(reserva);
    let uso = cache.sesion(&cancel).await.unwrap();
    let tamano_distinto = cache
        .reservar(&sha(b"bien"), 5, uso, &cancel)
        .await
        .unwrap();
    assert!(!tamano_distinto
        .procedencia_confirmada(&procedencia, &cancel)
        .await
        .unwrap());
}

#[tokio::test]
async fn hash_malformado_no_puede_salir_del_cache() {
    let root = Carpeta::nueva();
    let cache = CacheContenido::nueva(root.0.join("cache"));
    let cancel = CancellationToken::new();
    let uso = cache.sesion(&cancel).await.unwrap();
    for hash in [
        String::new(),
        "../personal".into(),
        "a".repeat(127),
        "g".repeat(128),
    ] {
        assert!(cache
            .reservar(&hash, 4, uso.clone(), &cancel)
            .await
            .is_err());
    }
    assert!(!root.0.join("personal").exists());
}

#[tokio::test]
async fn una_lease_de_otra_raiz_no_autoriza_reservar_contenido() {
    let root = Carpeta::nueva();
    let primera = CacheContenido::nueva(root.0.join("primera"));
    let segunda = CacheContenido::nueva(root.0.join("segunda"));
    let cancel = CancellationToken::new();
    let uso = primera.sesion(&cancel).await.unwrap();
    let otra = segunda.sesion(&cancel).await.unwrap();
    assert!(segunda
        .reservar(&sha(b"bien"), 4, uso, &cancel)
        .await
        .is_err());
    drop(otra);
}

#[tokio::test]
async fn no_se_confirma_procedencia_sin_objeto_verificado() {
    let root = Carpeta::nueva();
    let cache = CacheContenido::nueva(root.0.join("cache"));
    let cancel = CancellationToken::new();
    let uso = cache.sesion(&cancel).await.unwrap();
    let reserva = cache
        .reservar(&sha(b"bien"), 4, uso, &cancel)
        .await
        .unwrap();
    let procedencia = Procedencia::Modrinth {
        proyecto: "p".into(),
        version: "v".into(),
    };
    assert!(reserva
        .confirmar_procedencia(&procedencia, &cancel)
        .await
        .is_err());
    assert!(!reserva
        .procedencia_confirmada(&procedencia, &cancel)
        .await
        .unwrap());
}

#[tokio::test]
async fn cuota_cubre_parciales_pero_respeta_bloqueo_del_escritor() {
    let root = Carpeta::nueva();
    let carpeta = root.0.join("cache");
    let cache = CacheContenido::nueva(carpeta.clone());
    let cancel = CancellationToken::new();
    let uso = cache.sesion(&cancel).await.unwrap();
    drop(uso);
    let hash = sha(b"bien");
    let parcial = carpeta.join("parciales").join(format!("{hash}.part"));
    fs::write(&parcial, b"bien").unwrap();
    let lock = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(parcial.with_extension("lock"))
        .unwrap();
    lock.try_lock().unwrap();
    let limpiador = cache.con_limites(0, Duration::from_secs(7 * 86400));
    limpiador.limpiar(&cancel).await.unwrap();
    assert!(parcial.exists());
    drop(lock);
    limpiador.limpiar(&cancel).await.unwrap();
    assert!(!parcial.exists());
}
