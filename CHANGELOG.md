# Changelog

## [Sin publicar]

- Servidores sigue PantallaServidores del System Design: banda de 150 px con «Fase experimental» y tres pestañas (Servidores, Con amigos y Red privada) con una línea que se desliza bajo la activa, contenido que entra con fundido y las flechas izquierda y derecha para cambiar de pestaña. Nada abre un diálogo encima.
- Servidores: buscador y «Agregar servidor», que despliega el formulario en la misma página y prueba la dirección mientras se escribe (jugadores y ping si responde, aviso si está mal escrita o no responde). El servidor guardado aparece primero con un brillo breve. Cada fila muestra jugadores, versión, señal de cuatro barras y Jugar; al tocarla se abre hacia abajo con el mensaje del servidor, los mods, la instancia con que se juega, el nombre y el interruptor «Sincronizar mods al iniciar», editables. Quitar confirma en la misma fila y la fila sale deslizándose; los servidores que no responden quedan atenuados con «Probar de nuevo».
- Con amigos: a la izquierda, Compartir una instancia, Unirme con un código y las listas Compartes y Recibes; a la derecha, el detalle. Lo que compartes muestra el código con Copiar código, Copiar mensaje y Cambiar código, la caducidad, el límite de personas, el cupo, las personas con Quitar, Pausar o Reanudar y Dejar de compartir, todo con confirmación en el mismo lugar. Lo que recibes muestra el anfitrión, tu copia y Sincronizar y jugar o Jugar, y permite Salir de esta instancia (el núcleo quita su vínculo sin tocar mods ni mundos). Compartir va en tres pasos dentro del mismo panel, con barra que avanza: Instancia (las ya compartidas y las abiertas en Minecraft quedan deshabilitadas con su motivo), Qué compartes (Lo necesario para jugar, Todo o Elegir a mano, cuadros por tipo con «21 / 46», caducidad y límite de personas) y Código («Ya compartes…», copiar código y mensaje, Ver publicación). Unirme con un código usa tres campos de 4 que avanzan solos y reparten lo pegado; al completar los 12 caracteres valida solo, muestra la tarjeta de quien invita con su huella y «Unirme y descargar» sigue hasta la revisión de lo que se instala. Reemplazar los mods de una instancia existente aún no está disponible.
- Red privada: estado de ZeroTier con la red y tu IP para copiar, invitar a la red y desconectar con confirmación; desconectada, Conectar.
- Unirme con un código: al escribir el código a mano, un guion o un espacio tras un grupo completo pasa al siguiente y el cursor ya no queda fuera del texto, que se desplazaba a la izquierda.
- Instalar lo compartido ya no parpadea: el nombre del archivo y el contador se muestran como máximo 4 veces por segundo (siempre el final y cada cambio de etapa) y el nombre ocupa una sola línea de alto fijo.
- Con amigos: al abrir una publicación se consulta quién se unió cada 5 segundos durante 3 minutos (antes tardaba hasta un minuto), y después una vez por minuto para cuidar el presupuesto del servicio. Cada persona muestra el comienzo de su clave para distinguir equipos.
- Con amigos › Servidor: abre un servidor de Minecraft en tu equipo con los mods de la instancia que compartes, al estilo de Essential. Ícaro descarga el servidor oficial (Vanilla o Fabric), copia los mods sin los exclusivos de cliente, pide aceptar el EULA en el mismo lugar y muestra el estado, quién juega, la consola con comandos, «Jugar en mi servidor» y «Cerrar servidor», que guarda el mundo. Tus amigos lo ven abierto en lo que reciben y entran con «Entrar al servidor»: la conexión viaja cifrada por el mismo canal de Compartir, directa o por relé, sin abrir puertos ni estar en la misma red, y solo para las personas de esa instancia. «Ajustes del servidor» edita `server.properties` (mensaje, modo de juego, dificultad, jugadores, distancias, semilla, PvP, volar, extremo, bloques de comandos, lista blanca, protección del spawn y comprobar cuentas, que pide confirmar al desactivarlo para cuentas sin conexión) y la memoria, y abre la carpeta del servidor. Cerrar Ícaro con el servidor abierto lo detiene guardando el mundo antes de salir.

## [0.4.2] - 2026-11-10

- Al cambiar de sección, pantalla, pestaña, ajuste o página, todo vuelve arriba en vez de abrirse a la altura en que quedó lo anterior. Un panel desplazable dentro de una página se mueve primero y, al llegar a su borde, la rueda sigue con la página: ya no queda fija debajo de un panel.
- La pestaña Archivos ocupa todo el alto de la ventana: el árbol y la vista previa se desplazan por dentro.
- Registro legible `logs/latest.log`, como el de Minecraft: `[2026-11-10 02:34:50] [cuenta/INFO]: renovar_sesion: completado en 2713 ms`, con hora local, componente, nivel (INFO, WARN, ERROR), etapa, duración y causa de cada fallo. Los errores guardan su mensaje completo, con rutas, y el archivo y la línea del código; también quedan los avisos que ve quien usa Ícaro y los cierres inesperados. Es local: `diagnostico.jsonl` sigue sin rutas ni textos. Al abrir Ícaro, el anterior pasa a `AAAA-MM-DD-N.log` y se conservan los últimos 10. `launcher.log` también usa fecha legible.
- Consola › «Ícaro (registro del launcher)» muestra ese registro dentro del launcher, con filtros por nivel, búsqueda, exportar y carpeta; con la consola vacía, «Ver el registro de Ícaro» lo abre.

## [0.4.1] - 2026-11-10

- Compartir ya no abre diálogos: sus fases son páginas dentro de Servidores, con migas, contenido centrado y desplazamiento suave, y pasan de una a otra deslizándose 24 px y apareciendo (sin movimiento con «reducir movimiento»). Se puede navegar a otra sección y volver; la fase espera en Servidores.
- Servidores › Instancias compartidas reúne lo que compartes (estado, revisión, personas, código con copiar, caducidad, acción principal y menú con Pausar, Cambiar código y Dejar de compartir) y lo que recibes (de quién es, si el anfitrión está disponible, revisión y Jugar o Sincronizar), con sus estados vacíos.
- «¿Qué instancia compartes?» muestra versión, loader, mods, tamaño y cuándo se jugó por última vez; las ya compartidas abren su publicación y la que está en uso queda bloqueada. Incluye buscador y estado sin instancias.
- Tras publicar aparece «Ya compartes…»: código para copiar o enviar con instrucciones, personas con barra de cupo, caducidad con fecha exacta, límite de personas, cambiar código con confirmación y anular. Las personas se muestran como «Equipo 9e2a 7b14» hasta que el servicio comparta nombres.
- Instancias abre con un saludo bajo la banda (SaludoInicio): una frase según la hora del día, o de regreso tras una semana sin jugar, con el nombre de la cuenta, y debajo la última instancia jugada y cuándo. Las frases vienen en el launcher y cambian al abrirlo. El núcleo guarda cuándo terminó la última partida de cada instancia.
- Novedades de cada versión (NovedadesVersion): lista de versiones, filtros Todo, Nuevo, Mejoras y Correcciones con su número, funciones nuevas en tarjetas con acceso directo y aviso de versión de mantenimiento. Se abre sola la primera vez que se entra a una versión nueva y queda en Ajustes › Acerca de. Las notas salen de `docs/releases`; una nota nueva puede llevar `titulo` y `accion` (`servidores`, `cuenta` o `tienda`).
- Aviso de actualización (AvisoActualizacion): una tarjeta flotante abajo a la derecha reemplaza al banner. Muestra disponible, descargando (con velocidad y Cancelar), lista para reiniciar, espera a Minecraft, importante y error con «Descargar desde la web»; «Ver todo lo que cambia» abre Novedades en la versión nueva. Minimizada queda como pastilla en la barra lateral y no vuelve como tarjeta hasta la siguiente versión. El manifiesto admite un motivo `importante` opcional.
- Plata y Bronce tiñen toda la tinta de la interfaz (líneas, marcos, selección, iconos y textos), no solo el botón principal.
- Crear instancia permite usar una imagen propia como portada y marca con un marco grueso la portada elegida; las tarjetas de loader tienen el mismo alto.
- Jugar ya no lleva a Servidores mientras comprueba si la instancia viene de una compartida. «Publicación» de Compartir cabe en una pantalla, con la invitación junto a los accesos.
- Transición al pulsar Jugar (AjusteArranque, en Ajustes › Apariencia): Marea, Horizonte, Tinta, Bloques, Respiro o el Umbral de siempre, con tono, variación al azar, frase y registro mientras carga, y «Probar». Quien ya usaba Ícaro conserva el Umbral.
- Los menús de las instancias y de la cuenta salen unidos a su botón, hacia arriba si no caben, con cabecera, grupos y «Mover a grupo» como submenú. El menú de cuenta permite elegir con qué cuenta jugar, añadir cuentas, abrir Cuenta y skin y cerrar sesión.
- Canal de actualizaciones (CanalActualizaciones): antes de pasar a Beta se guarda una copia de ajustes, cuentas sin conexión y grupos. Volver a Estable explica qué se deja de tener y ofrece esperar a la próxima estable o volver ahora restaurando una copia compatible; sin copia, esperar o empezar de cero. «Copias antes de actualizar» guarda una copia antes de cada cambio de versión y conserva las 3 últimas, sin borrar nunca la última hecha en una versión estable. Los mundos, mods e instancias no se tocan.
- En Instancias compartidas se puede quitar una instancia que ya no compartes, con confirmación en la misma fila, y «Volver a compartir» muestra un esqueleto mientras carga. El panel de Publicación ya no va dentro de un recuadro.
- El menú de una instancia ya no tapa su botón al desplazar: si no cabe ni abajo ni arriba se abre al costado, y al cambiar de lado se desliza en 180 ms en vez de saltar.
- El bloque de cuenta ocupa la barra lateral hasta sus bordes y, al abrir su menú, la pestaña coincide con la línea de la barra. Los bordes de los menús ya no desaparecen al pasar el mouse y su sombra no deja ver lo que hay debajo.
- Al pulsar Jugar, la pantalla de carga ofrece «Cancelar» desde el primer momento: detiene la preparación o cierra el juego que aún no abrió su ventana. Los textos largos del registro y el nombre de la instancia se recortan con «…» en vez de salirse de la pantalla.
- Arrastrar la skin hacia la derecha la gira hacia la derecha. La tarjeta de actualización aparece con una transición suave al abrirla desde la barra lateral.
- El saludo de Instancias suma frases sobre el Nether, el End, granjas técnicas, redstone y las Ciudades Antiguas: 67 en total.

## [0.4.0] - 2026-11-09

- «Revisar publicación» sigue RevisionPublicacion versión 32 del System Design: lo que se publica se ordena por tipo (mods, configuraciones, resourcepacks, shaders y scripts) con atajos, casillas por tipo de tres estados, búsqueda y filtros, y una casilla por fila. El núcleo lee nombre, versión y entorno de `fabric.mod.json`, `quilt.mod.json` y `neoforge.mods.toml` y asocia cada configuración a su mod. Los tamaños se muestran legibles.
- Los diálogos de compartir usan el desplazamiento suave del launcher; tocar una tarjeta de tipo lleva a su sección. El hover ya no tapa los bordes y las casillas cambian de tono.

- Compartir instancia enmarca también el grabado y permite seleccionar de una vez los mods elegibles en la revisión. La autorización de archivos propios se conserva.
- La preparación automática de Minecraft tras crear una instancia usa un estado de instalación alojado en el heap. Evita el desbordamiento de pila observado en el launcher de desarrollo de Windows.

- Compartir experimental reutiliza contenido verificado entre instancias, deduplica por SHA-512 y descarga en paralelo con cupos por origen. Los archivos conservan su verificación y se aplican mediante la transacción recuperable.

- Compartir experimental: conflictos de selección/orden de packs revisables, biblioteca de publicaciones persistidas y reconexión sin revocar accesos; contadores cancelables, precomprobación de disco y limpieza conservadora del staging. Cierre con drenaje y bandeja Windows restaurable usando el icono propio; conserva la ventana minimizada si la bandeja falla. Servicio gratuito de invitaciones desplegado con prueba HTTP remota. Sigue bajo bandera; no acredita una partida ni sincronización entre dos equipos.

- Diagnóstico automático local con operaciones y etapas correlacionadas, causas tipadas, rotación, estado de salud y registro de emergencia ante panic. Se instrumentan Compartir, preparación/ejecución del juego, autenticación y actualizador sin copiar secretos ni mensajes dinámicos; ver [guía y límites](docs/diagnostico.md).

- Compartir experimental: Sincronizar publicación y comprobación remota previa a Jugar, sin consumir otra invitación; conflictos de archivo revisables y reserva de actividad por ID compartida con instalación/Java. Error de red o acceso revocado no inicia una partida automáticamente. Falta validarlo con Minecraft real y completar orden de packs y cierre/bandeja.

- Se añade el controlador experimental del publicador de instancias, deshabilitado por defecto: revisión y consentimiento, guardado de privados mediante tareas, publicación e invitaciones/accesos. Incluye cancelación y separación del panel publicado frente a otros borradores; ver el registro de progreso para las validaciones pendientes.

- En desarrollo en `v0.4.x`: Compartir instancias con publicador e invitado experimentales, revisiones seleccionadas, confianza persistente, invitaciones Workers/D1, transporte iroh y aplicación recuperable. El invitado muestra progreso por etapa y recupera su destino incompleto al reintroducir la invitación tras reiniciar, sin duplicarlo. El coordinador tiene pruebas con perfil inyectado; faltan validarlo con Minecraft real, orden de packs y cierre/bandeja. Sigue deshabilitado por defecto y sin despliegue. Ver [progreso](docs/superpowers/progreso/2026-11-07-compartir-instancias.md).
- Los botones comunes centran icono/texto con padding simétrico; los controles segmentados y resúmenes de compartir conservan alineación y medidas uniformes. Se corrigen el pie recortado y las dimensiones de las invitaciones dentro de paneles desplazables, con pruebas y capturas en ambos temas.

- Actualizar o instalar varios mods a la vez ya no descarta todos cuando uno falla: los que se pueden guardar se guardan, cada uno en su propia transacción, y el aviso nombra el mod que falló con su motivo.
- En la tienda de mods, al pasar de página la lista vuelve arriba con el desplazamiento suave y los botones de página quedan enteros. La rueda ya no se desfasa cuando la lista cambia de alto, lo que antes dejaba los botones fuera de vista al subir y volver a bajar.
- La tienda permite mostrar 20, 30, 50 o 100 resultados por página.
- La cuenta y el estudio de skin siguen el System Design actualizado, con un visor 3D compartido que gira de a 90° (Frente, Lado derecho, Espalda, Lado izquierdo). La cuenta muestra el tiempo jugado total y la instancia más jugada en lugar de UUID, correo o sesión; permite elegir la capa de la cuenta, previsualizar una skin del historial y descargar la skin. El estudio añade vista plana, encuadre de busto, caminata, la textura plana con la zona de cada capa, biblioteca y descarga. Se admiten skins antiguas de 64 × 32.
- Los iconos de los botones de solo icono quedan centrados en ambos ejes.

## [0.3.4] - 2026-10-27

- El editor de una instancia muestra el tiempo jugado y el espacio que ocupa, en lugar de «—». El tiempo se acumula al cerrar cada partida en `estadisticas.json` de la instancia y empieza en cero; las partidas anteriores a esta versión no se cuentan. Ambos datos se calculan en segundo plano al abrir la instancia.
- El perfil Microsoft recordado muestra su nombre mientras se renueva la sesión en segundo plano. Si falla, conserva el perfil como vencido sin habilitar acceso autenticado; no guarda tokens fuera del almacén protegido.
- El canal beta no ofrece una estable de la misma versión como actualización por su número de compilación. Tras aplicar una actualización, la pantalla ofrece «Continuar».
- El refresco de la galería utiliza índices para conservar imágenes y selección sin comparar cada captura con todas las anteriores. Las mediciones y límites figuran en el [registro de optimización](docs/superpowers/mediciones/2026-10-26-registro.md).
- Cambiar un ajuste global ya no reescribe la configuración de todas las instancias: con 50 instancias, la interfaz dejaba de responder unos 380 ms y ahora tarda unos 3 ms (medido en compilación de pruebas).
- Se reduce el tamaño de artefactos de desarrollo desactivando símbolos por defecto.
- El flujo Windows compila con 4 procesos de Cargo, conserva en caché las dependencias por perfil y los grabados preparados por hash, y compila launcher y servicio en una sola invocación. Con caché, el job bajó de 29 a unos 7 minutos. La compilación y la publicación van en jobs separados: `main` y `develop` ya no esperan el build del otro y la versión se vuelve a comprobar antes de publicar. El release usa LTO thin, con un ejecutable un 1,3 % mayor.
- Se separan las pruebas de sincronización e instalación de Minecraft y se comprueba la colisión de nombres antes de recuperar una instancia de la papelera.

## [0.3.3] - 2026-10-24

- Instalar mods desde la tienda e importar JAR ya no fallan con «Los archivos o metadatos de mods no son válidos» en instancias con dos JAR del mismo proyecto, como dos versiones de Sodium. La sincronización ya no registra ambos con la misma identidad, y un inventario guardado así conserva el primero; el otro sigue en `mods` como archivo local, sin borrar nada.
- «Ver detalle» en un aviso de error muestra ese error en lugar de la pantalla de falla de la tienda con datos fijos.

## [0.3.2] - 2026-10-24

- Reduce las imágenes empaquetadas de unos 192 MB a 104 MB. Los grabados que no se recortan, invierten ni escalan conservan su WebP original en lugar de recodificarse sin pérdida (mismos píxeles), y la variante Tinta que solo invierte o repite Piedra se obtiene al decodificar; si es igual, comparte la textura.
- Las descargas reales ya no reconstruyen la interfaz en cada cuadro: las barras de progreso se redibujan solas. Los avisos piden cuadros solo durante su entrada y su fundido.
- La revisión de resourcepacks y shaders consulta las fichas por lotes y revisa hasta cuatro paquetes a la vez, en lugar de tres solicitudes seguidas por paquete.
- Las consultas a cuentas, Java, catálogos y loaders comparten un cliente HTTP y reutilizan conexiones. Las vistas leen la lista de instancias sin copiarla entera.
- La sincronización ya no se bloquea con «Falta una dependencia compatible del anfitrión» cuando un JAR propio exige una dependencia que Modrinth declara exclusiva de servidor, como WorldgenFeatureFix para Epic Terrain: el cliente la omite con un aviso, igual que el anfitrión. Compatible con servicios de sincronización anteriores.
- `icaro-sync-host` añade `--incluir-servidor` (todos) y `--incluir-archivo <jar>` (repetible) para publicar también los mods exclusivos de servidor y reproducir la carpeta `mods` completa. Sin estas opciones se siguen excluyendo.

## [0.3.1] - 2026-10-23

- Reduce el tiempo de aplicación de parches al instalar Forge, conservando las verificaciones y los archivos del cliente.
- Muestra el avance y el tiempo transcurrido durante la instalación de Forge y NeoForge.
- Corrige la detección de Minecraft en Discord al jugar desde el launcher en Windows.

## [0.3.0] - 2026-10-22

- Quilt se instala y se ejecuta con su perfil oficial, igual que Fabric: crear instancia, Jugar, Cambiar versión y Cambiar versión del loader. El perfil y las bibliotecas se descargan solo de `meta.quiltmc.org`, `maven.quiltmc.org` y `maven.fabricmc.net`, con SHA-1 y tamaño comprobados, y se guardan en `meta/quilt/<minecraft>/<loader>`. Admite las versiones ofuscadas (con los mapeos `hashed` e `intermediary`) y las que Quilt publica sin ofuscar, como 26.3. El selector de versiones del loader ordena las estables antes que las betas, así que lo primero que se ofrece es una estable.
- Publicación por canales. `develop` publica cada cambio como beta (`v<versión>-beta.<compilación>`, prerelease) y `main` publica la estable (`v<versión>-build.<compilación>`) y crea su tag fuente `v<versión>`. Las ramas `vX.Y.x` y los tags ya no publican. `main` exige una versión de Cargo mayor que la última estable, y `develop` una versión mayor que esa estable para sus betas. Los cambios que solo tocan documentación no compilan.
- Ajustes → Acerca de añade el canal de actualizaciones Estable/Beta, guardado en la configuración. El canal Estable nunca ofrece una beta; el Beta ofrece la release más nueva entre betas y estables. Una compilación beta se identifica en Acerca de y en la pantalla de actualización. Los launchers anteriores ignoran las prereleases.
- Capturas ya no congela la interfaz al abrirse. La cuadrícula usa miniaturas de 480 × 270 generadas en segundo plano (hasta cuatro a la vez, de la más reciente a la más antigua), que aparecen a medida que están listas; antes cada PNG se decodificaba y se subía a la GPU a resolución completa en el primer cuadro. El visor sigue abriendo el original sin recodificar, y un PNG dañado deja su hueco vacío sin afectar al resto.

- Añade instalación y preparación de Forge y NeoForge con sus instaladores oficiales, respetando la versión elegida y la configuración de Java.
- Los MRPACK y ZIP reconocen Forge y NeoForge y conservan la versión del loader indicada por el paquete.
- Instala Fabric API compatible por defecto al preparar una nueva instancia Fabric. Conserva copias locales o desactivadas y el contenido de modpacks; no vuelve a añadirlo si lo retiras después.
- Corrige la apertura del inicio de sesión en Windows para utilizar cualquier navegador configurado como predeterminado del sistema.
- Prepara Forge y NeoForge en una carpeta aparte y reutiliza sus archivos verificados, conservando la instalación anterior si el nuevo intento falla.

Consulta las [notas de 0.3.0](docs/releases/0.3.0.md) y el [alcance y validación](docs/loaders-forge-neoforge.md). Quedan pendientes la validación manual del inicio de sesión con distintos navegadores, las partidas reales con mods, las migraciones y el recorrido nativo en Linux. No se admiten todos los formatos históricos de Forge.

## [0.2.5] - 2026-10-16

- Sustituye Invitado por cuentas sin conexión con nombre propio, UUID estable, guardado, selección y eliminación desde el componente CuentaSinConexion entregado. Conserva sus iconos, cabeza rayada, temas y confirmación en fila; eliminar un perfil no toca mundos ni instancias.
- Restaura la cuenta local al abrir y mantiene sus datos separados de las credenciales Microsoft. Permite lanzar Vanilla/Fabric sin una sesión Microsoft; los servicios oficiales y los servidores con autenticación siguen requiriendo una cuenta Microsoft.
- Añade el componente SkinSinConexion: PNG local, frente/espalda, modelo clásico/delgado, capa exterior y cabezas personalizadas. Guarda la skin por cuenta y prepara texturas firmadas con un agente verificado para el siguiente inicio del juego. Conserva la vista anterior ante errores y permite quitar la skin.
- Quita del formulario la explicación técnica sobre el cálculo del UUID.

Consulta [cuentas sin conexión](docs/cuentas-sin-conexion.md) para el alcance y la validación.

## [0.2.4] - 2026-10-16

- La revisión de sincronización guarda un inventario de `mods` (`.icaro-mods-inventario.json`, junto al bloqueo de la carpeta de juego) y solo relee los archivos nuevos, modificados o con más de 7 días desde su última lectura completa. También reutiliza la identidad Fabric de cada JAR mientras su hash no cambie. En un banco de 500 MiB, Jugar sin cambios pasa de 2,4 s a unos 45 ms; la primera revisión sigue leyendo todo.
- La sincronización y el instalador de mods piden los nombres de proyecto y los mods exclusivos de cliente por lotes de 50, en lugar de una ficha por mod. La caché de Modrinth ya no queda bloqueada mientras espera la red: una consulta lenta no retiene los aciertos de caché ni otros recursos, y dos peticiones simultáneas del mismo recurso siguen produciendo una sola.
- Las peticiones a la API de Modrinth siguen un presupuesto preventivo (4 por segundo, ráfagas de 8) y esperan al reinicio de la ventana cuando las cabeceras indican que quedan menos de 10, en lugar de frenar solo tras recibir un 429. La espera creciente ante un 429 añade hasta un 10 % aleatorio.
- Si el anfitrión y la carpeta `mods` no cambiaron desde la última sincronización correcta, Jugar confirma el plan sin consultar Modrinth. El registro `minecraft/.icaro-sync-verificado.json` caduca a las 24 horas.
- Las descargas de la sincronización van directamente a disco con el SHA-512 calculado por bloques, sin cargar cada JAR en memoria, y se hacen en paralelo: hasta cuatro desde Modrinth y dos desde el anfitrión.
- `icaro-sync-host` solo vuelve a calcular el hash de los JAR que cambiaron en lugar de toda la carpeta cada 10 segundos, envía cada JAR por bloques sin cargarlo en memoria, atiende las descargas en cola en vez de cortar la novena conexión y mantiene abiertas las conexiones correctas. El índice usa `ETag`/`304` y las descargas cortadas del anfitrión se reanudan con `Range`/`If-Range` validados. Es compatible con launchers y servicios anteriores.

Validación: pruebas del núcleo, de la interfaz y del servicio con servidores locales y fuentes simuladas. La descarga real desde el CDN de Modrinth y el servicio actualizado en el Arch del anfitrión siguen pendientes. Consulta las [notas de 0.2.4](docs/releases/0.2.4.md).

## [0.2.3] - 2026-10-16

- Acerca de muestra la versión compilada del workspace en lugar del texto fijo `0.1.0`.
- Acerca de añade **Código fuente en GitHub**, con icono de enlace externo, que abre el repositorio declarado en `Cargo.toml`.
- Amplía la guía de colaboradores con el estado del hotfix, los pendientes y las prioridades de la investigación de sincronización.
- La sincronización calcula los hashes de `mods` una vez por revisión en lugar de dos, y al aplicar solo vuelve a leerlos si cambiaron los metadatos de algún archivo. Pulsar Jugar con el conjunto al día pasa de tres lecturas completas a una.
- La lectura local de mods se ejecuta fuera del executor asíncrono y se detiene al cancelar. El límite de 90 segundos de la revisión ya no incluye el disco, por lo que una carpeta grande no se presenta como un fallo de Modrinth.
- Las compilaciones de desarrollo optimizan `sha2` y `sha1`; las mediciones de sincronización en debug dejan de estar dominadas por el hashing sin optimizar.
- Aplicar una sincronización ya no copia toda la carpeta `mods`: descarga y verifica solo lo nuevo, y mueve únicamente los archivos afectados mediante un diario que se deshace si falla o si el launcher se cierra a mitad. Los mods personales no se copian ni se recrean.
- La barra de progreso de la sincronización cuenta los cambios del plan en lugar de todos los mods del índice.
- Añade `logs/sincronizacion.log`, con rotación, para diagnosticar sincronizaciones: índice del anfitrión, revisión, plan, descargas, commit, rollback, recuperación y errores con su fase y duración. No incluye rutas absolutas ni datos de la cuenta.

Validación: pruebas del núcleo y de la interfaz y un banco sintético de 500 MiB; el usuario confirmó la mejora en Windows. La validación en HDD y Linux sigue pendiente. Consulta las [notas de 0.2.3](docs/releases/0.2.3.md).

## [0.2.2] - 2026-10-15

- Identifica versiones anteriores y copias duplicadas de un mod por su ID interno y sus hashes. La sincronización respalda y retira las copias activas del mismo mod del servidor, conserva otros mods personales y verifica que no queden dos versiones activas de un mod requerido.

- Reutiliza el paquete de actualización ya descargado tras reabrir el launcher o reintentar una instalación fallida; vuelve a verificar tamaño y SHA-256 y descarta archivos parciales o alterados.

- Mantiene bloqueada la actualización hasta cerrar el launcher, evita ayudantes simultáneos y normaliza las rutas para PowerShell/Inno Setup. Comprueba la versión instalada y muestra el motivo de un fallo.
- Distribuye el espacio del resumen de importación para que una lista larga de carpetas no aplaste las etiquetas.
- Muestra el plan de sincronización aunque un JAR propio choque con el del anfitrión; exige autorizar su reemplazo y conserva una copia verificable fuera de `mods`.
- Añade **Importar .jar** en la lista de mods: selección múltiple, publicación atómica y conservación de nombres ya ocupados.

Incluye una [investigación de rendimiento](docs/investigaciones/rendimiento-sincronizacion-2026-10-15.md) con mediciones y un banco de pruebas reproducible. Las optimizaciones propuestas siguen pendientes. Consulta las [notas de 0.2.2](docs/releases/0.2.2.md) para la validación y sus límites. Esta preparación local todavía requiere publicar los artefactos para distribuir la actualización.

## [0.2.1] - 2026-10-15

- Añade selección de MRPACK/ZIP por ruta y con Examinar, y adapta ImportarModpack al diseño entregado: grabado, iconos, fases de lectura, resumen, nombre/grupo y recuperación de errores.
- Conecta el componente ZonaSoltar del diseño a la importación local: clic, arrastre, revisión y protección frente a archivos adicionales durante una operación.

- Corrige la rueda del selector de versiones dentro del asistente de instancias.
- El ejecutable del launcher para Windows abre la interfaz sin una consola de fondo.
- Añade importación local de MRPACK y ZIP completos Vanilla/Fabric en instancias nuevas, con revisión, extracción por bloques y recuperación desde Descargas.
- Comparte caché y pausa de Modrinth entre tienda, cesta e instalación; agrupa comprobaciones de actualización por hash y fichas por proyecto para evitar repetir peticiones al añadir otro mod.
- Adapta la publicación a la rama de mantenimiento `v0.2.x`, conserva el tag `v0.2.1` para el código y evita volver a compilar/publicar el mismo contenido al integrarlo en otras ramas.

Validación: pruebas del núcleo y de la interfaz, importación ZIP64 sintética de más de 2 GiB, conservación de archivos y capturas en Tinta/Piedra. El usuario confirmó la importación de su instancia real. Los ZIP que solo contienen referencias de CurseForge y los instaladores de Quilt/Forge/NeoForge siguen pendientes; consulta las [notas completas](docs/releases/0.2.1.md).

## [0.2.0] - 2026-10-14

Publicación consolidada de las mejoras de servidores y las correcciones probadas durante su preparación. Su tag es `v0.2.0`; la rama de mantenimiento es `v0.2.x`.

### Añadido

- Servicio independiente para sincronizar los mods de un servidor dedicado, con compilación para Linux y uso sin instalar el launcher en el anfitrión.
- Distribución de JAR privados o ausentes en Modrinth desde el anfitrión, verificando tamaño y SHA-512.
- Preferencias de Minecraft compartidas entre instancias, con filtrado de teclas de mods ausentes.
- Caché persistente de metadatos de Modrinth y consultas de versiones en lotes de hasta 50 IDs.

### Corregido

- Rutas extendidas de Windows en los argumentos de Java y el classpath, que impedían encontrar `KnotClient` de Fabric.
- Recuperación de dependencias obligatorias omitidas por índices antiguos, incluidas bibliotecas como CodxLib y Lithostitched cuando sus JAR admiten cliente.
- Dependencias de cliente no declaradas de Litemoretica 1.2.5 para Minecraft 1.21.1: la sincronización añade Litematica y resuelve MaLiLib automáticamente.
- Conservación de mods personales del cliente, tanto del catálogo como de JAR manuales y desactivados, durante la sincronización.
- Desactivación persistente de la sincronización al iniciar después de completarla correctamente; los fallos conservan la preferencia para reintentar.
- Selección manual de Java más reciente que el mínimo del perfil, como Java 25 para Minecraft que recomienda Java 21.
- Navegación bloqueada durante la consulta de mods y lentitud al volver a las instancias por la revisión repetida de miles de archivos.
- Lectura de descriptores Fabric incompletos y publicación de mods universales con metadatos incompletos, conservando los JAR originales con avisos.
- Resolución de dependencias y pins del catálogo sin sustituir silenciosamente el conjunto instalado del servidor.
- Detección de redes conectadas desde ZeroTier oficial y actualización de instancias vinculadas a servidores.

### Rendimiento y recuperación

- Descarga solo los archivos ausentes o cambiados; repetir una sincronización al día evita volver a copiar el conjunto completo.
- Reutiliza las consultas vigentes entre aperturas y conserva respuestas parciales correctas ante fallos posteriores.
- Respeta los límites de Modrinth y sus tiempos de espera ante HTTP 429.
- Permite salir de la consulta o navegar mientras termina la aplicación del plan, sin abrir Minecraft desde otra pantalla.

### Validación

- Pruebas de sincronización, dependencias, conservación de archivos, caché, Java y navegación.
- Integración con Litemoretica, Litematica y MaLiLib reales en una instancia temporal vacía; una segunda sincronización no propone cambios.
- El usuario confirmó el funcionamiento del launcher y el inicio del juego con la corrección de dependencias.
- La publicación de instaladores y los artefactos Linux se comprueban por separado en GitHub Actions. Los loaders adicionales y el resto del roadmap siguen pendientes.

## [0.1.0]

Primera entrega pública para Windows: instancias Vanilla y Fabric, catálogo de contenido, cuentas, consola, capturas y actualización del launcher. Consulta las [notas completas](docs/releases/0.1.0.md).

[0.2.2]: docs/releases/0.2.2.md
[0.2.1]: https://github.com/00faithlss/Icaro-launcher/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/00faithlss/Icaro-launcher/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/00faithlss/Icaro-launcher/tree/v0.1.0
