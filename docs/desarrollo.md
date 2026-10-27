# Desarrollo

## Requisitos

Rust estable, instalado con [rustup](https://rustup.rs), en Windows o Arch Linux.
En Arch Linux, el backend de credenciales requiere `libdbus` para compilar y un servicio de secretos en la sesión del usuario para recordar cuentas.

## Compilar y probar

```
cargo run -p icaro-cli
cargo test --workspace
```

Ejemplos de la interfaz:

```
cargo run -p icaro-ui --example galeria
cargo run -p icaro-ui --example shell
cargo run -p icaro-ui --example launcher
```

## Estructura

Las referencias son `icaro-design-system.rar` e `icaro-componentes-nuevos.zip` (12-10-2026), que aporta `CambioVersionInstancia`, `InfoProyecto` y `PaginaMod`. Las funcionalidades nuevas se conectan a sus componentes y a las vistas existentes. Las diferencias de pantallas que ya tienen diseño se registran en la [comparación con el system design](revision-system-design.md) para evaluar cambios antes de sustituirlas.

| Carpeta | Contenido |
| --- | --- |
| `crates/core` | Autenticación, instancias, Vanilla/Fabric/Java, MRPACK de Modrinth, lanzamiento, mods/paquetes, cambio de versión, exportación y actualización del launcher |
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
cargo build -j 1 -p icaro-ui --example launcher
$env:ICARO_CAPTURA_DISENO = Join-Path $env:TEMP 'icaro-tienda.png'
$env:ICARO_VISTA_DISENO = 'tienda'
# $env:ICARO_PIEDRA = '1' para el tema claro.
.\target\debug\examples\launcher.exe
Remove-Item Env:ICARO_CAPTURA_DISENO, Env:ICARO_VISTA_DISENO
```

Si la compilación de pruebas agota la memoria, ejecuta los objetivos por separado con `-j 1`: núcleo, biblioteca de interfaz y ejemplo launcher.

## Validación de la entrega del 12-10-2026

- Núcleo: 83 pruebas pasadas y 5 ignoradas; incluye migración de versión, respaldo/copia, conservación de mods desactivados y recuperación ante fallos.
- Biblioteca de interfaz: 49 pruebas pasadas. Ejemplo launcher: 26 pasadas y 2 ignoradas; incluye navegación, agrupación de versiones y descarte de respuestas tardías del selector.
- Compilación del launcher completada sin advertencias. Tras los ajustes finales de la tabla, se repitieron las 26 pruebas del ejemplo launcher, incluida una fila compatible en su recorrido de navegación, sin fallos.
- Capturas de los nuevos componentes revisadas en Tinta y Piedra; selector también a 940×720. Los avatares se comprobaron con imágenes públicas reales.

Esta validación no equivale a instalar loaders ni a probar una actualización publicada: ambos límites siguen registrados en los roadmaps. Las pruebas de red ignoradas se ejecutan aparte cuando se requiere comprobar el proveedor real.

## Validación de los ajustes Java (12-10-2026)

- Núcleo: 108 pruebas aprobadas y 5 omitidas. Incluye prioridad local sobre runtime guardado, respaldo sin red, descarga desactivada, versión manual incompatible y arquitectura.
- Biblioteca de interfaz: 55 pruebas aprobadas. Launcher: 35 aprobadas y 2 omitidas, ejecutado con `--test-threads=1` porque varias pruebas comparten la lista global de instancias. Una ejecución concurrente falló en la comprobación de lista vacía del arranque; la ejecución secuencial completa pasó.
- Revisadas las capturas aisladas `java-instancia` y `java-global` en Tinta y Piedra. Conservan los componentes existentes de campos, selectores, interruptores y tabla.
- Falta el recorrido nativo en Arch y repetir el lanzamiento real de Minecraft con las nuevas selecciones; estas pruebas no requieren ni usan las cuentas del usuario.

## Java con perfiles y modos de Prism (12-10-2026)

La resolución automática distingue descarga administrada y detección sin descargas. Vanilla guarda la lista compatible por versión de los metadatos públicos de Prism, con respaldo Mojang si no está disponible. La detección incluye runtimes del launcher oficial tradicional y Microsoft Store. Probar inicia la JVM con memoria y argumentos de la instancia; usa el perfil guardado tras reiniciar cuando está disponible.

Validación: 110 pruebas del núcleo, 55 de la biblioteca de interfaz y 35 del launcher aprobadas (estas últimas con `--test-threads=1`), más las dos pruebas explícitas de JVM y metadatos reales.

La validación incluye perfiles explícitos con varias versiones, runtime administrado sin consultar candidatos locales, ruta manual compatible, búsqueda acotada de runtimes oficiales, argumentos con espacios, límites de memoria y fallo de JVM. Se ejecutaron también una prueba con el Java 25 ya guardado en Ícaro y la consulta real del perfil Prism de Minecraft 1.17.1, sin descargar nuevos runtimes ni usar cuentas.

Fabric y Quilt integran el mínimo de Java de su perfil oficial. Los requisitos de Forge y NeoForge dependen de sus instaladores pendientes. Faltan el recorrido nativo en Arch y el lanzamiento completo del juego con los nuevos modos.

## Almacenamiento compartido de Vanilla (13-10-2026)

El instalador usa assets compartidos, bibliotecas por SHA-1/ruta Maven y metadatos/clientes bajo `meta/minecraft`. Mantiene los nativos extraídos y el marcador de instalación por instancia. Migra copias antiguas verificadas al preparar, publica el marcador de esquema 2 y después retira los duplicados verificados. El bloqueo del SO serializa las preparaciones del mismo almacén y admite cancelación; no debe eliminarse `meta/minecraft/instalacion.lock` para desbloquearlo.

Validación: 114 pruebas del núcleo aprobadas (7 omitidas) y 35 del launcher aprobadas (2 omitidas; ejecución secuencial). Los casos nuevos cubren dos instancias sin red, reparación de corrupción del mismo tamaño, cambio de nombre, eliminación de una instancia sin perder el almacén común, cancelación de migración conservando el perfil anterior, espera cancelable y argumentos de lanzamiento con rutas compartidas/privadas.

La instancia local de prueba Vanilla 26.3 se migró y volvió a preparar: 5.215 archivos verificados y 65 entradas de classpath; la segunda preparación no anunció descargas. Se comprobó esquema 2 y ausencia de las carpetas antiguas `assets`, `libraries` y `versions` dentro de la instancia. El usuario confirmó el funcionamiento de Vanilla con este almacenamiento; la ejecución en Linux sigue pendiente.

Para preparar una instancia existente sin abrir Minecraft ni acceder a cuentas:

```powershell
cargo run -j 1 -p icaro-core --example preparar_minecraft -- "$env:APPDATA\Icaro" <id-de-instancia>
```

El comando usa el mismo instalador que la interfaz: puede descargar archivos faltantes, verificar, reparar y migrar copias antiguas. Requiere una instancia ya existente y no modifica sus preferencias Java/memoria.

## Reorganización funcional (13-10-2026)

La configuración por instancia usa `instance.json` de esquema 2; migra CFG conservando sus claves y los campos desconocidos del JSON. Java, descarga, memoria, Metaspace y argumentos tienen herencia global independiente. Los ajustes antiguos mantienen valores propios. Las exportaciones Ícaro conservan ajustes portables y marcan las rutas manuales de Java para seleccionarlas de nuevo en destino.

Instancias, mods, packs, archivos y capturas usan la papelera del sistema. Deshacer funciona para instancias, mods y capturas durante la sesión; la recuperación posterior permanece disponible en la papelera del sistema. Windows usa `IFileOperation` con `FOFX_RECYCLEONDELETE` y fallo temprano; no se solicita borrado permanente. Las rutas demasiado largas se rechazan antes de operar. Linux usa la implementación Freedesktop de `trash`; falta validación nativa en ese sistema.

Almacenamiento obtiene tamaños reales, limpia exclusivamente `cache/`, permite trasladar papeleras internas y recuperar retiradas de mods interrumpidas mediante su diario. La recuperación comprueba hashes y conserva las colisiones. El registro del launcher rota a 1 MiB con tres archivos anteriores. Los respaldos de transacciones completadas se limpian; los respaldos de mundos y datos personales no son caché.

Se ejecutó `reorganizar_datos` sobre el directorio local de Ícaro: se trasladaron 15 elementos previamente eliminados a la papelera del sistema y se retiró la carpeta interna vacía. En ese momento no había instancias activas. Assets, bibliotecas y Java permanecen en el almacén común.

```powershell
cargo run -j 1 -p icaro-core --example reorganizar_datos -- "$env:APPDATA\Icaro"
```

Este comando traslada papeleras antiguas, migra ajustes y recupera operaciones interrumpidas. La interfaz ofrece las mismas acciones de mantenimiento en Almacenamiento. No necesita iniciar sesión ni abrir el juego.

Validación final: 117 pruebas del núcleo, 55 de la biblioteca de interfaz y 35 del launcher aprobadas, más la prueba nativa explícita de papelera: 208 pruebas únicas aprobadas. El núcleo conserva 8 pruebas omitidas por defecto y el launcher 2. Las comprobaciones específicas de exportación portable y retirada de packs también pasaron tras sus últimos ajustes. Compilación del launcher y `git diff --check` correctos. Se revisó Almacenamiento en Tinta y Piedra y Java global en Piedra mediante capturas de fixtures. El usuario confirmó el funcionamiento con esta estructura en Windows; Linux sigue pendiente.


## Licencia

GPL-3.0-only. El texto completo está en [LICENSE](../LICENSE).

## Validación de Fabric, MRPACK y ajustes de interfaz

Pasaron 127 pruebas del núcleo, 55 de componentes y 38 del launcher, sin fallos. Quedaron omitidas por defecto 12 pruebas del núcleo y dos del launcher que necesitan servicios o fixtures externos. La instalación real de Fabulously Optimized 14.1.1 con Minecraft 26.2/Fabric 0.19.5 y la reutilización sin descargas se comprobaron por separado; esa prueba no inicia el juego ni usa una cuenta. El usuario confirmó Fabric 26.3 con un mod real en Windows.

Se inspeccionaron capturas de modpacks y eliminación en Tinta/Piedra, incluidos los chips de versiones, licencia larga y Cancelar alineado al pie de la tarjeta. El ejecutable compiló y git diff --check pasó. Los comandos reproducibles de instalación están en [Vanilla, Fabric y Java](vanilla-y-java.md).

## Servidores reales y detección de ZeroTier

Las escenas `servidores`, `servidores-vacio`, `servidores-formulario`, `red-privada-autorizada`, `red-privada-ausente`, `sincronizacion-revision`, `sincronizacion-confirmar` y sus variantes con sufijo `-angosto` usan fixtures aislados. `ICARO_TEMA_DISENO=piedra|tinta` fija el tema de la captura. No consultan cuentas ni redes del usuario. El diagnóstico `detectar_zerotier` es de lectura. La [guía de servidores](servidores.md) y la [guía del servicio de mods](sincronizacion-servidores.md) explican el uso y sus límites.

El servicio del anfitrión se compila con `cargo build -j 1 -p icaro-core --example sync_host`; su ayuda muestra los argumentos de datos, instancia e IP privada de escucha. El [empaquetado Windows](instalador-windows.md) genera launcher, servicio, portable e instalador desde la misma versión y reúne los avisos de licencia de sus dependencias.

Los grabados preparados se incluyen como WebP VP8L sin pérdida; se conservan las vistas previas RGBA y la decodificación en segundo plano. El 13-10-2026 se compararon los 144 resultados con los PNG anteriores: dimensiones y todos los canales RGBA idénticos; 558.684.090 bytes pasaron a 159.119.906 bytes. No se cambiaron las fuentes originales ni los recortes.

Las escenas de prueba incluyen `modpacks-tienda`, `modpacks-versiones`, sus variantes `-angosto`, `cuenta-original` y `estudio-original`. `ICARO_COMPILACION` incorpora el número de compilación del workflow; en compilaciones locales toma 0.

## Validación previa a v0.1.0

El 13-10-2026 pasaron 146 pruebas del núcleo, 56 de componentes, 49 del launcher y la regresión del ayudante real con Windows PowerShell: 252 en total, sin fallos. Se mantienen 13 pruebas del núcleo y dos del launcher omitidas por defecto por sus requisitos externos. La interfaz se ejecuta con `--test-threads=1` para las fixtures que comparten estado.

También pasaron `git diff --check`, la validación del workflow con actionlint y el cálculo del roadmap (61/83, 73 %). Se verificaron los hashes y tamaños de los paquetes locales, el contenido del ZIP portable, la actualización mediante setup silencioso en una ruta aislada y la conservación de los metadatos de usuario revisados. Los estados de actualización tienen 16 capturas nativas de fixtures en Tinta/Piedra y dos tamaños. La descarga/aplicación desde una release de GitHub en otro equipo requiere validación adicional.

## Retorno nativo del inicio de sesión

Windows utiliza `icaro://oauth/microsoft` sin `ICARO_APP_RETURN`. La asociación se prepara desde el instalador y al iniciar el acceso desde el launcher. El receptor loopback solo comunica el proceso del enlace con el intento original; en este modo devuelve un acuse vacío, sin servir la página local de autorización.

La corrección pasó 21 pruebas de autenticación y tres de integración de sesión en la interfaz. Una consulta sin cookies ni cuenta a los endpoints oficiales de Microsoft confirmó que aceptan el retorno nativo, conservan el `state` y devuelven `login_required` cuando no hay sesión. No equivale a completar el acceso con una cuenta real.

El registro del instalador se probó con su definición real adaptada únicamente a un AppId, asociación y grupo de Inicio aislados: ruta con espacios/acento, comando con el enlace como un argumento, retirada de la asociación propia y conservación de una registrada por otra copia. La asociación real `icaro` del usuario permaneció intacta.

El ejecutable release recibió un enlace de prueba mediante `--oauth-return`, entregó el código y el `state` intactos al receptor aislado y terminó sin abrir otra ventana. La prueba no canjeó un código de Microsoft ni modificó la sesión del usuario; la confirmación del navegador y el acceso completo deben comprobarse con el paquete corregido.

## Tamaño de compilaciones de desarrollo

El perfil `dev` omite símbolos de depuración para reducir el espacio de `target`. Para restaurarlos en PowerShell durante una sesión, usa `$env:CARGO_PROFILE_DEV_DEBUG = "2"` antes de ejecutar Cargo; en Bash usa `export CARGO_PROFILE_DEV_DEBUG=2`. Esto puede aumentar el espacio y el tiempo de compilación. La compilación incremental sigue activa por defecto. Las mediciones y sus limitaciones están en [el registro de optimización](superpowers/mediciones/2026-10-26-registro.md).

## Medición de compilación en GitHub Actions

El workflow Windows conserva las dependencias de pruebas en `target/ci-tests` y las de producción con CRT estático en `target/windows-package-build`. Las variantes siguen separadas por sus perfiles, target y flags. La caché depende de la versión de Rust y de los manifiestos/lockfile; una caché ausente o invalidada vuelve a construir las dependencias.

El artefacto `icaro-windows-diagnostico` reúne la revisión, toolchain, CPU/memoria del runner y los informes HTML de `cargo --timings`. No contiene los paquetes de distribución. El resumen de la ejecución indica si hubo coincidencia exacta de caché; revisar también sus pasos de restauración y guardado y los crates realmente recompilados.

Comparar dos ejecuciones manuales de la misma revisión, con los mismos parámetros, separando caché fría y caliente. El tiempo de cola se calcula aparte del tiempo de los jobs. Las ejecuciones manuales generan artefactos y no publican releases. La [línea base de Actions](superpowers/mediciones/2026-10-26-actions-linea-base.md) documenta los tiempos previos; las mejoras nuevas requieren medición remota.

La ejecución manual permite elegir 1, 2 o 4 procesos de Cargo. El valor normal sigue siendo 1 hasta disponer de una comparación estable de tiempo y memoria; las pruebas mantienen `--test-threads=1`. Para medir el empaquetado local se puede pasar `-Trabajos 2 -Medir` a `scripts/empaquetar_windows.ps1`: conserva el target/CRT y añade los informes HTML y `target/ci/release-tiempos.json` con tiempos separados del launcher y del servicio.
