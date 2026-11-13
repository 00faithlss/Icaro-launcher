# Movimiento

[Documentación](README.md) · [Estado y pendientes](roadmap.md)

Comportamiento de las animaciones del ejemplo `launcher`. La lógica está en [movimiento.rs](../crates/ui/src/movimiento.rs), el dibujo de los efectos en [efectos.rs](../crates/ui/src/efectos.rs) y los grabados en [laminas.rs](../crates/ui/src/laminas.rs). Las duraciones se definen en [tema.rs](../crates/ui/src/tema.rs) y en el [launcher](../crates/ui/examples/launcher.rs).

## Animaciones activas

| Efecto | Dónde se dispara | Comportamiento |
| --- | --- | --- |
| Entrada del contenido | Al cambiar de sección o pestaña | Fundido de 320 ms. En el editor de instancia y en la página de un mod se aplica al contenido de la pestaña. |
| Umbral | Botón Jugar en tarjetas, pie, editor y menú contextual | Secuencia de luz y registro de arranque. |
| Picar | Mantener Supr sobre la instancia seleccionada | Una etapa de grietas cada 120 ms, hasta 10 etapas. Soltar retira dos etapas cada 40 ms; la rotura dibuja 48 escombros. |
| Colocar | Tarjeta restituida al deshacer una eliminación | Aro de 2 px durante 120 ms. |
| Losa | Apertura de diálogos | Desplazamiento vertical de 280 ms. |
| Menús y panel lateral | Apertura del menú contextual o del panel de instancia | Entrada del menú en 160 ms y del panel en 240 ms. |
| Avisos | Mensajes temporales | Duración de 3 segundos, con fundido en los últimos 180 ms. Un nuevo mensaje sustituye al anterior. |

## Grabados y visor

Los grabados se muestran sin animación de revelado ni zoom al pasar el puntero. Las utilidades Prensa y Lupa permanecen en la biblioteca de efectos, pero no se aplican a los grabados del launcher.

El visor se abre y cierra de inmediato, con fondo opaco y bloqueo de interacción con el contenido de detrás. Las capturas usan un marco 16:9 y los grabados conservan su proporción original. Las flechas recorren las imágenes y los controles de zoom cambian el aumento en pasos de 25 %.

## Reducir movimiento

El ajuste del launcher se comunica con `movimiento::fijar_reducir`. Con la opción activa:

- El Umbral usa un fundido a `fulgor` de 120 ms y muestra el registro completo.
- Picar conserva las grietas y omite los escombros.
- El contenido, los menús, el panel lateral y los diálogos aparecen sin desplazamiento ni fundido de entrada.
- Los avisos mantienen su duración de 3 segundos y se retiran sin fundido.

## Comprobación manual

```sh
cargo run -p icaro-ui --example launcher
```

Variables de entorno para abrir estados concretos:

| Variable | Efecto |
| --- | --- |
| `ICARO_UMBRAL=1` | Abre con el Umbral en marcha. |
| `ICARO_PICAR=7:1` | Detiene la eliminación de la instancia de índice 1 en la etapa 7. |
| `ICARO_PANTALLA=fallo` | Muestra el informe de fallo con un grabado lateral. |

Las pruebas de la lógica de movimiento se ejecutan con `cargo test -p icaro-ui`.
