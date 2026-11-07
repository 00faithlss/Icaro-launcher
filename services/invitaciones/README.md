# Invitaciones de Ícaro (HTTP v1)

Estado del 07-11-2026: 17 pruebas locales del servicio pasadas y una prueba de
interoperabilidad Rust/D1/iroh con aplicación y revocación. El [progreso general](../../docs/superpowers/progreso/2026-11-07-compartir-instancias.md)
separa estas comprobaciones de la integración pendiente del launcher.

Worker y D1 preparados y probados **solo localmente**. No existe un endpoint de
producción configurado. No se ha creado cuenta, base remota ni despliegue, y no
se habilita ningún plan de pago. Este servicio intercambia metadatos de encuentro
y autorizaciones; los archivos viajan directamente entre launchers.

## Desarrollo local

Desde esta carpeta, con Node/npm:

```powershell
$env:WRANGLER_SEND_METRICS = 'false'
npm.cmd ci --no-audit --no-fund
npm.cmd test
npm.cmd run migrate:local
npm.cmd run dev -- --port 8787
```

El cliente debe recibir explícitamente `http://127.0.0.1:8787` para desarrollo.
Fuera de loopback requiere HTTPS. Una URL ausente significa función sin
configurar; nunca sustituirla por una URL ficticia. Los tests ejecutan workerd y
SQLite/D1 reales de Miniflare, sin red ni SQL simulado y sin datos del usuario.
Las fixtures de reloj, colisión y medición instrumentan exclusivamente el código
cargado por el test; no hay puertas de prueba en el Worker publicado.

Empaquetado sin publicar:

```powershell
npm.cmd exec wrangler deploy -- --dry-run --outdir ../../target/desarrollo-compartir/invitaciones/worker
```

## Firma y tipos

Todas las rutas son `POST /v1/<acción>`, sin query, y reciben JSON UTF-8:

```json
{"public_key":"64 hex minúsculas","nonce":"32 hex minúsculas","issued_at":1794000000,"payload":"{\"code\":\"0123456789AB\"}","signature":"128 hex minúsculas"}
```

`public_key` es la clave pública Ed25519 de 32 bytes; `signature`, una firma de
64 bytes, ambas codificadas hexadecimal. `nonce` son 16 bytes aleatorios nuevos
en **cada solicitud**, también en reintentos. `issued_at` es Unix en segundos,
entero seguro; tolerancia de reloj ±120 s. Se verifica Ed25519 WebCrypto sobre
estos bytes exactos, sin salto final:

```text
icaro-invitaciones/1\nPOST\n/ruta\npublic_key\nnonce\nissued_at\npayload
```

En esta notación `\n` representa un byte LF; `/ruta` es, por ejemplo,
`/v1/claim`. `payload` es un **string con JSON**, no un objeto anidado: se firma
literalmente ese string, evitando desacuerdos de canonicalización Rust/JS. El
JSON exterior puede cambiar espacios sin alterar la firma. No registrar el
cuerpo, el código o la clave privada.

El encuentro `Meeting` es `{payload:string, signature:128hex}`. Su payload JSON:

```json
{"host":"64hex","publication":"32hex","endpoint_hints":["dirección de transporte"],"issued_at":1794000000}
```

El anfitrión firma exactamente `icaro-encuentro/1\n` seguido del string payload.
`host` debe coincidir con la identidad que crea/actualiza, y `publication` con
la publicación. Se verifica fecha ±120 s al registrarlo; el cliente conserva
la firma original y confirma la **clave completa y su huella**, nunca confía
por nombre. No se impone al encuentro almacenado el TTL de las solicitudes:
puede seguir resolviéndose días después. Se permiten hasta 8 hints de 512
caracteres, payload de encuentro 4096 caracteres. El servicio no se conecta a
esas direcciones ni interpreta su protocolo.

## Rutas y DTOs

`publication`: ID aleatorio de 16 bytes, 32 hex minúsculas. `guest`: clave de
32 bytes, 64 hex minúsculas. Las mutaciones de anfitrión exigen la clave que
registró esa publicación; conocer un código no permite administrarla.

| Ruta | Payload JSON dentro de `payload` | Respuesta 200 |
| --- | --- | --- |
| `/v1/create` | `{publication,duration_hours,capacity?,meeting}` | `{code,expires_at,capacity,accepted:0}` |
| `/v1/resolve` | `{code}` | `Resolved` |
| `/v1/claim` | `{code}` | `Resolved` con `authorized:true` |
| `/v1/update` | `{code,duration_hours,capacity?,meeting?}` | `Resolved` |
| `/v1/revoke-code` | `{code}` | `{ok:true}` |
| `/v1/revoke-access` | `{publication,guest}` | `{ok:true}` |
| `/v1/stop` | `{publication}` | `{ok:true}` |
| `/v1/authorizations` | `{publication,after?:guest}` | `{publication,active,guests:[guest],next:guest|null}` |

`Resolved`:

```json
{"host":"64hex","publication":"32hex","meeting":{"payload":"...","signature":"128hex"},"expires_at":1794086400,"capacity":10,"accepted":1,"authorized":true}
```

`duration_hours`: exclusivamente 24, 48 o 168, desde la **creación** del código.
`capacity`: entero 1..4294967295; omitido significa 10 (también en update).
No hay máximo de producto de 50. Para conservar cupo al modificar otro ajuste,
el cliente debe reenviar el cupo actual. `accepted` cuenta identidades que
consumieron ese código, incluso si luego se revoca su acceso; no representa
sesiones conectadas. La revocación preventiva de un invitado aún no aceptado
no consume cupo. Una revocación individual deja una marca que impide volver a
reclamar con la misma identidad y otro código de esa publicación.

El código tiene 12 caracteres Crockford (60 bits aleatorios). Se admiten letras
minúsculas, espacios/guiones, O→0 e I/L→1, con entrada de hasta 20 caracteres;
la respuesta usa el formato canónico sin separadores. Se guarda solamente su
SHA-256. Una colisión reintenta hasta cuatro veces y falla con 503 sin sobrescribir.
El código recién creado se devuelve una vez; no hay recuperación de texto plano.

Una reclamación nueva comprueba fecha, revocación y cupo e inserta la identidad
en **una sentencia SQL**, con incremento de cupo en trigger de la misma
transacción y PK `(publication,guest)`. El reintento firmado con nonce nuevo no
consume otro cupo. Las autorizaciones existentes sobreviven a caducar o revocar
el código y pueden resolverlo/reclamarlo de nuevo; un invitado desconocido no.
Cambiar ajustes exige código vigente, fecha futura y cupo no inferior al ya
consumido. Volver a compartir es una operación explícita: conserva el origen,
genera otra invitación y exige nueva aceptación a los invitados anteriores.
Crear un código tras detener limpia las autorizaciones de la generación anterior;
renovar un código mientras se comparte conserva los accesos y las revocaciones.

`authorizations` devuelve hasta 100 claves, ordenadas, con cursor `next` para la
página siguiente. Un anfitrión consulta al recibir una petición de conexión y
persiste el acceso antes de servir. **La respuesta claim del invitado no es una
credencial que el anfitrión deba creer**: el anfitrión consulta el servicio
autenticado y contrasta la clave del par en su transporte. La autenticidad del
servicio depende de HTTPS y la URL configurada; no hay clave de firma del
servicio ni un token autónomo. No sondear continuamente por invitado. La
revocación desde el propio anfitrión debe cortar también sus sesiones y su
almacén local; una revocación remota no notifica por push a launchers desconectados.

## Errores, límites y almacenamiento

Errores JSON `{error:string}`, siempre `Cache-Control: no-store`:

| Estado | Ejemplos |
| --- | --- |
| 400 | `invalid_request`, `invalid_json`, `invalid_payload` |
| 401 | `invalid_signature`, `expired_signature`, `invalid_meeting_signature` |
| 403 | `wrong_host`, `access_revoked` |
| 404 | `not_found` |
| 409 | `replayed_nonce`, `capacity_full`, `capacity_below_accepted`, `expiry_in_past` |
| 410 | `invitation_expired`, `sharing_stopped` |
| 413 | `body_too_large` |
| 429 | `rate_limited`, `identity_rate_limited`, `host_invitation_limit` |
| 503 | `daily_budget_exhausted`, `temporarily_unavailable`, `code_collision` |

No reintentar 403/410 automáticamente. Ante 429 esperar al menos `Retry-After`
(60 s) con jitter; ante 503 mostrar indisponibilidad y permitir reintento
limitado, conservando el vínculo local. Un fallo D1 nunca concede acceso.

Cuerpo limitado por lectura a 16 KiB, payload string 12000 caracteres. Límites
persistentes en D1: 60 solicitudes/minuto por origen (variable
`RATE_PER_MINUTE`), 60/minuto por identidad y presupuesto diario global
`DAILY_REQUEST_BUDGET=5000`. Se almacenan hashes diarios de IP, no IP en claro;
`CF-Connecting-IP` se confía solo detrás de Cloudflare. Hasta 100 invitaciones
vigentes por anfitrión como protección operativa, distinta del cupo de personas.
No usar este ejemplo como garantía de resistencia a ataques distribuidos.

Los nonces se retienen 300 s, fuera de la ventana de firmas; cron cada cinco
minutos elimina nonces y contadores vencidos. Se conservan claves públicas,
publicaciones, encuentros firmados, hashes, fechas y marcas de revocación para
respetar accesos tras caducar códigos. No se guardan mods, nombres de cuenta,
tokens Microsoft o claves privadas. La eliminación de publicaciones abandonadas
y política de retención administrativa necesitan definirse antes de producción;
no se eliminan accesos persistentes automáticamente.

Medición automatizada local de sentencias preparadas (incluye seguridad, no el
SQL interno del trigger): crear **11**, resolver **7**, aceptar **8**, reintentar
aceptación **8**. El trigger suma una actualización en la primera aceptación.
No equivale a filas facturadas: índices y limpieza añaden escrituras. La cuota
gratuita consultada permite 5 millones de filas leídas y 100000 escritas/día;
el presupuesto conservador no garantiza disponibilidad frente a tráfico
malicioso: incluso solicitudes rechazadas consumen invocaciones y contadores.
Fuentes oficiales: [D1 pricing](https://developers.cloudflare.com/d1/platform/pricing/),
[D1 batch](https://developers.cloudflare.com/d1/worker-api/d1-database/),
[Workers limits](https://developers.cloudflare.com/workers/platform/limits/).

## Despliegue futuro: requiere autorización independiente

Estas instrucciones **no se han ejecutado**. En una cuenta explícitamente Free,
sin actualizar el plan, un operador autorizado usaría:

```powershell
npx wrangler login
npx wrangler d1 create icaro-invitaciones
# Sustituir database_name/database_id locales por los devueltos por Cloudflare.
npx wrangler d1 migrations apply DB --remote
# Configurar una ruta/dominio propio o workers_dev=true conscientemente.
npx wrangler deploy
```

No hay secretos del servicio requeridos: la autorización depende de firmas
Ed25519 por equipo. Las claves privadas pertenecen al almacén seguro local del
launcher; nunca se envían al servicio. El token de despliegue, si se usa en vez
de login, pertenece al entorno del operador, nunca al repositorio ni al chat.
Antes de publicar hay que revisar retención, métricas reales y protecciones
disponibles en el plan Free, y configurar explícitamente HTTPS en el cliente.
Las pruebas locales no acreditan cuota real, disponibilidad, latencia, despliegue
Cloudflare ni conexión entre dos equipos.
