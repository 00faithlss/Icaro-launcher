// SPDX-License-Identifier: GPL-3.0-only
use super::*;
#[test]
fn inventario_no_publica_y_no_duplica_pack_integrado() {
    let d = Carpeta::nueva();
    // El inventario enumera archivos; no extrae los recursos internos del JAR.
    d.escribir("mods/punchy.jar", b"jar con pack integrado");
    d.escribir("resourcepacks/externo.zip", b"pack externo");
    d.escribir("config/mod.toml", b"opcion=true");
    d.escribir("kubejs/server_scripts/receta.js", b"receta");
    d.escribir("saves/mundo/level.dat", b"personal");
    d.escribir("options.txt", b"volumen:0.5");
    d.escribir("mods/.icaro-mods.json", b"registro interno");
    let antes = fs::read(d.0.join("options.txt")).unwrap();
    let inv = inventario::analizar(&d.0, &CancellationToken::new(), &Limites::default()).unwrap();
    assert_eq!(inv.archivos.len(), 4);
    assert!(inv.archivos.windows(2).all(|a| a[0].ruta < a[1].ruta));
    let jar = inv
        .archivos
        .iter()
        .find(|a| a.ruta.as_str() == "mods/punchy.jar")
        .unwrap();
    assert_eq!(
        jar.sha512,
        format!("{:x}", Sha512::digest(b"jar con pack integrado"))
    );
    assert_eq!(fs::read(d.0.join("options.txt")).unwrap(), antes);
    assert!(!d.0.join(".icaro-compartir").exists());
}

#[test]
fn cancelar_y_exceder_limites_no_publican_archivos() {
    let d = Carpeta::nueva();
    d.escribir("mods/a.jar", b"contenido");
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(inventario::analizar(&d.0, &cancel, &Limites::default()).is_err());
    let limites = Limites {
        archivo_bytes: 1,
        ..Limites::default()
    };
    assert!(inventario::analizar(&d.0, &CancellationToken::new(), &limites).is_err());
    assert_eq!(fs::read(d.0.join("mods/a.jar")).unwrap(), b"contenido");
    assert_eq!(fs::read_dir(&d.0).unwrap().count(), 1);
}

#[test]
fn una_configuracion_privada_prohibida_no_bloquea_compartir_mods() {
    let d = Carpeta::nueva();
    d.escribir("mods/valido.jar", b"jar");
    d.escribir("config/.env", b"secreto");
    d.escribir("config/credentials.json", b"token");
    let inv = inventario::analizar(&d.0, &CancellationToken::new(), &Limites::default()).unwrap();
    assert_eq!(inv.archivos.len(), 1);
    assert_eq!(inv.archivos[0].ruta.as_str(), "mods/valido.jar");
    assert_eq!(inv.bloqueados.len(), 2);
    assert_eq!(fs::read(d.0.join("config/.env")).unwrap(), b"secreto");
}

#[cfg(windows)]
#[test]
fn inventario_rechaza_union_sin_recorrer_el_destino() {
    let d = Carpeta::nueva();
    let fuera = Carpeta::nueva();
    fuera.escribir("personal.txt", b"no compartir");
    let junction = d.0.join("config");
    // Crear una unión NTFS no requiere privilegios de enlaces simbólicos.
    let estado = std::process::Command::new("cmd.exe")
        .args(["/C", "mklink", "/J"])
        .arg(&junction)
        .arg(&fuera.0)
        .output()
        .unwrap();
    assert!(
        estado.status.success(),
        "{}",
        String::from_utf8_lossy(&estado.stderr)
    );
    let resultado = inventario::analizar(&d.0, &CancellationToken::new(), &Limites::default());
    fs::remove_dir(&junction).unwrap();
    assert!(resultado.is_err());
    assert_eq!(
        fs::read(fuera.0.join("personal.txt")).unwrap(),
        b"no compartir"
    );
}
