# Referencias de Compartir instancia

[Historial técnico](../../README.md) · [Roadmap visual](../../../roadmap-system-design.md)

Paquetes entregados por el usuario, conservados byte a byte como referencia del diseño. No son código integrado en el launcher ni instrucciones que prevalezcan sobre los acuerdos de producto.

1. [icaro-compartir-instancia.zip](icaro-compartir-instancia.zip): paquete base, versión interna 29; 13 componentes y PantallaServidores adaptada, con fuentes, grabados, tokens y previews.
2. [icaro-revisar-publicacionv2.zip](icaro-revisar-publicacionv2.zip): ampliación, versión interna 31; añade RevisionPublicacion y actualiza PanelPublicacion, EntradaInvitacion, InvitacionCompartir, SelectorInstanciaDestino y los recursos compartidos. No sustituye los componentes ausentes del paquete base.

Para consultar las previews, extraer el primero en una carpeta de consulta y superponer el contenido interior del segundo: las carpetas raíz de ambos ZIP tienen nombres diferentes. Mantener los dos originales intactos. Cada componente incluye README y preview.html; `#piedra` selecciona el tema claro.

El usuario exige conservar el diseño y los iconos tal cual. Los [acuerdos de producto](../../specs/2026-11-07-sincronizacion-entre-launchers-design.md) documentan los cambios expresos y los pendientes. En particular, minimizar mantiene la publicación activa aunque el panel del ZIP contenga el texto «Pausada: Ícaro en segundo plano». El máximo de 50 personas de la preview no es un límite del producto; el [protocolo del servicio](../../../../services/invitaciones/README.md#rutas-y-dtos) define los valores admitidos.

Se compararon los tokens y las reglas CSS de las marcas `+`, `~`, `−`, `!` y `=`: están conservados entre ambos paquetes. El registro del 07-11-2026 documentó quince vistas iced con fixtures en Tinta/Piedra y ventana pequeña, anteriores a la conexión de los controladores. El [roadmap actual](../../../roadmap.md) describe la integración experimental posterior; estas previews no son evidencia de funcionamiento real. La regla posterior del usuario exige centrado, padding simétrico y medidas uniformes en controles/celdas equivalentes. Ver [progreso](../../progreso/2026-11-07-compartir-instancias.md). Los datos, códigos y hashes de ejemplo de las previews no son credenciales operativas.
