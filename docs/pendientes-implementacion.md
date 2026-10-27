# Implementación pendiente

Actualizado el **22-10-2026**. Este documento describe el trabajo que falta; no presenta estas funciones como disponibles. El [roadmap del System Design](roadmap-system-design.md) detalla los componentes y sus diferencias visuales.

## 1. Instalación y ejecución de loaders oficiales

**Base de almacenamiento:** Vanilla y Fabric comparten assets, bibliotecas y metadatos. Forge y NeoForge preparan sus clientes y bibliotecas compartidas en meta/loaders, con el instalador oficial y una carpeta aislada por intento. Quilt utiliza su perfil oficial; ver [organización de datos](organizacion-datos.md).

**Estado actual:** el asistente y Cambiar versión consultan los catálogos oficiales y guardan la versión elegida. Vanilla y Fabric 26.3 tienen confirmación del usuario en Windows. Forge y NeoForge ya conectan instalación, Java y argumentos oficiales con la cola y el lanzamiento; la primera instalación Fabric añade Fabric API compatible respetando mods existentes y modpacks. Se comprobó el procesamiento oficial de Forge 1.20.1, NeoForge 1.21.1 y NeoForge 26.3, la reutilización sin red y el rechazo de una biblioteca alterada. La partida real y las migraciones de estos dos loaders siguen pendientes. Consulta [formatos admitidos y límites](loaders-forge-neoforge.md).

- [x] Conectar Fabric al perfil oficial, versión fijada, bibliotecas verificadas, argumentos y mínimo de Java.
- [x] Conectar Forge y NeoForge mediante sus instaladores oficiales spec 1, versión fijada, preparación aislada, Java, bibliotecas verificadas y argumentos del perfil.
- [ ] Validar Quilt en una partida real y ampliar los formatos históricos de Forge todavía no admitidos.
- [ ] Respetar la versión concreta guardada en la instancia y su compatibilidad con Minecraft; resolver Java y dependencias del perfil resultante.
- [ ] Preparar la instalación fuera de la carpeta activa, verificar las descargas y publicar el resultado solo al terminar. Conservar la instalación anterior ante error o cancelación.
- [ ] Integrar el avance con la cola, pausa/cancelación, consola y estados estables del botón; comprobar archivos existentes sin mostrar una instalación ficticia.
- [ ] Lanzar el perfil instalado con sus argumentos, bibliotecas y nativos reales y con los mods de esa instancia.
- [ ] Conectar el mismo recorrido a Crear instancia, Instalar/Jugar y Cambiar versión. Probar cada loader con una instancia nueva, una instalación reutilizada y una migración.

Las tareas pendientes de este apartado corresponden a Quilt y a la validación completa de Forge/NeoForge: partida con mods, migración, cancelación/recuperación durante los procesadores y Linux. Los puntos siguientes conservan su estado pendiente hasta validar ese recorrido; una preparación del cliente no lo acredita.

**Criterio de finalización:** instalar y abrir una instancia de cada loader con un mod compatible, reutilizar sus archivos al reiniciar y recuperarse de una descarga fallida sin perder mundos ni mods.

## 2. Importación e instalación de modpacks

**Estado actual:** la tienda instala los MRPACK de Modrinth en nuevas instancias Vanilla/Fabric/Forge/NeoForge. La revisión muestra Minecraft, la versión exacta del loader y los archivos opcionales; la instalación verifica SHA-512/SHA-1 y aplica overrides/client-overrides antes de preparar el juego. Pasó la instalación completa de Fabulously Optimized 14.1.1 con Minecraft 26.2/Fabric 0.19.5 y reutilización sin descargas. Instancias también permite seleccionar MRPACK locales y ZIP completos, incluidos perfiles Vanilla/Fabric/Forge/NeoForge de Prism/MultiMC y contenido exportado por Ícaro. La lectura/extracción se hace por bloques; consulta [importación local](importar-modpacks.md). Quilt y los formatos históricos no admitidos de Forge siguen pendientes. El usuario confirmó la importación de su instancia; las comprobaciones de compatibilidad y juego siguen siendo específicas de cada paquete.

- [x] Leer MRPACK desde el catálogo de Modrinth: metadatos, Minecraft, loader, referencias de archivos, hashes y archivos incluidos; presentar una revisión antes de importar.
- [x] Descargar y verificar los archivos referenciados de MRPACK de Modrinth, aplicar los archivos incluidos y preparar Vanilla o Fabric mediante sus perfiles oficiales.
- [ ] Extender la instalación de modpacks a Quilt; validar partidas y migraciones de Forge/NeoForge.
- [x] Importar contenido de ZIP de Ícaro y perfiles Vanilla/Fabric/Forge/NeoForge compatibles de Prism/MultiMC; detectar el formato y explicar los formatos no admitidos.
- [x] Conectar la selección de archivo a la importación real de MRPACK y ZIP completos.
- [x] Conectar ZonaSoltar a la importación real, con clic, arrastre y estados ocupado/cancelado.
- [x] Validar rutas, enlaces, colisiones, integridad y límites de extracción en la importación local.
- [ ] Integrar ArchivosNoVerificados para los archivos sin origen comprobado; conservar las decisiones de confianza asociadas al contenido exacto.
- [ ] Completar el descubrimiento/importación desde otros launchers y la duplicación de una instancia sin alterar el original.
- [ ] Probar la importación manual de los ZIP exportados en Prism/MultiMC; actualmente se verifica su estructura con pruebas.
- [ ] Implementar la exportación MRPACK con versiones concretas y referencias verificadas, separada de la exportación ZIP y de la lista de mods.

**Criterio de finalización:** importar un pack en una instancia nueva, instalarlo y abrirlo; un fallo debe dejar el origen intacto y permitir cancelar o reintentar.

## 3. Editor, descargas y recuperación

- [ ] Completar las opciones pendientes de General y Java y su persistencia.
- [x] Conectar selección global/por instancia, herencia, modos automáticos como Prism, detección de runtimes oficiales, listas compatibles por perfil y prueba con memoria/argumentos. Ver la [política de Java](organizacion-datos.md#java-según-el-perfil-y-modo-de-gestión); falta validar el recorrido nativo en Linux y los requisitos de loaders.
- [ ] Gestionar los mundos con acciones reales y conectar sus respaldos y recuperación a los componentes del diseño.
- [ ] Ordenar la prioridad de resourcepacks por arrastre y ampliar el soporte de configuración de shaders a otros motores.
- [ ] Completar la recuperación de archivos y capturas desde sus papeleras y la revisión de integridad.
- [ ] Guardar el historial de descargas entre sesiones y unificar los instaladores en la cola global.
- [x] Acumular y guardar el tiempo jugado por instancia (`estadisticas.json`, desde que se inicia el juego hasta que se cierra) y mostrar el tamaño de la instancia en el editor. Falta comprobarlo en una partida real y mostrarlo también en la lista de instancias.

## 4. Cuenta, servidores y publicación

- [x] Sustituir Invitado por cuentas sin conexión persistentes, con selección, eliminación y conservación de mundos/instancias.
- [x] Conectar SkinSinConexion: PNG local, vista previa, modelo y avatares; servicio firmado y agente verificado al lanzar.
- [ ] Validar la skin local en una partida Vanilla/Fabric/Forge/NeoForge real y comprobar su comportamiento en LAN y servidores compatibles.
- [ ] Completar la gestión persistente de varias cuentas Microsoft y sus capes.
- [x] Conectar PantallaCuenta y EstudioSkin originales con perfil/PNG reales, modelo, capas, historial y subida; validación manual de la subida pendiente.
- [ ] Completar el recorrido de servidores, ZeroTier y sincronización descrito abajo. Lista, estado/SRV, conexión con una instancia, solicitud de pertenencia a redes y sincronización de Modrinth están conectados; quedan permisos/instalación de ZeroTier, salida, miembros y pruebas entre equipos.
- [ ] Resolver la integración opcional de CurseForge cuando exista acceso válido a su API y condiciones de distribución.
- [ ] Completar el traslado de la raíz de datos y la bandeja, y revisar los comportamientos de cierre; tamaños, caché, rotación y recuperación de operaciones ya están conectados.
- [ ] Publicar el ejecutable y los metadatos de actualización y probar una actualización completa. La comprobación y descarga del actualizador ya están conectadas; ver el [contrato de publicación](actualizaciones-launcher.md).

### Alcance de Servidores, ZeroTier y sincronización

El [roadmap general](../README.md#servidores-zerotier-y-sincronización) cuenta ocho tareas separadas. Las pantallas originales de lista, PanelLateral, RedPrivada y SincronizarMods usan datos reales; los ejemplos quedan en la maqueta aislada. Ver [servidores](servidores.md) y [servicio del anfitrión](sincronizacion-servidores.md).

**Servidores:** se guardan dirección, puerto, instancia y orden por arrastre, se consulta estado con IP/DNS/SRV y se prepara el lanzamiento por perfil. Falta validar la conexión dentro del juego con equipos reales. Los servidores directos funcionan sin ZeroTier; la consulta del servicio de sincronización usa la IP privada elegida.

**ZeroTier:** detección y solicitud de pertenencia por ID conectadas a la CLI oficial, con autorización, IP y aviso al aceptar. La CLI necesita sus permisos habituales; faltan instalación/elevación automática y salida desde la interfaz. Crear una red abre Central. Miembros y su administración necesitan una fuente del controlador; los peers locales no se presentan como amigos.

**Sincronización:** servicio explícito en el PC del anfitrión que comparte el índice de una instancia cliente, con versiones exactas de Modrinth. Consulta previa a Jugar, revisión de diferencias, hashes/tamaño, conservación de archivos locales y preparación con respaldo/cancelación conectadas; prueba con mods reales por HTTP local. Desde la P0 de rendimiento, la revisión calcula los hashes una vez y fuera del executor, la aplicación mueve solo los archivos afectados con un diario que se deshace tras un fallo o cierre, y `logs/sincronizacion.log` registra cada fase. En un banco de 500 MiB en debug, revisión, aplicación y Jugar sin cambios pasaron de 92 s, 93 s y 137 s a 2,3 s, 0,03 s y 2,4 s; el usuario confirmó la mejora con una instancia por actualizar y otra nueva. El inventario incremental (P1) reutiliza hashes e identidades de archivos sin cambios: en el mismo banco, Jugar sin cambios baja a unos 45 ms tras la primera revisión. También están hechos los nombres y exclusivos de cliente por lotes, la caché de Modrinth sin bloqueo durante la red, el presupuesto preventivo de la API, la confirmación sin red de una sincronización ya verificada (vigencia de 24 h) y las descargas en streaming con concurrencia acotada. La P2 del host también está hecha: inventario incremental en `icaro-sync-host`, ETag/304, conexiones persistentes, cola de descargas, envío por bloques y reanudación con rangos validados. Queda la caché de contenido compartida entre instancias (prevista para 0.3) y validar el servicio actualizado en el Arch del anfitrión ([investigación](investigaciones/rendimiento-sincronizacion-2026-10-15.md)). Faltan integración visual del publicador con diseño aprobado, proveedores adicionales y prueba entre equipos ZeroTier.

**Criterio de finalización:** desde equipos reales, guardar un servidor, conectar directamente o mediante una red ZeroTier autorizada, revisar y sincronizar mods, jugar y reconectar. Comprobar en Windows y Arch Linux los errores de servicio, autorización pendiente, manifiesto incompatible y descarga interrumpida sin perder los archivos anteriores.

## 5. Validación y diseño

- [ ] Repetir en Arch Linux el recorrido de Vanilla confirmado en Windows y validar los nuevos loaders en ambos sistemas.
- [ ] Revisar foco, teclado, ventanas pequeñas, escala, ambos temas, metales, rendimiento y virtualización.
- [ ] Completar las transiciones y efectos pendientes del System Design, respetando Reducir movimiento.
- [ ] Validar el empaquetado y los recorridos completos con archivos reales antes de la primera publicación.

## 6. Instalador de Windows y distribución

- [x] Crear un instalador por usuario, con identidad e icono de Ícaro, accesos directos y registro de desinstalación.
- [x] Separar la carpeta del programa de los datos de usuario; desinstalar retira archivos registrados y conserva los datos externos al programa y archivos ajenos.
- [x] Automatizar la compilación del instalador y del portable con versiones consistentes, requisitos revisados y hashes reales.
- [ ] Validar instalación limpia, actualización, reinstalación y desinstalación en un equipo de prueba.

Las tres casillas del roadmap general agrupan creación, empaquetado y validación. El workflow prepara assets portables, setup y latest.json; la actualización instalada usa el instalador completo, pendiente de validar desde una release remota. No se habilitará actualización instalada como verificada antes de probarla. Ver [instalador de Windows](instalador-windows.md).

## Correcciones recientes

Ajustes → Acerca de muestra la versión compilada del workspace y un enlace al repositorio de GitHub. La sincronización ya no copia la carpeta `mods` completa y deja un registro rotativo propio para diagnosticar fallos.

La creación/preparación de instancias mantiene estable el estado del botón frente a eventos intercalados de descarga y verificación. La trama de Preparando es estática y no cambia cada fotograma. Ya se incorporaron la exportación de instancia, los seis formatos de lista de mods, el editor de iconos con vista previa inmediata, los avisos, apariencia, verificación de archivos y la confirmación de eliminación con bloques y grietas en toda la tarjeta; sus límites siguen registrados en el roadmap del diseño.

La composición original de Cuenta/Estudio, PNG real, modelo/capas, subida e historial local están conectados. Pendientes: prueba manual con una cuenta real, capes propias, varias cuentas Microsoft y cambio de nombre dentro de Ícaro. Ver [cuenta](cuenta.md).
