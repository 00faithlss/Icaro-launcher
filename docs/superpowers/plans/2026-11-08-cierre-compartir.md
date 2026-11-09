# Cierre de pendientes de Compartir

> Ejecución continua solicitada por el usuario, con Claude Code Opus 5.5 medio y supervisión. Aplicar executing-plans y pruebas de regresión.

**Objetivo:** terminar los pendientes locales antes de la validación real entre equipos.
**Arquitectura:** conservar servicios recuperables del núcleo y componentes iced originales; módulos pequeños para selección de packs, biblioteca, cierre y bandeja.
**Tecnologías:** Rust/iced, Windows nativo con windows-sys existente y Worker/D1.
**Referencia:** specs/2026-11-07-sincronizacion-entre-launchers-design.md y paquetes originales de diseño.

## Restricciones y decisiones de ejecución

- Una carpeta, `v0.4.x`, versión 0.3.4. No commit/push/merge/release por inferencia.
- Mantener los cambios previos sin registrar y todos los iconos/medidas del diseño.
- Cargo único, `-j 2`, APPDATA temporal, sin datos reales ni keyring en pruebas.
- La orden «todo de corrido» delega las decisiones de implementación; no se abre otra ronda de diseño.
- Orden de packs: conservar lo personal y pedir elección explícita en la revisión existente ante cambios simultáneos. Registrar la selección local revisada y rechazar cambios posteriores antes de aplicar. No copiar otras opciones.
- Biblioteca: recuperar publicaciones persistidas desde el selector/panel existente; no crear otra identidad ni convertir una revisión privada en compartida sin consentimiento.
- Cierre: minimizar conserva servicio; esperar rechaza nuevas transferencias, termina las admitidas y después cierra. Dejar de compartir revoca; cerrar la aplicación conserva autorizaciones para reanudar. Bandeja restaurable Windows; sin soporte nativo no ocultar una ventana irrecuperable.
- Espacio: comprobar conservadoramente antes de descargar/preparar; no borrar archivos ajenos ni contenido vigente. Recuperar diarios antes de limpiar restos propios abandonados.
- Servicio: preparar y verificar despliegue gratuito, sin habilitar facturación ni publicar canales. Si falta una sesión de Cloudflare, completar preparación y solicitar solo el acceso externo que falte.

## Focos de revisión

- Cambios externos en options.txt tras revisar; no sobrescribirlos.
- Packs desactivados o retirados mientras se conserva orden local; no dejar referencias rotas nuevas.
- Cierre durante publicación o aplicación atómica; esperar acuse antes de salir.
- Reinicio de Explorer y ausencia de bandeja; conservar vía para restaurar.
- Sin espacio o limpieza concurrente; no tocar transacciones activas ni revisiones publicadas.

## Bloques

- [x] Packs: regresión RED de conflicto/resolución; implementar contrato tipado en núcleo y fila de conflicto en revisión existente; probar conservación y cambio externo.
- [x] Publicador: Claude prepara pruebas, ejecutar RED, implementar recuperación/listado/contadores en módulos independientes y verificar.
- [x] Cierre: módulo nativo separado, controlador de cierre, guardias de actividad y regresiones; validar restauración y capturas.
- [x] Espacio/recuperación: presupuesto con suma comprobada, precomprobación y limpieza solo de staging propio sin actividad, regresiones de preservación.
- [x] Servicio: comprobar cuenta disponible sin exponer credenciales, configuración de producción separada, migración y dry-run; desplegar solo si existe acceso válido y plan gratuito.
- [x] Verificación conjunta: core/UI/Worker, build, revisión independiente, capturas nativas Tinta/Piedra y ventana pequeña.
- [x] Actualizar guías, progreso, roadmap y límites con evidencia; diferenciar validación local, despliegue y partida real.
