# Peticiones a Modrinth

[Documentación](README.md) · [Estado y pendientes](roadmap.md)

La tienda, la cesta, la instalación de mods y paquetes y su revisión de actualizaciones comparten el catálogo del launcher. Conservan la caché de metadatos y la misma pausa del proveedor: instalar otro mod no crea un cliente sin los resultados de la primera operación.

La comprobación de mods identificados usa `/version_files/update` por lotes de hasta 512 hashes con Minecraft, loader y canal `release`. Sus resultados se reutilizan durante cinco minutos; un hash nuevo consulta únicamente lo que falta. Se descartan candidatos de otro proyecto, incompatibles o más antiguos que la versión instalada. Los registros antiguos sin hash mantienen su comprobación por proyecto.

Las fichas de mods se consultan mediante `/projects` en lotes de hasta 50 IDs. Las respuestas válidas alimentan también las consultas individuales de la ficha y la resolución de dependencias. Los miembros de proyectos se guardan en la misma caché, evitando repetir la consulta de autoría. Los resultados válidos se conservan al terminar la instalación, incluso si otra consulta falla.

Una prueba con 163 mods comprueba una petición de actualizaciones y cuatro de fichas. Repetir desde un clon compartido no añade peticiones; añadir un hash nuevo requiere una consulta que contiene solo ese hash. La autoría y otros servicios se contabilizan por separado.

Un HTTP 429 real sigue respetando `Retry-After`, el reinicio indicado por Modrinth y la espera creciente existente. Esta reducción de tráfico no garantiza que nunca se alcance el límite, que también puede depender de otros clientes de la misma conexión.

Contrato oficial: [proyectos por lotes](https://docs.modrinth.com/api/operations/getprojects/) y [actualizaciones por hashes](https://docs.modrinth.com/api/operations/getlatestversionsfromhashes/).
