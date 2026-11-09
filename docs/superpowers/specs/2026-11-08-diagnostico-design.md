# Registros de diagnóstico locales

Petición del usuario del 08-11-2026: seguir desarrollando con Claude y añadir logs
que permitan saber qué falló. Se mantienen su delegación de decisiones técnicas,
el diseño existente y el trabajo local en `v0.4.x`; no se autoriza commit/push.

## Enfoque y alcance

Se amplía el registro existente con un canal de diagnóstico estructurado local.
No requiere cuentas, servidores, telemetría ni planes de pago. Un servicio remoto
añadiría costes y exposición de datos sin ayudar a reproducir los fallos locales.
Escribir desde cada callback mantendría las carreras y bloquearía iced; se usa un
escritor dedicado, sin nuevas dependencias, y emisores que solo encolan en memoria.
Estas decisiones son técnicas del implementador dentro del trabajo delegado.

Cada línea JSON contiene esquema, fecha UTC, versión/compilación, sesión aleatoria,
secuencia, operación, componente, etapa, duración y resultado. Las operaciones tienen
un identificador estable desde inicio a fin; se registran etapas y cantidades reales,
no cada frame ni cada bloque de descarga. Un futuro descartado deja una operación
abandonada. Una cancelación confirmada se distingue de un fallo y de abandono.

La primera integración cubre arranque/cierre, Compartir (publicar, invitaciones,
consulta, instalación, descargas/aplicación), preparación/ejecución de Minecraft,
y resultados de cuenta/actualizador. El registro antiguo de sincronización sigue
disponible; se revisa su exposición de rutas, direcciones y texto libre. La cobertura
exacta de la entrega se documentará: no se afirmará instrumentar todos los servicios.

## Privacidad y causas

La API acepta etiquetas de código estáticas, contadores y causas tipadas; no acepta
mensajes dinámicos arbitrarios, `Debug` de acciones ni cuerpos HTTP. No se escriben
tokens, códigos de invitación, cuentas, nombres de instancia/archivo, direcciones,
URLs, claves ni rutas absolutas. Una instancia se identifica por alias derivado de
su ID y la sesión; no se registra el ID de cuenta. La ubicación del código conserva
solo el nombre del archivo Rust y su línea. Los códigos HTTP y del sistema operativo
y las categorías timeout/conexión/integridad/permiso ayudan a distinguir las causas.
Los códigos de error remotos pasan por una lista de etiquetas conocidas, nunca se
copian directamente. Donde el código existente ya perdió la causa se documenta el
límite y se añaden puntos de captura antes de esa conversión en recorridos críticos.

## Persistencia y fallos del propio registro

`logs/diagnostico.jsonl` rota antes de superar 1 MiB, con tres respaldos. Un bloqueo
del sistema operativo serializa escritura/rotación entre procesos. La cola es acotada
y `try_send` no espera: si se satura, se contabilizan eventos descartados y se deja
evidencia al recuperarse. El estado de salud expone fallos de escritura y descartes;
no se interpreta una escritura fallida como éxito. Los archivos/enlaces no regulares
se rechazan. Un error del logger no interrumpe una instalación o sincronización.

El cierre pide vaciar y terminar con espera limitada. El hilo escritor no mantiene
la ventana bloqueada esperando disco/red; las consultas explícitas de vaciado se
ejecutan fuera de update/view. No se promete persistencia de lo que siga en memoria
ante kill, corte de luz, disco lleno o fallo del sistema de archivos.

El hook de panic registra ubicación y categoría, sin payload ni backtrace con rutas.
Como release usa `panic=abort`, dispone de escritura de emergencia independiente
en `logs/panico.jsonl`, acotada y con bloqueo no bloqueante. Esta es una excepción
fatal a la emisión normal por cola: no puede depender de que iced/Tokio sigan vivos.
Es un intento de último recurso; no captura aborts externos ni falta de memoria.

## Comprobación

Las pruebas usan raíces temporales y APPDATA aislado. Deben probar JSON válido con
productores concurrentes, etapas/duración/correlación, abandono/cancelación, cola
saturada con contabilidad, rotación y fallo de escritura visibles, archivos no regulares,
cierre con plazo y un subproceso cuyo hook previo aborta inmediatamente. Una prueba
de fallo real de servicio debe dejar código HTTP/etapa sin código de invitación ni URL.
La integración no cambia iconos ni añade una pantalla de diseño inventada. Los logs
se encuentran en la carpeta de datos; las instrucciones explican cómo leerlos y
cuáles son sus límites. Perfil/partida y redes reales siguen siendo otras validaciones.
