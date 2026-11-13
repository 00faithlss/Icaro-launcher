# Roadmap de Ícaro

[Documentación](README.md) · [README principal](../README.md) · [Roadmap visual](roadmap-system-design.md)

Revisado el **11-11-2026** contra `v0.4.x` (`8729e21`, workspace 0.4.2). Este es el registro de alcance funcional: distingue código conectado, pruebas locales y validación en uso real. Las prioridades no son fechas de entrega ni compromisos de versión. Los detalles visuales se mantienen en el roadmap del System Design.

## Código y versiones publicadas

| Línea | Estado comprobado el 11-11-2026 |
| --- | --- |
| Estable | [0.3.3, compilación 40389040180](https://github.com/00faithlss/Icaro-launcher/releases/tag/v0.3.3-build.40389040180), desde `main`. |
| Beta | [0.4.2, compilación 42764789581](https://github.com/00faithlss/Icaro-launcher/releases/tag/v0.4.2-beta.42764789581), desde `develop`. |
| Desarrollo | `v0.4.x` conserva la versión 0.4.2 y añade cambios posteriores a esa beta: nueva pantalla Servidores, salida de instancias recibidas y servidor alojado para amigos. |

La revisión documental comprobó referencias remotas y páginas de releases; no volvió a ejecutar CI, descargar artefactos ni probar una actualización instalada. Tener el mismo número en Cargo no implica contener los mismos cambios. El [changelog](../CHANGELOG.md) y las [notas de versiones](releases/README.md) conservan el historial.

## Qué está implementado

| Área | Alcance conectado | Validación o límite principal |
| --- | --- | --- |
| Cuentas | Microsoft con PKCE y almacén del sistema; varias cuentas sin conexión con identidad estable. | Acceso y recuperación Microsoft comprobados anteriormente en Windows. Faltan renovación/cierre manuales, Linux y varias cuentas Microsoft persistentes. |
| Skins | Perfil, visor 3D, estudio, historial, biblioteca, descarga, selector de capa y skins locales por cuenta. | Falta validar operaciones Microsoft con cuenta real y la skin local dentro de una partida. |
| Minecraft y Java | Vanilla, Fabric y Quilt con perfiles oficiales; Forge/NeoForge con instaladores oficiales spec 1. Selección global/por instancia, detección y runtime gestionado. | Vanilla y Fabric tienen confirmación de juego en Windows. Quilt, Forge y NeoForge requieren partida con mods; Forge/NeoForge, migración y cancelación durante procesadores. |
| Instancias | Persistencia, carpetas legibles, grupos, icono/portada, cambio de versión con respaldo y copia, archivos, capturas, tiempo jugado y tamaño en el editor. | Mundos, recuperación completa y algunas opciones del editor siguen incompletos. |
| Catálogo | Modrinth, compatibilidad, dependencias, JAR propios, actualizaciones, selección múltiple, resourcepacks y shaders con Iris. | Otros motores de shaders, prioridad de recursos por arrastre y perfil interno de creadores pendientes. |
| Modpacks | MRPACK y ZIP completos locales; MRPACK de Modrinth; perfiles Vanilla/Fabric/Forge/NeoForge compatibles. Exportación ZIP Ícaro/Prism y listas de mods. | Importar Quilt, descubrir otros launchers, exportar MRPACK y resolver ZIP de CurseForge solo por IDs pendientes. |
| Servidores | Lista persistida, consulta DNS/SRV, instancia vinculada y sincronización HTTP con anfitrión dedicado. | Falta validar conexión y sincronización entre equipos Windows/Linux. |
| Red privada | Detección de ZeroTier, solicitud de unión, autorización/IP, invitación copiada y desconexión con confirmación. | Instalación/permisos automáticos y administración de miembros pendientes; desconexión nueva sin validación nativa completa. |
| Compartir instancias | Publicador/invitado experimentales, invitaciones, revisión, contenido verificado, recuperación, biblioteca, reconexión y salida conservando archivos. | Activación explícita. Pruebas locales y HTTP remoto del servicio; faltan perfil/partida reales y dos equipos. |
| Alojar servidor | Servidor dedicado Vanilla/Fabric con los mods de la instancia compartida, EULA, consola, `server.properties` y túnel iroh para los invitados. | Probado con túnel local y eco TCP; falta una partida real entre dos equipos por Internet. Forge/NeoForge/Quilt pendientes. [Guía](alojar-servidor.md). |
| Aplicación | Temas Tinta/Piedra, metales, escala, movimiento reducido, transiciones al jugar, consola y registros del launcher. | Falta revisión transversal de foco, teclado, ventanas pequeñas y Linux. |
| Distribución | Instalador Windows por usuario, portable, canales Estable/Beta, copias de ajustes y actualización verificada. | Releases publicadas; instalación limpia y ciclo completo de actualización/reinstalación/desinstalación pendientes en equipo de prueba. |

## 1. Compartir instancias

Prioridad de la línea `v0.4.x`. En el código actual se activa en **Servidores → Con amigos → Activar Compartir experimental**. La nueva pantalla agrupa **Compartes** y **Recibes**; la beta 0.4.2 publicada conserva la navegación anterior.

Ya están conectados la revisión del contenido y consentimiento, confianza por clave completa, destino nuevo, instalación exacta, progreso, recuperación al reintroducir el código, consulta previa a Jugar, conflictos y elección del orden de packs. También existen caché por SHA-512 con copias independientes, concurrencia acotada, reserva por instancia, precomprobación de espacio y limpieza conservadora del staging. El cierre drena transferencias y permite restaurar desde la bandeja Windows; Linux conserva la ventana minimizada.

- [ ] Recorrer publicador e invitado desde la ventana con un perfil Minecraft descargado y una partida real; las pruebas con perfil inyectado no cubren ese paso.
- [ ] Validar dos equipos, conexión directa y relé, Windows/Linux, reconexión y revocación durante el uso.
- [ ] Probar cortes, disco lleno, cancelación, cierre y reinicio con transferencias reales; comprobar reserva de actividad y conservación de archivos personales.
- [ ] Validar bandeja tras reiniciar Explorer y completar la bandeja nativa Linux.
- [ ] Medir en release mediana/p95, RAM, bytes, caché y llamadas; definir limpieza del contenido publicado según revisiones retenidas y retención administrativa del servicio.
- [ ] Completar detección de dependencias ausentes, publicación de shaderpacks y detalle por resourcepack descritos en las notas de 0.4.0.

**Criterio de cierre:** aceptar una invitación entre dos equipos, instalar y jugar, recibir una revisión y recuperarse de errores sin perder mundos, ajustes ni mods personales. [Guía técnica](compartir-instancias.md) · [Evidencias por etapa](superpowers/progreso/2026-11-07-compartir-instancias.md) · [Servicio](../services/invitaciones/README.md).

## 2. Juego, loaders e importación

- [ ] Validar Quilt, Forge y NeoForge con mods compatibles: instancia nueva, reutilización, reinicio, cambio de versión y recorrido nativo Linux.
- [ ] Validar cancelación y recuperación durante los procesadores de Forge/NeoForge; ampliar formatos históricos de Forge fuera de spec 1 cuando se aborden.
- [ ] Extender la importación de modpacks a Quilt y validar partidas de packs Forge/NeoForge.
- [ ] Completar descubrimiento de otros launchers y duplicación desde la acción general. La copia del flujo de cambio de versión y la importación local ya existen.
- [ ] Probar manualmente los ZIP exportados en Prism/MultiMC; las pruebas de estructura no acreditan ese recorrido.
- [ ] Implementar exportación MRPACK con versiones concretas y referencias verificadas.
- [ ] Integrar la revisión de archivos sin origen verificado en los importes pendientes, con confianza asociada al contenido exacto.
- [ ] Resolver acceso y distribución de CurseForge antes de admitir ZIP que solo referencian IDs; mantener el proveedor deshabilitado mientras no exista integración válida.

**Criterio de cierre:** instalar, abrir y reutilizar cada perfil admitido, y poder cancelar o reintentar conservando el origen y la instalación anterior. [Loaders](loaders-forge-neoforge.md) · [Importación](importar-modpacks.md) · [Java](vanilla-y-java.md).

## 3. Instancias, descargas y recuperación

- [ ] Completar General/Java y persistencia de las opciones pendientes, sin duplicar selección/herencia de Java ya conectadas.
- [ ] Leer y gestionar mundos, crear/restaurar respaldos y conectar sus acciones al juego.
- [ ] Ordenar resourcepacks por arrastre en el editor y admitir otros motores de shaders. La elección del orden al sincronizar una publicación es otro recorrido y ya existe.
- [ ] Completar previews, teclado y recuperación desde Archivos/Capturas; conservar papelera del sistema y Deshacer existentes.
- [ ] Persistir el historial de descargas y unificar los instaladores en la cola global.
- [ ] Mostrar tiempo jugado y tamaño en la lista de instancias; validar las estadísticas en una partida real.
- [ ] Validar en juego las preferencias compartidas de sensibilidad, teclas y gráficos.
- [ ] Completar revisión/reparación integral, traslado de la raíz de datos y diagnóstico específico de conflictos.

**Criterio de cierre:** acciones persistentes y recuperables, con errores que conserven los archivos y decisiones del usuario. [Instancias](instancias-locales.md) · [Almacenamiento](organizacion-datos.md) · [Preferencias](opciones-minecraft.md).

## 4. Cuenta, servidores y red

- [ ] Validar renovación, cierre de sesión, almacén Linux y apertura/retorno con los navegadores Windows previstos en la guía de autenticación.
- [ ] Completar varias cuentas Microsoft persistentes y cambio de nombre dentro de Ícaro.
- [ ] Validar subida/historial/biblioteca de skins, selección y descarga de capas con una cuenta real.
- [ ] Validar skins locales en partidas Vanilla/Fabric/Forge/NeoForge, LAN y servidores compatibles.
- [ ] Completar instalación/permisos de ZeroTier y administración de miembros; validar unión y desconexión desde la interfaz con redes reales.
- [ ] Validar conexión al servidor, sincronización HTTP y reconexión entre Windows y Arch Linux, con red directa y ZeroTier.
- [ ] Validar el rendimiento de sincronización dedicada en HDD/Linux, evaluar caché de contenido compartida para ese recorrido HTTP y proveedores adicionales. No trasladar resultados del benchmark a garantías de tiempo del launcher.

**Criterio de cierre:** sesión o conexión real con cancelación, autorización pendiente, fallo de red y recuperación comprobados. [Cuenta](cuenta.md) · [Autenticación](login-y-catalogo.md) · [Servidores](servidores.md) · [Sincronización dedicada](sincronizacion-servidores.md).

## 5. Diseño y distribución

- [ ] Revisar foco/teclado, ventana pequeña, escala, metales y ambos temas; mantener líneas visibles en hover e iconos centrados.
- [ ] Completar adaptación angosta, tooltips, virtualización y movimientos del [roadmap visual](roadmap-system-design.md).
- [ ] Repetir en Arch/Linux el recorrido completo de instalación, cuentas, juego, cierre y recuperación.
- [ ] Validar instalación limpia, actualización remota instalada/portable, reinstalación y desinstalación Windows, conservando cuentas, ajustes e instancias.
- [ ] Validar cambios de canal y restauración de copias con paquetes publicados. Un cliente antiguo sigue ejecutando su propio actualizador.

**Criterio de cierre:** paquetes reales y recorridos nativos comprobados. Las pruebas de PowerShell, fixtures y capturas no sustituyen esa validación. [Contrato del actualizador](actualizaciones-launcher.md) · [Instalador](instalador-windows.md).

## Cómo mantener este roadmap

Registrar implementación y validación por separado, enlazar la guía o evidencia y actualizar la fecha solo en el alcance revisado. Una tarea cerrada pasa a la tabla de capacidades o al changelog; no copiarla en varios inventarios. Las notas de versiones, planes y mediciones conservan el contexto de su fecha. No se calcula un porcentaje global: las tareas no tienen el mismo tamaño ni el mismo riesgo.
