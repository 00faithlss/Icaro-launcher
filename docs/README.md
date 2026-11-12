# Documentación de Ícaro

[Volver al proyecto](../README.md) · [Roadmap funcional](roadmap.md) · [Historial de cambios](../CHANGELOG.md)

Las guías describen el código de `v0.4.x`. Para saber qué incluye una descarga, consulta sus notas de versión. El roadmap distingue implementación de validación real; las pruebas y planes antiguos conservan el contexto de su fecha.

## Usar Ícaro

| Necesitas… | Consulta |
| --- | --- |
| Instalar y empezar | [README principal](../README.md#empezar) |
| Entrar con Microsoft | [Inicio de sesión y catálogo](login-y-catalogo.md) |
| Crear un perfil local | [Cuentas sin conexión y skins locales](cuentas-sin-conexion.md) |
| Cambiar skin o capa | [Cuenta y estudio de skin](cuenta.md) |
| Instalar Minecraft y elegir Java | [Vanilla, Fabric, Quilt y Java](vanilla-y-java.md), [Forge, NeoForge y Fabric API](loaders-forge-neoforge.md) |
| Importar un pack | [MRPACK y ZIP locales](importar-modpacks.md) |
| Gestionar instancias y archivos | [Instancias locales](instancias-locales.md), [organización de datos](organizacion-datos.md) |
| Compartir ajustes entre partidas | [Preferencias de Minecraft](opciones-minecraft.md) |
| Conectar a un servidor o una red | [Servidores y ZeroTier](servidores.md) |
| Compartir una instancia por invitación | [Compartir instancias — experimental](compartir-instancias.md) |
| Sincronizar con un servidor dedicado | [Servicio del anfitrión HTTP](sincronizacion-servidores.md) |
| Revisar animaciones o detección en Discord | [Movimiento](movimiento.md), [Discord](discord.md) |
| Investigar un fallo | [Registros de diagnóstico](diagnostico.md) |

## Desarrollar y mantener

| Documento | Contenido |
| --- | --- |
| [Desarrollo](desarrollo.md) | Requisitos, compilación, pruebas aisladas y capturas del diseño |
| [Arquitectura e integraciones](arquitectura.md) | Capas, módulos, credenciales e integridad de archivos |
| [Roadmap funcional](roadmap.md) | Capacidades, prioridades, pendientes y criterios de cierre |
| [Roadmap del System Design](roadmap-system-design.md) | Componentes, estados visuales y decisiones respecto de las referencias |
| [Peticiones a Modrinth](modrinth-peticiones.md) | Caché, lotes, concurrencia y presupuesto del proveedor |
| [Actualizaciones del launcher](actualizaciones-launcher.md) | Contrato de manifiestos, canales, copias y aplicación instalada/portable |
| [Instalador de Windows](instalador-windows.md) | Empaquetado, conservación de datos y comprobaciones de distribución |
| [Créditos](creditos.md) | Bibliotecas, tipografías, iconos, imágenes y atribuciones |
| [Reglas del repositorio](../AGENTS.md) | Ramas, validación, preservación de datos y alcance de publicación |

## Documentación junto al código

- [Servicio de invitaciones](../services/invitaciones/README.md): desarrollo local, firma, rutas HTTP y mantenimiento del Worker/D1.
- [Fixtures de loaders](../crates/core/tests/fixtures/loaders/README.md): origen y hashes de perfiles oficiales usados en pruebas.
- [Grabados de Compartir](../crates/ui/src/compartir/recursos/README.md): procedencia y hashes de los recursos originales.
- [Paquetes de diseño de Compartir](superpowers/referencias/compartir-instancia/README.md): referencias conservadas y cómo consultarlas.

## Historial y evidencias

- [Notas de versiones](releases/README.md) y [changelog](../CHANGELOG.md): cambios de cada entrega; los JSON de releases también alimentan las novedades del launcher.
- [Investigaciones](investigaciones/README.md): mediciones, fuentes y límites de las conclusiones.
- [Planes, decisiones y seguimiento](superpowers/README.md): diseños acordados, implementación y evidencias históricas.

## Mantener la documentación

El README principal presenta el proyecto y cómo empezar. Este índice enlaza todas las guías activas. El roadmap funcional concentra el estado; el visual registra componentes y diferencias de diseño. Cada guía explica su recorrido sin duplicar un inventario general de pendientes.

Antes de eliminar un documento, trasladar su contenido único y corregir enlaces. Conservar licencias, referencias originales, notas de versión, investigaciones y evidencias útiles. Las guías retiradas `acerca-de.md`, `pendientes-implementacion.md`, `integracion-real.md` y `revision-system-design.md` se consolidaron en el README, Desarrollo, Arquitectura y los dos roadmaps; su historial permanece en Git.
