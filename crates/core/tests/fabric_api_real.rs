// SPDX-License-Identifier: GPL-3.0-only
use icaro_core::{instancias::Almacen, mods};
use sha2::{Digest, Sha512};
use tokio_util::sync::CancellationToken;

#[tokio::test]
#[ignore = "Descarga Fabric API oficial de Modrinth en una instancia temporal; no inicia el juego"]
async fn fabric_api_oficial_instala_y_reutiliza_el_mod_verificado() {
    let mut nonce = [0u8; 16];
    getrandom::fill(&mut nonce).unwrap();
    let root = std::env::temp_dir().join(format!("icaro-api-real-{:x}", Sha512::digest(nonce)));
    let store = Almacen::nuevo(root.join("instances"));
    let i = store
        .crear_con_loader("Fabric", "1.21.1", "fabric", 0)
        .unwrap();
    mods::fabric_api::instalar_por_defecto(&store, &i, CancellationToken::new(), |_| {})
        .await
        .unwrap();
    let inventory = mods::inventario(&store, &i.id).unwrap();
    let api = inventory
        .iter()
        .find(|m| m.project_id == "P7dR8mSH")
        .expect("Fabric API para Minecraft 1.21.1");
    assert!(api.activo);
    let path = store.juego(&i.id).unwrap().join("mods").join(&api.archivo);
    assert_eq!(
        format!("{:x}", Sha512::digest(std::fs::read(path).unwrap())),
        api.sha512
    );
    mods::fabric_api::instalar_por_defecto(&store, &i, CancellationToken::new(), |_| {
        panic!("El mod existente se conserva")
    })
    .await
    .unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
