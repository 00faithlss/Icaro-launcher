# Diseño: reducción integral del trabajo repetido

Fecha: 26-10-2026. Estado: diseño y worktree aprobados por el usuario; no autoriza publicación.

## Intención y decisiones del usuario

El usuario quiere optimizar ampliamente el launcher aunque no lo perciba lento. Su preocupación es que funciones o tareas repetidas consuman rendimiento innecesariamente. Eligió **optimización integral por rondas** frente a una reorganización estructural previa. El objetivo es reducir trabajo real de CPU, disco, memoria y red, conservando las funciones y el diseño.

También solicitó reducir las compilaciones de GitHub Actions, que observa entre 30 minutos y una hora. Propuesta del agente: atender CI como primera ronda de implementación, manteniendo el resto de la optimización integral. Separar la espera en cola del tiempo de trabajo y medir las compilaciones nuevas y las que reutilizan caché.

Esta propuesta concreta el diseño general de optimización del 26-10-2026 que ya existe en `v0.3.x`. No vuelve a presentar sus mejoras aplicadas como trabajo pendiente.

## Base y entorno de trabajo

- Base de implementación: `v0.3.x`, revisión `86e985b306b766f1394fefa86c2ae6884677e876`, contrastada con GitHub el 26-10-2026.
- La carpeta original está en `codex/fix-login-dia`, revisión `6b235f8`, con cinco archivos pendientes de autenticación y documentación. Esos cambios se conservan; no se trasladan incidentalmente al trabajo de rendimiento.
- El usuario aprobó implementar en un worktree separado desde la base indicada. Copia creada en `C:/Users/Administrator/.codex/worktrees/optimizacion-integral/Icaro-launcher`, rama `codex/optimizacion-integral`. Los cambios originales de login permanecen en su carpeta.
- El entorno observado es Windows, Ryzen 5 5600X y aproximadamente 16 GiB de RAM. La memoria libre observada fue aproximadamente 2,2 GiB; no se ejecutarán cargas que la agoten. Cargo y rustc no aparecen en el PATH de esta consola: localizar una instalación utilizable antes de compilar y comunicar cualquier impedimento real. No instalar herramientas ni cambiar el sistema como parte de la exploración.
- No borrar targets, cachés ni datos personales para producir mediciones. Usar fixtures y directorios temporales propios. No modificar versiones ni publicar, integrar, comitear o subir cambios sin el alcance solicitado.

## Hallazgos de lectura confirmados

Los siguientes son mecanismos presentes en el código; todavía no son mediciones de su impacto:

1. `launcher.rs::actualizar` excluye algunos mensajes transitorios, pero en muchos otros llama a `guardar_si_cambio`. Esa función copia datos de estado y serializa toda la configuración antes de compararla con `ultimo_guardado`. Evita escrituras idénticas, pero no ese trabajo previo.
2. `estado.rs::Guardado::escribir`, cuando cambia el estado, recorre la configuración de todas las instancias. `Almacen::guardar_configuracion` vuelve a leer y serializar cada documento y compara sus bytes. Ya evita escribir documentos idénticos: la oportunidad es reducir recorridos, lecturas y serializaciones, sin atribuir escrituras inexistentes.
3. Abrir una instancia construye `Explorador::desde_ruta`, enumera su árbol visible y consulta metadatos antes de utilizar la pestaña Archivos. También carga inventarios locales de mods y recursos. La enumeración del explorador vuelve a consultar metadatos y cuenta hijos al reconstruir los nodos.
4. Mientras está activa la galería o el visor, Capturas sondea cada dos segundos. El listado recorre las carpetas de capturas de todas las instancias, incluso desde la pestaña de una instancia. La mezcla por índices ya fue optimizada en `86e985b`; sigue existiendo el trabajo de enumeración, creación de registros y ordenación.
5. `pedir_miniaturas` prepara las miniaturas que faltan de toda la colección, no solo las visibles. Conserva sus handles durante la sesión sin un presupuesto propio para esa colección. Cada resultado también busca linealmente su captura. Se deben medir el volumen de trabajo y la memoria antes de elegir una estrategia.
6. Las instalaciones normales de mods y paquetes todavía tienen rutas que copian el árbol completo de destino y descargan archivos completos a memoria. El sincronizador ya tiene streaming, concurrencia acotada y transacciones por archivos afectados: reutilizar lo que proceda, sin duplicarlo ni suponer que ambas rutas son idénticas.
7. Los MRPACK del catálogo descargan archivos en un bucle y los mantienen completos en memoria para verificarlos y escribirlos. Existe caché de modpacks y existe importación local por bloques; revisar ambas antes de diseñar otro almacén o atribuir el problema a toda importación.
8. Los registros reales de Actions ya están disponibles: el último job estable tomó 29 min 16 s, con 15 min 9 s en verificación y 13 min 9 s en compilación/empaquetado. Las pruebas ejecutadas consumieron aproximadamente 50 s; el resto de ese paso es principalmente construcción de sus ejecutables. La ejecución de 0.3.1 que superó una hora esperó 35 min 36 s antes de iniciar su job. Ver `docs/superpowers/mediciones/2026-10-26-actions-linea-base.md`.
9. `main` y `develop` todavía no contienen la caché añadida a `v0.3.x`, y la consulta actual de cachés del repositorio devolvió una lista vacía. El script de empaquetado usa un target propio y CRT estático, distinto del target normal de pruebas. Los comandos compilan con `-j1`, y el grupo global de concurrencia serializa las ejecuciones completas. Estas son oportunidades a ensayar, no una estimación ya demostrada del ahorro.

## Enfoques considerados

### Elegido: optimización integral por rondas

Medir los recorridos pertinentes, reducir primero el trabajo confirmado y verificar cada cambio antes de ampliar el alcance. Mantiene el programa utilizable, permite atribuir los resultados y limita el tamaño de las regresiones. Requiere varias rondas para cubrir áreas independientes; no reduce el objetivo a una sola corrección.

### Alternativa: reorganización estructural primero

Centralizar estado, tareas, descargas y cachés antes de optimizar sus recorridos. Puede facilitar cambios posteriores, pero amplía el riesgo y retrasa la prueba de beneficios. El usuario no eligió empezar por este enfoque. Se permiten extracciones pequeñas cuando sean necesarias para una optimización concreta.

## Objetivos y límites

- Reducir operaciones repetidas bajo condiciones equivalentes y registrar antes/después. Menos líneas, más módulos o más cachés no son por sí mismos una mejora de rendimiento.
- Mantener pantallas, componentes, grabados, iconos, temas y animaciones. Cualquier ajuste visible o cambio del momento en que aparece información debe quedar explicado y comprobado.
- Preservar mundos, mods propios y desactivados, configuraciones, archivos de origen, respaldos y recuperación tras error o cancelación.
- Conservar hashes, validación de rutas, límites de extracción y aislamiento de credenciales. No eliminar una comprobación de integridad solo porque otra parezca parecida: determinar antes qué límite de confianza protege.
- Mantener separadas las políticas de red de autenticación, catálogo, CDN y anfitrión; no unificar clientes ignorando sus restricciones.
- Acotar concurrencia, memoria y espacio de cualquier caché nueva. Una caché debe invalidarse o descartarse ante cambios, errores o contenido dudoso. No usar enlaces duros como copias independientes de JAR mutables.
- No cambiar el motor de Minecraft, prometer más FPS del juego, reescribir el launcher, modernizar dependencias ni alterar los canales de publicación como parte de esta tarea.

## Rondas y orden propuesto por el agente

### 0. Línea base y atribución

Preparar escenarios reproducibles con pocos datos y con colecciones grandes sintéticas. Registrar revisión, perfil, sistema, tamaño de datos y condiciones de caché. Separar mediciones de componentes de la experiencia de la aplicación completa.

Contar primero eventos y operaciones: serializaciones de estado, documentos leídos/escritos, archivos enumerados, miniaturas solicitadas, descargas, bytes y aciertos de caché. Añadir tiempos con reloj monotónico y memoria del proceso cuando se pueda atribuir. Ninguna instrumentación guardará tokens, nombres de cuenta, direcciones privadas o rutas personales.

Para mediciones económicas, realizar suficientes repeticiones para mediana y p95, con al menos 20 muestras del mismo escenario; separar el primer uso. Para builds costosos, comenzar con tres ejecuciones en condiciones comparables y publicar la mediana y las muestras; no presentar un p95 representativo con tan pocos ensayos. Evitar compilaciones o cargas concurrentes durante comparaciones.

### 1. GitHub Actions y compilación

Primero resolver la reutilización de dependencias: configurar y verificar la caché tanto del target de pruebas como de `target/windows-package-build`, manteniendo separadas las variantes de perfil, target y flags de CRT. Una caché de pruebas no sustituye la compilación de producción. Comprobar el SHA de commit real de la acción, la toolchain utilizada, las claves, los aciertos y el costo de restauración/guardado. Evitar cachear los paquetes de distribución y mantener el número de compilación y canal correctos en los binarios.

Comparar con los registros históricos y con una ejecución fría y otra caliente de la misma revisión. El primer ensayo debe aislar la caché. Después comparar el paralelismo de Cargo permitido por CPU y memoria del runner, manteniendo `--test-threads=1` para la ejecución de pruebas que lo requiere. Registrar memoria, fallos y tiempos; no confundir `-j` de compilación con concurrencia de las pruebas.

Estudiar por qué el núcleo se vuelve a compilar entre comandos y qué parte procede de features, perfiles y flags distintos. Reutilizar artefactos compatibles y, si la evidencia lo justifica, reorganizar las invocaciones. No declarar toda recompilación como redundante.

Comparar por separado el perfil release actual con alternativas como ThinLTO y otras unidades de generación, midiendo tiempo, tamaño y comportamiento del ejecutable. No desactivar las optimizaciones de producción para obtener solamente una compilación rápida. Revisar también el trabajo de preparación de imágenes en `build.rs`, sin retirar recursos ni cambiar píxeles para acelerar el build.

Tras medir lo anterior, reducir la espera entre ramas separando, cuando proceda, la compilación de la publicación serializada. Si se permite construir en paralelo o dividir verificación y empaquetado en jobs, la publicación debe depender de todas las comprobaciones y consumir únicamente artefactos de la misma revisión, versión, canal y compilación. Renovar la comprobación de versión dentro de la exclusión de publicación y no cancelar una publicación ya iniciada. Evaluar el costo de minutos y transferencias de artefactos antes de adoptar más jobs; no es un cambio obligatorio del primer ensayo.

La ejecución manual sigue generando únicamente artefactos. No usar pushes a `main` o `develop` para experimentar, porque publican. Las mediciones nuevas en GitHub requieren que los cambios estén en una rama remota autorizada y un workflow manual sin publicación; preparar primero los archivos y las comprobaciones locales. No afirmar una mejora remota hasta completar esos ensayos.

Aceptación: existe evidencia de restauración y guardado de las dependencias de las dos rutas; se reduce la compilación repetida en una ejecución caliente; se informan por separado cola, compilación de pruebas, ejecución de pruebas, release, empaquetado y caché. Se conservan todas las pruebas, la validación PE/CRT, las licencias, los hashes y el contrato del actualizador. La hora de espera observada no se utiliza como tiempo de compilación base.

### 2. Persistencia y apertura de instancias

Reducir la reconstrucción del estado ante eventos que no cambian datos persistentes. Identificar cambios por ámbito y por identidad estable de instancia para evitar revisar todas sus configuraciones cuando cambia una opción global. Preservar la mezcla de campos desconocidos, migraciones y errores de disco existentes.

Mover las operaciones de disco que bloqueen la actualización de iced a servicios apropiados. Si se introduce guardado asíncrono o agrupado, preservar el orden de versiones y los cambios pendientes al cerrar; una respuesta antigua no puede sobrescribir una configuración nueva. No introducir un guardado diferido que pueda perder datos para obtener una cifra mejor.

Cargar el explorador y los inventarios cuando se necesiten y reutilizar resultados válidos. Conservar la actualización tras cambios externos y operaciones propias. Los resultados deben comprobar instancia y generación antes de aplicarse.

Aceptación: mensajes transitorios no reconstruyen una configuración completa; un ajuste global no provoca lecturas de todas las configuraciones de instancia; un cambio de una instancia solo persiste su ámbito; abrir otra pestaña no enumera el árbol de Archivos. Reinicio, error, cambio rápido de instancia y cierre conservan los datos.

### 3. Capturas, imágenes y vistas

Acotar el refresco al ámbito necesario, evitar trabajo si no cambió el contenido y estudiar notificaciones de disco con recuperación por sondeo cuando se justifique. No asumir que el tamaño o fecha de una carpeta detectan toda modificación de su contenido.

Priorizar miniaturas visibles y un margen de precarga, controlar su presupuesto y evitar búsquedas lineales por cada resultado. Evaluar una caché de miniaturas en disco solo si las mediciones justifican el espacio y su gestión. Mantener el original para el visor y conservar selección, orden, zoom y acciones.

Revisar reconstrucciones y copias de listas en las vistas activas y el trabajo realizado en segundo plano u oculto. No eliminar las actualizaciones necesarias para reflejar capturas F2 o cambios externos.

Aceptación: la pestaña de una instancia no enumera las capturas de las demás; la preparación inicial no decodifica obligatoriamente toda una colección; el consumo queda acotado; modificar, borrar, renombrar y crear capturas actualiza correctamente la vista y el visor.

### 4. Instalaciones, sincronización y descargas

Antes de modificar estas rutas, releer la investigación del 15-10-2026, sus datos y el banco aislado. Revisar los diarios y garantías de cada instalador, incluida la recuperación de versiones anteriores del programa.

Extender streaming y planificación por archivos afectados a las rutas que todavía cargan o copian todo, reutilizando los servicios existentes cuando sus contratos coincidan. Evitar volver a descargar el mismo contenido entre recorridos o instancias mediante una caché verificada compartida solo cuando sus límites, publicación e invalidación estén definidos. Conservar una copia independiente en el destino y la verificación correspondiente a cada publicación.

Mantener los límites y la pausa compartida de Modrinth; las descargas del CDN y anfitrión se gestionan por separado. Añadir concurrencia únicamente dentro de presupuestos y con cancelación y progreso reales.

Aceptación: una modificación pequeña no copia archivos ajenos al plan cuando la transacción pueda garantizarlo; el pico de memoria no crece con el archivo completo por transferencia en las rutas convertidas; la caché corrupta nunca publica contenido no verificado; fallos de hash, red, disco, cancelación y cierre recuperan los originales. Comparar una descarga nueva y una reutilización.

### 5. Tamaño y estructura restante

Las optimizaciones de `debug = 0`, caché de Actions y extracción de pruebas ya existen en la base de trabajo. La compilación se atiende en la primera ronda. Volver a medir tras los cambios funcionales cuando puedan afectar al tamaño o al tiempo de construcción; mantener los resultados comparables.

Extraer código común cuando elimine trabajo o haga más seguro mantener los recorridos optimizados. Documentar las refactorizaciones sin beneficio de rendimiento medido como mejoras de mantenimiento. No retirar backends gráficos ni soporte de plataformas sin comprobar su necesidad y acordar la consecuencia.

## Verificación y entrega de cada ronda

- Regresiones centradas en comportamiento y presupuestos de operaciones, evitando pruebas que reproduzcan la implementación.
- Escenarios relevantes: cero cambios, cambio de un ajuste, muchas instancias, creación/cambio de capturas, datos corruptos, cambios externos, navegación rápida y respuestas tardías.
- Para instalación/sincronización: cinco retiradas, actualización, descargas múltiples, duplicados, caché corrupta, 429, cortes, disco lleno, cancelación y recuperación. No ejecutar modos IO contra una instancia real sin corpus estable y espacio suficiente.
- Ejecutar las pruebas requeridas por el repositorio en la copia de implementación: `cargo test --locked -j1 -p icaro-core --lib --test actualizacion_local`, `cargo test --locked -j1 -p icaro-ui --lib --example launcher -- --test-threads=1` y `git diff --check`, junto a las pruebas específicas necesarias. Distinguir fallos anteriores de regresiones y no ocultarlos.
- Guardar medidas, comandos, revisión, limitaciones y resultados por ronda en `docs/superpowers/mediciones/`. Actualizar las guías o notas afectadas por el comportamiento final.
- No afirmar una mejora global a partir de un componente. Informar cualquier regresión reproducible fuera de la variación de las muestras y corregirla o justificar el intercambio antes de conservar el cambio.
- Validar visualmente los recorridos modificados en ambos temas. Las pruebas con fixtures no acreditan juego real, cuenta real, Windows instalado o Linux nativo.

## Decisiones que quedan al implementador

Elegir la instrumentación, estructuras de índices, límites iniciales y tamaño de cada cambio según los datos recogidos, conservando los contratos anteriores. Dentro de una ronda puede cambiar el orden por evidencia de costo. Si una mejora exige cambiar funciones, diseño, seguridad, compatibilidad o consumir recursos fuera de estos límites, explicar la decisión antes de ampliar el alcance.

Este documento no promete un porcentaje ni un tiempo final universal. El resultado esperado es una reducción demostrable y sostenida del trabajo innecesario, con cobertura gradual de todo el launcher.
