# Optimización de Actions: cambios y comprobaciones

Base: `86e985b`, rama de trabajo `codex/optimizacion-integral`. No se ha hecho commit, push ni publicación. La [línea base real](2026-10-26-actions-linea-base.md) distingue cola, compilación y ejecución de pruebas.

## Caché y diagnóstico

La caché declara dos targets sin solapamiento: `target/ci-tests` y `target/windows-package-build`. Conserva las claves de compilador/entorno/manifiestos y excluye `target/dist`. La acción v2.8.2 se fija a su commit desreferenciado, `779680da715d629ac1d338a641029a2f4372abb5`.

Se añaden los informes HTML de Cargo y un registro de entorno limitado a revisión, toolchain y recursos del runner, separados de los artefactos de distribución. La validación local utiliza actionlint 1.7.12, descargado de su release oficial y contrastado con sus checksums, y el parser de PowerShell 7.6.5.

Pendiente: ejecutar la misma revisión en Actions con caché fría y caliente, incluyendo el tiempo de restauración/guardado y los crates recompilados. Todavía no hay un porcentaje de ahorro medido para estos cambios.

## Paralelismo y medición del empaquetado

`empaquetar_windows.ps1` acepta `-Trabajos` (1–16, por defecto 1) y `-Medir`, que añade `--timings` y registra en `target/ci/release-tiempos.json` el tiempo del launcher y del servicio por separado. El workflow manual ofrece 1, 2 o 4; las pruebas siguen con `--test-threads=1`. Pendiente: comparar 1, 2 y 4 en Actions sobre la misma revisión antes de cambiar el valor normal.

## Construcción separada de la publicación

El job `windows` ya no comparte el bloqueo global: `main` y `develop` pueden compilar a la vez. Solo `publicar` queda serializado; repite la decisión de versión dentro de la exclusión y valida el artefacto descargado de la propia ejecución antes de crear la release.

Comprobado localmente (27-10-2026): 26 regresiones de `scripts/ci/probar_publicacion_windows.ps1` aprobadas (decisión de versión, artefactos sintéticos, contrato de Cargo simulado y puertas del workflow), análisis de los seis bloques `run` con el parser de PowerShell y `git diff --check`. actionlint 1.7.12 (release oficial de rhysd/actionlint, SHA-256 `6e7241b51e6817ea6a047693d8e6fed13b31819c9a0dd6c5a726e1592d22f6e9` contrastado con su archivo de checksums) no informa errores en ningún workflow, sin shellcheck ni pyflakes porque los bloques son PowerShell. Ninguna de estas pruebas compila Rust ni ejecuta Actions.

Pendiente remoto: un ensayo manual para confirmar que `publicar` se omite y que el artefacto sale completo; y, con autorización, la primera publicación real tras el cambio. No se ha medido cuánto reduce la cola; la espera de 35 min 36 s de la línea base debería desaparecer cuando dos ramas compilan a la vez, pero falta confirmarlo.
