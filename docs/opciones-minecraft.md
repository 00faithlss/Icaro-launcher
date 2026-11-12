# Preferencias compartidas de Minecraft

[Documentación](README.md) · [Estado y pendientes](roadmap.md)

Ícaro comparte automáticamente sensibilidad, controles y ajustes gráficos entre las instancias. Se aplican antes de iniciar Minecraft y se recogen del `options.txt` que el juego guarda al terminar. La primera sesión aporta las preferencias iniciales; los cambios posteriores pasan a las siguientes instancias que abras.

El archivo global es `Icaro/opciones-minecraft.json`, separado de las opciones y de los ajustes de Java de cada instancia. No necesita una pantalla nueva ni cambios en los componentes del System Design.

## Qué se comparte

- Sensibilidad, inversión del ratón y opciones de entrada compatibles.
- Teclas Vanilla y teclas de mods que ya tengan una acción registrada en el `options.txt` del destino.
- FOV, distancia de renderizado y simulación, límite de FPS, VSync, modo gráfico, iluminación, nubes, partículas, escala de interfaz y otros efectos gráficos compatibles.

Cada instancia conserva su selección de resourcepacks, idioma, sonidos, archivos de shaders y opciones ajenas a esta lista. No se copia todo el archivo. Un ajuste ausente o con un tipo diferente se conserva en su formato local. Las teclas con códigos numéricos antiguos y las teclas modernas mantienen preferencias separadas para evitar aplicar una codificación incorrecta.

Una instancia nueva recibe opciones básicas y teclas Vanilla utilizando la versión de datos de su propio cliente oficial. Las teclas de mods empiezan a participar cuando Minecraft las registra en su archivo de opciones. Si el cliente no contiene metadatos suficientes, se conserva el primer inicio del juego y la mezcla comienza cuando existe su `options.txt`.

## Guardado y sesiones simultáneas

Se mezclan cambios por opción: cerrar una instancia que conserva su sensibilidad anterior no revierte una sensibilidad modificada desde otra sesión. Si ambas cambian la misma opción, prevalece el último cierre que guarde ese cambio. Las instancias que ya están ejecutándose reciben las preferencias nuevas en su siguiente inicio.

La mezcla conserva la versión de datos y las líneas ajenas de cada archivo. Los archivos se publican mediante escritura temporal y reemplazo completo. Un archivo global dañado o de esquema desconocido se conserva; se avisa en la consola y Minecraft puede iniciar con sus ajustes locales. Si Java no llega a iniciarse, no se recogen preferencias nuevas.

## Verificación

Las pruebas cubren mezcla de sensibilidad, teclas y gráficos, acciones de mods ausentes, conservación de resourcepacks/idioma, versión propia de una instancia nueva, archivo global dañado, formatos antiguos de teclas y cambios de sesiones simultáneas. El lanzamiento del launcher habilita este recorrido para todas las instancias, incluida la conexión desde Servidores.

```powershell
cargo test --locked -j 1 -p icaro-core opciones_minecraft --lib
```

La comprobación manual cambiando opciones dentro de Minecraft y abriendo otra instancia sigue pendiente.
