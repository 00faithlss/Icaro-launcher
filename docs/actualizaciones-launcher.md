# Actualizaciones de Icaro Launcher

El [instalador de Windows](instalador-windows.md) usa Inno Setup por usuario. El workflow compila, prueba y publica por canales: un push a `develop` publica una **beta** y un push a `main` publica una **estable**. Otras ramas y los tags no publican, y los cambios que solo tocan `docs/` o archivos Markdown no compilan. Una ejecución manual solo entrega artefactos. La primera publicación automática pasó en GitHub; cada revisión nueva requiere completar sus comprobaciones antes de estar disponible para otros equipos.

La pantalla sigue `ActualizacionLauncher` del System Design: Saturno en el lateral, versión y tamaño reales, notas agrupadas y acciones **Actualizar ahora**, **Al cerrar Icaro Launcher** y **Más tarde**. Se abre desde **Ajustes → Acerca de → Actualización**; la comprobación por sí sola no interrumpe al usuario. Una nueva versión se anuncia con el banner informativo existente.

El canal se elige en **Ajustes → Acerca de** y se guarda en la configuración; sin elección se usa el de la compilación (`ICARO_CANAL`, `beta` en las compilaciones de `develop`). En el canal Estable, la comprobación consulta la release marcada como más reciente de `00faithlss/Icaro-launcher` en la API oficial de GitHub y rechaza borradores, prereleases y versiones anteriores. En el canal Beta consulta las 30 releases más recientes y elige la más nueva por versión y compilación entre las estables (`-build.`, sin marca de prerelease) y las betas (`-beta.`, marcadas como prerelease); un tag y una marca que no coinciden se ignoran. Cambiar de Beta a Estable no instala una versión anterior. También ofrece una compilación más nueva de la misma versión: `ICARO_COMPILACION` incorpora el identificador del workflow al programa y `latest.json`. Las compilaciones locales usan 0. Sin releases (404) no inventa una actualización.

Para publicar una actualización portable de Windows x86-64, una release necesita:

- Un tag coherente con versión y compilación: `v0.2.0-build.12345` para una estable o `v0.3.0-beta.12345` para una beta, que debe estar marcada como prerelease. Los manifiestos anteriores sin compilación (0) siguen admitiendo `v0.2.0`; una beta exige compilación.
- Un ejecutable portable `.exe` adjunto como asset de esa misma release.
- Un asset `latest.json` con el contrato siguiente. El hash debe ser el SHA-256 real del ejecutable; los valores de este ejemplo no se publican ni se consumen como una actualización.

```json
{
  "version": "0.2.0",
  "compilacion": 12345,
  "notas": [
    {"tipo": "nuevo", "texto": "Descripción de la función para quien juega."},
    {"tipo": "mejora", "texto": "Descripción de una mejora visible."},
    {"tipo": "correccion", "texto": "Descripción del problema resuelto."}
  ],
  "plataformas": {
    "windows-x86_64": {
      "archivo": "icaro-windows-x86_64.exe",
      "sha256": "REEMPLAZAR_POR_EL_SHA256_REAL_DE_64_CARACTERES"
    },
    "windows-x86_64-setup": {
      "archivo": "icaro-windows-x86_64-setup.exe",
      "sha256": "REEMPLAZAR_POR_EL_SHA256_REAL_DEL_INSTALADOR"
    }
  }
}
```

Las notas deben describir cambios para el usuario. El programa rechaza manifiestos incoherentes, rutas fuera de los assets del repositorio, hashes inválidos, archivos de más de 1 GiB y descargas cuyo tamaño o hash no coincidan. La consulta usa HTTPS; no ejecuta contenido del manifiesto ni mensajes de release.

**Actualizar ahora** muestra el porcentaje y la barra de progreso del System Design en esta misma pantalla. Descarga a una carpeta aleatoria dentro de `%LOCALAPPDATA%\Icaro\updates`, rechaza enlaces simbólicos y puntos de redirección, comprueba tamaño y SHA-256 y vuelve a comprobar el archivo antes de aplicarlo. La comprobación final se realiza fuera del hilo de interfaz; la pantalla sigue respondiendo y avisa del reinicio. Un ayudante oculto espera a que cierre el proceso de Icaro Launcher, bloquea la escritura del archivo mientras lo verifica y copia, reemplaza el ejecutable portable de forma atómica y lo vuelve a abrir en **Actualización**, con el resultado. Conserva el ejecutable anterior con un sufijo `.icaro-anterior-…` para recuperar el portable si falla el reemplazo. Las instancias y sus archivos no se modifican.

**Al cerrar Icaro Launcher** descarga y verifica en ese momento, y aplica el reemplazo al salir normalmente del launcher, sin abrirlo de nuevo. El resultado se muestra en la misma pantalla en la próxima apertura. La programación permanece en la sesión abierta; si el proceso termina de forma abrupta, no se ejecuta el reemplazo. **Más tarde** devuelve a la pantalla anterior sin descargar y cancela una programación previa al cierre. Durante la descarga y preparación las acciones quedan deshabilitadas para evitar solicitudes contradictorias. Nunca aplica una actualización mientras Minecraft esté abierto o se esté preparando su instalación o lanzamiento.

Si el ejecutable pertenece a la ubicación registrada por Inno Setup en HKCU, el actualizador exige `windows-x86_64-setup`. Tras verificarlo y esperar al cierre de Ícaro, ejecuta ese instalador por usuario en la misma ruta con `/VERYSILENT /SUPPRESSMSGBOXES /SP- /NORESTART /NOCLOSEAPPLICATIONS /NORESTARTAPPLICATIONS`, en segundo plano y sin abrir el asistente. Actualiza el launcher, el servicio y el registro de desinstalación. Una instalación nunca cae al reemplazo del portable si falta su setup. El portable conserva el reemplazo atómico descrito arriba.

La interacción queda dentro de Ícaro; sustituir los archivos necesita el breve cierre y reapertura que prevé el diseño original. El ayudante registra éxito o fallo en un JSON acotado y propio de la ruta del ejecutable, escrito de forma atómica. Al reabrir, Ícaro consume ese resultado una sola vez y solo confirma éxito si la versión **y la compilación** del programa abierto coinciden con las esperadas. Si falla, muestra el motivo en esta pantalla para reintentar; una cuenta que no pudo restaurarse no oculta ese resultado. El registro detallado del instalador permanece en `updates`.

El bloqueo de la interfaz se mantiene hasta el cierre definitivo; un mutex impide que dos ayudantes actualicen a la vez la misma copia. PowerShell y el instalador reciben rutas Win32 sin el prefijo extendido de Rust. Tras un código de salida correcto, se verifica además la versión del ejecutable instalado. Si otra ventana de la misma copia sigue abierta, se registra el fallo sin abrir otra ventana ni iniciar el instalador.

Al reabrir o reintentar, se busca el paquete completo de la actualización vigente en `updates`. Solo se reutiliza después de verificar tamaño y SHA-256 contra el manifiesto actual, fuera del hilo de interfaz. Los `.part`, archivos alterados y paquetes de otro hash no se reutilizan. Un fallo de instalación se muestra para un reintento explícito; abrir el launcher o comprobar versiones no inicia una descarga ni una instalación por sí solo.

El workflow genera hashes de los archivos finales, `latest.json` y una release propia para cada compilación. Usa las notas Markdown y JSON de `docs/releases/<versión>` para describir la entrega y los cambios dentro del launcher. Serializa las publicaciones, evita marcar una versión inferior como la más reciente y no vuelve a publicar el mismo contenido de una versión aunque se integre en otra rama. No publica si fallan pruebas, empaquetado, notas o comprobaciones de versión. La primera release remota se generó correctamente; queda pendiente la prueba de descarga/aplicación desde GitHub en otro equipo. No hay artefacto de actualización automática para Linux.

Validación local del 13-10-2026: 9 pruebas del módulo de actualización, 49 del launcher y una regresión de integración con Windows PowerShell aprobadas. El ayudante real rechazó un hash incorrecto sin sustituir el portable, aplicó el reintento y sobrescribió el resultado de fallo. También ejecutó el setup silencioso sobre una compilación distinta en una ruta de prueba con espacios y acento: ejecutable final y registro comprobados; dos metadatos de usuario y un archivo ajeno conservados tras desinstalar la prueba. Se inspeccionaron 16 capturas nativas de fixtures de descarga, preparación, éxito y fallo, a 1280×800 y 940×720, en Tinta/Piedra, reutilizando la pantalla original. Los fixtures no descargan ni instalan actualizaciones. Esta validación local no sustituye el recorrido completo desde una release remota.

Referencias: [releases de GitHub](https://docs.github.com/en/rest/releases/releases) y [parámetros de Inno Setup](https://jrsoftware.org/ishelp/topic_setupcmdline.htm).

## Flujo de publicación

El trabajo se hace en ramas `feat/*`, `fix/*`, `perf/*` o `ci/*`, que no publican, y se integra en `develop`:

1. **Beta.** Cada push a `develop` que cambia código publica `v<versión>-beta.<compilación>` como prerelease, con la versión de Cargo de `develop` y sus notas `docs/releases/<versión>.md/.json`. La versión debe ser mayor que la última estable: tras publicar una estable, sube la versión de `develop` a la próxima entrega antes de integrar más cambios, o el workflow falla con ese aviso.
2. **Estable.** Al integrar `develop` en `main` se publica `v<versión>-build.<compilación>`, marcada como la más reciente, y el workflow crea el tag fuente `v<versión>` sobre la misma revisión. `main` exige una versión mayor que la última estable: no publica otra compilación de una versión ya estable.

Si la última release del mismo canal y versión contiene exactamente el mismo árbol y sus artefactos de actualización, el workflow no vuelve a compilar. El canal se distingue por la rama: la estable de `main` se publica aunque su contenido coincida con la última beta. No renombres el tag de una release descargable: los launchers comprueban que coincida con `latest.json`. Las ramas `vX.Y.x` y los tags `v*` anteriores se conservan como historial, pero ya no publican.

La verificación pública se ejecuta una vez publicada la release, sin aplicar la actualización:

```powershell
$env:ICARO_QA_VERSION_ACTUAL = '0.2.0'
$env:ICARO_QA_VERSION_ESPERADA = '0.2.1'
cargo test --locked -p icaro-core --test actualizacion_publicada -- --ignored --nocapture
Remove-Item Env:ICARO_QA_VERSION_ACTUAL,Env:ICARO_QA_VERSION_ESPERADA
```
