# Cuentas sin conexión

[Documentación](README.md) · [Estado y pendientes](roadmap.md)

Implementación local basada en `icaro-cuenta-sin-conexion.zip`, con sus componentes CuentaSinConexion, SelectorCuenta, EstadosSesion, PrimeraEjecucion y PantallaCuenta. Sustituye el acceso anónimo de Invitado por perfiles con nombre propio. Conserva los temas Tinta/Piedra, la cabeza rayada y la iconografía del diseño, incluido `info-box`.

## Uso

- En la primera ejecución o el acceso de Microsoft, elige **Usar cuenta sin conexión**. También está disponible en el selector como **Añadir cuenta sin conexión**.
- Escribe entre 3 y 16 caracteres: letras ASCII, números o `_`. El formulario muestra el contador y el motivo de un nombre inválido; no admite nombres guardados repetidos, ignorando mayúsculas al comprobar duplicados.
- **Guardar y usar cuenta** crea y selecciona el perfil. Mientras guarda, el campo y las acciones quedan bloqueados, incluido Enter. Un error conserva el nombre para reintentar.
- Las cuentas guardadas aparecen en el componente y el selector. **Usar** selecciona otra; **Eliminar** pide confirmación en la misma fila. Quitar la activa selecciona la siguiente disponible o intenta recuperar Microsoft. No borra mundos, inventarios, mods ni instancias.
- La barra lateral muestra el nombre y **Sin conexión**. Cada cuenta ofrece **Skin** para elegir una imagen local; sin imagen conserva la cabeza rayada. Los servicios de cuenta, capas y cambios de nombre de Microsoft permanecen separados.

Cada nombre deriva un UUID v3 de `OfflinePlayer:<nombre>`, compatible con la identidad local de Minecraft. Mayúsculas y minúsculas forman parte de la identidad: cambiar el nombre puede mostrar otro inventario o progreso; el anterior sigue asociado al nombre original.

## Almacenamiento y alcance

Los nombres y la selección activa se escriben de forma atómica en `accounts/locales.json`, dentro de la raíz de datos del launcher. No contiene tokens ni contraseñas. Microsoft mantiene su almacén de credenciales del sistema; activar una cuenta local no elimina esas credenciales. Un archivo local dañado se conserva y se muestra el error.

La identidad local permite ejecutar el juego en mundos locales y servidores que admiten cuentas sin conexión. No autentica ante los servicios de Microsoft/Mojang ni sustituye la sesión exigida por servidores con autenticación. La primera instalación y las descargas de juego/mods siguen necesitando conexión; «sin conexión» describe la cuenta, no garantiza que todas las operaciones del launcher funcionen sin red.

No cambiar de cuenta mientras Minecraft se ejecuta, se prepara un lanzamiento o se está subiendo una skin. El launcher conserva la cuenta seleccionada si falla el guardado y descarta resultados de operaciones anteriores.

## Skin local

El panel `SkinSinConexion`, basado en `icaro-skin-sin-conexion.zip`, permite arrastrar un PNG o elegirlo por archivo. Acepta 64 × 64 y 64 × 32, hasta 1 MB; normaliza el formato antiguo y rechaza imágenes vacías o dañadas. Muestra frente/espalda, selección Clásico/Delgado y una casilla de capa exterior que solo modifica la vista previa. Un error conserva la imagen anterior. **Guardar skin** actualiza las cabezas de la lista, selector y barra lateral; se aplica al siguiente inicio del juego. **Quitar skin** vuelve al aspecto predeterminado.

Los PNG normalizados se guardan por SHA-256 en `skins/`; el modelo y la referencia se publican atómicamente en `accounts/locales.json`. Quitar la skin o la cuenta elimina su referencia, conservando las imágenes en caché. No se modifican mundos ni archivos del modpack.

Para mostrarla en el juego se inicia un servicio de texturas firmado, limitado a `127.0.0.1` y a la duración de la partida. Solo las cuentas locales con skin guardada reciben el argumento `-javaagent`; las cuentas Microsoft no lo reciben. La primera vez requiere descargar [authlib-injector 1.2.8, compilación 56](https://authlib-injector.yushi.moe/artifact/56/authlib-injector-1.2.8.jar), sin modificaciones, comprobando el SHA-256 fijado en el código. Las siguientes ejecuciones reutilizan la copia verificada. Su [código y licencia AGPL-3.0](https://github.com/yushijinhun/authlib-injector) están disponibles en el proyecto original. La integración sigue sus [especificaciones del launcher](https://yushijinhun.github.io/authlib-injector/en/launcher-technical-specification.html) y del [servidor Yggdrasil](https://yushijinhun.github.io/authlib-injector/en/yggdrasil-server-technical-specification.html).

Tú la ves en tus mundos; los demás jugadores dependen del soporte de skins del servidor. El servicio local solo entrega perfiles y texturas: no autentica cuentas ni acepta `join` o `refresh`.

## Validación

Pruebas de UUID conocido, nombres inválidos y duplicados, persistencia/restauración, eliminación con conservación de archivos, archivo corrupto, argumentos locales/Microsoft, PNG, firma del perfil, textura servida y rechazo de autenticación. Las capturas de diseño usan datos aislados, sin guardar cuentas de usuario. La prueba ignorada `agente_oficial_verificado_arranca_en_java` descarga y verifica el agente, comprueba su arranque con `java -version` y rechaza una caché alterada; requiere red y Java en PATH. No sustituye la validación visual de una partida. El inicio y la conexión real a servidores con estas cuentas deben validarse en el equipo del usuario; las pruebas no acreditan compatibilidad con todos los mods o servidores.
