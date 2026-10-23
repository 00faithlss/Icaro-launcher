# Importar modpacks locales

En **Instancias → Importar**, arrastra un archivo `.mrpack` o `.zip` a **ZonaSoltar**, haz clic en ella o pulsa **Examinar**. También puedes pegar su ruta en **Archivo del modpack** y pulsar Enter. Este control selecciona el archivo, no la carpeta de destino. La zona muestra el nombre durante el arrastre y cambia al estado «Suelta para importar». Ícaro revisa el paquete antes de crear una instancia nueva; el archivo original se conserva. Cancelar el selector mantiene la zona abierta; mientras se revisa o instala un paquete, no se acepta otro archivo.

La pantalla **Importar modpack** conserva el grabado y los iconos del diseño en ambos temas. Muestra cinco fases reales de revisión; las lecturas menores a 400 ms pasan directamente al resultado. En **Listo**, revisa tamaños, carpetas y archivos por descargar, cambia el nombre y elige un grupo existente antes de instalar. Java se comprueba durante la preparación de la instancia, por lo que el resumen no lo marca como instalado antes de verificarlo. Un problema impide confirmar y permite corregir el perfil o elegir otro archivo. Cancelar la revisión no crea una instancia.

## Formatos

- **MRPACK:** lee `modrinth.index.json`, las versiones de Minecraft y del loader (Fabric, Forge o NeoForge), los archivos opcionales y las capas `overrides` y `client-overrides`. Descarga las referencias admitidas con verificación SHA-512/SHA-1. Excluye archivos exclusivos de servidor.
- **ZIP completo:** importa las carpetas de juego incluidas, como `mods`, `resourcepacks`, `shaderpacks`, `config`, `defaultconfigs`, `kubejs`, `scripts` y mundos. Reconoce contenido en la raíz, en `minecraft/`, `.minecraft/` o dentro de una carpeta contenedora.
- **Prism/MultiMC:** reconoce los componentes Vanilla/Fabric/Forge/NeoForge de `mmc-pack.json` y la carpeta de juego incluida.
- **Manifiesto de CurseForge:** reconoce el perfil y la carpeta de overrides si el archivo contiene el paquete completo. Los ZIP que solo referencian archivos de CurseForge mediante IDs no están admitidos; necesitan esos archivos completos o una exportación MRPACK.

Si el ZIP no indica su perfil, escribe la versión de Minecraft, selecciona Vanilla, Fabric, Forge o NeoForge e indica la versión exacta del loader correspondiente. Pulsa **Volver a revisar** y comprueba el resumen antes de **Instalar en nueva instancia**. Quilt sigue pendiente. Consulta el [alcance de Forge y NeoForge](loaders-forge-neoforge.md), incluidos sus formatos de instalador admitidos y la validación de juego pendiente. Los modpacks conservan su conjunto de mods: no se añade Fabric API automáticamente.

## Paquetes grandes y recuperación

Los archivos locales se procesan en un trabajador, por bloques de 1 MiB. El límite es 8 GiB de archivo comprimido, 32 GiB extraídos, 8 GiB por entrada y 10.000 entradas. Se guarda una copia verificada en `cache/modpacks/locales/`; necesitas espacio para ella y para el contenido instalado. Puedes mover el archivo original después de la revisión.

La instalación aparece en **Descargas**, con progreso, pausa/cancelación y reintento. Una cancelación conserva los archivos completos y mantiene la instancia pendiente. Un reintento comprueba los archivos existentes y rechaza sobrescribir contenido modificado. No borres la copia en caché mientras la instalación esté pendiente.

Se rechazan rutas fuera de la instancia, enlaces, nombres no válidos y colisiones de archivos/carpetas o mayúsculas. La copia del ZIP se verifica mediante SHA-512; cada entrada se extrae a un temporal y se comprueba antes de publicarla. Las cuentas y metadatos de otros launchers no se importan como configuración del juego.

La prueba grande se ejecuta explícitamente:

```powershell
cargo test --locked -p icaro-core --lib zip64_mayor_de_dos_gib_se_importa_por_bloques -- --ignored
```

Esta prueba usa un ZIP64 sintético; no acredita la compatibilidad de todos los mods de un paquete real.
