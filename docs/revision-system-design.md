# Comparación con el System Design

Referencias: `icaro-design-system.rar`, entregado el 11 de octubre de 2026, e `icaro-componentes-nuevos.zip`, entregado el 12 de octubre de 2026 (InfoProyecto, PaginaMod y CambioVersionInstancia). Las definiciones y previews originales son la referencia para la composición, los controles, iconos Pixelarticons, tipografías, colores y medidas. Se revisan las pantallas con capturas de los widgets reales en Tinta y Piedra; los datos de las capturas son fixtures, separados de las cuentas e instancias del usuario.

El inventario completo de avances y pendientes por componente está en el [Roadmap del System Design](roadmap-system-design.md). Las próximas entregas funcionales se siguen en el [roadmap general](../README.md#roadmap).

## Vistas recuperadas

| Componente original | Implementación |
| --- | --- |
| `Asistente` | Diálogo de 880 px y cuatro pasos: `SelectorVersion` completo en el primero y `SelectorLoader` en el segundo, con sus radios de rombo, Quilt y versiones seleccionables. Se conservan los pasos de nombre/icono y revisión y el pie original. Mojang y los catálogos oficiales proporcionan las versiones reales. |
| `FilaMod` y `ModsInstancia` | Filas con separadores, casilla de selección, interruptor, icono, nombre, creador, descripción, estado, versión y acciones. La selección muestra la barra invertida para activar, desactivar, actualizar y quitar. Quitar conserva los archivos en la papelera y ofrece Deshacer durante diez segundos. |
| `TiendaMods` | Filtros laterales, encabezado con instancia, Minecraft y loader, búsqueda a la derecha, filas compactas con categoría, creador y descargas. Agregar prepara una lista; la barra inferior invertida enumera los mods, sus dependencias y el tamaño antes de Instalar. Volver a mis mods recupera la lista instalada. |
| `FichaMod` | Panel lateral de 640 px con acciones, descripción, tres miniaturas, versiones compatibles, nombres de dependencias y casillas para las opcionales. Las imágenes abren el visor existente. Ver página abre la ficha completa; Volver recupera el panel anterior. |
| `PaginaMod` | Referencia nueva: icono de 64 px, autor enlazado, categorías/lado y resumen; galería en tres columnas 16:9 con destacada de dos columnas; tabla con filtros, canales y compatibilidad real, grupos de dependencias. Columna fija de 320 px con acción principal, aviso, InfoProyecto y tres mods reales más descargados del dueño. Galería se oculta si no hay imágenes. |
| `InfoProyecto` | Referencia nueva: compatibilidad en fichas mono (máximo tres filas), snapshots opcionales, familia `.x` solo demostrada por el manifiesto oficial completo; plataformas, entornos, enlaces con sus SVG exactos, etiquetas, avatares/roles/Dueño y detalles. Compartido por página y parte baja del panel rápido. Datos no publicados se omiten. |
| `AccionesInstancia`: exportar | Diálogo de 560 px con formatos ZIP/MRPACK, casillas de contenido, tamaños, destino con icono de carpeta y selector nativo, estimación y acciones del pie. ZIP exporta los archivos reales seleccionados y los metadatos de la instancia. |
| `ArchivosInstancia` | Barra de ruta y acciones con sus iconos; árbol con tamaños a la izquierda y vista previa de solo lectura a la derecha, dentro del mismo marco. Renombrar edita el nombre dentro del árbol; Enter confirma y Escape cancela. |
| `CambioVersionInstancia` y `CambiarVersion` | Selector nuevo del ZIP: filtros, búsqueda, versión actual deshabilitada, recomendación desde 95 % y compatibilidad de todos los mods instalados mediante hashes y consultas por lotes. Resumen con loader, actualizaciones, desactivaciones, respaldo y copia. Confirmación original con lámina y riesgo; descarga los reemplazos antes de modificar la instancia y recupera los archivos anteriores si falla. |
| `ActualizacionLauncher` | Saturno lateral, versión y notas reales, acciones Actualizar ahora, Al cerrar Ícaro y Más tarde. Comprobación y descarga verificada conectadas; falta publicar una release portable para verificar la actualización completa. |

La confirmación de actualización conserva `Dialogo`, el velo con losa y la caída de Faetón. Vanilla omite la pestaña y el contador de mods, manteniendo el resto del editor.

El 12 de octubre el usuario pidió expresamente que resourcepacks y shaders compartieran el mismo orden y diseño de filas y opciones que mods. Se reutilizan `FilaMod` y `ModsInstancia` para ambas listas, con una pestaña Shaders junto a Recursos; esta sustitución de `RecursosInstancia` sigue esa indicación. Los shaders se descargan como ZIP y pueden activarse con la configuración existente de Iris; Vanilla por sí solo no los renderiza. La prioridad por arrastre de resourcepacks queda pendiente.

El selector de versiones conserva su marco durante hover y selección y usa los estados de instalación en memoria, sin revisar archivos de Minecraft durante el render. El panel rápido cierra con clic fuera; volver desde la página completa restaura la vista de origen. Los avatares de los miembros y la organización se cargan desde las URLs publicadas por Modrinth, incluidos los avatares de GitHub.

## Diferencias y funciones pendientes

- **InfoProyecto/PaginaMod nuevos**: la API v2 no entrega recuentos internos del MRPACK ni mínimo del lector/impacto de shaders; no se presentan cifras ni requisitos inventados. Resolución se muestra solo si figura como categoría publicada; tamaño usa el archivo de la versión elegida. Perfil del creador dentro de la tienda no tiene componente entregado: avatar y Ver perfil abren por ahora el perfil público de Modrinth. El motivo textual de una incompatibilidad tampoco se inventa cuando la API solo declara su tipo.

- **Cuenta (`PantallaCuenta`)** recupera la composición original por petición del usuario: Apolo, cuerpo de frente/espalda, perfil y PNG reales, historial local y EstudioSkin. Modelo, capas exteriores, giro, carga/arrastre y subida están conectados. Falta probar la subida con una cuenta real y conectar capes propias/varias cuentas Microsoft; el cambio de nombre abre el perfil oficial.
- **Inicio de sesión (`CodigoDispositivo`)** usa el flujo por navegador y retorno directo a Ícaro solicitado y probado por el usuario. No muestra código ni QR del flujo que fallaba; conserva la lámina de Hermes y el progreso real.
- **CurseForge** aparece como fuente deshabilitada, en el lugar definido por el diseño. No hay una conexión simulada en el modo real.
- **MRPACK** conserva su selector, pero no permite exportar hasta disponer de instalación real de Minecraft y de la versión concreta del loader. No genera un JSON aislado presentado como modpack.
- Crear Vanilla o Fabric guarda su configuración y prepara Minecraft, el perfil y Java; Jugar conecta los estados originales, registros y fallo. Fabric 26.3 fue confirmado con un mod real en Windows. Quilt se instala igual que Fabric; Forge y NeoForge siguen pendientes. Los MRPACK de Modrinth crean instancias Vanilla/Fabric; importar archivos locales y duplicar instancias desde las acciones generales siguen pendientes.
- El explorador envía las eliminaciones a la papelera del sistema; la recuperación desde su propia pestaña todavía requiere conexión al flujo completo definido en el diseño.

## Implementación

- Asistente y exportación: [pantallas.rs](../crates/ui/examples/launcher_aux/pantallas.rs).
- Tienda, filas, selección, lista de instalación y ficha rápida: [diseno_mods.rs](../crates/ui/examples/launcher_aux/diseno_mods.rs).
- Página completa: [ficha.rs](../crates/ui/examples/launcher_aux/ficha.rs).
- Columna y creadores: [info_proyecto.rs](../crates/ui/examples/launcher_aux/info_proyecto.rs).
- Cambio de versión: [selector_cambio.rs](../crates/ui/examples/launcher_aux/selector_cambio.rs), [versiones_instancia.rs](../crates/core/src/versiones_instancia.rs).
- Resourcepacks y shaders: [packs_reales.rs](../crates/ui/examples/launcher_aux/packs_reales.rs), [mods.rs](../crates/core/src/mods.rs).
- Actualización del launcher: [actualizacion_real.rs](../crates/ui/examples/launcher_aux/actualizacion_real.rs), [contrato y límites](actualizaciones-launcher.md).
- Navegación y servicios: [servicios.rs](../crates/ui/examples/launcher_aux/servicios.rs), [mods_reales.rs](../crates/ui/examples/launcher_aux/mods_reales.rs).
- Archivos: [editor.rs](../crates/ui/src/editor.rs), [archivos.rs](../crates/ui/examples/launcher_aux/archivos.rs).
- Exportación ZIP y papelera de mods: [exportar.rs](../crates/core/src/exportar.rs), [mods.rs](../crates/core/src/mods.rs).
- Capturas aisladas para revisión: [pruebas_diseno.rs](../crates/ui/examples/launcher_aux/pruebas_diseno.rs).

## Cuentas sin conexión (0.2.5)

CuentaSinConexion sustituye Invitado por perfiles locales persistentes. SkinSinConexion conserva el panel entregado: ZonaSoltar rayada, iconos, frente/espalda, modelo, casilla de capa exterior y estados vacío/error/guardando. Los avatares usan el PNG guardado. Se revisaron ambos temas a 1280 × 800 y 940 × 720; la aplicación dentro de una partida y la visibilidad en servidores siguen pendientes. Ver [guía](cuentas-sin-conexion.md).
