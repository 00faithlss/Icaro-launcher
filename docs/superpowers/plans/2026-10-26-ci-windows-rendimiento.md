# Optimización de CI Windows: plan de implementación

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reducir recompilaciones y espera entre ramas en GitHub Actions, conservando las verificaciones y la publicación correcta del launcher.

**Architecture:** Primero habilitar una caché verificable y mediciones comparables. Después separar el job de construcción del job de publicación, conservando la exclusión mutua y revalidando versiones dentro de ella. El paralelismo de Cargo se vuelve un parámetro de ensayo; su valor de producción y el perfil release solo cambian con evidencia.

**Tech Stack:** GitHub Actions, PowerShell 7, Cargo/Rust estable, GitHub CLI y Swatinem/rust-cache v2.8.2 fijado a commit.

**Spec:** `docs/superpowers/specs/2026-10-26-reduccion-trabajo-repetido-design.md`, ronda 1. Evidencia: `docs/superpowers/mediciones/2026-10-26-actions-linea-base.md`.

## Global Constraints

- Base `v0.3.x`, `86e985b306b766f1394fefa86c2ae6884677e876`; trabajo en `codex/optimizacion-integral` dentro del worktree aprobado.
- Conservar los cinco archivos anteriores de autenticación/documentación y los datos personales; no borrar targets ni cachés existentes.
- Conservar todas las pruebas de Windows, `--test-threads=1`, CRT estático, validación PE, licencias, hashes y contrato del actualizador.
- Mantener los canales: push a `develop` publica beta, push a `main` publica estable; otras ramas y ejecuciones manuales no publican.
- Mantener correctos revisión, versión, canal e `ICARO_COMPILACION`; la caché no puede reutilizar un ejecutable de otra compilación como producto final.
- No crear commits, hacer push, integrar, crear tags o releases sin el alcance solicitado. No lanzar builds remotos de prueba mediante pushes a las ramas de publicación.
- No cambiar el perfil release ni retirar funciones para mejorar una cifra sin comparar tiempo, tamaño y comportamiento del ejecutable.
- No atribuir los 35 min 36 s de cola al tiempo de compilación. No declarar beneficios remotos hasta medir una revisión autorizada en Actions.

## Review Focus

- Una ejecución manual debe terminar con artefactos, sin alcanzar una llamada de publicación; verificar el grafo y la condición del job final.
- Una estable publicada mientras otra revisión compila debe volver a comprobarse dentro de la exclusión final; una versión obsoleta no reemplaza la última estable.
- Un artefacto de otra revisión, canal o compilación debe rechazarse antes de generar el manifiesto o llamar a GitHub Releases.
- Un error en pruebas, empaquetado, restauración de archivos o consulta de GitHub debe impedir publicar y conservar un diagnóstico útil.
- Una caché fría, ausente o invalidada debe permitir construir desde cero; la caché caliente no puede alterar la identidad del producto.

---

## Archivos y responsabilidades

| Archivo | Responsabilidad |
| --- | --- |
| `.github/workflows/windows-package.yml` | Caché, parámetros manuales, jobs, tiempos, artefactos y puertas de publicación |
| `scripts/empaquetar_windows.ps1` | Compilación de producción con paralelismo configurable, timings y metadatos de canal; conserva empaquetado y validaciones |
| `scripts/ci/publicacion_windows.psm1` (nuevo) | Decidir si una versión puede publicarse y validar la identidad/integridad de sus artefactos |
| `scripts/ci/probar_publicacion_windows.ps1` (nuevo) | Regresiones de decisiones y rechazo de artefactos con fixtures temporales propios, sin llamadas de publicación |
| `docs/actualizaciones-launcher.md` | Explicar el nuevo reparto de jobs y la exclusión final |
| `docs/desarrollo.md` | Documentar caché, timings y ensayo manual |
| `docs/superpowers/mediciones/2026-10-26-actions-optimizacion.md` (nuevo) | Resultados por cambio, incluidos límites y ensayos pendientes |

### Task 1: Caché y diagnóstico de ambas construcciones

**Files:** modificar workflow; crear el registro de resultados; actualizar la guía de desarrollo.

**Interfaces:** el job de construcción conserva los dos comandos de pruebas y el script de empaquetado. Usa `target/ci-tests` para las pruebas mediante `CARGO_TARGET_DIR`, mientras producción conserva `target/windows-package-build`. Las rutas no se solapan. El script seguirá imponiendo su propio `--target-dir` para producción.

- [x] **Step 1: Registrar la configuración anterior y las comprobaciones de la nueva caché.** Comparar con la línea base: la acción actual usa el objeto de tag `401aff9…`, solo declara `shared-key`, y no existe aún una ejecución caliente. Confirmar el commit real `779680da715d629ac1d338a641029a2f4372abb5` de v2.8.2 mediante el proveedor.
- [x] **Step 2: Configurar la acción con ese commit y dos entradas `workspaces`: `. -> target/ci-tests` y `. -> target/windows-package-build`.** Conservar el hash de entorno/toolchain/lockfile y un `shared-key` explícito de Windows. No incluir `target/dist`, ni desactivar los componentes de clave por comodidad. Verificar en el código de la acción cómo trata ambos directorios.
- [x] **Step 3: Añadir `--timings` a las dos invocaciones de pruebas y registrar `rustc -Vv`, `cargo -V`, CPU y memoria del runner.** Guardar los HTML de Cargo como artefacto de diagnóstico independiente de `icaro-windows-x86_64`; registrar el resultado `cache-hit`. Los pasos de caché ya exponen su duración en GitHub. Evitar volcar variables de entorno completas.
- [x] **Step 4: Validar sintaxis YAML/expresiones con actionlint y analizar los bloques PowerShell con su parser.** Usar una distribución verificable de actionlint en un directorio de herramientas temporal si no está disponible; no cambiar el sistema. Confirmar que un error de Cargo conserva el `throw` y bloquea el resto de la entrega.
- [ ] **Step 5: Documentar el primer ensayo remoto pendiente.** Tras la autorización de publicación de la rama de trabajo, ejecutar manualmente dos veces la misma revisión, con los mismos perfiles y flags: primera caché fría, segunda caliente. Registrar aciertos y tiempo total, incluido restaurar/guardar. No hacer push ni disparar publicaciones desde este paso sin el alcance correspondiente.

**Resultado verificable:** cambios locales que cachean las dos rutas explícitas, mantienen las pruebas y generan diagnósticos sin cambiar la publicación. La efectividad remota queda marcada pendiente hasta ejecutar los dos ensayos.

### Task 2: Paralelismo configurable y medición del empaquetado

**Files:** modificar `scripts/empaquetar_windows.ps1`, workflow y guía; añadir las regresiones pertinentes al script de pruebas de CI.

**Interfaces:** añadir `[ValidateRange(1,16)][int]$Trabajos = 1` y `[switch]$Medir` al script de empaquetado. Ambos comandos `cargo build` consumen `Trabajos`; `Medir` añade `--timings`. El valor de producción empieza en 1. El workflow manual expone `trabajos` como elección `1`, `2`, `4`, por defecto `1`; lo pasa como un entero validado y conserva `--test-threads=1`.

- [x] **Step 1: Preparar una regresión del contrato de argumentos usando Cargo simulado.** Capturar las invocaciones reales y detener el fixture antes de ejecutar ISCC o publicar archivos. Afirmar que ambos builds reciben el valor elegido, target MSVC, CRT estático y target-dir original; que `0` y `17` fallan antes de invocar Cargo; y que un fallo de compilación impide empaquetar.
- [x] **Step 2: Ejecutar la regresión y comprobar que falla por la ausencia de los parámetros nuevos.** El fixture no utiliza Rust ni una instalación personal y crea sus archivos bajo un directorio temporal propio.
- [x] **Step 3: Implementar los parámetros y conservar la línea de argumentos existente salvo esos dos puntos.** Registrar tiempos separados de launcher y servicio con reloj monotónico en los diagnósticos. Añadir `canal` a los metadatos de artefactos, validando `beta` o `estable`, con el mismo valor que utiliza el build; el modo de prueba se sigue identificando como tal.
- [x] **Step 4: Repetir las regresiones y el análisis sintáctico.** Las pruebas existentes de Rust se ejecutan en la copia de implementación cuando se localice una toolchain; si no hay una utilizable, dejar el impedimento explícito y comprobar el resto. No presentar este fixture como una compilación real.
- [ ] **Step 5: Ensayar después de Task 1 un único cambio de paralelismo por ejecución autorizada.** Comparar 1 con 2 y después 4 solo si memoria/errores lo permiten. Considerar diferencias de claves de caché al comparar. Cambiar el valor normal únicamente si mejora de manera repetible sin agotar memoria; registrar las muestras y decisión.

**Resultado verificable:** se puede medir distinto paralelismo sin editar el workflow entre cada ensayo ni alterar la ejecución secuencial de las pruebas. No se promete una mejora por aumentar hilos.

### Task 3: Construcción independiente de la cola de publicación

**Files:** modificar workflow y empaquetado; crear módulo y pruebas; actualizar contrato y registro.

**Interfaces:**

- `Get-IcaroDecisionPublicacion([string]$Version, [string]$Canal, [string]$Arbol, [object[]]$Releases, [scriptblock]$ResolverArbol)` devuelve un objeto con `Repetida` (bool) y `Motivo` (string); lanza error ante una versión no publicable o una consulta fallida. `ResolverArbol` recibe el tag y devuelve su árbol; las pruebas usan fixtures.
- `Test-IcaroArtefactosPublicacion([string]$Directorio, [string]$Revision, [string]$Version, [long]$Compilacion, [string]$Canal)` devuelve `Metadata`, `Portable` y `Setup` tras comprobar identidad y hashes. Rechaza rutas fuera del directorio, pruebas, metadatos ausentes, identidades distintas y archivos alterados.
- Job `windows`: construye, verifica y sube el artefacto existente. Expone `repetida`, `version`, `revision` y `canal`. Tiene permisos de lectura y concurrencia `icaro-windows-build-${{ github.ref }}`, con `cancel-in-progress: false`.
- Job `publicar`: depende de `windows`; solo se habilita para `push`, construcción correcta y `repetida != 'true'`. Usa el grupo global `icaro-windows-publication`, con `cancel-in-progress: false`, y permisos `contents: write`, `actions: read`. Revalida la versión dentro de esa exclusión antes de publicar.

- [x] **Step 1: Escribir y ejecutar regresiones de decisión.** Casos: misma revisión ya publicada en el mismo canal se omite; mismo contenido en Beta no impide su estable; misma versión con contenido nuevo no reemplaza una estable; Beta de versión no superior a la estable se rechaza; fallo de `ResolverArbol` no se trata como permiso para publicar. Confirmar el fallo inicial por ausencia del módulo.
- [x] **Step 2: Extraer al módulo la lógica de versiones existente conservando esas reglas.** La comprobación temprana sigue evitando builds innecesarios; la comprobación final vuelve a consultar releases y árbol. No convertir errores de GitHub en listas vacías ni cambiar las convenciones de tag/manifiesto.
- [x] **Step 3: Añadir regresiones de artefactos y hacerlas pasar.** Cubrir revisión/canal/compilación incorrectos, `prueba = true`, falta de portable/setup, hash alterado y nombre que escapa del directorio. El conjunto válido devuelve solo archivos de su carpeta y sus metadatos concordantes. Usar archivos sintéticos para estas comprobaciones; el PE real ya lo valida empaquetado.
- [x] **Step 4: Separar los jobs y mover la publicación existente al job final.** Mantener el nombre del artefacto. Descargarlo con `gh run download` del `github.run_id` actual y nombre exacto, en carpeta propia vacía; comprobar salida y metadatos antes de generar `latest.json`. Mantener las notas, hashes calculados de los archivos finales, prerelease/latest y tag fuente originales. Revalidar versiones dentro del bloqueo; si ya está publicada exactamente esa entrega, omitirla.
- [x] **Step 5: Verificar el grafo y las puertas con actionlint y fixtures de publicación.** Afirmar que `workflow_dispatch` no habilita `publicar`, que un build/prueba fallido no permite publicarlo y que el bloqueo global está únicamente en el job de publicación. Confirmar que ambas ramas conservan su canal y que ninguna publicación activa se cancela automáticamente.
- [x] **Step 6: Ejecutar las regresiones de PowerShell y los checks del repositorio disponibles.** Registrar cualquier límite de toolchain. Revisar el diff para que no retire pruebas, validaciones o permisos necesarios ni cambie los canales. Actualizar `docs/actualizaciones-launcher.md` con la comprobación temprana y final y el intercambio de artefactos.
- [x] **Step 7: Documentar la validación remota pendiente del reparto de jobs.** El ensayo manual verifica construcción y artefactos sin publicar; la prueba de publicaciones simultáneas requiere un entorno aislado o autorización específica, no una release real de prueba. Los tiempos históricos de cola quedan como referencia, sin afirmar que todas las colas de GitHub desaparecerán.

**Resultado verificable:** `main` y `develop` pueden construir sin bloquearse mutuamente durante todo el build; únicamente el tramo final de publicación queda serializado y conserva los controles de versión e integridad.

## Cierre y continuidad

- [ ] Ejecutar `git diff --check`, las regresiones nuevas y los checks aplicables. Revisar de forma independiente el conjunto de CI antes de entregarlo; distinguir pruebas sintéticas, compilación real y ejecución remota.
- [ ] Publicar un resumen con archivos, evidencia, riesgos y mediciones pendientes; preparar cualquier solicitud de push sobre un resultado concreto y revisable.
- [ ] Con los ensayos de CI completados o con la limitación remota documentada, preparar planes independientes para persistencia/apertura, capturas e instalaciones según el diseño aprobado. Este plan no afirma implementar esas rondas.
