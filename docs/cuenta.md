# Cuenta y estudio de skin

Las [cuentas sin conexión](cuentas-sin-conexion.md) reemplazan Invitado con perfiles locales guardados. Su botón **Skin** abre el componente local `SkinSinConexion`, con PNG, modelo y vista previa. El estudio, historial y servicios descritos a continuación corresponden a Microsoft.

**Gestionar cuenta y skin**, en el menú de la barra lateral, abre `PantallaCuenta` original: Apolo Belvedere, nombre, skin de frente/espalda, historial y cierre de sesión. **Abrir estudio** usa `EstudioSkin` original, con dos cuerpos, suelo de líneas, giro, modelo, capas y acciones. Se conservan los componentes, medidas, fuentes, iconos y temas del System Design.

El perfil y la skin activa se consultan en Minecraft Services. El PNG se descarga exclusivamente desde `textures.minecraft.net`, sin enviar allí credenciales. La cuenta solo muestra datos disponibles; no inventa un correo, la fecha del próximo cambio de nombre ni otras cuentas conectadas. **Cambiar** abre el perfil oficial de Minecraft.net. Cambiar de nombre dentro del launcher queda pendiente.

## Skin real

- **Cargar .png** usa el selector nativo; soltar un archivo sobre el estudio hace la misma validación.
- Se admite PNG completo de 64 × 64 y hasta 1 MB. Se comprueba cabecera y decodificación; una cabecera aislada no basta.
- Se renderizan los píxeles reales, el modelo clásico/delgado y las capas exteriores de sombrero, chaqueta, mangas y pantalones. Las casillas controlan la vista previa; no son capes de la cuenta.
- Flechas, botones y arrastre mantienen el giro original en pasos de 45°.
- **Aplicar a la cuenta** envía el PNG y el modelo a Minecraft Services. Durante la subida se mantiene la escena y se bloquean cambios de modelo/textura; un error conserva la selección para reintentar.
- Después de una respuesta válida se actualizan cuerpo y cabeza de la barra lateral. Una respuesta de otra cuenta o de una operación anterior no reemplaza la cuenta actual.
- El historial local conserva las últimas ocho skins disponibles para esa cuenta; **Usar de nuevo** hace una subida real. Se guarda en `Icaro/accounts/<UUID>/skins/`, con nombres SHA-256 y modelo. Contiene PNG, nunca tokens. No pretende recuperar skins utilizadas antes de Ícaro.

Al abrir Ícaro, el último nombre de perfil disponible se recupera del almacén protegido y puede mostrarse mientras la sesión se renueva en segundo plano. Los registros anteriores obtienen el nombre al renovarse correctamente. Si falla la renovación, el perfil se muestra como vencido y no permite operaciones que requieran autenticación; volver a entrar para recuperarlas.

Los tokens permanecen en el núcleo y en el almacén protegido existente. El launcher sigue recordando una cuenta por equipo. Si Minecraft rechaza la sesión, se muestra el error y se puede volver a iniciar sesión desde el menú de cuenta.

## Validación y límites

Hay pruebas de PNG completos, límites y URLs oficiales, composición UV de frente/espalda, modelo delgado y capas. La subida se conectó siguiendo las solicitudes públicas de [Prism](https://github.com/PrismLauncher/PrismLauncher/blob/develop/launcher/minecraft/skins/SkinRequests.cpp). Las pruebas no modifican una cuenta real: queda pendiente comprobar manualmente una subida y reutilización del historial con la cuenta del usuario.

Gestión persistente de varias cuentas Microsoft, selección de capes propias y cambio de nombre dentro del launcher siguen pendientes. El perfil consulta las capes, pero no añade una pantalla de selección nueva sin el componente aprobado. El modo `ICARO_DEMO=1` conserva las muestras originales y su subida simulada; sus resultados nunca se presentan como una subida real.

Las escenas `cuenta-original` y `estudio-original` permiten revisar las composiciones en el capturador descrito en [desarrollo](desarrollo.md). El login se describe en [inicio de sesión y catálogo](login-y-catalogo.md).
