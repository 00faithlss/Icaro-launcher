# Roadmap del System Design

[Documentación](README.md) · [Estado y pendientes](roadmap.md)

Revisión documental: **11-11-2026**, contra `v0.4.x`. Esta revisión no volvió a capturar la aplicación: las evidencias visuales conservan su fecha. Referencias: `icaro-design-system.rar`, `icaro-componentes-nuevos.zip`, los paquetes de exportación/iconos y las [ampliaciones de Compartir](superpowers/referencias/compartir-instancia/README.md). El [roadmap funcional](roadmap.md) concentra prioridades y validaciones de producto.

Las siguientes entregas deben conectar y completar esos diseños, conservando sus composiciones, iconos, tipografías, colores y controles. Las diferencias conocidas se registran para evaluación antes de sustituir una vista.

## Cómo leer el estado

| Estado | Significado |
| --- | --- |
| Implementado | Está disponible en el alcance descrito. No significa que todos los usos futuros del componente estén conectados. |
| Parcial | Ya hay implementación, pero faltan partes del diseño o acciones reales. |
| Maqueta | La composición existe, pero usa ejemplos o acciones simuladas. Falta conectarla al núcleo. |
| Pendiente | Falta implementar o integrar el componente indicado. |
| A evaluar | Hay una diferencia con la referencia o una decisión de alcance que debe revisarse con el usuario. |

Una vista puede estar implementada visualmente y tener su función pendiente. Por ejemplo, Compartir instancias tiene componentes iced y núcleo probados, con controladores experimentales conectados y validación entre equipos pendiente; el diálogo de exportación genera ZIP, pero no MRPACK. Quilt, Forge y NeoForge ya tienen instalación oficial, con validaciones de partida/migración/Linux pendientes.

## Cambios recientes de la línea 0.4

- Servidores usa `PantallaServidores` con pestañas Servidores, Con amigos y Red privada; filas desplegables, formularios en la página y confirmaciones. Con amigos integra Compartes/Recibes y los pasos de compartir/unirse. Cambio posterior a la beta 0.4.2.
- Cuenta y EstudioSkin incorporan visor 3D, cuatro vistas, biblioteca, descarga y selector de capa. La cuenta muestra tiempo jugado e instancia más jugada por decisión del usuario.
- Transiciones al jugar, menús unidos a su botón, metales sobre toda la tinta, aviso de actualización, novedades y canal Estable/Beta conectados.
- Compartir conecta revisión, consentimiento, biblioteca, reconexión, orden de packs y bandeja Windows bajo activación experimental. Las capturas con fixtures y las pruebas locales no acreditan partidas ni dos equipos.

## Base visual y componentes compartidos

| Referencia original | Estado | Lo que tenemos y lo que falta |
| --- | --- | --- |
| Tokens de color, tipografía, bordes y espaciado | Parcial | Tinta/Piedra, Archivo, Martian Mono, medidas, metales Mármol/Plata/Bronce y escala conectados. Falta revisión transversal de foco, estados y Linux. |
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
| `ListaReordenable` | Pendiente | La pantalla Servidores nueva ya no ordena por arrastre, como su diseño; el orden es el de guardado, con el más reciente primero. Falta aplicarla a recursos reales y persistir sus prioridades en el juego. |
| `SelectorColor` | Pendiente | Falta conectar el selector definido a iconos/colores de instancia y revisar su composición original. |
| `ZonaSoltar` | Parcial | La importación local de modpacks conecta clic y arrastre de MRPACK/ZIP a la revisión real, con los estados de reposo y arrastre del diseño. Skins también valida PNG reales por arrastre. Falta extenderla a la importación de recursos independientes. |

## Ventana, bienvenida y sesión

| Referencia original | Estado | Lo que tenemos y lo que falta |
| --- | --- | --- |
| `AppShell` | Parcial | Barra de título, siete secciones, cuenta y controles de ventana; Ctrl+1 a Ctrl+7 y Ctrl+K. Jugar se oculta sin sesión/selección; muestra proceso y preparación reales de Vanilla. Falta completar teclado, bandeja y estados de los demás instaladores. |
| `DisenoAngosto` | Pendiente | El cambio de tamaño de ventana está implementado. Falta el conjunto original de quiebres: barra colapsada, filtros en popover e índice de Ajustes como selector. |
| `PrimeraEjecucion` | Parcial | Bienvenida sin cuenta activa, acceso a Microsoft y entrada con cuentas sin conexión guardadas. Java y preparación inicial aún no ejecutan tareas reales. |
| `SelectorCuenta`, `EstadosSesion` | Parcial | Menú con cuenta Microsoft, cuentas locales persistentes, iniciar/cerrar sesión, recuperación y errores. Falta conectar todos los estados del diseño y varias cuentas Microsoft persistentes. |
| `CuentaSinConexion` | Implementado | Perfiles locales persistentes, validación, selección y eliminación con confirmación; conserva mundos e instancias. |
| `SkinSinConexion` | Parcial | PNG local, modelo, frente/espalda, avatares y preparación del juego conectados. Pruebas de firma, servicio y agente con Java; pendiente validación en partida real. |
| `CodigoDispositivo` | A evaluar | El modo real conserva Hermes y usa autorización por navegador con retorno a Ícaro y progreso. Código/QR quedan solo en la maqueta; se retiraron del flujo real por el problema observado y el recorrido por navegador fue aprobado y probado por el usuario. |
| `EstadosPantalla`, `EstadoVacio`, `EnConstruccion` | Parcial | Componentes y estados vacíos disponibles; catálogos con carga, error y cancelación, e instancias locales recuperadas del disco. Falta aplicar el conjunto completo de carga, vacío, error y sin conexión a cada servicio. |
| `MenuBandeja` | Parcial | Bandeja Windows restaurable y cierre con drenaje conectados para Compartir experimental. Faltan validación entre equipos/reinicio de Explorer, bandeja nativa Linux y completar los usos del diseño. |

## Instancias

| Referencia original | Estado | Lo que tenemos y lo que falta |
| --- | --- | --- |
| `TarjetaInstancia`, `PanelLateral` | Parcial | Lista, grupos, portadas y metadatos persistidos. Instaladores oficiales conectados; Vanilla/Fabric confirmados en Windows. Tiempo jugado y tamaño visibles en el editor; pendientes en la lista. |
| `Asistente`, `SelectorVersion`, `SelectorLoader` | Parcial | Cuatro pasos, selectores oficiales de Minecraft/loader, portada propia y elección persistida. Instalación Vanilla/Fabric/Quilt/Forge/NeoForge conectada; partidas de los tres últimos y Linux pendientes. |
| `EditorInstancia` | Parcial | Nombre, grupo, icono/portada y Java guardados; Vanilla omite Mods. Memoria y argumentos aplicados al juego. Faltan opciones de General, resolución/servidor y validación de persistencia completa. |
| `AccionesInstancia` | Parcial | Abrir carpeta, importar MRPACK/ZIP, exportar ZIP, eliminar a papelera y Deshacer conectados. Duplicación general y acciones pendientes según el roadmap funcional. |
| `CambioVersionInstancia`, `CambiarVersion` | Parcial | Compatibilidad por hashes/lotes, recomendación desde 95 %, cambio separado de Minecraft y versión de loader, respaldo obligatorio al bajar, copia independiente y recuperación. Instaladores oficiales conectados; faltan partidas, migraciones y Linux. |
| `ImportarClonar` | Parcial | MRPACK y ZIP completos reales con revisión, clic/arrastre, hashes y recuperación. Faltan descubrimiento de otros launchers, duplicación general e importación Quilt. |
| `ExportarModpack` | Pendiente | Falta el flujo completo MRPACK: nombre, versión, autor, referencias de mods y archivos incluidos. El selector MRPACK del diálogo de instancia permanece deshabilitado. |
| `ArchivosInstancia`, `Explorador` | Parcial | Barra con iconos, árbol y tamaños, abrir/copiar ruta/renombrar, vista previa de texto y papelera. Eliminación mediante papelera del sistema; falta recuperación desde esta pestaña y navegación completa por flechas y previews de imágenes, JAR y mundos. |
| `RecursosInstancia` | Parcial | Por petición del usuario, resourcepacks y nueva pestaña Shaders reutilizan filas y controles de `ModsInstancia`: búsqueda, filtros, selección, activación, actualizaciones y retirada. ZIP e inventarios reales; recursos activos en `options.txt`, shaders en configuración existente de Iris. Versiones no declaradas requieren confirmación. Falta prioridad de resourcepacks por arrastre y soporte de otros motores de shaders. |
| `MundosInstancia` | Maqueta | Lista y panel de copias disponibles. Falta leer mundos, gestionar copias/restauración y conectar sus acciones al juego y disco. |
| `Capturas` | Parcial | PNG reales por instancia, filtros, visor, copiar/abrir carpeta en Windows y eliminación con papelera/Deshacer. Falta completar recuperación y validación nativa Linux. |
| `BotonJugar`, `Umbral` | Parcial | Estados originales conectados a instalación, preparación y ejecución Vanilla; Jugar sin MB, Preparando al verificar archivos guardados e Instalando solo al descargar archivos. En Windows el Umbral espera la ventana del proceso Minecraft y retira la capa completa con fundido, sin cortina ni remontar la vista, según la corrección solicitada por el usuario. Falta detección equivalente en Linux y cerrar launcher conservando juego mediante bandeja. |
| `Picar` | Parcial | Diez etapas y eliminación de instancia/modpack instalado manteniendo el botón; soltar cancela. Revisa respaldos de mundos y exige confirmación si no hay copia vigente. Papelera y Deshacer. Faltan los recorridos de mundos y archivos individuales. |

## Mods y modpacks

| Referencia original | Estado | Lo que tenemos y lo que falta |
| --- | --- | --- |
| `FilaMod` | Implementado | Casilla, interruptor, icono, nombre/creador, descripción, estados, versión y acciones; desactivados atenuados y conflictos identificados. |
| `ModsInstancia` | Implementado | Inventario real, filtros, selección múltiple, activar/desactivar, actualizaciones con confirmación, quitar a papelera y Deshacer. |
| `TiendaMods` | Parcial | Composición original, filtros laterales, contexto de Minecraft/loader, búsqueda, filas y lista inferior de instalación. Modrinth conectado; CurseForge visible pero deshabilitado. |
| `FichaMod` | Implementado | Panel rápido con acciones, miniaturas, versiones compatibles, nombres/estado de dependencias y selección de opcionales. Ver página y Volver conservan la navegación anterior. |
| `PaginaMod` (ZIP nuevo, 12-10) | Parcial | Cabecera, galería 3 columnas 16:9, tabla y columna de 320 px con InfoProyecto y creadores. MRPACK Vanilla/Fabric/Forge/NeoForge conectados, también importación local. Perfil interno del creador pendiente de diseño; datos no publicados se omiten. |
| `InfoProyecto` (ZIP nuevo, 12-10) | Parcial | Versiones mono, tres filas, snapshots opcionales, `.x` solo con manifiesto que prueba toda la familia; plataformas, entornos, enlaces, etiquetas, creadores y detalles reales, compartidos con el panel rápido. Faltan conteos de contenido del MRPACK y datos fiables de rendimiento/mínimos de shaders; no se simulan. |
| `VisorCaptura` | Parcial | Compartido entre capturas locales y galería de proyectos; originales, zoom, arrastre, flechas y Escape. Conserva barra de búsqueda, arrastre y controles de ventana. Copiar y abrir carpeta conectados en Windows; eliminación reversible pendiente. |
| `BibliotecaMods` | A evaluar | El original reúne un mod y todas las instancias que lo usan. La sección actual es **Modpacks**, según el alcance trabajado, y los mods se gestionan por instancia. Esa biblioteca global no está implementada; evaluar su ubicación antes de añadir una sección. |

## Servicios, ajustes y pantallas auxiliares

| Referencia original | Estado | Lo que tenemos y lo que falta |
| --- | --- | --- |
| `PantallaDescargas`, `ColaDescargas` | Parcial | Cola real de MRPACK/Vanilla/Fabric/Java con pausa, reanudación, cancelación y reintento en las filas originales. Quitar un registro terminado o fallido oculta su progreso sin borrar la instancia ni sus archivos. Falta historial persistente y unificar mods y demás instaladores en la cola global. |
| `PantallaConsola`, `Consola` | Implementado | Barra interior del diseño con filtros independientes y conteos, búsqueda con resaltado, autoscroll con pausa al subir e Ir al final, copiar/limpiar e iconos. Render de filas visibles, selector por instancia, último registro desde latest.log, exportación TXT y vacío con San Jerónimo. Registros en vivo acotados y credenciales ocultas. |
| `PantallaServidores`, `TarjetaServidor` | Parcial | Banda de 150 px y pestañas Servidores/Con amigos/Red privada. Búsqueda, formulario con consulta de dirección, filas desplegables, confirmación al quitar y reintento. Compartes/Recibes integran publicación, sincronización y salida conservando archivos. Falta validación entre equipos. |
| `RedPrivada`, `SincronizarMods` | Parcial | ZeroTier con consulta, unión, IP, invitación copiada y desconexión confirmada; faltan instalación/permisos y miembros. Host HTTP con revisión/aplicación exacta, progreso, respaldo, cancelación y diario recuperable; falta validación entre equipos. |
| `PantallaCuenta` | Parcial | Apolo, visor 3D, perfil/skin, tiempo total e instancia más jugada; historial, descarga y selector de capa. Cambiar nombre abre Minecraft.net. Varias cuentas Microsoft y validación de operaciones reales pendientes. |
| `EstudioSkin` | Parcial | Modelo clásico/delgado, cuatro vistas, busto/caminata, capas, vista plana, historial/biblioteca, PNG 64 × 64 y 64 × 32, subida y descarga. Falta validación con cuenta real. |
| `PantallaAjustes`, `FilaAjuste` | Parcial | Índice y filas existentes; los ajustes de Java por instancia se guardan. Falta completar las opciones generales pendientes y conectar los controles que todavía describen servicios no implementados. |
| `AjustesGenerales` | Parcial | Controles disponibles, abrir carpeta real y mantener/minimizar al iniciar Java. Faltan opciones generales, idioma completo e inicio con el sistema; validar los comportamientos de cierre y ampliar la bandeja fuera de Compartir. |
| `Apariencia` | Implementado | Tinta/Piedra/Sistema, Mármol/Plata/Bronce, escala de 80 a 150 % y reducir movimiento guardados; seguimiento del tema de Windows. Validación manual de escritorios Linux pendiente. |
| `AjustesJava` | Parcial | Configuración por instancia guardada y aplicada; selección automática según metadatos, detección de Java del sistema y descarga verificada del runtime oficial. Ajustes/editor muestran runtimes reales guardados; sliders limitados por la RAM real. Selección global y por instancia, rutas manuales, detección acotada de runtimes del launcher oficial y prueba con memoria/argumentos conectadas. Faltan completar recomendaciones de memoria y validación nativa en Linux. |
| `AjustesAlmacenamiento`, `Almacenamiento` | Parcial | Tamaños reales fuera del hilo de dibujo, limpieza limitada a caché, rotación de registros, traslado explícito de papeleras antiguas y recuperación de transacciones. Falta traslado de la raíz de datos y validación nativa en Linux. |
| `ActualizacionLauncher` | Parcial | Aviso flotante, novedades, canal Estable/Beta, copias, progreso y SHA-256. Publicaciones desde develop/main; setup silencioso y reemplazo portable. Ciclo nativo completo con paquetes publicados pendiente. |
| `Integridad` | Maqueta | Cuadrícula y progreso de ejemplo disponibles. Falta revisar/reparar todos los archivos reales de una instancia; la verificación de mods descargados ya existe en otro flujo. |
| `ArchivosNoVerificados` | Implementado | Lista real con SHA-512, identificación por Modrinth, caché, confiar y quitar/deshacer. Sin evidencia de malware no se inventa una acusación. Integración con futuros importes pendiente. |
| `PantallaError`, `InformeFallo` | Parcial | Fallos reales del juego con código de salida, causa desconocida cuando corresponde, registro y copia de informe en la composición original. Falta diagnóstico específico de conflictos y exportación del informe a archivo. |
| `Gabinete` | Parcial | Mosaico y visor integrados en Acerca de, junto a la versión compilada y el enlace al repositorio. Faltan sus usos originales como fondo de bienvenida en ventanas grandes y selector completo de portada, además del acercamiento y Lupa previstos. |
| `Movimiento`, `TransicionesPantalla` | Parcial | Losa, avisos, Picar, fundidos y transiciones al jugar Marea/Horizonte/Tinta/Bloques/Respiro/Umbral; reducir movimiento y ajustes conectados. Muro/Rastrillo/Prensa y Lupa pendientes de revisión respecto de la referencia. |

## Orden para completar el diseño

El [roadmap funcional](roadmap.md) define prioridades, alcance y criterios de aceptación. La revisión visual acompaña cada cambio: conservar composiciones originales, comprobar Tinta/Piedra, ventana pequeña, teclado/foco y escala, con capturas reales. Ningún hover puede ocultar bordes; iconos y rótulos se centran con padding simétrico y medidas uniformes.

## Diferencias para evaluar

- **Cuenta:** composición original recuperada; conservarla al añadir varias cuentas Microsoft y validar las capas ya conectadas, usando componentes originales.
- **BibliotecaMods:** decidir dónde encaja la biblioteca entre instancias, conservando la sección Modpacks actual.
- **Movimiento:** revisar el fundido actual frente a las transiciones y efectos originales antes de cambiarlo.
- **Inicio de sesión:** conservar el recorrido por navegador ya aprobado; revisar código/QR únicamente si se retoma ese modo y se resuelve su redirección.

Los ajustes de adaptación ya conocidos —tercer filtro de versiones antiguas en el asistente y pestaña Mods oculta en Vanilla— se conservan. La antigua comparación se consolidó en este inventario.

## Decisiones respecto de las referencias

- Recursos y Shaders reutilizan `FilaMod`/`ModsInstancia` por petición del usuario; la prioridad por arrastre sigue pendiente.
- Cuenta sustituye UUID, correo y estado de sesión por tiempo jugado total e instancia más jugada. Selección de capas ya conectada; falta validación real.
- El acceso Microsoft usa navegador y retorno directo; código/QR quedan en demo.
- `InfoProyecto` omite métricas internas del MRPACK, motivos de incompatibilidad y rendimiento de shaders que el proveedor no publica; los perfiles de creadores abren Modrinth.
- MRPACK de exportación sigue deshabilitado; no presentar un manifiesto aislado como modpack completo.
- `SkinSinConexion` conserva ZonaSoltar, frente/espalda, modelo y estados del paquete original. Las capturas previas cubrieron 1280 × 800 y 940 × 720 en ambos temas, no una partida.
- Minimizar una publicación no la pausa; los acuerdos del usuario prevalecen sobre el texto de la preview.

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
