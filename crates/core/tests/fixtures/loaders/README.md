# Perfiles oficiales de loaders

Los archivos *-version.json contienen el version.json original de estos instaladores oficiales. No incluyen cuentas ni credenciales. Las pruebas sin red construyen un JAR mínimo con el perfil y una cabecera de instalación; no simulan haber ejecutado los procesadores oficiales.

| Archivo | Instalador de origen | SHA-256 del JAR original |
| --- | --- | --- |
| forge-version.json | https://maven.minecraftforge.net/net/minecraftforge/forge/1.20.1-47.4.10/forge-1.20.1-47.4.10-installer.jar | 1912760b4cb6b803d8a826de603c9076b1da71ec2765e9a1f8c1ca78f65278e3 |
| neoforge-version.json | https://maven.neoforged.net/releases/net/neoforged/neoforge/21.1.219/neoforge-21.1.219-installer.jar | 140df0fa17fd438848051ecfa3d091081515ad12bfe34fa126405037ba01de44 |
| neoforge26-version.json | https://maven.neoforged.net/releases/net/neoforged/neoforge/26.3.0.95/neoforge-26.3.0.95-installer.jar | 59ed4c58cc5b49e3df876658196d34c23184f5f73986e352508cdbddc88aa7fb |

Las pruebas ignoradas descargan y verifican los instaladores reales antes de ejecutarlos en una carpeta temporal.
