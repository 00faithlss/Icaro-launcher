# Validaciones iniciales — octubre de 2026

[Historial técnico](../README.md) · [Estado actual](../../roadmap.md)

Registro trasladado desde Desarrollo el 11-11-2026. Se conservan resultados, comandos y límites de su momento; no son instrucciones de arranque ni evidencia de la versión actual. Las acciones de mantenimiento descritas pueden modificar datos y no forman parte de las pruebas aisladas.

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


## Validación de Fabric, MRPACK y ajustes de interfaz

Pasaron 127 pruebas del núcleo, 55 de componentes y 38 del launcher, sin fallos. Quedaron omitidas por defecto 12 pruebas del núcleo y dos del launcher que necesitan servicios o fixtures externos. La instalación real de Fabulously Optimized 14.1.1 con Minecraft 26.2/Fabric 0.19.5 y la reutilización sin descargas se comprobaron por separado; esa prueba no inicia el juego ni usa una cuenta. El usuario confirmó Fabric 26.3 con un mod real en Windows.

Se inspeccionaron capturas de modpacks y eliminación en Tinta/Piedra, incluidos los chips de versiones, licencia larga y Cancelar alineado al pie de la tarjeta. El ejecutable compiló y git diff --check pasó. Los comandos reproducibles de instalación están en [Vanilla, Fabric y Java](../../vanilla-y-java.md).

## Servidores reales y detección de ZeroTier

Las escenas `servidores`, `servidores-vacio`, `servidores-formulario`, `red-privada-autorizada`, `red-privada-ausente`, `sincronizacion-revision`, `sincronizacion-confirmar` y sus variantes con sufijo `-angosto` usan fixtures aislados. `ICARO_TEMA_DISENO=piedra|tinta` fija el tema de la captura. No consultan cuentas ni redes del usuario. El diagnóstico `detectar_zerotier` es de lectura. La [guía de servidores](../../servidores.md) y la [guía del servicio de mods](../../sincronizacion-servidores.md) explican el uso y sus límites.

El servicio del anfitrión se compila con `cargo build -j 1 -p icaro-core --example sync_host`; su ayuda muestra los argumentos de datos, instancia e IP privada de escucha. El [empaquetado Windows](../../instalador-windows.md) genera launcher, servicio, portable e instalador desde la misma versión y reúne los avisos de licencia de sus dependencias.

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
