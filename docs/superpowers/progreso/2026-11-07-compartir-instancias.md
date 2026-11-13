# Progreso de Compartir instancias — 08-11-2026

[Historial técnico](../README.md) · [Estado actual](../../roadmap.md#1-compartir-instancias)

Registro acumulativo de evidencias del 07–08 de noviembre. Los apartados antiguos conservan pendientes que se resolvieron después; consultar el roadmap para continuar y las secciones fechadas para valorar cada prueba.

Rama de trabajo comprobada: `v0.4.x`. Versión del workspace: `0.3.4`, sin bump.
El plan original está en `d78e628`. La implementación se registra en `d48181f`
(núcleo y servicio) y `f4fbf08` (interfaz experimental), por solicitud del usuario
de subir todo el bloque a `v0.4.x`. Este registro acompaña esos commits y su
documentación; no supone una release ni una función habilitada por defecto.
No se revalidaron releases ni CI remotos.

El publicador y el invitado se subieron después en `01a55ed` y `3ed741b`.
La continuación del 08-11-2026 añade progreso y recuperación del intento; se registra
y sube a `v0.4.x` por autorización del usuario, sin cambio de versión ni despliegue,
en `bb44998`. Los cambios posteriores se registran en la misma rama.

## Caché y descargas del 08-11-2026

El invitado comparte contenido verificado por SHA-512 entre instancias. El
coordinador deduplica los archivos, resuelve solo metadatos pendientes y limita
las descargas por origen. El anfitrión separa los cupos de archivos y de control.
Las métricas del coordinador quedan en el diagnóstico local. La transacción del
destino conserva copias independientes y su recuperación ante cortes.

Ver [mediciones locales y pendientes](../../investigaciones/rendimiento-compartir-2026-11-08.md).

## Cierre de pendientes del 08-11-2026

El usuario pidió continuar todos los puntos seguidos y luego inició sesión en
Cloudflare. Se mantuvieron `v0.4.x`, versión 0.3.4, la carpeta única y los cambios
anteriores. No se realizó commit, push, integración de canales ni release.

- Orden de packs: el conflicto se revisa en la misma pantalla y controles del
  diseño original. Elegir anfitrión o conservar la selección local conserva los
  personales; se retiran activaciones obsoletas y se rechazan cambios externos
  antes de descargar/preparar/aplicar. Una regresión reprodujo y corrigió activar
  un pack que el usuario había decidido conservar ausente.
- Biblioteca: lista publicaciones propias persistidas, recupera sin crear otra
  identidad/origen y reconecta sin revocar accesos. Los destinos del invitado y
  publicaciones ajenas se excluyen. El cierre durante recuperación conserva el
  contexto y al reabrir reconecta. Contadores cancelables cada minuto con panel
  abierto para respetar el presupuesto del servicio; errores conservan datos.
- Cierre: sin transferencias no muestra diálogo; con ellas ofrece esperar en
  segundo plano o cerrar ahora. Espera el acuse de operaciones locales, rechaza
  nuevas transferencias al drenar y conserva autorizaciones al cerrar. El diálogo
  muestra equipo, archivo, bytes y porcentaje desde el transporte en memoria.
- Bandeja Windows: hilo nativo con icono propio, restaurar/salir, retirada al cerrar
  y gestión de pérdida de icono/hilo. Sin bandeja conserva una ventana minimizada.
  Linux usa ese fallback; todavía falta su integración nativa.
- Espacio: suma comprobada, dos copias, margen por entrada y 64 MiB para metadatos;
  consulta el volumen antes de descargar/preparar/publicar. No reserva bloques.
  Staging reservado y limpieza de parciales propios de más de siete días, sin
  borrado recursivo, conservando ajenos, archivos recientes y descargas ocupadas.
- Servicio: D1 migrada y Worker desplegado en
  `https://icaro-invitaciones.maurigajardov.workers.dev`, con configuración remota
  separada y sin contratar un plan. Cloudflare confirmó Free al rechazar el límite
  de CPU personalizado; se retiró esa opción y se conservó el plan gratuito.
  Creación, aceptación, reconexión y ambas revocaciones pasaron con identidades
  efímeras; se detuvo la fixture remota después. El launcher usa ese endpoint bajo
  la bandera experimental y permite sustituirlo/deshabilitarlo por entorno.

Claude Code Opus 5.5 medium colaboró en investigación, pruebas, núcleo de biblioteca
y módulo nativo. Alcanzó su cuota antes de terminar la integración; se continuó con
supervisión y trabajo local sin contratar ni simular otro modelo. La revisión
independiente detectó pérdida de acceso al icono tras recreación y activación de un
pack ausente; ambos casos se corrigieron. Las capturas detectaron texto sin contraste
en la opción elegida; ahora hereda la tinta del botón y las tarjetas tienen el mismo
alto, padding simétrico e iconos centrados.

Evidencias locales en `%TEMP%/icaro-compartir-desarrollo-20261107/`: núcleo completo
431 aprobadas, 36 ignoradas, y 206 UI aprobadas, 4 ignoradas. Además, 24 pruebas del
Worker y recorrido explícito con Worker/D1/iroh. Este último reconstruye el nodo,
conserva ACL/revisión, descarga el mismo JAR y comprueba revocación sin otro cupo.
El fixture compuesto requiere pila de 8 MiB en Windows debug: la de 2 MiB desbordó
al encadenar instalación/sincronización; ahora la configura su propio hilo sin
alterar el launcher. No confundirlo con una validación de pila o gameplay reales.
La prueba nativa de bandeja verifica recurso propio, evento de restaurar, recreación
del icono y cierre del hilo; no reinicia Explorer ni acredita un clic físico.

Verificación definitiva: `core-definitivo.log`, `ui-definitivo.log`,
`worker-rust-definitivo.log` y `build-definitivo.log`, sin fallos. La regresión de
packs ausentes cubre ZIP y carpeta. Capturas finales revisadas:
`08-cierre-entrega-tinta.png` (1280×800) y `08-cierre-entrega-piedra.png` (940×720),
con controles del mismo alto, contraste corregido y pie visible. Se cerró el Worker
local de pruebas; el remoto se mantiene desplegado. `git diff --check` pasa.

Pendiente: prueba completa desde ventana con descarga real de Minecraft y partida,
dos equipos y redes directo/relé, Linux nativo, cortes/disco lleno reales. Quedan
mediciones completas y limpieza de contenido publicado que ya no referencian las
revisiones retenidas.
No se habilitó Compartir por defecto ni se publicó otra versión del launcher.

## Diagnóstico local del 08-11-2026

Se implementó el registro automático descrito en la [guía de diagnóstico](../../diagnostico.md),
con cola acotada, escritor dedicado, rotación, correlación de operaciones, causas
tipadas, aviso de salud y emergencia antes de abortar por panic. Las integraciones
cubren Compartir, preparación/ejecución del juego, autenticación y actualizador;
el contexto se conserva al pasar a trabajo bloqueante. No se registran mensajes
dinámicos, tokens, invitaciones, nombres ni rutas completas. Los registros antiguos
de sincronización filtran sus nuevas entradas sin reescribir el historial.

Claude Code colaboró con `claude-opus-5-5`, esfuerzo `medium`, en implementación
acotada y revisión independiente. Las regresiones reprodujeron pérdida de contexto,
escritura parcial en Windows, eventos pendientes al cerrar y abandono tardío de una
operación antes de corregirlos. Las pruebas usan APPDATA y directorios temporales.
Este bloque se conserva en la misma versión y rama. El cierre de pendientes de
arriba detalla la integración posterior de packs, biblioteca y bandeja.

Verificación final del bloque: **414 pruebas del núcleo** y **175 de UI** aprobadas,
sin fallos; 36 y 4 ignoradas respectivamente. Incluye HTTP local y subproceso que
aborta, no autenticación ni partida real. `APPDATA` estuvo aislado. No se ha hecho
una captura específica del aviso de salud; reutiliza el componente de aviso existente.
La compilación final del ejecutable del launcher terminó sin avisos; formato de
los módulos nuevos y `git diff --check` pasan. Los módulos nuevos del registro se
mantienen separados por responsabilidad (el mayor tiene 417 líneas).

## Continuación del 08-11-2026: Sincronizar y Jugar

Se conectaron el menú «Sincronizar publicación» y la consulta remota antes de Jugar
bajo la bandera experimental. `/v1/reconnect` exige la identidad ya autorizada y el
origen vinculado, sin código ni nuevo cupo. Los cambios se revisan antes de descargar;
los conflictos permiten ambas elecciones y el manifiesto se vuelve a comprobar antes
de aplicar. Jugar vuelve a consultar incluso después del éxito. Sin servicio/anfitrión
o con acceso revocado se muestra error y no se inicia automáticamente una partida.

La guardia de actividad usa el bloqueo del sistema operativo por ID y la misma `Arc`
desde la revisión hasta la preparación y el futuro de Java. La instalación inicial
también lo usa. La prueba local encontró que Windows rechazaba renombrar el destino
recuperado con el bloqueo dentro de `minecraft`; una regresión reprodujo Acceso denegado.
Se trasladó a `.icaro-actividades/<id>` del almacén, fuera de la carpeta renombrable,
y se probó conservar la reserva después de renombrar y recargar el almacén.

Claude Code implementó reconexión y la guardia bajo alcance separado y realizó una
revisión fresca, siempre con `claude-opus-5-5` y esfuerzo `medium`, verificados en sus
respuestas. La revisión encontró que Jugar podía promover una instalación sin consultar,
que cancelar una consulta no esperaba su acuse y que los mods de otra instancia bloqueaban
la consulta. Las tres regresiones fallaron antes de corregirlas: ahora se exige pulsar
Jugar otra vez al terminar una instalación, se espera el acuse al cancelar y se comprueba
el ID ocupado. Los procesos ya ejecutándose conservan su acción Detener.

Una segunda revisión no encontró Critical/Important; se reforzó además el caso
defensivo de una actividad que aparezca después de consultar: no se fusiona con
otro trabajo ni pierde la reserva revisada. Su regresión falló antes del arreglo.
Quedan mejoras menores de diagnóstico cuando una reanudación automática coincide
con otro modal o una actividad cambia al terminar la revisión; fallan de forma segura.

El recorrido explícito Worker/D1/iroh pasó con perfil inyectado: recuperación/renombrado,
revisión intacta, transferencia de la misma reserva a preparación, dos revisiones con
conflictos (una por cada elección), archivo personal intacto y revocación que impide
otra consulta. No acredita una partida ni la descarga real del perfil de Minecraft.
El conflicto de orden de packs sigue fallando con diagnóstico antes de descargar;
su resolución visual, biblioteca, cierre/bandeja y validaciones entre equipos siguen pendientes.

Verificación: **372 pruebas del núcleo** (`--lib --tests`, 36 ignoradas), **175 UI**
(70 biblioteca y 105 launcher, 4 ignoradas) y **24 del Worker**, sin fallos. Además
pasaron explícitamente la interoperabilidad Rust de reconexión y el recorrido del
controlador con Worker/D1/iroh. Compilación del workspace y del launcher pasada.
Las suites usan APPDATA temporal; ninguna prueba acredita gameplay ni dos equipos.

Cuatro capturas finales nativas revisadas: error del controlador en Tinta 1280×800
y Piedra 940×720, conflictos en Tinta 940×720 y Piedra 1280×800. La primera captura
detectó un aviso flotando sobre la lista; el README original de `ErroresCompartir`
exige mostrarlo dentro de progreso/panel. Se añadió el estado de error al progreso,
conservando cabecera, aviso, controles, iconos y pie originales; también lo usa el
instalador inicial. Las capturas finales muestran contenido legible y controles
centrados, sin recorte del pie. Son fixtures, no pruebas manuales entre equipos.
PNG y logs quedan en `%TEMP%/icaro-compartir-desarrollo-20261107/`, sufijo `-final-`.
Formato de módulos afectados y `git diff --check` pasan. El mayor módulo del
controlador tiene 490 líneas; `target` ocupa aproximadamente 4,24 GiB, sin incremental.

## Continuación del 08-11-2026: progreso y recuperación

Se utilizó Claude Code con `--model claude-opus-5-5 --effort medium`, verificado en
las respuestas de implementación y revisión. Su alcance fue el registro persistente
y sus pruebas; la integración, supervisión, correcciones y ejecución de pruebas se
realizaron en esta sesión.

`intentos.rs` identifica únicamente el destino incompleto propio por ID, origen y
perfil. Al reintroducir la invitación tras reiniciar se confirma ese destino antes
de revisar/aplicar. Se puede renombrar conservando el ID. No guarda códigos ni secretos.
`avance.rs` usa `watch` y una salida acotada: contadores del perfil, archivos verificados
y movimientos guardados; porcentaje de la etapa actual, sin un total global estimado.

Verificación: **353 pruebas del núcleo**, **167 UI** (70 biblioteca y 97 launcher),
sin fallos; 27 ignoradas en núcleo y 4 en UI. Recorrido explícito con Worker/D1 e
iroh pasado: cancelar justo después de crear, guardar el intento, fallar el perfil,
abrir otro almacén, recuperar/renombrar el mismo ID, emitir cuatro fases y aplicar
mods/pack conservando un mundo personal. El perfil sigue inyectado en esta prueba.

Se observaron fallos antes de añadir interfaces, admitir el registro propio antes
del vínculo, conservar temporales internos y corregir la cancelación tras crear.
La revisión de Claude detectó ese último fallo; la regresión pasó después de guardar
el intento antes de atender la cancelación. El renombrado se hace después de validar
el registro y con metadatos recién leídos. Se mantienen como límites el rechazo de
registros corruptos, la limpieza posterior de marcadores ya completos y la ventana
de cierre abrupto entre crear y registrar; nunca se adoptan huérfanas por nombre.

Compilación del launcher sin avisos, formato de módulos afectados y `git diff --check`
pasados. Cuatro capturas reales con fixtures: recuperación Tinta 1280×800/Piedra
940×720 y progreso Tinta 940×720/Piedra 1280×800. Iconos, textos y controles centrados;
contenido desplazable y pie visible. El mayor módulo del controlador tiene 476 líneas.

Referencias: [acuerdos](../specs/2026-11-07-sincronizacion-entre-launchers-design.md),
[plan](../plans/2026-11-07-compartir-instancias.md),
[guía técnica](../../compartir-instancias.md) y
[diseños originales](../referencias/compartir-instancia/README.md).

## Estado por etapa

| Etapa del plan | Estado comprobado | Qué falta |
| --- | --- | --- |
| 1. Contratos e inventario | Manifiesto acotado, rutas portables, hashes por bloques, cancelación y diagnóstico de archivos prohibidos. | Inventario incremental de las carpetas nuevas y mediciones; la lectura inicial es completa. |
| 2. Selección y publicación | Nuevos privados, exclusiones, consentimiento por hash, snapshots y pospuestos. Controlador probado con D1 local, archivos reales y selección privada. | Recorrer todas las acciones desde la ventana; precomprobar espacio y limpiar contenido no usado. |
| 3. Comparación y aplicación | Comparación de tres vías y transacción conjunta de archivos, packs, options y base. Respaldos, recuperación y cambios archivo/carpeta probados. | Resolver el conflicto de orden de packs desde la interfaz y ampliar inyección de fallos, incluido disco lleno. |
| 4. Identidad y transporte | Firma Ed25519, keyring sin fallback plano, endpoint iroh fijado a la clave completa, ACL persistente, streaming, reanudación y revocación. | Probar directo/relé entre equipos y redes reales; completar la coordinación con el ciclo de vida del launcher. |
| 5. Invitaciones | Worker/D1 y cliente Rust; caducidad, cupo, aceptación idempotente, revocaciones, renovación y reapertura comprobados localmente. | Configurar y desplegar con cuenta/autorización; validar costes y retención en el entorno real. |
| 6. Coordinación | Invitado experimental con confianza, destino/revisión, aplicación explícita, progreso, recuperación y reserva por ID compartida con preparación/Java. | Validar perfil y partida reales, ampliar cortes/disco lleno. |
| 7. Componentes iced | Quince vistas con acciones tipadas, recursos originales y escenas de captura. Centrado, padding simétrico y celdas uniformes corregidos. | Conectar modelos a datos reales, recorrer todas las acciones y completar foco/teclado/escala. Las capturas usan fixtures. |
| 8. Launcher y segundo plano | Publicador/invitado, Sincronizar/Jugar, conflictos de archivo y reserva de actividad bajo bandera. Coordinador con D1, iroh y perfil inyectado. | Orden de packs, biblioteca general, cierre/bandeja y validación nativa del perfil/partida. |
| 9. Entrega y documentación | Pruebas locales, capturas, compilación del workspace y documentación de estado. | Mediciones, revisión final del recorrido completo y validaciones nativas/entre equipos. No es una entrega terminada. |

## Evidencia anterior a la limpieza

| Comprobación | Resultado | Límite de la evidencia |
| --- | --- | --- |
| Núcleo: lib, actualización local y seis binarios de compartir | 328 pasadas; 27 ignoradas en esa ejecución. | Las ignoradas incluyen servicios externos y ayudantes de corte; estos últimos son invocados por las pruebas de recuperación. |
| UI: lib y ejemplo launcher, secuencial | 70 + 74 pasadas; 3 ignoradas. | Comprueba lógica y construcción de widgets; no todos los recorridos reales de la nueva función. |
| Worker/D1 mediante Miniflare | 17 pasadas. | D1 local; no hay despliegue remoto. |
| Rust → Worker/D1 local → iroh → aplicación | 1 prueba explícita ignorada ejecutada y pasada. | Aceptación, configuración/pack externo, conservación de otras opciones y revocación. No acredita una partida, dos equipos ni todo el ciclo de actualización. |
| Compilación del ejemplo launcher y `git diff --check` | Correctos. | No equivale a `cargo build --workspace` ni a CI remoto. |
| Capturas reales | Tinta/Piedra a 1280×800 y publicación a 940×720. | Publicación con privados/pospuestos, conflicto, cupo, cierre y error; datos de prueba. |

Comandos reproducibles y escenas en [la guía](../../compartir-instancias.md#validación-local).
Los logs y capturas de esta sesión están en `target/desarrollo-compartir/`, que está
ignorado y puede desaparecer al limpiar `target`. Las capturas finales del resumen
y panel llevan `-uniforme-`; las de cierre, `-final-`. No son assets del producto.
El Worker local se detuvo al acabar las pruebas.

## Continuación tras cargo clean

El usuario solicitó limpiar y continuar. `cargo clean` eliminó **16.863 archivos,
9,1 GiB**. Antes se copiaron registro, logs y últimas capturas a
`%TEMP%/icaro-compartir-desarrollo-20261107/`. La validación posterior usa esa carpeta,
`APPDATA` temporal separado y `CARGO_INCREMENTAL=0`; no se cambió Cargo globalmente.

Se añadió `launcher_aux/compartir_real/` con módulos de estado, tareas, resultados,
invitaciones, modelos, vistas y pruebas. Solo se habilita mediante
`ICARO_COMPARTIR_DESARROLLO=1`; el modo demo no ejecuta servicios. Su alcance y
configuración están en [la guía](../../compartir-instancias.md#controlador-experimental-del-publicador).

Las exclusiones se guardan antes de mostrar la elección como lista; un fallo conserva
el borrador. La confirmación de propios no se reutiliza entre revisiones nuevas.
Las respuestas tardías se descartan; cancelar durante preparación espera su resultado
y respeta la fase atómica. Panel/publicación conservan datos propios separados del
borrador de otra instancia. Escape cierra el controlador y el modal bloquea navegación.

Verificación posterior: **70 pruebas UI de biblioteca + 88 del launcher pasadas**,
3 ignoradas; incluyen **14 del controlador**. Las 40 pruebas de compartir del núcleo
pasaron de nuevo, con el ayudante ignorado habitual. La revisión independiente encontró
dos problemas importantes —cancelación visible ignorada y mezcla entre publicación
detenida/borrador distinto—; ambos se reprodujeron con pruebas fallidas y se corrigieron.
Se cubrió además el cierre que esperaba una reapertura fallida por red. Esto no
valida todavía publicar/reabrir desde la UI contra el Worker real local.

Las escenas `compartir-controlador` y `compartir-controlador-angosto` recorren la capa
modal registrada en el launcher con fixtures, sin identidad real ni llamadas de red.
La primera captura detectó alineación incorrecta de la capa; se acotaron sus dimensiones
para centrarla y mantener la barra de ventana y los botones visibles.
La compilación final y las cuatro capturas corregidas pasaron (Tinta/Piedra y
1280×800/940×720), con sufijo `-centrado-` en la carpeta temporal. `git diff --check`
pasó. Tras estas verificaciones, `target` ocupa aproximadamente **3,19 GiB**;
contiene los artefactos recién compilados, sin caché incremental de esta ejecución.

Regresiones corregidas: contenido alterado después de preparar, recuperación tras
cortes, transición archivo/carpeta, colisión inicial con el pack vanilla, pie de
diálogo fuera de pantalla y dimensiones de invitación/resumen incompatibles con
un área desplazable. Las dos últimas cuentan con pruebas de construcción que
fallaron antes de corregirse. Las celdas de resumen comparten altura y espacios
para rótulo, valor y descripción; los botones no usan desplazamientos manuales.

## Continuación: publicador y servicio local

Claude Code implementó un bloque acotado bajo supervisión; se revisó el diff y se
ejecutaron las pruebas desde esta sesión. La configuración del servicio y la
identidad son inyectables, sin añadir dependencias ni cambiar el diseño. Los
futuros usados por las pruebas son los mismos del publicador. La reapertura cierra
explícitamente el nodo nuevo si falla la operación remota.

Verificación posterior al último push: **159 pruebas UI** (70 de biblioteca y
89 del launcher; 4 ignoradas), **17 del Worker**, y dos recorridos explícitos con
Worker/D1 local: el del núcleo y el del controlador del publicador. Este último
comprueba selección privada, consentimiento, contenido y pack externo, conservación
de `options.txt`, código del panel, aceptación, ajuste a 48 horas/cupo 3 sin nueva
revisión, detención y reapertura con el mismo origen y nueva aceptación.

Se observó el fallo de compilación de las pruebas antes de añadir sus interfaces;
después pasaron contra el servicio real local. La revisión independiente no encontró
regresiones. Esto no acredita clics en la ventana, instalación del invitado, relés,
dos equipos ni partida. Este bloque se registra y sube a `v0.4.x` por solicitud
expresa del usuario, sin versión nueva ni despliegue.

## Qué sigue, en orden

### Continuación posterior al commit `01a55ed`

El bloque del publicador se subió a `origin/v0.4.x` en `01a55ed`, con autorización
expresa. El trabajo posterior del invitado se registra y sube también a `v0.4.x`
por la autorización posterior del usuario, sin integración en canales ni release.
Claude Code implementó confianza/huellas bajo alcance limitado; se revisó y añadió
bloqueo de archivo entre procesos. El controlador y coordinador del invitado están
separados en módulos de tareas, transiciones, instalación, vistas y escenas.

Validación posterior: **342 pruebas del núcleo**, **164 UI** (70 biblioteca y 94
launcher), sin fallos; 27 ignoradas en núcleo y 4 en UI. El recorrido explícito con
Worker/D1 e iroh pasó con perfil inyectado: resolver sin destino, confirmar confianza,
rechazar instalación sin confianza, conservar destino tras fallo del perfil,
reintentar sin duplicarlo y aplicar mods/pack dejando intacto un mundo personal.
También comprueba que el lanzamiento de un vínculo sin base aplicada se rechaza.

La revisión independiente detectó navegación Atrás, revisión nueva sin salida y
pérdida del ID/perfil del destino al fallar o pegar otro código. Se corrigieron y
añadieron regresiones; Atrás, conservación de ID, falta de confianza y bloqueo de
otro proceso tuvieron fallo observado antes del arreglo. La revisión final no
encontró Critical/Important pendientes. No acredita instalar un perfil real desde
esta ruta, jugar, dos equipos o relés. Las capturas siguen usando fixtures.

Compilación del launcher sin avisos y cuatro capturas nuevas pasadas: huella Tinta
1280×800, huella Piedra 940×720, destino Tinta 940×720 y revisión Piedra 1280×800.
Se revisaron centrado, alturas de controles, desplazamiento interno y pie visible.
Escenas `compartir-controlador-invitado-*`; PNG y logs en la carpeta temporal de
evidencias. El mayor módulo del controlador tiene 469 líneas. Formato de los módulos
nuevos y `git diff --check` pasan. No se recompiló ni consultó CI remoto para una release.

Antes de registrar cada commit se aislaron los cambios restantes mediante un
resguardo temporal. La validación del índice pasó: 328 pruebas del núcleo, 17 del
Worker, 158 de UI, compilación del launcher y `cargo build --locked -j 2 --workspace`.
Los resguardos de esta operación se restauran sin incluir caches, SQLite local,
datos de instancia ni credenciales. El código sigue siendo experimental y las
validaciones externas enumeradas abajo continúan pendientes.

1. Completar el recorrido del publicador desde la ventana, diagnóstico,
   actualización de estados/contadores y biblioteca de
   publicaciones. Sus módulos, tareas, consentimiento y guardado de privados ya existen.
2. Validar la instalación exacta conectada al invitado con el perfil real y ampliar
   cortes/disco lleno. Progreso por etapa y recuperación del destino al reintroducir
   la invitación tras reiniciar ya están conectados y probados localmente.
3. Validar Sincronizar/Jugar durante una partida real y resolver visualmente el orden
   de packs. La consulta, confirmación, conflictos de archivo y reserva ya están conectados.
4. Integrar accesos, reconexión, minimizar y cierre. La bandeja debe poder restaurar
   Ícaro; esperar transferencias debe rechazar nuevas y cerrar al terminar.
5. Completar espacio, limpieza, caché/concurrencia y mediciones; repetir pruebas y
   capturas tras integrar. Hacer revisión final del conjunto.
6. Preparar despliegue gratuito y validar con dos equipos, redes, Linux y partida
   real. No cambiar canales, versión ni publicar por inferencia.

Mantener módulos por responsabilidad y evitar ampliar los archivos grandes del
launcher con la lógica nueva. La regla de centrado y medidas quedó en `AGENTS.md`.
