// SPDX-License-Identifier: GPL-3.0-only
//! Una misma reserva desde la revisión hasta que termina Java, fuera de iced update.
use icaro_core::{
    actividad_instancia::Guardia,
    instancias::{Almacen, Instancia},
};
use std::sync::Arc;
pub async fn adquirir(
    store: Almacen,
    instancia: Instancia,
    previa: Option<Arc<Guardia>>,
) -> Result<Arc<Guardia>, String> {
    tokio::task::spawn_blocking(move || {
        let ruta = Guardia::ruta_instancia(&store, &instancia.id).map_err(|e| e.to_string())?;
        match previa {
            Some(g) if g.ruta() == ruta => Ok(g),
            Some(_) => Err("La carpeta revisada cambió. Comprueba otra vez la instancia.".into()),
            None => Guardia::adquirir_instancia(&store, &instancia.id)
                .map(Arc::new)
                .map_err(|e| e.to_string()),
        }
    })
    .await
    .map_err(|_| "No se pudo reservar la actividad de la instancia.".to_string())?
}
