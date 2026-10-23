// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

//! Lanzamiento Vanilla mediante argumentos literales, sin intérprete de comandos.
//! Las credenciales solo se entregan al proceso de Java y se ocultan en registros.

use crate::{auth, minecraft::Instalado};
use std::{collections::HashMap, path::Path, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Command,
};
use tokio_util::sync::CancellationToken;
use zeroize::{Zeroize, Zeroizing};

const MAX_LINEA: usize = 64 * 1024;
const MAX_EVENTO_XML: usize = 256 * 1024;
const INICIO_XML: &[u8] = b"<log4j:Event";
const FIN_XML: &[u8] = b"</log4j:Event>";
const MAX_ARGUMENTOS: usize = 256;

#[derive(Clone)]
pub struct Config {
    pub memoria_min_mb: u32,
    pub memoria_max_mb: u32,
    /// Separa argumentos con espacios y permite comillas; nunca invoca un shell.
    pub argumentos_jvm: String,
    pub resolucion: Option<(u32, u32)>,
    pub fullscreen: bool,
    pub servidor: Option<String>,
    /// Archivo de preferencias compartidas; el launcher lo sitúa en su raíz de datos.
    pub opciones_compartidas: Option<std::path::PathBuf>,
    pub cuentas_locales: Option<std::path::PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            memoria_min_mb: 512,
            memoria_max_mb: 2048,
            argumentos_jvm: String::new(),
            resolucion: None,
            fullscreen: false,
            servidor: None,
            opciones_compartidas: None,
            cuentas_locales: None,
        }
    }
}

impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("memoria_min_mb", &self.memoria_min_mb)
            .field("memoria_max_mb", &self.memoria_max_mb)
            .field("argumentos_jvm", &"[PERSONALIZADOS]")
            .field("resolucion", &self.resolucion)
            .field("fullscreen", &self.fullscreen)
            .field("servidor", &self.servidor)
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origen {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nivel {
    Info,
    Advertencia,
    Error,
    Depuracion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Salida {
    pub codigo: Option<i32>,
    pub exito: bool,
    pub cancelado: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Evento {
    Inicio {
        pid: u32,
    },
    Linea {
        origen: Origen,
        nivel: Nivel,
        texto: String,
    },
    Fin(Salida),
}

/// No conserva rutas, argumentos, mensajes del sistema ni credenciales.
#[derive(Debug, Clone, Copy, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    #[error("El lanzamiento fue cancelado.")]
    Cancelado,
    #[error("La cuenta necesita renovar su sesión antes de jugar.")]
    Cuenta,
    #[error("No se pudo preparar la skin local. Revisa la conexión o vuelve a guardar/quitar la skin desde la cuenta.")]
    Skin,
    #[error("La memoria debe estar entre 256 y 131072 MiB, y la máxima debe ser mayor o igual a la mínima.")]
    Memoria,
    #[error("La resolución debe estar entre 64 y 16384 píxeles por lado.")]
    Resolucion,
    #[error("Los argumentos de Java tienen comillas inválidas, demasiados valores o caracteres no admitidos.")]
    ArgumentosJvm,
    #[error("Los argumentos adicionales no pueden cambiar la memoria, la clase principal, las bibliotecas ni la carpeta del juego.")]
    ArgumentoProtegido,
    #[error(
        "La instalación no contiene los archivos o argumentos necesarios para lanzar Minecraft."
    )]
    Instalacion,
    #[error("La dirección del servidor no es válida.")]
    Servidor,
    #[error("No se pudo preparar la carpeta del juego.")]
    Carpeta,
    #[error("No se pudo iniciar Java. Revisa el ejecutable seleccionado.")]
    Java,
    #[error("No se pudo esperar la salida del juego.")]
    Espera,
    #[error("No se pudo detener el juego.")]
    Detener,
    #[error("No se pudieron leer los registros del juego.")]
    Registros,
}

fn validar(config: &Config) -> Result<(), Error> {
    if !(256..=131072).contains(&config.memoria_min_mb)
        || !(256..=131072).contains(&config.memoria_max_mb)
        || config.memoria_min_mb > config.memoria_max_mb
    {
        return Err(Error::Memoria);
    }
    if config.resolucion.is_some_and(|(ancho, alto)| {
        !(64..=16384).contains(&ancho) || !(64..=16384).contains(&alto)
    }) {
        return Err(Error::Resolucion);
    }
    Ok(())
}

fn protegida(argumento: &str) -> bool {
    let a = argumento.to_ascii_lowercase();
    let opcion = a.split('=').next().unwrap_or("");
    matches!(
        opcion,
        "-cp"
            | "-classpath"
            | "--class-path"
            | "--module-path"
            | "-p"
            | "-jar"
            | "-m"
            | "--module"
            | "-djava.class.path"
            | "-djava.library.path"
            | "-dorg.lwjgl.librarypath"
            | "-dorg.lwjgl.system.sharedlibraryextractpath"
            | "-dnet.java.games.input.librarypath"
            | "-duser.dir"
            | "-djava.home"
    ) || a.starts_with("-xms")
        || a.starts_with("-xmx")
        || a.starts_with("-xbootclasspath")
        || a.starts_with("-xx:maxheapsize")
        || a.starts_with("-xx:initialheapsize")
        || a.starts_with("-xx:minheapsize")
        || a.starts_with("-xx:maxram")
        || a.starts_with("-xx:minram")
        || a.starts_with("-xx:initialram")
}

/// Las barras de las rutas Windows permanecen literales; solo \" y \' escapan
/// una comilla. No expande variables, sustituciones, redirecciones ni comodines.
/// Comparte la validación y las opciones JVM entre Probar y el lanzamiento real.
pub(crate) fn argumentos_prueba(config: &Config) -> Result<Vec<String>, Error> {
    validar(config)?;
    let mut args = vec![
        format!("-Xms{}M", config.memoria_min_mb),
        format!("-Xmx{}M", config.memoria_max_mb),
    ];
    args.extend(argumentos_extra(&config.argumentos_jvm)?.iter().cloned());
    Ok(args)
}

fn argumentos_extra(texto: &str) -> Result<Zeroizing<Vec<String>>, Error> {
    if texto.len() > 16 * 1024 || texto.chars().any(|c| c.is_control() && !c.is_whitespace()) {
        return Err(Error::ArgumentosJvm);
    }
    let mut argumentos = Zeroizing::new(Vec::new());
    let mut actual = Zeroizing::new(String::new());
    let mut comilla = None;
    let mut iniciado = false;
    let mut caracteres = texto.chars().peekable();
    while let Some(c) = caracteres.next() {
        if c == '\\'
            && caracteres.peek().is_some_and(|next| {
                matches!(next, '\'' | '"') && (comilla.is_none() || comilla == Some(*next))
            })
        {
            actual.push(caracteres.next().ok_or(Error::ArgumentosJvm)?);
            iniciado = true;
        } else if comilla == Some(c) {
            comilla = None;
        } else if comilla.is_none() && matches!(c, '\'' | '"') {
            comilla = Some(c);
            iniciado = true;
        } else if comilla.is_none() && c.is_whitespace() {
            if iniciado {
                argumentos.push(std::mem::take(&mut *actual));
                iniciado = false;
            }
        } else {
            actual.push(c);
            iniciado = true;
        }
    }
    if comilla.is_some() {
        return Err(Error::ArgumentosJvm);
    }
    if iniciado {
        argumentos.push(std::mem::take(&mut *actual));
    }
    if argumentos.len() > MAX_ARGUMENTOS {
        return Err(Error::ArgumentosJvm);
    }
    for argumento in argumentos.iter() {
        if !argumento.starts_with('-') || argumento == "--" || protegida(argumento) {
            return Err(Error::ArgumentoProtegido);
        }
    }
    Ok(argumentos)
}

/// Sustituye una sola vez, de forma que un valor (incluido el token) nunca se
/// interpreta como otra plantilla ni se vuelve a separar por espacios.
fn expandir(plantilla: &str, valores: &HashMap<&str, &str>) -> Result<String, Error> {
    if plantilla.contains('\0') {
        return Err(Error::Instalacion);
    }
    let mut salida = Zeroizing::new(String::new());
    let mut restante = plantilla;
    while let Some(inicio) = restante.find("${") {
        salida.push_str(&restante[..inicio]);
        let desde = &restante[inicio + 2..];
        let fin = desde.find('}').ok_or(Error::Instalacion)?;
        let valor = valores.get(&desde[..fin]).ok_or(Error::Instalacion)?;
        salida.push_str(valor);
        restante = &desde[fin + 1..];
    }
    salida.push_str(restante);
    Ok(std::mem::take(&mut *salida))
}

// Java no interpreta los prefijos extendidos que devuelve canonicalize en Windows.
// La conversión se limita a rutas; nunca se aplica a credenciales expandidas.
pub(crate) fn ruta_java(texto: &str) -> String {
    #[cfg(windows)]
    {
        if let Some(unc) = texto.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{unc}");
        }
        if let Some(disco) = texto.strip_prefix(r"\\?\") {
            if disco.as_bytes().get(1) == Some(&b':') {
                return disco.to_owned();
            }
        }
    }
    texto.to_owned()
}

fn ruta_texto(path: &Path) -> Result<String, Error> {
    path.to_str()
        .filter(|s| !s.contains('\0'))
        .map(ruta_java)
        .ok_or(Error::Instalacion)
}

fn preparar(
    instalado: &Instalado,
    juego: &Path,
    nombre: &str,
    uuid: &str,
    token: &str,
    xuid: &str,
    config: &Config,
) -> Result<Zeroizing<Vec<String>>, Error> {
    validar(config)?;
    if instalado.main_class.is_empty()
        || !instalado
            .main_class
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'$'))
        || instalado.classpath.is_empty()
    {
        return Err(Error::Instalacion);
    }
    let rutas = instalado
        .classpath
        .iter()
        .map(|path| ruta_texto(path))
        .collect::<Result<Vec<_>, _>>()?;
    let classpath = std::env::join_paths(rutas).map_err(|_| Error::Instalacion)?;
    let classpath = classpath.to_str().ok_or(Error::Instalacion)?;
    let bibliotecas = instalado
        .assets
        .parent()
        .ok_or(Error::Instalacion)?
        .join("libraries");
    let sesion = Zeroizing::new(format!("token:{token}:{uuid}"));
    let ancho = config
        .resolucion
        .map(|r| r.0.to_string())
        .unwrap_or_default();
    let alto = config
        .resolucion
        .map(|r| r.1.to_string())
        .unwrap_or_default();
    let juego = ruta_texto(juego)?;
    let assets = ruta_texto(&instalado.assets)?;
    let natives = ruta_texto(&instalado.natives)?;
    let bibliotecas = ruta_texto(&bibliotecas)?;
    let valores = HashMap::from([
        ("auth_player_name", nombre),
        ("auth_uuid", uuid),
        ("auth_access_token", token),
        ("auth_session", sesion.as_str()),
        ("user_properties", "{}"),
        ("user_type", if token == "0" { "legacy" } else { "msa" }),
        ("clientid", auth::CLIENT_ID),
        ("auth_client_id", auth::CLIENT_ID),
        ("auth_xuid", xuid),
        (
            "version_name",
            instalado.perfil_id.as_deref().unwrap_or(&instalado.version),
        ),
        ("version_type", "release"),
        ("game_directory", juego.as_str()),
        ("assets_root", assets.as_str()),
        ("game_assets", assets.as_str()),
        ("assets_index_name", instalado.asset_index.as_str()),
        ("natives_directory", natives.as_str()),
        ("classpath", classpath),
        ("library_directory", bibliotecas.as_str()),
        ("launcher_name", "Icaro"),
        ("launcher_version", env!("CARGO_PKG_VERSION")),
        ("classpath_separator", if cfg!(windows) { ";" } else { ":" }),
        ("resolution_width", ancho.as_str()),
        ("resolution_height", alto.as_str()),
    ]);
    let mut argumentos = Zeroizing::new(Vec::new());
    argumentos.push(format!("-Xms{}M", config.memoria_min_mb));
    argumentos.push(format!("-Xmx{}M", config.memoria_max_mb));
    for plantilla in &instalado.jvm_arguments {
        // Mojang no decide la memoria de la instancia.
        if plantilla.starts_with("-Xms") || plantilla.starts_with("-Xmx") {
            continue;
        }
        // El perfil guarda algunas rutas literales, como la configuración de Log4j.
        let plantilla = if plantilla.starts_with("-D") {
            if let Some((clave, valor)) = plantilla.split_once('=') {
                format!("{clave}={}", ruta_java(valor))
            } else {
                plantilla.clone()
            }
        } else {
            plantilla.clone()
        };
        argumentos.push(expandir(&plantilla, &valores)?);
    }
    argumentos.extend(argumentos_extra(&config.argumentos_jvm)?.drain(..));
    argumentos.push(instalado.main_class.clone());
    let mut juego_args = Zeroizing::new(Vec::new());
    for plantilla in &instalado.game_arguments {
        juego_args.push(expandir(&ruta_java(plantilla), &valores)?);
    }
    argumentos.extend(juego_args.drain(..));
    if let Some((ancho, alto)) = config.resolucion {
        argumentos.extend([
            "--width".into(),
            ancho.to_string(),
            "--height".into(),
            alto.to_string(),
        ]);
    }
    if config.fullscreen {
        argumentos.push("--fullscreen".into());
    }
    if let Some(servidor)=&config.servidor {
        let destino=crate::servidores::direccion(servidor).map_err(|_|Error::Servidor)?;
        let moderno=instalado.quick_play_multiplayer.unwrap_or_else(||{
            let partes:Vec<_>=instalado.version.split(['.','-']).collect();
            partes.first().and_then(|p|p.parse::<u32>().ok()).is_some_and(|a|a>=26)
                || (partes.first()==Some(&"1")&&partes.get(1).and_then(|p|p.parse::<u32>().ok()).is_some_and(|b|b>=20))
        });
        if moderno {argumentos.extend(["--quickPlayMultiplayer".into(),servidor.trim().into()]);}
        else {argumentos.extend(["--server".into(),destino.host,"--port".into(),destino.puerto.to_string()]);}
    }
    Ok(argumentos)
}

fn redactar(texto: &str, token: &str) -> String {
    if token.is_empty() {
        texto.to_owned()
    } else {
        texto.replace(token, "[REDACTED]")
    }
}

/// Oculta la credencial vigente también en registros recuperados del disco.
pub fn registro_para_exportar(texto: &str, sesion: Option<&auth::Session>) -> String {
    redactar(texto, sesion.map_or("", auth::Session::minecraft_token))
}

fn nivel(texto: &str, origen: Origen) -> Nivel {
    let texto = texto.to_ascii_uppercase();
    if texto.contains("/ERROR]")
        || texto.contains("/FATAL]")
        || texto.contains("[ERROR]")
        || texto.contains("EXCEPTION")
    {
        Nivel::Error
    } else if texto.contains("/WARN]") || texto.contains("[WARN]") {
        Nivel::Advertencia
    } else if texto.contains("/DEBUG]") || texto.contains("/TRACE]") {
        Nivel::Depuracion
    } else if origen == Origen::Stderr {
        Nivel::Advertencia
    } else {
        Nivel::Info
    }
}

fn emitir_linea(
    bytes: &[u8],
    omitida: bool,
    origen: Origen,
    token: &str,
    evento: &impl Fn(Evento),
) {
    let texto = if omitida {
        "[Línea de registro omitida: supera 64 KiB.]".to_owned()
    } else {
        let limpio = Zeroizing::new(String::from_utf8_lossy(bytes).into_owned());
        redactar(limpio.trim_end_matches('\r'), token)
    };
    evento(Evento::Linea {
        origen,
        nivel: nivel(&texto, origen),
        texto,
    });
}

fn nivel_xml(valor: Option<&str>, texto: &str, origen: Origen) -> Nivel {
    match valor.map(str::to_ascii_uppercase).as_deref() {
        Some("FATAL" | "ERROR") => Nivel::Error,
        Some("WARN" | "WARNING") => Nivel::Advertencia,
        Some("DEBUG" | "TRACE") => Nivel::Depuracion,
        Some("INFO" | "ALL") => Nivel::Info,
        _ => nivel(texto, origen),
    }
}

fn texto_xml(nodo: roxmltree::Node<'_, '_>, destino: &mut String) {
    for texto in nodo
        .descendants()
        .filter_map(|n| n.is_text().then(|| n.text()).flatten())
    {
        destino.push_str(texto);
    }
}

fn excepcion_xml(nodo: roxmltree::Node<'_, '_>, destino: &mut String, profundidad: usize) {
    if profundidad > 32 {
        return;
    }
    let nombre = nodo
        .attribute("name")
        .or_else(|| nodo.attribute("class"))
        .unwrap_or("");
    let mensaje = nodo.attribute("message").unwrap_or("");
    if !nombre.is_empty() || !mensaje.is_empty() {
        if !destino.is_empty() && !destino.ends_with('\n') {
            destino.push('\n');
        }
        match nodo.tag_name().name() {
            "Cause" | "CausedBy" => destino.push_str("Caused by: "),
            "Suppressed" => destino.push_str("Suppressed: "),
            _ => {}
        }
        destino.push_str(nombre);
        if !nombre.is_empty() && !mensaje.is_empty() {
            destino.push_str(": ");
        }
        destino.push_str(mensaje);
    }
    for hijo in nodo.children() {
        if hijo.is_text() {
            let texto = hijo.text().unwrap_or("");
            if !texto.trim().is_empty() {
                destino.push_str(texto);
            }
        } else if hijo.is_element() {
            match hijo.tag_name().name() {
                "ExtendedStackTraceItem" | "StackTraceElement" | "StackTraceItem" => {
                    destino.push_str("\n\tat ");
                    destino.push_str(
                        hijo.attribute("class")
                            .or_else(|| hijo.attribute("className"))
                            .unwrap_or("?"),
                    );
                    destino.push('.');
                    destino.push_str(
                        hijo.attribute("method")
                            .or_else(|| hijo.attribute("methodName"))
                            .unwrap_or("?"),
                    );
                    destino.push('(');
                    if hijo.attribute("nativeMethod") == Some("true") {
                        destino.push_str("Native Method");
                    } else if let Some(archivo) = hijo
                        .attribute("file")
                        .or_else(|| hijo.attribute("fileName"))
                    {
                        destino.push_str(archivo);
                        if let Some(linea) = hijo
                            .attribute("line")
                            .or_else(|| hijo.attribute("lineNumber"))
                        {
                            destino.push(':');
                            destino.push_str(linea);
                        }
                    } else {
                        destino.push_str("Unknown Source");
                    }
                    destino.push(')');
                }
                _ => excepcion_xml(hijo, destino, profundidad + 1),
            }
        }
    }
}

/// Mojang entrega fragmentos sin declarar el prefijo; la envoltura solo agrega
/// esa declaración. No se permiten DTD, entidades externas ni más de 2048 nodos.
fn emitir_xml(
    bytes: &[u8],
    origen: Origen,
    token: &str,
    evento: &impl Fn(Evento),
) -> Result<(), ()> {
    let fragmento = std::str::from_utf8(bytes).map_err(|_| ())?;
    let mut xml = Zeroizing::new(String::with_capacity(fragmento.len() + 96));
    xml.push_str("<icaro xmlns:log4j=\"http://logging.apache.org/log4j/2.0/events\">");
    xml.push_str(fragmento);
    xml.push_str("</icaro>");
    let documento = roxmltree::Document::parse_with_options(
        &xml,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: 2048,
        },
    )
    .map_err(|_| ())?;
    let raiz = documento
        .root_element()
        .children()
        .find(|n| n.is_element())
        .ok_or(())?;
    if raiz.tag_name().name() != "Event" {
        return Err(());
    }
    let mut mensaje = Zeroizing::new(String::new());
    let mut excepcion = Zeroizing::new(String::new());
    for nodo in raiz.children().filter(|n| n.is_element()) {
        match nodo.tag_name().name() {
            "Message" | "message" => texto_xml(nodo, &mut mensaje),
            "Throwable" | "throwable" | "Thrown" => excepcion_xml(nodo, &mut excepcion, 0),
            _ => {}
        }
    }
    if !excepcion.is_empty() {
        if !mensaje.is_empty() && !mensaje.ends_with('\n') {
            mensaje.push('\n');
        }
        mensaje.push_str(&excepcion);
    }
    let limpio = redactar(&mensaje, token);
    let nivel = nivel_xml(raiz.attribute("level"), &limpio, origen);
    let thread = redactar(raiz.attribute("thread").unwrap_or(""), token);
    let thread: String = thread
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(256)
        .collect();
    for linea in limpio.lines() {
        let texto = if thread.is_empty() {
            linea.to_owned()
        } else {
            format!("[{thread}] {linea}")
        };
        if texto.len() > MAX_LINEA {
            emitir_linea(&[], true, origen, token, evento);
        } else {
            evento(Evento::Linea {
                origen,
                nivel,
                texto,
            });
        }
    }
    Ok(())
}

/// El detector solo delimita fragmentos; el contenido se interpreta con el
/// parser XML. Conserva su estado entre lecturas, incluso dentro de un CDATA.
#[derive(Default)]
struct MarcoXml {
    bytes: Zeroizing<Vec<u8>>,
    cola: Zeroizing<Vec<u8>>,
    cdata: bool,
    comentario: bool,
    omitido: bool,
}
impl MarcoXml {
    fn agregar(&mut self, byte: u8) -> bool {
        if !self.omitido {
            if self.bytes.len() == MAX_EVENTO_XML {
                self.bytes.zeroize();
                self.omitido = true;
            } else {
                self.bytes.push(byte);
            }
        }
        if self.cola.len() == FIN_XML.len() {
            self.cola.remove(0);
        }
        self.cola.push(byte);
        if self.cdata {
            if self.cola.ends_with(b"]]>") {
                self.cdata = false;
                self.cola.zeroize();
            }
        } else if self.comentario {
            if self.cola.ends_with(b"-->") {
                self.comentario = false;
                self.cola.zeroize();
            }
        } else if self.cola.ends_with(b"<![CDATA[") {
            self.cdata = true;
            self.cola.zeroize();
        } else if self.cola.ends_with(b"<!--") {
            self.comentario = true;
            self.cola.zeroize();
        } else if self.cola.ends_with(FIN_XML) {
            return true;
        }
        false
    }
}

#[derive(Default)]
struct FormatoRegistros {
    linea: Zeroizing<Vec<u8>>,
    omitida: bool,
    inicio: usize,
    descartar_busqueda: bool,
    despues_xml: bool,
    xml: Option<MarcoXml>,
}
impl FormatoRegistros {
    fn advertir_xml(origen: Origen, evento: &impl Fn(Evento)) {
        evento(Evento::Linea {
            origen,
            nivel: Nivel::Advertencia,
            texto: "[Evento de registro XML omitido: inválido, incompleto o supera 256 KiB.]"
                .into(),
        });
    }
    fn agregar(&mut self, byte: u8, origen: Origen, token: &str, evento: &impl Fn(Evento)) {
        if let Some(xml) = &mut self.xml {
            if xml.agregar(byte) {
                if xml.omitido || emitir_xml(&xml.bytes, origen, token, evento).is_err() {
                    Self::advertir_xml(origen, evento);
                }
                self.xml = None;
                self.despues_xml = true;
            }
            return;
        }
        if byte == b'\n' {
            if self.omitida
                || !self.despues_xml
                || self.linea.iter().any(|b| !b.is_ascii_whitespace())
            {
                emitir_linea(&self.linea, self.omitida, origen, token, evento);
            }
            self.linea.zeroize();
            self.omitida = false;
            self.inicio = 0;
            self.descartar_busqueda = false;
            return;
        }
        if self.omitida {
            return;
        }
        if self.linea.len() == MAX_LINEA {
            self.linea.zeroize();
            self.omitida = true;
            return;
        }
        self.linea.push(byte);
        if self.descartar_busqueda {
            return;
        }
        if self.linea.len() == self.inicio + 1 && byte.is_ascii_whitespace() {
            self.inicio += 1;
            return;
        }
        let candidato = &self.linea[self.inicio..];
        if candidato.len() <= INICIO_XML.len() {
            self.descartar_busqueda = !INICIO_XML.starts_with(candidato);
        } else if candidato.len() == INICIO_XML.len() + 1
            && (byte.is_ascii_whitespace() || byte == b'>')
        {
            let mut xml = MarcoXml::default();
            for &byte in candidato {
                xml.agregar(byte);
            }
            self.linea.zeroize();
            self.inicio = 0;
            self.descartar_busqueda = false;
            self.xml = Some(xml);
        } else {
            self.descartar_busqueda = true;
        }
    }
    fn finalizar(self, origen: Origen, token: &str, evento: &impl Fn(Evento)) {
        if self.xml.is_some() {
            Self::advertir_xml(origen, evento);
        } else if self.omitida
            || (!self.linea.is_empty()
                && (!self.despues_xml || self.linea.iter().any(|b| !b.is_ascii_whitespace())))
        {
            emitir_linea(&self.linea, self.omitida, origen, token, evento);
        }
    }
}

async fn leer(
    mut lector: impl AsyncRead + Unpin,
    origen: Origen,
    token: &str,
    evento: &(impl Fn(Evento) + Send + Sync),
) -> Result<(), Error> {
    let mut formato = FormatoRegistros::default();
    let mut bloque = Zeroizing::new([0u8; 8192]);
    loop {
        let cantidad = lector
            .read(&mut *bloque)
            .await
            .map_err(|_| Error::Registros)?;
        if cantidad == 0 {
            formato.finalizar(origen, token, evento);
            return Ok(());
        }
        for &byte in &bloque[..cantidad] {
            formato.agregar(byte, origen, token, evento);
        }
    }
}

async fn proceso(
    mut comando: Command,
    token: &str,
    cancel: CancellationToken,
    on_event: &(impl Fn(Evento) + Send + Sync),
) -> Result<Salida, Error> {
    if cancel.is_cancelled() {
        return Err(Error::Cancelado);
    }
    comando
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    // La configuración global de Java podría cambiar el classpath, imprimir
    // parámetros privados o introducir argumentos fuera de esta instancia.
    for variable in [
        "JAVA_TOOL_OPTIONS",
        "JDK_JAVA_OPTIONS",
        "_JAVA_OPTIONS",
        "CLASSPATH",
    ] {
        comando.env_remove(variable);
    }
    #[cfg(windows)]
    comando.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    let mut hijo = comando.spawn().map_err(|_| Error::Java)?;
    drop(comando);
    let stdout = hijo.stdout.take().ok_or(Error::Registros)?;
    let stderr = hijo.stderr.take().ok_or(Error::Registros)?;
    on_event(Evento::Inicio {
        pid: hijo.id().ok_or(Error::Java)?,
    });
    let finalizo = CancellationToken::new();
    let esperar = async {
        let salida = async {
            let (estado, cancelado) = tokio::select! {
                biased;
                _ = cancel.cancelled() => {
                    hijo.start_kill().map_err(|_| Error::Detener)?;
                    (hijo.wait().await.map_err(|_| Error::Espera)?, true)
                }
                estado = hijo.wait() => (estado.map_err(|_| Error::Espera)?, false),
            };
            Ok::<Salida, Error>(Salida {
                codigo: estado.code(),
                exito: estado.success() && !cancelado,
                cancelado,
            })
        }
        .await;
        finalizo.cancel();
        salida
    };
    let capturar = async {
        let lecturas = async {
            tokio::join!(
                leer(stdout, Origen::Stdout, token, on_event),
                leer(stderr, Origen::Stderr, token, on_event)
            )
        };
        tokio::pin!(lecturas);
        tokio::select! {
            result = &mut lecturas => result,
            _ = finalizo.cancelled() => {
                // Un descendiente que herede los pipes no puede retener el
                // cierre del launcher una vez que se ha recogido Java.
                tokio::time::timeout(Duration::from_secs(2), lecturas)
                    .await.unwrap_or((Ok(()), Ok(())))
            }
        }
    };
    let (salida, (stdout, stderr)) = tokio::join!(esperar, capturar);
    let salida = salida?;
    on_event(Evento::Fin(salida));
    stdout?;
    stderr?;
    Ok(salida)
}

fn java_del_juego(java: &Path) -> std::path::PathBuf {
    #[cfg(windows)]
    if java
        .file_name()
        .is_some_and(|name| name.eq_ignore_ascii_case("java.exe"))
    {
        // Discord reconoce Minecraft mediante javaw.exe. Solo se utiliza el
        // acompañante del mismo runtime; las sondas e instaladores usan java.
        let ventana = java.with_file_name("javaw.exe");
        if ventana.is_file() {
            return ventana;
        }
    }
    java.to_path_buf()
}

/// Espera hasta que Java termina o hasta que se cancela y se recoge el proceso.
/// No guarda ni publica la línea de comandos que contiene la credencial.
pub async fn ejecutar(
    instalado: &Instalado,
    java: &Path,
    juego: &Path,
    cuenta: &auth::Session,
    config: Config,
    cancel: CancellationToken,
    on_event: impl Fn(Evento) + Send + Sync,
) -> Result<Salida, Error> {
    if cancel.is_cancelled() {
        return Err(Error::Cancelado);
    }
    if !cuenta.puede_jugar() {
        return Err(Error::Cuenta);
    }
    if cuenta.minecraft_token().chars().any(char::is_control) {
        return Err(Error::Cuenta);
    }
    validar(&config)?;
    let java = java.canonicalize().map_err(|_| Error::Java)?;
    if !java.is_file() {
        return Err(Error::Java);
    }
    std::fs::create_dir_all(juego).map_err(|_| Error::Carpeta)?;
    let juego = juego.canonicalize().map_err(|_| Error::Carpeta)?;
    if !instalado.assets.is_dir()
        || !instalado.natives.is_dir()
        || instalado
            .classpath
            .iter()
            .any(|path| !path.is_file() || !path.is_absolute())
        || !instalado.assets.is_absolute()
        || !instalado.natives.is_absolute()
    {
        return Err(Error::Instalacion);
    }
    let argumentos = preparar(
        instalado,
        &juego,
        &cuenta.profile.name,
        &cuenta.profile.id,
        if cuenta.es_local() { "0" } else { cuenta.minecraft_token() },
        cuenta.xbox_user_id().unwrap_or(""),
        &config,
    )?;
    let mut comando = Command::new(java_del_juego(&java));
    let skin = if cuenta.es_local() {
        if let Some(raiz)=&config.cuentas_locales {
            on_event(Evento::Linea {origen:Origen::Stdout,nivel:Nivel::Info,texto:"Preparando la skin local…".into()});
            crate::skin_local::preparar(raiz,cuenta,cancel.clone()).await.map_err(|_|if cancel.is_cancelled(){Error::Cancelado}else{Error::Skin})?
        } else {None}
    } else {None};
    if let Some(skin)=&skin {comando.args(&skin.argumentos);}
    let opciones = config.opciones_compartidas.as_ref().and_then(|path| {
        match crate::opciones_minecraft::preparar(path, &juego, instalado) {
            Ok(sesion) => Some(sesion),
            Err(error) => {
                on_event(Evento::Linea { origen: Origen::Stderr, nivel: Nivel::Advertencia,
                    texto: format!("No se aplicaron las preferencias compartidas: {error}") });
                None
            }
        }
    });
    comando.current_dir(&juego).args(argumentos.iter());
    drop(argumentos);
    let resultado = proceso(comando, cuenta.minecraft_token(), cancel, &on_event).await;
    if resultado.is_ok() {
        if let Some(sesion) = opciones {
            if let Err(error) = crate::opciones_minecraft::capturar(&sesion) {
                on_event(Evento::Linea { origen: Origen::Stderr, nivel: Nivel::Advertencia,
                    texto: format!("No se guardaron las preferencias compartidas: {error}") });
            }
        }
    }
    resultado
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::PathBuf, sync::Mutex};

    #[cfg(windows)]
    #[test]
    fn juego_prefiere_javaw_del_mismo_runtime_y_conserva_java_si_falta() {
        let root = std::env::temp_dir().join(format!(
            "icaro-javaw-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let java = root.join("java.exe");
        let javaw = root.join("javaw.exe");
        assert_eq!(java_del_juego(&java), java);
        std::fs::write(&javaw, b"fixture").unwrap();
        assert_eq!(java_del_juego(&java), javaw);
        let personalizado = root.join("otro.exe");
        assert_eq!(java_del_juego(&personalizado), personalizado);
        std::fs::remove_file(javaw).unwrap();
        std::fs::remove_dir(root).unwrap();
    }

    #[cfg(windows)]
    #[tokio::test]
    #[ignore = "Requiere ICARO_PRUEBA_JAVA con la ruta a java.exe de un JDK local; no ejecuta Minecraft"]
    async fn javaw_real_conserva_stdout_stderr_y_cancelacion() {
        let java = PathBuf::from(std::env::var_os("ICARO_PRUEBA_JAVA").expect("ICARO_PRUEBA_JAVA"));
        let javaw = java_del_juego(&java);
        assert!(javaw.file_name().unwrap().eq_ignore_ascii_case("javaw.exe"));
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/qa-javaw");
        std::fs::create_dir_all(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let source = root.join("IcaroPruebaRegistros.java");
        std::fs::write(
            &source,
            r#"class IcaroPruebaRegistros {
            public static void main(String[] args) throws Exception {
                System.out.println("salida-javaw");
                System.err.println("error-javaw");
                if (args.length > 0) Thread.sleep(60000);
            }
        }"#,
        )
        .unwrap();
        let status = Command::new(java.with_file_name("javac.exe"))
            .arg(&source)
            .creation_flags(0x0800_0000)
            .status()
            .await
            .unwrap();
        assert!(status.success());
        let eventos = Mutex::new(Vec::new());
        let mut command = Command::new(&javaw);
        command.arg("-cp").arg(&root).arg("IcaroPruebaRegistros");
        let salida = proceso(command, "", CancellationToken::new(), &|event| {
            eventos.lock().unwrap().push(event);
        })
        .await
        .unwrap();
        assert!(salida.exito);
        let eventos = eventos.into_inner().unwrap();
        assert!(eventos.iter().any(|event| matches!(event, Evento::Linea { origen: Origen::Stdout, texto, .. } if texto == "salida-javaw")));
        assert!(eventos.iter().any(|event| matches!(event, Evento::Linea { origen: Origen::Stderr, texto, .. } if texto == "error-javaw")));
        let cancel = CancellationToken::new();
        let mut command = Command::new(&javaw);
        command
            .arg("-cp")
            .arg(&root)
            .args(["IcaroPruebaRegistros", "esperar"]);
        let salida = tokio::time::timeout(
            Duration::from_secs(10),
            proceso(command, "", cancel.clone(), &|event| {
                if matches!(event, Evento::Linea { .. }) {
                    cancel.cancel();
                }
            }),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(salida.cancelado && !salida.exito);
    }

    fn instalado() -> Instalado {
        Instalado {
            version: "1.21.1".into(),
            perfil_id: None,
            quick_play_multiplayer: None,
            main_class: "net.minecraft.client.main.Main".into(),
            classpath: vec![
                PathBuf::from("bibliotecas con espacios/lwjgl.jar"),
                PathBuf::from("cliente.jar"),
            ],
            natives: PathBuf::from("datos/natives con espacios"),
            assets: PathBuf::from("datos/assets"),
            asset_index: "17".into(),
            java_major: 21,
            java_component: "java-runtime-delta".into(),
            java_perfil: None,
            game_arguments: vec![
                "--username".into(),
                "${auth_player_name}".into(),
                "--accessToken".into(),
                "${auth_access_token}".into(),
                "--gameDir".into(),
                "${game_directory}".into(),
                "--xuid".into(),
                "${auth_xuid}".into(),
            ],
            jvm_arguments: vec![
                "-Djava.library.path=${natives_directory}".into(),
                "-cp".into(),
                "${classpath}".into(),
            ],
        }
    }

    #[test]
    fn lanzamiento_resuelve_almacen_compartido_y_datos_privados() {
        let mut installed = instalado();
        let shared = std::env::temp_dir().join("Icaro datos compartidos");
        let private = shared.join("instances/Primera/minecraft");
        installed.assets = shared.join("assets");
        installed.natives = shared.join("instances/Primera/.icaro/natives");
        installed.classpath = vec![
            shared.join("libraries/hash/library.jar"),
            shared.join("meta/minecraft/clients/hash/client.jar"),
        ];
        installed
            .jvm_arguments
            .push("-DlibraryDirectory=${library_directory}".into());
        installed
            .game_arguments
            .extend(["--assetsDir".into(), "${assets_root}".into()]);
        let args = preparar(
            &installed,
            &private,
            "Jugador",
            "uuid",
            "token",
            "xuid",
            &Config::default(),
        )
        .unwrap();
        assert!(args.contains(&format!(
            "-DlibraryDirectory={}",
            shared.join("libraries").display()
        )));
        let cp = args.iter().position(|a| a == "-cp").unwrap();
        assert_eq!(
            args[cp + 1],
            std::env::join_paths(&installed.classpath)
                .unwrap()
                .to_str()
                .unwrap()
        );
        assert!(args.contains(&private.to_string_lossy().into_owned()));
        assert!(args.contains(&installed.assets.to_string_lossy().into_owned()));
        assert!(args.contains(&format!(
            "-Djava.library.path={}",
            installed.natives.display()
        )));
    }

    #[test]
    fn argumentos_locales_usan_identidad_estable_y_no_credenciales_microsoft() {
        let mut instalado = instalado();
        instalado.game_arguments = vec!["${auth_player_name}".into(),"${auth_uuid}".into(),"${auth_access_token}".into(),"${user_type}".into(),"${auth_xuid}".into()];
        let cuenta = auth::Session::local("Jugador_1").unwrap();
        let argumentos = preparar(&instalado,Path::new("minecraft"),&cuenta.profile.name,&cuenta.profile.id,"0","",&Config::default()).unwrap();
        assert!(argumentos.contains(&cuenta.profile.name));
        assert!(argumentos.contains(&cuenta.profile.id));
        assert!(argumentos.contains(&"legacy".to_string()));
        assert!(argumentos.contains(&"0".to_string()));
        let oficial = preparar(&instalado,Path::new("minecraft"),"Oficial","uuid","token-oficial","xuid",&Config::default()).unwrap();
        assert!(oficial.contains(&"msa".to_string()) && oficial.contains(&"token-oficial".to_string()));
    }

    #[test]
    #[cfg(windows)]
    fn lanzamiento_convierte_rutas_extendidas_sin_alterar_credenciales() {
        let mut installed = instalado();
        installed.assets = PathBuf::from(r"\\?\C:\Icaro con espacios\assets");
        installed.natives = PathBuf::from(r"\\?\C:\Icaro con espacios\natives");
        installed.classpath = vec![
            PathBuf::from(r"\\?\C:\Icaro con espacios\fabric-loader.jar"),
            PathBuf::from(r"\\?\UNC\servidor\juegos\cliente.jar"),
        ];
        installed
            .jvm_arguments
            .push(r"-Dlog4j.configurationFile=\\?\C:\Icaro con espacios\client.xml".into());
        installed.game_arguments.extend([
            "--assetsDir".into(),
            "${assets_root}".into(),
            "--gameAssets".into(),
            r"\\?\C:\Icaro con espacios\legacy".into(),
        ]);
        let token = r"\\?\C:\TOKEN_${version_name}";
        let args = preparar(
            &installed,
            Path::new(r"\\?\C:\Icaro con espacios\juego"),
            "Jugador",
            "uuid",
            token,
            "xuid",
            &Config::default(),
        )
        .unwrap();
        let cp = args.iter().position(|a| a == "-cp").unwrap();
        assert_eq!(
            args[cp + 1],
            r"C:\Icaro con espacios\fabric-loader.jar;\\servidor\juegos\cliente.jar"
        );
        for esperado in [
            r"C:\Icaro con espacios\juego",
            r"C:\Icaro con espacios\assets",
            r"C:\Icaro con espacios\legacy",
            r"-Djava.library.path=C:\Icaro con espacios\natives",
            r"-Dlog4j.configurationFile=C:\Icaro con espacios\client.xml",
            token,
        ] {
            assert!(args.iter().any(|a| a == esperado), "Falta {esperado}");
        }
        assert_eq!(ruta_java(r"C:\normal\cliente.jar"), r"C:\normal\cliente.jar");
    }

    #[test]
    fn expansion_preserves_literal_paths_and_never_reexpands_credentials() {
        let config = Config {
            resolucion: Some((1280, 720)),
            fullscreen: true,
            ..Config::default()
        };
        let args = preparar(
            &instalado(),
            Path::new("juego con espacios"),
            "Jugador",
            "uuid",
            "TOKEN_${version_name}",
            "2533274991020393",
            &config,
        )
        .unwrap();
        assert!(args.iter().any(|a| a == "juego con espacios"));
        assert!(args.iter().any(|a| a == "TOKEN_${version_name}"));
        let xuid = args.iter().position(|a| a == "--xuid").unwrap();
        assert_eq!(args[xuid + 1], "2533274991020393");
        let cp = args.iter().position(|a| a == "-cp").unwrap();
        assert!(args[cp + 1].contains("bibliotecas con espacios"));
        assert!(args
            .iter()
            .any(|a| a == "-Djava.library.path=datos/natives con espacios"));
        assert!(args.ends_with(&[
            "--width".into(),
            "1280".into(),
            "--height".into(),
            "720".into(),
            "--fullscreen".into()
        ]));
        let values = HashMap::from([("auth_access_token", "CANARY")]);
        let error = expandir("${no_existe}CANARY", &values).unwrap_err();
        assert!(!format!("{error:?}: {error}").contains("CANARY"));
    }

    #[test]
    fn fabric_conserva_perfil_memoria_y_argumentos_oficiales() {
        let mut installed = instalado();
        installed.perfil_id = Some("fabric-loader-0.16.14-1.21.1".into());
        installed.main_class = "net.fabricmc.loader.impl.launch.knot.KnotClient".into();
        installed
            .game_arguments
            .extend(["--version".into(), "${version_name}".into()]);
        installed
            .jvm_arguments
            .push("-DFabricMcEmu= net.minecraft.client.main.Main ".into());
        let config = Config {
            memoria_min_mb: 1024,
            memoria_max_mb: 4096,
            argumentos_jvm: "-Dpersonal=valor".into(),
            ..Config::default()
        };
        let args = preparar(
            &installed,
            Path::new("juego"),
            "Jugador",
            "uuid",
            "token",
            "xuid",
            &config,
        )
        .unwrap();
        for expected in [
            "-Xms1024M",
            "-Xmx4096M",
            "-Dpersonal=valor",
            "-DFabricMcEmu= net.minecraft.client.main.Main ",
            "net.fabricmc.loader.impl.launch.knot.KnotClient",
            "fabric-loader-0.16.14-1.21.1",
        ] {
            assert!(args.iter().any(|a| a == expected), "{expected}");
        }
    }

    #[test]
    fn conectar_servidor_respeta_perfil_quickplay_y_argumentos_literales(){
        let mut game=instalado();game.quick_play_multiplayer=Some(true);
        let c=Config{memoria_min_mb:1024,memoria_max_mb:4096,servidor:Some("play.example.net".into()),..Config::default()};
        let args=preparar(&game,Path::new("juego"),"Jugador","uuid","token","xuid",&c).unwrap();
        assert!(args.windows(2).any(|v|v==["--quickPlayMultiplayer","play.example.net"]));
        assert!(args.iter().any(|v|v=="-Xmx4096M"));assert!(!args.iter().any(|v|v=="--server"));
        game.quick_play_multiplayer=Some(false);
        let args=preparar(&game,Path::new("juego"),"Jugador","uuid","token","xuid",&c).unwrap();
        assert!(args.windows(2).any(|v|v==["--server","play.example.net"]));
        assert!(args.windows(2).any(|v|v==["--port","25565"]));
        let invalid=Config{servidor:Some("host --accessToken bad".into()),..Config::default()};
        assert!(matches!(preparar(&game,Path::new("juego"),"Jugador","uuid","token","xuid",&invalid),Err(Error::Servidor)));
        assert!(!preparar(&game,Path::new("juego"),"Jugador","uuid","token","xuid",&Config::default()).unwrap().iter().any(|v|v=="--server"||v=="--quickPlayMultiplayer"));
    }

    #[test]
    fn custom_arguments_do_not_override_managed_launch_arguments() {
        let args = argumentos_extra(
            r#"-Dmensaje="hola mundo" -Druta="C:\Juegos con espacios\perfil" -XX:+UseG1GC"#,
        )
        .unwrap();
        assert_eq!(
            &*args,
            &[
                "-Dmensaje=hola mundo",
                r"-Druta=C:\Juegos con espacios\perfil",
                "-XX:+UseG1GC"
            ]
        );
        for valor in [
            "-Xmx8G",
            "-Xms1G",
            "-cp evil.jar",
            "--class-path=evil.jar",
            "-Djava.library.path=evil",
            "-Duser.dir=otro",
            "-XX:MaxHeapSize=8000000",
            "evil.Main",
            "@argumentos",
            "--",
        ] {
            assert_eq!(
                argumentos_extra(valor).unwrap_err(),
                Error::ArgumentoProtegido
            );
        }
        assert_eq!(
            argumentos_extra("-Dtest=\"sin cerrar").unwrap_err(),
            Error::ArgumentosJvm
        );
        assert!(validar(&Config {
            memoria_min_mb: 4096,
            memoria_max_mb: 1024,
            ..Config::default()
        })
        .is_err());
        assert!(validar(&Config {
            resolucion: Some((0, 720)),
            ..Config::default()
        })
        .is_err());
    }

    #[tokio::test]
    async fn logs_hide_tokens_across_reads_and_bound_long_lines() {
        use tokio::io::AsyncWriteExt;
        let token = "CANARY_GAME_TOKEN";
        let (mut writer, reader) = tokio::io::duplex(32);
        let eventos = Mutex::new(Vec::new());
        let escritura = async move {
            writer
                .write_all(b"[main/ERROR] CANARY_GAME_")
                .await
                .unwrap();
            writer.write_all(b"TOKEN\r\n").await.unwrap();
            writer.write_all(&vec![b'x'; MAX_LINEA + 1]).await.unwrap();
            writer.write_all(token.as_bytes()).await.unwrap();
            writer.write_all(b"\nultima linea").await.unwrap();
        };
        let callback = |e| eventos.lock().unwrap().push(e);
        let lectura = leer(reader, Origen::Stdout, token, &callback);
        // El cierre del escritor entrega EOF después de todos los fragmentos.
        let ((), result) = tokio::join!(escritura, lectura);
        result.unwrap();
        let eventos = eventos.into_inner().unwrap();
        assert_eq!(eventos.len(), 3);
        assert!(!format!("{eventos:?}").contains(token));
        assert!(
            matches!(&eventos[0], Evento::Linea { nivel: Nivel::Error, texto, .. } if texto.contains("[REDACTED]"))
        );
        assert!(matches!(&eventos[1], Evento::Linea { texto, .. } if texto.contains("omitida")));
        assert!(matches!(&eventos[2], Evento::Linea { texto, .. } if texto == "ultima linea"));
    }

    async fn registros_fragmentados(bytes: &[u8], fragmento: usize, origen: Origen) -> Vec<Evento> {
        use tokio::io::AsyncWriteExt;
        let (mut writer, reader) = tokio::io::duplex(17);
        let eventos = Mutex::new(Vec::new());
        let escritura = async move {
            for bloque in bytes.chunks(fragmento) {
                writer.write_all(bloque).await.unwrap();
            }
        };
        let callback = |evento| eventos.lock().unwrap().push(evento);
        let ((), resultado) = tokio::join!(
            escritura,
            leer(reader, origen, "CANARY_GAME_TOKEN", &callback)
        );
        resultado.unwrap();
        eventos.into_inner().unwrap()
    }

    #[tokio::test]
    async fn xml_real_de_mojang_conserva_mensaje_thread_y_nivel_sin_etiquetas() {
        let entrada = concat!(
            "Java preparado\r\n",
            "  <log4j:Event logger=\"com.mojang.datafixers.DataFixerBuilder\" timestamp=\"1791743629424\" level=\"INFO\" thread=\"Datafixer Bootstrap #0\">\n",
            "    <log4j:Message><![CDATA[307 Datafixer optimizations took 235 milliseconds]]></log4j:Message>\n",
            "  </log4j:Event>\n",
            "  <log4j:Event logger=\"net.minecraft.client.Minecraft\" timestamp=\"1791743633089\" level=\"INFO\" thread=\"Render thread\">\n",
            "    <log4j:Message><![CDATA[Backend library: LWJGL version 3.4.3+4]]></log4j:Message>\n",
            "  </log4j:Event>\n",
            "[main/WARN] texto plano\nultima línea"
        );
        let registros = registros_fragmentados(entrada.as_bytes(), 1, Origen::Stderr).await;
        assert_eq!(registros.len(), 5);
        assert!(matches!(&registros[0], Evento::Linea { texto, .. } if texto == "Java preparado"));
        assert!(
            matches!(&registros[1], Evento::Linea { nivel:Nivel::Info, texto, origen:Origen::Stderr } if texto == "[Datafixer Bootstrap #0] 307 Datafixer optimizations took 235 milliseconds")
        );
        assert!(
            matches!(&registros[2], Evento::Linea { nivel:Nivel::Info, texto, .. } if texto == "[Render thread] Backend library: LWJGL version 3.4.3+4")
        );
        assert!(
            matches!(&registros[3], Evento::Linea { nivel:Nivel::Advertencia, texto, .. } if texto == "[main/WARN] texto plano")
        );
        assert!(matches!(&registros[4], Evento::Linea { texto, .. } if texto == "ultima línea"));
        assert!(!format!("{registros:?}").contains("log4j:"));
    }

    #[tokio::test]
    async fn xml_fragmentado_no_cierra_cdata_por_etiquetas_literales_y_decodifica_entidades() {
        let entrada = concat!(
            "<log4j:Event level='WARN' thread='Worker &amp; 4'>",
            "<!-- texto </log4j:Event> -->",
            "<log4j:Message><![CDATA[Escena: café </log4j:Event> &amp; literal]]> y &lt;entidad&gt;</log4j:Message>",
            "</log4j:Event>",
            "<log4j:Event level='TRACE' thread='Render thread'><log4j:Message>fin</log4j:Message></log4j:Event>"
        );
        let registros = registros_fragmentados(entrada.as_bytes(), 2, Origen::Stdout).await;
        assert_eq!(registros.len(), 2);
        assert!(
            matches!(&registros[0], Evento::Linea { nivel:Nivel::Advertencia, texto, .. } if texto == "[Worker & 4] Escena: café </log4j:Event> &amp; literal y <entidad>")
        );
        assert!(
            matches!(&registros[1], Evento::Linea { nivel:Nivel::Depuracion, texto, .. } if texto == "[Render thread] fin")
        );
    }

    #[tokio::test]
    async fn xml_throwable_conserva_stack_trace_y_oculta_tokens_despues_de_decodificar() {
        let entrada = concat!(
            "<log4j:Event level='WARN' thread='CANARY&#95;GAME_TOKEN'>\n",
            "<log4j:Message><![CDATA[Falló CANARY_GAME_]]><![CDATA[TOKEN]]></log4j:Message>\n",
            "<log4j:Throwable><![CDATA[java.lang.IllegalStateException: CANARY_GAME_TOKEN\n",
            "\tat minecraft.Main.run(Main.java:42)\n",
            "Caused by: java.io.IOException: conexión]]></log4j:Throwable>\n",
            "</log4j:Event>\n"
        );
        let registros = registros_fragmentados(entrada.as_bytes(), 3, Origen::Stdout).await;
        assert_eq!(registros.len(), 4);
        assert!(!format!("{registros:?}").contains("CANARY_GAME_TOKEN"));
        for registro in &registros {
            assert!(
                matches!(registro, Evento::Linea { nivel:Nivel::Advertencia, texto, .. } if texto.starts_with("[[REDACTED]] "))
            );
        }
        assert!(
            matches!(&registros[0], Evento::Linea { texto, .. } if texto.ends_with("Falló [REDACTED]"))
        );
        assert!(
            matches!(&registros[1], Evento::Linea { texto, .. } if texto.contains("java.lang.IllegalStateException: [REDACTED]"))
        );
        assert!(
            matches!(&registros[2], Evento::Linea { texto, .. } if texto.ends_with("\tat minecraft.Main.run(Main.java:42)"))
        );
        assert!(
            matches!(&registros[3], Evento::Linea { texto, .. } if texto.ends_with("Caused by: java.io.IOException: conexión"))
        );
    }

    #[tokio::test]
    async fn xml_excepciones_estructuradas_conservan_causas_y_lineas() {
        let entrada = concat!(
            "<log4j:Event level='FATAL' thread='Render thread'>",
            "<log4j:Message>Error de arranque</log4j:Message>",
            "<log4j:Thrown name='java.lang.RuntimeException' message='fallo'>",
            "<log4j:ExtendedStackTrace><log4j:ExtendedStackTraceItem class='minecraft.Main' method='run' file='Main.java' line='12'/></log4j:ExtendedStackTrace>",
            "<log4j:Cause name='java.io.IOException' message='CANARY_GAME_TOKEN'/>",
            "</log4j:Thrown></log4j:Event>"
        );
        let registros = registros_fragmentados(entrada.as_bytes(), 7, Origen::Stdout).await;
        assert_eq!(registros.len(), 4);
        assert!(registros.iter().all(|r| matches!(
            r,
            Evento::Linea {
                nivel: Nivel::Error,
                ..
            }
        )));
        assert!(
            matches!(&registros[2], Evento::Linea { texto, .. } if texto.ends_with("\tat minecraft.Main.run(Main.java:12)"))
        );
        assert!(
            matches!(&registros[3], Evento::Linea { texto, .. } if texto.ends_with("Caused by: java.io.IOException: [REDACTED]"))
        );
        assert!(!format!("{registros:?}").contains("CANARY_GAME_TOKEN"));
    }

    #[tokio::test]
    async fn xml_invalido_incompleto_o_excesivo_se_acota_y_recupera_texto_normal() {
        let mut entrada = String::from("<log4j:Event level='INFO'><log4j:Message><![CDATA[");
        entrada.push_str(&"x".repeat(MAX_EVENTO_XML));
        entrada.push_str("CANARY_GAME_TOKEN]]></log4j:Message></log4j:Event>\nnormal\n");
        entrada.push_str(
            "<log4j:Event level='INFO'><log4j:Message>&externa;</log4j:Message></log4j:Event>\n",
        );
        entrada.push_str("<log4j:Event level='INFO'><log4j:Message>CANARY_GAME_TOKEN");
        let registros = registros_fragmentados(entrada.as_bytes(), 8192, Origen::Stdout).await;
        assert_eq!(registros.len(), 4);
        assert!(matches!(&registros[1], Evento::Linea { texto, .. } if texto == "normal"));
        for index in [0, 2, 3] {
            assert!(
                matches!(&registros[index], Evento::Linea { nivel:Nivel::Advertencia, texto, .. } if texto.contains("XML omitido"))
            );
        }
        assert!(!format!("{registros:?}").contains("CANARY_GAME_TOKEN"));
        assert!(emitir_xml(b"<log4j:Event><!DOCTYPE a [<!ENTITY x SYSTEM 'file:///secret'>]><log4j:Message>&x;</log4j:Message></log4j:Event>", Origen::Stdout, "", &|_| panic!("DTD jamás debe publicarse")).is_err());
    }

    // El binario de pruebas sirve como proceso independiente sin Java ni red.
    #[test]
    fn proceso_fixture() {
        let Ok(modo) = std::env::var("ICARO_LAUNCH_TEST_FIXTURE") else {
            return;
        };
        println!("fixture-ready CANARY_GAME_TOKEN");
        eprintln!("[main/ERROR] CANARY_GAME_TOKEN");
        if modo == "cancelar" {
            loop {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    }

    fn fixture(modo: &str) -> Command {
        let mut comando = Command::new(std::env::current_exe().unwrap());
        comando
            .args([
                "--exact",
                "lanzamiento::tests::proceso_fixture",
                "--nocapture",
            ])
            .env("ICARO_LAUNCH_TEST_FIXTURE", modo);
        comando
    }

    #[tokio::test]
    async fn process_fixture_reports_output_and_reaps_on_cancellation() {
        let eventos = Mutex::new(Vec::new());
        let salida = proceso(
            fixture("terminar"),
            "CANARY_GAME_TOKEN",
            CancellationToken::new(),
            &|e| eventos.lock().unwrap().push(e),
        )
        .await
        .unwrap();
        assert!(salida.exito && !salida.cancelado && salida.codigo == Some(0));
        let eventos = eventos.into_inner().unwrap();
        assert!(matches!(eventos.first(), Some(Evento::Inicio { .. })));
        assert!(matches!(eventos.last(), Some(Evento::Fin(_))));
        assert!(!format!("{eventos:?}").contains("CANARY_GAME_TOKEN"));
        let cancel = CancellationToken::new();
        let salida = tokio::time::timeout(std::time::Duration::from_secs(10), proceso(fixture("cancelar"), "CANARY_GAME_TOKEN", cancel.clone(), &|e| {
            if matches!(e, Evento::Linea { ref texto, .. } if texto.contains("fixture-ready")) { cancel.cancel(); }
        })).await.unwrap().unwrap();
        assert!(salida.cancelado && !salida.exito);
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert_eq!(
            proceso(fixture("cancelar"), "CANARY_GAME_TOKEN", cancel, &|_| {})
                .await
                .unwrap_err(),
            Error::Cancelado
        );
    }
}
