// SPDX-License-Identifier: GPL-3.0-only
//! Revisiones locales materializadas. La firma y el transporte se añaden por separado.
use super::{
    comprobar_cancelacion, publicacion::Borrador, Artefacto, Error, Limites, Manifiesto, Origen,
    Perfil, RutaCompartida,
};
use sha2::{Digest, Sha512};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone)]
pub struct ContextoPublicacion {
    pub juego: PathBuf,
    /// Almacén exclusivo de esta publicación, fuera de la instancia.
    pub almacen: PathBuf,
    pub origen: Origen,
    pub perfil: Perfil,
    pub limites: Limites,
}
#[derive(Debug, Clone)]
pub struct Revision {
    pub manifiesto: Manifiesto,
    almacen: PathBuf,
}
impl Revision {
    /// Solo permite archivos enumerados por esta revisión, nunca rutas libres.
    pub fn archivo(&self, ruta: &RutaCompartida) -> Result<PathBuf, Error> {
        let indice = self
            .manifiesto
            .archivos
            .iter()
            .position(|a| &a.ruta == ruta)
            .ok_or(Error::Datos("archivo no publicado"))?;
        self.archivo_en(indice)
    }
    /// El transporte obtiene el índice de su instantánea de esta misma revisión.
    pub(crate) fn archivo_en(&self, indice: usize) -> Result<PathBuf, Error> {
        let artefacto = self.manifiesto.archivos.get(indice).ok_or(Error::Datos("archivo no publicado"))?;
        let path = self.almacen.join("contenido").join(&artefacto.sha512);
        let meta = crate::almacenamiento::regular(&path).map_err(|_| Error::Disco)?;
        if !meta.is_file() || meta.len() != artefacto.bytes {
            return Err(Error::CambioExterno);
        }
        Ok(path)
    }
}
pub struct RevisionPreparada {
    revision: Revision,
    _bloqueo: fs::File,
    limites: Limites,
    cancel: CancellationToken,
}

fn directorio(path: &Path) -> Result<(), Error> {
    fs::create_dir_all(path).map_err(|_| Error::Disco)?;
    if !crate::almacenamiento::regular(path)
        .map_err(|_| Error::Disco)?
        .is_dir()
    {
        return Err(Error::Datos("almacén de publicación"));
    }
    Ok(())
}
fn comprobar_contexto(ctx: &ContextoPublicacion) -> Result<(), Error> {
    ctx.origen.validar()?;
    ctx.perfil.validar()?;
    directorio(&ctx.almacen)?;
    let almacen = ctx.almacen.canonicalize().map_err(|_| Error::Disco)?;
    let juego = ctx.juego.canonicalize().map_err(|_| Error::Disco)?;
    if almacen.starts_with(&juego) || juego.starts_with(&almacen) {
        return Err(Error::Datos("almacén dentro de la instancia"));
    }
    Ok(())
}
pub fn vigente(ctx: &ContextoPublicacion) -> Result<Option<Revision>, Error> {
    let path = ctx.almacen.join("vigente.json");
    match fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(Error::Disco),
        Ok(_) => (),
    }
    let meta =
        crate::almacenamiento::regular(&path).map_err(|_| Error::Datos("revisión vigente"))?;
    if !meta.is_file() || meta.len() > ctx.limites.manifiesto_bytes as u64 {
        return Err(Error::Limite("manifiesto"));
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|_| Error::Disco)?
        .take(ctx.limites.manifiesto_bytes as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Disco)?;
    let manifiesto = Manifiesto::leer(&bytes, &ctx.limites)?;
    if manifiesto.origen != ctx.origen || manifiesto.perfil != ctx.perfil {
        return Err(Error::Datos("origen o perfil de la revisión vigente"));
    }
    Ok(Some(Revision {
        manifiesto,
        almacen: ctx.almacen.clone(),
    }))
}

pub fn preparar_revision(
    ctx: &ContextoPublicacion,
    borrador: &Borrador,
    cancel: &CancellationToken,
) -> Result<RevisionPreparada, Error> {
    comprobar_cancelacion(cancel)?;
    comprobar_contexto(ctx)?;
    let bloqueo =
        crate::mods::guardia(&ctx.almacen).map_err(|_| Error::Datos("publicación ocupada"))?;
    let anterior = vigente(ctx)?;
    if anterior.as_ref().map(|r| &r.manifiesto) != borrador.base.as_ref() {
        return Err(Error::CambioExterno);
    }
    if !borrador.autorizacion_pendiente().is_empty() {
        return Err(Error::Datos("autorización de archivos propios pendiente"));
    }
    let numero = anterior.as_ref().map_or(Ok(1), |r| {
        r.manifiesto
            .revision
            .checked_add(1)
            .ok_or(Error::Limite("número de revisión"))
    })?;
    let manifiesto = borrador.manifiesto(ctx.origen.clone(), ctx.perfil.clone(), numero);
    manifiesto.validar(&ctx.limites)?;
    if let Some(base) = &borrador.base {
        if base.archivos == manifiesto.archivos && base.packs == manifiesto.packs {
            return Err(Error::Datos("no hay cambios para publicar"));
        }
    } else if manifiesto.archivos.is_empty()
        && manifiesto.packs.is_empty()
        && ctx.perfil.loader != "vanilla"
    {
        // Una instancia Vanilla comparte su versión de Minecraft aunque no
        // tenga archivos; con un loader, una publicación vacía es un descuido.
        return Err(Error::Datos("no hay contenido seleccionado"));
    }
    let contenido = ctx.almacen.join("contenido");
    super::espacio::comprobar(&ctx.almacen, &manifiesto.archivos.iter().map(|a| a.bytes).collect::<Vec<_>>())?;
    directorio(&contenido)?;
    for archivo in &manifiesto.archivos {
        comprobar_cancelacion(cancel)?;
        let fuente = if borrador.usa_base(archivo) {
            anterior
                .as_ref()
                .ok_or(Error::CambioExterno)?
                .archivo(&archivo.ruta)?
        } else {
            fuente_local(&ctx.juego, &archivo.ruta)?
        };
        copiar_verificado(&fuente, &contenido, archivo, cancel)?;
    }
    comprobar_cancelacion(cancel)?;
    Ok(RevisionPreparada {
        revision: Revision {
            manifiesto,
            almacen: ctx.almacen.clone(),
        },
        _bloqueo: bloqueo,
        limites: ctx.limites.clone(),
        cancel: cancel.clone(),
    })
}

fn fuente_local(juego: &Path, ruta: &RutaCompartida) -> Result<PathBuf, Error> {
    let base = juego.canonicalize().map_err(|_| Error::Disco)?;
    let mut path = base.clone();
    for parte in ruta.as_str().split('/') {
        path.push(parte);
        crate::almacenamiento::regular(&path)
            .map_err(|_| Error::Datos("archivo o carpeta no regular"))?;
    }
    if !path
        .canonicalize()
        .map_err(|_| Error::Disco)?
        .starts_with(&base)
    {
        return Err(Error::CambioExterno);
    }
    Ok(path)
}
fn abrir_lectura(path: &Path) -> Result<fs::File, Error> {
    let mut opciones = fs::OpenOptions::new();
    opciones.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        opciones.share_mode(1).custom_flags(0x00200000);
    }
    opciones.open(path).map_err(|_| Error::Disco)
}
fn copiar_verificado(
    fuente: &Path,
    contenido: &Path,
    archivo: &Artefacto,
    cancel: &CancellationToken,
) -> Result<(), Error> {
    let destino = contenido.join(&archivo.sha512);
    // Incluso al reutilizar contenido se lee y verifica el origen previsto.
    let mut entrada = abrir_lectura(fuente)?;
    let meta = entrada.metadata().map_err(|_| Error::Disco)?;
    if !meta.is_file() || meta.len() != archivo.bytes {
        return Err(Error::CambioExterno);
    }
    let mut nonce = [0u8; 16];
    getrandom::fill(&mut nonce).map_err(|_| Error::Disco)?;
    let nombre: String = nonce.iter().map(|b| format!("{b:02x}")).collect();
    let temp = contenido.join(format!(".icaro-{nombre}.part"));
    let resultado = (|| {
        let mut salida = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)
            .map_err(|_| Error::Disco)?;
        let mut hash = Sha512::new();
        let mut bytes = 0u64;
        let mut buf = [0u8; 64 * 1024];
        loop {
            comprobar_cancelacion(cancel)?;
            let n = entrada.read(&mut buf).map_err(|_| Error::Disco)?;
            if n == 0 {
                break;
            }
            bytes = bytes.checked_add(n as u64).ok_or(Error::CambioExterno)?;
            if bytes > archivo.bytes {
                return Err(Error::CambioExterno);
            }
            hash.update(&buf[..n]);
            salida.write_all(&buf[..n]).map_err(|_| Error::Disco)?;
        }
        if bytes != archivo.bytes || format!("{:x}", hash.finalize()) != archivo.sha512 {
            return Err(Error::CambioExterno);
        }
        salida.sync_all().map_err(|_| Error::Disco)?;
        drop(salida);
        drop(entrada);
        if destino == fuente {
            // El origen ya es el objeto conservado de una revisión pospuesta.
            fs::remove_file(&temp).map_err(|_| Error::Disco)?;
        } else {
            // Renombrar publica una copia independiente; nunca se usan enlaces duros.
            if destino.exists() {
                crate::almacenamiento::regular(&destino)
                    .map_err(|_| Error::Datos("contenido no regular"))?;
            }
            fs::rename(&temp, &destino).map_err(|_| Error::Disco)?;
        }
        Ok(())
    })();
    if resultado.is_err() {
        let _ = fs::remove_file(&temp);
    }
    resultado
}

pub fn publicar_revision(preparada: RevisionPreparada) -> Result<Revision, Error> {
    comprobar_cancelacion(&preparada.cancel)?;
    preparada.revision.manifiesto.validar(&preparada.limites)?;
    for archivo in &preparada.revision.manifiesto.archivos {
        let path = preparada.revision.archivo(&archivo.ruta)?;
        verificar_contenido(&path, archivo, &preparada.cancel)?;
    }
    let revisiones = preparada.revision.almacen.join("revisiones");
    directorio(&revisiones)?;
    guardar_manifiesto(
        &revisiones.join(format!("{}.json", preparada.revision.manifiesto.revision)),
        &preparada.revision.manifiesto,
    )?;
    // El puntero vigente es el último cambio: una preparación fallida no lo toca.
    comprobar_cancelacion(&preparada.cancel)?;
    guardar_manifiesto(
        &preparada.revision.almacen.join("vigente.json"),
        &preparada.revision.manifiesto,
    )?;
    Ok(preparada.revision)
}

fn verificar_contenido(
    path: &Path,
    archivo: &Artefacto,
    cancel: &CancellationToken,
) -> Result<(), Error> {
    let mut entrada = abrir_lectura(path)?;
    let mut hash = Sha512::new();
    let mut bytes = 0u64;
    let mut buf = [0u8; 64 * 1024];
    loop {
        comprobar_cancelacion(cancel)?;
        let n = entrada.read(&mut buf).map_err(|_| Error::Disco)?;
        if n == 0 {
            break;
        }
        bytes = bytes.checked_add(n as u64).ok_or(Error::CambioExterno)?;
        if bytes > archivo.bytes {
            return Err(Error::CambioExterno);
        }
        hash.update(&buf[..n]);
    }
    if bytes != archivo.bytes || format!("{:x}", hash.finalize()) != archivo.sha512 {
        return Err(Error::CambioExterno);
    }
    Ok(())
}

/// Persiste exactamente la representación compacta que valida el protocolo.
fn guardar_manifiesto(path: &Path, manifiesto: &Manifiesto) -> Result<(), Error> {
    let parent = path.parent().ok_or(Error::Datos("destino de manifiesto"))?;
    let bytes = serde_json::to_vec(manifiesto).map_err(|_| Error::Datos("manifiesto"))?;
    let mut nonce = [0u8; 16];
    getrandom::fill(&mut nonce).map_err(|_| Error::Disco)?;
    let nombre: String = nonce.iter().map(|b| format!("{b:02x}")).collect();
    let temporal = parent.join(format!(".icaro-{nombre}.tmp"));
    let resultado = (|| {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporal)
            .map_err(|_| Error::Disco)?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|_| Error::Disco)?;
        drop(file);
        fs::rename(&temporal, path).map_err(|_| Error::Disco)
    })();
    if resultado.is_err() {
        let _ = fs::remove_file(&temporal);
    }
    resultado
}
