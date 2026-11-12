# Registros de diagnóstico

[Documentación](README.md) · [Estado y pendientes](roadmap.md)

El launcher normal inicia automáticamente un registro local. No requiere cuenta,
servicio de pago ni envío de información. Demo y capturas de diseño no lo activan.
Este bloque está en desarrollo en `v0.4.x`; no constituye una versión publicada.

## Dónde encontrarlos

En Windows, dentro de la raíz de datos de Ícaro:

```text
%APPDATA%\Icaro\logs\diagnostico.jsonl
%APPDATA%\Icaro\logs\panico.jsonl
```

Cada línea es un objeto JSON independiente. El registro normal rota a
`diagnostico.jsonl.1`, `.2` y `.3`, con límite de 1 MiB por archivo. La emergencia
rota a `panico.jsonl.1`, con límite de 256 KiB por archivo. No borrar estos archivos
antes de investigar: las operaciones recientes pueden empezar en una copia rotada.
El botón existente para exportar registros del juego conserva su alcance de
Minecraft; todavía no exporta estos archivos de diagnóstico.

## Cómo investigar un fallo

1. Localizar la hora UTC y el resultado `error`, `cancelado` o `abandonado`.
2. Agrupar por `sesion` y `operacion` para reconstruir las etapas anteriores.
3. Leer `componente`, `accion`, `etapa`, `duracion_ms` y `causa`. Cuando está
   disponible se conserva el estado HTTP, código de servicio permitido, tipo de
   error de disco/código del sistema o código de salida del proceso Java.
4. Revisar eventos de descartes y el aviso del launcher: un fallo del escritor
   significa que el archivo podría estar incompleto.

Se instrumentan publicación, invitaciones, instalación, consulta y aplicación de
Compartir, preparación y ejecución del juego, autenticación y actualizador.
Las tareas anidadas y el trabajo bloqueante propagan la misma operación. Los
errores HTTP y de almacenamiento capturados fuera de esas operaciones generan
incidentes con ubicación del código. La cobertura es progresiva: un error que
ya perdió su causa en un servicio anterior puede conservar solo una categoría
general. No se inventa una cadena de causas a partir del mensaje mostrado en UI.

`exito` acredita el desenlace de esa operación, no una partida jugable ni todo el
recorrido. `abandonado` indica que dejó de observarse la operación: un trabajo
bloqueante ya iniciado puede seguir terminando. La cancelación no oculta un error
de recuperación. El tiempo transcurrido usa un reloj monotónico.

## Privacidad y límites

El formato acepta causas tipadas, números y etiquetas estáticas. No copia mensajes
dinámicos de errores, URL, códigos de invitación, tokens, nombres de cuenta o
instancia, rutas completas ni contenido de un panic. Las instancias se correlacionan
con un alias por sesión. No se envían registros automáticamente.

El antiguo `sincronizacion.log` continúa existiendo; sus nuevas entradas filtran
mensajes y anonimizan identificadores. Las entradas anteriores no se reescriben.
Estos registros son distintos de los logs que escriben Minecraft y sus mods.

El escritor usa un hilo y una cola acotada: la emisión no hace IO de disco desde
iced. Los contadores reflejan descartes y fallos de escritura; el launcher avisa
una vez si detecta un problema. El cierre solicita vaciado con plazo de 750 ms.
Ante panic se intenta escribir el archivo de emergencia antes del hook anterior,
incluido un hook que aborte el proceso. No garantiza conservar eventos ante disco
lleno, terminación forzada, falta de memoria o pérdida de energía.

Se rechazan destinos no regulares y enlaces detectados, y se vuelven a comprobar
las carpetas durante escritura/rotación. Esto no constituye protección frente a
una carrera deliberada de otro proceso con los mismos permisos del usuario.

## Validación

Las regresiones cubren correlación, privacidad, concurrencia, rotación, escrituras
parciales, cierre con eventos pendientes, pérdida de eventos, fallo HTTP local,
propagación a tareas bloqueantes y emergencia antes de abortar un subproceso.
Se ejecutan con una raíz temporal y `APPDATA` aislado. Las pruebas de enlaces
dependen de los permisos disponibles en Windows. Falta validación con dos equipos,
Linux, autenticación real y una partida real; las fixtures no la sustituyen.
