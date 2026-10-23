# Inicio de sesión y catálogo

## Compatibilidad del navegador en Windows

La apertura del inicio de sesión usa `ShellExecuteW` y la asociación HTTPS del sistema. Windows activa el navegador predeterminado, incluidos los que usan `DelegateExecute`, sin reconstruir ni ejecutar su comando del registro. El enlace completo conserva los parámetros PKCE y el retorno `icaro://oauth/microsoft`.

Validación pendiente de esta corrección, antes de publicar:

1. Compilar y ejecutar las comprobaciones de Windows indicadas en `AGENTS.md`.
2. Probar Dia, Edge, Chrome y Firefox como predeterminados de HTTPS, tanto cerrados como ya abiertos. Cada intento debe abrir Microsoft y regresar al launcher tras autorizar.
3. Cancelar un intento y comenzar otro; el retorno anterior no debe autenticar el intento nuevo. Probar también el rechazo del consentimiento y el vencimiento.
4. Comprobar que una asociación HTTPS ausente o dañada muestra el error de apertura, y que Linux conserva su apertura y retorno locales.

Las pruebas de construcción del enlace no acreditan la apertura real ni la autenticación. El éxito del shell solo confirma que aceptó la solicitud; la sesión se confirma al recibir y verificar la respuesta de Microsoft.

El ejemplo `launcher` conecta Microsoft, guarda instancias, instala Minecraft Vanilla/Fabric y prepara Java según el perfil, ejecuta el juego, consulta Modrinth, instala y actualiza mods compatibles y exporta ZIP. Fabric y los MRPACK de Modrinth Vanilla/Fabric están conectados; Quilt, Forge y NeoForge siguen pendientes. Las secciones usan datos reales; los ejemplos se conservan únicamente con `ICARO_DEMO=1`. El recorrido del juego se describe en [Vanilla y Java](vanilla-y-java.md).

## Iniciar sesión

```sh
cargo run -p icaro-ui --example launcher
```

1. Al abrir Ícaro sin cuenta activa aparece la bienvenida. Pulsa **Iniciar sesión**; también puedes acceder desde el menú de cuenta.
2. Pulsa **Continuar con Microsoft** en la página de vinculación. Se abre el navegador para elegir una cuenta y autorizar a Ícaro.
3. Autoriza la vinculación con Ícaro desde Microsoft. En Windows, acepta **Abrir Icaro Launcher** si el navegador muestra esa confirmación.
4. Cuando Microsoft devuelve la autorización, Ícaro vuelve al frente y muestra el progreso: validar la sesión, comprobar Minecraft Java, obtener el perfil y guardar la cuenta. La cuenta aparece activa al terminar; no requiere pulsar una confirmación en el launcher.

El Client ID público de Ícaro está incluido en el núcleo. El registro de Azure debe admitir cuentas personales de Microsoft. No se usa un secreto de cliente.

El acceso utiliza PKCE S256 y comprobación de `state`. En Windows vuelve por defecto a `icaro://oauth/microsoft`; no abre una página de `localhost` en el navegador ni necesita una variable de entorno. El intento vence a los diez minutos. La comprobación de la cuenta continúa en Ícaro.

El código y el QR se retiraron del acceso real: en el navegador probado, el enlace del proveedor enviaba al panel de cuenta de Microsoft en lugar de completar la vinculación. El núcleo conserva la API de [código de dispositivo](https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-device-code), cuya emisión real se comprobó, para una integración posterior.

### Volver directamente a Ícaro en Windows

Este retorno usa `icaro://oauth/microsoft`, con el mismo mecanismo de enlace propio del [retorno nativo de Prism](https://github.com/PrismLauncher/PrismLauncher/blob/develop/launcher/minecraft/auth/steps/MSAStep.cpp). Al abrirlo, el navegador puede pedir permiso para iniciar Ícaro; puede recordar una autorización anterior. Las pantallas de consentimiento y confirmación de identidad pertenecen a Microsoft y pueden variar según la cuenta y los permisos previos. Ícaro no controla su texto ni fuerza su aparición en cada acceso.

> [!IMPORTANT]
> El registro público de Ícaro en Azure debe incluir exactamente `icaro://oauth/microsoft` en **Autenticación → Aplicaciones móviles y de escritorio**. Una instalación del launcher no puede modificar ese registro remoto. `http://localhost` se conserva para el acceso en Linux. [Configuración de las URI de redirección](https://learn.microsoft.com/en-us/entra/identity-platform/how-to-add-redirect-uri).

El instalador registra el protocolo para el usuario actual de Windows, sin permisos de administrador. Al pulsar **Continuar con Microsoft**, el launcher también comprueba y registra el ejecutable actual, lo que permite el retorno en compilaciones portables y de desarrollo. No cambia automáticamente a una página local si no puede registrar el protocolo: muestra el error en Ícaro.

La respuesta abre un proceso breve que entrega el código al intento original y se cierra sin crear otra ventana. Solo ese intento conserva el verificador PKCE y acepta su `state`; los tokens de sesión no se transmiten mediante el enlace. El receptor loopback queda como comunicación interna entre los dos procesos y responde sin HTML; el navegador vuelve al protocolo de Ícaro.

`ICARO_APP_RETURN` ya no selecciona el acceso: no es necesaria y se ignora si quedó definida de una prueba anterior. El retorno directo se comprobó anteriormente con una cuenta real en Windows; este cambio lo hace predeterminado. En Linux continúa el retorno local mientras se completa el registro del protocolo nativo para esa plataforma.

**Cancelar** interrumpe el intento. Si el navegador permanece abierto, su autorización posterior no activa la cuenta en Ícaro. Ante un error se muestra la etapa o un mensaje específico, sin copiar la descripción del proveedor ni sus credenciales.

## Sesión recordada

La primera integración admite una cuenta activa y recordada por equipo. La credencial de renovación se guarda mediante Credential Manager en Windows y Secret Service en Linux, separada de los ajustes del launcher. Al volver a abrir Ícaro se renueva la autorización y se comprueba que el perfil corresponda a la cuenta guardada.

El progreso de comprobación aparece en cada apertura porque se recupera una credencial de renovación, no una sesión activa sin verificar. Esa recuperación no solicita otra autorización en el navegador. Las instancias y sus ajustes se leen por separado desde el disco.

Durante la recuperación, la bienvenida indica que la sesión se está comprobando. Si se recupera una cuenta, el launcher abre Instancias; si no hay credencial o la recuperación falla, mantiene la bienvenida y muestra el error cuando corresponda. **Usar cuenta sin conexión** abre el formulario para guardar un nombre local y jugar en mundos o servidores que lo admitan. Sustituye Invitado y conserva las credenciales Microsoft por separado; consulta [cuentas sin conexión](cuentas-sin-conexion.md). Si hay una cuenta local activa al iniciar, se restaura directamente sin renovar Microsoft.

En Windows, la credencial se guarda como bytes UTF-8 para evitar duplicar el tamaño del token al convertirlo a UTF-16. También se leen registros UTF-16 de versiones anteriores. Una prueba con una entrada aislada confirma que una credencial de 2.000 caracteres, rechazada por el formato anterior, se puede guardar y recuperar con el nuevo. La recuperación al reabrir el launcher se confirmó también con una cuenta real después de esta corrección.

Si el almacén del sistema no está disponible o no permite guardar, la cuenta funciona durante esa apertura y la interfaz indica que la sesión no quedó recordada. No se escribe una copia de la credencial en archivos JSON. En Linux se requiere un servicio de secretos disponible en la sesión y `libdbus` para compilar este backend.

**Cerrar sesión** cancela solicitudes, retira la cuenta activa y elimina la credencial guardada. Si la eliminación falla, la interfaz lo indica y permite reintentar. Este cierre es local: la sesión del navegador de Microsoft se administra desde el navegador.

La barra inferior de **Jugar** aparece en la lista de instancias cuando hay una cuenta activa y una instancia seleccionada. Muestra el nombre y la versión de esa instancia. Al abrir el launcher no se selecciona una instancia por defecto; las pantallas de vinculación, cuenta y otros pasos tampoco muestran la barra.

La autorización se renueva antes de vencer, con una sola operación de autenticación en curso. Los intentos antiguos no pueden guardar credenciales ni sustituir la cuenta después de una cancelación o cierre de sesión.

## Instancias

**Crear instancia** utiliza los cuatro pasos del asistente existente: versión de Minecraft, loader, nombre y portada, y revisión. Las versiones proceden del manifiesto de Mojang, con lanzamientos, snapshots y versiones antiguas. La consulta se realiza al abrir el asistente; su caché se reutiliza durante 24 horas. Si falla una actualización, se conserva una copia válida anterior. Si todavía no existe una copia y no hay conexión, cierra y vuelve a abrir el asistente para reintentar.

El asistente guarda la versión y familia de loader. Una instancia Vanilla nueva descarga y verifica Minecraft y prepara su Java; con una cuenta activa, Jugar ejecuta el proceso. Las instancias con loader quedan **Por instalar** y permiten descargar mods compatibles, mientras su instalación y lanzamiento siguen pendientes. Exportar ZIP guarda el contenido elegido; importar, duplicar, exportar MRPACK y cambiar de versión siguen pendientes en el modo real.

Cada instancia tiene un identificador propio y guarda sus metadatos en `%APPDATA%/Icaro/instances/<id>/instance.json` en Windows; sus archivos de juego pertenecen a la subcarpeta `minecraft`. Cambiar el nombre desde el editor no cambia esa carpeta. El nombre se guarda al salir del editor o cerrar desde el botón del launcher. Los grupos y ajustes de Java se guardan en `estado.json`, asociados al identificador de la instancia. Las instancias cuyo archivo de metadatos no se pueda leer se omiten de la lista y se notifica el problema, conservando sus archivos en disco.

**Eliminar** desde el menú confirma el nombre de la instancia elegida. La carpeta completa se envía a la papelera del sistema y se retira de la lista activa. **Deshacer** recupera la última eliminación de esa apertura, con sus archivos, grupo y ajustes de Java. Mantener Supr sobre una instancia seleccionada conserva la animación de eliminación y utiliza la misma papelera. Al reiniciar, las instancias eliminadas permanecen fuera de la lista; sus carpetas siguen en la papelera del sistema. Si la operación falla, la instancia continúa en la lista y se muestra el error.

Para probar el guardado, crea dos instancias con nombres y portadas distintos, cambia el nombre de una desde el editor y vuelve a Instancias. Cierra Ícaro y ábrelo otra vez: deben aparecer las mismas instancias y sus ajustes de Java y grupos. **Abrir carpeta** debe mostrar una carpeta distinta para cada una. La comprobación de Microsoft al abrir es independiente de esta carga local.

## Modrinth

La sección **Modpacks** reutiliza `TiendaMods`: búsqueda, categorías, Minecraft, loader, orden y resultados en lista. **Elegir versión** abre la tabla del proyecto; cada fila instala su versión exacta en una instancia nueva, sin elegir automáticamente la última. **Agregar mods** abre `TiendaMods`, con los filtros laterales y la lista de instalación del System Design, limitada a la versión de Minecraft y el loader de la instancia. **Volver a mis mods** regresa a sus archivos instalados. Vanilla no muestra la pestaña ni el contador de mods. Los MRPACK de Modrinth crean una instancia nueva con Minecraft y el loader definidos por el pack; admiten Vanilla/Fabric, revisión de opcionales, verificación y reintento. Los recursos y shaders ZIP se instalan desde sus tiendas. La importación de archivos locales y los demás loaders siguen pendientes.

Las consultas se realizan en segundo plano, con una breve espera al escribir, paginación y descarte de respuestas anteriores. El catálogo de modpacks permite filtrar por versión estable de Minecraft y loader, y ordenar por relevancia, descargas, actualización o fecha de publicación. Las opciones proceden de Modrinth y los filtros se conservan al volver de una ficha. Una respuesta HTTP 429 impone una espera antes de consultar de nuevo.

Las tarjetas muestran el icono, la descripción, el autor, los loaders y las descargas. Los loaders aparecen como insignias identificadas, y toda la tarjeta abre la ficha con las pestañas **Descripción**, **Galería**, **Versiones** y **Cambios**. La cabecera y **Volver al catálogo** permanecen visibles mientras se desplaza el contenido; cambiar de pestaña abre su contenido desde arriba. Los textos del catálogo y la ficha utilizan composición avanzada y fuentes de respaldo del sistema para representar los caracteres de otros idiomas. La columna de información muestra compatibilidad, creadores y organización propietaria cuando corresponde, licencia, fechas, estadísticas, etiquetas y enlaces publicados. La columna lateral conserva el ancho y la posición definidos en el diseño. Los botones de enlaces externos, incluidos perfiles y **Ver en Modrinth**, muestran el icono de salida; el retorno al catálogo utiliza una flecha y las acciones conservan los iconos del sistema de diseño. **Ver en Modrinth** abre la página oficial.

La galería conserva el orden publicado y permite recorrer todas las imágenes con miniaturas y botones anterior/siguiente. Pulsar una imagen o miniatura abre el mismo visor que usan las capturas, con zoom, arrastre, flechas y cierre con Escape. El visor solicita una imagen de mayor resolución al abrirla y cancela la carga al cerrar o cambiar de imagen. Las versiones tienen filtros propios de Minecraft y loader, opción para incluir betas y alphas y paginación de 25 entradas. Estos filtros no alteran los del catálogo. **Ver cambios** abre el changelog de la versión elegida; también se puede cambiar de versión desde esa pestaña. Los changelogs se solicitan individualmente y las respuestas obsoletas se descartan. Las fichas de mods y recursos incluyen las dependencias declaradas por versión.

Las descripciones y los changelogs se representan con controles nativos, títulos, listas, estilos y enlaces, sin ejecutar HTML. Las imágenes externas de esos textos se presentan como enlaces; los iconos e imágenes alojadas en el CDN de Modrinth se cargan dentro de la aplicación. Cuando el creador no publica galería, cambios o dependencias, la ficha muestra el estado vacío correspondiente.

La carga automática de imágenes acepta únicamente HTTPS del CDN de Modrinth, sin redirecciones ni credenciales. Las miniaturas tienen un límite de descarga de 4 MiB; la caché conserva hasta 64 entradas o 32 MiB de píxeles. El visor y **Abrir original** utilizan `raw_url` cuando la respuesta ofrece un original válido del CDN: `url` puede apuntar a una vista previa de 350 píxeles. Si no hay original válido, utilizan `url`. El visor tiene un límite de descarga de 16 MiB y conserva las dimensiones del archivo sin reducirlo a una miniatura. Admite hasta 4.096 píxeles por lado, con un límite de memoria de decodificación de 64 MiB. Mientras carga muestra el estado de espera; si falla, muestra el error. Si falta una imagen de una tarjeta o su formato no se admite, la tarjeta conserva su tamaño y muestra un icono de respaldo.

El catálogo utiliza un cliente HTTP separado del login y no recibe credenciales de Minecraft. CurseForge está pendiente de acceso y condiciones de distribución; no aparece como proveedor conectado.

### Instalar y actualizar mods

En las fichas abiertas desde una instancia, Minecraft y loader quedan fijados a esa instancia; la opción de betas y alphas continúa disponible. Los filtros libres de versión y loader se mantienen en las fichas del catálogo de modpacks.

**Agregar** aparece en la fila y junto a las versiones compatibles. Prepara una lista sin descargar archivos: la barra inferior muestra los mods, sus dependencias y el tamaño. **Instalar** descarga esa lista, incluidas las dependencias obligatorias transitivas. Las opcionales se eligen mediante las casillas de la ficha rápida. Los proyectos ya instalados indican su estado. La pestaña Dependencias obtiene los nombres y permite abrirlos dentro del catálogo. Volver desde una dependencia recupera la ficha anterior, su pestaña y versión seleccionada; volver de una ficha recupera la búsqueda, los filtros y la página del catálogo. **Ver página** abre la ficha completa y Volver recupera el panel rápido.

La instalación comprueba otra vez Minecraft y loader, los requisitos de versiones y los conflictos declarados. Cada archivo `.jar` se verifica por tamaño y SHA-512, con un límite de 128 MiB por archivo. Los archivos se preparan en una carpeta temporal dentro de la instancia y se publican juntos al terminar. Una descarga fallida conserva el conjunto anterior; tampoco se sobrescriben mods gestionados que se hayan modificado a mano. Se conservan los archivos locales que no gestiona Ícaro.

Los nombres e identificadores de proyecto y versión se guardan en `minecraft/mods/.icaro-mods.json`. Al abrir la pestaña se leen los archivos locales y se consultan las actualizaciones compatibles. Los mods desactivados usan `.jar.disabled`; una dependencia obligatoria desactivada se activa si la necesita un mod que se está instalando activo.

**Actualizar todos** aparece cuando hay versiones estables compatibles más nuevas. Antes de actualizar uno o todos los mods, el diálogo pide confirmación porque las nuevas versiones pueden ser incompatibles con otros mods. Los requisitos incompatibles declarados impiden la sustitución; no se puede garantizar compatibilidad entre mods que no declaran sus conflictos. Las versiones beta más recientes no se rebajan automáticamente a una estable anterior.

La lista instalada usa las filas del diseño original, con creador, descripción, icono, estado y versión. Seleccionar varios mods muestra la barra invertida para activar, desactivar, actualizar o quitar. Quitar los mueve a la papelera interna de la instancia y ofrece Deshacer durante diez segundos; no sobrescribe archivos que aparezcan después de la eliminación.

Para comprobarlo, crea una configuración Fabric 1.21.1, abre Mods y busca Reese’s Sodium Options. Agrégalo a la lista y pulsa Instalar: debe incluirlo y añadir Sodium. Reabre la instancia para comprobar la persistencia. La descarga de mods no instala Minecraft ni Fabric.

### Exportar una instancia

Desde el menú de la instancia, **Exportar instancia** abre el diálogo original: formato, casillas de contenido, tamaños y carpeta de destino. Mods y configuración son obligatorios; mundos, opciones, capturas y registros son opcionales. **Cambiar** abre el selector de carpetas del sistema. ZIP contiene `instance.json` y `minecraft/` con los archivos elegidos; usa un nombre nuevo y no sobrescribe exportaciones anteriores. El destino debe quedar fuera de la carpeta de juego.

MRPACK conserva su opción en el diseño, pero su exportación queda desactivada hasta disponer del instalador y de la versión concreta del loader. El modo de demostración no exporta archivos ficticios.

## Atajos

Ctrl+1 a Ctrl+7 abre las secciones de la barra lateral en su orden visible; Ctrl+K abre la búsqueda. Los atajos de navegación permanecen desactivados durante la bienvenida, el login, los diálogos y el visor de imágenes.

## Modo de demostración

Para revisar las pantallas originales sin autenticación ni catálogo de red, inicia el ejemplo con `ICARO_DEMO=1`. En PowerShell:

```powershell
$env:ICARO_DEMO = '1'
cargo run -p icaro-ui --example launcher
Remove-Item Env:ICARO_DEMO
```

Las cuentas, los códigos de dispositivo, las tiendas y las instalaciones de este modo son simulados. Sus controles no autentican ni instalan contenido real.

## Validación

```sh
cargo test --workspace
cargo test -p icaro-ui --example launcher
cargo test -p icaro-ui --example launcher viewer_loads_the_original -- --ignored --nocapture
cargo test -p icaro-core --test catalogo_real -- --ignored
cargo test -p icaro-core --test vault_native -- --ignored
cargo test -p icaro-core windows_reads_legacy -- --ignored
cargo test -p icaro-core --test device_real -- --ignored
```

Las pruebas locales cubren la solicitud de códigos, la espera de confirmación, la ampliación del intervalo, el vencimiento, PKCE, validación del retorno, cancelación, intercambio de credenciales con respuestas controladas, titularidad, perfil, límites de respuestas, redirecciones y escrituras de sesión obsoletas. También comprueban el rechazo de enlaces de retorno ajenos o malformados, la entrega al intento original y las cabeceras de privacidad de la página local. La prueba opcional de catálogo consulta la API pública de Modrinth. `vault_native`, disponible en Windows, escribe, lee y elimina una credencial de prueba en un espacio separado de las cuentas del launcher. `device_real` comprueba que Microsoft emita un código para Ícaro sin imprimirlo, abrir el navegador ni autorizar una cuenta.

El 11 de octubre de 2026 se completaron pruebas manuales del login con retorno local y del retorno directo a Ícaro con una cuenta real en Windows. El retorno directo mostró el permiso del navegador para abrir la aplicación y activó la cuenta. Después de corregir el almacenamiento de credenciales largas, se confirmó también la recuperación de la sesión al reabrir, la bienvenida sin cuenta y la separación de los datos de muestra. Las pruebas controladas comprueban que el progreso siga las etapas de autenticación y que un intento cancelado no actualice la interfaz. El nuevo progreso visual y el arrastre de los bordes de la ventana quedan pendientes de comprobación manual. La solicitud de código real a Microsoft pasó la prueba automatizada; su acceso desde la interfaz quedó desactivado por el problema de redirección observado. También quedan pendientes las pruebas manuales de renovación y eliminación al cerrar sesión, y la comprobación del almacén de Linux.

Para comprobar la recuperación, cierra Ícaro y vuelve a abrirlo: debe recuperar el mismo perfil sin solicitar autorización en el navegador. Para comprobar el cierre de sesión, pulsa **Cerrar sesión** y reinicia: debe volver a la bienvenida si no seleccionas otra cuenta guardada. La renovación automática requiere una prueba con la sesión próxima a vencer.

Las pruebas del catálogo comprueban la combinación de filtros, el descarte de respuestas anteriores, la conservación de filtros al volver, el formato de las descripciones y los límites de imágenes. También cubren el cambio rápido de versión y changelog, la cancelación, los filtros propios de la ficha, la paginación y la carga de una sola página de miniaturas. Las pruebas del visor comprueban navegación, zoom, cancelación y descarte de imágenes recibidas después de cerrar o cambiar de foto, además de conservar mayor detalle sin ampliar imágenes pequeñas. Las consultas públicas con Fabulously Optimized y Sodium verificaron compatibilidad, creadores, organización, galería e historial de cambios. Las nuevas composiciones se revisaron mediante capturas de los widgets reales en Tinta y Piedra, a 1280×800.

Las pruebas automatizadas no modifican las cuentas guardadas del launcher: usan un almacén en memoria o una entrada de prueba aislada en Windows.

La prueba opcional del visor consulta la imagen «FPS in high render distance» de Fabulously Optimized y comprueba que se seleccione el original y que la decodificación conserve sus dimensiones. El 11 de octubre de 2026 verificó una vista previa de 349×185 y un original de 1920×1017.

## Instalación y registros de descargas

Crear Vanilla o Fabric prepara sus archivos oficiales con Java según el perfil. La familia se elige al crear la instancia; sus versiones solo se muestran al seleccionar ese loader. Cambiar versión modifica Minecraft; Cambiar versión del loader modifica únicamente la versión del mismo loader.

Instalar modpack revisa el MRPACK, permite elegir archivos opcionales y crea una instancia independiente con la versión de Minecraft y el loader del pack. La cola permite pausar, cancelar y reintentar. Quitar de Descargas en un registro completado o fallido oculta ese registro; conserva la instancia y sus archivos. El historial todavía no se guarda entre sesiones.

## Servidores

La sección permite guardar, editar, quitar y ordenar servidores por arrastre, consultar Minecraft con DNS SRV y preparar la conexión mediante una instancia vinculada. RedPrivada consulta ZeroTier y solicita pertenencia por ID cuando su CLI tiene permisos. SincronizarMods obtiene el índice del servicio del anfitrión y aplica versiones exactas de Modrinth con revisión previa. Quedan instalación/permisos automáticos de ZeroTier y pruebas entre equipos. Consulta [servidores y ZeroTier](servidores.md) y [sincronización](sincronizacion-servidores.md).
