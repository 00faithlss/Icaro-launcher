# Instalador de Windows y distribución

[Documentación](README.md) · [Estado y pendientes](roadmap.md)

El instalador usa Inno Setup 6.7.3, el asistente estándar en español y los recursos existentes de Ícaro. Instala por usuario en `%LOCALAPPDATA%\Programs\Icaro Launcher`, con icono, acceso en Inicio, escritorio optativo y entrada de desinstalación. Incluye `IcaroLauncher.exe`, `icaro-sync-host.exe` y avisos de licencia.

El asistente se usa en la primera instalación. Las actualizaciones iniciadas desde Ícaro ejecutan el setup oculto y silencioso en la misma ruta; la descarga, preparación y resultado se muestran en la pantalla original **Actualización** del launcher. Después del breve reinicio vuelve a ella. Ver [actualizaciones del launcher](actualizaciones-launcher.md) para el recorrido y su verificación.

Registra `icaro://oauth/microsoft` en `HKCU\Software\Classes\icaro`, con icono y comando del ejecutable instalado. El inicio de sesión usa ese retorno de forma predeterminada en Windows, sin página local ni variables de prueba. Al desinstalar solo retira la asociación si todavía apunta al ejecutable de esa instalación; conserva una asociación que otra copia de Ícaro haya registrado después. Ver [inicio de sesión](login-y-catalogo.md).

Los datos continúan en `%APPDATA%\Icaro`, separados del programa. El script no incorpora `[UninstallDelete]`, ni elimina carpetas de datos o credenciales. Desinstalar retira archivos/accesos registrados por esa instalación y conserva archivos ajenos añadidos manualmente.

## Compilar y empaquetar

Requiere Windows, PowerShell 7, Rust/MSVC y un compilador Inno Setup disponible. Compilar desde la raíz:

```powershell
.\scripts\empaquetar_windows.ps1 -Compilador 'C:\ruta\a\Inno Setup 6\ISCC.exe'
```

El script compila launcher y servicio en release x86-64, con CRT estático y Cargo.lock fijado. Revisa arquitectura PE, dependencias importadas de Visual C++ y coherencia de la versión del ejecutable con Cargo. El recurso de versión de Windows se genera desde `CARGO_PKG_VERSION`; no requiere editar cifras duplicadas en un RC.

Genera en `target/dist/windows`: ejecutable portable `.exe`, paquete portable `.zip` con servicio/licencias, instalador `-setup.exe` y `-artefactos.json`. Este último contiene versión, revisión Git, indicación de cambios locales, plataforma, tamaños, SHA-256 e importaciones reales. Compilar release con cambios locales no convierte ese estado en una release publicada.

El empaquetado reúne GPL del proyecto, atribución de Blondel, licencias OFL de Archivo/Martian Mono, Pixelarticons y los avisos disponibles en los paquetes Cargo, con un índice de sus versiones/licencias/repositorios. Los grabados y fuentes del programa siguen siendo los entregados; no se reemplazan por recursos de instalación nuevos.

Para volver a empaquetar ejecutables ya compilados se admiten `-Binario` y `-Servicio`, manteniendo las comprobaciones. `-Prueba` marca explícitamente los nombres/metadatos de paquetes de prueba y permite validar un binario debug sin presentarlo como compilación de producción.

## Automatización

[`windows-package.yml`](../.github/workflows/windows-package.yml) se ejecuta manualmente o por pushes a `develop` y `main`, salvo cambios exclusivamente documentales según sus filtros. Otras ramas y los tags no publican. Ejecuta pruebas y valida la versión antes de publicar. Obtiene Inno Setup del proveedor y valida su firma. Las acciones GitHub están fijadas a revisiones. La ejecución manual conserva artefactos; los pushes de beta/estable publican una release y `latest.json` con hashes reales para instalada y portable, según el [contrato del actualizador](actualizaciones-launcher.md). La primera publicación remota completó pruebas, empaquetado y publicación. Las notas de la entrega se mantienen en `docs/releases`; una integración con contenido idéntico no genera otra actualización.

El instalador y binarios de Ícaro todavía no tienen firma digital configurada. La firma comprobada del compilador pertenece al proveedor de Inno Setup, no a Ícaro.

## Validación

Validación local del 13-10-2026: artefactos release generados e integridad comprobada; instalación y reinstalación en una ruta del workspace con espacios y acento, acceso de Inicio, apertura de las escenas del launcher instalado en Tinta/Piedra y ayuda del servicio. La desinstalación retiró programa, acceso y registro, conservó un archivo ajeno y dejó intactos los tres metadatos de usuario revisados. Esto no sustituye una máquina limpia ni la validación de cuenta, inicio del juego y actualización sobre versiones publicadas. La primera ejecución remota ya publicó los paquetes correctamente.

Quedan pruebas en un equipo limpio con otro usuario/rutas con acentos, actualización instalada, apertura del launcher/juego y conservación de la sesión protegida. El actualizador portable está descrito en [actualizaciones](actualizaciones-launcher.md); actualizar su ejecutable no valida por sí solo el registro y recorrido de una instalación.

El roadmap cuenta creación del instalador, automatización del empaquetado y validación completa como tareas separadas.

Optimización local del 13-10-2026: setup de 343.407.464 a 170.999.897 bytes aproximadamente, sin cambiar los píxeles de los 144 grabados. El ayudante real del actualizador esperó un proceso de prueba, verificó SHA-256 y ejecutó el setup sobre una instalación del workspace con espacios/acento, sustituyendo una compilación anterior. Registro y ejecutable nuevos comprobados; desinstalación y dos metadatos de usuario conservados. Esta prueba del instalador/ayudante no sustituye la descarga desde una release remota ni una máquina limpia.

La compilación local con regreso a la pantalla de Actualización genera un setup de 171.002.283 bytes. El ayudante real volvió a verificar y aplicar este paquete en modo silencioso en la misma ruta de prueba, registró el resultado correcto y conservó los dos metadatos y el archivo ajeno. El reintento portable y la sustitución de un resultado previo se prueban automáticamente con Windows PowerShell en `crates/core/tests/actualizacion_local.rs`; el workflow incluye esa regresión.
