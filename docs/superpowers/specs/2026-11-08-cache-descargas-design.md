# Caché y descargas de Compartir

## Encargo y decisiones

El usuario pide investigar y optimizar caché/descargas, medir y ejecutar la
implementación de corrido. Continúa la delegación de decisiones técnicas;
se conserva Rust, una carpeta y `v0.4.x`, sin commit/push ni cambios visuales.

Se consideraron tres enfoques: aumentar solo concurrencia (no evita repetir
archivos), usar únicamente CAS (no acelera la primera descarga) y combinar CAS
verificado con concurrencia por origen. Se elige el tercero como decisión de
implementación: ataca ambos costes sin depender de un servicio de pago.

## Contrato

- CAS explícito en `raiz/compartir/cache-contenido`, compartido entre instancias.
  Objetos por SHA-512 y tamaño, publicación mediante parcial verificado y rename
  en el mismo volumen; sin enlaces duros. La transacción sigue creando una copia
  independiente y verificando su hash antes de aplicar.
- Buscar objetos antes de resolver el catálogo. Para Modrinth, saltar API solo
  cuando exista confirmación persistida exacta de proyecto/versión/hash/tamaño.
  Reutilizar bytes propios no acredita su procedencia en Modrinth; validar esa
  asociación antes de registrarla. Un JSON dañado provoca consulta, no confianza.
- Agrupar contenido idéntico dentro del plan conservando destinos y decisiones
  independientes. Un productor por hash entre operaciones y procesos, con espera
  cancelable y nueva consulta tras adquirir el lock.
- Lease global compartida mientras se usan objetos; productor por clave. No
  retener múltiples locks de productor hasta preparar la transacción. La lease
  vive en la fuente capturada por el trabajo bloqueante, incluso tras descartar
  su futuro. Limpiar requiere lease exclusiva no bloqueante: omitir si está ocupado.
- Cuota inicial 1 GiB y caducidad de siete días, limpieza LRU solo de entradas
  reconocidas. Puede excederse temporalmente con operaciones activas; volver a
  limpiar al terminar. Conservar parciales recientes/reanudables y locks activos.
  No tocar snapshots publicados, instancias, mundos ni entradas ajenas.
- Concurrencia inicial máxima: dos archivos del anfitrión y cuatro CDN. Son
  límites del cliente que se compararán con medidas; el host admite dos streams
  por identidad y no debe saturarse con consultas de control adicionales.
- Sin archivo completo en memoria: escritor bloqueante dueño de archivo y lock,
  buffer de 256 KiB y cola acotada de cuatro bloques. Solo una finalización explícita
  permite éxito tras tamaño/digest/flush/sync. Cancelar/drop termina el escritor
  cooperativamente; el lock no se suelta mientras aún pueda escribir.
- Reanudación propia: hashear el prefijo existente y continuar el digest con los
  bytes recibidos. No releer todo al terminar. Conservar validación de EOF/tamaño,
  corruptos, bloqueo de parciales y rechazo de desplazamientos inválidos.
- Separar cliente de descargas del timeout total de 30 s del JSON: conexión acotada
  e inactividad de lectura acotada, pool reutilizado. Conservar HTTPS, validación de
  URL y redirecciones deshabilitadas. No mover la cuota API al CDN o a iroh.
- Los futuros de descarga comparten el contexto de diagnóstico, progreso monótono
  por contenido terminado y contadores seguros. Emitir hits/misses, bytes reutilizados,
  payload descargado, lotes API y pico de concurrencia, sin nombres/rutas/URL.

## Medición y límites

Primero compilar y guardar un banco del estado secuencial anterior. Misma máquina,
perfil y corpus para comparar frío, caché poblada y segunda instancia; mediana/p95,
peticiones/payload por origen y RAM cuando esté disponible. Usar iroh/HTTP locales
y datos sintéticos, con APPDATA aislado. No consultar instancias ni credenciales.
Los resultados de loopback y hash optimizado en debug no acreditan una partida,
rendimiento de Internet/relé, HDD ni Linux. No prometer un tiempo universal.

## Casos obligatorios

Integridad/corrupción de igual tamaño, dos consumidores del mismo hash, dos rutas
iguales en contenido, procedencias diferentes, cancelación de red y hashing,
drop con escritura pendiente, recuperación de parcial, limpieza con lease y cuota,
links/destinos ajenos, error de catálogo y preservación de transacción/archivos.
