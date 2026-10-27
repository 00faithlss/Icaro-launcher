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

## Ensayo remoto: caché fría y caliente con 4 procesos (27-10-2026)

Ejecuciones manuales de `b32e210`, `trabajos=4`, sin publicar. Runner: AMD EPYC 7763, 2 núcleos/4 hilos, 16 GiB. La ejecución `40428497528` sobre `a3128d8` falló antes de compilar porque la regresión dejaba activo su sustituto de Cargo; se corrigió en `b32e210` y no forma parte de la comparación.

| Medida | Línea base `40389040180` (`-j1`) | Fría [`40428836537`](https://github.com/00faithlss/Icaro-launcher/actions/runs/40428836537) | Caliente [`40430942181`](https://github.com/00faithlss/Icaro-launcher/actions/runs/40430942181) |
| --- | ---: | ---: | ---: |
| Job Windows | 29 min 16 s | 19 min 13 s | 11 min 8 s |
| Restaurar caché | — | 1 s (sin caché) | 26 s |
| Verificar núcleo e interfaz | 15 min 9 s | 6 min 41 s | 2 min 52 s |
| Compilar pruebas (Cargo) | 6 min 26 s + 7 min 51 s | — | 1 min 1 s + 1 min 5 s |
| Compilar y empaquetar | 13 min 9 s | 11 min 4 s | 7 min 16 s |
| Release launcher (`release-tiempos.json`) | 11 min 18 s (Cargo) | 9 min 22 s | 5 min 58 s |
| Release `sync_host` | 1 min 33 s (Cargo) | 1 min 22 s | 58 s |
| Guardar caché | — | 43 s | 0 s (ya existía) |

La caché caliente se restauró correctamente. Con 4 procesos y caché, el job baja un 62 % respecto de la línea base; sin caché, un 34 %. Son una muestra por configuración, no una mediana: repetir antes de tomarlas como valores estables. No se compararon 2 procesos ni se midió la RAM máxima del runner; ninguna ejecución falló por memoria.

Lo que más tiempo consume ahora es el release del launcher, casi 6 minutos con caché. Las dependencias ya llegan compiladas, así que ese tiempo corresponde al código del workspace y, previsiblemente, a la fase única de LTO completo con `codegen-units = 1` y `opt-level = "z"`. Siguiente ensayo propuesto: un perfil con LTO thin o más unidades de generación, comparando tiempo, tamaño del ejecutable y comportamiento antes de cambiar el de distribución. El valor por defecto de `trabajos` sigue siendo 1 hasta decidirlo.

## Ensayo remoto: LTO thin (27-10-2026)

Ejecución manual [`40433751137`](https://github.com/00faithlss/Icaro-launcher/actions/runs/40433751137) de `9121175`, `trabajos=4`, `perfil_release=lto-thin` (solo cambia `CARGO_PROFILE_RELEASE_LTO`; `opt-level = "z"` y `codegen-units = 1` se mantienen). Cambiar el LTO invalida las dependencias: el paso de empaquetado recompiló 426 crates, como la ejecución fría de LTO completo. Por eso se compara con esa ejecución y no con la caliente.

| Medida (Cargo) | LTO completo, frío `40428836537` | LTO thin `40433751137` |
| --- | ---: | ---: |
| Release launcher | 9 min 22 s | 7 min 59 s (−15 %) |
| Release `sync_host` | 1 min 21 s | 1 min 13 s |
| `icaro-0.3.3-windows-x86_64.exe` | 124 083 200 B (ejecución caliente `40430942181`) | 125 688 832 B (+1,3 %) |
| Instalador | 93 071 491 B | 93 244 559 B (+0,2 %) |

El job completo tardó 13 min 48 s. La caché no guarda las dependencias con LTO thin, porque su clave ya tenía una coincidencia exacta; el tiempo con caché caliente y LTO thin no está medido. Las pruebas y la validación del empaquetado pasaron, pero no se ejecutó la interfaz: el comportamiento del ejecutable thin no está comprobado. El perfil de distribución no cambió.

## LTO thin en `Cargo.toml`: ejecuciones fría y caliente (27-10-2026)

Revisión `043dac5`, `trabajos=4`, perfil de distribución con `lto = "thin"`. Ejecuciones [`40436876752`](https://github.com/00faithlss/Icaro-launcher/actions/runs/40436876752) y [`40438245521`](https://github.com/00faithlss/Icaro-launcher/actions/runs/40438245521), una tras otra.

| Medida | LTO completo, caliente `40430942181` | Thin, 1.ª `40436876752` | Thin, 2.ª `40438245521` |
| --- | ---: | ---: | ---: |
| Job Windows | 11 min 8 s | 13 min 20 s | 11 min 23 s |
| Verificar núcleo e interfaz | 2 min 52 s | 2 min 55 s | 2 min 55 s |
| Release launcher (Cargo) | 5 min 57 s | 7 min 53 s | 5 min 55 s |
| Release `sync_host` (Cargo) | 58 s | 1 min 9 s | 53 s |
| Crates compilados al empaquetar | 4 | 426 | 426 |
| `.exe` portable | 124 083 200 B | 125 688 832 B | 125 688 832 B |

La caché no guardó las dependencias compiladas con thin: ambas ejecuciones restauraron la caché existente con coincidencia exacta y su paso final no la volvió a guardar (1 s y 0 s). La clave de rust-cache no cambia con el perfil de `Cargo.toml`, de modo que la caché sigue conteniendo las dependencias del LTO completo y las pruebas siguen aprovechándola. Por eso ninguna de las dos ejecuciones mide thin con caché caliente.

Aun recompilando las 426 dependencias, la segunda ejecución thin igualó el release con LTO completo y caché caliente (5 min 55 s frente a 5 min 57 s). La diferencia entre las dos ejecuciones thin, casi 2 minutos con el mismo trabajo, indica una variación del runner que una sola muestra no permite descartar. Para medir thin con caché hay que incluir el perfil en la clave de la caché (entrada `key` de rust-cache) y repetir las dos ejecuciones. El comportamiento del ejecutable thin sigue sin probarse.

## LTO thin con caché propia (27-10-2026)

`b7d2b66` puso el perfil en la entrada `key` de rust-cache, que esa acción ignora cuando hay `shared-key`: sus ejecuciones [`40439859285`](https://github.com/00faithlss/Icaro-launcher/actions/runs/40439859285) y [`40441206356`](https://github.com/00faithlss/Icaro-launcher/actions/runs/40441206356) conservaron la clave anterior y recompilaron las 426 dependencias del release (7 min 34 s y 7 min 40 s). `ca747c2` llevó el perfil a `shared-key`.

| Medida | LTO completo, caliente `40430942181` | Thin, fría [`40442649129`](https://github.com/00faithlss/Icaro-launcher/actions/runs/40442649129) | Thin, caliente [`40444389010`](https://github.com/00faithlss/Icaro-launcher/actions/runs/40444389010) |
| --- | ---: | ---: | ---: |
| Job Windows | 11 min 8 s | 17 min 42 s | **9 min 40 s** |
| Restaurar caché | 26 s | 2 s (sin caché) | 27 s |
| Verificar núcleo e interfaz | 2 min 52 s | 6 min 41 s | 3 min 0 s |
| Compilar y empaquetar | 7 min 16 s | 9 min 36 s | **5 min 37 s** |
| Release launcher (Cargo) | 5 min 57 s | 8 min 2 s | **4 min 18 s (−28 %)** |
| Release `sync_host` (Cargo) | 58 s | 1 min 12 s | 56 s |
| Crates compilados (pruebas / release) | 3 / 4 | 429 / 426 | 3 / 4 |
| Guardar caché | 0 s | 43 s | 0 s |
| `.exe` portable | 124 083 200 B | 125 688 832 B | 125 688 832 B |

Con caché, LTO thin reduce el release del launcher en 1 min 39 s y el job en 1 min 28 s frente a LTO completo; respecto de la línea base de 29 min 16 s, el job queda en 9 min 40 s (−67 %). El coste es un ejecutable 1,6 MB mayor (+1,3 %). Una muestra por configuración; el runner varió hasta 2 minutos entre ejecuciones con el mismo trabajo. El comportamiento del ejecutable thin no se probó: falta abrir el launcher e iniciar una partida antes de publicarlo.

Cada cambio de perfil o de la versión de Rust deja la primera ejecución sin caché (unos 18 minutos con 4 procesos).
