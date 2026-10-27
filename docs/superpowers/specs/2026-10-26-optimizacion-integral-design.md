# Diseño: optimización integral de Ícaro

## Objetivo y límites

Reducir tiempos de compilación, espacio temporal y esperas percibidas, y dividir módulos grandes según sus responsabilidades. Mantener Rust, los formatos de datos, la seguridad, los flujos de publicación y el comportamiento visible salvo que se documente y valide un cambio. No prometer un porcentaje de mejora sin medición. No registrar tokens, nombres de usuario, direcciones privadas ni rutas personales en métricas.

Base: develop 0.3.3, commit f7b96af. Mantener intactos los cambios locales anteriores. Todo cambio se verificará localmente y no se publicará sin autorización explícita.

## Enfoques considerados

1. **Medir y optimizar por etapas (recomendado):** establecer una línea base, actuar sobre los mayores costos y dividir módulos con pruebas. Permite atribuir mejoras y revertir cada cambio; requiere varias rondas de medición.
2. **Reescritura o refactorización general inmediata:** puede cambiar la arquitectura de una vez, pero mezcla riesgos y dificulta demostrar mejoras. Se descarta como primera etapa.
3. **Solo optimizar compilación:** da resultados potencialmente rápidos, pero deja fuera los tiempos de carga y la mantenibilidad solicitados. Se incorpora como primera etapa del enfoque 1.

## 1. Línea base reproducible

Registrar revisión, sistema, hardware, versión de Rust, perfil y condiciones de red. Para cada escenario registrar tres ejecuciones y mediana; separar primera ejecución de repeticiones con caché. Nunca borrar el target existente: usar directorios de medición separados y explicar su limpieza posterior.

- Cargo: compilación limpia y repetida del launcher, pruebas y release; tiempo total, tamaño máximo y final de target, tamaño de binarios y consumo de memoria cuando esté disponible. Registrar tiempos por crate con las herramientas compatibles de Cargo.
- GitHub Actions: duración por paso, tiempo total y caché fría y caliente; también fallos y consumo de almacenamiento de caché. Actualmente el flujo usa -j1 y release con LTO. No atribuirles costos sin medir. El acceso a la API desde este entorno devolvió Forbidden: usar registros facilitados por el usuario o ejecuciones autorizadas.
- Launcher Windows: inicio hasta ventana interactiva, apertura de instancia, primera y siguientes visitas a catálogo/capturas, sincronización sin cambios y con cambios, preparación y arranque del juego. Registrar p50 y casos lentos con datos locales controlados, sin depender de una cuenta o una red real para pruebas repetibles.
- Inventario: módulos, tamaño y responsabilidades; perfiles de CPU, disco, memoria y asignaciones donde la instrumentación sea viable. El número de líneas orienta la revisión, no mide rendimiento.

## 2. Compilación y espacio

Comparar, por separado, caché de dependencias de Actions, niveles de paralelismo, perfiles dev con menos información de depuración y configuraciones de release con LTO/código en unidades diferentes. Medir compilación limpia y repetida, almacenamiento, tamaño y rendimiento del ejecutable. Mantener opciones de publicación salvo evidencia suficiente. Los cambios de caché deberán respetar plataforma, toolchain, lockfile, invalidación y límites de GitHub. No ejecutar publicaciones de prueba: el workflow manual solo debe generar artefactos.

## 3. Rendimiento y cargas

Instrumentar fases relevantes con reloj monotónico y datos agregados sin credenciales; conservar los logs existentes de sincronización. Buscar esperas en el hilo de interfaz, lecturas repetidas, decodificación de imágenes, asignaciones y consultas redundantes. Optimizar primero las rutas medidas: arranque, navegación, catálogo/capturas y sincronización. No presentar indicadores ficticios: estados de carga deben reflejar trabajo real, permitir cancelar cuando corresponda y mantener la interfaz utilizable.

## Inicio y recuperación de sesión

Priorizar que la ventana y los últimos datos visibles de la cuenta aparezcan sin esperar a Microsoft, al catálogo ni a la descarga de imágenes. Medir por separado creación de ventana, lectura de datos locales, recuperación del almacén seguro y renovación en red, con red normal, lenta y desconectada. Revisar primero el flujo existente y las cuentas sin conexión antes de decidir qué datos necesitan caché adicional.

Mostrar el último perfil y avatar disponibles localmente mientras se verifica o renueva la sesión en segundo plano. Distinguir datos mostrados de credenciales válidas: una imagen o nombre guardados no prueban que se pueda autenticar. Si la credencial sigue vigente, evitar renovaciones innecesarias; si está vencida, requiere verificación o falla la red, comunicarlo claramente al realizar una acción que la necesite, con reintento y sin bloquear la navegación. Nunca prometer acceso online sin comprobación; mantener el funcionamiento de las cuentas sin conexión.

Las credenciales permanecen en el almacén protegido existente, sin tokens en caché de perfil, registros ni métricas. Invalidar o aislar el perfil guardado al cerrar sesión o cambiar de cuenta; impedir que una respuesta tardía de una cuenta anterior sobrescriba la actual. Comprobar inicio frío y repetido, caché ausente o dañada, caducidad, desconexión, cambio de cuenta y cierre de sesión. Comparar tiempo hasta ventana interactiva y hasta mostrar el perfil con la línea base; la disponibilidad instantánea depende de que existan datos locales y de la velocidad del sistema.

## 4. Estructura

Dividir primero por fronteras funcionales estables, no por un número arbitrario de líneas: launcher.rs (estado, mensajes y vistas), sincronizacion.rs (protocolo, planificación y aplicación), minecraft.rs (manifiestos, instalación y loaders), y módulos de servicios/pantallas. Identificar dependencias y pruebas antes de mover código; hacer extracciones pequeñas sin cambios de lógica y comprobar compilación y regresiones tras cada una. Evitar ciclos, duplicación y cambios públicos innecesarios.

## 5. Verificación y entrega

Para cada mejora guardar medidas anteriores y posteriores bajo condiciones comparables; conservar solo cambios con una mejora observable o una ganancia clara de mantenibilidad sin regresión significativa. Ejecutar pruebas relevantes y la suite requerida por CI; documentar fallos preexistentes sin ocultarlos. Verificar cancelación, datos locales y compatibilidad con anfitriones anteriores. Validar manualmente Windows y sincronización real cuando se disponga de ese entorno. Informar tiempo, almacenamiento, tamaño del ejecutable, pruebas y limitaciones; no afirmar mejora global basándose en un único ensayo.

## Resultado esperado

Un registro de mediciones, cambios independientes revisables, módulos más fáciles de mantener y una configuración de compilación elegida según resultados. Las metas numéricas concretas se fijarán después de la línea base para no inventar mejoras.
