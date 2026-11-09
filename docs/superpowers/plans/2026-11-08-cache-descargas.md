# Plan de optimización de caché y descargas

> Ejecutar de corrido con executing-plans; auditorías y tareas independientes según dispatching-parallel-agents. Cargo único supervisado por root.

**Objetivo:** reducir transferencias repetidas y esperas secuenciales sin perder integridad/recuperación.
**Arquitectura:** CAS con leases/locks, cola por origen y escritor con propiedad completa del IO.
**Tecnologías:** Rust/std/Tokio/reqwest/sha2 existentes; sin dependencias nuevas.
**Diseño:** [contrato](../specs/2026-11-08-cache-descargas-design.md).

## Restricciones

- `v0.4.x`, misma carpeta, versión 0.3.4; conservar cambios locales anteriores.
- Registrar y subir a `v0.4.x` por solicitud posterior del usuario. Usar la carpeta
  actual y datos sintéticos en las pruebas.
- Cargo único con `-j 2`, `CARGO_INCREMENTAL=0`, APPDATA temporal.
- Módulos por responsabilidad; no cambiar el diseño ni trabajar disco/red en iced.
- Conservar copias independientes, SHA-512 completo y diario recuperable.

## Focos de revisión

- Un futuro descartado no debe liberar el lock mientras el actor siga escribiendo.
- Una lease de otra raíz, o metadata corrupta, no acredita integridad/procedencia.
- Error en un archivo cancela y drena los demás sin publicar una preparación parcial.
- Cuota/limpieza no elimina objetos usados, parciales activos ni snapshots.
- Un cache hit conserva la consulta de autorización/manifiesto previa del invitado.

## 1. Investigación y base

- [x] Leer investigación anterior/datos/banco e inspeccionar implementación actual.
- [x] Consultar fuentes primarias de Tokio, Modrinth e iroh y auditorías independientes.
- [x] Compilar banco secuencial antes de editar rutas de producción y guardar binario, datos y condiciones en carpeta temporal de evidencia.

## 2. CAS

Archivos: `compartir/cache.rs`, `cache/{archivos,limpieza,metadatos}.rs`,
`tests/compartir_cache.rs`. API exacta:
`CacheContenido::nueva(PathBuf)`, `con_limites(bytes,Duration)`,
`sesion(&Token)->Arc<GuardiaUso>`, `reservar(hash,bytes,Arc<GuardiaUso>,&Token)->ReservaHash`,
`limpiar(&Token)->InformeCache`; operaciones async salvo construcción/límites.
`ReservaHash::{buscar(&Token),publicar(&Token),procedencia_confirmada(&Procedencia,&Token),confirmar_procedencia(&Procedencia,&Token)}` async;
`staging()->PathBuf` sin IO. Constructor no crea carpetas.

- [x] Escribir regresiones y observar RED por módulo ausente (`cache-rojo.log`).
- [x] Implementar publicación, leases, espera de locks, LRU/TTL y confirmación exacta de procedencia y probar rechazo de raíces cruzadas/objetos ausentes.

## 3. Escritura y transporte

Archivos: `descarga_archivo.rs` y submódulos; `compartir/transporte/descarga.rs`,
`http.rs`, `modrinth.rs`. Mantener API pública anterior de descarga.

- [x] Pruebas del actor para digest, prefijo, tamaño, finalización explícita, drop y lock ocupado.
- [x] Implementar actor y hash incremental propio, con cliente HTTP de descargas y pool compartido.
- [x] Verificar integridad, cancelación, recuperación y ausencia de escrituras tardías en pruebas locales.

## 4. Coordinador e integración

Archivos: `compartir/descargas.rs` y módulos de cola/métricas; dos tareas UI del
invitado/sincronización pasan ruta de caché explícita. Sin nuevas dependencias.

- [x] Regresiones de deduplicación, límites/concurrencia, caché caliente sin catálogo y procedencia sin aprobación.
- [x] Buscar primero, resolver solo metadata necesaria, descargar agrupado con límites 2/4, conservar lease hasta terminar preparación y limpiar al liberar.
- [x] Instrumentar métricas y progreso y comparar el banco frío/caliente con la base para 32 archivos.

## 5. Entrega

- [x] Revisión independiente de integridad, concurrencia, leases, cuotas y errores.
- [x] Verificar núcleo, UI y banco local, con repetición focalizada de caché, descarga y transporte tras ajustar el drenaje.
- [x] Actualizar guía, progreso, pendientes y changelog y registrar [los resultados locales](../../investigaciones/rendimiento-compartir-2026-11-08.md).
- [ ] Repetir el banco en release y medir los recorridos de red reales.
