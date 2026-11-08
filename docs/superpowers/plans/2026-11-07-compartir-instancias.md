# Plan de implementación de Compartir instancias

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** publicar revisiones seleccionadas de una instancia y sincronizarlas desde otro Ícaro mediante invitaciones, creando siempre una instancia nueva al aceptar y conservando cambios personales.

**Architecture:** un módulo `compartir` añade manifiestos versionados, revisiones inmutables, autorizaciones persistentes y comparación de tres vías. Workers/D1 resuelve códigos; iroh autentica a los equipos y transporta contenido propio. El protocolo HTTP actual se mantiene compatible; el nuevo recorrido reutiliza catálogo, streaming, instalación y recuperación sin aplicar dos transacciones independientes a una misma revisión.

**Tech Stack:** Rust 2021 estable 1.99.1, Tokio, serde, SHA-512, keyring existente, iroh con versión fijada tras comprobar compatibilidad; iced 0.13. Worker JavaScript ESM con SQL D1, Wrangler y pruebas locales, dependencias fijadas en su lockfile.

**Spec:** [acuerdos y antecedentes](../specs/2026-11-07-sincronizacion-entre-launchers-design.md), [referencias originales](../referencias/compartir-instancia/README.md).

## Estado de ejecución — 07-11-2026

El usuario solicitó después subir todo el bloque a `v0.4.x`: núcleo/servicio en
`d48181f`, interfaz experimental en `f4fbf08` y actualización documental adjunta.
La autorización cubre esos commits y su push; no incluye merge, versión o release.

Consultar [progreso, evidencia y orden siguiente](../progreso/2026-11-07-compartir-instancias.md).
Las casillas marcadas corresponden al alcance probado; las que agrupan validaciones
todavía incompletas se mantienen abiertas aunque exista parte del código.
Las interfaces de las tareas 6 y 8 son el objetivo pendiente de integración.

| Tarea | Estado |
| --- | --- |
| 1 | Base local implementada y probada; inventario nuevo conservador, sin optimización incremental. |
| 2 | Implementada y conectada al publicador experimental; pendientes espacio y limpieza. |
| 3 | Implementada y probada localmente; faltan disco lleno y resolución visual del orden de packs. |
| 4 | Identidad/transporte implementados y probados con endpoints locales; sin validación entre redes. |
| 5 | Servicio/cliente implementados; 17 pruebas D1 y recorrido Rust local; sin despliegue. |
| 6 | Vínculo, destino nuevo y coordinadores conectados al invitado experimental; faltan validar perfil real, exclusión de partida y eventos completos. |
| 7 | Quince vistas y capturas implementadas; acciones reales, foco/teclado y escala pendientes. |
| 8 | Publicador e invitado experimentales bajo bandera; confianza persistente, destino/revisión e instalación conectados. Prueba Worker con perfil inyectado; Jugar y cierre/bandeja pendientes. |
| 9 | Verificación local/documentación parciales; entrega, mediciones y validación externa pendientes. |

## Restricciones globales

- Trabajar en `v0.4.x`, creada desde `v0.3.x` por petición explícita del usuario. No crear otras ramas ni worktrees.
- El usuario autorizó subir este plan a `v0.4.x` y empezar el desarrollo allí. No inferir autorización para publicar commits posteriores de implementación, hacer merge, tags, despliegues o releases.
- Preservar diseño, textos e iconos de los ZIP, con las correcciones explícitas acordadas. Tinta/Piedra, iconos centrados, sin sustituciones.
- Siempre crear una instancia nueva al aceptar; las siguientes sincronizaciones usan su vínculo estable, no el código ni una IP como origen.
- Mantener HTTP privado, verificación criptográfica y protección de mundos, mods personales, desactivados y ajustes.
- Nunca realizar disco/red en update/view; lectura y hashing en trabajo bloqueante acotado y cancelable.
- Compilar con `-j 2`; UI con `--test-threads=1`; pruebas de almacenamiento con APPDATA temporal.
- Cloudflare solo en planes gratuitos. Preparar y validar localmente; desplegar requiere un paso final con cuenta del usuario y autorización específica.
- Sin promesas de conectividad universal o relé gratuito ilimitado. No usar las instancias reales en pruebas.

## Revisión de riesgos

1. Nombres Windows, rutas relativas, colisiones de mayúsculas, enlaces y uniones: rechazar antes de leer o publicar; pruebas en tarea 1 y 3.
2. Cambio externo después de aprobar, cierre abrupto y disco lleno: conservar archivos y recuperación coherente de toda la revisión; tarea 3.
3. Dos personas reclaman el último cupo y una aceptación se reintenta tras un corte: nunca exceder cupo ni consumirlo dos veces; tareas 4 y 5.
4. Cambio de nombre o identidad y reinstalación: no heredar autorización o gestión de archivos por nombre visible; tareas 4 y 6.
5. Packs personales, versiones pospuestas y cambios solo de activación: conservar contenido ajeno y reproducir el orden compartido acordado; tareas 2 y 3.

## Decisiones técnicas propuestas para esta implementación

No son nuevos requisitos atribuidos al usuario. Se revisan con este plan antes de implementar.

- Separar manifiesto nuevo del `Indice` HTTP esquema 1/2, que tiene `deny_unknown_fields` y solo admite JAR. Esquema nuevo `icaro-compartir/1`, sin ampliar silenciosamente el contrato anterior.
- SHA-512 como identidad de contenido; SHA-256 para huellas y visualización donde el paquete lo exige. Firma Ed25519 de la revisión y del registro de encuentro, con la misma identidad pública del endpoint; verificar claves completas, no solo la huella abreviada.
- Topes iniciales configurables y validados: manifiesto 4 MiB, 20.000 archivos, archivo 512 MiB y revisión 4 GiB. Mensajes de control 64 KiB y dos transferencias propias por invitado. No confundir concurrencia con cupo de personas.
- Cupo inicial 10; no fijar un máximo de producto de 50 por el ejemplo visual. Validar rango técnico `1..=u32::MAX` y acotar sesiones activas por recursos, con cola y errores claros.
- Caducidades de 24 h, 48 h y 168 h desde creación. Permitir cambiar ajustes de una invitación vigente; tras aceptaciones, no bajar el cupo por debajo de las identidades aceptadas ni acortar el plazo al pasado. Vencida o revocada exige generar otra.
- Orden de packs: conservar el orden relativo de packs personales; insertar el bloque compartido con el orden del anfitrión. Si existe colisión de identidad o selección local modificada, mostrar conflicto en vez de alterar prioridades silenciosamente.
- Conservar la revisión vigente y las revisiones referidas por borradores, sesiones o transferencias. Posponer un cambio referencia bytes de la revisión anterior; limpieza solo de contenido sin referencias y sin transferencias.
- Los accesos sobreviven al cierre normal. «Dejar de compartir» revoca todos; cerrar sin dejar de compartir preserva autorizaciones para la próxima apertura. Al cerrar esperando descargas no aceptar nuevas y evitar prolongación indefinida.
- Prohibir archivos internos de Ícaro, rutas fuera de las cuatro raíces permitidas, enlaces/reparse points y archivos evidentemente privados como `.env`, claves y credenciales. Configuraciones se revisan por archivo, sin afirmar que pueda detectarse todo secreto automáticamente.
- Scripts y JAR propios requieren autorización por contenido. El manifiesto nunca solicita ejecución de comandos. Un script solo se ejecuta si el juego/mod correspondiente lo hace al jugar.

## Tarea 1: contratos, inventario y validación

**Archivos:** crear `crates/core/src/compartir.rs`, `compartir/manifiesto.rs`, `compartir/inventario.rs`, `compartir/tests.rs`; exportar el módulo en `crates/core/src/lib.rs`.

**Interfaces:** `RutaCompartida::parse(&str) -> Result<RutaCompartida, Error>`; `Manifiesto::validar(&self, &Limites) -> Result<(), Error>`; `inventario::analizar(&Path, &CancellationToken, &Limites) -> Result<Inventario, Error>`. Datos serializables versionados: `Origen { anfitrion, publicacion }`, `Perfil { minecraft, loader, loader_version }`, artefactos con ruta/tamaño/SHA-512/procedencia y selección de packs.

- [x] Cubrir rutas/colisiones Windows, enlaces, archivos internos/prohibidos, límites, pack integrado y cancelación en `tests/compartir/{contratos,inventario}.rs`, con sandboxes y sin publicación durante el análisis.
- [x] Observar el fallo inicial por contrato ausente con `--test compartir_instancias` antes de implementar.
- [x] Implementar validación y hashing por bloques, sin confiar solo en metadatos ni alterar HTTP. Decisión aplicada: inventario nuevo completo conservador; reutilización incremental pendiente.
- [x] Pasar las pruebas de contratos y lectura/serialización acotadas en el binario de integración.

## Tarea 2: borrador privado y revisiones inmutables

**Archivos:** crear `compartir/publicacion.rs`, `compartir/revisiones.rs`; ampliar pruebas del módulo. Reutilizar catálogo en `crates/core/src/modrinth.rs` y parsing ya existente en `mods`.

**Interfaces:** `Borrador::comparar(&Inventario, Option<&Manifiesto>, &Seleccion) -> Result<Borrador, Error>`; `Borrador::elegir(&mut self, &RutaCompartida, Eleccion) -> Result<(), Error>`; `preparar_revision(&ContextoPublicacion, &Borrador, &CancellationToken) -> Result<RevisionPreparada, Error>`; `publicar_revision(RevisionPreparada) -> Result<Revision, Error>`.

- [x] Probar nuevos privados, exclusiones persistentes, actualizaciones/retiradas pospuestas, cambios de orden y conservación de la revisión ante fallo; ver `tests/compartir/{seleccion,revisiones}.rs`.
- [ ] Ejecutar filtro de tarea 1 y verificar fallo significativo.
- [x] Implementar selección persistente, borrador, autorización por hash y snapshot independiente con puntero final. La firma de red usa la revisión publicada mediante `firmas::RevisionFirmada`.
- [ ] Probar modificación durante copia, falta de espacio, cancelación y limpieza; no enlazar archivos mutables ni omitir problemas silenciosamente.

## Tarea 3: plan de tres vías y transacción de toda la revisión

**Archivos:** crear `compartir/plan.rs`, `compartir/aplicacion.rs`, `compartir/packs.rs`; extraer primitivas reutilizables a `crates/core/src/mods/transaccion.rs` desde `mods/sincronizacion.rs`; adaptar `mods.rs` para recuperación compartida sin cambiar su contrato público.

**Interfaces vigentes:** `planificar(&Manifiesto, &BaseAplicada, &Inventario) -> Result<PlanCompartido, Error>`; `PlanCompartido::resolver(&mut self, &RutaCompartida, Resolucion) -> Result<(), Error>`; `aplicar(PlanPreparado, &CancellationToken, impl Fn(EventoAplicacion)) -> Result<BaseAplicada, Error>`; `recuperar(&Path) -> Result<(), Error>`. La base guarda la revisión remota aplicada; las copias personales conservadas se distinguen por su contenido al comparar de nuevo. La preparación contiene el contexto y retiene el bloqueo.

- [x] Probar la matriz: local intacto/remoto cambiado actualiza; local cambiado/remoto intacto conserva; ambos cambiados exige elección; remoto retirado/local modificado exige elección; mismo hash final no genera conflicto artificial.
- [x] Probar edición exclusiva de resourcePacks, conservación de orden personal, cambio externo, recuperación entre movimientos y cinco retiradas sin copiar mods ajenos; ver `tests/compartir/{packs,aplicacion,recuperacion}.rs`.
- [x] Implementar staging verificado y un único diario para archivos, selección de packs y base, con regresiones RED→GREEN. Se comparte bloqueo y punto de recuperación con HTTP, sin cambiar su diario anterior.
- [ ] Ejecutar regresiones HTTP existentes y nuevas. Probar fallos inyectados de E/S, cancelación antes de aplicación y recuperación idempotente; publicación final de la base solo tras confirmar todos los cambios.

## Tarea 4: identidad, autorizaciones y transporte iroh

**Archivos:** crear `compartir/identidad.rs`, `compartir/accesos.rs`, `compartir/protocolo.rs`, `compartir/transporte.rs`; modificar `crates/core/Cargo.toml`, `Cargo.lock` y puntos necesarios de descarga de `mods/sincronizacion.rs`.

**Interfaces vigentes:** `Identidad::cargar_o_crear(&Path)`; `transporte::NodoAnfitrion::{iniciar,direccion,conectar,revocar,drenar,parar}`; `transporte::Cliente::{conectar,consultar,descargar,cerrar}`. `anfitrion::PublicacionActiva` une revisión, ACL y servicio de invitaciones. Consulta/descarga autentican la clave completa y restringen hash/desplazamiento. Se conserva el protocolo HTTP anterior.

- [x] Fijar iroh 1.4.1 con tls-ring, comprobar MSRV y compilar en Windows con Rust 1.99.1; lock actualizado mediante Cargo.
- [ ] Probar identidad persistente aislada con almacén de claves inyectable, error del keyring sin fallback a texto plano, firma inválida, publicación distinta, identidad con mismo nombre y revocación tras reinicio.
- [x] Implementar consulta/lectura acotadas con clave completa, streaming, offsets, cancelación y límites. La aceptación se realiza en el Worker y el anfitrión consulta su autorización antes de servir.
- [x] Probar dos endpoints locales sin relés: archivo autorizado, hash/offset rechazados, parcial cancelado/reanudado, integridad, revocación y cierre; ver `tests/compartir_transporte.rs`.

## Tarea 5: servicio gratuito de invitaciones y cliente

**Archivos:** crear `services/invitaciones/package.json`, lock, `wrangler.jsonc`, `src/index.js`, `migrations/0001_invitaciones.sql`, `test/invitaciones.test.js`, README; crear `compartir/invitaciones.rs`. Herramientas y dependencias instaladas solo en el servicio; ignorar allí node_modules y .wrangler.

**Interfaces HTTP v1 vigentes:** solicitudes firmadas y acotadas; DTOs en el README del servicio. `ClienteInvitaciones::{nueva,resolver,aceptar,actualizar,revocar_codigo,revocar_acceso,detener,autorizados}` son asíncronos. La identidad pertenece al cliente; `aceptar(&Codigo)` reclama idempotentemente. Código y acceso se revocan por separado. `AutoridadServicio` consulta como anfitrión antes de permitir el transporte.

- [ ] Probar 24/48/168 h con reloj controlado, normalización de 12 caracteres, cupo 1 y 10, colisión de código, firma inválida y actualización por un equipo distinto.
- [ ] Probar dos reclamaciones simultáneas por último cupo, reintento idempotente tras perder respuesta, invitado conocido con cupo completo y revocación frente a solicitud en curso.
- [ ] Implementar SQL parametrizado, reclamación atómica y acceso con capacidad limitada; el anfitrión verifica la autorización recibida y la persiste antes de servir archivos. Registrar solo metadatos mínimos, caducidad y hashes; D1 no recibe claves privadas ni archivos.
- [ ] Proteger alta y resolución con límites por origen/equipo y controles de repetición; medir consultas por flujo, sin un sondeo continuo por invitado que agote la cuota gratuita.
- [x] Ejecutar 17 pruebas con Miniflare/D1 y la interoperabilidad Rust con Worker local; la atomicidad usa SQLite/D1 real local.
- [ ] Preparar instrucciones exactas de creación/migración/despliegue, configuración de URL y secretos. Si falta URL, mostrar función sin configurar; no inventar endpoint de producción ni habilitar planes de pago.

## Tarea 6: creación, vínculo y coordinación del recorrido

**Archivos actuales y pendientes:** existen `compartir/{anfitrion,vinculos,descargas}.rs`. Falta completar la coordinación con `instancias.rs`, `minecraft.rs` y `lanzamiento.rs`, incluida la exclusión real de instalación/juego; no existe todavía el ejemplo por consola propuesto.

**Interfaces actuales:** `vinculos::{publicacion,crear_destino,leer}`, `descargas::preparar` y `anfitrion::PublicacionActiva`. El destino recibe un ID nuevo; vínculo y base aplicada son metadatos privados separados. **Objetivo pendiente:** coordinar instalación y eventos con cantidades reales, conservando preparación/revisión y aplicación como pasos explícitos.

- [ ] Probar dos aceptaciones que no colisionen por nombre, renombrado local que conserve vínculo, identidad distinta que no herede origen, fallo de loader sin estado «listo» y reintento sobre la creación incompleta propia.
- [ ] Probar Minecraft ocupado y carrera entre lanzar y aplicar: mismo mecanismo de exclusión, no solo una comprobación visual previa.
- [ ] Implementar creación/instalación exacta reutilizando servicios; no añadir Fabric API ni dependencias extra contra el manifiesto sin revisión. La utilidad solo usa directorios de prueba explícitos y no busca datos reales por defecto.
- [ ] Probar recorrido completo por consola y servicio local: publicar, aceptar, planificar, confirmar, aplicar, modificar, publicar revisión y actualizar preservando un archivo personal.

## Tarea 7: componentes originales en iced

**Archivos:** crear `crates/ui/src/compartir.rs` y submódulos de publicación, invitaciones, aceptación, revisión, progreso y cierre; exportar en `crates/ui/src/lib.rs`; adaptar `servidores.rs`. Añadir escenas en `launcher_aux/pruebas_diseno.rs`.

**Interfaces:** modelos de vista y eventos tipados sin acceso a disco/red. Cada función de vista recibe `Paleta`, modelo y acciones; devuelve `Element`. Componer primitivas existentes y reproducir los 14 componentes base más RevisionPublicacion según sus README/preview.

- [x] Conservar ambos ZIP originales y usar sus componentes/assets como referencia; comparar capturas reales en Tinta/Piedra.
- [x] Implementar las quince vistas puras: selección/publicación, marcas/contadores, consentimiento, huella/sello, invitación/cupo, destino, revisión, progreso, conexión, revocación, errores y cierre. Falta conectar las acciones al recorrido normal.
- [ ] Corregir únicamente la contradicción de segundo plano y los acuerdos expresos; conservar medidas, fuentes, grabados e iconos. No mostrar UUID/correo de cuentas en estos componentes.
- [ ] Probar transiciones relevantes con datos aislados y producir capturas reales de escenas: publicación con privados, retirada pospuesta, conflicto, cupo completo, error, packs y cierre; Tinta/Piedra y ventana pequeña.

## Tarea 8: integración del launcher y segundo plano

Avance tras la limpieza: existe `launcher_aux/compartir_real/`, protegido por
`ICARO_COMPARTIR_DESARROLLO=1`. Revisión/publicación, consentimiento, privados,
invitaciones/accesos y detención/reapertura tienen controlador inicial. Quince pruebas
cubren cancelación, errores de guardado, respuestas tardías, consentimiento, separación
de publicación/borrador, configuración inválida sin escrituras y aislamiento del modo
demo. Una prueba explícita con Worker/D1 local cubre publicación y código del panel,
pack externo, aceptación, ajustes y reapertura, usando futuros/reductores reales.
Continuación tras `01a55ed`: el invitado ya conecta código, consulta autenticada,
confianza persistente, nombre/revisión y coordinador de instalación/aplicación.
La prueba inyecta el perfil; no acredita descargar Minecraft desde esta ruta.
Falta recorrer todas las acciones desde la ventana, completar progreso y reintento
tras reinicio, Jugar y cierre/bandeja; no cerrar esta tarea todavía.

**Archivos:** crear `crates/ui/examples/launcher_aux/compartir_real.rs` y submódulos de estado/tareas/vistas; modificar `launcher.rs`, `launcher_aux/servicios.rs`, `servidores_reales.rs`, `sincronizacion_real.rs`, `juego_real.rs`, `pantallas.rs` y `pruebas_diseno.rs` según sus puntos de entrada existentes.

**Interfaces:** `compartir_real::actualizar(&mut App, Accion) -> Task<Mensaje>`; `subscription(&App) -> Subscription<Mensaje>`; `al_cerrar(&mut App) -> Task<Mensaje>`. Identificadores de petición descartan respuestas viejas; cancelación coopera con tareas del núcleo.

- [ ] Probar pulsar Jugar con revisión nueva que exige confirmación, revisión intacta sin descargas, anfitrión inaccesible y solicitud cancelada cuya respuesta llega después.
- [ ] Integrar accesos en Servidores y paneles completos. Task/Subscription ejecutan servicios y disco fuera de update/view; los callbacks solo cambian estado.
- [ ] Probar minimizar que mantiene publicación, seguir hasta terminar que cierra después y rechaza nuevas descargas, cerrar ahora que conserva autorización y detener que revoca todo.
- [ ] Implementar bandeja con biblioteca y API compatibles verificadas; no presentar ventana oculta sin forma de restaurarla. Revalidar actualizador y cierre del juego para no romper sus diálogos existentes.
- [ ] Ejecutar suite UI secuencial y capturas reales antes de considerar terminada la interfaz.

## Tarea 9: validación conjunta y documentación

**Archivos:** actualizar `docs/sincronizacion-servidores.md`, `docs/servidores.md`, `docs/desarrollo.md`, `docs/pendientes-implementacion.md`, `docs/integracion-real.md` y notas de la versión vigente solo por capacidades realmente integradas. No subir versión ni publicar release.

- [x] Ejecutar con APPDATA temporal las suites del núcleo y UI: 328 pruebas core y 144 UI pasadas, sin fallos; ignoradas y alcance detallados en el registro de progreso. El fallo conocido de papelera no apareció en esta ejecución.
- [ ] Ejecutar pruebas del servicio D1 local y end-to-end aislado; `cargo build --locked -j 2 --workspace`, formato de Rust cambiado y `git diff --check`. No aplicar formato masivo a módulos ajenos.
- [ ] Medir cero cambios, cinco retiradas, una actualización, packs, privados y descargas múltiples: mediana/p95, bytes, API, memoria y cuotas. No ejecutar benchmark io sobre instancias reales.
- [ ] Revisar recuperación tras corte/cierre, disco lleno, cancelación y cambio de revisión; ninguna fase puede presentar éxito parcial como revisión aplicada.
- [ ] Preparar resultados y limitaciones concretas. Separar pruebas automatizadas de dos equipos reales, Windows/Linux, directo/relé y partida. Estas últimas necesitan equipos/redes disponibles y no se simulan como realizadas.
- [ ] Entregar configuración y artefactos listos para revisar antes de solicitar el despliegue de Cloudflare. No pedir credenciales por el chat ni escribirlas en el repositorio.

## Autorrevisión del plan

### Continuación del núcleo y revisión visual

Estado actualizado en [la guía de implementación](../../compartir-instancias.md).
Se implementaron aplicación y recuperación unificadas, transiciones archivo/carpeta,
identidad segura, firmas, transporte iroh, Worker/D1, cliente de invitaciones,
autorización del anfitrión, vínculos estables, creación de destino con loader fijo y
coordinación de descargas. La prueba Rust con Worker/D1 e iroh locales recorre la
aceptación, descarga, aplicación de configuración/texturas y revocación.

Las quince vistas están implementadas y las escenas de diseño se capturan desde
el launcher en ambos temas y ventana pequeña. Las capturas detectaron y permitieron
corregir el pie recortado en listas largas; la revisión visual sigue en curso.
El controlador funcional de Servidores/Jugar y la bandeja todavía están pendientes:
no hay una función nueva habilitada en el recorrido normal del launcher.

El código se registra en los commits indicados al inicio, sin cambio de versión
ni despliegue. El apartado de inicio que sigue se conserva como historial, no como
estado vigente.

### Inicio de ejecución en v0.4.x — historial superado

Al iniciar esta etapa, el plan se había publicado en `d78e628` y el desarrollo posterior seguía sin commit; subir el plan no autorizó publicar automáticamente la implementación. La autorización posterior y los commits actuales se registran al inicio de este documento.

- Base local implementada: manifiesto versionado y acotado, rutas portables, inventario cancelable sin escrituras, diagnóstico de archivos privados prohibidos, selección persistente y borrador con contenido nuevo privado por defecto.
- Revisiones locales implementadas: copias independientes por contenido, autorización de JAR/packs/scripts propios, posponer cambios conservando contenido publicado, exclusión de preparaciones simultáneas y promoción final de una revisión coherente. Todavía no se firman ni se sirven por red.
- Comparación de tres vías y combinación del orden de packs implementadas como operaciones sin escritura. La aplicación unificada, recuperación y edición de `options.txt` siguen pendientes.
- Todavía faltan transporte iroh, servicio Workers/D1, gestión persistente de accesos, creación coordinada de destino e integración visual. No hay una función nueva operativa en el launcher.

Decisiones de ejecución: las regresiones se concentran en `crates/core/tests/compartir_instancias.rs`, usando `--test compartir_instancias` en lugar del filtro de pruebas unitarias propuesto. El inventario nuevo hace lectura completa conservadora; reutilizar una caché para las carpetas adicionales y medir el coste quedan pendientes. La revisión final local revalida contenido antes de publicar, con coste adicional de lectura que también necesita medirse. El almacén por contenido conserva versiones pospuestas; la política de limpieza y el control previo de espacio no están implementados.

La revisión independiente de la base detectó tamaño compacto frente a JSON indentado, colisiones archivo/carpeta y bloqueo por contenido privado prohibido. Se reprodujeron con pruebas fallidas antes de corregirse. También se añadieron regresiones por contenido alterado y cancelación entre preparación y publicación.

El recorrido cubre todos los acuerdos vigentes: producto en tareas 1–6, fidelidad visual en 7, integración y cierre en 8, validación en 9. Las firmas, límites y política de orden indicados arriba son propuestas técnicas revisables. La compatibilidad de iroh y el despliegue gratuito deben verificarse con herramientas, sin confundir documentación con pruebas.

Las limitaciones externas que no pueden darse por resueltas localmente son la cuenta y despliegue Cloudflare, prueba con dos equipos/redes, recorrido Linux y partida real. No impiden preparar y probar el código localmente una vez aprobado este plan.
