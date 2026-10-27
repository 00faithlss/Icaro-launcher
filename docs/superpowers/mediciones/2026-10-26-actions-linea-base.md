# Línea base real de GitHub Actions para Windows

Consulta: 26-10-2026. Repositorio: `00faithlss/Icaro-launcher`. Workflow: `.github/workflows/windows-package.yml`. Datos obtenidos de las ejecuciones, jobs, pasos y registros reales; no se lanzó una ejecución nueva ni se modificó el workflow.

## Duraciones observadas

La espera se calcula desde la creación de la ejecución hasta el inicio del job. El tiempo del job va desde su inicio hasta su finalización; los pasos usan las marcas que expone GitHub. Resolución de segundos para los tiempos de la API.

| Ejecución | Rama y revisión | Espera previa | Job Windows | Verificar núcleo e interfaz | Compilar y empaquetar |
| --- | --- | ---: | ---: | ---: | ---: |
| [40389040180](https://github.com/00faithlss/Icaro-launcher/actions/runs/40389040180) | `main`, `671137f`, 0.3.3 | 3 s | 29 min 16 s | 15 min 9 s | 13 min 9 s |
| [40067576397](https://github.com/00faithlss/Icaro-launcher/actions/runs/40067576397) | `develop`, `f7b96af`, 0.3.3 | 3 s | 32 min 57 s | 16 min 18 s | 15 min 57 s |
| [39899147001](https://github.com/00faithlss/Icaro-launcher/actions/runs/39899147001) | `main`, `917a7de`, 0.3.1 | 35 min 36 s | 26 min 30 s | 14 min 6 s | 10 min 59 s |

La ejecución 39899147001 se creó a las 04:13:08 UTC del 23-10-2026, comenzó su job a las 04:48:44 UTC y terminó el job a las 05:15:14 UTC. Su espera coincide con la beta 39899058945, que terminó a las 04:48:42 UTC, y el workflow usa un grupo global de concurrencia con `cancel-in-progress: false`. Su duración completa de aproximadamente 62 minutos incluye esa espera; no equivale a una compilación de 62 minutos.

## Desglose de Cargo en la última estable

Los registros de 40389040180 muestran:

- Compilación del primer comando de pruebas: 6 min 26 s.
- Compilación del segundo comando de pruebas: 7 min 51 s.
- Ejecución de las cuatro baterías informadas: 21,07 s, 11,54 s, 4,70 s y 12,91 s; aproximadamente 50,22 s en total. Resultados informados: 255 pruebas del núcleo aprobadas y 24 ignoradas, 2 del actualizador aprobadas, 60 de la biblioteca de interfaz aprobadas, 68 del launcher aprobadas y 2 ignoradas.
- Compilación release del launcher: 11 min 18 s.
- Segundo comando release para `sync_host`: 1 min 33 s.

Los tiempos de compilación anteriores son los informados por Cargo. Los pasos de GitHub también contienen inicio de comandos y trabajo auxiliar; no deben sumarse como si provinieran de un perfilador único.

## Configuración y oportunidades identificadas

1. Las revisiones remotas consultadas son `main = 671137f`, `develop = f7b96af` y `v0.3.x = 86e985b`. Las dos ramas de publicación aún no incluyen `Swatinem/rust-cache`, que sí se añadió en la última. `gh cache list` devolvió `[]` en esta consulta: todavía no existe evidencia de una ejecución caliente con esa mejora.
2. Pruebas y empaquetado usan `-j1`. El script de producción compila en `target/windows-package-build`, para `x86_64-pc-windows-msvc`, con `target-feature=+crt-static`. Las pruebas usan el target normal y su perfil correspondiente. Estas variantes deben mantenerse diferenciadas en la caché y la validación.
3. `icaro-core` aparece compilado en ambos comandos de pruebas y en ambos comandos de release. Todavía falta determinar qué diferencias de features, perfiles o flags justifican esas construcciones y cuáles podrían evitarse.
4. El grupo global `icaro-windows-publication` serializa el workflow completo de ambas ramas. Separar la construcción del tramo de publicación podría reducir la espera, pero exige conservar la exclusión de publicación y revalidar versiones dentro de ella.
5. El perfil release utiliza `opt-level = "z"`, `lto = true` y `codegen-units = 1`. Cargo documenta que LTO completo y el número de unidades afectan al intercambio entre compilación y código generado. Comparar alternativas en la misma revisión; todavía no se atribuye una fracción concreta del tiempo a estos ajustes. [Referencia de Cargo](https://doc.rust-lang.org/cargo/reference/profiles.html).
6. La caché usa por defecto el target del workspace y claves que incluyen el compilador y el entorno de Cargo. Comprobar la ruta personalizada de producción y sus aciertos explícitamente. El `shared-key` no garantiza por sí solo reutilización entre todas las ramas. [Documentación de rust-cache v2.8.2](https://github.com/Swatinem/rust-cache/blob/v2.8.2/README.md).
7. La referencia `401aff9a7a08acb9d27b64936a90db81024cff97` usada actualmente es el objeto de tag anotado `v2.8.2`; su commit es `779680da715d629ac1d338a641029a2f4372abb5`, contrastado mediante la API de tags y `git ls-remote` con desreferenciación. Usar y verificar el commit real al fijar la acción; no se ha ejecutado el workflow actual para comprobar cómo resuelve el objeto de tag.

## Límites y siguiente ensayo

Son tres revisiones y ejecuciones distintas, con condiciones del runner potencialmente diferentes. No forman una comparación controlada ni permiten prometer un porcentaje de ahorro. El tiempo de cola no se mezcla con el de CPU. No se dispone aún de tiempos por crate ni de memoria máxima.

El primer ensayo debe comparar caché fría y caliente de la misma revisión, con perfiles y flags iguales. Registrar costo de restauración/guardado, aciertos, espacio y tiempo por fase. Después variar una sola decisión por ensayo: paralelismo de Cargo, invocaciones, perfil release o estructura de jobs. La ejecución manual debe generar artefactos sin publicar; conservar pruebas, CRT estático, licencias, validación de binarios y hashes.
