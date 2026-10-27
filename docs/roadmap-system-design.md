# Roadmap del System Design

Actualizado el **12-10-2026**. Referencias: `icaro-design-system.rar` y `icaro-componentes-nuevos.zip` (`CambioVersionInstancia`, `InfoProyecto` y `PaginaMod`), `icaro-exportar-instancia.zip`, `icaro-icono-instancia.zip` e `icaro-exportar-lista-mods.zip`, sus componentes, definiciones y previews originales. Este archivo sigue la implementación del diseño; el [roadmap general](../README.md#roadmap) sigue las funciones del launcher.

Las siguientes entregas deben conectar y completar esos diseños, conservando sus composiciones, iconos, tipografías, colores y controles. Las diferencias conocidas se registran para evaluación antes de sustituir una vista.

## Cómo leer el estado

| Estado | Significado |
| --- | --- |
| Implementado | Está disponible en el alcance descrito. No significa que todos los usos futuros del componente estén conectados. |
| Parcial | Ya hay implementación, pero faltan partes del diseño o acciones reales. |
| Maqueta | La composición existe, pero usa ejemplos o acciones simuladas. Falta conectarla al núcleo. |
| Pendiente | Falta implementar o integrar el componente indicado. |
| A evaluar | Hay una diferencia con la referencia o una decisión de alcance que debe revisarse con el usuario. |

Una vista puede estar implementada visualmente y tener su función pendiente. Por ejemplo, el asistente instala Vanilla y Fabric; Quilt, Forge y NeoForge siguen pendientes; el diálogo de exportación genera ZIP, pero no MRPACK.

## Avances recientes

- [x] Esquinas exteriores de la ventana con radio pequeño mediante DWM en Windows 11; al maximizar o acoplar, Windows aplica su geometría habitual. Botones, tarjetas y paneles conservan esquinas rectas. Windows anteriores y Linux mantienen la geometría disponible del sistema; redondeado explícito en Linux pendiente de evaluación.

- [x] Botón de preparación con trama estática y estado estable durante eventos intercalados de descarga/verificación, evitando destellos y cambios rápidos de composición.
- [x] Confirmación de eliminación con nombre y consecuencias, diez bloques cuadrados y grietas en toda la tarjeta. Iconos con vista previa inmediata y guardado final de calidad; botones conservan contorno e invierten tinta/papel. Capturas vacías dentro de una instancia ofrecen Jugar.
- [x] Menú de Mods anclado al botón y reposicionado al maximizar; Agregar acepta ZIP de recursos/shaders beta o de otra versión; ArchivosNoVerificados limita la lámina y usa interlineado absoluto.
- [x] Tiendas de mods, resourcepacks y shaders con composición y cesta compartidas; filtros de entorno distinguen cliente, servidor y ambos.
- [x] Apariencia, insignias, progreso y toasts recuperan los componentes originales y sus estados reales.
- [x] Servidores guarda y consulta la lista real y conecta la instancia vinculada; la maqueta permanece separada.
- [x] Rutas legibles e `instance.json` de esquema 2, migración conservadora de CFG e identificación de archivos no verificados.
- [x] ExportarInstancia nuevo con árbol, selección por grupos, presets y ZIP Ícaro/Prism.
- [x] IconoInstancia: editor y menú, imagen/capturas/generado, recorte, estilos y guardado reversible.
- [x] ExportarListaMods: seis formatos, selección, campos, plantilla y metadatos consultados sin instalar.
- [x] Corregido el cierre al cambiar a Versiones: la página reconcilia correctamente el tipo del panel desplazable al cambiar de pestaña o tamaño de ventana.


- [x] Paso 1 con `SelectorVersion` y paso 2 con `SelectorLoader`, separados; versiones oficiales de Fabric, Quilt, NeoForge y Forge, sin valores de muestra en modo real.
- [x] `CambioVersionInstancia` del ZIP nuevo: lista virtualizada, compatibilidad de todos los mods por lotes en segundo plano, caché, recomendación desde 95 %, loader seleccionable y actualizaciones opcionales. Respaldo obligatorio al bajar, copia y recuperación si falla la publicación de los cambios.
- [x] `PaginaMod` e `InfoProyecto` nuevos: galería, tabla de versiones, columna de 320 px, enlaces/iconos originales y avatares reales. Perfil interno del creador pendiente de componente; los datos ausentes de la API se omiten.
- [x] Recursos y nueva pestaña Shaders reutilizan `FilaMod`/`ModsInstancia` por petición del usuario, con inventarios reales y acciones individuales y múltiples.
- [x] Clic fuera cierra la ficha rápida; volver desde la página del proyecto restaura el contexto de instancia o catálogo.
- [x] Resourcepacks ZIP, inventario y activación en `options.txt`; compatibilidad independiente del loader e instalación fuera de versión con confirmación.
- [x] Barra de ventana conservada en los visores, panel de mod entrando desde la derecha, selección de todos los mods y menú del editor conectado.
- [x] `ActualizacionLauncher` conectado a releases de GitHub y descarga verificada; falta publicar los artefactos para probar una actualización completa.
- [x] Vanilla y Java oficiales conectados al asistente y a Jugar, con archivos guardados y verificados.
- [x] Estados de BotonJugar, cola de preparación, registros reales en Umbral/Consola e informe de fallo sin causas ficticias.
- [x] Asistente de cuatro pasos con selector de versiones, tarjetas de loader, radios de rombo y botones con sus flechas.
- [x] Filas de mods con casilla, interruptor, icono, creador, descripción, estado, versión y acciones.
- [x] Barra invertida de selección múltiple y papelera de mods con Deshacer durante diez segundos.
- [x] Tienda con filtros laterales, contexto de instancia y lista inferior de instalación con dependencias y tamaño.
- [x] Ficha lateral con miniaturas, versiones compatibles y dependencias obligatorias y opcionales.
- [x] Página completa con navegación fija, tabla de versiones y columna lateral de información.
- [x] Diálogo de exportación con casillas, tamaños, destino y acciones del pie; ZIP conectado a los archivos reales.
- [x] Archivos con barra de acciones, árbol y tamaños, vista previa de solo lectura y renombrado dentro de la fila.
- [x] Galería del catálogo conectada al visor existente, usando originales de mayor resolución.
- [x] Capturas aisladas de las vistas principales en Tinta y Piedra, a 1280×800.

## Base visual y componentes compartidos

| Referencia original | Estado | Lo que tenemos y lo que falta |
| --- | --- | --- |
| Tokens de color, tipografía, bordes y espaciado | Parcial | Tinta/Piedra, Archivo, Martian Mono y medidas compartidas. Falta completar metales de acento, escala y revisar todos los estados de foco. |
| Iconografía Pixelarticons | Implementado | Iconos originales de secciones y acciones; los botones restaurados usan el mismo sistema. |
| `Boton`, `Campo`, `Selector`, `Interruptores` | Implementado | Controles compartidos, variantes, casillas cuadradas, interruptores y rayado de deshabilitados. |
| `Chips`, `Insignia`, `Pestanas`, `Contador`, `Senal` | Implementado | Filtros, estados acompañados de texto, pestañas y contadores usados en las vistas. |
| `Progreso`, `BloqueCodigo` | Parcial | Bloques por conteos, continuo por bytes, indeterminado, spinner y esqueleto con animación y movimiento reducido. Operaciones reales de mods, Vanilla y Java conectadas; integridad completa pendiente. |
| `Tabla` | Parcial | Tabla de versiones en la página del proyecto. Falta completar su uso y navegación en las demás tablas del diseño. |
| `Tarjeta`, `Avatar` | Parcial | Tarjetas y representaciones de respaldo disponibles; iconos de proyectos y creadores desde Modrinth. La cuenta ya muestra su textura real; falta completar los demás avatares y datos opcionales. |
| `DeslizadorMemoria` | Parcial | Configuración por instancia guardada; memoria mínima/máxima y Metaspace aplicados al proceso; límites según RAM real en Windows/Linux. Java compatible detectado o descargado. Falta completar recomendaciones y advertencias de reserva para sistema/otras instancias. |
| `Dialogo`, `MenuContextual`, `Toast` | Implementado | Diálogos con velo y menús. Cola de hasta tres toasts con entrada, fundido, pausa al pasar el cursor, Escape y acciones. Errores con detalle; Deshacer de mods de diez segundos. |
| `Tooltip` | Parcial | Ayudas disponibles en algunos controles; falta cubrir todos los botones de solo icono y el modo angosto. |
| `Banda`, `Detalle`, `Lamina`, `Cover` | Implementado | Grabados, recortes, portadas de respaldo y composición de títulos sobre bloques propios. Las capturas reales como portada siguen pendientes. |
| `Lupa` | Parcial | Utilidades existentes y zoom del visor. La lupa sobre las láminas del launcher todavía no está aplicada. |
| `ListaVirtual` | Parcial | Selectores de versiones y consola dibujan solo filas visibles; compatibilidad en segundo plano con caché y cuatro destinos simultáneos. Catálogos con paginación; falta extender la virtualización al explorador de archivos. |
| `ListaReordenable` | Parcial | El arrastre de servidores reales guarda el orden al soltar y Escape lo cancela. Falta aplicarla a recursos reales y persistir sus prioridades en el juego. |
| `SelectorColor` | Pendiente | Falta conectar el selector definido a iconos/colores de instancia y revisar su composición original. |
| `ZonaSoltar` | Parcial | La importación local de modpacks conecta clic y arrastre de MRPACK/ZIP a la revisión real, con los estados de reposo y arrastre del diseño. Skins también valida PNG reales por arrastre. Falta extenderla a la importación de recursos independientes. |

## Ventana, bienvenida y sesión

| Referencia original | Estado | Lo que tenemos y lo que falta |
| --- | --- | --- |
| `AppShell` | Parcial | Barra de título, siete secciones, cuenta y controles de ventana; Ctrl+1 a Ctrl+7 y Ctrl+K. Jugar se oculta sin sesión/selección; muestra proceso y preparación reales de Vanilla. Falta completar teclado, bandeja y estados de los demás instaladores. |
| `DisenoAngosto` | Pendiente | El cambio de tamaño de ventana está implementado. Falta el conjunto original de quiebres: barra colapsada, filtros en popover e índice de Ajustes como selector. |
| `PrimeraEjecucion` | Parcial | Bienvenida sin cuenta activa, acceso a Microsoft y entrada con cuentas sin conexión guardadas. Java y preparación inicial aún no ejecutan tareas reales. |
| `SelectorCuenta`, `EstadosSesion` | Parcial | Menú con cuenta Microsoft, cuentas locales persistentes, iniciar/cerrar sesión, recuperación y errores. Falta conectar todos los estados del diseño y varias cuentas Microsoft persistentes. |
| `CuentaSinConexion` | Conectado | Perfiles locales persistentes, validación, selección y eliminación con confirmación; conserva mundos e instancias. |
| `SkinSinConexion` | Parcial | PNG local, modelo, frente/espalda, avatares y preparación del juego conectados. Pruebas de firma, servicio y agente con Java; pendiente validación en partida real. |
| `CodigoDispositivo` | A evaluar | El modo real conserva Hermes y usa autorización por navegador con retorno a Ícaro y progreso. Código/QR quedan solo en la maqueta; se retiraron del flujo real por el problema observado y el recorrido por navegador fue aprobado y probado por el usuario. |
| `EstadosPantalla`, `EstadoVacio`, `EnConstruccion` | Parcial | Componentes y estados vacíos disponibles; catálogos con carga, error y cancelación, e instancias locales recuperadas del disco. Falta aplicar el conjunto completo de carga, vacío, error y sin conexión a cada servicio. |
| `MenuBandeja` | Pendiente | Falta integración nativa, estado del juego, instancias recientes y comportamiento al cerrar la ventana. |

## Instancias

| Referencia original | Estado | Lo que tenemos y lo que falta |
| --- | --- | --- |
| `TarjetaInstancia`, `PanelLateral` | Parcial | Lista, selección, grupos y metadatos guardados. Vanilla pasa de instalación a lista y ejecución con datos reales. Fabric usa el perfil oficial y el usuario confirmó su arranque con un mod real en Windows. Quilt, Forge y NeoForge quedan pendientes. El tiempo jugado se acumula por instancia y el editor lo muestra con su tamaño; falta mostrarlos en la lista. |
| `Asistente`, `SelectorVersion`, `SelectorLoader` | Parcial | Selectores separados por paso, fechas, búsqueda y versiones antiguas; catálogo oficial de versiones de loaders y elección persistida. Crear Vanilla o Fabric prepara Minecraft, Java y el perfil oficial. Faltan Quilt, Forge y NeoForge y la validación de Fabric con un mod real. |
| `EditorInstancia` | Parcial | Encabezado y pestañas existentes; nombre, grupo y Java guardados; Vanilla omite Mods. Java, memoria y argumentos aplicados al juego. General requiere icono/color, notas y guardado completo de resolución/servidor. |
| `AccionesInstancia` | Parcial | Abrir carpeta, eliminar a papelera y Deshacer conectados. Exportación ZIP real con diálogo original. Duplicar, importar y otras acciones aún necesitan núcleo. |
| `CambioVersionInstancia`, `CambiarVersion` | Parcial | Selector nuevo con compatibilidad de todos los mods instalados por SHA512, recomendación desde 95 %, familia de loader conservada, cambio separado de su versión, actualizaciones opcionales y desactivación desplegable. Respaldo obligatorio al bajar, copia independiente, preparación aislada y recuperación si falla; conserva mods inactivos. Minecraft y Fabric se preparan después mediante Instalar; Quilt, Forge y NeoForge siguen pendientes. |
| `ImportarClonar` | Maqueta | Selector de orígenes y composición disponibles. Falta leer packs, detectar otros launchers, mostrar datos reales e importar/clonar archivos. |
| `ExportarModpack` | Pendiente | Falta el flujo completo MRPACK: nombre, versión, autor, referencias de mods y archivos incluidos. El selector MRPACK del diálogo de instancia permanece deshabilitado. |
| `ArchivosInstancia`, `Explorador` | Parcial | Barra con iconos, árbol y tamaños, abrir/copiar ruta/renombrar, vista previa de texto y papelera. Eliminación mediante papelera del sistema; falta recuperación desde esta pestaña y navegación completa por flechas y previews de imágenes, JAR y mundos. |
| `RecursosInstancia` | Parcial | Por petición del usuario, resourcepacks y nueva pestaña Shaders reutilizan filas y controles de `ModsInstancia`: búsqueda, filtros, selección, activación, actualizaciones y retirada. ZIP e inventarios reales; recursos activos en `options.txt`, shaders en configuración existente de Iris. Versiones no declaradas requieren confirmación. Falta prioridad de resourcepacks por arrastre y soporte de otros motores de shaders. |
| `MundosInstancia` | Maqueta | Lista y panel de copias disponibles. Falta leer mundos, gestionar copias/restauración y conectar sus acciones al juego y disco. |
| `Capturas` | Parcial | Galería original conectada a las PNG que Minecraft guarda por instancia, filtros por identidad, refresco en segundo plano y visor con el original. Copiar imagen/archivos y abrir carpeta funcionan en Windows. Falta eliminación reversible y copia de imágenes en Linux. |
| `BotonJugar`, `Umbral` | Parcial | Estados originales conectados a instalación, preparación y ejecución Vanilla; Jugar sin MB, Preparando al verificar archivos guardados e Instalando solo al descargar archivos. En Windows el Umbral espera la ventana del proceso Minecraft y retira la capa completa con fundido, sin cortina ni remontar la vista, según la corrección solicitada por el usuario. Falta detección equivalente en Linux y cerrar launcher conservando juego mediante bandeja. |
| `Picar` | Parcial | Diez etapas y eliminación de instancia/modpack instalado manteniendo el botón; soltar cancela. Revisa respaldos de mundos y exige confirmación si no hay copia vigente. Papelera y Deshacer. Faltan los recorridos de mundos y archivos individuales. |

## Mods y modpacks

| Referencia original | Estado | Lo que tenemos y lo que falta |
| --- | --- | --- |
| `FilaMod` | Implementado | Casilla, interruptor, icono, nombre/creador, descripción, estados, versión y acciones; desactivados atenuados y conflictos identificados. |
| `ModsInstancia` | Implementado | Inventario real, filtros, selección múltiple, activar/desactivar, actualizaciones con confirmación, quitar a papelera y Deshacer. |
| `TiendaMods` | Parcial | Composición original, filtros laterales, contexto de Minecraft/loader, búsqueda, filas y lista inferior de instalación. Modrinth conectado; CurseForge visible pero deshabilitado. |
| `FichaMod` | Implementado | Panel rápido con acciones, miniaturas, versiones compatibles, nombres/estado de dependencias y selección de opcionales. Ver página y Volver conservan la navegación anterior. |
| `PaginaMod` (ZIP nuevo, 12-10) | Parcial | Cabecera de 64 px, autor/categorías/lado y resumen, columna 320 px con InfoProyecto y tres mods reales del dueño; galería 3 columnas 16:9, filtros, canales, compatibilidad y grupos de dependencias. Perfil interno del creador requiere diseño; motivos de incompatibilidad y datos no publicados se omiten. MRPACK de Modrinth instala en nuevas instancias Vanilla/Fabric con revisión y cola; los otros loaders e importación local siguen pendientes. |
| `InfoProyecto` (ZIP nuevo, 12-10) | Parcial | Versiones mono, tres filas, snapshots opcionales, `.x` solo con manifiesto que prueba toda la familia; plataformas, entornos, enlaces, etiquetas, creadores y detalles reales, compartidos con el panel rápido. Faltan conteos de contenido del MRPACK y datos fiables de rendimiento/mínimos de shaders; no se simulan. |
| `VisorCaptura` | Parcial | Compartido entre capturas locales y galería de proyectos; originales, zoom, arrastre, flechas y Escape. Conserva barra de búsqueda, arrastre y controles de ventana. Copiar y abrir carpeta conectados en Windows; eliminación reversible pendiente. |
| `BibliotecaMods` | A evaluar | El original reúne un mod y todas las instancias que lo usan. La sección actual es **Modpacks**, según el alcance trabajado, y los mods se gestionan por instancia. Esa biblioteca global no está implementada; evaluar su ubicación antes de añadir una sección. |

## Servicios, ajustes y pantallas auxiliares

| Referencia original | Estado | Lo que tenemos y lo que falta |
| --- | --- | --- |
| `PantallaDescargas`, `ColaDescargas` | Parcial | Cola real de MRPACK/Vanilla/Fabric/Java con pausa, reanudación, cancelación y reintento en las filas originales. Quitar un registro terminado o fallido oculta su progreso sin borrar la instancia ni sus archivos. Falta historial persistente y unificar mods y demás instaladores en la cola global. |
| `PantallaConsola`, `Consola` | Implementado | Barra interior del diseño con filtros independientes y conteos, búsqueda con resaltado, autoscroll con pausa al subir e Ir al final, copiar/limpiar e iconos. Render de filas visibles, selector por instancia, último registro desde latest.log, exportación TXT y vacío con San Jerónimo. Registros en vivo acotados y credenciales ocultas. |
| `PantallaServidores`, `TarjetaServidor` | Parcial | Lista real, PanelLateral de 400 px, edición y eliminación sin borrar la instancia, arrastre persistente, estado y DNS SRV, lanzamiento hacia la dirección vinculada y sincronización previa opcional. Falta validar conexión dentro del juego con equipos reales en Windows/Arch. |
| `RedPrivada`, `SincronizarMods` | Parcial | Pasos y láminas originales, detección y solicitud de pertenencia a ZeroTier por ID, autorización e IP reales. Servicio del anfitrión y revisión/aplicación exacta de mods de Modrinth con progreso por fila y por cambio del plan, respaldo y cancelación; prueba HTTP con mods reales. La aplicación mueve solo los archivos afectados con diario recuperable y registra cada paso en `logs/sincronizacion.log`. Faltan instalación/elevación automática de ZeroTier, salida desde la interfaz, miembros/controlador y validación con dos equipos. |
| `PantallaCuenta` | Parcial | Composición original recuperada: Apolo, cuerpo de frente/espalda, perfil/skin reales, historial local y cierre de sesión. Cambiar nombre abre Minecraft.net; gestión de varias cuentas Microsoft y capes pendiente. |
| `EstudioSkin` | Parcial | Textura real, modelo clásico/delgado, capas exteriores, rotación, selector/arrastre de PNG, validación completa de 64 × 64 y subida real a Minecraft. Falta prueba manual de subida y selector de capes propias. |
| `PantallaAjustes`, `FilaAjuste` | Parcial | Índice y filas existentes; los ajustes de Java por instancia se guardan. Falta persistir las preferencias generales y conectar los controles que todavía describen servicios no implementados. |
| `AjustesGenerales` | Parcial | Controles disponibles, abrir carpeta real y mantener/minimizar al iniciar Java. Faltan persistencia general, idioma completo, inicio con el sistema y bandeja para cerrar launcher conservando el juego. |
| `Apariencia` | Implementado | Tinta/Piedra/Sistema, Mármol/Plata/Bronce, escala de 80 a 150 % y reducir movimiento guardados; seguimiento del tema de Windows. Validación manual de escritorios Linux pendiente. |
| `AjustesJava` | Parcial | Configuración por instancia guardada y aplicada; selección automática según metadatos, detección de Java del sistema y descarga verificada del runtime oficial. Ajustes/editor muestran runtimes reales guardados; sliders limitados por la RAM real. Selección global y por instancia, rutas manuales, detección acotada de runtimes del launcher oficial y prueba con memoria/argumentos conectadas. Faltan completar recomendaciones de memoria y validación nativa en Linux. |
| `AjustesAlmacenamiento`, `Almacenamiento` | Parcial | Tamaños reales fuera del hilo de dibujo, limpieza limitada a caché, rotación de registros, traslado explícito de papeleras antiguas y recuperación de transacciones. Falta traslado de la raíz de datos y validación nativa en Linux. |
| `ActualizacionLauncher` | Parcial | Banner y pantalla originales, versión/compilación, progreso y SHA-256. El workflow publica paquetes desde main y ramas/tags de versión; instalada usa Inno Setup silencioso y portable conserva reemplazo atómico. Vuelve a la misma pantalla con el resultado tras el reinicio. Primera publicación remota y recorrido completo todavía pendientes de validar. |
| `Integridad` | Maqueta | Cuadrícula y progreso de ejemplo disponibles. Falta revisar/reparar todos los archivos reales de una instancia; la verificación de mods descargados ya existe en otro flujo. |
| `ArchivosNoVerificados` | Implementado | Lista real con SHA-512, identificación por Modrinth, caché, confiar y quitar/deshacer. Sin evidencia de malware no se inventa una acusación. Integración con futuros importes pendiente. |
| `PantallaError`, `InformeFallo` | Parcial | Fallos reales del juego con código de salida, causa desconocida cuando corresponde, registro y copia de informe en la composición original. Falta diagnóstico específico de conflictos y exportación del informe a archivo. |
| `Gabinete` | Parcial | Mosaico y visor integrados en Acerca de, junto a la versión compilada y el enlace al repositorio. Faltan sus usos originales como fondo de bienvenida en ventanas grandes y selector completo de portada, además del acercamiento y Lupa previstos. |
| `Movimiento`, `TransicionesPantalla` | A evaluar | Losa, avisos, Picar y Umbral implementados parcialmente. El launcher cambia contenido con fundido; Muro/Rastrillo/Prensa y Lupa no están aplicados como establece la referencia. Reducir movimiento existe; falta revisar el recorrido completo. |

## Orden para completar el diseño

El [detalle de implementación pendiente](pendientes-implementacion.md) define el alcance de instalación oficial de loaders, importación de modpacks y las comprobaciones que faltan.

Los nuevos instaladores deben conservar la [organización de datos](organizacion-datos.md) ya implementada: assets y bibliotecas compartidos, migración de `.icaro`, configuración por instancia y sustitución de papeleras internas por la del sistema con recuperación de los datos anteriores.

1. **Instancia jugable:** extender el instalador a loaders manteniendo los componentes ya conectados; repetir en Linux el recorrido de Vanilla confirmado en Windows. Completar historial de descargas y unificar los instaladores en la cola global.
2. **Editor y acciones:** completar General/Java, recuperación de Archivos/Capturas, prioridad de Recursos y Mundos; importar/clonar y exportar MRPACK. El cambio de versión con respaldo/copia ya está conectado; Fabric ya se prepara con la versión elegida; faltan los instaladores de Quilt, Forge y NeoForge.
3. **Cuenta:** probar subida de skin con una cuenta real, conectar capes propias y gestión persistente de varias cuentas Microsoft. La composición original y el estudio ya usan texturas reales.
4. **Servicios adicionales:** servidores, red/sincronización, integridad, integración de archivos sin verificar con futuros importes, almacenamiento y bandeja. Publicar los artefactos para validar la actualización del launcher ya implementada.
5. **Revisión transversal:** ventanas angostas, escala, metales, foco y teclado, tooltips, virtualización y movimientos pendientes; comprobar ambas paletas.

## Diferencias para evaluar

- **Cuenta:** composición original recuperada; conservarla al añadir varias cuentas y capes, usando componentes originales.
- **BibliotecaMods:** decidir dónde encaja la biblioteca entre instancias, conservando la sección Modpacks actual.
- **Movimiento:** revisar el fundido actual frente a las transiciones y efectos originales antes de cambiarlo.
- **Inicio de sesión:** conservar el recorrido por navegador ya aprobado; revisar código/QR únicamente si se retoma ese modo y se resuelve su redirección.

Los ajustes de adaptación ya conocidos —tercer filtro de versiones antiguas en el asistente y pestaña Mods oculta en Vanilla— se conservan. La [comparación con el System Design](revision-system-design.md) describe las vistas recuperadas y sus límites.

## Dónde continuar

| Área | Implementación de referencia |
| --- | --- |
| Asistente, exportación, ajustes y pantallas auxiliares | [pantallas.rs](../crates/ui/examples/launcher_aux/pantallas.rs) |
| Filas, selección, tienda y ficha rápida | [diseno_mods.rs](../crates/ui/examples/launcher_aux/diseno_mods.rs) |
| Página completa, versiones, galería y cambios | [ficha.rs](../crates/ui/examples/launcher_aux/ficha.rs) |
| Columna compartida y creadores | [info_proyecto.rs](../crates/ui/examples/launcher_aux/info_proyecto.rs) |
| Selector y migración de versión | [selector_cambio.rs](../crates/ui/examples/launcher_aux/selector_cambio.rs), [versiones_instancia.rs](../crates/core/src/versiones_instancia.rs) |
| Recursos y shaders | [packs_reales.rs](../crates/ui/examples/launcher_aux/packs_reales.rs), [mods.rs](../crates/core/src/mods.rs) |
| Actualización del launcher | [actualizacion_real.rs](../crates/ui/examples/launcher_aux/actualizacion_real.rs), [actualizacion.rs](../crates/core/src/actualizacion.rs), [contrato de publicación](actualizaciones-launcher.md) |
| Servicios y navegación | [servicios.rs](../crates/ui/examples/launcher_aux/servicios.rs), [mods_reales.rs](../crates/ui/examples/launcher_aux/mods_reales.rs) |
| Instancias, editor y archivos | [instancias_reales.rs](../crates/ui/examples/launcher_aux/instancias_reales.rs), [editor.rs](../crates/ui/src/editor.rs), [archivos.rs](../crates/ui/examples/launcher_aux/archivos.rs) |
| Componentes y tokens | [componentes.rs](../crates/ui/src/componentes.rs), [tema.rs](../crates/ui/src/tema.rs), [iconos.rs](../crates/ui/src/iconos.rs) |
| Cuenta y skin de referencia | [cuenta.rs](../crates/ui/src/cuenta.rs), [sesion.rs](../crates/ui/examples/launcher_aux/sesion.rs) |
| Núcleo conectado | [auth.rs](../crates/core/src/auth.rs), [instancias.rs](../crates/core/src/instancias.rs), [mods.rs](../crates/core/src/mods.rs), [exportar.rs](../crates/core/src/exportar.rs) |
| Vanilla, Java y ejecución | [minecraft.rs](../crates/core/src/minecraft.rs), [java.rs](../crates/core/src/java.rs), [lanzamiento.rs](../crates/core/src/lanzamiento.rs), [juego_real.rs](../crates/ui/examples/launcher_aux/juego_real.rs) |
| Revisión visual aislada | [pruebas_diseno.rs](../crates/ui/examples/launcher_aux/pruebas_diseno.rs), [guía de capturas](desarrollo.md#capturas-aisladas-del-diseño) |
