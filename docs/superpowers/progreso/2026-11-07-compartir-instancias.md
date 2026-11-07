# Progreso de Compartir instancias — 07-11-2026

Rama de trabajo comprobada: `v0.4.x`. Versión del workspace: `0.3.4`, sin bump.
El plan original está en `d78e628`. La implementación se registra en `d48181f`
(núcleo y servicio) y `f4fbf08` (interfaz experimental), por solicitud del usuario
de subir todo el bloque a `v0.4.x`. Este registro acompaña esos commits y su
documentación; no supone una release ni una función habilitada por defecto.
No se revalidaron releases ni CI remotos.

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
| 6. Coordinación | Origen estable, instancia nueva con loader fijado, autorización del anfitrión y preparación remota sin aplicación automática. | Instalación del perfil, confianza persistente, exclusión entre partida/aplicación, reintentos y eventos de progreso del recorrido completo. |
| 7. Componentes iced | Quince vistas con acciones tipadas, recursos originales y escenas de captura. Centrado, padding simétrico y celdas uniformes corregidos. | Conectar modelos a datos reales, recorrer todas las acciones y completar foco/teclado/escala. Las capturas usan fixtures. |
| 8. Launcher y segundo plano | Controlador experimental del publicador registrado en Servidores, deshabilitado por defecto; publicación/código y futuros de edición/reapertura probados con D1 local. | Aceptación/instalación del invitado, Sincronizar/Jugar, biblioteca persistente de publicaciones, cierre/bandeja y recorrido completo de ventana. |
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

Antes de registrar cada commit se aislaron los cambios restantes mediante un
resguardo temporal. La validación del índice pasó: 328 pruebas del núcleo, 17 del
Worker, 158 de UI, compilación del launcher y `cargo build --locked -j 2 --workspace`.
Los resguardos de esta operación se restauran sin incluir caches, SQLite local,
datos de instancia ni credenciales. El código sigue siendo experimental y las
validaciones externas enumeradas abajo continúan pendientes.

1. Completar el recorrido del publicador desde la ventana, diagnóstico,
   actualización de estados/contadores y biblioteca de
   publicaciones. Sus módulos, tareas, consentimiento y guardado de privados ya existen.
2. Conectar invitación, confianza del anfitrión y destino nuevo; instalar las
   versiones exactas sin añadir mods ajenos al manifiesto. Conservar un destino
   incompleto recuperable si falla la instalación.
3. Conectar Sincronizar y la comprobación previa a Jugar. Revisar y confirmar antes
   de descargar/aplicar; resolver conflictos y bloquear la carrera con una partida.
4. Integrar accesos, reconexión, minimizar y cierre. La bandeja debe poder restaurar
   Ícaro; esperar transferencias debe rechazar nuevas y cerrar al terminar.
5. Completar espacio, limpieza, caché/concurrencia y mediciones; repetir pruebas y
   capturas tras integrar. Hacer revisión final del conjunto.
6. Preparar despliegue gratuito y validar con dos equipos, redes, Linux y partida
   real. No cambiar canales, versión ni publicar por inferencia.

Mantener módulos por responsabilidad y evitar ampliar los archivos grandes del
launcher con la lógica nueva. La regla de centrado y medidas quedó en `AGENTS.md`.
