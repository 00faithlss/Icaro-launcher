# Compartir instancias: implementación en desarrollo

Actualizado el **08-11-2026**. El [registro de progreso](superpowers/progreso/2026-11-07-compartir-instancias.md)
enumera etapas, resultados de pruebas y el orden del trabajo siguiente.

Trabajo de `v0.4.x`, sin publicación del launcher. Existen controladores experimentales
del publicador y del invitado, con activación explícita desde Servidores. La aceptación e instalación,
Sincronizar y la consulta previa a Jugar ya están conectados en modo experimental. El perfil
real y las validaciones entre equipos todavía no completan el recorrido normal. Las escenas
de diseño son fixtures y no habilitan una publicación.

## Módulos

- `crates/core/src/compartir/`: inventario, selección privada, revisiones por
  contenido, firmas, invitaciones, transporte iroh, vínculos y aplicación recuperable.
- `crates/ui/src/compartir/`: componentes del diseño original, sin disco ni red.
- `crates/ui/examples/launcher_aux/compartir_diseno.rs`: escenas aisladas con
  los widgets reales. Se capturan mediante el mecanismo existente del launcher.
- `services/invitaciones/`: Worker y migraciones D1. Sus instrucciones locales,
  límites y preparación de despliegue están en su `README.md`.

El controlador está en `launcher_aux/compartir_real/`, separado en estado, tareas,
acciones, resultados, invitaciones, modelos y vistas. El archivo existente del
launcher solo registra mensajes, la capa modal y los bloqueos de navegación.

## Controlador experimental del publicador

En **Servidores**, pulsa **Activar Compartir experimental**. La elección se guarda
en este equipo y aparecen **Compartir instancia** y **Aceptar invitación**. La
primera abre la revisión de una instancia y, al publicarla, permite generar el
código. La segunda abre la entrada del código en el equipo invitado.

`ICARO_COMPARTIR_DESARROLLO=1` permite activar el controlador desde la consola
para pruebas. Usa el endpoint gratuito desplegado de invitaciones. La variable
`ICARO_SERVICIO_INVITACIONES` permite sustituirlo por HTTPS o loopback HTTP;
un valor vacío lo deshabilita y no permite publicar. Con Worker
local, `ICARO_COMPARTIR_LOCAL=1` usa iroh sin relés y solo loopback. El modo demo
no ejecuta estos servicios. Usar una raíz de pruebas propia; no instancias reales.

El controlador permite revisar el inventario y el catálogo, elegir contenido,
guardar exclusiones mediante tareas y confirmar archivos propios antes de publicar.
En **Revisar publicación**, **Seleccionar todos los mods** marca los JAR elegibles.
Cada mod se puede ajustar por separado y los JAR propios siguen pidiendo confirmación.
La publicación y la generación de invitación son pasos distintos: si el transporte
falla después del guardado, la interfaz conserva la revisión como detenida. Permite
generar/editar/revocar códigos, revisar/revocar accesos y detener/reabrir la publicación.
La edición conserva los ajustes confirmados si el servicio falla.

El controlador tiene una prueba contra Worker/D1 local con inventario y contenido
reales: publicación seleccionada, privados, consentimiento, pack externo y código
mostrado en el panel. También comprueba aceptación, edición del mismo código sin
otra revisión y reapertura con nueva aceptación. Usa los futuros y reductores del
publicador; no automatiza clics ni el recorrido completo de `App`.

Es un bloque experimental: falta recorrer sus operaciones completas desde la
ventana y con una partida real. La biblioteca recupera publicaciones propias ya
persistidas desde Servidores, sin crear otra identidad/origen. Solo hay un anfitrión
activo a la vez. Reconectar conserva los accesos; «Volver a compartir» tras detener
exige otra aceptación. Los contadores se consultan cada minuto con tareas
cancelables y conservan sus datos ante errores de red.

Cerrar ofrece esperar o cerrar ahora. Esperar rechaza nuevas transferencias,
drena las admitidas y después cierra el transporte conservando accesos. La bandeja
Windows usa el icono propio y permite restaurar o salir; si falla, minimiza sin
ocultar una ventana irrecuperable. En Linux se usa ese fallback visible y queda
pendiente la bandeja nativa. Las operaciones de instalación/publicación esperan su
acuse antes del cierre para conservar la recuperación.

Antes de descargar o preparar se calcula el presupuesto de disco (dos copias de
los archivos, margen por entrada y 64 MiB para diario/opciones/manifiesto) y se
consulta el volumen. Es una estimación, no una reserva: otro proceso puede consumir
espacio después. El staging queda reservado; se limpian parciales propios de más
de siete días sin borrar recursivamente, ignorando entradas ajenas y locks activos.
Se conservan parciales recientes para reanudarlos y los snapshots publicados.

## Invitado experimental

La entrada valida el código contra el servicio sin reservar cupo. Pulsar Continuar
registra la aceptación y consulta únicamente el manifiesto firmado, sin descargar
mods ni crear destinos. Esto puede consumir un cupo aunque después se cancele la
pantalla de huella; reconectar con la misma identidad no consume otro.

La pantalla original muestra huella y sello de la clave completa. La confianza se
guarda atómicamente en `compartir/confianza.json`, acotada a 1000 publicaciones y
256 KiB, con bloqueo entre procesos. Un archivo corrupto es un error; otra clave
requiere confirmación explícita. No se guardan claves privadas en ese archivo.

Después se elige el nombre y se revisan los archivos. Solo Sincronizar crea el
destino nuevo, instala el perfil exacto con el servicio existente y descarga/aplica
la revisión verificada. Las instancias vinculadas no reciben Fabric API sugerida.
Si el perfil o los metadatos fallan, se retiene el ID para reintentar. Tras cerrar o
reiniciar, volver a introducir la invitación y confirmar la huella recupera el destino
incompleto propio. La pantalla lo identifica como «Continuar instalación» antes de
revisar/aplicar; permite conservarlo o renombrarlo sin cambiar su identidad.
Una publicación cambiada exige revisar otra vez. Cambiar de origen o perfil propone
otro destino y conserva el anterior. No se adopta una instancia personal existente.

El lanzamiento normal rechaza un vínculo sin base aplicada coherente, fuera del hilo
de la interfaz. Bajo la bandera experimental también consulta la revisión remota
antes de Jugar y comparte una reserva de actividad con instalación y sincronización.
La prueba de este coordinador inyecta la instalación del perfil: los archivos,
transporte, hashes, packs y transacción son reales; no acredita descargar Minecraft
ni jugar desde esta ruta nueva.

## Sincronizar y comprobación antes de Jugar

Con la bandera experimental, el menú de instancias ofrece «Sincronizar publicación».
Una instancia sin vínculo se informa como tal; nunca se adopta por nombre. Un destino
vinculado usa `/v1/reconnect` con su identidad autorizada y origen persistido, sin
reintroducir el código ni consumir otro cupo. Caducar o revocar el código no elimina
ese acceso; revocar a la persona o detener la publicación sí impide la reconexión.

La consulta verifica la firma, el perfil exacto y la revisión, inventaría los archivos
y muestra el componente original de revisión. No descarga contenido ni aplica el plan.
Los conflictos de archivo permiten cambiar entre «Del anfitrión» y «Conservar la mía»
antes de confirmar. Sincronizar vuelve a consultar el manifiesto antes de descargar;
si cambió, exige revisar otra vez. Se conservan los archivos personales.

Jugar consulta de nuevo incluso después de una sincronización exitosa. Si la revisión
está al día continúa la preparación sin descargar contenido de la publicación; si hay
cambios pide confirmación. Si el anfitrión o el servicio no están disponibles, muestra
un error y permite reintentar: no inicia automáticamente una partida sin comprobarla.
Los cambios simultáneos de selección/orden de packs aparecen como otra fila de
conflicto en la revisión original: del anfitrión o conservar la mía. Ambas conservan
el orden relativo de los packs personales; conservar la selección local no activa
automáticamente los nuevos. Los retirados dejan de estar activos. La selección local
revisada se vuelve a comprobar antes de descargar y preparar; el diario protege el
resto de `options.txt` frente a cambios externos. Conservar un archivo ausente no
introduce una activación nueva que lo requiera.

`actividad_instancia::Guardia` usa un bloqueo del sistema operativo por ID en
`instances/.icaro-actividades/<id>/.icaro-actividad.lock`. Está fuera de la carpeta
renombrable de cada instancia: la recuperación puede cambiar su nombre sin liberar
la reserva. Una misma `Arc` pasa desde la revisión a la preparación y al futuro que
ejecuta Java. Otra preparación o sincronización del mismo ID se rechaza. Soltar la
última referencia o terminar el proceso libera el bloqueo; no depende de PID ni de
borrar un archivo. Esto coordina esos recorridos del launcher, no herramientas externas.

## Progreso y recuperación del instalador

`avance.rs` recibe los contadores del perfil de Minecraft, archivos completamente
descargados/verificados y movimientos durables de aplicación. El porcentaje corresponde
a la etapa actual; sin total conocido no se estima un porcentaje global. En descargas
se cuenta por archivo, sin simular velocidad o progreso por bytes. El canal `watch`
conserva solo el último avance y la salida a iced está acotada. Las respuestas se
filtran por solicitud y se descartan al cerrar o cancelar el intento.

`intentos.rs` guarda `.icaro-compartir-instalacion.json` antes de instalar el perfil:
esquema, ID local, origen, perfil y revisión revisada, sin códigos, URLs ni secretos.
La búsqueda usa el almacén existente y rechaza informes de más de 1000 instancias;
no es todavía un inventario incremental. Cada registro tiene un límite de 16 KiB.
Un registro corrupto, ajeno o varios candidatos se diagnostican; no se crea otra copia
silenciosamente ni se adopta una instancia personal por nombre.

La cancelación posterior a crear el destino guarda primero el intento recuperable.
Los temporales internos de una escritura interrumpida se conservan y solo se admiten
al preparar un destino con registro propio válido. La limpieza del registro exige
vínculo correcto y una base aplicada exactamente igual al manifiesto. Si la base ya
se aplicó y quedó el registro, no se ofrece la instancia completa como incompleta.
Sigue existiendo una ventana de cierre abrupto entre crear y registrar: una instancia
huérfana sin registro se conserva y no se adopta automáticamente. La biblioteca
del anfitrión ya recupera publicaciones propias; la recuperación de intentos no
constituye una biblioteca general de instalaciones del invitado.

## Recorrido comprobado localmente

Un anfitrión publica una revisión seleccionada, firma su manifiesto y sirve su
contenido mediante iroh. La aceptación de un código se registra en D1. Antes de
admitir la conexión, el anfitrión consulta las autorizaciones con su propia
identidad; el código o una afirmación del invitado no sustituyen esa comprobación.

La conexión comprueba la clave completa del endpoint y la firma de la revisión.
`vinculos::crear_destino` siempre crea otra instancia y fija la versión exacta del
loader. Crear el vínculo no marca la revisión como aplicada.

`descargas::preparar` descarga únicamente los archivos del plan revisado, fuera
de la instancia. Los archivos Modrinth se buscan por la versión publicada y se
comparan por tamaño/hash; el contenido propio se obtiene del anfitrión. Devuelve
una preparación; el llamante tiene que ejecutar `aplicacion::aplicar` por separado.
Esa transacción incluye archivos, selección de packs y base aplicada, conserva
respaldos y recupera cortes entre movimientos. No copia todo el directorio `mods`.

La prueba real local cubre configuración y un pack externo, conserva las demás
opciones de Minecraft y verifica que revocar al invitado corta su conexión.
Los packs integrados no se descargan por separado: viajan dentro del JAR seleccionado.

## Accesos y reapertura

El código puede durar 24 horas, 2 días o 1 semana, con cupo configurable. Otra
invitación invalida el código anterior y conserva los accesos ya aceptados.
Revocar el código y revocar una identidad son operaciones distintas.

Cerrar el transporte conserva autorizaciones. Detener la publicación guarda
primero el bloqueo local de todos los accesos, cierra conexiones y solicita la
revocación remota. Si falla la red, reiniciar no restaura esos permisos.

Para volver a compartir, iniciar un nodo nuevo para el mismo origen y usar
`PublicacionActiva::volver_a_compartir`. Esta operación reintenta detener en el
servicio antes de generar otra invitación y limpiar los permisos locales. Así,
un cierre anterior sin red no puede restaurar a los antiguos invitados. Estos
deben aceptar la invitación nueva.

## Validación local

Ejecutar las pruebas Rust con `APPDATA` en una carpeta temporal propia y `-j 2`.
Las pruebas de corte crean procesos auxiliares y usan sandboxes temporales.

```powershell
$env:APPDATA = "$PWD/target/desarrollo-compartir/appdata"
cargo test --locked -j 2 -p icaro-core --lib --test actualizacion_local `
  --test compartir_instancias --test compartir_estructura --test compartir_autoridad `
  --test compartir_invitaciones --test compartir_transporte --test compartir_vinculos
cargo test --locked -j 2 -p icaro-ui --lib --example launcher -- --test-threads=1
```

La prueba `rust_interopera_con_worker_d1_local` requiere levantar explícitamente
el Worker local y establecer `ICARO_INVITACIONES_TEST_URL=http://127.0.0.1:8787`.
Se ejecuta con `--test compartir_invitaciones -- --ignored`. No utiliza producción.
No se configura una URL pública inexistente ni se despliega sin cuenta/autorización.

Con ese mismo Worker levantado, comprobar el controlador del publicador:

```powershell
$env:CARGO_INCREMENTAL = '0'
$env:ICARO_PRUEBA_WORKER_URL = 'http://127.0.0.1:8787'
cargo test --locked -j 2 -p icaro-ui --example launcher publicador_real_publica_seleccion -- --ignored --test-threads=1
```

La prueba exige loopback, crea identidades en memoria y una instancia temporal;
no usa el keyring ni datos personales. Detiene los nodos antes de limpiar sus
archivos. Las pruebas normales comprueban además que una configuración ausente o
inválida rechaza publicar antes de escribir la selección o la revisión.

Escenas: `compartir-publicacion`, `compartir-privados`, `compartir-retirada`,
`compartir-packs`, `compartir-conflicto`, `compartir-invitacion`, `compartir-cupo`,
`compartir-cierre`, `compartir-error`, `compartir-propios` y `compartir-destino`.
El sufijo `-angosto` activa la ventana pequeña. Usar `ICARO_DEMO=1`,
`ICARO_TEMA_DISENO=tinta|piedra`, `ICARO_VISTA_DISENO` y `ICARO_CAPTURA_DISENO`.
Estas escenas no realizan solicitudes ni crean identidades en el almacén seguro.

## Caché y descargas

Instalar o sincronizar reutiliza archivos de `compartir/cache-contenido` bajo la
raíz de datos del launcher. Se comprueban el tamaño y SHA-512 de cada objeto y se
conservan copias independientes en las instancias. Si se desconoce la procedencia
de Modrinth, se consulta la versión exacta. La autorización y la revisión se
consultan antes de preparar los archivos.

El coordinador descarga hasta dos archivos del anfitrión y hasta cuatro del CDN
en paralelo, con un máximo de dos por host del CDN. El contenido propio admite
reanudar un parcial verificado al finalizar. La limpieza intenta mantener hasta
1 GiB y retirar objetos sin uso tras siete días. Respeta objetos y parciales en
uso. Los límites de red, escritura y espera mantienen acotado el trabajo simultáneo.

El [registro de rendimiento](investigaciones/rendimiento-compartir-2026-11-08.md)
documenta la fixture local y lo que queda por medir.

## Pendiente antes de habilitarlo

- Validar el controlador experimental, aceptación, confianza
  e instalación en pruebas nativas reales. La aceptación,
  confianza persistente, destino/revisión y coordinador de instalación ya están conectados
  bajo bandera; progreso por etapa y recuperación al reintroducir la invitación ya
  están conectados. Falta validar el perfil descargado y ampliar cortes/disco lleno.
- Validar la comprobación al pulsar Jugar y la reserva durante una partida real,
  cierre con transferencias entre equipos y el reinicio real de Explorer.
- Completar bandeja nativa Linux; actualmente se conserva la ventana minimizada.
- Medir recorridos completos en release y en redes reales. La limpieza de la caché
  del invitado y la limpieza de staging están conectadas. Queda la recolección de
  contenido publicado que ya no referencian las revisiones retenidas.
- Validar el servicio desplegado entre dos equipos, redes, relé, Windows/Linux y
  una partida real. Las pruebas remotas HTTP no acreditan esos recorridos.
