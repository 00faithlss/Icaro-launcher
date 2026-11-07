# Compartir instancias: implementación en desarrollo

Actualizado el **07-11-2026**. El [registro de progreso](superpowers/progreso/2026-11-07-compartir-instancias.md)
enumera etapas, resultados de pruebas y el orden del trabajo siguiente.

Trabajo de `v0.4.x`, sin publicación del launcher. Existe un controlador experimental
del publicador, deshabilitado por defecto. La aceptación del invitado, instalación,
Sincronizar/Jugar y bandeja todavía no completan el recorrido normal. Las escenas
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

Para desarrollo explícito, `ICARO_COMPARTIR_DESARROLLO=1` muestra los accesos en
Servidores. `ICARO_SERVICIO_INVITACIONES` debe contener una URL HTTPS válida o
loopback HTTP; sin servicio configurado, la revisión no se publica. Con Worker
local, `ICARO_COMPARTIR_LOCAL=1` usa iroh sin relés y solo loopback. El modo demo
no ejecuta estos servicios. Usar una raíz de pruebas propia; no instancias reales.

El controlador permite revisar el inventario y el catálogo, elegir contenido,
guardar exclusiones mediante tareas y confirmar archivos propios antes de publicar.
La publicación y la generación de invitación son pasos distintos: si el transporte
falla después del guardado, la interfaz conserva la revisión como detenida. Permite
generar/editar/revocar códigos, revisar/revocar accesos y detener/reabrir la publicación.
La edición conserva los ajustes confirmados si el servicio falla.

Es un bloque experimental, no una función entregada: falta validar sus operaciones
de red desde la interfaz con el servicio local, actualizar automáticamente contadores
y disponibilidad, y completar el recorrido del invitado/Jugar/cierre del launcher.
El botón de aceptar indica esa limitación. Se admite un anfitrión activo en este
controlador; el panel no es todavía una biblioteca persistente de publicaciones.

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

Escenas: `compartir-publicacion`, `compartir-privados`, `compartir-retirada`,
`compartir-packs`, `compartir-conflicto`, `compartir-invitacion`, `compartir-cupo`,
`compartir-cierre`, `compartir-error`, `compartir-propios` y `compartir-destino`.
El sufijo `-angosto` activa la ventana pequeña. Usar `ICARO_DEMO=1`,
`ICARO_TEMA_DISENO=tinta|piedra`, `ICARO_VISTA_DISENO` y `ICARO_CAPTURA_DISENO`.
Estas escenas no realizan solicitudes ni crean identidades en el almacén seguro.

## Pendiente antes de habilitarlo

- Completar y validar el controlador experimental; añadir aceptación, confianza
  persistente y biblioteca de publicaciones. Las tareas, la revisión del anfitrión,
  el consentimiento y la persistencia de privados ya tienen implementación inicial.
- Conectar la comprobación al pulsar Jugar, instalación exacta y bloqueo durante
  la partida; gestión de cierre, bandeja restaurable y detención con transferencias.
- Exponer conflictos de orden de packs como una decisión revisable en la interfaz.
- Acotar concurrencia y caché de descargas del nuevo coordinador, limpieza del
  staging/CAS y precomprobación de espacio. Medir los recorridos completos.
- Configurar y desplegar el servicio gratuito; validar entre dos equipos, redes,
  relé, Windows/Linux y una partida real. Las pruebas locales no acreditan eso.
