# Ronda 2: guardado del estado del launcher

Fecha: 27-10-2026. Rama `codex/optimizacion-integral`, Windows 11, Ryzen 5 5600X, Rust 1.99.1, perfil `test` (sin optimizar). Datos sintéticos en un `APPDATA` temporal; no se usaron datos personales.

## Qué se midió

`guardar_si_cambio` se ejecuta tras casi cada mensaje de la interfaz. Una prueba temporal ignorada creó N instancias, llamó 500 veces a `guardar_si_cambio` sin cambios y 20 veces alternando la escala de la interfaz, un ajuste global. La prueba no forma parte del cambio; una copia queda fuera del repositorio para repetir la medición.

| N instancias | Sin cambios (por llamada) | Cambio global, antes | Cambio global, después |
| ---: | ---: | ---: | ---: |
| 10 | 85–105 µs | 15,5 ms | 1,8 ms |
| 50 | 358–413 µs | 380,6 ms | 3,1 ms |

## Causa

Cuando el texto del estado cambiaba, `Guardado::escribir` creaba un `Almacen` nuevo y llamaba a `guardar_configuracion` para todas las instancias. Con la caché de rutas vacía, cada búsqueda de carpeta recorría todas las instancias: trabajo cuadrático en el hilo de la interfaz, aunque solo hubiera cambiado un ajuste global.

## Cambio

`escribir` recibe el último texto escrito y el `Almacen` de la aplicación. Solo guarda las configuraciones de instancia cuyo valor cambió; sin un estado anterior válido, como al iniciar, guarda todas. `guardar_configuracion` conserva su mezcla de campos y su comparación de bytes. La regresión `solo_se_guardan_las_configuraciones_de_instancia_que_cambiaron` cubre ajuste global, cambio de una instancia y primer guardado.

## Límites

- Mediciones en perfil `test`; el release será más rápido en ambos casos. Una muestra por N.
- El coste sin cambios (serializar y comparar el estado completo en cada mensaje) sigue ahí: 0,1–0,4 ms con 10–50 instancias. Es la siguiente oportunidad de esta ronda, junto con la apertura del explorador de archivos.
- Una configuración modificada fuera del launcher entre dos guardados ya no se sobrescribe por un cambio global; se mantiene hasta que cambie esa instancia.
