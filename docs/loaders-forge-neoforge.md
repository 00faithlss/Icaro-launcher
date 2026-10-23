# Forge, NeoForge y Fabric API

## Instalación y lanzamiento

Crear una instancia, Instalar/Jugar y preparar una nueva versión admiten Vanilla, Fabric, Forge y NeoForge. Se conserva la versión exacta guardada en loader.json; sincronizar no selecciona automáticamente otra versión. Quilt conserva su instalación mediante el perfil oficial.

Forge y NeoForge se descargan de sus Maven oficiales. Se verifica el SHA-1 del instalador antes de ejecutarlo. El instalador oficial prepara el cliente, las bibliotecas y los procesadores dentro de meta/loaders, en una carpeta nueva por intento. Recibe una copia independiente del cliente de Mojang y un perfil de launcher vacío; no recibe cuentas, tokens, mundos ni mods.

Se respeta el Java seleccionado y el permiso de descargarlo. Los argumentos de juego, módulos y clase principal proceden del perfil oficial. Las bibliotecas del loader sustituyen las versiones equivalentes de Vanilla en el classpath. Las rutas Maven y los clasificadores se validan; no se aceptan rutas que salgan del almacenamiento del launcher.

La caché común se publica al terminar con éxito, después de verificar las bibliotecas contra sus hashes oficiales y registrar los hashes de los archivos producidos por los procesadores. Las preparaciones posteriores vuelven a comprobar esos archivos antes de reutilizarlos. Una caché alterada requiere reparar; no se certifica por tamaño o fecha. El marcador activo de la instancia se publica al final.

Cancelar termina el proceso del instalador y conserva la instalación anterior. Una preparación fallida no se usa para lanzar. Su carpeta conserva instalador.log para diagnóstico; un reintento crea otra preparación. No se borran automáticamente estos diagnósticos ni las preparaciones anteriores que pueden seguir usando otras instancias.

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
