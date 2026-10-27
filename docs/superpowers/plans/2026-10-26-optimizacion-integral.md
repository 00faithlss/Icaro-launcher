# Plan de optimización integral

**Objetivo:** Reducir esperas y espacio con mediciones verificables; conservar comportamiento, credenciales protegidas y compatibilidad.
**Diseño:** ../specs/2026-10-26-optimizacion-integral-design.md
**Base:** f7b96af (develop 0.3.3). Sin commits ni push sin autorización.

1. Medir línea base: tamaño de archivos, compilación limpia e incremental en directorio separado y métricas por crate; recopilar datos de Actions y Windows cuando estén disponibles. Anotar revisión, entorno y limitaciones.
2. Analizar inicio de sesión: probar recuperación local, renovación, cambio de cuenta, cierre y fallos de red; establecer estados visibles y tiempos antes de implementar datos de perfil persistidos. No guardar credenciales fuera del almacén protegido.
3. Comparar perfiles de Cargo y alternativas de caché/parallelismo de Actions con resultados fríos y calientes; no cambiar publicación ni eliminar pruebas. Registrar tamaño y comportamiento de ejecutables.
4. Instrumentar y medir inicio, navegación, imágenes y sincronización con datos no sensibles; mejorar puntos calientes comprobados, con regresiones por cada cambio.
5. Dividir los módulos más grandes por responsabilidad mediante movimientos pequeños y pruebas de comportamiento; no asumir que menos líneas significa más velocidad.
6. Ejecutar pruebas relevantes, revisar seguridad y compatibilidad y comparar resultados; documentar lo no validado en Windows, Actions y red real.
