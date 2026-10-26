# Changelog

## [Sin publicar]

- El perfil Microsoft recordado muestra su nombre mientras se renueva la sesión en segundo plano. Si falla, conserva el perfil como vencido sin habilitar acceso autenticado; no guarda tokens fuera del almacén protegido.
- El canal beta no ofrece una estable de la misma versión como actualización por su número de compilación. Tras aplicar una actualización, la pantalla ofrece «Continuar».
- El refresco de la galería utiliza índices para conservar imágenes y selección sin comparar cada captura con todas las anteriores. Las mediciones y límites figuran en el [registro de optimización](docs/superpowers/mediciones/2026-10-26-registro.md).
- Se reduce el tamaño de artefactos de desarrollo desactivando símbolos por defecto y se añade caché de Cargo al flujo Windows. El beneficio en GitHub y el launcher Windows aún debe medirse.
- Se separan las pruebas de sincronización e instalación de Minecraft y se comprueba la colisión de nombres antes de recuperar una instancia de la papelera.

## [0.3.3] - 2026-10-24

- Instalar mods desde la tienda e importar JAR ya no fallan con «Los archivos o metadatos de mods no son válidos» en instancias con dos JAR del mismo proyecto, como dos versiones de Sodium. La sincronización ya no registra ambos con la misma identidad, y un inventario guardado así conserva el primero; el otro sigue en `mods` como archivo local, sin borrar nada.
- «Ver detalle» en un aviso de error muestra ese error en lugar de la pantalla de falla de la tienda con datos fijos.

## [0.3.2] - 2026-10-24

- Reduce las imágenes empaquetadas de unos 192 MB a 104 MB. Los grabados que no se recortan, invierten ni escalan conservan su WebP original en lugar de recodificarse sin pérdida (mismos píxeles), y la variante Tinta que solo invierte o repite Piedra se obtiene al decodificar; si es igual, comparte la textura.
- Las descargas reales ya no reconstruyen la interfaz en cada cuadro: las barras de progreso se redibujan solas. Los avisos piden cuadros solo durante su entrada y su fundido.
- La revisión de resourcepacks y shaders consulta las fichas por lotes y revisa hasta cuatro paquetes a la vez, en lugar de tres solicitudes seguidas por paquete.
- Las consultas a cuentas, Java, catálogos y loaders comparten un cliente HTTP y reutilizan conexiones. Las vistas leen la lista de instancias sin copiarla entera.
- La sincronización ya no se bloquea con «Falta una dependencia compatible del anfitrión» cuando un JAR propio exige una dependencia que Modrinth declara exclusiva de servidor, como WorldgenFeatureFix para Epic Terrain: el cliente la omite con un aviso, igual que el anfitrión. Compatible con servicios de sincronización anteriores.
- `icaro-sync-host` añade `--incluir-servidor` (todos) y `--incluir-archivo <jar>` (repetible) para publicar también los mods exclusivos de servidor y reproducir la carpeta `mods` completa. Sin estas opciones se siguen excluyendo.

## [0.3.1] - 2026-10-23

- Reduce el tiempo de aplicación de parches al instalar Forge, conservando las verificaciones y los archivos del cliente.
- Muestra el avance y el tiempo transcurrido durante la instalación de Forge y NeoForge.
- Corrige la detección de Minecraft en Discord al jugar desde el launcher en Windows.

## [0.3.0] - 2026-10-22

- Quilt se instala y se ejecuta con su perfil oficial, igual que Fabric: crear instancia, Jugar, Cambiar versión y Cambiar versión del loader. El perfil y las bibliotecas se descargan solo de `meta.quiltmc.org`, `maven.quiltmc.org` y `maven.fabricmc.net`, con SHA-1 y tamaño comprobados, y se guardan en `meta/quilt/<minecraft>/<loader>`. Admite las versiones ofuscadas (con los mapeos `hashed` e `intermediary`) y las que Quilt publica sin ofuscar, como 26.3. El selector de versiones del loader ordena las estables antes que las betas, así que lo primero que se ofrece es una estable.
- Publicación por canales. `develop` publica cada cambio como beta (`v<versión>-beta.<compilación>`, prerelease) y `main` publica la estable (`v<versión>-build.<compilación>`) y crea su tag fuente `v<versión>`. Las ramas `vX.Y.x` y los tags ya no publican. `main` exige una versión de Cargo mayor que la última estable, y `develop` una versión mayor que esa estable para sus betas. Los cambios que solo tocan documentación no compilan.
- Ajustes → Acerca de añade el canal de actualizaciones Estable/Beta, guardado en la configuración. El canal Estable nunca ofrece una beta; el Beta ofrece la release más nueva entre betas y estables. Una compilación beta se identifica en Acerca de y en la pantalla de actualización. Los launchers anteriores ignoran las prereleases.
- Capturas ya no congela la interfaz al abrirse. La cuadrícula usa miniaturas de 480 × 270 generadas en segundo plano (hasta cuatro a la vez, de la más reciente a la más antigua), que aparecen a medida que están listas; antes cada PNG se decodificaba y se subía a la GPU a resolución completa en el primer cuadro. El visor sigue abriendo el original sin recodificar, y un PNG dañado deja su hueco vacío sin afectar al resto.

- Añade instalación y preparación de Forge y NeoForge con sus instaladores oficiales, respetando la versión elegida y la configuración de Java.
- Los MRPACK y ZIP reconocen Forge y NeoForge y conservan la versión del loader indicada por el paquete.
- Instala Fabric API compatible por defecto al preparar una nueva instancia Fabric. Conserva copias locales o desactivadas y el contenido de modpacks; no vuelve a añadirlo si lo retiras después.
- Corrige la apertura del inicio de sesión en Windows para utilizar cualquier navegador configurado como predeterminado del sistema.
- Prepara Forge y NeoForge en una carpeta aparte y reutiliza sus archivos verificados, conservando la instalación anterior si el nuevo intento falla.

Consulta las [notas de 0.3.0](docs/releases/0.3.0.md) y el [alcance y validación](docs/loaders-forge-neoforge.md). Quedan pendientes la validación manual del inicio de sesión con distintos navegadores, las partidas reales con mods, las migraciones y el recorrido nativo en Linux. No se admiten todos los formatos históricos de Forge.

## [0.2.5] - 2026-10-16

- Sustituye Invitado por cuentas sin conexión con nombre propio, UUID estable, guardado, selección y eliminación desde el componente CuentaSinConexion entregado. Conserva sus iconos, cabeza rayada, temas y confirmación en fila; eliminar un perfil no toca mundos ni instancias.
- Restaura la cuenta local al abrir y mantiene sus datos separados de las credenciales Microsoft. Permite lanzar Vanilla/Fabric sin una sesión Microsoft; los servicios oficiales y los servidores con autenticación siguen requiriendo una cuenta Microsoft.
- Añade el componente SkinSinConexion: PNG local, frente/espalda, modelo clásico/delgado, capa exterior y cabezas personalizadas. Guarda la skin por cuenta y prepara texturas firmadas con un agente verificado para el siguiente inicio del juego. Conserva la vista anterior ante errores y permite quitar la skin.
- Quita del formulario la explicación técnica sobre el cálculo del UUID.

Consulta [cuentas sin conexión](docs/cuentas-sin-conexion.md) para el alcance y la validación.

## [0.2.4] - 2026-10-16

- La revisión de sincronización guarda un inventario de `mods` (`.icaro-mods-inventario.json`, junto al bloqueo de la carpeta de juego) y solo relee los archivos nuevos, modificados o con más de 7 días desde su última lectura completa. También reutiliza la identidad Fabric de cada JAR mientras su hash no cambie. En un banco de 500 MiB, Jugar sin cambios pasa de 2,4 s a unos 45 ms; la primera revisión sigue leyendo todo.
- La sincronización y el instalador de mods piden los nombres de proyecto y los mods exclusivos de cliente por lotes de 50, en lugar de una ficha por mod. La caché de Modrinth ya no queda bloqueada mientras espera la red: una consulta lenta no retiene los aciertos de caché ni otros recursos, y dos peticiones simultáneas del mismo recurso siguen produciendo una sola.
- Las peticiones a la API de Modrinth siguen un presupuesto preventivo (4 por segundo, ráfagas de 8) y esperan al reinicio de la ventana cuando las cabeceras indican que quedan menos de 10, en lugar de frenar solo tras recibir un 429. La espera creciente ante un 429 añade hasta un 10 % aleatorio.
- Si el anfitrión y la carpeta `mods` no cambiaron desde la última sincronización correcta, Jugar confirma el plan sin consultar Modrinth. El registro `minecraft/.icaro-sync-verificado.json` caduca a las 24 horas.
- Las descargas de la sincronización van directamente a disco con el SHA-512 calculado por bloques, sin cargar cada JAR en memoria, y se hacen en paralelo: hasta cuatro desde Modrinth y dos desde el anfitrión.
- `icaro-sync-host` solo vuelve a calcular el hash de los JAR que cambiaron en lugar de toda la carpeta cada 10 segundos, envía cada JAR por bloques sin cargarlo en memoria, atiende las descargas en cola en vez de cortar la novena conexión y mantiene abiertas las conexiones correctas. El índice usa `ETag`/`304` y las descargas cortadas del anfitrión se reanudan con `Range`/`If-Range` validados. Es compatible con launchers y servicios anteriores.

Validación: pruebas del núcleo, de la interfaz y del servicio con servidores locales y fuentes simuladas. La descarga real desde el CDN de Modrinth y el servicio actualizado en el Arch del anfitrión siguen pendientes. Consulta las [notas de 0.2.4](docs/releases/0.2.4.md).

## [0.2.3] - 2026-10-16

- Acerca de muestra la versión compilada del workspace en lugar del texto fijo `0.1.0`.
- Acerca de añade **Código fuente en GitHub**, con icono de enlace externo, que abre el repositorio declarado en `Cargo.toml`.
- Amplía la guía de colaboradores con el estado del hotfix, los pendientes y las prioridades de la investigación de sincronización.
- La sincronización calcula los hashes de `mods` una vez por revisión en lugar de dos, y al aplicar solo vuelve a leerlos si cambiaron los metadatos de algún archivo. Pulsar Jugar con el conjunto al día pasa de tres lecturas completas a una.
- La lectura local de mods se ejecuta fuera del executor asíncrono y se detiene al cancelar. El límite de 90 segundos de la revisión ya no incluye el disco, por lo que una carpeta grande no se presenta como un fallo de Modrinth.
- Las compilaciones de desarrollo optimizan `sha2` y `sha1`; las mediciones de sincronización en debug dejan de estar dominadas por el hashing sin optimizar.
- Aplicar una sincronización ya no copia toda la carpeta `mods`: descarga y verifica solo lo nuevo, y mueve únicamente los archivos afectados mediante un diario que se deshace si falla o si el launcher se cierra a mitad. Los mods personales no se copian ni se recrean.
- La barra de progreso de la sincronización cuenta los cambios del plan en lugar de todos los mods del índice.
- Añade `logs/sincronizacion.log`, con rotación, para diagnosticar sincronizaciones: índice del anfitrión, revisión, plan, descargas, commit, rollback, recuperación y errores con su fase y duración. No incluye rutas absolutas ni datos de la cuenta.

Validación: pruebas del núcleo y de la interfaz y un banco sintético de 500 MiB; el usuario confirmó la mejora en Windows. La validación en HDD y Linux sigue pendiente. Consulta las [notas de 0.2.3](docs/releases/0.2.3.md).

## [0.2.2] - 2026-10-15

- Identifica versiones anteriores y copias duplicadas de un mod por su ID interno y sus hashes. La sincronización respalda y retira las copias activas del mismo mod del servidor, conserva otros mods personales y verifica que no queden dos versiones activas de un mod requerido.

- Reutiliza el paquete de actualización ya descargado tras reabrir el launcher o reintentar una instalación fallida; vuelve a verificar tamaño y SHA-256 y descarta archivos parciales o alterados.

- Mantiene bloqueada la actualización hasta cerrar el launcher, evita ayudantes simultáneos y normaliza las rutas para PowerShell/Inno Setup. Comprueba la versión instalada y muestra el motivo de un fallo.
- Distribuye el espacio del resumen de importación para que una lista larga de carpetas no aplaste las etiquetas.
- Muestra el plan de sincronización aunque un JAR propio choque con el del anfitrión; exige autorizar su reemplazo y conserva una copia verificable fuera de `mods`.
- Añade **Importar .jar** en la lista de mods: selección múltiple, publicación atómica y conservación de nombres ya ocupados.

Incluye una [investigación de rendimiento](docs/investigaciones/rendimiento-sincronizacion-2026-10-15.md) con mediciones y un banco de pruebas reproducible. Las optimizaciones propuestas siguen pendientes. Consulta las [notas de 0.2.2](docs/releases/0.2.2.md) para la validación y sus límites. Esta preparación local todavía requiere publicar los artefactos para distribuir la actualización.

## [0.2.1] - 2026-10-15

- Añade selección de MRPACK/ZIP por ruta y con Examinar, y adapta ImportarModpack al diseño entregado: grabado, iconos, fases de lectura, resumen, nombre/grupo y recuperación de errores.
- Conecta el componente ZonaSoltar del diseño a la importación local: clic, arrastre, revisión y protección frente a archivos adicionales durante una operación.

- Corrige la rueda del selector de versiones dentro del asistente de instancias.
- El ejecutable del launcher para Windows abre la interfaz sin una consola de fondo.
- Añade importación local de MRPACK y ZIP completos Vanilla/Fabric en instancias nuevas, con revisión, extracción por bloques y recuperación desde Descargas.
- Comparte caché y pausa de Modrinth entre tienda, cesta e instalación; agrupa comprobaciones de actualización por hash y fichas por proyecto para evitar repetir peticiones al añadir otro mod.
- Adapta la publicación a la rama de mantenimiento `v0.2.x`, conserva el tag `v0.2.1` para el código y evita volver a compilar/publicar el mismo contenido al integrarlo en otras ramas.

Validación: pruebas del núcleo y de la interfaz, importación ZIP64 sintética de más de 2 GiB, conservación de archivos y capturas en Tinta/Piedra. El usuario confirmó la importación de su instancia real. Los ZIP que solo contienen referencias de CurseForge y los instaladores de Quilt/Forge/NeoForge siguen pendientes; consulta las [notas completas](docs/releases/0.2.1.md).

## [0.2.0] - 2026-10-14

Publicación consolidada de las mejoras de servidores y las correcciones probadas durante su preparación. Su tag es `v0.2.0`; la rama de mantenimiento es `v0.2.x`.

### Añadido

- Servicio independiente para sincronizar los mods de un servidor dedicado, con compilación para Linux y uso sin instalar el launcher en el anfitrión.
- Distribución de JAR privados o ausentes en Modrinth desde el anfitrión, verificando tamaño y SHA-512.
- Preferencias de Minecraft compartidas entre instancias, con filtrado de teclas de mods ausentes.
- Caché persistente de metadatos de Modrinth y consultas de versiones en lotes de hasta 50 IDs.

### Corregido

- Rutas extendidas de Windows en los argumentos de Java y el classpath, que impedían encontrar `KnotClient` de Fabric.
- Recuperación de dependencias obligatorias omitidas por índices antiguos, incluidas bibliotecas como CodxLib y Lithostitched cuando sus JAR admiten cliente.
- Dependencias de cliente no declaradas de Litemoretica 1.2.5 para Minecraft 1.21.1: la sincronización añade Litematica y resuelve MaLiLib automáticamente.
- Conservación de mods personales del cliente, tanto del catálogo como de JAR manuales y desactivados, durante la sincronización.
- Desactivación persistente de la sincronización al iniciar después de completarla correctamente; los fallos conservan la preferencia para reintentar.
- Selección manual de Java más reciente que el mínimo del perfil, como Java 25 para Minecraft que recomienda Java 21.
- Navegación bloqueada durante la consulta de mods y lentitud al volver a las instancias por la revisión repetida de miles de archivos.
- Lectura de descriptores Fabric incompletos y publicación de mods universales con metadatos incompletos, conservando los JAR originales con avisos.
- Resolución de dependencias y pins del catálogo sin sustituir silenciosamente el conjunto instalado del servidor.
- Detección de redes conectadas desde ZeroTier oficial y actualización de instancias vinculadas a servidores.

### Rendimiento y recuperación

- Descarga solo los archivos ausentes o cambiados; repetir una sincronización al día evita volver a copiar el conjunto completo.
- Reutiliza las consultas vigentes entre aperturas y conserva respuestas parciales correctas ante fallos posteriores.
- Respeta los límites de Modrinth y sus tiempos de espera ante HTTP 429.
- Permite salir de la consulta o navegar mientras termina la aplicación del plan, sin abrir Minecraft desde otra pantalla.

### Validación

- Pruebas de sincronización, dependencias, conservación de archivos, caché, Java y navegación.
- Integración con Litemoretica, Litematica y MaLiLib reales en una instancia temporal vacía; una segunda sincronización no propone cambios.
- El usuario confirmó el funcionamiento del launcher y el inicio del juego con la corrección de dependencias.
- La publicación de instaladores y los artefactos Linux se comprueban por separado en GitHub Actions. Los loaders adicionales y el resto del roadmap siguen pendientes.

## [0.1.0]

Primera entrega pública para Windows: instancias Vanilla y Fabric, catálogo de contenido, cuentas, consola, capturas y actualización del launcher. Consulta las [notas completas](docs/releases/0.1.0.md).

[0.2.2]: docs/releases/0.2.2.md
[0.2.1]: https://github.com/00faithlss/Icaro-launcher/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/00faithlss/Icaro-launcher/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/00faithlss/Icaro-launcher/tree/v0.1.0
