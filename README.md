# Ícaro Launcher

Un launcher de Minecraft Java escrito en Rust, con instancias independientes, mods de Modrinth y una interfaz inspirada en la mitología y los grabados clásicos.

[Descargar para Windows](https://github.com/00faithlss/Icaro-launcher/releases/latest) · [Versiones beta](https://github.com/00faithlss/Icaro-launcher/releases) · [Documentación](docs/README.md) · [Roadmap](docs/roadmap.md)

[![Licencia: GPL-3.0-only](https://img.shields.io/badge/License-GPL--3.0--only-red.svg)](LICENSE)

## Empezar

1. Descarga el **instalador** o el **ZIP portable** desde [Releases](https://github.com/00faithlss/Icaro-launcher/releases/latest). Las betas aparecen por separado como *Pre-release*.
2. Abre Ícaro e inicia sesión con Microsoft o crea una [cuenta sin conexión](docs/cuentas-sin-conexion.md). Esta última no permite entrar en servidores que exigen autenticación Microsoft.
3. Crea una instancia con la versión de Minecraft y el loader que necesites, o [importa un modpack MRPACK/ZIP](docs/importar-modpacks.md). Ícaro prepara el juego y resuelve Java según el perfil.
4. Instala los mods desde la instancia y pulsa **Jugar**.

Los paquetes de escritorio se distribuyen para Windows. El código también contempla Linux, cuyo recorrido completo todavía necesita validación; consulta los [requisitos de desarrollo](docs/desarrollo.md#requisitos).

## Qué puedes hacer

- **Organizar tus partidas:** instancias, grupos, portadas e iconos; versiones separadas de Minecraft y loader; ajustes de Java y memoria por instancia.
- **Gestionar mods y packs:** catálogo de Modrinth, dependencias, actualizaciones, JAR propios, resourcepacks y shaders con Iris.
- **Importar y exportar:** MRPACK y ZIP completos compatibles, exportación ZIP Ícaro/Prism y listas de mods en varios formatos.
- **Personalizar tu cuenta:** skins, visor 3D, historial y perfiles sin conexión con skin local.
- **Conectar con servidores:** lista guardada, estado, instancia vinculada y sincronización de archivos exactos con un anfitrión dedicado.
- **Revisar lo que ocurre:** descargas, consola de Minecraft, capturas y registros de diagnóstico.
- **Elegir la apariencia:** temas Tinta y Piedra, acentos Mármol/Plata/Bronce, escala y movimiento reducido.

Vanilla, Fabric y Quilt usan perfiles oficiales; Forge y NeoForge admiten los instaladores oficiales spec 1. Vanilla y Fabric tienen confirmación de juego en Windows. Las partidas con Quilt/Forge/NeoForge y las migraciones de estos últimos aún requieren validación. Consulta el [alcance de los loaders](docs/loaders-forge-neoforge.md).

## Compartir con amigos

**Compartir instancias es experimental.** Permite elegir contenido, generar una invitación y revisar los cambios antes de sincronizar, conservando los archivos personales. La sincronización del contenido no sustituye una conexión LAN o un servidor de Minecraft.

En el código actual se activa desde **Servidores → Con amigos**; la beta 0.4.2 publicada utiliza la navegación anterior de Servidores. Hay pruebas locales del recorrido y del servicio remoto de invitaciones; faltan la validación entre dos equipos y una partida real con amigos. Consulta la [guía de Compartir](docs/compartir-instancias.md).

## Roadmap

El trabajo actual se desarrolla en **`v0.4.x`**, con versión de workspace **0.4.2** y cambios posteriores a la beta de ese número. `develop` es el canal de publicación beta y `main`, el estable. Las funciones descritas corresponden al código de esta rama; la versión que descargues puede tener un alcance anterior.

El [roadmap](docs/roadmap.md) reúne lo implementado, los pendientes y la evidencia necesaria para cerrarlos. Las prioridades son validar Compartir entre equipos, completar las partidas con loaders y Linux, avanzar en mundos/respaldos y comprobar el ciclo completo de actualización Windows. El [changelog](CHANGELOG.md) conserva los cambios por versión.

## Compilar

Con Rust estable y las herramientas de compilación de tu plataforma:

```sh
git clone https://github.com/00faithlss/Icaro-launcher.git
cd Icaro-launcher
git switch v0.4.x
cargo run --locked -j 2 -p icaro-ui --example launcher
```

Para explorar la interfaz con datos de muestra, establece `ICARO_DEMO=1`. Los comandos de pruebas, dependencias de Linux y capturas aisladas están en la [guía de desarrollo](docs/desarrollo.md). La distribución de responsabilidades se explica en [Arquitectura](docs/arquitectura.md).

## Documentación y soporte

El [índice de documentación](docs/README.md) organiza las guías de cuentas, instancias, Java, servidores, desarrollo y publicación. Si encuentras un fallo, revisa [cómo obtener el diagnóstico](docs/diagnostico.md) y abre un [issue](https://github.com/00faithlss/Icaro-launcher/issues) con la versión, sistema y pasos para reproducirlo. Revisa los registros antes de adjuntarlos y retira información personal.

## Licencia y créditos

El código se distribuye bajo [GNU GPL-3.0-only](LICENSE). Las bibliotecas, fuentes, iconos e imágenes conservan sus propias licencias; sus atribuciones están en [Créditos](docs/creditos.md).

El icono utiliza una fotografía de *La caída de Ícaro*, de Merry-Joseph Blondel: © Marie-Lan Nguyen / Wikimedia Commons, [CC BY 2.5](https://creativecommons.org/licenses/by/2.5/), con recorte y adaptación a grises.
