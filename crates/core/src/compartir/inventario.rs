// SPDX-License-Identifier: GPL-3.0-only
//! Lectura local conservadora, cancelable por bloques y sin publicación ni escritura.
use super::{comprobar_cancelacion, Error, Limites, RutaCompartida};
use sha2::{Digest, Sha256, Sha512};
use std::{collections::BTreeSet, fs, io::Read, path::Path, time::SystemTime};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArchivoInventario {
    pub ruta: RutaCompartida,
    pub bytes: u64,
    pub sha512: String,
    pub sha256: String,
}
#[derive(Debug, Clone, Default)]
pub struct Inventario {
    pub archivos: Vec<ArchivoInventario>,
    pub bytes: u64,
    /// Diagnósticos sin leer el contenido prohibido; no bloquean archivos ajenos.
    pub bloqueados: Vec<ArchivoBloqueado>,
}

#[derive(Debug, Clone)]
pub struct ArchivoBloqueado {
    pub ruta: String,
    pub motivo: String,
}

#[derive(PartialEq, Eq)]
struct Estado {
    bytes: u64,
    modificado: Option<SystemTime>,
    creado: Option<SystemTime>,
    #[cfg(unix)]
    nodo: (u64, u64),
}
impl Estado {
    fn de(meta: &fs::Metadata) -> Self {
        Self {
            bytes: meta.len(),
            modificado: meta.modified().ok(),
            creado: meta.created().ok(),
            #[cfg(unix)]
            nodo: {
                use std::os::unix::fs::MetadataExt;
                (meta.dev(), meta.ino())
            },
        }
    }
}

fn metadatos(path: &Path) -> Result<fs::Metadata, Error> {
    crate::almacenamiento::regular(path).map_err(|_| Error::Datos("archivo o carpeta no regular"))
}
fn omitido(nombre: &str) -> bool {
    let nombre = nombre.to_lowercase();
    nombre.starts_with(".icaro") || nombre == ".git" || nombre == ".papelera"
}

/// Debe ejecutarse con spawn_blocking al integrarse con el launcher.
/// La primera lectura siempre verifica el contenido completo: no confía en mtime/tamaño.
pub fn analizar(
    juego: &Path,
    cancel: &CancellationToken,
    limites: &Limites,
) -> Result<Inventario, Error> {
    comprobar_cancelacion(cancel)?;
    if !metadatos(juego)?.is_dir() {
        return Err(Error::Datos("carpeta de juego"));
    }
    let base = juego.canonicalize().map_err(|_| Error::Disco)?;
    let mut lector = Lector {
        base: &base,
        cancel,
        limites,
        salida: Inventario::default(),
        rutas: BTreeSet::new(),
        nodos: 0,
    };
    for carpeta in ["mods", "config", "kubejs", "resourcepacks"] {
        let path = base.join(carpeta);
        match fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(_) => return Err(Error::Disco),
            Ok(_) => lector.directorio(&path, carpeta, 0)?,
        }
    }
    comprobar_cancelacion(cancel)?;
    lector.salida.archivos.sort_by(|a, b| a.ruta.cmp(&b.ruta));
    Ok(lector.salida)
}

struct Lector<'a> {
    base: &'a Path,
    cancel: &'a CancellationToken,
    limites: &'a Limites,
    salida: Inventario,
    rutas: BTreeSet<String>,
    nodos: usize,
}
impl Lector<'_> {
    fn directorio(&mut self, path: &Path, relativa: &str, profundidad: usize) -> Result<(), Error> {
        comprobar_cancelacion(self.cancel)?;
        if profundidad > 30 {
            return Err(Error::Limite("profundidad de carpetas"));
        }
        if !metadatos(path)?.is_dir() {
            return Err(Error::Datos("carpeta compartida"));
        }
        for entry in fs::read_dir(path).map_err(|_| Error::Disco)? {
            comprobar_cancelacion(self.cancel)?;
            self.nodos += 1;
            if self.nodos > self.limites.archivos.saturating_mul(4).saturating_add(100) {
                return Err(Error::Limite("entradas del inventario"));
            }
            let entry = entry.map_err(|_| Error::Disco)?;
            let nombre = entry
                .file_name()
                .into_string()
                .map_err(|_| Error::Datos("nombre de archivo"))?;
            if omitido(&nombre) {
                continue;
            }
            let ruta = format!("{relativa}/{nombre}");
            if relativa == "mods" && !nombre.to_ascii_lowercase().ends_with(".jar") {
                continue;
            }
            let compartida = match RutaCompartida::parse(&ruta) {
                Ok(ruta) => ruta,
                Err(error) => {
                    self.salida.bloqueados.push(ArchivoBloqueado {
                        ruta,
                        motivo: error.to_string(),
                    });
                    continue;
                }
            };
            let meta = metadatos(&entry.path())?;
            if meta.is_dir() {
                // Los mods anidados no son mods activos del perfil y no se publican.
                if relativa == "mods" {
                    continue;
                }
                self.directorio(&entry.path(), &ruta, profundidad + 1)?;
            } else if meta.is_file() {
                if relativa == "mods" && !nombre.to_ascii_lowercase().ends_with(".jar") {
                    continue;
                }
                self.archivo(&entry.path(), compartida, &meta)?;
            } else {
                return Err(Error::Datos("tipo de archivo no compartible"));
            }
        }
        Ok(())
    }
    fn archivo(
        &mut self,
        path: &Path,
        ruta: RutaCompartida,
        meta: &fs::Metadata,
    ) -> Result<(), Error> {
        if self.salida.archivos.len() >= self.limites.archivos || !self.rutas.insert(ruta.clave()) {
            return Err(Error::Limite("archivos o rutas duplicadas"));
        }
        if meta.len() > self.limites.archivo_bytes {
            return Err(Error::Limite("tamaño por archivo"));
        }
        let total = self
            .salida
            .bytes
            .checked_add(meta.len())
            .ok_or(Error::Limite("tamaño total"))?;
        if total > self.limites.revision_bytes {
            return Err(Error::Limite("tamaño total"));
        }
        if !path
            .canonicalize()
            .map_err(|_| Error::Disco)?
            .starts_with(self.base)
        {
            return Err(Error::CambioExterno);
        }
        let mut opciones = fs::OpenOptions::new();
        opciones.read(true);
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            // Sin compartir escritura/borrado durante el hash, ni seguir un enlace final.
            opciones.share_mode(1).custom_flags(0x00200000);
        }
        let mut file = opciones.open(path).map_err(|_| Error::Disco)?;
        let inicio = file.metadata().map_err(|_| Error::Disco)?;
        if !inicio.is_file() || Estado::de(&inicio) != Estado::de(meta) {
            return Err(Error::CambioExterno);
        }
        let mut sha512 = Sha512::new();
        let mut sha256 = Sha256::new();
        let mut buf = [0u8; 64 * 1024];
        let mut bytes = 0u64;
        loop {
            comprobar_cancelacion(self.cancel)?;
            let n = file.read(&mut buf).map_err(|_| Error::Disco)?;
            if n == 0 {
                break;
            }
            bytes = bytes
                .checked_add(n as u64)
                .ok_or(Error::Limite("tamaño por archivo"))?;
            if bytes > self.limites.archivo_bytes || bytes > inicio.len() {
                return Err(Error::CambioExterno);
            }
            sha512.update(&buf[..n]);
            sha256.update(&buf[..n]);
        }
        if bytes != inicio.len()
            || Estado::de(&file.metadata().map_err(|_| Error::Disco)?) != Estado::de(&inicio)
            || Estado::de(&metadatos(path)?) != Estado::de(&inicio)
            || !path
                .canonicalize()
                .map_err(|_| Error::Disco)?
                .starts_with(self.base)
        {
            return Err(Error::CambioExterno);
        }
        self.salida.bytes = total;
        self.salida.archivos.push(ArchivoInventario {
            ruta,
            bytes,
            sha512: format!("{:x}", sha512.finalize()),
            sha256: format!("{:x}", sha256.finalize()),
        });
        Ok(())
    }
}
