# Compartir instancias y publicar revisiones entre dos Ícaro

Estado al 07-11-2026: acuerdos de producto vigentes, núcleo y componentes implementados; integración funcional del launcher y validación externa pendientes. [Progreso y evidencia](../progreso/2026-11-07-compartir-instancias.md).

El transporte iroh, Worker/D1 y las quince vistas iced ya existen. Hay controladores experimentales del publicador, invitado y Sincronizar/Jugar bajo `ICARO_COMPARTIR_DESARROLLO=1`; por defecto sigue deshabilitado. Al 08-11-2026 se conectaron elección de orden de packs, biblioteca, reconexión conservando accesos, contadores y bandeja Windows, y se desplegó el servicio gratuito con prueba HTTP remota. Faltan perfil/partida reales, validación entre equipos y bandeja nativa Linux. Los acuerdos siguientes sustituyen los puntos incompatibles de la propuesta inicial, conservada al final como antecedente.

## Acuerdos vigentes

### Referencias e interfaz

Los [dos paquetes de diseño](../referencias/compartir-instancia/README.md) se conservan sin modificar. El segundo complementa al primero y reemplaza sus componentes y recursos coincidentes. El usuario exige conservar tal cual estructura, textos, medidas, fuentes, grabados, colores, estados e iconos, incluidos `+`, `~`, `−`, `!` y `=` con sus cuadros de 20 × 20, monoespaciada de 14 px, fondos y bordes originales. Todos los iconos se centran en ambos ejes.

Los acuerdos explícitos prevalecen sobre las notas de los ZIP. Sus apartados «En iced» son orientación técnica, no decisiones de arquitectura confirmadas. La comparación de documentación y CSS no sustituye la revisión de previews renderizadas ni las capturas reales iced en Tinta/Piedra.

Regla añadida expresamente por el usuario: rótulos e iconos de botones centrados
en ambos ejes, padding simétrico y controles equivalentes con el mismo alto; las
celdas de un resumen comparten ancho y alto aunque sus textos ocupen varias líneas.
Evitar compensaciones mediante márgenes negativos o desplazamientos manuales.

### Encuentro y presupuesto

Se eligió Cloudflare Workers Free + D1 Free para resolver códigos cortos de invitación, con presupuesto de cero y sin contratar planes de pago. Solo intercambia metadatos de encuentro y autorizaciones; no aloja mods ni partidas. El servicio se implementó y probó localmente, sin crear recursos remotos ni desplegar. iroh está implementado como canal autenticado entre equipos, conservando el recorrido HTTP privado existente y sin exigir ZeroTier.

Fuentes consultadas: [límites de Workers](https://developers.cloudflare.com/workers/platform/limits/) y [precios de D1](https://developers.cloudflare.com/d1/platform/pricing/). Revalidar las condiciones antes de desplegar. El servicio y los relés tienen límites propios; no se promete capacidad gratuita ilimitada.

La implementación vincula código, clave pública y publicación mediante peticiones y encuentro firmados; D1 aplica aceptación atómica, límites y protección de repetición. Se prueba localmente; falta validar el entorno desplegado y su consumo real. Un código de 12 caracteres no contiene una invitación completa ni un secreto de 256 bits. Los detalles de la invitación autocontenida del antecedente no se trasladan automáticamente a este recorrido.

### Invitaciones y accesos

- Caducidad configurable: 24 horas por defecto, 2 días o 1 semana. La implementación permite editar una invitación vigente incluso tras aceptaciones, sin reducir el cupo por debajo de los aceptados ni llevar el vencimiento al pasado; falta conectar esos ajustes al controlador.
- Cupo configurable de identidades distintas, por ejemplo 10. Con límite 1 funciona como invitación de un uso. Reconectarse no consume otro cupo.
- Al vencer, revocarse o completarse el cupo, no admite personas nuevas. Generar otra invitación invalida el código anterior sin revocar accesos aceptados.
- Los accesos aceptados permanecen hasta revocarlos, incluso tras cerrar y abrir Ícaro. Para sincronizar deben estar conectados ambos equipos y disponible la publicación.
- Revocar corta transferencias y futuras sincronizaciones; no elimina lo ya instalado en el invitado.

El máximo de 50 personas y el valor inicial de 10 del ZIP son propuestas del diseño, no límites confirmados. Cupo de invitación, accesos autorizados y transferencias simultáneas son cantidades diferentes; el máximo de ocho sesiones del antecedente tampoco es una decisión aprobada.

### Contenido y destino

Aceptar una invitación siempre crea una instancia nueva vinculada a esa publicación, instalando Minecraft y loader necesarios mediante los servicios existentes. No se ofrece una instancia existente como destino. Las sincronizaciones posteriores actualizan la instancia vinculada.

El anfitrión selecciona mods, configuraciones, scripts de `kubejs` y resourcepacks. Los mundos, cuentas, credenciales y ajustes personales quedan fuera. Los JAR y packs propios se transfieren desde su equipo con autorización y verificación de integridad. CurseForge requiere validar una integración real; los textos de catálogo de las previews no acreditan que exista.

Los packs integrados viajan dentro del JAR sin duplicarse; los externos requieren su ZIP o carpeta. Se comparten selección, activación y orden. La instancia nueva reproduce la selección compartida y las actualizaciones conservan packs personales, ajustando solo la selección pertinente sin copiar todo `options.txt`. El ejemplo de Punchy está documentado en su [guía de resourcepacks](https://wiki.punchymod.com/punchy/resource-packs). Falta precisar cómo combinar el orden compartido y los packs personales y tratar ajustes específicos de mods.

### Publicación manual y conflictos

Instalar, actualizar o quitar contenido local nunca cambia automáticamente lo publicado. PanelPublicacion indica cambios sin publicar y abre RevisionPublicacion mediante «Revisar cambios».

- Contenido nuevo: no se comparte automáticamente; se elige compartir o mantener privado. Un mod solo de cliente puede compartirse si el anfitrión lo selecciona.
- Contenido privado: permanece excluido entre revisiones y puede volver a incluirse.
- Actualizaciones y retiradas compartidas: se proponen para publicar y pueden posponerse, manteniendo la versión publicada anterior.
- Problemas: bloquean únicamente la selección que necesita el contenido afectado. Los archivos propios nuevos o modificados necesitan la autorización correspondiente.
- Preparar una revisión mantiene la anterior disponible; la nueva solo es visible tras publicarse correctamente. Cancelar o fallar no publica una mezcla parcial.

Al pulsar Jugar se comprueba la revisión nueva; también existe Sincronizar manualmente. El invitado revisa y confirma antes de descargar o modificar archivos. No se modifica una instancia en uso por Minecraft.

Para archivos gestionados se compara revisión aplicada anterior, copia local y nueva revisión: si solo cambia el anfitrión se propone actualizar; si solo cambia el invitado se conserva su copia; si cambian ambos se elige «Del anfitrión» o «Conservar la mía». Retirar un archivo modificado por el invitado también exige resolver el conflicto. Falta concretar la persistencia de esas elecciones en siguientes revisiones.

### Cierre y segundo plano

Minimizar mantiene la publicación funcionando. Al cerrar con transferencias activas se puede seguir en bandeja hasta terminarlas y cerrar después, o cerrar ahora e interrumpirlas, permitiendo reanudar al volver a compartir la misma revisión. Dejar de compartir ofrece esperar a que terminen, sin aceptar nuevas descargas, o detener ahora; al detenerse revoca los accesos.

El texto «Pausada: Ícaro en segundo plano» de PanelPublicacion contradice este acuerdo y debe corregirse en la implementación y en una futura actualización de la referencia. Los ZIP originales se conservan intactos.

### Implementación técnica y validación pendiente

El índice HTTP anterior solo cubre parte del alcance. El nuevo manifiesto `icaro-compartir/1` ya contempla configuraciones, scripts, resourcepacks, orden y comparación de tres vías, sin ampliar silenciosamente aquel contrato. Conservar el presupuesto de API, streaming, integridad, mods personales y recuperación; leer antes la investigación de rendimiento según AGENTS.md. La optimización incremental de las carpetas nuevas sigue pendiente.

Claves seguras, origen estable, revisiones inmutables/pospuestas, límites, reanudación, cancelación, validación de rutas y respaldos están implementados con pruebas locales. Faltan limpieza, control previo de espacio y coordinación con instalación, partida y launcher. Los scripts seleccionados requieren autorización por hash; su integración visual aún está pendiente. No prometer que son inocuos ni duplicar todos los archivos del invitado o usar enlaces duros a JAR mutables como copias independientes.

Los límites técnicos y módulos concretos son decisiones de implementación registradas en el plan; no se atribuyen como preferencias del usuario. La utilidad de consola sigue pendiente y no sustituye la integración visual. La autorización documental original en `v0.3.x` y la creación/publicación del plan en `v0.4.x` no autorizan commits posteriores de implementación, merges, tags, releases ni despliegues.

Las pruebas locales cubren plazos/cupos, aceptación idempotente, persistencia y revocación, privados/pospuestos, comparación de tres vías, packs, identidad, integridad, cortes y cancelación. Se produjeron capturas reales con fixtures en ambos temas y ventana pequeña. Falta completar el recorrido integrado, disco lleno, mediciones y pruebas en dos equipos por LAN, Internet directo, relé y Linux. Usar datos aislados y APPDATA temporal. La partida LAN es otro recorrido todavía pendiente; sincronizar no acredita jugar.

## Antecedente: propuesta inicial no aprobada

El texto siguiente se conserva para trazabilidad de la exploración. Sus plazos de 15 minutos, invitación de un uso, accesos temporales, destino existente, alcance limitado a mods y ausencia de referencia visual están sustituidos por los acuerdos anteriores. Sus detalles técnicos restantes siguen siendo propuestas pendientes de validación.

## Objetivo y límites

Un anfitrión comparte el conjunto de mods de una instancia y otro Ícaro revisa y aplica los cambios a una instancia compatible. No requiere ZeroTier ni un servidor contratado por el usuario. Se propone iroh para conectar ambos equipos, con identidad criptográfica, conexión cifrada y relés públicos como alternativa cuando no hay ruta directa. Sus límites y disponibilidad deben mostrarse como límites del servicio, sin prometer conectividad o rendimiento universal.

Es sincronización en una dirección: el anfitrión define una revisión y el invitado decide cuándo aplicarla. No mezcla automáticamente dos conjuntos de mods ni copia mundos, cuentas, ajustes personales o configuraciones. Tampoco instala o cambia Minecraft/loader de la instancia de destino ni transporta todavía la partida LAN. La compatibilidad del perfil es un requisito comprobado antes de aplicar.

## Enfoques

Se adopta como dirección la recomendación de iroh aceptada por el usuario tras la investigación de MercHosting. Permite un canal propio entre launchers sin gestionar un VPS inicialmente. Se descarta usar un túnel gratuito de Minecraft de Playit para sincronización: no acredita soporte gratuito para el servicio HTTP propio. Mantener únicamente IP privada/ZeroTier no cumple la conexión buscada entre equipos sin esa dependencia. El protocolo HTTP privado existente se conserva como recorrido compatible.

## Entregas propuestas

1. Núcleo y utilidad de consola de ambos roles para demostrar publicación, invitación, revisión y aplicación usando iroh. Las pruebas iniciales usan carpetas temporales y datos de muestra; después, copias de instancias en dos equipos reales. Esta utilidad es una herramienta de diagnóstico y prueba del núcleo, no la experiencia final del usuario.
2. Integración en la sección Servidores del launcher reutilizando su lista, formularios y la pantalla Sincronizar mods. El objetivo final incluye esta integración; la primera entrega técnica no se presenta como la función terminada del launcher.

## Interfaz propuesta

No se propone una sección nueva de navegación ni sustituir la lista actual. Servidores agrupa los dos accesos: compartir una instancia propia y aceptar una invitación. Se mantiene agregar servidores por dirección para el recorrido actual.

El anfitrión elige una instancia, revisa qué se publicará y confirma compartir los JAR propios identificados que no provengan del catálogo. Ve la invitación, su caducidad, estado de publicación y una acción para dejar de compartir. El invitado introduce la invitación, comprueba la identidad del anfitrión y elige una instancia local compatible. Después reutiliza Sincronizar mods para diferencias, conflictos, aprobación, progreso, cancelación y resultado.

Red privada/ZeroTier sigue disponible para conexiones existentes, pero no es un paso obligatorio de una invitación iroh. La conexión de Minecraft por dirección y su estado deben distinguirse del estado de sincronización. Una invitación de mods no habilita Jugar hacia un destino inexistente ni promete acceso al mundo.

Se usan componentes, tokens, textos y estados de las referencias originales donde existan. Los formularios de compartir y aceptar requieren composición nueva con esos componentes; no hay todavía una maqueta nueva aprobada. Antes de la entrega visual se revisarán las referencias del System Design y capturas reales Tinta/Piedra, ventana pequeña y errores/carga. Ninguna operación de disco o red se ejecuta en update/view.

## Identidad, invitación y sesión

Cada almacén de Ícaro mantiene una clave persistente independiente de Microsoft. Se guarda en el almacén de credenciales del sistema cuando esté disponible; si no lo está, falla explícitamente esta función en la primera entrega, sin guardar la clave en texto plano silenciosamente. El cambio de clave es un anfitrión distinto y no puede heredar el origen anterior automáticamente.

La invitación contiene versión de protocolo, identidad pública del anfitrión, identificador aleatorio del conjunto compartido, pistas de conexión y un secreto aleatorio de al menos 256 bits. No contiene rutas ni credenciales de cuentas. Es un acceso portador temporal: quien tenga el código puede intentar reclamarlo. Caduca por defecto a los 15 minutos y se consume con una aceptación correcta, vinculando la sesión a la identidad del invitado. El anfitrión limita intentos y sesiones. El invitado confirma la huella del anfitrión; no se presupone una identidad real por un nombre de pantalla.

La sesión y las autorizaciones de esta primera entrega son temporales y no sobreviven al cierre del anfitrión. Tras reconectar con la misma identidad de invitado, una sesión vigente puede reanudar la transferencia. Dejar de compartir revoca invitaciones y sesiones, cierra transferencias y listeners. No se imprimen invitaciones o secretos en logs; la consola muestra el código únicamente como salida solicitada para compartir. La interfaz evita incluirlo al copiar diagnósticos.

## Publicación y protocolo

Se crea una publicación por instancia, identificada por la clave del anfitrión y un ID persistente de publicación asociado a esa instancia. Esa pareja, nunca una IP o un token de invitación, define el origen usado para conservar o retirar mods gestionados. Reabrir la publicación con una invitación nueva conserva el origen; otra instancia o clave no lo hereda.

Se reutiliza Indice con sus versiones exactas y SHA-512 y se lo envuelve en un protocolo propio versionado de iroh. El motor actual de publicación desde instancia debe admitir JAR propios identificables reutilizando las validaciones existentes para carpetas dedicadas, sin duplicar parsers. Un archivo inválido o de identidad dudosa bloquea su publicación con diagnóstico; no se omite silenciosamente. Se conserva la política de compatibilidad cliente/servidor actual; reproducir todos los mods opcionales del anfitrión queda fuera de esta primera entrega y se explica en la revisión.

El anfitrión materializa una revisión coherente del índice y una instantánea independiente de los JAR propios autorizados fuera de la instancia. Comprueba el espacio, copia sin enlaces duros, verifica tamaño/hash y publica después. Un cambio durante la preparación obliga a repetirla o falla sin publicar una mezcla. Las instantáneas se limpian al dejar de compartir o recuperan/limpian al siguiente arranque; no se borran datos personales. No se generan instantáneas de todo mods en el invitado ni se cambia su transacción incremental.

Solo se permiten operaciones explícitas: reclamar invitación, obtener índice/revisión y leer un archivo autorizado de esa revisión por hash y desplazamiento. No hay rutas arbitrarias, navegación de carpetas ni ejecución de comandos. Índice máximo de 1 MiB y hasta 512 artefactos, conforme al contrato actual. Dos transferencias de JAR simultáneas por sesión y un máximo inicial de ocho sesiones por publicación; metadatos no esperan a los JAR. Los límites de bytes por archivo y agregado de la instantánea se configuran explícitamente y se documentan antes de la prueba; el implementador propone valores conservadores y errores claros al alcanzarlos.

El transporte conserva streaming, límites de tiempo, cancelación y reanudación por revisión/hash/desplazamiento. Los parciales viven solo en staging y nunca se activan sin verificar tamaño y SHA-512. Se revalida la revisión vigente justo antes de iniciar la transacción final; un cambio del anfitrión exige revisar de nuevo. Revocar durante descargas cancela sin publicar; una transacción local final ya iniciada termina o revierte con las garantías existentes.

## Integración con el motor actual

Se separa identidad de origen de mecanismo de descarga. Se añade una interfaz acotada para consultar revisión y descargar archivos propios, con adaptadores HTTP privado e iroh. Las llamadas públicas existentes conservan su comportamiento mediante el adaptador HTTP. El origen iroh jamás se convierte en una IP local que pudiera cambiar o autorizar retiradas de otro anfitrión.

Los artefactos de catálogo siguen usando Fuente/Catalog, lotes, presupuesto de API y CDN actuales. Se conservan Plan, análisis incremental, conflictos, autorización de reemplazos, archivos desactivados, mods personales y diario recuperable. No se aplica sobre una instancia ocupada por Minecraft. La consola debe comprobar la ocupación mediante el mismo mecanismo del núcleo que usa el launcher, o restringirse a copias de prueba aisladas si esa comprobación no es accesible; esa limitación debe quedar visible en su ayuda.

Se registran fases, origen público abreviado, revisión, bytes y ruta directa/relé cuando el SDK permita observarla. Nunca tokens, claves, rutas absolutas o datos de cuenta. Hashes y firmas tienen funciones distintas: el canal autentica al interlocutor y los hashes verifican el contenido. Exportar manifiestos para usarlos fuera del canal autenticado no forma parte de este protocolo inicial.

## Validación y entrega

Se trabaja en v0.3.x, sin nuevas ramas/worktrees, commits o publicaciones. Antes de añadir iroh se comprueban Rust 1.99.1, APIs y soporte Windows/Linux; se actualiza Cargo.lock mediante Cargo sin subir otras dependencias deliberadamente.

Regresiones necesarias: invitación incorrecta/caducada/reutilizada, identidad distinta, revocación, límites de mensajes, archivo no publicado, hash incorrecto, corte y reanudación, cancelación, revisión cambiada, preservación de mods personales y origen estable tras nueva invitación. Pruebas de red local con relés desactivados; pruebas externas separadas para ruta directa y relé. Las pruebas que acceden al almacenamiento del usuario se ejecutan con APPDATA temporal y nunca sincronizan sus instancias reales por defecto.

La consola permite ejecutar anfitrión e invitado, mostrar un plan sin escribir y aplicar solo con aprobación explícita del operador. La ayuda y la guía incluirán comandos exactos cuando exista la utilidad, sin inventar hoy comandos operativos. Se ejecutan las comprobaciones Windows del repositorio con -j 2 y pruebas UI secuenciales, además de git diff --check. La prueba de papelera conocida se informa aparte si reaparece con su error habitual.

Para la validación real hacen falta dos equipos con Ícaro actualizado, conexión a Internet y copias de instancias con el mismo Minecraft/loader. Primero misma LAN y después redes distintas, registrando si la ruta es directa o relé. Se comprueban cero cambios, altas, actualización, retirada gestionada y un JAR propio; repetir después de corte y cancelación. Se informa por separado qué pasó con fixtures, dos equipos Windows y Linux; ninguna prueba de sincronización acredita por sí sola jugar una partida.

## Decisiones propuestas por el implementador

Son propuestas, no preferencias ya confirmadas: primera prueba por consola antes de la interfaz, mods compatibles según la política actual en lugar de clonar todo mods, una revisión materializada, invitaciones de un uso de 15 minutos, autorizaciones temporales, claves en el almacén de credenciales y mantener el juego fuera de este primer alcance. La integración final se concentra en Servidores y Sincronizar mods. El usuario puede ajustar estos puntos antes del plan de implementación.
