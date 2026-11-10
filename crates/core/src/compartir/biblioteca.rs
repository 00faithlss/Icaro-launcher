// SPDX-License-Identifier: GPL-3.0-only
//! Publicaciones propias ya persistidas, leídas sin efectos. Nunca crea origen,
//! identidad, revisión, carpetas ni accesos; nunca adopta publicaciones ajenas.
use super::{
    accesos,
    identidad::{AlmacenClave, Identidad},
    manifiesto::hexadecimal,
    revisiones::ContextoPublicacion,
    vinculos, Error, Limites, Manifiesto, Origen,
};
use crate::instancias::{Almacen, Instancia};
use std::{fs, io::Read, path::Path};

/// Precedencia: `Ilegible > Detenida > PerfilCambiado > SinRevision > Lista`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EstadoGuardada {
    Lista,
    SinRevision,
    Detenida,
    PerfilCambiado,
    Ilegible,
}
/// Solo identificadores y números: ni nombres de instancia ni rutas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicacionGuardada {
    pub instancia: String,
    /// `None` si el registro de publicación es ilegible.
    pub origen: Option<Origen>,
    pub revision: Option<u64>,
    pub archivos: usize,
    /// Permitidos que no están revocados.
    pub autorizados: usize,
    pub estado: EstadoGuardada,
}

fn anfitrion_valido(anfitrion: &str) -> Result<(), Error> {
    if hexadecimal(anfitrion, 64) {
        Ok(())
    } else {
        Err(Error::Datos("identidad del anfitrión"))
    }
}

/// Ejecutar fuera del hilo de iced. Un registro dañado no impide listar las demás.
pub fn listar(
    raiz: &Path,
    store: &Almacen,
    anfitrion: &str,
) -> Result<Vec<PublicacionGuardada>, Error> {
    anfitrion_valido(anfitrion)?;
    let informe = store.cargar().map_err(|_| Error::Disco)?;
    let mut salida = Vec::new();
    for instancia in &informe.instancias {
        let Ok(juego) = store.juego(&instancia.id) else {
            continue;
        };
        match fs::symlink_metadata(juego.join(vinculos::PUBLICACION)) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            _ => (),
        }
        // Un destino de invitado nunca es una publicación propia.
        if matches!(vinculos::leer(&juego), Ok(Some(_))) {
            continue;
        }
        let origen = match vinculos::origen_guardado(&juego) {
            Ok(Some(origen)) if origen.anfitrion == anfitrion => origen,
            Ok(_) => continue,
            Err(_) => {
                salida.push(PublicacionGuardada {
                    instancia: instancia.id.clone(),
                    origen: None,
                    revision: None,
                    archivos: 0,
                    autorizados: 0,
                    estado: EstadoGuardada::Ilegible,
                });
                continue;
            }
        };
        salida.push(describir(raiz, store, instancia, juego, origen));
    }
    Ok(salida)
}

fn describir(
    raiz: &Path,
    store: &Almacen,
    instancia: &Instancia,
    juego: std::path::PathBuf,
    origen: Origen,
) -> PublicacionGuardada {
    let ctx = vinculos::contexto(raiz, juego, origen, vinculos::perfil_de(store, instancia));
    let vigente = leer_vigente(&ctx.almacen, &ctx.limites);
    let resumen = accesos::resumen(&ctx.almacen.join("accesos.json"));
    let manifiesto = vigente.as_ref().ok().and_then(Option::as_ref);
    let estado = match (&vigente, &resumen) {
        (Err(_), _) | (_, Err(_)) => EstadoGuardada::Ilegible,
        (_, Ok(r)) if r.detenida => EstadoGuardada::Detenida,
        (Ok(Some(m)), _) if m.origen != ctx.origen || m.perfil != ctx.perfil => {
            EstadoGuardada::PerfilCambiado
        }
        (Ok(None), _) => EstadoGuardada::SinRevision,
        (Ok(Some(_)), _) => EstadoGuardada::Lista,
    };
    PublicacionGuardada {
        instancia: instancia.id.clone(),
        revision: manifiesto.map(|m| m.revision),
        archivos: manifiesto.map_or(0, |m| m.archivos.len()),
        autorizados: resumen.map_or(0, |r| r.autorizados),
        origen: Some(ctx.origen),
        estado,
    }
}

/// Como `revisiones::vigente`, pero sin exigir el perfil actual: así se distingue
/// una revisión de otro perfil de un archivo dañado.
fn leer_vigente(almacen: &Path, limites: &Limites) -> Result<Option<Manifiesto>, Error> {
    let path = almacen.join("vigente.json");
    match fs::symlink_metadata(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(_) => return Err(Error::Disco),
        Ok(_) => (),
    }
    let meta =
        crate::almacenamiento::regular(&path).map_err(|_| Error::Datos("revisión vigente"))?;
    if !meta.is_file() || meta.len() > limites.manifiesto_bytes as u64 {
        return Err(Error::Limite("manifiesto"));
    }
    let mut bytes = Vec::new();
    fs::File::open(&path)
        .map_err(|_| Error::Disco)?
        .take(limites.manifiesto_bytes as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Error::Disco)?;
    Manifiesto::leer(&bytes, limites).map(Some)
}

/// Igual que `vinculos::publicacion`, pero falla si no hay origen guardado: nunca
/// genera otra publicación. Ejecutar fuera del hilo de iced.
pub fn abrir(
    raiz: &Path,
    store: &Almacen,
    instancia: &Instancia,
    anfitrion: &str,
) -> Result<ContextoPublicacion, Error> {
    anfitrion_valido(anfitrion)?;
    let perfil = vinculos::perfil_de(store, instancia);
    perfil.validar()?;
    let juego = store.juego(&instancia.id).map_err(|_| Error::Disco)?;
    if vinculos::leer(&juego)?.is_some() {
        return Err(Error::Datos("la instancia es un destino de invitado"));
    }
    let origen = vinculos::origen_guardado(&juego)?
        .ok_or(Error::Datos("la instancia no tiene una publicación guardada"))?;
    if origen.anfitrion != anfitrion {
        return Err(Error::Datos("la publicación pertenece a otra identidad"));
    }
    Ok(vinculos::contexto(raiz, juego, origen, perfil))
}

/// Quita una publicación propia de la biblioteca: borra la marca de
/// publicación de la instancia y su copia de revisiones y accesos. No toca
/// mods, mundos ni ajustes. Solo para publicaciones sin nodo activo (en pausa
/// o detenidas). Ejecutar fuera del hilo de iced.
pub fn eliminar(
    raiz: &Path,
    store: &Almacen,
    instancia: &Instancia,
    anfitrion: &str,
) -> Result<(), Error> {
    anfitrion_valido(anfitrion)?;
    let juego = store.juego(&instancia.id).map_err(|_| Error::Disco)?;
    let _bloqueo = crate::mods::guardia(&juego).map_err(|_| Error::Datos("instancia ocupada"))?;
    let origen = vinculos::origen_guardado(&juego)?
        .ok_or(Error::Datos("la instancia no tiene una publicación guardada"))?;
    if origen.anfitrion != anfitrion {
        return Err(Error::Datos("la publicación pertenece a otra identidad"));
    }
    // El identificador forma parte de una ruta: debe ser hexadecimal.
    if origen.publicacion.is_empty()
        || !hexadecimal(&origen.publicacion, origen.publicacion.len())
    {
        return Err(Error::Datos("identificador de publicación"));
    }
    let marca = juego.join(vinculos::PUBLICACION);
    crate::almacenamiento::regular(&marca).map_err(|_| Error::Datos("marca de publicación"))?;
    fs::remove_file(&marca).map_err(|_| Error::Disco)?;
    let carpeta = raiz.join("compartir/publicaciones").join(&origen.publicacion);
    if !carpeta.exists() {
        return Ok(());
    }
    // Nunca sigue enlaces ni uniones fuera de la carpeta de Ícaro.
    match crate::almacenamiento::regular(&carpeta) {
        Ok(m) if m.is_dir() => fs::remove_dir_all(&carpeta).map_err(|_| Error::Disco),
        _ => Err(Error::Datos("carpeta de publicación")),
    }
}

/// Recupera una identidad existente; sin clave guardada devuelve `None` y no crea otra.
pub fn identidad_existente(almacen: &impl AlmacenClave) -> Result<Option<Identidad>, Error> {
    Identidad::existente(almacen)
}
