# Desarrollo

[Documentación](README.md) · [Estado y pendientes](roadmap.md)

Para investigar errores, consultar [Registros de diagnóstico](diagnostico.md):
ubicación, correlación de operaciones, causas y límites de captura.

Comprobación aislada de bandeja Windows (crea el icono unos instantes, sin cuentas
ni red): `cargo run --locked -j 2 -p icaro-ui --example probar_bandeja`.
Las escenas `compartir-cierre-real` y `compartir-cierre-real-angosto` capturan el
controlador de cierre con fixtures en Tinta/Piedra. El recorrido ignorado
`publicador_real_publica_seleccion_y_el_invitado_resuelve_el_codigo` requiere
`ICARO_PRUEBA_WORKER_URL=http://127.0.0.1:8787`, Worker local y APPDATA temporal;
su hilo reserva 8 MiB de pila para componer las tareas en debug de Windows.

## Requisitos

Rust estable, instalado con [rustup](https://rustup.rs), en Windows o Arch Linux. En Windows se necesitan Visual Studio Build Tools 2022 con C++ y Windows SDK. Si Cargo no está en el PATH de PowerShell:

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
```

El trabajo diario va en `v0.4.x`; `develop` publica beta y `main`, estable. Consulta [AGENTS.md](../AGENTS.md) antes de cambiar ramas o preparar una publicación.
En Arch Linux, el backend de credenciales requiere `libdbus` para compilar y un servicio de secretos en la sesión del usuario para recordar cuentas.

## Compilar y probar

En Windows, usar como máximo dos procesos de Cargo y aislar `APPDATA` antes de
ejecutar pruebas que guardan estado. No usar los datos reales de una instancia.

```powershell
$env:APPDATA = "$PWD/target/pruebas-appdata"
cargo build --locked -j 2 --workspace
cargo test --locked -j 2 -p icaro-core --lib --test actualizacion_local
cargo test --locked -j 2 -p icaro-ui --lib --example launcher -- --test-threads=1
git diff --check
```

La asignación de `APPDATA` afecta a esa consola; restaurarla o cerrar la consola
antes de iniciar el launcher con datos normales. En Linux, aislar también la raíz
de datos según las pruebas que se ejecuten; esta asignación es específica de Windows.

### Desarrollo de Compartir instancias (v0.4.x)

Tras la limpieza solicitada el 07-11-2026, usar `CARGO_INCREMENTAL=0` en las
consolas de validación de este trabajo limita el crecimiento de la caché incremental;
las recompilaciones pueden tardar más. No cambia la configuración global de Cargo.
`cargo clean` elimina también evidencias guardadas en `target`: el registro y las
capturas se conservaron en `%TEMP%/icaro-compartir-desarrollo-20261107/`.

El publicador experimental se activa en Servidores → Con amigos o para pruebas con
`ICARO_COMPARTIR_DESARROLLO=1`. Su configuración y alcance están en la guía técnica.
La activación explícita se conserva hasta validar el instalador del invitado con
perfil real, Jugar y cierre/bandeja.
El invitado ya tiene controlador experimental de código, huella, destino, revisión y
aplicación; la prueba de integración inyecta el perfil de Minecraft.
El intento incompleto persiste en la instancia y se recupera al reintroducir la
invitación tras reiniciar. Los contadores de progreso representan la etapa actual;
las descargas avanzan por archivo verificado, sin estimar un total global.
El menú «Sincronizar publicación» y Jugar consultan el origen vinculado mediante
reconexión autorizada. Sin anfitrión/servicio se muestra error y no se lanza el juego.
La reserva por ID vive fuera de la carpeta renombrable, en `.icaro-actividades` del
almacén. La escena `compartir-controlador-invitado-sync-error` muestra el diagnóstico
real con fixture, sin red, y admite `-angosto` y ambos temas.

El [registro de progreso](superpowers/progreso/2026-11-07-compartir-instancias.md)
indica evidencia y pendientes. La [guía técnica](compartir-instancias.md) contiene
los binarios de pruebas de compartir, la interoperabilidad con Worker local y las escenas
`compartir-*`; el [README del servicio](../services/invitaciones/README.md) explica
D1/Miniflare/Wrangler. No levantar producción para las pruebas locales.
Las escenas `compartir-controlador-invitado-identidad`,
`compartir-controlador-invitado-destino` y `compartir-controlador-invitado-revision`
recorren los modelos reales con claves solo en memoria y acciones de demo.
`compartir-controlador-invitado-recuperacion` y
`compartir-controlador-invitado-progreso` añaden los estados de este bloque.

Para capturar, compilar el ejemplo launcher y establecer `ICARO_DEMO=1`,
`ICARO_VISTA_DISENO=compartir-publicacion`, `ICARO_TEMA_DISENO=tinta` o `piedra`
y `ICARO_CAPTURA_DISENO` con una ruta PNG propia. El sufijo de escena `-angosto`
usa 940×720. Comprobar salida correcta y fecha del archivo: un PNG de una ejecución
anterior no demuestra que la nueva vista haya arrancado. Las capturas son fixtures.

Regla de interfaz: centrar icono/texto en ambos ejes, padding simétrico y alturas
iguales en controles equivalentes y celdas de resumen. En iced, `center(Fill)`
redefine también el tamaño del contenedor: para conservar medidas fijas ya
establecidas, usar `align_x/align_y`. El contenido de un scroll vertical no puede
pedir `Fill` en ese eje. Las regresiones de construcción cubren esos casos.

Ejemplos de la interfaz:

```
cargo run --locked -j 2 -p icaro-ui --example galeria
cargo run --locked -j 2 -p icaro-ui --example shell
cargo run --locked -j 2 -p icaro-ui --example launcher
```

## Estructura

Las referencias son `icaro-design-system.rar` e `icaro-componentes-nuevos.zip` (12-10-2026), que aporta `CambioVersionInstancia`, `InfoProyecto` y `PaginaMod`. Las funcionalidades nuevas se conectan a sus componentes y a las vistas existentes. Las diferencias y decisiones se registran en el [roadmap del System Design](roadmap-system-design.md). El mapa de capas y servicios está en [Arquitectura](arquitectura.md).

| Carpeta | Contenido |
| --- | --- |
| `crates/core` | Autenticación, instancias, Minecraft/loaders/Java, MRPACK de Modrinth, lanzamiento, mods/paquetes, cambio de versión, exportación y actualización del launcher |
| `crates/cli` | Herramienta de línea de comandos |
| `crates/ui` | Sistema de diseño en iced: tema, tipografía, iconos y componentes |

## Estados del ejemplo launcher

El ejemplo `launcher` acepta variables de entorno para abrir una pantalla en un estado concreto:

| Variable | Efecto |
| --- | --- |
| `ICARO_DEMO=1` | Cuentas y tiendas simuladas, sin consultas a servicios reales |
| `ICARO_SECCION` | Sección inicial, de 1 a 7 |
| `ICARO_PIEDRA` | Tema claro |
| `ICARO_VACIO` | Todas las pantallas sin datos, para ver los estados vacíos |
| `ICARO_AJUSTE` | Página de Ajustes (7 es Acerca de, que incluye el Gabinete) |
| `ICARO_EDITOR` | Pestaña del editor de instancia (5 es Capturas) |
| `ICARO_BUSCAR` | Texto de búsqueda de la tienda de mods |

En `ICARO_DEMO=1`, las capturas son recortes de grabados y sus acciones se simulan. En modo real se leen las PNG de las instancias; copiar y abrir carpeta funcionan en Windows y el visor conserva los controles de ventana.
El login y el catálogo se prueban según la guía de [inicio de sesión y catálogo](login-y-catalogo.md). La instalación y ejecución de Vanilla/Fabric y la instalación de modpacks se prueban según [Vanilla y Java](vanilla-y-java.md). Las variables de estados ficticios de cuenta requieren el modo de demostración.

## Capturas aisladas del diseño

El ejemplo puede capturar los widgets reales con datos de prueba y cerrar esa ventana, sin consultar Microsoft o Modrinth ni guardar cambios en las cuentas e instancias del usuario. `ICARO_VISTA_DISENO` admite `asistente`, `exportar`, `mods`, `seleccion`, `tienda`, `ficha`, `pagina`, `archivos`, `archivos-renombrar`, `juego-descargas`, `juego-consola`, `juego-consola-vacia`, `juego-fallo`, `juego-editor`, `juego-instalando`, `juego-ejecutando` y las fases `juego-umbral-entrada`, `juego-umbral-arco`, `juego-umbral-registro`, `juego-umbral-retirada`, `juego-umbral-fallo`. `juego-capturas` y `juego-capturas-visor` leen una PNG de prueba indicada en `ICARO_QA_CAPTURA`, sin modificarla ni refrescarla desde el inventario real. El explorador usa una carpeta de muestra dentro del directorio temporal. Las escenas del juego no ejecutan Java ni usan una cuenta.

Las escenas `pagina-modpack` y `pagina-modpack-detalles` revisan los chips de compatibilidad y las licencias largas, y `picar` muestra la confirmación de eliminación con Cancelar alineado al pie de la tarjeta. Los selectores separados se revisan con `asistente-version` y `asistente-loader`; el componente nuevo de cambio de versión con `cambiar-version-seleccion`, `cambiar-version-carga`, `cambiar-version-detalle` y `cambiar-version-seleccion-angosto` (940×720). `cambiar-version` es la confirmación de riesgo. `pagina-galeria`, `pagina-descripcion`, `pagina-dependencias` y `pagina-creadores` revisan la página nueva y su InfoProyecto; `pagina-creadores-minima` omite datos opcionales del fixture para mostrar los avatares en el primer cuadro. Los dos fixtures de creadores leen el JSON y las imágenes públicas descargadas previamente en `ICARO_QA_CREATORES`.

Para resourcepacks están disponibles `recursos-lista`, `recursos-seleccion`, `recursos-ficha-compatible`, `recursos-ficha-otra`, `recursos-ficha-vanilla` y `recursos-confirmacion-otra`. Shaders usa `shaders-lista`, `shaders-seleccion`, `shaders-actualizacion`, `shaders-confirmacion` y `shaders-ficha-compatible`. `actualizacion` y `actualizacion-al-cerrar` muestran la composición de actualización con datos de prueba, sin descargar ni sustituir ningún ejecutable. `acerca-de` abre Ajustes → Acerca de con la versión compilada y el enlace al repositorio. Estas versiones y notas ficticias se usan exclusivamente en las capturas aisladas.

```powershell
cargo build --locked -j 2 -p icaro-ui --example launcher
$env:ICARO_DEMO = '1'
$env:ICARO_CAPTURA_DISENO = Join-Path $env:TEMP 'icaro-tienda.png'
$env:ICARO_VISTA_DISENO = 'tienda'
$env:ICARO_TEMA_DISENO = 'tinta' # Repetir con 'piedra'.
.\target\debug\examples\launcher.exe
Remove-Item Env:ICARO_DEMO, Env:ICARO_CAPTURA_DISENO, Env:ICARO_VISTA_DISENO, Env:ICARO_TEMA_DISENO
```

Si la compilación de pruebas agota la memoria, ejecuta los objetivos por separado con `-j 1`: núcleo, biblioteca de interfaz y ejemplo launcher.

## Evidencias anteriores

Las [validaciones iniciales de octubre](superpowers/mediciones/2026-10-validaciones-iniciales.md) conservan resultados de Java, almacenamiento, Fabric, modpacks, servidores, instalador y retorno nativo. El [índice técnico](superpowers/README.md) reúne las mediciones y planes posteriores.

## Tamaño de compilaciones de desarrollo

El perfil `dev` omite símbolos de depuración para reducir el espacio de `target`. Para restaurarlos en PowerShell durante una sesión, usa `$env:CARGO_PROFILE_DEV_DEBUG = "2"` antes de ejecutar Cargo; en Bash usa `export CARGO_PROFILE_DEV_DEBUG=2`. Esto puede aumentar el espacio y el tiempo de compilación. La compilación incremental sigue activa por defecto. Las mediciones y sus limitaciones están en [el registro de optimización](superpowers/mediciones/2026-10-26-registro.md).

## Medición de compilación en GitHub Actions

El workflow Windows conserva las dependencias de pruebas en `target/ci-tests` y las de producción con CRT estático en `target/windows-package-build`. Las variantes siguen separadas por sus perfiles, target y flags. La caché depende de la versión de Rust y de los manifiestos/lockfile; una caché ausente o invalidada vuelve a construir las dependencias.

El artefacto `icaro-windows-diagnostico` reúne la revisión, toolchain, CPU/memoria del runner y los informes HTML de `cargo --timings`. No contiene los paquetes de distribución. El resumen de la ejecución indica si hubo coincidencia exacta de caché; revisar también sus pasos de restauración y guardado y los crates realmente recompilados.

Comparar dos ejecuciones manuales de la misma revisión, con los mismos parámetros, separando caché fría y caliente. El tiempo de cola se calcula aparte del tiempo de los jobs. Las ejecuciones manuales generan artefactos y no publican releases. La [línea base de Actions](superpowers/mediciones/2026-10-26-actions-linea-base.md) documenta los tiempos previos; las mejoras nuevas requieren medición remota.

La ejecución manual permite elegir 1, 2 o 4 procesos de Cargo. El CI usa 4 por defecto: con caché, el job bajó de 29 a 11 minutos sin errores de memoria (una muestra, ver el registro). Las pruebas mantienen `--test-threads=1`, y el script local conserva 1 por defecto para equipos con poca memoria libre. La entrada `perfil_release` permite ensayar `lto-thin` en una ejecución manual; las publicaciones siempre usan el perfil `release` de `Cargo.toml`. Para medir el empaquetado local se puede pasar `-Trabajos 2 -Medir` a `scripts/empaquetar_windows.ps1`: conserva el target/CRT y añade los informes HTML y `target/ci/release-tiempos.json` con tiempos separados del launcher y del servicio.

## Versión y enlace de Acerca de

Ajustes → Acerca de muestra `env!("CARGO_PKG_VERSION")`, heredado del workspace; el actualizador usa la misma fuente. El enlace Código fuente en GitHub toma `workspace.package.repository`, heredado por la UI, y se valida antes de abrirlo. No fijar versiones ni URLs duplicadas en la interfaz. Recompilar y publicar es necesario para cambiar un ejecutable ya distribuido.

## Licencia

[GPL-3.0-only](../LICENSE); atribuciones de terceros en [Créditos](creditos.md).
