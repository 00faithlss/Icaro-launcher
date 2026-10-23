# Repository Guidelines

## Project Structure & Module Organization

This Rust 2021 workspace contains:

- `crates/core/src/`: authentication, instances, Minecraft/Java installation, mod synchronization, storage, and updates. Integration tests live in `crates/core/tests/`; utilities and the dedicated sync host live in `crates/core/examples/`.
- `crates/ui/src/`: reusable iced components, themes, and rendering. The desktop application is `crates/ui/examples/launcher.rs`; its service integrations are in `launcher_aux/`. Fonts, images, and icons live in `crates/ui/assets/`.
- `crates/cli/`: fetches and verifies Minecraft metadata.

Use `docs/` for guides, `docs/releases/` for release notes, `scripts/` for maintenance, and `packaging/` for platform launchers. Generated output belongs in ignored `target/`.

## Build, Test, and Development Commands

Run from the repository root with stable Rust:

```sh
cargo build --locked --workspace
cargo run --locked -p icaro-ui --example launcher
cargo run --locked -p icaro-cli
cargo build --locked --release -p icaro-core --example sync_host
```

The CLI accesses Mojang; the final command builds the standalone host. Set `ICARO_DEMO=1` for simulated accounts and catalogs. Linux dependencies are documented in `docs/desarrollo.md`.

Match Windows CI validation:

```sh
cargo test --locked -j1 -p icaro-core --lib --test actualizacion_local
cargo test --locked -j1 -p icaro-ui --lib --example launcher -- --test-threads=1
git diff --check
```

## Coding Style & Naming Conventions

Use four-space indentation and standard Rust naming: `snake_case` functions/modules, `PascalCase` types, and `SCREAMING_SNAKE_CASE` constants. Preserve existing Spanish domain names and comments. Format changed Rust code with rustfmt; use `cargo clippy --workspace --all-targets` for lint feedback. Avoid unrelated formatting changes. Keep filesystem/network work in core services and avoid blocking iced updates. Preserve the existing design system and asset attribution.

## Testing Guidelines

Use `#[test]` and `#[tokio::test]`, generally in colocated `mod tests`. Name tests after observable behavior. Add regressions for meaningful fixes, particularly integrity, cancellation, recovery, and file preservation. No coverage threshold is configured. External-service tests are typically ignored; run specific ones with `-- --ignored`. Distinguish fixture success from actual gameplay and platform validation.

## Commit & Pull Request Guidelines

Use Conventional Commits with Spanish descriptions, such as `fix(sync): conservar mods personales`. Keep changes focused. PRs should explain the problem, resulting behavior, validation, and remaining limitations; link relevant issues and include screenshots for UI changes. Update affected guides and release notes. Keep Cargo versions aligned with version branches/tags. Commit, push, and merge only when requested.

## Security & Data Preservation

Never commit credentials, tokens, or personal instance data. Preserve user mods, worlds, and settings; retain hash verification and atomic publication when changing installers or synchronization.

## Estado y fuentes de referencia

Base de trabajo: `0.2.5`, línea de mantenimiento `v0.2.x`. Consultar `git status` y las referencias remotas antes de asumir rama, publicación o estado de CI. 0.2.3 se publicó como `v0.2.3-build.38851354006` (16-10-2026). Cada versión se publica al subir `v0.2.x`; el tag fuente `v<versión>` se crea después de que exista su release `v<versión>-build.*`, para que el CI del tag la reconozca como repetida. Revalidar con `gh release view` antes de asumir el estado. La versión visible y el actualizador deben usar `env!("CARGO_PKG_VERSION")`, heredado del workspace; nunca escribir `0.1.0` u otra versión fija en la interfaz.

- [Notas de 0.2.5](docs/releases/0.2.5.md): cuentas sin conexión persistentes y skins locales con el componente original; conservar las limitaciones de autenticación y la validación pendiente en una partida real.
- [Notas de 0.2.4](docs/releases/0.2.4.md): inventario incremental, plan sin red, consultas por lotes y presupuesto de la API, descargas en streaming y paralelas, y host incremental con ETag/304, conexiones persistentes y reanudación.
- [Notas de 0.2.3](docs/releases/0.2.3.md): revisión con una sola lectura de hashes, aplicación por archivos afectados con diario recuperable, registro `logs/sincronizacion.log` y enlace al repositorio en Acerca de.
- [Notas de 0.2.2](docs/releases/0.2.2.md): recuperación del actualizador, bloqueo de ayudantes simultáneos, respaldo de conflictos, identificación de duplicados por ID/hash, importación múltiple de JAR y corrección del resumen de modpacks.
- [Pendientes funcionales](docs/pendientes-implementacion.md), [roadmap visual](docs/roadmap-system-design.md) e [integración real](docs/integracion-real.md): fuentes de alcance; una pantalla conectada o una prueba local no acreditan todo el recorrido.
- Conservar ZonaSoltar, iconos, grabados, tipografías, temas Tinta/Piedra y estados del diseño. Comparar las referencias antes de sustituir componentes o assets.
- Cuentas sin conexión: implementación en `auth::Session::local`, `cuentas_locales.rs` y `launcher_aux/cuenta_sin_conexion.rs`; sustituye Invitado según `icaro-cuenta-sin-conexion.zip`. Consultar [su guía](docs/cuentas-sin-conexion.md). Los perfiles locales se guardan sin tokens en `accounts/locales.json`; nunca enviarlos al almacén de Microsoft ni habilitar sus servicios oficiales. Conservar UUID por nombre, confirmación al quitar, mundos y selección ante errores. La gestión de varias cuentas Microsoft sigue pendiente.
- Skins locales: `skin_local.rs`, `skin_local/servidor.rs` y `launcher_aux/skin_sin_conexion.rs` conservan el componente `icaro-skin-sin-conexion.zip`. PNG por hash y modelo por cuenta; agente oficial fijado por SHA-256 y servidor firmado de solo lectura en loopback, únicamente para una partida local con skin. No ampliar esta API para autenticar cuentas. El arranque del agente con Java y las pruebas del protocolo no acreditan la visibilidad en una partida o servidor real.

## Investigación y optimización pendiente

Leer la [investigación del 15-10-2026](docs/investigaciones/rendimiento-sincronizacion-2026-10-15.md), sus [datos](docs/investigaciones/datos/rendimiento-sincronizacion-2026-10-15.json) y el banco aislado `scripts/rendimiento_sincronizacion/` antes de modificar sincronización, hashing o descargas.

La investigación detectó hashes sin optimizar en debug, barridos completos repetidos y copia de todo `mods` incluso para retirar pocos archivos. La diferencia de SHA-512 medida entre debug/release es de un componente; no demuestra una aceleración equivalente del launcher. El estado de cada prioridad se detalla a continuación; solo los puntos marcados como aplicados están implementados:

1. **P0:** usar release para medir experiencia real. Aplicado en `feat/sync-incremental`: `sha2`/`sha1` optimizados en desarrollo, una sola lectura de hashes por revisión con detección de cambios por metadatos (si difieren, se comparan hashes completos), lectura local en `spawn_blocking` con cancelación y límite de 90 s solo para la red. También aplicado: transacción por archivos afectados con diario (`.icaro-mods-tx-*`), rollback en proceso y recuperación en `recuperar()` tras cierre abrupto, y progreso por cambio del plan. El usuario confirmó la mejora en Windows con una instancia por actualizar y otra nueva. Cada fase queda en `logs/sincronizacion.log` (activado por `activar_registro` al iniciar; sin rutas absolutas ni datos de cuenta): consultarlo antes de diagnosticar un fallo de sincronización. Pendiente de P0: validar en HDD y Linux.
2. **P1:** aplicado el inventario incremental (`sincronizacion/inventario.rs`, `minecraft/.icaro-mods-inventario.json`): reutiliza hash e identidad Fabric si los metadatos coinciden y la lectura completa tiene menos de 7 días; excluye archivos modificados hace menos de 2 s. También aplicada la reutilización del plan validado (`minecraft/.icaro-sync-verificado.json`, vigencia 24 h, `Plan::sin_red` nunca ejecuta transacciones). También aplicadas las descargas en streaming (`Fuente::descargar_a`, `descargar_propio_a`) con concurrencia acotada (`en_paralelo`: 4 Modrinth, 2 anfitrión). Pendiente: caché de contenido verificado compartida entre instancias, prevista para 0.3.
3. **P1 API:** conservar lotes/caché/pausa compartida de Modrinth. Aplicado: turno por clave (`Catalog::turno`) sin retener el mutex de la caché durante la red, y nombres/`solo_cliente` por lotes (`Fuente::nombres`, `Fuente::solo_cliente_lote`). Aplicado también el presupuesto preventivo (`modrinth/presupuesto.rs`: 4/s, ráfaga 8, pausa con `Remaining` ≤ 10 hasta `Reset`, máximo 60 s) y jitter en la espera propia tras un 429. El límite observado/documentado en la investigación fue 300 solicitudes/minuto por IP; verificar reglas y cabeceras actuales, sin tratarlo como constante universal ni aplicarlo al CDN/anfitrión.
4. **P2:** aplicada en `feat/sync-inventario`: inventario en memoria del host (`dedicado::INVENTARIOS`, `sha_vigente`), ETag/304 del índice con caché en el cliente, conexiones persistentes (15 s, 100 solicitudes; los errores cierran), cola FIFO de 8 descargas sin bloquear el índice, envío por bloques y `Range`/`If-Range` con reanudación validada en `descargar_propio_a`. Mantener la compatibilidad con hosts y clientes anteriores: ETag y rangos son opcionales.

No confiar solo en tamaño/mtime para integridad ni usar enlaces duros como copias independientes de JAR mutables. Sincronizar reproduce los archivos exactos del anfitrión; no instala automáticamente la última versión del catálogo. Conservar otros mods personales, archivos desactivados, dependencias y respaldos verificables.

Medir mediana/p95, bytes leídos/escritos, llamadas API, caché, espera por cuota y RAM. Cubrir cero cambios, cinco retiradas, actualización, descargas múltiples, duplicados, cambios externos, caché corrupta, 429, cortes, disco lleno, cancelación y recuperación tras cierre. Los objetivos de 1–2 segundos de la investigación son metas pendientes, no garantías.

```sh
cargo run --locked --offline --release --manifest-path scripts/rendimiento_sincronizacion/Cargo.toml
```

El benchmark de hash no usa la instancia. Su modo `io` requiere espacio para una copia y solo debe ejecutarse con el corpus estable; consultar primero sus instrucciones.

## Funciones y validaciones todavía pendientes

- Quilt y los formatos históricos de Forge siguen pendientes. Forge/NeoForge spec 1 ya usan instaladores oficiales, Java, preparación aislada y caché verificada; se comprobaron procesadores y caché de Forge 1.20.1 y NeoForge 1.21.1/26.3. Falta validar partida con mods, migración, cancelación/recuperación y Linux; ver docs/loaders-forge-neoforge.md. Fabric API se añade solo en la primera instalación normal de Fabric, conservando modpacks, copias locales y la elección de retirarlo/desactivarlo.
- Importar ZIP de CurseForge que solo referencian archivos por ID requiere una integración válida; completar exportación MRPACK, descubrimiento de otros launchers y revisión de archivos sin origen verificado. Importación local MRPACK/ZIP completo Vanilla/Fabric/Forge/NeoForge e importación de JAR propios ya existen; no duplicarlas.
- Completar opciones General/Java, mundos y respaldos, orden de resourcepacks, otros motores de shaders, recuperación/papeleras, historial persistente de descargas, cola unificada y tiempo jugado.
- Completar instalación/permisos, salida y miembros de ZeroTier, traslado de raíz de datos y bandeja; validar cuenta/skins con cuenta real y funciones de varias cuentas/capes según su guía.
- Validar recorridos nativos en Arch/Linux, sincronización entre equipos y servidor real, reconexión al juego y estados de foco/teclado/escala/ventana pequeña en ambos temas.
- Probar actualización Windows completa, instalación limpia, reinstalación y desinstalación en equipo de prueba. Las pruebas de PowerShell y del instalador simulado no sustituyen esa validación. Un cliente 0.2.0 ejecuta su actualizador antiguo: no garantizar que incorpore retroactivamente el arreglo de 0.2.2.

## Publicación y criterios de entrega

Seguir [el contrato del actualizador](docs/actualizaciones-launcher.md) y `.github/workflows/windows-package.yml`. El push de una rama `v*` puede compilar y publicar automáticamente: comprobar su alcance antes de hacerlo. El tag fuente `v<versión>` y la release descargable `v<versión>-build.<compilación>` cumplen funciones distintas; conservar la correspondencia entre manifiesto, build, hashes y assets. No renombrar una release rompiendo ese contrato.

Mantener `Cargo.toml`, los paquetes locales de `Cargo.lock` y `docs/releases/<versión>.md/.json` alineados. No modificar versiones de dependencias o del benchmark aislado como parte de un bump del launcher. Verificar remoto, CI, release más reciente y artefactos independientemente; actualizar `main` por sí solo no distribuye el programa. Commit, push, tag, release y merge requieren el alcance solicitado por el usuario; no inferir autorización para integrar `develop`/`main` de un push a mantenimiento.
