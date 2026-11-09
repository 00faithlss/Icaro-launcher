# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

Las reglas del proyecto, los comandos de compilación y prueba, la forma de trabajar (ramas `v0.X.x`, commits, iconos centrados, fidelidad al System Design, entorno Windows) y el estado de publicación están en `AGENTS.md`, compartido con otros agentes. Léelo siempre:

@AGENTS.md

## Comandos frecuentes en este equipo

Cargo puede no estar en el PATH de la consola: anteponer `export PATH="$HOME/.cargo/bin:$PATH"` (Bash) o `$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"` (PowerShell). Compilar con `-j 2`.

```sh
cargo build --locked -j 2 -p icaro-ui --example launcher
# Una prueba concreta del launcher o del núcleo, por nombre o fragmento:
cargo test --locked -j 2 -p icaro-ui --example launcher <nombre> -- --test-threads=1
cargo test --locked -j 2 -p icaro-core --lib <ruta::del::test>
cargo test --locked -j 2 -p icaro-ui --lib <nombre>
```

Capturas de una escena (PowerShell, desde la raíz):

```powershell
$env:ICARO_DEMO = '1'; $env:ICARO_CAPTURA_DISENO = "$env:TEMP\escena.png"; $env:ICARO_VISTA_DISENO = 'cuenta-original'
.\target\debug\examples\launcher.exe
Remove-Item Env:ICARO_DEMO, Env:ICARO_CAPTURA_DISENO, Env:ICARO_VISTA_DISENO
```

## Arquitectura

- `icaro-core` hace todo el trabajo de disco y red con funciones `async` o bloqueantes: autenticación y tokens (`auth`, `vault`), perfil, skins y capas de Microsoft (`cuenta`), instancias y sus metadatos (`instancias`, con `estadisticas` de tiempo jugado y tamaño), instalación de Minecraft, loaders y Java (`minecraft`, `java`, `lanzamiento`), catálogo de Modrinth con caché y presupuesto de solicitudes (`modrinth`), mods/packs y la sincronización transaccional con anfitriones (`mods`, `mods/sincronizacion`), y el actualizador (`actualizacion`). No depende de la interfaz.
- `icaro-ui` (biblioteca) son componentes iced puros, sin estado de aplicación: tema y tokens (`tema::Paleta`, `espacio`, `medida`, `texto`), fuentes, iconos SVG, botones, pantallas grandes como funciones `fn(Paleta, Datos, Mensajes) -> Element` (`cuenta`, `editor`, `capturas`…), el desplazamiento suave (`scroll`) y el visor 3D de skins (`visor_skin`). Reciben datos ya calculados y devuelven mensajes genéricos `M`.
- La aplicación es el ejemplo `crates/ui/examples/launcher.rs`, una app iced de un solo `App` y un `enum Mensaje`. `actualizar` envuelve a `actualizar_estado`: después de cada mensaje no transitorio llama a `guardar_si_cambio` (estado en `estado.rs`, configuración por instancia en `instance.json`) y pide en segundo plano los datos que falten (por ejemplo `estadisticas_reales::pedir`).
- `launcher_aux/` divide la app por área. Cada módulo sigue el mismo patrón: un `Estado` en `App`, un `enum Accion` envuelto en una variante de `Mensaje`, `actualizar(app, accion) -> Task` y funciones de vista que arman los `Datos` de `icaro_ui`. Los módulos `*_real` conectan con `icaro-core`. Con `ICARO_DEMO=1` (`app.servicios.demo`) las vistas usan muestras y simulan acciones; ambos caminos comparten los mismos componentes, así que las capturas de demo sirven para revisar el diseño.
- Las respuestas asíncronas llevan una generación o identidad (`generacion`, `secuencia`, id de cuenta o instancia) y se descartan si ya no corresponden: mantener ese patrón al añadir tareas.
- `crates/ui/build.rs` prepara los grabados (WebP sin pérdida y miniaturas RGBA) en `OUT_DIR`; las pantallas los incluyen con `include_bytes!`.
- Las pruebas de la app viven sobre todo en `launcher_aux/servicios.rs` (recorridos largos con un `Almacen` en un directorio temporal) y en `mod tests` de cada módulo.
