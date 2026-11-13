# Arquitectura e integraciones

[Documentación](README.md) · [Desarrollo](desarrollo.md) · [Estado funcional](roadmap.md)

Este mapa explica responsabilidades y contratos. Las capacidades y validaciones pendientes tienen una única fuente en el roadmap; los procedimientos viven en sus guías.

## Capas del proyecto

| Capa | Responsabilidad |
| --- | --- |
| [icaro-core](../crates/core/src/lib.rs) | Autenticación, credenciales, instalaciones, Java, instancias, mods, almacenamiento, sincronización y actualización. Sin dependencia de la interfaz. |
| [icaro-ui](../crates/ui/src/lib.rs) | Componentes iced, tokens, temas, fuentes, iconos y render. Reciben datos y producen mensajes. |
| [launcher](../crates/ui/examples/launcher.rs) | Aplicación de escritorio: estado, navegación y tareas. `launcher_aux/` separa cada integración por área. |
| [icaro-cli](../crates/cli/src/main.rs) | Consulta el manifiesto de Mojang y verifica los metadatos de una versión. |
| [Invitaciones](../services/invitaciones/README.md) | Worker/D1 para encuentro y autorizaciones. Los archivos se transfieren entre launchers mediante iroh. |

Los controladores de `launcher_aux/` traducen mensajes a tareas y resultados a modelos de vista. Disco y red se ejecutan en servicios asíncronos o `spawn_blocking`, nunca en `update`/`view`. Las respuestas incluyen identidad o generación para descartar resultados que ya no corresponden a la cuenta, instancia o consulta actual.

`ICARO_DEMO=1` usa muestras separadas. Las escenas de diseño prueban los widgets con fixtures; no acreditan autenticación, instalación, despliegue ni partidas reales.

## Servicios y fuentes de verdad

| Dominio | Módulos del núcleo | Guía |
| --- | --- | --- |
| Microsoft y credenciales | `auth`, `vault`, `cuenta` | [Inicio de sesión](login-y-catalogo.md), [cuenta y skins](cuenta.md) |
| Perfiles locales | `cuentas_locales`, `skin_local` | [Cuentas sin conexión](cuentas-sin-conexion.md) |
| Instalación y ejecución | `manifest`, `minecraft`, `java`, `lanzamiento` | [Vanilla y Java](vanilla-y-java.md), [Forge/NeoForge](loaders-forge-neoforge.md) |
| Instancias y archivos | `instancias`, `versiones_instancia`, `exportar` | [Instancias](instancias-locales.md), [datos](organizacion-datos.md) |
| Catálogo y paquetes | `modrinth`, `mods` | [Peticiones](modrinth-peticiones.md), [importación](importar-modpacks.md) |
| Servidor dedicado | `servidores`, `zerotier`, `mods/sincronizacion` | [Servidores](servidores.md), [host HTTP](sincronizacion-servidores.md) |
| Compartir entre launchers | `compartir` | [Compartir instancias](compartir-instancias.md) |
| Servidor alojado para amigos | `alojamiento`, `minecraft::servidor`, `compartir::transporte::servidor` | [Alojar un servidor](alojar-servidor.md) |
| Distribución y soporte | `actualizacion`, módulos de diagnóstico | [Actualizador](actualizaciones-launcher.md), [registros](diagnostico.md) |

## Identidad y credenciales

Microsoft usa PKCE S256, validación de `state`, comprobaciones de Xbox, titularidad y perfil Minecraft. Windows vuelve por `icaro://oauth/microsoft`; Linux conserva el retorno local. La contraseña se introduce en el navegador del proveedor. La aplicación de escritorio no incorpora un secreto de cliente.

Las credenciales persistentes pertenecen al almacén protegido del sistema: Credential Manager o Secret Service. Si no está disponible, la sesión queda en memoria. `estado.json` e `instance.json` no son almacenes de tokens. Mantener separados identidad, catálogo y descargas; no enviar tokens de Minecraft a Modrinth ni a las URLs de texturas.

Conservar cancelación, renovación sin peticiones duplicadas, descarte de respuestas de sesiones anteriores y eliminación de credenciales al cerrar sesión. Evitar secretos en errores, `Debug`, URLs, argumentos registrados y reportes. El registro legible y el diagnóstico estructurado tienen distintos límites de captura: consultar [diagnóstico](diagnostico.md) antes de compartirlos.

Las cuentas sin conexión no autentican en servicios oficiales. Su UUID estable, PNG local y servidor de texturas firmado no sustituyen la autenticación Microsoft. El agente de skins se verifica por SHA-256 y su servicio se limita a loopback para la partida local.

## Integridad y conservación de archivos

Comprobar tamaño y hashes del proveedor, restringir destinos y validar rutas, enlaces y límites de archivos comprimidos. Preparar el contenido fuera del destino activo y conservar publicación atómica, diarios recuperables y copias independientes. No usar enlaces duros como respaldo de JAR mutables ni confiar solo en tamaño/mtime para verificar integridad.

La sincronización reproduce versiones exactas del anfitrión, conserva mods personales y archivos desactivados y permite recuperar una operación interrumpida. HTTP dedicado y Compartir por invitación son recorridos distintos. Antes de cambiar hashing, descargas o sincronización, leer la [investigación y banco aislado](investigaciones/README.md).

El catálogo resuelve dependencias y conflictos, vuelve a comprobar la compatibilidad de la versión elegida y omite datos que la API no publica. Un hash válido acredita integridad del contenido esperado, no ausencia de malware. CurseForge permanece opcional y deshabilitado sin acceso e integración válidos; nunca incorporar una clave privada al repositorio ni asumir que esconderla en el ejecutable la protege.

## Validación por capas

Las pruebas locales comprueban contratos, rutas peligrosas, integridad, persistencia, cancelación y recuperación. Las pruebas con proveedores comprueban peticiones e instaladores reales. Las pruebas nativas comprueban cuentas, partidas, redes, empaquetado y recuperación en Windows/Linux. Registrar cada nivel por separado y conservar sus límites en el [roadmap](roadmap.md).
