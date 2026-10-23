# Integración del launcher

Estado y plan de implementación de la autenticación, la instalación del juego y las tiendas de mods. La guía de [inicio de sesión y catálogo](login-y-catalogo.md) describe cómo probar los servicios conectados.

Actualizado el **22-10-2026**. El [roadmap general](../README.md#roadmap) y el [roadmap del diseño](roadmap-system-design.md) distinguen los recorridos conectados de las maquetas y pendientes.

## Cuentas sin conexión y skins locales (0.2.5)

Invitado se sustituye por perfiles locales guardados con identidad estable por nombre. El selector conserva Microsoft separado; no se envían credenciales locales a sus servicios. El componente SkinSinConexion carga PNG, muestra frente/espalda, permite elegir modelo y actualiza los avatares. El lanzamiento con skin local usa un servicio firmado en loopback y un agente oficial verificado por SHA-256. Las pruebas cubren persistencia, protocolo y arranque del agente con Java; queda pendiente comprobar la skin en una partida real y en servidores. Ver [alcance y almacenamiento](cuentas-sin-conexion.md).

Consulta el [alcance de Forge, NeoForge y Fabric API](loaders-forge-neoforge.md) para los formatos admitidos y la validación pendiente.

## Estado actual

| Parte | Implementación actual |
| --- | --- |
| Núcleo | `crates/core/src/manifest.rs` consulta el manifiesto de Mojang y verifica el SHA-1 del JSON de una versión. |
| Inicio de sesión | `icaro-core::auth` usa PKCE, Xbox, titularidad y perfil de Minecraft. La interfaz ofrece un único acceso por navegador. Windows vuelve por defecto mediante `icaro://oauth/microsoft`, registrado por el instalador y por el launcher al iniciar el acceso. El retorno directo se probó anteriormente con una cuenta real; Linux conserva el retorno local. Sin cuenta activa aparece la bienvenida. La API de código de dispositivo se conserva sin acceso desde la interfaz real. |
| Tienda | `icaro-core::modrinth` consulta búsqueda, fichas, versiones y creadores reales. `icaro-core::mods` descarga mods y dependencias obligatorias, verifica tamaño y SHA-512 y prepara las actualizaciones antes de sustituir los archivos. Resourcepacks y shaders ZIP cuentan con inventario, instalación, activación, actualización y eliminación reales. `ICARO_DEMO=1` conserva el catálogo estático. Los MRPACK de Modrinth se revisan e instalan en nuevas instancias Vanilla/Fabric/Forge/NeoForge, con archivos opcionales, configuración incluida y hashes verificados; la importación local está conectada y Quilt sigue pendiente. |
| Credenciales | `icaro-core::vault` recuerda una cuenta mediante Credential Manager en Windows o Secret Service en Linux. Si no puede guardar, la sesión queda solo en memoria. |
| Instancias | Selectores separados de Minecraft y loader con catálogos oficiales; guardan la elección, nombre y portada en una carpeta propia. Crear Vanilla/Fabric/Forge/NeoForge prepara Minecraft, el perfil oficial y Java. Cambio de versión con revisión de todos los mods, actualizaciones opcionales, respaldo/copia y recuperación ante fallos. Vanilla omite Mods; el editor renombra la carpeta legible, conservando el identificador y recuperando la ruta anterior si falla el guardado. Cambiar versión conserva la familia del loader; cambiar únicamente su versión tiene una acción separada. Quilt sigue pendiente; Forge/NeoForge requieren aún validación de partida y migración. |
| Persistencia | `launcher_aux/estado.rs` guarda grupos y ajustes en `%APPDATA%/Icaro/estado.json` en Windows o en el directorio de datos de Linux. El modo de demostración usa `icaro-demo/estado.json`, dentro del directorio temporal. No es un almacén de credenciales. |
| Juego | Vanilla se descarga y verifica desde Mojang, detecta o instala Java oficial y se ejecuta con la cuenta y ajustes de la instancia. Cola, Umbral, consola y fallo reciben estados/registros reales; las capturas se leen del disco. El usuario confirmó el lanzamiento en Windows. Fabric 26.3 fue confirmado por el usuario con un mod real en Windows. Quilt sigue pendiente. Forge y NeoForge preparan sus clientes con instaladores oficiales; faltan partida con mods, migración y recorrido completo en Linux. |
| Exportación | `icaro-core::exportar` genera ZIP Ícaro o Prism/MultiMC con metadatos y archivos seleccionados; el diálogo nuevo muestra el árbol, presets y selector nativo. ExportarListaMods produce texto en seis formatos y consulta metadatos sin instalar. La exportación MRPACK sigue pendiente; la instalación de MRPACK de Modrinth ya está conectada para Vanilla/Fabric/Forge/NeoForge. |
| Actualización | `icaro-core::actualizacion` consulta releases, valida el manifiesto y descarga por tamaño/SHA-256. Portable o setup completo silencioso según la instalación, ahora o al cerrar en Windows x86-64. Progreso, preparación y resultado en la misma pantalla original, con regreso después del reinicio. Falta publicar los artefactos y probar una actualización completa; ver [contrato](actualizaciones-launcher.md). |
| Servidores y red | Lista y PanelLateral reales, orden por arrastre, consulta Minecraft/IP/DNS/SRV y lanzamiento por perfil. ZeroTier detectado y solicitud de pertenencia mediante CLI. SincronizarMods conectado al servicio explícito del anfitrión con versiones/hashes de Modrinth, preparación y recuperación; prueba HTTP con archivos reales. Faltan instalación/elevación de ZeroTier, salida/miembros y validación del juego entre equipos Windows/Arch. Ver [alcance pendiente](pendientes-implementacion.md#alcance-de-servidores-zerotier-y-sincronización). |

La aplicación utiliza el Client ID público aprobado. Windows solicita `icaro://oauth/microsoft` como retorno predeterminado; Linux conserva `http://localhost`. Ambas URI deben estar registradas en Azure para aplicaciones móviles y de escritorio. La prueba manual anterior en Windows confirmó la autorización con una cuenta personal de Microsoft y el retorno directo al launcher.

Las instancias se recuperan desde el disco sin consultar Microsoft. Cada una tiene un identificador estable y una carpeta en `Icaro/instances/<nombre-legible>`; los grupos se asocian a ese identificador. Los ajustes Java/memoria se guardan en instance.json de esquema 2 con herencia global; .icaro contiene el perfil y los nativos privados, mientras assets y bibliotecas se comparten en la raíz. El catálogo de versiones se conserva en caché durante 24 horas y una copia válida permite elegir versiones si la actualización de red falla. La guía de inicio de sesión incluye el procedimiento para probar esta primera etapa.

## Inicio de sesión

La página de vinculación ofrece **Continuar con Microsoft**, que abre la autorización en el navegador. El bloque de código y QR se retiró porque el enlace del proveedor redirigía al panel de cuenta en el navegador probado. El núcleo conserva las solicitudes y la espera del código de dispositivo para una integración posterior. El código fijo y la confirmación simulada se conservan únicamente con `ICARO_DEMO=1`; no autentican cuentas reales.

El botón utiliza PKCE S256, comprobación de `state` y retorno a `icaro://oauth/microsoft` en Windows. El protocolo se registra para el usuario actual; el proceso activado entrega el código al receptor interno del intento original, que valida el `state` y conserva el verificador PKCE. El navegador no abre una página local y no hace falta una variable de prueba. Linux mantiene el retorno a `http://localhost`; el receptor se cierra al completar, cancelar o vencer. La guía de inicio de sesión detalla la configuración. El navegador del sistema recibe la contraseña; Ícaro recibe la autorización. Una aplicación de escritorio no debe incorporar un secreto de cliente. Véase [flujo de autorización de Microsoft](https://learn.microsoft.com/en-us/entra/identity-platform/v2-oauth2-auth-code-flow) y [configuración de las URI de redirección](https://learn.microsoft.com/en-us/entra/identity-platform/how-to-add-redirect-uri).

La autenticación comprueba Xbox, la titularidad de Minecraft Java y el perfil del juego antes de activar la cuenta. El 11 de octubre de 2026 se confirmaron el retorno local y el directo a Ícaro con una cuenta real en Windows. Después de corregir el formato del almacén de Windows para admitir credenciales más largas, se confirmó la recuperación al reabrir el launcher. También se verificó la lectura del formato anterior con una entrada aislada. La interfaz muestra el progreso de las comprobaciones y vuelve al frente al recibir la autorización; esta presentación queda pendiente de comprobación manual. Faltan las pruebas manuales de renovación y eliminación al cerrar sesión y la validación en Linux.

## Seguridad que debe implementar el núcleo

- Almacenar la credencial de renovación mediante el almacén del sistema, por cuenta: Credential Manager en Windows y Secret Service en Linux. Si no está disponible, ofrecer una sesión sin persistencia. Nunca recurrir a JSON sin protección. [API de credenciales de Windows](https://learn.microsoft.com/en-us/windows/win32/api/wincred/nf-wincred-credwritew).
- Mantener secretos fuera de mensajes de interfaz, serialización de ajustes, `Debug`, errores, URLs, registros y reportes de fallos. Reducir copias y limpiar memoria cuando sea posible.
- Usar clientes HTTP separados para identidad, catálogo y archivos. Restringir destinos de credenciales; bloquear redirecciones de autenticación; conservar la validación TLS y aplicar límites de tiempo y tamaño.
- Cancelar solicitudes al salir o cerrar sesión, descartar respuestas de sesiones anteriores y borrar la credencial persistida al cerrar sesión. Renovar sin duplicar peticiones concurrentes.
- Nunca registrar la línea de lanzamiento con el token de Minecraft. El juego necesita recibir su credencial: hay que revisar por separado la exposición local de argumentos y los registros del proceso.

Estas medidas reducen exposición accidental. El almacén del sistema no garantiza aislamiento frente a un programa malicioso ejecutado con la misma identidad del usuario. Los hashes verifican integridad; no demuestran que un mod sea inocuo.

## Modrinth

El catálogo público usa la API documentada y un cliente independiente del login. No recibe el token de Minecraft. Identifica Ícaro mediante `User-Agent`, respeta la espera recibida con HTTP 429 y trata errores sin bloquear la interfaz. [Descripción de la API](https://docs.modrinth.com/api/).

El `User-Agent` debe incluir el nombre, la versión de Ícaro y un enlace al proyecto, por ejemplo `Icaro/0.1.0 (+https://github.com/00faithlss/Icaro-launcher)`.

La búsqueda usa texto, tipo de proyecto, versión de Minecraft y loader. Se pagina y se descartan respuestas antiguas al cambiar filtros. La selección final de versión vuelve a comprobar compatibilidad, porque encontrar un proyecto no garantiza que cualquier archivo sirva para la instancia. [Búsqueda](https://docs.modrinth.com/api/operations/searchprojects/) y [versiones](https://docs.modrinth.com/api/operations/getprojectversions/).

Antes de descargar, resolver dependencias obligatorias y conflictos; después verificar tamaño y SHA-512, escribir en un archivo temporal y publicar el resultado en la instancia. Validar nombres, rutas y archivos comprimidos para impedir escrituras fuera del destino. Guardar los IDs estables de proyecto y versión junto al archivo instalado para actualizaciones y reconocimiento posteriores.

Los modpacks crean instancias desde sus manifiestos; los mods se instalan dentro de cada instancia.

### Recursos, shaders y cambio de versión

Los resourcepacks no se filtran por el loader de mods. Un ZIP de recursos debe contener `pack.mcmeta`; si no anuncia la versión de Minecraft de la instancia, se permite instalarlo después de confirmar. Su activación se guarda en `options.txt`. Los shaders deben contener archivos bajo `shaders/` y usan la configuración existente de Iris; solo uno queda activo. Otros motores y la prioridad de resourcepacks por arrastre siguen pendientes.

`versiones_instancia` identifica todos los mods instalados mediante SHA-512 y consultas por lotes, incluidos los desactivados. El selector calcula compatibilidad en segundo plano con caché, sin verificar Minecraft al dibujar filas. Permite escoger la versión de Minecraft conservando la familia del loader, revisar actualizaciones y desactivaciones, respaldar mundos y crear una copia. Al aplicar, prepara mods/dependencias en una carpeta aislada, verifica los originales antes de sustituirlos y recupera archivos y metadatos si falla. Conserva los mods desactivados; el respaldo es obligatorio al bajar de versión. Fabric se prepara con el perfil oficial y la versión guardada; Forge y NeoForge usan sus instaladores oficiales y caché aislada verificada; Quilt sigue pendiente. Cambiar únicamente la versión del loader conserva Minecraft y los archivos de la instancia.

`InfoProyecto` y `PaginaMod` usan datos publicados por Modrinth y los componentes del ZIP nuevo. Las métricas internas de MRPACK, mínimos/rendimiento de shaders y razones no publicadas de incompatibilidad se omiten. El perfil interno del creador requiere un componente de diseño aún no entregado; sus enlaces abren Modrinth.

## CurseForge

La integración es técnicamente posible, pero necesita una clave de desarrollador y respetar la distribución autorizada por cada proyecto. [Solicitud de acceso](https://support.curseforge.com/support/solutions/articles/9000208346-about-the-curseforge-api-and-how-to-apply-for-a-key) y [API](https://docs.curseforge.com/rest-api/).

El [anuncio del 10 de junio de 2026](https://blog.curseforge.com/introducing-api-key-authentication-for-curseforge-file-downloads/) fija el 16 de julio como fecha de exigencia de clave en el CDN. Propone transmitirla mediante cabecera para reducir su aparición en registros e historial.

No publicar la clave en Git ni asumir que incluirla en el ejecutable la mantiene privada. Antes de elegir el mecanismo de distribución o un servicio intermediario, confirmar con CurseForge las condiciones aplicables a Ícaro: sus [términos públicos](https://support.curseforge.com/support/solutions/articles/9000207405-curseforge-3rd-party-api-terms-and-conditions) contienen restricciones de divulgación, caché y acceso mediante intermediarios. Un proxy no es una solución automáticamente autorizada. Mantener este proveedor opcional y explicar cuando un archivo requiere descarga manual.

## Orden y condiciones de aceptación

| Entrega | Comprobación necesaria |
| --- | --- |
| Autenticación y credenciales | Cuenta real, cancelación, rechazo, caducidad, renovación, reinicio y cierre de sesión; un secreto de prueba no aparece en archivos ni registros. |
| Instancia Vanilla | Instalar y lanzar con una cuenta válida; verificar archivos, Java, argumentos y aislamiento entre instancias. |
| Loader y Modrinth | Instalar una versión compatible con dependencias reales; probar hash incorrecto, red interrumpida, falta de espacio y HTTP 429. |
| Modpacks | Importar manifiestos y rechazar rutas peligrosas, archivos desproporcionados y conflictos sin dejar una instalación parcial activa. |
| CurseForge | Acceso y distribución acordados, protección de clave y tratamiento claro de descargas restringidas. |

La implementación debe situar los servicios en `icaro-core` y usar iced para presentar su estado y ejecutar trabajo asíncrono. Los ejemplos de diseño deben mantenerse separados del ejecutable de producción para impedir que una cuenta o descarga simulada se confunda con una real.

La cuenta y el estudio originales están conectados a perfiles y PNG reales; ver [cuenta](cuenta.md). Los modpacks reutilizan la tienda original y exigen una elección explícita de versión antes de revisar el MRPACK. Las actualizaciones distinguen compilaciones de la misma versión y usan el instalador completo si el ejecutable pertenece a la instalación registrada.
