// SPDX-License-Identifier: GPL-3.0-only
use icaro_core::compartir::espacio;
#[test]
fn presupuesto_incluye_descarga_y_copia_y_rechaza_desbordamiento() {
    assert!(espacio::presupuesto(&[10, 20]).unwrap() >= 60);
    assert!(espacio::presupuesto(&[u64::MAX]).is_err());
    assert!(espacio::comprobar_disponible(100, 99).is_err());
    espacio::comprobar_disponible(100, 100).unwrap();
}
#[test]
fn espacio_real_se_consulta_en_directorio_temporal() {
    assert!(espacio::disponible(&std::env::temp_dir()).unwrap() > 0);
}

#[test]
fn limpieza_conserva_ajenos_recientes_y_descarga_reservada() {
    use icaro_core::compartir::mantenimiento;
    use std::{
        fs,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };
    let p = std::env::temp_dir().join(format!(
        "icaro-staging-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let guardia = mantenimiento::reservar(&p).unwrap();
    let hash = "a".repeat(128);
    fs::write(p.join(format!("{hash}.part")), b"parcial").unwrap();
    fs::write(p.join("personal.txt"), b"personal").unwrap();
    let lock = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(p.join(format!("{hash}.lock")))
        .unwrap();
    lock.try_lock().unwrap();
    assert_eq!(
        mantenimiento::limpiar(&p, SystemTime::now(), Duration::ZERO).unwrap(),
        0
    );
    drop(lock);
    assert_eq!(
        mantenimiento::limpiar(&p, SystemTime::now(), Duration::from_secs(7 * 86400)).unwrap(),
        0
    );
    assert_eq!(
        mantenimiento::limpiar(&p, SystemTime::now(), Duration::ZERO).unwrap(),
        1
    );
    assert_eq!(fs::read(p.join("personal.txt")).unwrap(), b"personal");
    assert!(mantenimiento::reservar(&p).is_err());
    drop(guardia);
    fs::remove_dir_all(p).unwrap();
}
