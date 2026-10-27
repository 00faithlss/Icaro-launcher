# Investigación: redes virtuales, alojamiento y sincronización

**Fecha:** 2026-10-27
**Estado:** exploración. Nada del diseño futuro, sus costos, su viabilidad en las redes de los jugadores ni su paridad con Essential está confirmado o aprobado. Las ideas y fases de esta nota son hipótesis para evaluar, no compromisos de implementación.
**Alcance:** análisis de arquitectura y producto. No se cambió código, no se ejecutaron pruebas y no se validó el flujo con dos jugadores reales.

## Estado de confirmación

Los objetivos expresados por el usuario son que dos personas con Ícaro puedan compartir un mundo, que el anfitrión pueda abrir su mundo LAN, que la sincronización de mods sea más sencilla y que Ícaro no dependa de ZeroTier ni de infraestructura pagada por el usuario. Esto describe el problema deseado; **no confirma una arquitectura ni autoriza todavía una implementación concreta**.

Continúan sin confirmar:

- Si el primer prototipo usará una IP virtual del sistema, un túnel local por aplicación u otra forma de conexión.
- Si se permitirá depender de un servicio gratuito de coordinación, de sus límites y disponibilidad, o si el intercambio de invitaciones debe ser completamente local/manual.
- Si se acepta que algunas redes con NAT restrictivo o CGNAT no conecten sin relé.
- La necesidad y el alcance de un mod auxiliar para detectar el puerto LAN en cada loader.
- Los costos operativos: las cifras conversadas son escenarios ilustrativos sin cotización vigente ni mediciones de tráfico.
- Cómo implementa Essential su transporte interno. No se verificó su código ni se confirmó que Ícaro pueda reproducir su comportamiento.

ZeroTier, Tailscale, la perforación NAT, UPnP, QUIC, los servicios gratuitos y el puente descrito más adelante son alternativas de investigación. No son decisiones tomadas.

## Resumen ejecutivo

Ícaro ya tiene una base útil: guarda servidores, detecta ZeroTier, puede pedir unirse a una red, lanza Minecraft hacia un destino y sincroniza mods desde un servicio que corre en el PC anfitrión. La mejora grande no consiste en inventar otra VPN. Conviene separar cuatro responsabilidades que ahora aparecen unidas en la experiencia:

1. **Transporte privado:** ZeroTier, Tailscale u otra red ya instalada, LAN o conexión directa.
2. **Alojamiento:** el proceso Minecraft y el servicio de mods, que son dos servicios distintos.
3. **Identidad y autorización:** quién es el anfitrión, quién puede consultar el índice y quién puede recibir archivos.
4. **Compatibilidad del modpack:** versión de Minecraft y loader, mods exactos, dependencias, archivos y plan revisable.

Hipótesis de trabajo: estudiar un puente de aplicación entre launchers con conexión directa como camino principal, sin requerir una VPN ni operar un relé de juego. ZeroTier y otros proveedores podrían quedar como alternativas voluntarias. Mantener el servicio de mods junto al anfitrión es otra opción para evaluar. Antes de automatizar cualquier red, habría que resolver la confianza entre cliente y anfitrión y dar diagnósticos claros. Esta dirección no está aprobada.

## Estado observado en el repositorio

La rama revisada fue `v0.3.x`, en el commit `037e523`. Hay otro worktree, `/workspace/Icaro-launcher-actualizado`, con cambios locales de optimización y documentación; se dejó intacto.

El comportamiento implementado en esta rama es:

- `crates/core/src/zerotier.rs` detecta ZeroTier mediante API local y CLI. En Windows consulta el API local en loopback con el token de ZeroTier; también puede solicitar entrar o salir de una red. La interfaz muestra estado e IP asignada.
- La pertenencia a una red no autoriza por sí sola a un miembro en ZeroTier Central. Una solicitud de unión puede quedar esperando aprobación del administrador.
- `docs/servidores.md` describe conexión directa, DNS/SRV y una lista local de servidores. Los servidores normales no requieren ZeroTier.
- `crates/core/examples/sync_host.rs` inicia `icaro-sync-host` como proceso independiente. Lee una instancia o una carpeta de servidor dedicado y publica el índice en la IP indicada.
- El puerto de sincronización se deriva actualmente como el puerto de Minecraft más uno. El anfitrión debe arrancar el proceso, escoger la IP de ZeroTier y permitir el puerto en el firewall. Conectar la VPN no arranca este servicio.
- El índice `GET /icaro/v1/mods` y los JAR propios se sirven mediante HTTP sobre la red privada. Hay `ETag`/`304`, descarga por bloques, rangos para reanudar y SHA-512. El cliente obtiene los mods de catálogo desde Modrinth cuando corresponde; el servicio anfitrión entrega los JAR que no se pueden obtener de allí.
- Los archivos se comparan con el plan y se conservan respaldos según las reglas de sincronización. Las diferencias requieren una revisión del usuario; el modo de jugar ignorando un fallo también tiene confirmación.
- La propia guía reconoce que aún faltan instalación y permisos asistidos, salir de la red desde la interfaz, administración de miembros y una validación completa entre equipos Windows y Arch.

Fuentes de implementación: [ZeroTier](../../crates/core/src/zerotier.rs), [servicio de anfitrión](../../crates/core/examples/sync_host.rs), [servidores](../servidores.md) y [sincronización](../sincronizacion-servidores.md).

## Objetivo de experiencia: invitar a un mundo LAN

El resultado deseado descrito por el usuario es que dos jugadores con Ícaro puedan compartir el mundo que el anfitrión abre en LAN, sincronizar los mods y entrar desde una invitación, sin que ZeroTier sea obligatorio ni que el usuario tenga que contratar infraestructura. Es un objetivo de producto, todavía no un diseño aprobado.

### Flujo propuesto, aún por validar

1. El anfitrión abre una instancia compatible y publica su mundo en LAN.
2. Un componente auxiliar de Ícaro detecta el puerto local del servidor integrado y se lo comunica al puente local.
3. El anfitrión crea una invitación temporal. Un servicio de encuentro intercambia candidatos de red y credenciales efímeras; no lleva el tráfico del juego.
4. El amigo acepta la invitación. Ícaro compara Minecraft, loader y el manifiesto de mods, enseña diferencias y aplica la sincronización aprobada.
5. El launcher del invitado inicia Minecraft hacia un puerto de loopback. El puente del invitado lleva ese flujo de juego directamente al puente del anfitrión, que lo entrega al puerto LAN local.
6. Cuando el anfitrión cierra el mundo, Ícaro cierra la sesión, revoca la invitación y detiene los listeners temporales.

La interfaz podría mostrar una dirección virtual propia de la sesión, pero eso no sería una IP enrutable del sistema operativo. Una IP virtual real requiere una interfaz TUN/TAP o un controlador equivalente, rutas, reglas de firewall y probablemente permisos elevados. La opción menos invasiva por investigar es un proxy local de aplicación: el juego del invitado se conecta a `127.0.0.1:<puerto-local>` y el puente lleva el tráfico al anfitrión.

Minecraft Java transporta el juego por TCP. Si el anfitrión no tiene una ruta pública directa, el puente tendría que establecer su propio canal punto a punto (por ejemplo, encapsular el flujo TCP en un transporte cifrado sobre UDP/QUIC) usando señalización y técnicas de atravesamiento NAT. La elección de protocolo y bibliotecas no está hecha. UPnP/NAT-PMP, IPv6 global, configuración manual del router o un overlay existente son candidatos o alternativas, cada uno con requisitos y riesgos distintos.

El mundo integrado abre un puerto local al publicar en LAN; que Ícaro pueda descubrirlo de forma fiable sin un componente dentro de Minecraft debe probarse. Un mod auxiliar por loader podría anunciar puerto, versión, identidad y manifiesto. El alcance de loaders y versiones de Minecraft tampoco está decidido.

### Costos, dependencia y alcance de una primera versión

El objetivo económico planteado es no pagar tráfico de partida ni depender de un servidor relay operado por Ícaro. Un servicio de encuentro que solo intercambia invitaciones/candidatos movería pocos datos comparado con un relé, pero seguiría siendo una dependencia. Un free tier no garantiza disponibilidad, permanencia ni límites suficientes; alguien tendría que operarlo. El tráfico de juego y JAR propios sería directo entre anfitrión e invitado cuando la ruta directa funciona. Los mods públicos pueden seguir descargándose de Modrinth.

La conversación mencionó como orientación hipotética **US$20–100/mes** para coordinación más un relé pequeño, y **US$100–500 o más/mes** para mayor tráfico y varias regiones. No es una cotización, no fue verificada contra proveedores ni respaldada por mediciones. El usuario indicó que no dispone de presupuesto para esos gastos; por tanto, esas cifras no deben convertirse en requisito o plan de operación.

Sin tráfico por un relé propio se puede intentar conexión directa, pero no garantizarla en redes restrictivas o CGNAT. Si el camino directo falla, una primera versión sin costo de Ícaro tendría que explicar alternativas —por ejemplo, probar IPv6/UPnP con autorización o usar una red externa opcional— y reconocer cuándo no puede conectar. La política final ante ese fallo está pendiente de confirmación.

Essential sirve como referencia de experiencia (invitar y compartir un mundo desde el juego). No se comprobó su arquitectura interna, costos ni uso de relés; no se afirma que Ícaro pueda copiar su solución técnica.

## Qué ofrecen las redes virtuales

### ZeroTier

ZeroTier crea una red virtual administrada desde ZeroTier Central y asigna direcciones privadas a los miembros. Su cliente crea una interfaz virtual; las aplicaciones usan direcciones IP ordinarias. El plano de control distribuye configuración, pertenencia y reglas. Los nodos intentan establecer caminos directos entre pares usando UDP y técnicas de atravesamiento de NAT. Cuando las redes, firewalls o NAT no permiten el camino directo, el rendimiento y la disponibilidad dependen de los mecanismos de relé disponibles para esa combinación.

**Ventajas para Ícaro:** ya está parcialmente integrado; una red entre amigos permite conectar Minecraft y el servicio de mods sin publicar puertos en el router; las direcciones pueden restringirse al overlay; el usuario puede seguir usando la aplicación ZeroTier oficial.

**Límites:** cada amigo debe instalar el cliente, unirse a la red y, en una red privada, ser autorizado por el administrador. El ID de red no es una invitación que otorgue acceso. IP virtual no significa Internet más rápido ni que todos los peers puedan hablar con todos: mandan las rutas, reglas, firewall y el estado real del túnel. La API local es una API administrativa del cliente y su token es sensible; Ícaro debe seguir evitando elevarse o copiar ese secreto a logs.

ZeroTier no descubre automáticamente el proceso Minecraft ni `icaro-sync-host`; esos procesos deben arrancarse y escuchar en interfaces/puertos correctos. Tampoco demuestra que un JAR sea confiable solo por llegar por una red autorizada.

### Hamachi

LogMeIn Hamachi es una VPN comercial administrada: sus clientes crean redes virtuales con participantes y direcciones privadas. Sus modos tradicionales incluyen redes tipo malla, concentrador y gateway. Simplifica casos de juego entre amigos que aceptan instalar el cliente y depender del servicio del proveedor.

**Ventajas:** conocido por usuarios de juegos, configuración relativamente guiada, identidad y administración centralizadas.

**Límites para Ícaro:** producto cerrado, controlado por una cuenta y servicio externo; integra otra aplicación que instalar, actualizar y diagnosticar; la licencia y límites de los planes pueden cambiar; no aporta el inventario de mods, la autenticidad del anfitrión de Ícaro ni el arranque de nuestro servicio. Añadirlo como requisito aumentaría soporte sin resolver un problema que ZeroTier ya cubre. Solo tendría sentido como endpoint de red detectado, si usuarios reales ya lo usan, nunca como dependencia del protocolo de sincronización.

### Tailscale

Tailscale crea una red privada basada en WireGuard y agrega un plano de coordinación para identidad, claves, dispositivos, políticas y descubrimiento. Intenta conexiones directas entre dispositivos; cuando no es posible, usa relés DERP. El relé da conectividad con menor rendimiento que un camino directo, pero evita que ciertas restricciones de NAT/firewall dejen a los peers completamente incomunicados. La identidad suele estar ligada al proveedor de inicio de sesión y a la organización o “tailnet”.

**Ventajas:** experiencia cuidada, diagnóstico de tipo de conexión, administración de dispositivos y políticas, DNS de tailnet; buenas opciones para usuarios que ya lo tienen. Hay componentes y opciones de autoalojamiento de controladores comunitarios, pero no son intercambiables sin más con el servicio oficial.

**Límites:** el plano de coordinación y las cuentas forman parte de la experiencia; la aplicación sigue siendo requisito externo. Las políticas de acceso deben configurarse por el administrador. Que dos equipos tengan Tailscale no implica que compartan tailnet ni que puedan abrir cualquier puerto. Las condiciones de planes cambian y no deben incorporarse como una promesa fija de Ícaro.

### WireGuard directo

WireGuard es un protocolo VPN criptográfico ligero de capa 3. Proporciona túneles entre pares identificados por claves públicas, pero no es por sí mismo una plataforma de amigos: no incluye un directorio de usuarios, invitaciones, gestión central de claves, atravesamiento automático de NAT, relay, política de miembros ni interfaz de soporte. Es necesario distribuir claves, definir endpoints, rutas y firewall. Para Ícaro sería una base demasiado baja para usuarios generales, aunque puede servir a anfitriones avanzados que administren su propia VPN.

### Nebula y NetBird

- **Nebula** es una red overlay cifrada de código abierto con autoridades de certificados y “lighthouses” que ayudan a los nodos a descubrirse. El operador administra certificados, topología y despliegue. Da control y posibilidad de autoalojar, a cambio de trabajo de infraestructura y distribución de credenciales.
- **NetBird** ofrece una experiencia de red privada sobre WireGuard con plano de gestión, políticas, señalización y relés; se puede usar su servicio o desplegar componentes propios según edición y versión. Es más apropiado para equipos/organizaciones que para una partida casual que solo quiere pulsar Jugar.

Ambos son posibles proveedores de conectividad, no alternativas a la capa de sincronización. Para cada uno el launcher tendría que detectar interfaces/rutas y verificar alcance del endpoint, sin mantener sus credenciales ni replicar sus paneles de administración.

### Túneles de juego (por ejemplo, playit.gg)

Un túnel de juego suele ejecutar un agente en el PC anfitrión que abre una conexión saliente a un servicio externo. El proveedor entrega una dirección pública/hostname y encamina el tráfico al puerto local. Esto puede evitar reglas del router y funcionar cuando el anfitrión está detrás de CGNAT. El jugador conecta con el cliente normal de Minecraft; normalmente no obtiene una LAN virtual ni acceso general a otros puertos del anfitrión.

**Encaje posible:** buena opción de “conectar al servidor” para anfitriones que no quieren instalar una VPN a cada amigo. **No encaja automáticamente con la sincronización actual**, porque el servicio de mods es otro puerto y la dirección pública no debe permitir acceso abierto a un endpoint de descarga sin autorización. Se necesitaría un túnel TCP adicional o que ambos protocolos compartan un gateway autenticado. También hay latencia, dependencia del proveedor, límites de plan y necesidad de comprobar condiciones actuales.

### Port forwarding, DNS y servidor dedicado público

Abrir el puerto de Minecraft en router/firewall y dar un DNS es la opción sin cliente VPN, pero requiere IP pública alcanzable, configuración del router, firewall y exposición del servicio al Internet. CGNAT puede impedirlo. No se debe exponer el puerto de sincronización HTTP actual a Internet. Si se ofrece alojamiento público, sincronización debe pasar por HTTPS autenticado o distribución firmada con autorización y cuotas.

## Comparación para el producto

| Opción | Requiere cliente de red a cada jugador | Conexión directa / relay | Red privada general | Complejidad para amigos | Encaje actual |
| --- | --- | --- | --- | --- | --- |
| ZeroTier | Sí | Busca conexión entre pares; la ruta depende de NAT, firewall y relé | Sí | Media | Mejor punto de partida; ya detectado/integrado |
| Hamachi | Sí | Administrado por el proveedor; depende de su cliente/servicio | Sí | Baja a media | Detectar como red existente, no adoptarlo como requisito |
| Tailscale | Sí | Directa si es posible, DERP si hace falta | Sí | Baja si ya existe tailnet | Proveedor opcional a evaluar tras mejorar abstracción |
| WireGuard autogestionado | Sí | Directa configurada; traversal/relay externos | Sí | Alta | Para anfitriones avanzados, no primer recorrido |
| Nebula / NetBird | Sí | Directa y mecanismos de descubrimiento/relay según solución | Sí | Media-alta | Entornos autogestionados, quizá soporte futuro |
| Túnel de juego | Solo el anfitrión suele ejecutar agente | Salida hacia proveedor y relay público | No; solo servicios/puertos publicados | Baja para jugadores | Alternativa de alojamiento; requiere otro diseño seguro para sync |
| Puerto público | No | Conexión normal cliente-servidor | No | Media/alta para anfitrión | Soporte existente para Minecraft; no usar HTTP actual públicamente |

La disponibilidad de relé, planes gratuitos, límites de miembros y requisitos de instalación cambian. Ícaro debe comprobarlos en páginas oficiales antes de ofrecer instrucciones específicas o afirmar que un plan es gratuito.

## Riesgos y fallas que hay que modelar

### 1. “La VPN está conectada” no significa “el servicio responde”

Son capas diferentes: cliente VPN en línea → red aprobada → ruta/IP válida → firewall permite puerto → proceso `icaro-sync-host` escucha → índice listo → cliente valida versión/mods → plan aplicable. El mensaje de error debe reportar qué etapa falló. El fallo anterior del anfitrión es un ejemplo claro: ZeroTier tenía ping, pero TCP 25566 estaba cerrado/ausente.

### 2. El puerto `Minecraft + 1` es una convención, no un contrato robusto

Puede colisionar con otro programa, cambiarse en el host o quedar bloqueado aunque Minecraft use otro endpoint/proxy. Guardar explícitamente una URL o puerto de sincronización en el perfil del servidor es más claro. Puede mantenerse la derivación como valor predeterminado y comprobar el puerto con una solicitud de salud. Separar estado Minecraft y estado sync en interfaz y logs.

### 3. La IP virtual no es una identidad permanente

Las IP pueden cambiar, habrá varias interfaces/IPv4/IPv6 y DNS de red distintos. Guardar una IP sin actualizarla crea fallos difíciles de entender. Cada servidor debería guardar una identidad de anfitrión/red y una lista de endpoints candidatos (LAN, ZeroTier, Tailscale/DNS, público), con prioridad y última conexión satisfactoria. El launcher prueba endpoint de salud y conserva cuál funcionó; no debe escanear rangos de LAN.

### 4. Cifrado de overlay y autorización de dispositivo no autentican el servicio Ícaro

HTTP dentro de ZeroTier evita exponer tráfico como Internet público, y el overlay cifra su propio tránsito. Sin embargo, cualquier miembro autorizado que tenga ruta al puerto puede consultar/descargar el índice; el protocolo Ícaro no exige prueba de que el servidor HTTP corresponde al amigo que se eligió. Un cliente autorizado malicioso puede intentar consumir recursos. Un miembro con capacidad de interferir en la red puede intentar suplantación/alteración según las reglas de la capa y el endpoint. SHA-512 detecta archivo distinto al hash del índice, pero si índice y hash se sustituyen juntos, el cliente no conoce el origen legítimo.

El mínimo recomendable antes de ampliar proveedores: identificador criptográfico persistente del anfitrión (Ed25519 u otra firma estándar), manifiesto firmado con versiones, revisión y SHA-512 de cada artefacto, y vínculo/pinning explícito al aceptar una invitación. Un cambio de clave debe requerir confirmación; incluir caducidad y anti-replay. Para privacidad de transporte del protocolo propio, TLS con clave fijada o TLS mutual puede añadirse, pero una firma de manifiesto mantiene verificable el contenido incluso al transferirlo por CDN/caché. El sistema de claves debe tener respaldo, revocación y recuperación documentados; no ocultarlo detrás de ZeroTier.

### 5. El anfitrión sigue siendo una superficie de ataque

El servicio lee archivos del directorio de mods y ofrece descargas. Debe escuchar solo en la interfaz elegida, aceptar únicamente rutas fijas, rechazar symlinks/rutas traversal, poner límites de conexiones, tamaños, solicitudes y tiempo, no exponer archivos arbitrarios, no devolver trazas con rutas/secretos y cerrarse con el servidor. Un firewall del sistema debe limitar también origen/interfaz; una IP privada no reemplaza esas reglas.

### 6. Los mods requieren más que una lista plana

Ya se compara Minecraft, loader, dependencias, hash y JAR propios. Para evolucionar: fijar loader y entorno de cliente/servidor, respetar dependencias obligatorias/opcionales/incompatibles, dejar claros los mods que se conservan o eliminan, manejar shaders/resource packs/configs como categorías separadas y no borrar archivos no administrados. CurseForge y descargas con restricciones requieren integración con fuente/consentimiento legítimos; no replicar sus archivos desde terceros. Los archivos propios del host requieren revisión y atribución de procedencia.

### 7. Inicio del servicio y ciclo de vida

Ahora hace falta una terminal/comando aparte y el servicio puede tardar preparando índice. Mejorar instalación antes de automatizar elevación: empaquetar el host, ofrecer un botón “Compartir servidor” que revise directorio/versiones/IP/puertos, iniciar y monitorear proceso con cancelación, cerrar al parar el servidor y preservar logs acotados. En Windows, cualquier regla de firewall o servicio del sistema requiere acción explícita del usuario y explicar exactamente puerto e interfaz. En Linux, documentar systemd/usuario sin privilegios y señales de cierre.

## Arquitectura candidata para Ícaro (no aprobada)

### Capa de red y conexión

Crear conceptualmente una interfaz `ProveedorRed` con capacidades detectables: estado local, IPs/interfaces, rutas, nombre/DNS y salud. ZeroTier sería el primer adaptador; la resolución genérica del destino no debe exigir ZeroTier. Tailscale solo se implementaría si las pruebas con jugadores muestran demanda clara y hay una interfaz local soportada y estable. No leer archivos privados de configuración ni extraer tokens de otro cliente sin una API documentada.

El destino del servidor debe poder tener varios endpoints ordenados: LAN primero cuando aplica; overlay configurado; DNS/público como alternativa. La app prueba TCP al puerto Minecraft y al endpoint sync por separado y guarda latencia/tipo de camino cuando el proveedor lo permite. No confundir “ICMP ping” con puerto TCP abierto ni con ping del protocolo Minecraft.

### Invitación y red

La invitación debería ser un flujo guiado: identificar proveedor, mostrar los pasos de instalación/entrada, aceptar que el administrador puede tener que autorizar al miembro, comprobar que el peer aparece aprobado, comprobar ruta y salud de puertos. El ID de una red ZeroTier no es secreto ni prueba de que el amigo sea de confianza. No almacenar tokens de Central en el launcher.

Fase inicial: aceptar que el anfitrión o administrador cree y administre redes en la aplicación oficial; Ícaro detecta y explica. Después, evaluar onboarding asistido con el API oficial y una confirmación clara, sin administrar la red desde un backend propio. No prometer instalación sin privilegios si el sistema operativo exige permisos.

### Alojamiento

Mantener proceso Minecraft y servicio de publicación separados internamente, pero gestionarlos desde una sola experiencia. Un botón configura la carpeta, toma versiones reales del server y detecta endpoints; enseña puertos y firewall antes de iniciar. El host puede seguir corriendo un servidor dedicado sin launcher. La lista de salud debe mostrar por separado “Minecraft”, “índice de mods” y “red”.

Cambiar `puerto Minecraft + 1` por `endpoint_sync` explícito en configuración compartida, con derivación automática solo como conveniencia y sin asumir que dos servicios comparten IP. El cliente debe poder probar un `GET /health` pequeño antes de descargar el índice.

### Protocolo de sincronización

Conservar el contrato declarativo: el anfitrión publica una revisión inmutable del pack; el cliente resuelve fuentes, genera plan visible y aplica con rollback. Evolucionarlo así:

1. Manifiesto versionado, canónico y firmado por identidad del anfitrión; incluir juego, loader, versión, artefactos, hashes, tamaño, capacidades requeridas, revisión y fecha de emisión.
2. Cliente verifica firma antes de consultar fuentes o proponer cambios; hash final se comprueba siempre.
3. Autenticación de acceso al servicio por invitación/clave derivada por dispositivo o certificados del overlay. Evitar una contraseña compartida en la URL o un token permanente en logs.
4. Límites por cliente y anfitrión, reintentos con `Retry-After`/backoff, caché por hash/versión, descargas reanudables y concurrencia acotada. Los `ETag`, rangos y hashes actuales ya son buenos cimientos.
5. Tienda de contenido por SHA-512 compartida entre instancias: reutilizar el mismo JAR descargado, evitar duplicados en disco y descargar desde Modrinth u host según disponibilidad. La firma determina quién pidió publicar el hash; HTTPS y el hash validan el transporte/contenido.
6. Opcional futuro: caché entre amigos o BitTorrent/P2P de artefactos solo después de tener manifiestos firmados, privacidad, límites y consentimiento. Nunca distribuir mundos o archivos arbitrarios por defecto.

### Experiencia y diagnósticos

Una tarjeta de conexión debería distinguir: red privada apagada, red no autorizada, host fuera de línea, DNS/IP obsoleta, ruta sin conexión, puerto Minecraft cerrado, servicio sync detenido, índice preparándose/503, incompatibilidad de loader, límites Modrinth y descarga/hash fallido. Mostrar “Copiar diagnóstico” con proveedor, dirección anonimizada si el usuario quiere, etapa, código HTTP y latencia; excluir tokens, IP públicas y rutas privadas por defecto.

La sincronización debe ofrecer estados concretos: “revisando conexión”, “leyendo conjunto”, “consultando catálogo”, “descargando N de M”, “verificando” y “restaurando”. Si se cae la red durante un JAR, reanudar con hash y sin dejar un archivo parcial activo. El botón de cancelar debe parar tareas de red y restaurar una operación incompleta.

## Plan de evaluación candidato

### P0 — medir la conexión directa antes de diseñar el puente

Antes de escoger protocolos o proveedores, probar el mundo LAN integrado y el servicio de mods con dos equipos reales Windows/Arch. Separar: misma LAN, IPv6 público cuando exista, router con UPnP/NAT-PMP habilitado con consentimiento, port forwarding manual y redes con NAT restrictivo/CGNAT. Si se dispone de ZeroTier/Tailscale, incluirlos solo como controles comparativos opcionales, no como requisito del producto. Registrar tiempo de invitación, éxito de conexión, latencia, throughput, bytes retransmitidos, endpoint utilizado y tiempo de sincronización con 0, 1, 50 y 150 mods. No recopilar credenciales. Incluir suspensión/reanudación, cambio de IP, firewall bloqueado y corte durante un JAR.

### P1 — mejorar robustez sin otra red

- Diseñar diagnósticos por etapas y separar estado Minecraft/sync.
- Hacer endpoint sync explícito, health check y fallback a endpoint guardado; conservar derivación actual como migración.
- Confirmar que `icaro-sync-host` no escuche fuera de la interfaz esperada y documentar reglas de firewall de mínimo alcance.
- Validar con dos PCs y modpack de 142+ elementos; medir caché y reintentos bajo condiciones controladas.
- Firmar manifiesto y asociar una clave al anfitrión antes de enviar binarios a través de nuevos proveedores o Internet.

### P2 — integración de producto

- Prototipo que detecta el puerto de mundo LAN, abre un túnel local y conecta al invitado por loopback, sin crear interfaz virtual del sistema.
- Comprobar qué loaders pueden integrar un mod auxiliar mínimo y qué necesita permiso del sistema operativo.
- Diseñar invitación con identidad de anfitrión, expiración y revocación; usar señalización de poco tráfico sin relé de juego en el primer experimento.
- Tratar ZeroTier/Tailscale como posibles alternativas optativas para comparar o desbloquear casos, no como dependencia.

### P3 — opciones de acceso

Probar proveedor de túnel de juego como opción de conectividad al puerto Minecraft. Para sync, exigir canal autenticado independiente o backend que entregue el paquete firmado por HTTPS; no publicar el HTTP actual como puerto abierto. Evaluar soporte de servidores públicos con CDN solamente después de formalizar política de distribución, cuotas y consentimiento.

## Propuesta abierta para evaluar

1. **Probar un puente de aplicación sin IP de sistema** antes de adoptar una interfaz de red virtual. El anfitrión expondría el puerto del mundo LAN a un puente local; el jugador conectaría mediante un endpoint local administrado por Ícaro. La viabilidad está pendiente de un prototipo.
2. **Mantener la sincronización independiente de VPN** y basarla en un manifiesto autenticado del conjunto de mods. La forma de autenticación está por diseñar.
3. **No hacer obligatoria una VPN de terceros.** ZeroTier, Hamachi y Tailscale podrían investigarse como alternativas optativas; ninguna está seleccionada.
4. **Intentar conexión directa antes que relé.** Sin relé propio no se garantiza conexión en toda red. La aceptación de esta limitación debe confirmarse para el alcance del producto.
5. **No exponer HTTP de sincronización a Internet.** Antes de túneles públicos o hosting hacen falta autenticación e identidad del anfitrión.
6. **No reemplazar una VPN con una red virtual propia de capa de sistema en el primer prototipo.** Un túnel por aplicación parece menos invasivo, pero no está validado.

## Fuentes primarias para contrastar

Las páginas de proveedor son documentación viva. Reconfirmar capacidades, precios, límites y compatibilidad de SO antes de diseñar una integración concreta. Desde este entorno, las peticiones directas a la mayoría de los dominios de documentación quedaron bloqueadas por el proxy de red; por eso los enlaces se dejan para verificación humana y no afirmo haber comprobado en vivo sus límites comerciales el día de esta nota.

- [ZeroTier: conceptos](https://docs.zerotier.com/what-is-zerotier/), [FAQ](https://docs.zerotier.com/faq/), [CLI](https://docs.zerotier.com/cli/), [reglas de flujo](https://docs.zerotier.com/rules/), [Central](https://my.zerotier.com/), [API local del servicio](https://github.com/zerotier/ZeroTierOne/blob/dev/service/README.md).
- [Hamachi](https://vpn.net/) y [ayuda de GoTo](https://support.goto.com/hamachi).
- [Tailscale: descripción](https://tailscale.com/kb/1151/what-is-tailscale), [tipos de conexión](https://tailscale.com/kb/1257/connection-types), [DERP](https://tailscale.com/kb/1232/derp-servers).
- [WireGuard: sitio oficial](https://www.wireguard.com/), [conceptos y despliegue rápido](https://www.wireguard.com/quickstart/).
- [Nebula: documentación](https://nebula.defined.net/docs/), [repositorio](https://github.com/slackhq/nebula).
- [NetBird: arquitectura](https://docs.netbird.io/about-netbird/overview), [repositorio](https://github.com/netbirdio/netbird).
- [playit.gg](https://playit.gg/) y [soporte](https://playit.gg/support/).
- [Modrinth API](https://docs.modrinth.com/api/) y [versión por hash](https://docs.modrinth.com/api/operations/versionfromhash/).
- [Cloudflare Tunnel](https://developers.cloudflare.com/cloudflare-one/networks/connectors/cloudflare-tunnel/): referencia de túnel con agente saliente, con limitaciones para clientes TCP arbitrarios.
