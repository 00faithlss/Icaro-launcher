# Rendimiento de Compartir instancias

Se comparó el coordinador secuencial anterior con la caché compartida y los cupos
de descarga. El banco instala archivos sintéticos en destinos nuevos mediante
iroh en loopback de Windows. Mide desde la preparación hasta que se aplica la
transacción, con una conexión y un manifiesto consultado previamente. Verifica
los archivos del destino fuera de la ventana de medición.

| Configuración | Mediana | p95 | Contenido descargado por instalación | RAM pico del proceso |
| --- | ---: | ---: | ---: | ---: |
| Base secuencial | 645 ms | 855 ms | 8 MiB previstos por el manifiesto | 30,2 MiB |
| Caché fría, hasta dos descargas | 463 ms | 517 ms | 8 MiB verificados | 26,5 MiB |
| Caché caliente, tras una instalación | 254 ms | 292 ms | 0 B | 25,4 MiB |

Cada fila usa siete muestras de 32 archivos de 256 KiB, con el perfil debug y
hashes optimizados. La RAM es el pico acumulado del proceso. Los bytes verificados
cuentan contenido útil y excluyen cabeceras y retransmisiones. La pasada en frío
identificó un rechazo entre descargas consecutivas cuando el acuse de QUIC aún
ocupaba el cupo del anfitrión. Se acotó la espera de admisión y se verificó que
el cierre siga rechazando transferencias nuevas con los cupos ocupados.

```powershell
cargo run --locked -j 2 -p icaro-core --example banco_compartir -- --modo frio --muestras 7 --archivos 32 --bytes 262144
cargo run --locked -j 2 -p icaro-core --example banco_compartir -- --modo caliente --muestras 7 --archivos 32 --bytes 262144
```

El coordinador comparte un productor por hash, verifica la procedencia de
Modrinth cuando hace falta y preserva copias independientes. El escritor agrupa
la E/S y calcula el hash al recibir el contenido. El control de revisión conserva
su cupo durante las descargas. Las métricas de caché, payload, catálogo y
concurrencia quedan en el diagnóstico local. [Guía de Compartir](../compartir-instancias.md)

Queda medir en release con el catálogo y el CDN reales, dos equipos, conexión
directa y relé, instalaciones de Minecraft y discos lentos. La muestra de
loopback cubre el coordinador de archivos con datos sintéticos.
