# Vanilla y Java

[Documentación](README.md) · [Estado y pendientes](roadmap.md)

La creación de una instancia Vanilla prepara Minecraft y su Java automáticamente. Las instancias Vanilla anteriores se preparan al pulsar **Jugar**. La descarga no necesita una cuenta; para ejecutar el juego se requiere una sesión de Minecraft Java activa.

Assets y bibliotecas se reutilizan entre instancias desde la raíz de datos. Al preparar una instalación antigua, el launcher verifica y traslada sus archivos reutilizables al almacén común y retira las copias redundantes verificadas después de guardar el perfil actualizado. Mundos, mods y ajustes del juego permanecen en la instancia.

## Probar el recorrido

```powershell
cargo run -p icaro-ui --example launcher
```

1. Inicia sesión con Microsoft.
2. Crea una instancia seleccionando Minecraft y **Vanilla** en el asistente original.
3. Mira su preparación en **Descargas**. Puedes pausar, reanudar, cancelar y reintentar.
4. Abre la instancia y pulsa **Jugar**. El Umbral muestra registros reales y la consola recibe la salida de Java.
5. Al terminar, la instancia vuelve a estar disponible. Al jugar de nuevo, Ícaro comprueba los archivos guardados y descarga solamente los ausentes o dañados.
6. Mientras el juego está en ejecución, pulsa su botón: muestra **Cerrar juego** durante tres segundos. Un segundo clic lo detiene.

Fabric ya se prepara y lanza mediante su perfil oficial para la versión concreta elegida. Quilt, Forge y NeoForge conservan la gestión de mods; sus instaladores siguen pendientes.

## Archivos guardados

En Windows los datos están en `%APPDATA%\Icaro`; en Linux, en `$XDG_DATA_HOME/Icaro` o `~/.local/share/Icaro`.

- `instances/<id>/instance.json`: datos de la instancia.
- `instances/<id>/.icaro`: nativos extraídos y perfil instalado; assets, bibliotecas y clientes están en el almacén compartido.
- `instances/<id>/minecraft`: mundos, configuración, capturas y registros del usuario.
- `java`: runtimes oficiales gestionados y sus manifiestos de comprobación.

La comprobación inicial no vuelve a descargar una instalación válida. Los archivos verificados de Minecraft se conservan tras una cancelación; el runtime Java se reutiliza una vez publicado completo. Los binarios privados de Minecraft quedan fuera de las exportaciones ZIP.

El botón listo muestra **Jugar**, sin la memoria asignada. Al pulsarlo, la comprobación de los archivos y los componentes guardados muestra **Preparando**. **Instalando** se reserva para las descargas de archivos que faltan o no pasan la comprobación de integridad.

El indicador de descargas de la barra lateral aparece únicamente durante una descarga real. La comprobación y preparación de una instalación guardada no se cuentan como descargas; el indicador muestra el porcentaje disponible, sin inventar una velocidad de transferencia.

## Java y proceso

**Automático** usa dos modos como Prism: con descargas habilitadas usa el runtime administrado del perfil y lo descarga si falta; sin descargas detecta un Java compatible disponible, incluyendo runtimes del launcher oficial, y puede reutilizar una copia guardada por Ícaro. La compatibilidad proviene de las versiones mayores explícitas del perfil de Prism por versión; ante ausencia o fallo del complemento se mantiene el requisito oficial de Mojang. El perfil se guarda con la instalación. Una selección manual incompatible produce un error sin sustituirse automáticamente.

La pestaña reutiliza los controles existentes y muestra Java detectado del sistema, runtimes gestionados y el Java de la última preparación de la instancia, con versión, arquitectura, ruta y origen. Detectar instalaciones ejecuta las comprobaciones en segundo plano. Probar inicializa la JVM con la memoria mínima/máxima, Metaspace y argumentos de la instancia, y comprueba el perfil guardado cuando está disponible; Agregar ruta abre el selector de archivos. En Ajustes > Java y memoria se guarda la ruta global y el permiso de descarga automática. Las instancias nuevas heredan Java, descarga y memoria/argumentos globales. Cada instancia puede sustituir Java y memoria de forma independiente; las configuraciones antiguas mantienen sus valores propios. Desactivar las descargas activa la selección local y permite seguir usando Java local o guardado; muestra un error si falta uno compatible. Las selecciones manuales se conservan aunque sean inválidas, para corregirlas sin sustituciones silenciosas. La comprobación de arquitectura corresponde a la de los nativos del launcher. Los estados antiguos mantienen su selección propia y las descargas habilitadas. La memoria mínima, máxima, Metaspace y los argumentos adicionales se aplican al proceso sin pasar por un intérprete de comandos. La resolución y pantalla completa se aplican desde el editor abierto; su persistencia completa todavía está pendiente.

El launcher captura stdout y stderr, limita el registro en memoria y oculta el token de Minecraft. Una salida anormal abre el informe de fallo original con el código y los registros reales; si no se conoce la causa, no se atribuye a un mod ficticio. Cerrar el launcher o cerrar sesión detiene los procesos que controla. **Mantener abierto** y **Minimizar** están conectados; el modo de cerrar el launcher conservando el juego requiere la futura integración de bandeja.

Los eventos XML de Log4j se muestran como mensajes legibles, con su nivel, hilo y excepciones. En Windows el Umbral permanece hasta detectar una ventana visible de Minecraft del proceso iniciado, después de completar su entrada. La detección admite GLFW, SDL y LWJGL; iniciar Java o agotar el tiempo de observación no cuenta como tener el juego listo. Volver retira la capa sin detener Minecraft. La vista del launcher permanece montada debajo para evitar que vuelva a cargar al terminar la transición. La detección equivalente en Linux sigue pendiente.

La salida del Umbral usa un fundido de la capa completa de 200 ms (120 ms con reducir movimiento), sin recorte vertical. La consola reutiliza el panel original del diseño, con filtros independientes, búsqueda resaltada, autoscroll que se pausa al subir, copiar/limpiar, selector de instancia y exportación. Sin juego permite abrir el último `minecraft/logs/latest.log` guardado. El visor dibuja únicamente las filas visibles del registro.

Las capturas hechas con **F2** quedan en `instances/<id>/minecraft/screenshots`. La galería global y la pestaña Capturas las leen en segundo plano y se actualizan cada dos segundos mientras están abiertas. Galería y visor usan el archivo original, sin reducir su resolución. En Windows se puede copiar la imagen o la selección de archivos y abrir su carpeta. La eliminación real aún está pendiente; su acción informa que el archivo se conserva.

## Validación reproducible

```powershell
cargo test -j 1 -p icaro-core --lib
cargo test -j 1 -p icaro-ui --example launcher
cargo test -j 1 -p icaro-core minecraft::tests::metadatos_oficiales_actuales_y_legacy --lib -- --ignored --nocapture
cargo test -j 1 -p icaro-core minecraft::tests::instala_vanilla_real_y_reutiliza_archivos_verificados --lib -- --ignored --nocapture
cargo test -j 1 -p icaro-core java::tests::runtime_oficial_descarga_y_reutiliza_java --lib -- --ignored --nocapture
```

Las pruebas de red usan carpetas temporales aisladas, sin cuentas ni instancias del usuario. La instalación completa comprueba cliente, assets, nativos, reutilización y reparación de un JAR alterado. La prueba de Java descarga el runtime oficial, ejecuta `java -version` y comprueba su reutilización. Las pruebas del proceso usan un ejecutable local de prueba para verificar argumentos, registros y cancelación. La llegada al menú del juego con una cuenta real se valida manualmente.

Validación del 11 de octubre de 2026: 65 pruebas del núcleo, 49 de la biblioteca de interfaz y 22 del launcher aprobadas. Cubren registros XML, espera de la ventana, preparación de caché sin anunciar descarga, capturas originales y preservación del visor, búsqueda Unicode, virtualización y salida del Umbral sin cortina. En la validación anterior también pasaron las pruebas de red de metadatos actuales y antiguos, instalación completa de Vanilla 1.21.1 con reutilización y reparación, y descarga y reutilización de Java 21. Se generaron las vistas de consola y su estado vacío, capturas reales y visor, y salida del Umbral en Tinta y Piedra. El usuario confirmó previamente que Vanilla 26.3 abre y permanece funcionando en Windows; el recorrido final de arranque corregido todavía debe volver a probarse con el juego real.


Reorganización del 13-10-2026: `instance.json` de esquema 2 conserva identidad y ajustes de cada instancia; `estado.json` guarda preferencias globales. El CFG anterior se archiva dentro del JSON antes de retirarlo. Eliminar usa la papelera del sistema, y Almacenamiento muestra tamaños reales y permite limpiar solo la caché, recuperar operaciones interrumpidas y trasladar papeleras internas antiguas. Las capturas reales conectan eliminación y Deshacer con el sistema.

## Fabric (13-10-2026)

La preparación hereda los assets, cliente y nativos de Minecraft e incorpora la clase de arranque, argumentos y bibliotecas del [perfil oficial de Fabric](https://meta.fabricmc.net/). Guarda el perfil en `meta/fabric/<minecraft>/<loader>/perfil.json`; las bibliotecas entran en el almacén compartido y se verifican por tamaño y SHA-1. Si el perfil no incluye el hash de una biblioteca, se consulta su archivo `.sha1` en el Maven oficial. Se conserva la versión elegida en `loader.json`; cambiarla invalida el perfil instalado anterior.

La compatibilidad de Java resulta de las versiones admitidas por Minecraft y el mínimo publicado por Fabric. Crear e Instalar/Jugar usan el mismo instalador; después de Cambiar versión, el siguiente arranque prepara el nuevo perfil. La cancelación y los errores conservan el marcador anterior. La validación incluye argumentos oficiales, memoria, perfil fijado, reutilización y descarga oficial de bibliotecas. El arranque con cuenta y un mod real necesita validación manual.

```powershell
cargo test -j 1 -p icaro-core fabric_oficial_descarga_verifica_y_reutiliza_perfil --lib -- --ignored --nocapture
cargo test -j 1 -p icaro-core instala_fabric_real_y_reutiliza_sin_descargas --lib -- --ignored --nocapture
```

Validación del 13-10-2026 para Fabric: instalación completa de Minecraft 1.21.1/Fabric 0.16.14 en una carpeta temporal y segunda preparación sin descargas, perfil oficial y bibliotecas verificadas. El renombre de la instancia local a `.icaro` conservó el estado listo. El usuario también confirmó el arranque de Fabric 26.3 con un mod compatible en Windows; Linux sigue pendiente.

Comprobaciones de esta entrega: 121 pruebas del núcleo y 35 del launcher aprobadas, además de las dos pruebas de red de Fabric (bibliotecas oficiales e instalación completa). Ejecutable de desarrollo compilado.

### Versiones sin ofuscación

A partir de Minecraft 26.1, el perfil oficial de Fabric omite Intermediary. El API utiliza la referencia `net.fabricmc:intermediary:0.0.0` como marcador; el instalador acepta su ausencia únicamente cuando coinciden el perfil y los metadatos oficiales. Los perfiles antiguos conservan la comprobación de Intermediary para la versión exacta. Los perfiles cacheados anteriores mantienen ese requisito.

La auditoría del catálogo consulta todos los perfiles publicados, incluyendo releases y snapshots, y valida identidad, herencia, bibliotecas y mínimo de Java. Es una comprobación de metadatos; no equivale a arrancar cada versión con todos sus mods.

```powershell
cargo test -j 1 -p icaro-core fabric_catalogo_completo_respeta_contrato_oficial --lib -- --ignored --nocapture
```

Validación ampliada del 13-10-2026: los 531 perfiles del catálogo oficial de Fabric pasaron la auditoría con Fabric 0.19.5. Se corrigió también la validación de nombres históricos con espacios, como `1.14.2 Pre-Release 4`, en la configuración y las rutas Maven. La instancia local Fabric 26.3/0.19.5 completó su preparación, guardó el perfil Java 25 y reutilizó los componentes. Esta comprobación no arranca el juego ni accede a cuentas.

El usuario confirmó el arranque de Fabric 26.3 y la carga de un mod real en Windows el 13-10-2026. Las comprobaciones de otras versiones y plataformas siguen su alcance de validación independiente.

En Descargas, las filas completadas o con error tienen **Quitar de Descargas**. La acción retira únicamente la fila visible; conserva la instancia, sus archivos, Java y los registros. Las descargas activas mantienen Pausar/Cancelar; una cancelación debe terminar antes de retirar su fila. El asistente muestra el selector de versión de loader únicamente en la tarjeta seleccionada.

El menú de instancia separa **Cambiar versión** (Minecraft) y **Cambiar versión del loader**. Cambiar Minecraft conserva la familia del loader y prepara su versión oficial compatible para el destino. Cambiar el loader permite elegir su versión para el Minecraft actual, conserva los mods y mundos, y ofrece respaldo antes de publicar `loader.json`. La nueva selección invalida el perfil preparado; el próximo arranque lo verifica e instala. Vanilla no muestra esta acción. La eliminación utiliza **Continuar con la eliminación** y tiene **Cancelar** en la pantalla de confirmación por pulsación mantenida.

## Modpacks de Modrinth

La ficha tiene **Instalar modpack** y cada fila de Versiones ofrece su acción de instalación mediante un icono. La acción revisa el MRPACK oficial y crea una instancia independiente, sin comparar el pack con la instancia seleccionada. La revisión toma Minecraft y el loader de `modrinth.index.json` y permite incluir o excluir los archivos opcionales de cliente. En este momento se preparan packs Vanilla y Fabric; Quilt, Forge y NeoForge muestran su dependencia pendiente antes de crear la instancia.

Los archivos referenciados se verifican con SHA-512 y SHA-1, se omiten los exclusivos de servidor y se aplican `overrides` y después `client-overrides`. La descarga sigue los orígenes definidos por la [especificación MRPACK](https://support.modrinth.com/en/articles/8802351-modrinth-modpack-format-mrpack), con redirecciones acotadas y comprobadas. Las rutas, enlaces, colisiones por mayúsculas y tamaños de extracción se validan antes de instalar. Un archivo local modificado se conserva; una instalación completa conserva también la configuración modificada al jugar. La cola existente administra pausa, cancelación y reintento. La importación local de archivos y las actualizaciones de modpack permanecen pendientes.

```powershell
cargo test -j 1 -p icaro-core modpacks::tests --lib
cargo test -j 1 -p icaro-core modpack_oficial_crea_instancia_instala_y_reutiliza --lib -- --ignored --nocapture
```

Validación nativa del 13-10-2026: Fabulously Optimized 14.1.1, Minecraft 26.2 y Fabric 0.19.5, con archivos y perfil completos y segunda preparación sin descargas. La prueba no usa cuentas ni arranca el juego.
