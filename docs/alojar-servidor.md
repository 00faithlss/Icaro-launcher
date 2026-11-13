# Alojar un servidor con amigos — experimental

**Estado:** implementado en `v0.4.x` sobre Compartir instancias (experimental). Probado con pruebas
automáticas y un túnel local entre dos endpoints; falta una partida real entre dos equipos por
Internet.

## Qué hace

Desde **Servidores › Con amigos**, el detalle de una instancia que compartes tiene la sección
**Servidor**. **Abrir servidor** ejecuta un servidor de Minecraft en este equipo con los mods de esa
instancia. Las personas que ya recibieron la instancia lo ven abierto en su detalle y entran con
**Entrar al servidor**. Funciona como Essential: no hace falta abrir puertos, configurar el router
ni estar en la misma red.

- Solo **Vanilla** y **Fabric** por ahora (`alojamiento::loader_alojable`). Forge, NeoForge y Quilt
  muestran el motivo.
- La primera vez pide aceptar el [EULA de Minecraft](https://aka.ms/MinecraftEULA) con una
  confirmación en el mismo lugar; se guarda en `eula.txt`.
- Un servidor a la vez. **Cerrar servidor** escribe `stop` para que Minecraft guarde el mundo y solo
  fuerza el cierre si no termina en 60 s. Cerrar Ícaro con el servidor abierto hace lo mismo y
  después cierra la ventana.
- **Jugar en mi servidor** abre la instancia con `127.0.0.1:<puerto>` como servidor de inicio.

## Carpeta del servidor

`<datos>/alojamiento/<instancia>/` (botón **Abrir carpeta del servidor** en los ajustes). Se conserva
entre sesiones: mundo, `server.properties`, listas de operadores y blanca, registros y mods puestos a
mano no se tocan.

Al abrir, `alojamiento::preparar`:

1. Usa Minecraft y Java de la instancia (instala lo que falte con la misma preferencia de Java).
2. Descarga `server.jar` de Mojang, verificado con el SHA-1 del `version.json` ya verificado de la
   instancia (`minecraft::servidor::jar_vanilla`), en la caché `meta/minecraft/servers/<sha1>/`.
3. En Fabric, descarga el lanzador oficial de servidor de `meta.fabricmc.net`
   (`jar_fabric`, caché `meta/loaders/fabric-servidor/`) y escribe
   `fabric-server-launcher.properties` con `serverJar=server.jar` para que use el JAR verificado.
   Fabric no publica el hash de ese lanzador: se acepta solo por HTTPS desde su dominio oficial y si
   es un ZIP.
4. Copia los JAR activos de `mods/` de la instancia, sin los que declaran
   `"environment": "client"`. Registra lo copiado en `mods/.icaro-mods.json`: al volver a abrir solo
   copia lo que cambió y retira lo que Ícaro copió antes y ya no está. Un JAR puesto a mano con el
   mismo nombre que uno de la instancia se respeta.
5. Crea `server.properties` si falta y fija siempre `server-ip=127.0.0.1`: el servidor no escucha en
   la red local, solo se entra por el túnel. El puerto es el primero libre desde 25565.

## Túnel

`compartir::transporte::servidor` añade el ALPN `icaro-servidor/1` al mismo endpoint iroh de la
publicación.

- **Anfitrión:** las conexiones pasan las mismas comprobaciones que Compartir (autorizado por el
  servicio y no revocado en `accesos.json`) y quedan registradas para que **Quitar** a una persona
  las corte. Cada flujo empieza con un byte: `0` pregunta si el servidor está abierto y `1` pide
  conectarse; si lo está, se reenvía en ambos sentidos a `127.0.0.1:<puerto>`. Hasta 8 flujos por
  invitado.
- **El puerto publicado** vive en un registro por publicación (`servidor::abrir/cerrar`) para
  sobrevivir a reconexiones; la app lo vuelve a fijar cada 2 s mientras el servidor está abierto.
- **Invitado:** `conectar_servidor` (o `PublicacionActiva::conectar_servidor` si también comparte,
  para no abrir otra identidad) usa el encuentro firmado del servicio, igual que Sincronizar. El
  túnel abre un puerto en `127.0.0.1` y el juego se inicia con esa dirección. El túnel sigue abierto
  mientras Ícaro esté abierto; entrar otra vez lo reutiliza.

En **Pausa** o tras **Dejar de compartir** el nodo se detiene y nadie puede entrar, aunque el
servidor siga abierto.

## Ajustes del servidor

Página dentro del detalle (sin diálogos) que edita `server.properties` sin perder comentarios, orden
ni claves desconocidas (`alojamiento::propiedades`, con escapes `\uXXXX` para acentos del MOTD): mensaje,
modo de juego, dificultad, jugadores máximos, distancias de visión y simulación, semilla, PvP, volar,
extremo, bloques de comandos, lista blanca, protección del spawn y **Comprobar cuentas de Minecraft**
(`online-mode`). Está activado por defecto; desactivarlo pide confirmación porque cualquiera de los
invitados podría entrar con cualquier nombre. La memoria va en `.icaro-alojamiento.json`. Los cambios se
aplican al volver a abrir el servidor.

## Pruebas

```sh
cargo test --locked -j 2 -p icaro-core --lib alojamiento
cargo test --locked -j 2 -p icaro-core --test compartir_servidor
cargo test --locked -j 2 -p icaro-ui --example launcher alojamiento -- --test-threads=1
```

`compartir_servidor` levanta dos endpoints locales y un eco TCP: comprueba abierto/cerrado, el
reenvío y que una identidad no autorizada no consulta. Escenas de diseño: `servidores-amigos-servidor`,
`-servidor-eula`, `-servidor-ajustes` y `servidores-amigos-recibida-servidor`.

## Pendiente

- Partida real entre dos equipos por Internet (directa y por relé) y medir la latencia del relé.
- Forge, NeoForge y Quilt; configuraciones de servidor (`config/`) y datapacks.
- Respaldos del mundo e importar un mundo de un jugador.
- Avisar al anfitrión cuando alguien entra, aunque no esté mirando Con amigos.
