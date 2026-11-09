// SPDX-License-Identifier: GPL-3.0-only
//! Acceso acotado a los archivos de registro.
//!
//! Solo se aceptan carpetas y archivos regulares: los enlaces simbólicos, uniones
//! y otros puntos de reanálisis se rechazan sin seguirlos ni modificarlos. En
//! Windows la apertura usa `FILE_FLAG_OPEN_REPARSE_POINT`; en Unix se compara el
//! archivo abierto con la entrada de la carpeta. Un archivo existente se abre sin
//! `create` y uno nuevo con `create_new`, que falla ante cualquier entrada previa
//! (también un enlace roto); nunca se trunca al abrir.
//!
//! La carpeta `logs` y la raíz que la contiene se validan antes y después de
//! abrir, escribir y en cada paso de la rotación: si después del arranque se
//! sustituyen por un enlace o unión persistente, no se escribe ni se mueve nada a
//! través de él. Queda una ventana TOCTOU entre comprobación y uso, explotable
//! solo por el mismo usuario con un cambio y una restauración muy rápidos; en ese
//! caso puede quedar como mucho un archivo vacío creado fuera, o una escritura que
//! se revierte si la comprobación posterior la detecta. No es un aislamiento
//! absoluto (no hay `openat`/`O_NOFOLLOW` sin dependencias nuevas).
use super::modelos::Causa;
use std::{
    ffi::OsString,
    fs::{self, File, Metadata, OpenOptions, TryLockError},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

/// La ruta existe pero no es un archivo o carpeta regular.
pub(crate) const NO_REGULAR: Causa = Causa::Validacion("no_regular");
/// El archivo creció fuera del bloqueo respecto al tamaño que vio el llamador.
pub(crate) const TAMANO_AJENO: Causa = Causa::Validacion("tamano_ajeno");
/// Otro proceso mantiene el bloqueo más allá del plazo.
pub(crate) const BLOQUEO_OCUPADO: Causa = Causa::Disco {
    tipo: "bloqueo_ocupado",
    codigo_os: None,
};

pub(crate) fn causa_io(error: io::Error) -> Causa {
    Causa::de_io(&error)
}

#[cfg(windows)]
fn es_reanalisis(m: &Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
    m.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0
}

#[cfg(not(windows))]
fn es_reanalisis(_: &Metadata) -> bool {
    false
}

fn es_regular(m: &Metadata) -> bool {
    let tipo = m.file_type();
    tipo.is_file() && !tipo.is_symlink() && !es_reanalisis(m)
}

fn es_carpeta_real(m: &Metadata) -> bool {
    let tipo = m.file_type();
    tipo.is_dir() && !tipo.is_symlink() && !es_reanalisis(m)
}

/// Comprueba que `logs` sea una carpeta real, sin crearla.
pub(crate) fn carpeta_valida(logs: &Path) -> Result<(), Causa> {
    match fs::symlink_metadata(logs) {
        Ok(m) if es_carpeta_real(&m) => Ok(()),
        Ok(_) => Err(NO_REGULAR),
        Err(error) => Err(causa_io(error)),
    }
}

/// Valida la carpeta que contiene `ruta` (`logs`) y la que contiene a esta (la
/// raíz de datos, que el arranque ya exige real). Detecta su sustitución por un
/// enlace o unión después de iniciar.
fn carpetas_validas(ruta: &Path) -> Result<(), Causa> {
    let mut carpeta = ruta.parent();
    for _ in 0..2 {
        let Some(actual) = carpeta.filter(|c| !c.as_os_str().is_empty()) else {
            break;
        };
        carpeta_valida(actual)?;
        carpeta = actual.parent();
    }
    Ok(())
}

/// Devuelve `raiz/logs`, creándola si falta; rechaza enlaces y archivos.
pub(crate) fn carpeta_logs(raiz: &Path) -> Result<PathBuf, Causa> {
    crear_carpeta_real(raiz)?;
    let logs = raiz.join("logs");
    match fs::symlink_metadata(&logs) {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => match fs::create_dir(&logs) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(causa_io(error)),
        },
        Err(error) => return Err(causa_io(error)),
    }
    carpeta_valida(&logs)?;
    Ok(logs)
}

/// Primer arranque: crea los padres ausentes y valida cada componente antes
/// de descender. No crea archivos a través de un enlace o unión preexistente.
fn crear_carpeta_real(ruta: &Path) -> Result<(), Causa> {
    match fs::symlink_metadata(ruta) {
        Ok(meta) if es_carpeta_real(&meta) => return Ok(()),
        Ok(_) => return Err(NO_REGULAR),
        Err(e) if e.kind() == io::ErrorKind::NotFound => (),
        Err(e) => return Err(causa_io(e)),
    }
    if let Some(padre) = ruta.parent().filter(|p| !p.as_os_str().is_empty()) {
        crear_carpeta_real(padre)?;
    }
    match fs::create_dir(ruta) {
        Ok(()) => (),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => (),
        Err(e) => return Err(causa_io(e)),
    }
    let meta = fs::symlink_metadata(ruta).map_err(causa_io)?;
    if es_carpeta_real(&meta) {
        Ok(())
    } else {
        Err(NO_REGULAR)
    }
}

/// `None` si la ruta no existe, su tamaño si es un archivo regular; error en otro caso.
pub(crate) fn tamano_regular(ruta: &Path) -> Result<Option<u64>, Causa> {
    match fs::symlink_metadata(ruta) {
        Ok(m) if es_regular(&m) => Ok(Some(m.len())),
        Ok(_) => Err(NO_REGULAR),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(causa_io(error)),
    }
}

#[cfg(windows)]
fn sin_seguir_enlaces(opciones: &mut OpenOptions) {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
    opciones.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
}

#[cfg(not(windows))]
fn sin_seguir_enlaces(_: &mut OpenOptions) {}

/// Abre `ruta` sin truncarla, validando antes y después la carpeta y que la
/// entrada sea un archivo regular. Un archivo existente se abre sin `create`; si
/// falta, se crea con `create_new`. Si otro proceso crea o borra la entrada entre
/// la comprobación y la apertura, se vuelve a validar y se reintenta (acotado).
fn abrir_validado(ruta: &Path, opciones: &OpenOptions) -> Result<File, Causa> {
    carpetas_validas(ruta)?;
    let mut existe = tamano_regular(ruta)?.is_some();
    let mut reintentos = 0;
    let archivo = loop {
        let mut intento = opciones.clone();
        intento.create(false).truncate(false).create_new(!existe);
        sin_seguir_enlaces(&mut intento);
        match intento.open(ruta) {
            Ok(archivo) => break archivo,
            Err(error)
                if reintentos < 2
                    && matches!(
                        (existe, error.kind()),
                        (false, io::ErrorKind::AlreadyExists) | (true, io::ErrorKind::NotFound)
                    ) =>
            {
                reintentos += 1;
                existe = tamano_regular(ruta)?.is_some();
            }
            Err(error) => return Err(causa_io(error)),
        }
    };
    let abierto = archivo.metadata().map_err(causa_io)?;
    if !es_regular(&abierto) {
        return Err(NO_REGULAR);
    }
    misma_entrada(ruta, &abierto)?;
    carpetas_validas(ruta)?;
    Ok(archivo)
}

#[cfg(unix)]
fn misma_entrada(ruta: &Path, abierto: &Metadata) -> Result<(), Causa> {
    use std::os::unix::fs::MetadataExt;
    let entrada = fs::symlink_metadata(ruta).map_err(causa_io)?;
    if !es_regular(&entrada) || entrada.ino() != abierto.ino() || entrada.dev() != abierto.dev() {
        return Err(NO_REGULAR);
    }
    Ok(())
}

#[cfg(not(unix))]
fn misma_entrada(_: &Path, _: &Metadata) -> Result<(), Causa> {
    Ok(())
}

/// Archivo de bloqueo entre procesos, separado del registro que se rota.
pub(crate) fn abrir_bloqueo(ruta: &Path) -> Result<File, Causa> {
    abrir_validado(ruta, OpenOptions::new().read(true).write(true))
}

/// Intenta el bloqueo exclusivo del sistema operativo hasta `plazo`, sin esperas infinitas.
pub(crate) fn bloquear(archivo: &File, plazo: Duration, pausa: Duration) -> Result<(), Causa> {
    let fin = Instant::now() + plazo;
    loop {
        match archivo.try_lock() {
            Ok(()) => return Ok(()),
            Err(TryLockError::WouldBlock) if Instant::now() < fin => thread::sleep(pausa),
            Err(TryLockError::WouldBlock) => return Err(BLOQUEO_OCUPADO),
            Err(TryLockError::Error(error)) => return Err(causa_io(error)),
        }
    }
}

/// Añade `bytes` al final de `ruta`; debe llamarse con el bloqueo entre procesos
/// tomado. No escribe si el archivo creció más allá de `tamano_previo` (lo que vio
/// el llamador al calcular la cota). Si un intento anterior dejó un fragmento sin
/// salto de línea, lo recorta antes. Si la escritura falla, o la carpeta deja de
/// ser válida después de escribir, recorta al tamaño de inicio; si ese recorte
/// también falla, el siguiente intento elimina el fragmento antes de anexar.
pub(crate) fn anexar(ruta: &Path, tamano_previo: u64, bytes: &[u8]) -> Result<(), Causa> {
    let mut archivo = abrir_anexo(ruta)?;
    let fin = archivo.stream_position().map_err(causa_io)?;
    if fin > tamano_previo {
        return Err(TAMANO_AJENO);
    }
    let inicio = descartar_fragmento(&mut archivo, fin)?;
    let resultado = match archivo.write_all(bytes).and_then(|()| archivo.flush()) {
        Ok(()) => carpetas_validas(ruta),
        Err(error) => Err(causa_io(error)),
    };
    if resultado.is_err() {
        let _ = archivo.set_len(inicio);
    }
    resultado
}

/// Abre para lectura y escritura (no `append`: en Windows un descriptor solo de
/// anexado no puede recortar) y se coloca al final. El llamador debe tener el
/// bloqueo entre procesos para que la posición siga siendo el final al escribir.
pub(crate) fn abrir_anexo(ruta: &Path) -> Result<File, Causa> {
    let mut archivo = abrir_validado(ruta, OpenOptions::new().read(true).write(true))?;
    archivo.seek(SeekFrom::End(0)).map_err(causa_io)?;
    Ok(archivo)
}

/// Si el archivo de tamaño `fin` no termina en salto de línea, lo recorta justo
/// después del último para no concatenar un fragmento con la línea siguiente.
/// Deja el cursor en el nuevo final y lo devuelve.
fn descartar_fragmento(archivo: &mut File, fin: u64) -> Result<u64, Causa> {
    let mut corte = fin;
    let mut bloque = [0u8; 4096];
    while corte > 0 {
        let largo = corte.min(bloque.len() as u64) as usize;
        let desde = corte - largo as u64;
        archivo.seek(SeekFrom::Start(desde)).map_err(causa_io)?;
        archivo.read_exact(&mut bloque[..largo]).map_err(causa_io)?;
        if let Some(i) = bloque[..largo].iter().rposition(|&b| b == b'\n') {
            corte = desde + i as u64 + 1;
            break;
        }
        corte = desde;
    }
    if corte < fin {
        archivo.set_len(corte).map_err(causa_io)?;
    }
    archivo.seek(SeekFrom::Start(corte)).map_err(causa_io)?;
    Ok(corte)
}

/// Ruta del respaldo `n` (`diagnostico.jsonl.1`, …).
pub(crate) fn respaldo(base: &Path, n: usize) -> PathBuf {
    let mut nombre = OsString::from(base.as_os_str());
    nombre.push(format!(".{n}"));
    PathBuf::from(nombre)
}

/// Rota `base → .1 → … → .respaldos`. Valida todas las rutas antes de mover nada:
/// una entrada no regular detiene la rotación sin tocarla. Si un renombrado falla,
/// devuelve el error y el llamador no debe seguir escribiendo en `base`. La
/// carpeta se revalida antes de cada borrado o renombrado y al terminar.
pub(crate) fn rotar(base: &Path, respaldos: usize) -> Result<(), Causa> {
    carpetas_validas(base)?;
    for n in 1..=respaldos {
        tamano_regular(&respaldo(base, n))?;
    }
    if respaldos == 0 {
        quitar_regular(base)?;
        return carpetas_validas(base);
    }
    quitar_regular(&respaldo(base, respaldos))?;
    for n in (1..respaldos).rev() {
        mover_regular(&respaldo(base, n), &respaldo(base, n + 1))?;
    }
    mover_regular(base, &respaldo(base, 1))?;
    carpetas_validas(base)
}

fn quitar_regular(ruta: &Path) -> Result<(), Causa> {
    carpetas_validas(ruta)?;
    if tamano_regular(ruta)?.is_some() {
        fs::remove_file(ruta).map_err(causa_io)?;
    }
    Ok(())
}

fn mover_regular(origen: &Path, destino: &Path) -> Result<(), Causa> {
    carpetas_validas(origen)?;
    if tamano_regular(origen)?.is_some() {
        fs::rename(origen, destino).map_err(causa_io)?;
    }
    Ok(())
}
