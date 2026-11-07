# Plan de implementación de Compartir instancias

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** publicar revisiones seleccionadas de una instancia y sincronizarlas desde otro Ícaro mediante invitaciones, creando siempre una instancia nueva al aceptar y conservando cambios personales.

**Architecture:** un módulo `compartir` añade manifiestos versionados, revisiones inmutables, autorizaciones persistentes y comparación de tres vías. Workers/D1 resuelve códigos; iroh autentica a los equipos y transporta contenido propio. El protocolo HTTP actual se mantiene compatible; el nuevo recorrido reutiliza catálogo, streaming, instalación y recuperación sin aplicar dos transacciones independientes a una misma revisión.

**Tech Stack:** Rust 2021 estable 1.99.1, Tokio, serde, SHA-512, keyring existente, iroh con versión fijada tras comprobar compatibilidad; iced 0.13. Worker JavaScript ESM con SQL D1, Wrangler y pruebas locales, dependencias fijadas en su lockfile.

**Spec:** [acuerdos y antecedentes](../specs/2026-11-07-sincronizacion-entre-launchers-design.md), [referencias originales](../referencias/compartir-instancia/README.md).

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

- [ ] Escribir pruebas `rechaza_rutas_fuera_de_raices`, `rechaza_colisiones_windows`, `rechaza_enlaces_y_archivos_internos`, `limita_lectura_y_numero_de_entradas`, `un_pack_integrado_no_se_duplica` y `cancelar_inventario_no_publica`. Afirmar ausencia de lecturas fuera del sandbox y cero archivos publicados.
- [ ] Ejecutar filtro `cargo test --locked -j 2 -p icaro-core compartir::` y observar fallo por el contrato aún ausente.
- [ ] Implementar validación y lectura por bloques; reutilizar identidad y detección de cambios del inventario existente sin depender solo de mtime/tamaño. No alterar el protocolo HTTP.
- [ ] Ejecutar el mismo filtro y comprobar que pasa; añadir serialización/deserialización con límites antes de asignar memoria grande.

## Tarea 2: borrador privado y revisiones inmutables

**Archivos:** crear `compartir/publicacion.rs`, `compartir/revisiones.rs`; ampliar pruebas del módulo. Reutilizar catálogo en `crates/core/src/modrinth.rs` y parsing ya existente en `mods`.

**Interfaces:** `Borrador::comparar(&Inventario, Option<&Manifiesto>, &Seleccion) -> Result<Borrador, Error>`; `Borrador::elegir(&mut self, &RutaCompartida, Eleccion) -> Result<(), Error>`; `preparar_revision(&ContextoPublicacion, &Borrador, &CancellationToken) -> Result<RevisionPreparada, Error>`; `publicar_revision(RevisionPreparada) -> Result<Revision, Error>`.

- [ ] Probar `nuevo_es_privado_hasta_elegir`, `privado_persiste_entre_revisiones`, `posponer_actualizacion_conserva_bytes_publicados`, `posponer_retirada_conserva_archivo`, `cambio_de_orden_crea_revision` y `fallo_preparando_mantiene_revision_vigente`.
- [ ] Ejecutar filtro de tarea 1 y verificar fallo significativo.
- [ ] Implementar selección persistente, borrador y autorización por hash; snapshot independiente de archivos propios, índice coherente y puntero durable a revisión vigente. Firmar solo después de verificar contenido preparado.
- [ ] Probar modificación durante copia, falta de espacio, cancelación y limpieza; no enlazar archivos mutables ni omitir problemas silenciosamente.

## Tarea 3: plan de tres vías y transacción de toda la revisión

**Archivos:** crear `compartir/plan.rs`, `compartir/aplicacion.rs`, `compartir/packs.rs`; extraer primitivas reutilizables a `crates/core/src/mods/transaccion.rs` desde `mods/sincronizacion.rs`; adaptar `mods.rs` para recuperación compartida sin cambiar su contrato público.

**Interfaces:** `planificar(&Manifiesto, &BaseAplicada, &Inventario) -> Result<PlanCompartido, Error>`; `PlanCompartido::resolver(&mut self, &RutaCompartida, Resolucion) -> Result<(), Error>`; `aplicar(&ContextoDestino, PlanPreparado, &CancellationToken) -> Result<BaseAplicada, Error>`; `recuperar(&Path) -> Result<(), Error>`. La base guarda tanto la revisión remota como las resoluciones locales, sin perder la distinción al conservar una copia.

- [ ] Probar la matriz: local intacto/remoto cambiado actualiza; local cambiado/remoto intacto conserva; ambos cambiados exige elección; remoto retirado/local modificado exige elección; mismo hash final no genera conflicto artificial.
- [ ] Probar `solo_cambia_resourcepacks_en_options`, `packs_personales_conservan_orden`, `cambio_externo_aborta_sin_pisar`, `recupera_cierre_entre_mod_y_config` y `retirar_cinco_no_copia_mods_ajenos`.
- [ ] Ejecutar y observar fallos; implementar staging verificado y un único diario para mods, extras, packs, options y metadatos. No ejecutar primero sincronización de mods y después una transacción independiente de configuraciones.
- [ ] Ejecutar regresiones HTTP existentes y nuevas. Probar fallos inyectados de E/S, cancelación antes de aplicación y recuperación idempotente; publicación final de la base solo tras confirmar todos los cambios.

## Tarea 4: identidad, autorizaciones y transporte iroh

**Archivos:** crear `compartir/identidad.rs`, `compartir/accesos.rs`, `compartir/protocolo.rs`, `compartir/transporte.rs`; modificar `crates/core/Cargo.toml`, `Cargo.lock` y puntos necesarios de descarga de `mods/sincronizacion.rs`.

**Interfaces:** `Identidad::cargar_o_crear(&Path) -> Result<Identidad, Error>`; `Transporte` expone consulta de revisión y descarga autorizada por hash/desplazamiento; `NodoCompartido::iniciar(Identidad, ConfigConexion) -> Result<NodoCompartido, Error>`; `NodoCompartido::revocar(IdentidadPublica)`; `NodoCompartido::cerrar().await`. Mantener funciones HTTP actuales como adaptador compatible.

- [ ] Comprobar versión, features y MSRV oficiales de iroh; actualizar lock mediante Cargo sin subir otras dependencias deliberadamente. Verificar compilación Windows antes de extender el uso.
- [ ] Probar identidad persistente aislada con almacén de claves inyectable, error del keyring sin fallback a texto plano, firma inválida, publicación distinta, identidad con mismo nombre y revocación tras reinicio.
- [ ] Implementar protocolo limitado de consulta/reclamación/lectura; QUIC autentica la clave completa del par. Conservar streaming, rangos lógicos, cancelación, límites y metadatos sin esperar cola de JAR.
- [ ] Probar dos endpoints locales con relés desactivados: archivo autorizado, hash incorrecto, offset fuera de tamaño, corte/reanudación, revocación durante descarga y shutdown que no deje listeners.

## Tarea 5: servicio gratuito de invitaciones y cliente

**Archivos:** crear `services/invitaciones/package.json`, lock, `wrangler.jsonc`, `src/index.js`, `migrations/0001_invitaciones.sql`, `test/invitaciones.test.js`, README; crear `compartir/invitaciones.rs`. Herramientas y dependencias instaladas solo en el servicio; ignorar allí node_modules y .wrangler.

**Interfaces HTTP v1:** alta de invitación autenticada por prueba de posesión de clave del anfitrión, resolución de código, reclamación idempotente por identidad del invitado, actualización y revocación autenticadas. DTOs compartidos documentados en el README del servicio. `ClienteInvitaciones::resolver(&Codigo)`, `reclamar(&Codigo, &Identidad)`, `actualizar(...)` y `revocar(...)` son asíncronos, sin códigos en logs ni URL de consulta.

- [ ] Probar 24/48/168 h con reloj controlado, normalización de 12 caracteres, cupo 1 y 10, colisión de código, firma inválida y actualización por un equipo distinto.
- [ ] Probar dos reclamaciones simultáneas por último cupo, reintento idempotente tras perder respuesta, invitado conocido con cupo completo y revocación frente a solicitud en curso.
- [ ] Implementar SQL parametrizado, reclamación atómica y acceso con capacidad limitada; el anfitrión verifica la autorización recibida y la persiste antes de servir archivos. Registrar solo metadatos mínimos, caducidad y hashes; D1 no recibe claves privadas ni archivos.
- [ ] Proteger alta y resolución con límites por origen/equipo y controles de repetición; medir consultas por flujo, sin un sondeo continuo por invitado que agote la cuota gratuita.
- [ ] Ejecutar suite contra D1 local mediante Wrangler/Miniflare, además de pruebas Rust contra servicio HTTP local. No sustituir la prueba de atomicidad por un mock SQL.
- [ ] Preparar instrucciones exactas de creación/migración/despliegue, configuración de URL y secretos. Si falta URL, mostrar función sin configurar; no inventar endpoint de producción ni habilitar planes de pago.

## Tarea 6: creación, vínculo y coordinación del recorrido

**Archivos:** crear `compartir/servicio.rs`, `compartir/vinculo.rs`, `crates/core/examples/compartir_instancia.rs`; integrar `instancias.rs`, `minecraft.rs` y `lanzamiento.rs` solo donde haga falta una exclusión real de instalación/juego.

**Interfaces:** `ServicioCompartir` coordina publicación, invitaciones, accesos, revisión y aplicación; emite `EventoCompartir` con fases y cantidades reales. `aceptar(&SolicitudAceptacion) -> Result<DestinoVinculado, Error>` siempre crea un ID local nuevo. El vínculo persiste `Origen`, perfil exacto y base aplicada fuera de los archivos compartidos.

- [ ] Probar dos aceptaciones que no colisionen por nombre, renombrado local que conserve vínculo, identidad distinta que no herede origen, fallo de loader sin estado «listo» y reintento sobre la creación incompleta propia.
- [ ] Probar Minecraft ocupado y carrera entre lanzar y aplicar: mismo mecanismo de exclusión, no solo una comprobación visual previa.
- [ ] Implementar creación/instalación exacta reutilizando servicios; no añadir Fabric API ni dependencias extra contra el manifiesto sin revisión. La utilidad solo usa directorios de prueba explícitos y no busca datos reales por defecto.
- [ ] Probar recorrido completo por consola y servicio local: publicar, aceptar, planificar, confirmar, aplicar, modificar, publicar revisión y actualizar preservando un archivo personal.

## Tarea 7: componentes originales en iced

**Archivos:** crear `crates/ui/src/compartir.rs` y submódulos de publicación, invitaciones, aceptación, revisión, progreso y cierre; exportar en `crates/ui/src/lib.rs`; adaptar `servidores.rs`. Añadir escenas en `launcher_aux/pruebas_diseno.rs`.

**Interfaces:** modelos de vista y eventos tipados sin acceso a disco/red. Cada función de vista recibe `Paleta`, modelo y acciones; devuelve `Element`. Componer primitivas existentes y reproducir los 14 componentes base más RevisionPublicacion según sus README/preview.

- [ ] Extraer referencias a target y abrir previews de ambos temas; comparar primero los estados que se van a traducir, no reconstruir de memoria.
- [ ] Implementar lista de selección/posponer/privado, marcas originales, seis contadores, autorización por hash, huella/sello, caducidad/cupo, destino nuevo, progreso, conexión, revocación y errores integrados.
- [ ] Corregir únicamente la contradicción de segundo plano y los acuerdos expresos; conservar medidas, fuentes, grabados e iconos. No mostrar UUID/correo de cuentas en estos componentes.
- [ ] Probar transiciones relevantes con datos aislados y producir capturas reales de escenas: publicación con privados, retirada pospuesta, conflicto, cupo completo, error, packs y cierre; Tinta/Piedra y ventana pequeña.

## Tarea 8: integración del launcher y segundo plano

**Archivos:** crear `crates/ui/examples/launcher_aux/compartir_real.rs` y submódulos de estado/tareas/vistas; modificar `launcher.rs`, `launcher_aux/servicios.rs`, `servidores_reales.rs`, `sincronizacion_real.rs`, `juego_real.rs`, `pantallas.rs` y `pruebas_diseno.rs` según sus puntos de entrada existentes.

**Interfaces:** `compartir_real::actualizar(&mut App, Accion) -> Task<Mensaje>`; `subscription(&App) -> Subscription<Mensaje>`; `al_cerrar(&mut App) -> Task<Mensaje>`. Identificadores de petición descartan respuestas viejas; cancelación coopera con tareas del núcleo.

- [ ] Probar pulsar Jugar con revisión nueva que exige confirmación, revisión intacta sin descargas, anfitrión inaccesible y solicitud cancelada cuya respuesta llega después.
- [ ] Integrar accesos en Servidores y paneles completos. Task/Subscription ejecutan servicios y disco fuera de update/view; los callbacks solo cambian estado.
- [ ] Probar minimizar que mantiene publicación, seguir hasta terminar que cierra después y rechaza nuevas descargas, cerrar ahora que conserva autorización y detener que revoca todo.
- [ ] Implementar bandeja con biblioteca y API compatibles verificadas; no presentar ventana oculta sin forma de restaurarla. Revalidar actualizador y cierre del juego para no romper sus diálogos existentes.
- [ ] Ejecutar suite UI secuencial y capturas reales antes de considerar terminada la interfaz.

## Tarea 9: validación conjunta y documentación

**Archivos:** actualizar `docs/sincronizacion-servidores.md`, `docs/servidores.md`, `docs/desarrollo.md`, `docs/pendientes-implementacion.md`, `docs/integracion-real.md` y notas de la versión vigente solo por capacidades realmente integradas. No subir versión ni publicar release.

- [ ] Ejecutar con APPDATA temporal `cargo test --locked -j 2 -p icaro-core --lib --test actualizacion_local` y `cargo test --locked -j 2 -p icaro-ui --lib --example launcher -- --test-threads=1`; informar aparte el fallo conocido de papelera si coincide con su error habitual.
- [ ] Ejecutar pruebas del servicio D1 local y end-to-end aislado; `cargo build --locked -j 2 --workspace`, formato de Rust cambiado y `git diff --check`. No aplicar formato masivo a módulos ajenos.
- [ ] Medir cero cambios, cinco retiradas, una actualización, packs, privados y descargas múltiples: mediana/p95, bytes, API, memoria y cuotas. No ejecutar benchmark io sobre instancias reales.
- [ ] Revisar recuperación tras corte/cierre, disco lleno, cancelación y cambio de revisión; ninguna fase puede presentar éxito parcial como revisión aplicada.
- [ ] Preparar resultados y limitaciones concretas. Separar pruebas automatizadas de dos equipos reales, Windows/Linux, directo/relé y partida. Estas últimas necesitan equipos/redes disponibles y no se simulan como realizadas.
- [ ] Entregar configuración y artefactos listos para revisar antes de solicitar el despliegue de Cloudflare. No pedir credenciales por el chat ni escribirlas en el repositorio.

## Autorrevisión del plan

El recorrido cubre todos los acuerdos vigentes: producto en tareas 1–6, fidelidad visual en 7, integración y cierre en 8, validación en 9. Las firmas, límites y política de orden indicados arriba son propuestas técnicas revisables. La compatibilidad de iroh y el despliegue gratuito deben verificarse con herramientas, sin confundir documentación con pruebas.

Las limitaciones externas que no pueden darse por resueltas localmente son la cuenta y despliegue Cloudflare, prueba con dos equipos/redes, recorrido Linux y partida real. No impiden preparar y probar el código localmente una vez aprobado este plan.
