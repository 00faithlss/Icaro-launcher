# Instancias locales y archivos sin verificar

Las preferencias de sensibilidad, controles y gráficos se comparten automáticamente entre instancias mediante una mezcla de opciones compatibles al iniciar y al terminar Minecraft. Consulta [preferencias compartidas](opciones-minecraft.md) para el alcance, las teclas de mods y el comportamiento con sesiones simultáneas.

Las instancias nuevas usan `Icaro/instances/<nombre>/minecraft`, bajo el directorio de datos del usuario. Por ejemplo, una instancia llamada `boys` queda en `%APPDATA%\Icaro\instances\boys\minecraft` en Windows. El identificador interno sigue siendo estable: cuentas, preferencias, trabajos y registros no dependen del nombre de la carpeta.

Los caracteres reservados del sistema de archivos se sustituyen por `_`. Los nombres reservados de Windows reciben el prefijo `Instancia`, y las colisiones entre nombres saneados reciben `(2)`, `(3)`, etc. No se sobrescriben carpetas ajenas que ocupen ese nombre. Dos instancias con el mismo nombre visible, ignorando mayúsculas, siguen rechazándose.

## Metadatos y migración

La [organización de datos](organizacion-datos.md) describe los assets y bibliotecas compartidos, la configuración por instancia y la papelera del sistema ya conectados.

Cada carpeta contiene `instance.json` de esquema 2 como única configuración editable. Guarda identidad y ajustes de Java/memoria; el icono personalizado se conserva por separado. `.icaro` contiene el perfil instalado, los nativos y, cuando corresponde, el estado del modpack; los archivos del juego permanecen en `minecraft/`.

Los antiguos `instance.cfg` se leen durante la migración; sus claves se conservan en `legacy_cfg` y el CFG se retira después de publicar el JSON. Un JSON presente pero corrupto no se sustituye silenciosamente. Los campos desconocidos se preservan al guardar.

Al cargar el almacén, las carpetas antiguas cuyo nombre es el ID se migran mediante un cambio de nombre dentro del mismo almacén. No se copian ni borran mundos. Si el sistema bloquea la carpeta o falla la publicación de los metadatos, se conserva la carpeta anterior. La siguiente carga puede volver a intentar la migración. `.icaro-minecraft` se renombra a `.icaro`, actualizando las referencias del perfil; una colisión entre ambos nombres detiene la migración.

Renombrar una instancia cambia su carpeta y conserva el ID y todos sus archivos. Si falla la publicación de los metadatos, se revierte el cambio de carpeta y la configuración anterior. Mientras Minecraft o una instalación usan la instancia, la interfaz impide este cambio. Eliminar envía la carpeta a la papelera del sistema; Deshacer restaura durante la sesión y elige un nombre disponible. Un fallo no provoca borrado permanente.

**Cambiar versión** modifica únicamente Minecraft y conserva la familia del loader. **Cambiar versión del loader** conserva Minecraft, la familia, los mods y los mundos; permite elegir otra versión publicada del mismo loader y respaldar los archivos. Fabric y Quilt se preparan con la versión guardada; Forge y NeoForge requieren sus instaladores pendientes.

## ArchivosNoVerificados

La pantalla usa el componente y el grabado `TorreReja` del sistema de diseño. La revisión calcula SHA-512 en segundo plano y consulta los hashes exactos en Modrinth. Los resultados se cachean por contenido durante un día; una sustitución del archivo obliga a revisar el contenido nuevo. Se revisan también los JAR registrados cuyo contenido no coincide con su hash guardado.

- **Buscar en Modrinth** abre una búsqueda por nombre; no elige ni descarga automáticamente el primer resultado.
- **Confiar** guarda una decisión para ese SHA-512 exacto, sin convertirla en una verificación de Modrinth.
- **Quitar** retira el archivo de forma reversible y actualiza su registro si lo tenía. **Deshacer** no sobrescribe un archivo nuevo con el mismo nombre y comprueba que el respaldo conserve su hash.
- **Desactivar** está disponible para que un importador aplique una decisión o evidencia concreta del proveedor; conserva el JAR con extensión `.disabled`.

Una consulta fallida deja los archivos como procedencia desconocida y muestra el error. No desactiva archivos ni afirma que sean maliciosos. El endpoint de hashes utilizado no proporciona una marca de malware, por lo que esa fila del diseño sólo podrá mostrarse si una fuente futura entrega evidencia explícita. La pantalla está conectada a los mods locales actuales y preparada para que futuros importadores la reutilicen; no sustituye la implementación del importador.

Las pruebas de migración, colisiones, renombrado, rollback, confianza, retirada y restauración utilizan exclusivamente carpetas temporales aisladas.

## Icono de instancia

Haz clic en el icono del editor o elige **Cambiar icono** en sus acciones. Admite PNG, JPG, WebP o GIF de hasta 10 MB; recorta en cuadrado con arrastre, flechas, zoom de 100 a 400 % y estilos Original/Píxeles 32/Píxeles 16. También puedes elegir una captura de la instancia o una de doce variaciones procedurales. Se guarda `icon.png` de 256 × 256 junto a `icono.json`; Deshacer recupera el anterior durante la sesión.

La vista previa se actualiza directamente en RGBA con trabajo limitado a 256 × 256 muestras. El estilo Original se guarda con Lanczos sobre la imagen original en segundo plano; Píxeles 16/32 conserva los bloques de la vista previa. Cambiar la fuente invalida la imagen anterior mientras se decodifica la nueva.

Eliminar muestra una revisión de respaldos con **Continuar con la eliminación**, seguida de la confirmación con nombre, consecuencias, diez bloques de avance y grietas sobre toda la tarjeta. Soltar o pulsar **Cancelar** interrumpe la confirmación antes de iniciar la operación. Cancelar queda alineado con el pie de la tarjeta. Los archivos se conservan en la papelera del sistema. En Capturas dentro de la instancia, el estado vacío explica F2 y ofrece Jugar para esa instancia.

## Importar mods propios

En **Instancia → Mods → Importar .jar** se eligen uno o varios archivos propios. La copia se realiza fuera del hilo de interfaz y el lote completo se conserva sin cambios si falla un archivo o hay un nombre ocupado, incluso si el mod existente está desactivado. Se importan como archivos locales: no se descargan dependencias ni se certifica su compatibilidad. Minecraft y las demás operaciones de la instancia deben haber terminado.

## Exportar

ExportarInstancia muestra un árbol con carpetas, casillas mixtas, tamaños y selecciones Recomendada/Todo/Para compartir/Solo mundos. Genera ZIP de Ícaro o Prism/MultiMC; el segundo necesita la versión concreta del loader y usa `mmc-pack.json` con el esquema oficial. Sus contenidos se verifican con pruebas; la importación en las aplicaciones externas sigue pendiente de comprobación manual. MRPACK pertenece a otro componente.

En **Mods → más opciones → Exportar lista…** se exporta todo o la selección actual, con **Solo mods activos** marcado inicialmente. HTML, Markdown, texto, JSON, CSV y plantilla personalizada no copian los JAR. Nombre es obligatorio; versión, autores, URL y archivo son opcionales. Buscar actualizaciones completa metadatos mediante SHA-512 y Modrinth sin instalar. Un JAR reemplazado pierde la identificación antigua hasta reconocer su contenido actual. Copiar comprueba el portapapeles; Guardar abre el selector del sistema y muestra un aviso con acceso a la carpeta.

## Validación del 12-10-2026

Correcciones posteriores: el menú de Mods sigue la posición actual del botón al redimensionar. Agregar en tiendas de recursos/shaders prefiere un ZIP compatible (estable, si existe), admite beta y permite otras versiones con la confirmación de compatibilidad existente. ArchivosNoVerificados mantiene su lámina en 200 × 420 px y el título en líneas de 34 px, evitando que ocupe miles de píxeles por interpretar el interlineado como multiplicador.

Pasaron 104 pruebas del núcleo, 52 de componentes y 31 del launcher; siete pruebas que requieren servicios o condiciones externas quedaron ignoradas. Las pruebas de regresión cubren el cambio de pestañas y de anchura en páginas de mods, recursos y shaders, además de revisión de respaldos y cancelación de Picar. Se generaron e inspeccionaron capturas del ejecutable para exportación de instancia, iconos, Picar y exportación de lista. La importación manual del ZIP en Prism/MultiMC continúa pendiente.
