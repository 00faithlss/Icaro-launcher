# Video de presentación

Composición [HyperFrames](https://hyperframes.heygen.com/) del video que muestra Ícaro en el README. Es el launcher real grabado en uso (`assets/tomas/`), dentro de una ventana que flota sobre la pintura del icono, con un texto breve al pie en cada sección. No tiene sonido. El resultado publicado está en [`docs/media/icaro-launcher.mp4`](../media/icaro-launcher.mp4).

## Regenerarlo

Requiere Node.js 22 o posterior y FFmpeg (`npx hyperframes doctor` lo comprueba).

```sh
cd docs/video
npx hyperframes@0.9.42 lint
npx hyperframes@0.9.42 preview      # vista previa y editor en el navegador
npx hyperframes@0.9.42 render --output renders/icaro.mp4
ffmpeg -i renders/icaro.mp4 -c:v libx264 -preset slow -crf 23 -pix_fmt yuv420p -an -movflags +faststart ../media/icaro-launcher.mp4
```

## Tomas

`assets/tomas/` (no se versiona: pesa unos 40 MB) tiene un clip por sección, de 1920×1040 (la pantalla sin la barra de tareas), ya recortado y acelerado: instancias, crear instancia, descargas, mods, servidores, con amigos, apariencia y cierre. Se grabaron con el launcher compilado en release y `APPDATA` apuntando a una carpeta de prueba, con una cuenta sin conexión e instancias de ejemplo; no contienen datos personales. Compartir no se usó durante la grabación: su identidad vive en el almacén de credenciales del sistema, fuera de esa carpeta.

Los textos de `index.html` (función `texto`) siguen lo que ocurre en cada clip. Si se regraba una toma, hay que revisar esos tiempos con `npx hyperframes snapshot --at <segundos>`.

## Créditos

- Fondo (`assets/fondo-blondel.jpg`) e icono: *La caída de Ícaro*, Merry-Joseph Blondel, 1819 (Musée du Louvre). Fotografía © Marie-Lan Nguyen / Wikimedia Commons, [CC BY 2.5](https://creativecommons.org/licenses/by/2.5/). Adaptación: recorte y reducción de tamaño. [Imagen original](https://commons.wikimedia.org/wiki/File:Fall_of_Icarus_Blondel_decoration_Louvre_INV2624.jpg). La atribución también aparece en el video.
- Fuentes Archivo y Martian Mono, copiadas de `crates/ui/assets/fonts/`, con licencia OFL: [Archivo](../../crates/ui/assets/fonts/OFL-Archivo.txt) y [Martian Mono](../../crates/ui/assets/fonts/OFL-MartianMono.txt).
