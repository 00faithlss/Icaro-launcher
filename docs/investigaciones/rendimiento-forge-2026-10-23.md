# Escritura del cliente Forge 26.3

Medición local del 23-10-2026 en Windows, Ryzen 5 5600X (12 procesadores lógicos), Java Microsoft 25.0.1. Se utilizaron copias de los archivos oficiales en `target/qa-loaders` y `target/qa-forge-opt`, sin instancias, cuentas ni mods del usuario. Los [datos](datos/rendimiento-forge-2026-10-23.json) distinguen el procesador de la instalación completa.

## Diagnóstico

Forge 66.1.0 para Minecraft 26.3 usa BinaryPatcher 1.3.4. Su [implementación oficial](https://github.com/MinecraftForge/BinaryPatcher/blob/master/src/main/java/net/minecraftforge/binarypatcher/Patcher.java) conecta `ZipOutputStream` directamente a `FileOutputStream`. Una captura con `jcmd Thread.print` situó el hilo principal en `FileOutputStream.writeBytes`, desde `ZipOutputStream.writeLOC` y `Util.store`. La escritura de muchas entradas del ZIP sin un búfer intermedio era el coste que se buscó reducir; no se midió una mejora del algoritmo de parches ni de las descargas.

## Comparación aislada

Se ejecutó el procesador con el mismo cliente, parche, classpath y argumentos oficiales: `--data --unpatched --store --marker .forge_patched_minecraft`.

| Variante | Tiempo del proceso | Resultado |
| --- | ---: | --- |
| Procesador original | 114,079 s | Código 0 |
| Copia experimental del código con salida agrupada en 64 KiB | 4,657 s | Código 0 |
| JAR oficial más el adaptador de escritura de Ícaro | 4,719 s | Código 0 |

Las tres variantes produjeron un archivo de 82.957.733 bytes, SHA-256 `4338d02f85af70578bd56aef24b195cad6d4984abedd5459f95f5e814d6adc0d`. La copia experimental del código se usó solo para aislar el efecto; no forma parte del launcher. El adaptador no modifica el JAR oficial almacenado ni desactiva la comprobación de sus firmas. La comparación muestra el efecto de agrupar escrituras en este entorno, con una muestra por variante; no estima mediana ni p95.

## Integración

La adaptación se restringe a la clase oficial `Patcher` con SHA-256 `8e7427ef29897b978790f8db9c444a4d5800c75b7d4b627716b7a915db710e77`. Sustituye únicamente su referencia al flujo de salida por un flujo con un búfer de 64 KiB. El resto del procesamiento, los argumentos y los hashes oficiales siguen vigentes. Los runtimes sin compilador o sin instrumentación conservan el recorrido original. No afecta al proceso del juego.

La prueba `instaladores_oficiales_procesan_cliente_y_reutilizan_cache_verificada` completó una nueva preparación de Forge 66.1.0 con la adaptación activa, verificó su caché sin red y rechazó una biblioteca alterada. Incluyendo las comprobaciones de caché de Forge 1.20.1 y NeoForge 1.21.1/26.3, tardó 23,66 segundos. Las observaciones anteriores de esa prueba fueron 145,32 y 184,54 segundos; la última coincidió con otra compilación y no es una comparación controlada.

La regresión compara además el JAR de Forge 26.3 con el hash original anterior. Puede repetirse con:

```sh
cargo test --locked -j1 -p icaro-core --lib instaladores_oficiales_procesan_cliente_y_reutilizan_cache_verificada -- --ignored --nocapture
```

Una caché válida evita repetir el parcheado. Para medir otra preparación deben conservarse los resultados anteriores y usarse datos de prueba nuevos; no borrar instalaciones del usuario. No se midieron otras versiones de BinaryPatcher, HDD o Linux. Preparar el cliente no valida una partida real.
