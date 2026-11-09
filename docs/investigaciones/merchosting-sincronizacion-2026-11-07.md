# MercHosting: alojamiento, conexión y distribución de mods

Fecha: 07-11-2026. Investigación documental y lectura de código público.

Referencia de MercHosting: commit `137580c421e9c84cbcdcca2adc71dce904a9472c` de `procy00fr-source/MercHosting-app`. Los servicios externos se consultaron en la fecha del informe; sus condiciones pueden cambiar.

No se instaló ni ejecutó MercHosting, no se probaron dos equipos y no se modificó el motor de Ícaro. Las propuestas siguientes no son decisiones de arquitectura aprobadas.

Los acuerdos posteriores sobre invitaciones, contenido y publicación se recogen en el [diseño de compartir instancias](../superpowers/specs/2026-11-07-sincronizacion-entre-launchers-design.md), junto con las referencias visuales entregadas por el usuario. Este informe conserva el alcance de la investigación original.

## Resultado

MercHosting permite alojar un servidor sin contratar hosting: Minecraft corre en el PC del anfitrión, que aporta CPU, RAM, disco, electricidad y conexión. Para entrar desde Internet integra el agente de Playit, cuyo servicio dispone de opciones gratuitas para Minecraft. Para administración entre equipos usa otro canal, basado en iroh.

La función revisada para compartir mods genera un archivo `.mrpack` de referencias. No implementa por sí misma una sincronización continua y completa entre launchers. El código de esa exportación escribe únicamente `modrinth.index.json`, sin JAR, configuraciones ni mundos.

Por tanto, el proyecto demuestra una combinación de herramientas utilizables sin una cuota obligatoria en esos recorridos. No demuestra ausencia de infraestructura externa ni capacidad ilimitada gratuita.

Fuentes: [README](https://github.com/procy00fr-source/MercHosting-app/blob/137580c421e9c84cbcdcca2adc71dce904a9472c/README.md), [sitio del proyecto](https://procy00fr-source.github.io/merchosting/), [exportación de pack](https://github.com/procy00fr-source/MercHosting-app/blob/137580c421e9c84cbcdcca2adc71dce904a9472c/crates/merc-content/src/friendpack.rs).

## Conexión al servidor: Playit

El recorrido es: Minecraft del invitado → dirección pública de Playit → infraestructura de Playit → agente en el PC anfitrión → puerto local de Minecraft.

Solo el anfitrión ejecuta el agente. MercHosting descarga una versión fijada del agente oficial, comprueba SHA-256 y mantiene sus propios archivos de secreto y comunicación local. El usuario vincula el agente mediante el flujo de aprobación de Playit con su cuenta. El código revisado guía al usuario para crear el túnel en la web de Playit; no automatiza esa creación mediante una API propia.

Esto evita que cada invitado tenga que instalar una VPN. El servidor continúa dependiendo de que el PC y el agente estén encendidos, además de la disponibilidad y la ruta del proveedor.

La web oficial de Playit ofrece un nivel gratuito y muestra Minecraft entre los juegos admitidos sin la marca Premium. En cambio, marca HTTPS y TCP genérico como Premium. No se puede inferir que el mismo plan gratuito permita publicar el HTTP de sincronización de Ícaro: es otro servicio y otro protocolo. No se verificaron cuotas concretas desde una cuenta de Playit.

Fuentes: [integración de Playit en MercHosting](https://github.com/procy00fr-source/MercHosting-app/blob/137580c421e9c84cbcdcca2adc71dce904a9472c/crates/merc-net/src/playit.rs), [Playit oficial](https://playit.gg/).

## Pack para amigos: enlaces y hashes

El anfitrión prepara y guarda un `.mrpack`; lo comparte y el amigo lo importa en un launcher compatible. El launcher descarga los mods desde sus fuentes.

La implementación revisada:

- Fija Minecraft y la versión del loader.
- Incluye enlaces de `cdn.modrinth.com`, tamaño y SHA-1/SHA-512 del archivo exacto.
- Excluye los mods identificados como exclusivos del servidor.
- Enumera aparte los mods de CurseForge, JAR propios y archivos no reconocidos para instalación manual.
- No incluye configuraciones, mundos ni JAR dentro del archivo generado.

Los límites anteriores corresponden a esta implementación. No significan que el formato MRPACK en general prohíba todos los archivos adicionales.

La invitación y el pack facilitan preparar un cliente compatible. No equivalen a observar cambios del anfitrión y reconciliarlos automáticamente con una instancia existente. La administración remota sí puede transferir archivos con permisos, pero tampoco demuestra ese recorrido de sincronización de clientes.

Fuentes: [guía Pack para amigos](https://github.com/procy00fr-source/MercHosting-app/blob/137580c421e9c84cbcdcca2adc71dce904a9472c/docs/PACK_PARA_AMIGOS.md), [generación del archivo](https://github.com/procy00fr-source/MercHosting-app/blob/137580c421e9c84cbcdcca2adc71dce904a9472c/crates/merc-content/src/friendpack.rs), [integración de interfaz](https://github.com/procy00fr-source/MercHosting-app/blob/137580c421e9c84cbcdcca2adc71dce904a9472c/app/src/components/InvitePanel.tsx).

## Administración entre equipos: iroh

MercHosting utiliza `iroh = "1.4.1"` en `merc-remote`. Su canal de administración es independiente del túnel de juego de Playit.

El código crea una identidad por equipo y un endpoint con el preset `N0`. El transporte usa QUIC con cifrado y autenticación de extremo a extremo. iroh utiliza relés para ayudar a establecer la conexión, intenta pasar a una ruta directa y mantiene el relé si no consigue esa ruta. No basta con describirlo como P2P sin servidores.

Sobre ese transporte, MercHosting implementa sus propios mensajes, invitaciones con caducidad, permisos, revocación y transferencias de archivos por bloques con tamaño y SHA-256. Esas reglas de aplicación no las aporta automáticamente iroh.

Los relés públicos de n0 son gratuitos, compartidos y limitados, sin garantía de disponibilidad ni rendimiento. La documentación oficial los orienta a desarrollo y pruebas y recomienda opciones de producción de pago para capacidad predecible. Esto permite evaluar la tecnología sin contratar infraestructura; no justifica prometer que cualquier número de usuarios o volumen de JAR tendrá servicio gratuito garantizado.

Fuentes: [dependencia de iroh](https://github.com/procy00fr-source/MercHosting-app/blob/137580c421e9c84cbcdcca2adc71dce904a9472c/crates/merc-remote/Cargo.toml), [transporte](https://github.com/procy00fr-source/MercHosting-app/blob/137580c421e9c84cbcdcca2adc71dce904a9472c/crates/merc-remote/src/lib.rs), [autorización y transferencias](https://github.com/procy00fr-source/MercHosting-app/blob/137580c421e9c84cbcdcca2adc71dce904a9472c/crates/merc-supervisord/src/companions.rs), [relés de iroh](https://docs.iroh.computer/concepts/relays), [planes oficiales](https://www.iroh.computer/pricing).

## Aplicación posible a Ícaro

El motor existente ya prepara un plan, conserva mods personales, verifica los archivos y aplica cambios con recuperación. La carencia para conectar dos launchers por Internet está en el transporte, la identidad, las invitaciones y la publicación integrada. El endpoint actual exige direcciones privadas o loopback y deriva el puerto de sincronización del puerto de juego más uno; no debe trasladarse sin cambios a una dirección pública de Playit.

La hipótesis más útil que aporta este caso es probar iroh como canal de sincronización de Ícaro:

1. El anfitrión comparte una revisión del conjunto de mods desde una instancia elegida.
2. Una invitación vincula la identidad del anfitrión y concede acceso limitado al conjunto compartido.
3. El invitado obtiene el manifiesto por el canal autenticado y prepara el plan con el motor actual.
4. Los mods identificados se descargan del catálogo; los JAR propios autorizados pueden transferirse por el canal entre equipos.
5. Se verifica el contenido y la vigencia de la revisión antes de aplicar los cambios aprobados, conservando respaldo y recuperación.

Esto es una propuesta de evaluación, no una implementación ni una garantía. La autenticación del canal no sustituye las comprobaciones de rutas, tamaños, hashes, permisos y revisión del plan. Si un manifiesto debe conservar su autenticidad fuera de ese canal, hay que definir además su firma y validación.

La partida es otro recorrido: Playit podría ser una opción para el puerto de Minecraft. Transportar el juego también por iroh requeriría investigar un puente de aplicación, detección del puerto LAN, latencia y rendimiento; MercHosting no demuestra ese puente porque usa Playit para el juego.

## Qué falta comprobar

- Compatibilidad de la versión de iroh y sus dependencias con Rust y las plataformas de Ícaro.
- Conexión y revocación entre dos equipos reales, separando LAN, ruta directa por Internet y ruta por relé.
- Tiempo y transferencia de conjuntos representativos, especialmente cuando solo funciona el relé gratuito.
- Cancelación, corte, reanudación, revisión del anfitrión cambiada y preservación de mods personales.
- Límites y condiciones actuales de los servicios para el uso previsto, sin asumir capacidad gratuita ilimitada.
- Alcance de lo compartido: mods, configuraciones y paquetes de recursos tienen políticas distintas; los mundos y datos personales no se comparten por defecto.

Ver también la [investigación previa de redes](redes-virtuales-y-sincronizacion.md) y la [investigación de rendimiento](rendimiento-sincronizacion-2026-10-15.md). Este informe concreta herramientas que merecen un prototipo; no cambia las decisiones todavía abiertas de esas investigaciones.
