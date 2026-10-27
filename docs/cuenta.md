# Cuenta y estudio de skin

Las [cuentas sin conexión](cuentas-sin-conexion.md) reemplazan Invitado con perfiles locales guardados. Su botón **Skin** abre el componente local `SkinSinConexion`, con PNG, modelo y vista previa. El estudio, historial y servicios descritos a continuación corresponden a Microsoft.

**Gestionar cuenta y skin**, en el menú de la barra lateral, abre `PantallaCuenta` según el System Design actualizado: Apolo Belvedere, visor 3D de la skin con giro de a 90°, Abrir estudio y Descargar skin, y los bloques Perfil, Capa, Historial de skins y cierre de sesión. Perfil muestra el nombre, cuándo se puede cambiar, el tiempo jugado sumando todas las instancias y la instancia más jugada; por decisión del usuario no muestra UUID, correo ni el estado de la sesión. **Abrir estudio** usa `EstudioSkin`: visor 3D con vista 3D o plana, encuadre de cuerpo o busto, caminata, caras como segmentado, y un panel con modelo, capas visibles junto a la textura plana (marca la zona de cada capa), capa, biblioteca, Cargar .png, Descargar y Aplicar a la cuenta. Se conservan los componentes, medidas, fuentes, iconos y temas del System Design; los iconos de los botones van centrados en ambos ejes.

El perfil y la skin activa se consultan en Minecraft Services. El PNG se descarga exclusivamente desde `textures.minecraft.net`, sin enviar allí credenciales. La cuenta solo muestra datos disponibles; no inventa un correo, la fecha del próximo cambio de nombre ni otras cuentas conectadas. **Cambiar** abre el perfil oficial de Minecraft.net. Cambiar de nombre dentro del launcher queda pendiente.

## Skin real

- **Cargar .png** usa el selector nativo; soltar un archivo sobre el estudio hace la misma validación.
- Se admite PNG completo de 64 × 64, o del formato antiguo de 64 × 32 (se expande copiando en espejo brazo y pierna como hace Minecraft), hasta 1 MB. Se comprueba cabecera y decodificación; una cabecera aislada no basta.
- Se renderizan los píxeles reales, el modelo clásico/delgado y las capas exteriores de sombrero, chaqueta, mangas y pantalones. Las casillas controlan la vista previa; no son capes de la cuenta.
- El visor 3D solo descansa en Frente, Lado derecho, Espalda y Lado izquierdo. Flechas (con el cursor sobre el visor), botones y arrastre (70 px por cara) giran de a 90° con una animación breve; con movimiento reducido el giro es inmediato y la caminata se desactiva. Al elegir una capa, el visor gira a la espalda para mostrarla.
- **Capa** lista las capes de la cuenta más Ninguna. Elegir una la aplica al instante en Minecraft Services (`PUT`/`DELETE /minecraft/profile/capes/active`) y muestra la carga en la miniatura; sus texturas se descargan solo desde `textures.minecraft.net`.
- La **biblioteca** del estudio es el historial local de skins: elegir una la carga en el visor y **Guardar actual** la añade. **Descargar** guarda el PNG que se ve con el diálogo de guardado del sistema.
- En la cuenta, elegir una skin del historial la muestra en el visor con la barra «Vista previa · Descartar · Usar esta skin».
- **Aplicar a la cuenta** envía el PNG y el modelo a Minecraft Services. Durante la subida se mantiene la escena y se bloquean cambios de modelo/textura; un error conserva la selección para reintentar.
- Después de una respuesta válida se actualizan cuerpo y cabeza de la barra lateral. Una respuesta de otra cuenta o de una operación anterior no reemplaza la cuenta actual.
- El historial local conserva las últimas ocho skins disponibles para esa cuenta; **Usar esta skin** hace una subida real. Se guarda en `Icaro/accounts/<UUID>/skins/`, con nombres SHA-256 y modelo. Contiene PNG, nunca tokens. No pretende recuperar skins utilizadas antes de Ícaro.

Al abrir Ícaro, el último nombre de perfil disponible se recupera del almacén protegido y puede mostrarse mientras la sesión se renueva en segundo plano. Los registros anteriores obtienen el nombre al renovarse correctamente. Si falla la renovación, el perfil se muestra como vencido y no permite operaciones que requieran autenticación; volver a entrar para recuperarlas.

Los tokens permanecen en el núcleo y en el almacén protegido existente. El launcher sigue recordando una cuenta por equipo. Si Minecraft rechaza la sesión, se muestra el error y se puede volver a iniciar sesión desde el menú de cuenta.

## Validación y límites

Hay pruebas de PNG completos, límites y URLs oficiales, composición UV de frente/espalda, modelo delgado y capas. La subida se conectó siguiendo las solicitudes públicas de [Prism](https://github.com/PrismLauncher/PrismLauncher/blob/develop/launcher/minecraft/skins/SkinRequests.cpp). Las pruebas no modifican una cuenta real: queda pendiente comprobar manualmente una subida y reutilización del historial con la cuenta del usuario.

Gestión persistente de varias cuentas Microsoft y cambio de nombre dentro del launcher siguen pendientes. El cambio de capa y la descarga de capes no se han probado todavía con una cuenta real. El modo `ICARO_DEMO=1` conserva las muestras originales y su subida simulada; sus resultados nunca se presentan como una subida real.

Las escenas `cuenta-original` y `estudio-original` permiten revisar las composiciones en el capturador descrito en [desarrollo](desarrollo.md). El login se describe en [inicio de sesión y catálogo](login-y-catalogo.md).
