# Forge, NeoForge y Fabric API

## Instalación y lanzamiento

Crear una instancia, Instalar/Jugar y preparar una nueva versión admiten Vanilla, Fabric, Forge y NeoForge. Se conserva la versión exacta guardada en loader.json; sincronizar no selecciona automáticamente otra versión. Quilt conserva su instalación mediante el perfil oficial.

Forge y NeoForge se descargan de sus Maven oficiales. Se verifica el SHA-1 del instalador antes de ejecutarlo. El instalador oficial prepara el cliente, las bibliotecas y los procesadores dentro de meta/loaders, en una carpeta nueva por intento. Recibe una copia independiente del cliente de Mojang y un perfil de launcher vacío; no recibe cuentas, tokens, mundos ni mods.

Se respeta el Java seleccionado y el permiso de descargarlo. Los argumentos de juego, módulos y clase principal proceden del perfil oficial. Las bibliotecas del loader sustituyen las versiones equivalentes de Vanilla en el classpath. Las rutas Maven y los clasificadores se validan; no se aceptan rutas que salgan del almacenamiento del launcher.

La caché común se publica al terminar con éxito, después de verificar las bibliotecas contra sus hashes oficiales y registrar los hashes de los archivos producidos por los procesadores. Las preparaciones posteriores vuelven a comprobar esos archivos antes de reutilizarlos. Una caché alterada requiere reparar; no se certifica por tamaño o fecha. El marcador activo de la instancia se publica al final.

Cancelar termina el proceso del instalador y conserva la instalación anterior. Una preparación fallida no se usa para lanzar. Su carpeta conserva instalador.log para diagnóstico; un reintento crea otra preparación. No se borran automáticamente estos diagnósticos ni las preparaciones anteriores que pueden seguir usando otras instancias.

Mientras trabaja el instalador, el launcher lee su registro de forma incremental y muestra la fase, el número de bibliotecas o clases parcheadas y el tiempo transcurrido. No calcula un porcentaje de procesado porque el instalador no declara un total fiable. Se conserva el límite de veinte minutos y la cancelación del proceso. Forge 66.1.0 para Minecraft 26.3 completó la preparación y las comprobaciones de caché en unos 145 segundos en la prueba local del 23-10-2026; ese tiempo no es una garantía para otros equipos o conexiones.

La adaptación opcional de escritura reduce las llamadas al sistema del parcheador: agrupa su salida en bloques de 64 KiB, sin reescribir los JAR oficiales en disco. Solo adapta en memoria `net.minecraftforge.binarypatcher.Patcher` cuando su SHA-256 coincide con la implementación medida de BinaryPatcher 1.3.4. Los bytes del JAR generado se conservan. Otras implementaciones siguen usando su E/S original.

El adaptador se compila desde `BufferAgent.java` dentro de la preparación aislada, con el `javac` del mismo runtime elegido. Se comprueba que el runtime pueda cargarlo antes de iniciar el instalador. Si faltan el compilador o la instrumentación, o falla esa comprobación, se conserva el recorrido original. No se descarga otro JDK para activar esta mejora. El agente solo acompaña al proceso de instalación de Forge; no se añade a Minecraft ni a NeoForge o Quilt.

En la [medición del 23-10-2026](investigaciones/rendimiento-forge-2026-10-23.md), el procesador pasó de 114,079 a 4,719 segundos y produjo el mismo SHA-256. La prueba del instalador completo con tres comprobaciones de caché adicionales terminó en 23,66 segundos. Son mediciones locales, no una garantía de tiempo para otras versiones o equipos.

El adaptador admite el contrato oficial de instalación spec 1, con perfil JSON heredado de Minecraft. Los instaladores antiguos con otros formatos se rechazan explícitamente; no se acredita soporte para todas las versiones históricas de Forge. La verificación del instalador no sustituye la confianza en el código oficial que ejecuta sus procesadores.

## Fabric API por defecto

La primera instalación normal de una instancia Fabric busca Fabric API (proyecto oficial de Modrinth P7dR8mSH) compatible con la versión exacta de Minecraft y Fabric. Se usa el instalador de mods existente, con dependencias obligatorias y verificación SHA-512.

Se conserva una copia ya presente, incluida una copia local renombrada o desactivada identificada mediante fabric.mod.json. Los MRPACK y ZIP importados conservan el conjunto declarado por su autor. Una reparación posterior no vuelve a añadir Fabric API si el usuario lo retiró. Cambiar únicamente el loader tampoco modifica esa elección.

Si no existe una versión compatible, se informa durante la preparación y se instala Fabric sin ese mod. Los fallos de red o integridad se muestran como errores y permiten reintentar conservando los archivos anteriores.

## Modpacks y validación

MRPACK, ZIP completos de CurseForge y perfiles Prism/MultiMC pueden declarar Forge o NeoForge y fijar su versión. Los ZIP de CurseForge que solo contienen IDs de archivos siguen necesitando su integración de distribución; este cambio no descarga esos archivos por otra vía.

Las pruebas locales cubren perfiles oficiales, argumentos de módulos, sustitución de bibliotecas, rutas Maven, importación con versión fijada y conservación de mods/Fabric API. La prueba ignorada instaladores_oficiales_procesan_cliente_y_reutilizan_cache_verificada descarga los instaladores y clientes reales en target/qa-loaders y comprueba también la caché sin red y el rechazo de una biblioteca alterada. Conserva allí los registros y los runtimes para repetirla sin descargas innecesarias.

Compilar y preparar el cliente no acredita una partida: siguen pendientes las pruebas con un mod compatible, migración, reinicio, cancelación durante procesadores y recorrido nativo en Linux.

Validación local del 22-10-2026: compilación del launcher completada; 58 pruebas de UI y 67 del launcher aprobadas; 2 del actualizador aprobadas. Tras integrar Quilt y los canales, el núcleo obtuvo 245 aprobadas, excluyendo resourcepacks_bulk_activation_removal_preserves_unselected_and_renamed_update: había fallado antes y se bloqueó al repetirla, por lo que se interrumpió esa ejecución. Su código no se modificó. Pasaron las pruebas reales de los tres instaladores indicados, la caché sin red, la biblioteca alterada y la descarga/reutilización de Fabric API de Modrinth. Esto no informa el estado del CI remoto.
