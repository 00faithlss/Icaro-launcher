# Plan de registros de diagnóstico

> Ejecutar con `superpowers:executing-plans`; desarrollo supervisado con Claude Code Opus 5.5 medio.

**Objetivo:** conservar contexto útil de los errores sin bloquear iced ni registrar secretos.
**Arquitectura:** emisores tipados, mediciones de operación y un escritor dedicado con cola
acotada, rotación y estado de salud; emergencia síncrona exclusiva de panic fatal.
**Tecnologías:** Rust/std, Tokio, serde_json y sha2 existentes, sin dependencias nuevas.
**Diseño:** [contrato](../specs/2026-11-08-diagnostico-design.md).

## Restricciones

- Una carpeta, rama `v0.4.x`; conservar cambios anteriores sin commit y versión 0.3.4.
- Sin commit/push, despliegue ni telemetría; no leer datos personales ni keyring en pruebas.
- Cargo único con `-j 2`, APPDATA temporal y `CARGO_INCREMENTAL=0`.
- Nuevos módulos pequeños por responsabilidad; no concentrar lógica en launcher/juego_real.
- Solo el escritor hace IO normal del registro; etiquetas estáticas y causas tipadas.

## Revisión especial

- Cola llena: ni bloquear UI ni perder eventos sin reflejarlo en contadores.
- Disco ocupado/no regular/rotación fallida: conservar archivos ajenos y notificar estado.
- Panic con abort: escribir emergencia antes de llamar al hook anterior.
- Errores que contienen rutas/URL/credenciales: no serializar Display/Debug dinámico.
- Cierre con escritor atascado: plazo finito y diagnóstico de pérdida potencial.

## 1. Base tipada y persistencia

Archivos nuevos: `crates/core/src/diagnostico.rs`, `diagnostico/{modelos,escritor,panico}.rs`
y pruebas de integración `crates/core/tests/diagnostico.rs`.
Registro mínimo en `crates/core/src/lib.rs`.

Interfaces: `iniciar(root, version, compilacion) -> Result<Sesion, ErrorRegistro>`;
`Sesion::{registro,cerrar}`; `Registro::{medir,vaciar,estado}`; `Medicion::{etapa,dato,
instancia,terminar}`; `Medicion::global`; `Resultado::{Exito,Cancelado,Error(Causa)}`;
`Campo::{Numero,Etiqueta,CodigoSalida}`; `Causa::{de_io,de_red}`. La sesión instala
opcionalmente registro global y hook mediante métodos explícitos.

- [x] Escribir pruebas y observar RED por API ausente.
- [x] Implementar con Claude en archivos nuevos, sin shell/Cargo/Git desde Claude.
- [x] Verificar GREEN de correlación, concurrencia, rotación, salud, privacidad y cierre.
- [x] Probar panic con subproceso que aborta en el hook anterior; mantener payload fuera.

## 2. Integración de operaciones y causas

Crear `launcher_aux/diagnostico_real.rs` y helpers acotados para Compartir;
modificar solo puntos de entrada/etapa/resultado de tareas existentes y hooks mínimos
de `launcher.rs`, `juego_real.rs`, cuenta y actualizador. La guardia del logger vive
fuera de iced::run y solicita cierre al retornar.

- [x] Instrumentar compartir/publicar/invitaciones/instalar/consultar/aplicar, con
  causa segura antes de convertir errores a String donde sea posible.
- [x] Instrumentar preparación y ejecución del juego, código de salida y cancelación.
- [x] Conectar inicio, estado de salud y cierre del logger; demo no toca datos reales.
- [x] Revisar logs antiguos de sincronización y eliminar datos dinámicos sensibles.
- [x] Probar fallo de servicio/instalación con logs reales en raíz temporal y ausencia
  de nombres, códigos, URL y rutas de las fixtures.

## 3. Revisión, verificación y guías

- [x] Revisión fresca de Claude solo lectura; reproducir y corregir fallos importantes.
- [x] Núcleo y UI secuencial, pruebas de logger y recorrido local afectado; compilar
  launcher, rustfmt acotado y `git diff --check`. No hay rediseño visual; queda pendiente captura específica del aviso de salud.
- [x] Actualizar guía de diagnóstico, progreso, pendientes y changelog sin publicar.
- [x] Registrar cobertura/límites concretos y estado local; no commit/push por inferencia.
