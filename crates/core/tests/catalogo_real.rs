// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

use icaro_core::modrinth::{Catalog, Kind, Search};

#[tokio::test]
#[ignore = "Consulta de lectura a Modrinth, su CDN y avatares GitHub; requiere red"]
async fn creadores_cargan_avatares_publicados_y_organizacion() {
    let catalog = Catalog::new().unwrap();
    let project = catalog.project("AANobbMI").await.unwrap();
    let members = catalog.members(&project.id).await.unwrap();
    assert!(!members.is_empty());
    for member in members {
        if let Some(avatar) = member.user.avatar_url {
            let bytes = catalog.image(&avatar).await.unwrap_or_else(|error|
                panic!("Avatar de {}: {error}", member.user.username));
            assert!(!bytes.is_empty() && bytes.len() <= 4 * 1024 * 1024);
        }
    }
    let organization = catalog.organization(project.organization.as_deref().unwrap()).await.unwrap();
    let bytes = catalog.image(organization.icon_url.as_deref().unwrap()).await.unwrap();
    assert!(!bytes.is_empty() && bytes.len() <= 4 * 1024 * 1024);
}

#[tokio::test]
#[ignore = "Consulta de lectura a la API pública de Modrinth; requiere red"]
async fn consulta_catalogo_proyecto_y_versiones_compatibles() {
    let catalog = Catalog::new().unwrap();
    let results = catalog
        .search(&Search {
            query: "sodium".into(),
            kind: Kind::Mod,
            minecraft: Some("1.21.4".into()),
            loader: Some("fabric".into()),
            offset: 0,
            limit: icaro_core::modrinth::PAGE_SIZE,
            order: Default::default(),
        })
        .await
        .unwrap();
    let hit = results
        .hits
        .iter()
        .find(|hit| hit.slug == "sodium")
        .expect("Sodium en el catálogo público");
    let project = catalog.project(&hit.project_id).await.unwrap();
    assert_eq!(project.id, hit.project_id);
    let versions = catalog
        .versions(&project.id, Some("1.21.4"), Some("fabric"))
        .await
        .unwrap();
    assert!(!versions.is_empty());
    let full = catalog.version(&versions[0].id).await.unwrap();
    assert_eq!(full.project_id, project.id);
    assert!(!full.date_published.is_empty());
    assert!(full
        .changelog
        .as_deref()
        .is_some_and(|c| !c.trim().is_empty()));
    assert!(project.game_versions.iter().any(|v| v == "1.21.4"));
    assert!(project.loaders.iter().any(|l| l == "fabric"));
    assert!(!catalog.members(&project.id).await.unwrap().is_empty());
    assert!(versions.iter().all(
        |version| version.game_versions.iter().any(|v| v == "1.21.4")
            && version.loaders.iter().any(|l| l == "fabric")
    ));
    assert!(versions.iter().all(|version| version
        .files
        .iter()
        .any(|f| f.primary && f.hashes.sha512.len() == 128)));
}

#[tokio::test]
#[ignore = "Consulta de lectura a Modrinth y su CDN; requiere red"]
async fn modpacks_tienen_filtros_versiones_e_imagen_real() {
    let catalog = Catalog::new().unwrap();
    let tags = catalog.tags().await.unwrap();
    assert!(tags
        .versions
        .iter()
        .any(|v| v.version == "1.21.4" && v.version_type == "release"));
    assert!(tags
        .loaders
        .iter()
        .any(|l| l.name == "fabric" && l.supported_project_types.iter().any(|p| p == "modpack")));
    let results = catalog
        .search(&Search {
            query: "Fabulously Optimized".into(),
            kind: Kind::Modpack,
            minecraft: Some("1.21.4".into()),
            loader: Some("fabric".into()),
            offset: 0,
            limit: icaro_core::modrinth::PAGE_SIZE,
            order: icaro_core::modrinth::Order::Downloads,
        })
        .await
        .unwrap();
    let hit = results
        .hits
        .iter()
        .find(|h| h.slug == "fabulously-optimized")
        .unwrap();
    assert!(hit.categories.iter().any(|c| c == "fabric"));
    assert!(hit.versions.iter().any(|v| v == "1.21.4"));
    let bytes = catalog
        .image(hit.icon_url.as_deref().unwrap())
        .await
        .unwrap();
    assert!(!bytes.is_empty() && bytes.len() <= 4 * 1024 * 1024);
    let versions = catalog
        .versions(&hit.project_id, Some("1.21.4"), Some("fabric"))
        .await
        .unwrap();
    assert!(!versions.is_empty());
    let project = catalog.project(&hit.project_id).await.unwrap();
    assert!(project.game_versions.iter().any(|v| v == "1.21.4"));
    assert!(project.loaders.iter().any(|l| l == "fabric"));
    assert!(!project.gallery.is_empty());
    let gallery = catalog
        .gallery_image(project.gallery[0].original_url())
        .await
        .unwrap();
    assert!(!gallery.is_empty() && gallery.len() <= 16 * 1024 * 1024);
    assert!(project
        .gallery
        .windows(2)
        .all(|g| g[0].ordering <= g[1].ordering));
    let members = catalog.members(&project.id).await.unwrap();
    assert!(!members.is_empty());
    assert!(members
        .iter()
        .all(|m| !m.user.username.is_empty() && !m.role.is_empty()));
    let organization = catalog
        .organization(project.organization.as_deref().unwrap())
        .await
        .unwrap();
    assert_eq!(organization.name, "Fabulously Optimized");
    let full = catalog.version(&versions[0].id).await.unwrap();
    assert_eq!(full.project_id, project.id);
    assert!(full
        .changelog
        .as_deref()
        .is_some_and(|c| !c.trim().is_empty()));
}
