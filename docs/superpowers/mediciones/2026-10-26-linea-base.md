# Línea base de rendimiento (Linux)

Base f7b96af (0.3.3), con los cambios locales de actualización de canales en `crates/core/src/actualizacion.rs` y la pantalla. Rust 1.99.0, Cargo 1.99.0, límite de CPU 4 núcleos; sistema Linux en contenedor. Caché y `target` compartidos con compilaciones anteriores, por lo que estos resultados no son compilaciones limpias ni representan Windows o GitHub Actions. Disco 32 GiB; target anterior 23 GiB y espacio libre al inicio 5,1 GiB. No se eliminaron datos.

## Cargo con caché

`CARGO_TARGET_DIR=/workspace/Icaro-launcher/target cargo test --locked --offline -p icaro-core --lib --no-run`, medido con reloj monotónico, tres ejecuciones sin cambios: 0,237 s, 0,254 s y 0,220 s; mediana 0,237 s. Comprueba que el objetivo ya estaba compilado, no mide una recompilación tras editar un archivo.

La comprobación equivalente de la interfaz (`cargo test --locked --offline -p icaro-ui --example launcher --no-run`) tardó 0,321 s, 0,368 s y 0,408 s; mediana 0,368 s. También es una caché caliente sin cambios.

## Sincronización simulada

`cargo test --locked --offline -p icaro-core --lib medir_actualizacion_con_carpeta_grande -- --ignored --nocapture`, tres procesos con el mismo target y 500 MiB de archivos temporales sintéticos. El propio test mide:

| Fase | Ejecución 1 | Ejecución 2 | Ejecución 3 | Mediana |
|---|---:|---:|---:|---:|
| Revisión con cambio | 2,455 s | 2,443 s | 2,904 s | 2,455 s |
| Aplicación | 11,05 ms | 11,55 ms | 17,19 ms | 11,55 ms |
| Jugar sin cambios, primera vez | 11,24 ms | 10,91 ms | 11,93 ms | 11,24 ms |
| Jugar sin cambios, repetición | 10,70 ms | 12,04 ms | 11,87 ms | 11,87 ms |

La prueba utiliza un catálogo simulado y almacenamiento local; no incluye red, servidor real ni Windows. Cada ejecución terminó correctamente. Sus tiempos no son equivalentes a la experiencia completa de un usuario.

## Pendiente

Medir compilación limpia y una modificación real del launcher en un directorio aislado cuando haya espacio; probar diferentes perfiles con idéntica caché y condiciones. Medir carga de interfaz y cuenta en Windows, y recopilar tiempos por paso de Actions: la API devolvió Forbidden desde este entorno. Medir memoria y tamaño final de artefactos. No comparar la compilación de núcleo con el empaquetado de Windows.
