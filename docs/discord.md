# Detección de Minecraft en Discord

En Windows, Ícaro inicia el juego con `javaw.exe` de la misma carpeta que el `java.exe` validado, si existe. Las comprobaciones de Java y los instaladores conservan el ejecutable de consola. Si falta `javaw.exe`, se utiliza el Java seleccionado. La salida estándar, los errores y la cancelación del juego siguen gestionados por el launcher.

El [catálogo público de Discord](https://discord.com/api/v9/applications/detectable), consultado el 23-10-2026, identifica Minecraft en Windows mediante `javaw.exe` y el argumento `net.minecraft.client.main.Main`. La elección del ejecutable corrige una diferencia con ese criterio, pero los loaders pueden usar otras clases principales y la detección final depende de Discord. Esto no añade Rich Presence ni comunica cuentas, servidores o instancias a Discord.

El usuario confirmó la detección visible de Minecraft en Discord con este cambio el 23-10-2026; esa comprobación no acredita todos los loaders y versiones. Para comprobar otra combinación, abrir Discord de escritorio y Minecraft desde Ícaro. Revisar que esté habilitada la actividad y que Minecraft aparezca en Juegos registrados. Si no se detecta, Discord permite añadir el proceso abierto desde esa sección; consultar su [guía de actividad](https://support.discord.com/hc/en-us/articles/7931156448919-Activity-Sharing-on-Discord-FAQ).

La prueba `lanzamiento::tests::javaw_real_conserva_stdout_stderr_y_cancelacion` comprueba ambos canales de registros y la terminación al cancelar con un JDK real. Es una prueba ignorada, activable con `ICARO_PRUEBA_JAVA` apuntando a su `bin/java.exe`; no abre Minecraft ni modifica Discord.
