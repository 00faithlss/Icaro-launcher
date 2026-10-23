# Organización de datos y configuración

Investigación del **12-10-2026**. La reorganización funcional está implementada para las funciones existentes del launcher. El usuario confirmó el funcionamiento de Vanilla con la estructura nueva en Windows. La validación nativa en Linux sigue pendiente.

## Qué hace Prism Launcher

Prism usa `instance.cfg` como archivo de ajustes INI: texto con claves y valores. Guarda nombre, icono y ajustes específicos de la instancia, con opciones para heredar o sustituir valores globales de Java, memoria y ejecución. La extensión `.cfg` no aporta por sí sola rendimiento ni compatibilidad. Las versiones de Minecraft y los componentes del loader se describen por separado en `mmc-pack.json`.

Sus archivos reutilizables están en el directorio de datos del launcher: assets, bibliotecas, runtimes Java, caché y metadatos. La carpeta de cada instancia contiene sus datos particulares del juego. Las carpetas de temas, traducciones y catpacks corresponden a funciones de Prism; no necesitamos crearlas en Ícaro sin un uso real.

Prism sí conserva cachés y registros. Al eliminar una instancia, su código llama a la papelera del sistema mediante `QFile::moveToTrash`, con una adaptación al portal de Flatpak. No necesita una carpeta `.papelera` dentro de `instances`. El historial para deshacer guarda el destino devuelto por esa operación; la recuperación depende de la plataforma. Si enviar la instancia a la papelera falla, esa operación devuelve un error, sin recurrir al borrado permanente de la instancia.

Fuentes primarias consultadas:

- [Ubicación y finalidad de las carpetas de Prism](https://prismlauncher.org/wiki/getting-started/data-location/).
- [Lectura y escritura de ajustes INI](https://github.com/PrismLauncher/PrismLauncher/blob/develop/launcher/settings/INISettingsObject.cpp).
- [Ajustes propios y heredados de una instancia](https://github.com/PrismLauncher/PrismLauncher/blob/develop/launcher/BaseInstance.cpp) y [ajustes de Minecraft, Java y memoria](https://github.com/PrismLauncher/PrismLauncher/blob/develop/launcher/minecraft/MinecraftInstance.cpp).
- [Perfil de componentes y mmc-pack.json](https://github.com/PrismLauncher/PrismLauncher/blob/develop/launcher/minecraft/PackProfile.cpp).
- [Eliminación y recuperación de instancias](https://github.com/PrismLauncher/PrismLauncher/blob/develop/launcher/InstanceList.cpp) y [papelera del sistema](https://github.com/PrismLauncher/PrismLauncher/blob/develop/launcher/FileSystem.cpp).

## Estado comprobado en Ícaro

Las carpetas de instancia tienen nombres legibles y un ID interno estable. `instance.json` de esquema 2 es la única fuente editable: identidad y ajustes Java/memoria. Los CFG antiguos se leen durante la migración, sus claves se conservan en `legacy_cfg` y se retira el archivo CFG después de publicar el JSON. Los campos desconocidos de JSON y configuración se conservan al guardar. Los ajustes de instancias se trasladan desde `estado.json` por ID; el estado global conserva apariencia, grupos y preferencias globales.

El instalador Vanilla comparte `assets/`, `libraries/<sha1>/<ruta Maven>` y `meta/minecraft/` en la raíz de datos. Los índices de assets incluyen su SHA-1 en el nombre; el cliente y los metadatos también se separan por identidad verificada. Cada instancia conserva su marcador `instalado.json` y sus nativos extraídos en `.icaro`. Java se administra en `Icaro/java`, y los registros del juego pertenecen a `minecraft/logs`.

Al preparar una instalación antigua, se verifican tamaño y SHA-1 de sus copias contra el manifiesto actual, se publican archivos completos en el almacén común y se guarda el marcador de esquema 2. Solo después se eliminan las copias antiguas verificadas que ya tienen un destino válido, retirando sus carpetas si quedan vacías. Una cancelación previa a la publicación conserva esas copias. Las instancias de esquema 1 siguen siendo legibles hasta prepararse. Un bloqueo del sistema operativo coordina las preparaciones del mismo almacén, incluso entre procesos; la espera se puede cancelar. El archivo de bloqueo permanece en disco, pero el SO libera el bloqueo al cerrar el proceso.

La selección actual de Java respeta la ruta manual y usa dos modos automáticos como Prism. Con descargas habilitadas reutiliza o prepara el runtime administrado del perfil; con descargas deshabilitadas detecta instalaciones compatibles locales, incluidos runtimes del launcher oficial de Minecraft, y puede reutilizar las copias guardadas por Ícaro. La selección automática usa la lista explícita de versiones mayores del perfil; si los metadatos de Prism no están disponibles, se conserva el requisito oficial de Mojang. Una ruta seleccionada manualmente, global o por instancia, permite una versión igual o superior al mínimo del perfil: por ejemplo, Java 25 para un perfil que recomienda Java 21. Esta elección no amplía la lista de versiones recomendadas automáticamente.

La detección consulta `JAVA_HOME`, `PATH`, claves JavaSoft y carpetas de proveedores en Windows; en Linux busca en `/usr/lib/jvm`, `/usr/java`, `/opt/java`, `~/.jvm` y `~/.sdkman/candidates/java`. También inspecciona `.minecraft/runtime` y el runtime del launcher de Microsoft Store. El recorrido de runtimes oficiales está acotado, no sale de su raíz y deduplica ejecutables reales. Una instalación fuera de estas ubicaciones puede seleccionarse manualmente.

Eliminar instancias, archivos, mods, capturas y packs usa la papelera del sistema. Las instancias, mods y capturas conectan Deshacer durante la sesión; después se puede recuperar desde la papelera del sistema. Un fallo al enviar no autoriza un borrado permanente. En Windows se usa `IFileOperation` con reciclaje obligatorio; las rutas de 260 caracteres o más se rechazan antes de operar.

Las papeleras internas antiguas se detectan en Almacenamiento y se trasladan mediante una acción explícita. No se vacían al arrancar. Las retiradas de mods usan `.icaro-transactions` con un diario recuperable: Recuperar operaciones reanuda movimientos interrumpidos, comprueba hashes y rechaza colisiones. Los respaldos propios de instalación se retiran después de completar la operación; los datos de recuperación anteriores no se tratan como caché.

La organización de archivos compartidos debe cambiar en el instalador del launcher y en la construcción del lanzamiento. No aparecerá automáticamente al integrar los instaladores oficiales de loaders, y afecta también a Vanilla.

## Estructura objetivo

En Windows, conservar `%APPDATA%\Icaro` como raíz de datos. La instalación del ejecutable y los datos del usuario deben poder actualizarse de forma independiente.

```text
Icaro/
  assets/                   # Objetos e índices reutilizables de Minecraft
  libraries/<sha1>/         # Bibliotecas verificadas, con su ruta Maven
  java/                     # Runtimes administrados seleccionados según el perfil
  meta/minecraft/           # Catálogo, metadatos por versión/hash y clientes por hash
  meta/fabric/              # Perfiles oficiales por versión de Minecraft y Fabric
  meta/loaders/             # Instaladores y clientes procesados de Forge/NeoForge
  cache/                    # Respuestas y descargas temporales recuperables
  logs/                     # Registros del launcher y de sincronización, con rotación
  estado.json               # Preferencias globales, sin configuraciones activas por instancia
  opciones-minecraft.json   # Sensibilidad, teclas y gráficos compartidos del jugador
  instances/
    boys/
      instance.json         # Identidad y configuración por instancia, esquema 2
      icon.png
      icono.json
      .icaro/
        instalado.json      # Perfil instalado local, esquema 2
        modpack.json        # Plan y estado del MRPACK, cuando corresponde
        natives-<id>/       # Nativos extraídos de esta instancia
      minecraft/
        mods/
        resourcepacks/
        shaderpacks/
        saves/
        screenshots/
        logs/               # Registros de Minecraft de esta instancia
        options.txt
```

El perfil instalado y los nativos permanecen privados; los registros de procedencia de mods y recursos conservan sus ubicaciones actuales. Las exportaciones toman el contenido del juego y reconstruyen la configuración portable, sin incluir el almacén compartido ni el marcador con rutas locales. El CFG antiguo deja de ser una segunda fuente editable al migrarse.

## Java según el perfil y modo de gestión

**Implementado para Vanilla:** selección manual global o por instancia, herencia, detección de Java, comprobación de arquitectura, modo administrado y permiso de descarga persistente. Los requisitos de loaders deben integrarse con sus perfiles cuando se implemente su instalación; actualmente no se presentan como jugables.

1. **Ruta manual efectiva:** comprobar el ejecutable, la arquitectura y que su versión no sea inferior al mínimo del perfil. Se permite una versión superior elegida por el usuario y no se sustituye silenciosamente por el runtime recomendado. Ante un error, conservar la ruta para corregirla.
2. **Automático con descarga habilitada:** reutilizar una copia verificada del componente administrado requerido o descargarlo del catálogo oficial de Mojang. Este modo no elige primero un Java del sistema; corresponde al flujo administrado de Prism.
3. **Automático sin descarga:** detectar Java compatible disponible, ordenar candidatos compatibles por versión y usar el más reciente admitido. Si no hay uno local, comprobar runtimes guardados por Ícaro. Si tampoco hay uno compatible, mostrar qué Java falta; no descargar.

El perfil guarda las versiones mayores permitidas y el componente administrado. Para Vanilla se consultan `compatibleJavaMajors` y `compatibleJavaName` en los metadatos públicos de Prism por versión, validando identidad y formato. El resultado se guarda en el perfil instalado. Si falta o falla el complemento, usar `javaVersion` oficial de Mojang; las instalaciones antiguas conservan ese respaldo hasta prepararse nuevamente. La ausencia de red no amplía la lista compatible. Las respuestas de Prism tienen límite de tamaño, HTTPS sin redirecciones, cancelación y límite de tiempo.

La pestaña Java muestra ruta, versión, arquitectura, origen y la última preparación. **Probar** inicializa una JVM separada con `-Xms`, `-Xmx`, Metaspace y los argumentos de esa instancia, usando el mismo parser y validación del lanzamiento. Comprueba el perfil guardado incluso después de reiniciar; si aún no hay un perfil instalado, la prueba de JVM no certifica compatibilidad con Minecraft. Las instancias pueden heredar memoria, Metaspace y argumentos globales o sustituirlos. Los ajustes antiguos conservan su memoria propia. La prueba global de ruta usa memoria básica de comprobación; las pruebas por instancia usan sus ajustes efectivos. Una prueba correcta no equivale a llegar al menú del juego.

Fuentes: [selección automática de Prism](https://github.com/PrismLauncher/PrismLauncher/blob/develop/launcher/minecraft/launch/AutoInstallJava.cpp), [detección de runtimes](https://github.com/PrismLauncher/PrismLauncher/blob/develop/launcher/java/JavaUtils.cpp), [comprobación de Java](https://github.com/PrismLauncher/PrismLauncher/blob/develop/launcher/java/JavaChecker.cpp) y [metadatos por versión](https://github.com/PrismLauncher/meta-launcher/tree/master/net.minecraft).

Los Java del sistema permanecen en su ubicación original: no copiarlos a `Icaro/java`, modificarlos ni eliminarlos desde la limpieza de almacenamiento. Conservar los runtimes administrados existentes durante la migración. Las exportaciones no deben imponer rutas absolutas del PC de origen: conservar la intención de selección y solicitar resolverla en el equipo de destino.

## Alcance de la reorganización actual

Las instalaciones existentes son datos de prueba. El usuario autoriza eliminar copias redundantes de archivos de instalación o regenerarlas cuando sea necesario para reorganizar el almacenamiento compartido; no se requieren respaldos de esas copias.

El 13-10-2026 se migró la instancia local Vanilla 26.3: 5.215 archivos verificados, perfil de esquema 2 y 65 entradas de classpath. Las carpetas antiguas de assets, bibliotecas y versiones quedaron retiradas; una segunda preparación reutilizó el almacén común sin anunciar descargas. La llegada al menú con estas rutas queda pendiente de prueba manual.

## Trabajo pendiente y criterios

- [x] Unificar las rutas de instalación, lanzamiento, verificación, cambio de versión y exportación existentes. La importación futura debe reconstruir la instancia mediante `Almacen`, sin reutilizar marcadores ni rutas absolutas del ZIP.
- [x] Compartir assets y bibliotecas Vanilla por ruta e identidad verificadas. Coordinar instalaciones simultáneas y publicar archivos completos mediante escrituras atómicas.
- [x] Implementar la reorganización al preparar Vanilla: verificar archivos reutilizados, actualizar perfiles y retirar copias redundantes verificadas sin respaldos. Las pruebas comprueban la construcción del lanzamiento con rutas compartidas; la llegada al menú sigue siendo una comprobación manual.
- [x] Definir una sola fuente de configuración por instancia, con versión de esquema, herencia global y ajustes de Java/memoria portables. Conservar campos desconocidos durante la migración de JSON/CFG; evitar dos fuentes editables que puedan contradecirse.
- [x] Separar modo administrado y detección local, comprobar listas de compatibilidad por perfil y deduplicar runtimes oficiales por ejecutable real.
- [x] Comprobar explícitamente la arquitectura y persistir la selección global/por instancia y la opción de descarga automática. Los estados anteriores conservan su selección y no heredan automáticamente una nueva ruta global.
- [ ] Completar el recorrido real en Windows y Linux: reinstalación, cambio de versión, importación, varias versiones locales, perfil guardado tras reinicio y Java administrado. Las pruebas aisladas cubren listas explícitas, memoria/argumentos, prioridad de ambos modos, respaldo sin red y selección manual incompatible.
- [x] Usar la papelera del sistema para las eliminaciones del usuario y conectar recuperación donde la plataforma lo permita. Si falla, mostrar el error; no convertirla silenciosamente en borrado permanente.
- [x] Ofrecer recuperación o traslado explícito de los datos de las papeleras internas antiguas. No vaciarlas al actualizar ni cargar el launcher.
- [x] Separar respaldos de transacciones de las eliminaciones del usuario. Limpiar temporales al finalizar una operación y recuperar operaciones interrumpidas antes de retirar sus respaldos.
- [x] Incorporar tamaños reales, limpieza explícita de caché y rotación de registros en Almacenamiento. Excluir mundos, mods, iconos y configuración de la limpieza automática.
- [x] Verificar dos instancias de la misma versión reutilizando assets/bibliotecas, cambios de nombre y versión, cancelación, descarga corrupta, instancias antiguas y exportaciones sin rutas absolutas del equipo.

La integración de loaders debe apoyarse en estas rutas compartidas. La migración y las eliminaciones necesitan comprobaciones tanto en Windows como en Linux antes de considerarlas completas.


## Almacenamiento y mantenimiento

Almacenamiento mide archivos reales fuera del hilo de dibujo, muestra todas las categorías de la raíz y ofrece Actualizar, Limpiar caché, Recuperar operaciones y traslado de papeleras antiguas. La limpieza se limita a `cache/`; no retira assets, bibliotecas, runtimes, mundos, mods, iconos ni configuración. El recorrido tiene límites y rechaza enlaces y uniones de carpetas. Los registros `logs/launcher.log` y `logs/sincronizacion.log` rotan al superar 1 MiB y conservan tres archivos anteriores cada uno. No se registran tokens ni datos de cuentas.

Las exportaciones de Ícaro incluyen los ajustes portables conocidos; una selección manual de Java conserva una marca que obliga a resolverla en el PC de destino. No se exportan rutas absolutas de Java ni campos locales desconocidos. El ZIP no incluye el almacén común ni los marcadores de instalación. La importación completa y los instaladores de Forge/NeoForge están conectados; Quilt y la validación de partida/migración siguen pendientes, como se indica en `pendientes-implementacion.md`.

Migración local del 13-10-2026: se trasladaron 15 elementos de las papeleras internas a la papelera del sistema; `instances/.papelera` quedó retirado. No había instancias activas al ejecutar este mantenimiento.

### Nombre de la carpeta interna

La carpeta privada de cada instancia se llama `.icaro`. Al cargar las instancias o prepararlas, se migra `.icaro-minecraft` mediante renombre y se actualizan sus referencias en `instalado.json`, preservando campos adicionales, nativos y archivos del juego. Si existen ambos nombres, la migración se detiene con un error para conservar ambos perfiles. `instance.json` sigue siendo la configuración de la instancia: cambiar su extensión a `.cfg` no aporta rendimiento ni compatibilidad por sí solo.

Fabric comparte las mismas rutas de assets y bibliotecas; sus metadatos están en `meta/fabric/<minecraft>/<loader>`. Forge y NeoForge conservan sus instaladores verificados y preparaciones compartidas en `meta/loaders/<familia>/<minecraft>/<loader>`. Cada preparación tiene bibliotecas en disposición Maven para conservar las rutas de módulos del perfil oficial; no modifica la carpeta de juego activa. Consulta [recuperación y alcance](loaders-forge-neoforge.md). Quilt sigue pendiente.
