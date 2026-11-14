# Créditos

[Documentación](README.md) · [Estado y pendientes](roadmap.md)

## Icono de la aplicación

> La caída de Ícaro, Merry-Joseph Blondel, 1819. Fotografía © Marie-Lan Nguyen / Wikimedia Commons, [CC BY 2.5](https://creativecommons.org/licenses/by/2.5/). Adaptación: recorte, conversión a grises y ajuste de contraste. [Imagen original](https://commons.wikimedia.org/wiki/File:Fall_of_Icarus_Blondel_decoration_Louvre_INV2624.jpg).

## Bibliotecas

Selección de bibliotecas utilizadas por Ícaro. Las versiones exactas y el árbol completo se consultan en [Cargo.lock](../Cargo.lock); cada proyecto conserva su licencia y sus avisos de autoría.

| Componente | Uso | Licencia declarada |
| --- | --- | --- |
| [iced](https://github.com/iced-rs/iced) | Interfaz de escritorio | [MIT][mit] |
| [winreg](https://github.com/gentoo90/winreg-rs) | Registro del enlace de retorno en Windows | [MIT][mit] |
| [image](https://github.com/image-rs/image) | Preparación y decodificación de imágenes | [MIT][mit] OR [Apache-2.0][apache] |
| [pulldown-cmark](https://github.com/pulldown-cmark/pulldown-cmark) | Lectura de descripciones Markdown del catálogo | [MIT][mit] |
| [Tokio](https://github.com/tokio-rs/tokio) | Ejecución asíncrona | [MIT][mit] |
| [reqwest](https://github.com/seanmonstar/reqwest) | Peticiones HTTP | [MIT][mit] OR [Apache-2.0][apache] |
| [hickory-resolver](https://github.com/hickory-dns/hickory-dns) | Resolución DNS SRV de servidores | [MIT][mit] OR [Apache-2.0][apache] |
| [Serde](https://github.com/serde-rs/serde) | Serialización de datos | [MIT][mit] OR [Apache-2.0][apache] |
| [serde_json](https://github.com/serde-rs/json) | Lectura y escritura de JSON | [MIT][mit] OR [Apache-2.0][apache] |
| [sha1 · RustCrypto](https://github.com/RustCrypto/hashes) | Verificación de los manifiestos de Mojang | [MIT][mit] OR [Apache-2.0][apache] |
| [thiserror](https://github.com/dtolnay/thiserror) | Errores del núcleo | [MIT][mit] OR [Apache-2.0][apache] |
| [anyhow](https://github.com/dtolnay/anyhow) | Errores de la herramienta de línea de comandos | [MIT][mit] OR [Apache-2.0][apache] |
| [embed-resource](https://github.com/nabijaczleweli/rust-embed-resource) | Recursos del ejecutable de Windows | [MIT][mit] |
| [base64](https://github.com/marshallpierce/rust-base64) | Codificación del desafío PKCE | [MIT][mit] OR [Apache-2.0][apache] |
| [getrandom](https://github.com/rust-random/getrandom) | Aleatoriedad del sistema para PKCE y estado de autorización | [MIT][mit] OR [Apache-2.0][apache] |
| [sha2 · RustCrypto](https://github.com/RustCrypto/hashes) | SHA-256 del desafío PKCE y SHA-512 de los mods descargados | [MIT][mit] OR [Apache-2.0][apache] |
| [zeroize](https://github.com/RustCrypto/utils) | Limpieza de buffers de credenciales | [Apache-2.0][apache] OR [MIT][mit] |
| [tokio-util](https://github.com/tokio-rs/tokio) | Cancelación de autenticación y consultas | [MIT][mit] |
| [webbrowser](https://github.com/amodm/webbrowser-rs) | Apertura del navegador del sistema | [MIT][mit] OR [Apache-2.0][apache] |
| [keyring](https://github.com/hwchen/keyring-rs) | Credential Manager en Windows y Secret Service en Linux | [MIT][mit] OR [Apache-2.0][apache] |
| [rsa](https://github.com/RustCrypto/RSA) | Firma de perfiles de skins locales | [MIT][mit] OR [Apache-2.0][apache] |
| [rand](https://github.com/rust-random/rand) | Generación de claves del servicio local de texturas | [MIT][mit] OR [Apache-2.0][apache] |
| [md-5 · RustCrypto](https://github.com/RustCrypto/hashes) | Identidad local de Minecraft por nombre | [MIT][mit] OR [Apache-2.0][apache] |
| [windows-sys](https://github.com/microsoft/windows-rs) | Cambio de tamaño de la ventana en Windows | [MIT][mit] OR [Apache-2.0][apache] |

`MIT OR Apache-2.0` indica una elección entre esas dos licencias. El árbol completo, incluidas las dependencias transitivas, está registrado en [Cargo.lock](../Cargo.lock); los paquetes publicados incluyen sus propios textos de licencia.

El soporte opcional de skins locales descarga **authlib-injector 1.2.8 (compilación 56)** desde su fuente oficial, sin modificaciones y con SHA-256 fijado. Su [código fuente y licencia AGPL-3.0-or-later](https://github.com/yushijinhun/authlib-injector) están disponibles en el proyecto original. Consulta [su integración y alcance](cuentas-sin-conexion.md).

## Tipografías, iconos e imágenes

| Recurso | Autor o proyecto | Licencia y fuente |
| --- | --- | --- |
| Archivo | [Omnibus-Type](https://github.com/Omnibus-Type/Archivo) | [SIL Open Font License 1.1](https://github.com/Omnibus-Type/Archivo/blob/master/OFL.txt) |
| Martian Mono | [Evil Martians](https://github.com/evilmartians/mono) | [SIL Open Font License 1.1](https://github.com/evilmartians/mono/blob/main/OFL.txt) |
| Pixelarticons | [Gerrit Halfmann](https://github.com/halfmage/pixelarticons) | [MIT](../crates/ui/assets/iconos/LICENSE-pixelarticons.txt) |
| Fotografía de *La caída de Ícaro*, utilizada en el icono | Marie-Lan Nguyen / Wikimedia Commons; obra de Merry-Joseph Blondel | [CC BY 2.5](https://creativecommons.org/licenses/by/2.5/), con recorte y adaptación a grises. [Fuente y cambios](../crates/ui/assets/icono/FUENTES-Y-LICENCIAS.md) |
| Fotografía de *La caída de Ícaro*, fondo del video del README | Marie-Lan Nguyen / Wikimedia Commons; obra de Merry-Joseph Blondel | [CC BY 2.5](https://creativecommons.org/licenses/by/2.5/), con recorte y reducción de tamaño. [Proyecto del video](video/README.md) |

Las obras y reproducciones utilizadas en los grabados conservan las condiciones de sus respectivas fuentes. La atribución del icono se recoge al comienzo de esta guía.


[mit]: https://opensource.org/license/mit
[apache]: https://www.apache.org/licenses/LICENSE-2.0
