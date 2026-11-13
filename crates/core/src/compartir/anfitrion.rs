// SPDX-License-Identifier: GPL-3.0-only
//! Publicación activa: une revisiones inmutables, autorizaciones y transporte.
use super::{
    accesos::Accesos,
    identidad::Identidad,
    nombres::Nombres,
    invitaciones::{
        AutoridadServicio, Caducidad, ClienteInvitaciones, Codigo, EncuentroFirmado,
        InvitacionCreada,
    },
    revisiones::Revision,
    transporte::{Cliente, Modo, NodoAnfitrion},
    Error, Limites, Origen,
};
use std::{
    path::Path,
    sync::{Arc, RwLock},
};

pub struct PublicacionActiva {
    nodo: NodoAnfitrion,
    identidad: Identidad,
    servicio: ClienteInvitaciones,
    revision: Arc<RwLock<Revision>>,
    origen: Origen,
    accesos: Arc<Accesos>,
    nombres: Arc<Nombres>,
}
impl std::fmt::Debug for PublicacionActiva {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PublicacionActiva")
            .field("origen", &self.origen)
            .finish_non_exhaustive()
    }
}
impl PublicacionActiva {
    pub async fn iniciar(
        identidad: Identidad,
        revision: Revision,
        archivo_accesos: &Path,
        url: &str,
        modo: Modo,
        limites: Limites,
    ) -> Result<Self, Error> {
        let servicio = ClienteInvitaciones::nuevo(url, identidad.clone())?;
        let origen = revision.manifiesto.origen.clone();
        let archivo_nombres = archivo_accesos.with_file_name("nombres.json");
        let nombres = Arc::new(
            tokio::task::spawn_blocking(move || Nombres::abrir(archivo_nombres))
                .await
                .map_err(|_| Error::Disco)?,
        );
        let archivo_accesos = archivo_accesos.to_path_buf();
        let accesos = Arc::new(
            tokio::task::spawn_blocking(move || Accesos::abrir(archivo_accesos))
                .await
                .map_err(|_| Error::Disco)??,
        );
        let autoridad = Arc::new(AutoridadServicio::nueva(
            servicio.clone(),
            origen.publicacion.clone(),
            accesos.clone(),
        )?);
        let revision = Arc::new(RwLock::new(revision));
        let nodo = NodoAnfitrion::iniciar(
            identidad.clone(),
            revision.clone(),
            accesos.clone(),
            autoridad,
            modo,
            limites,
        )
        .await?;
        nodo.guardar_nombres(nombres.clone());
        Ok(Self {
            nodo,
            identidad,
            servicio,
            revision,
            origen,
            accesos,
            nombres,
        })
    }
    pub fn encuentro(&self) -> Result<EncuentroFirmado, Error> {
        EncuentroFirmado::desde_direccion(
            &self.identidad,
            &self.origen.publicacion,
            &self.nodo.direccion(),
        )
    }
    pub async fn invitar(
        &self,
        caducidad: Caducidad,
        cupo: u32,
    ) -> Result<InvitacionCreada, Error> {
        self.servicio
            .nueva(
                &self.origen.publicacion,
                &self.encuentro()?,
                caducidad,
                cupo,
            )
            .await
    }
    pub async fn revocar_codigo(&self, codigo: &Codigo) -> Result<(), Error> {
        self.servicio.revocar_codigo(codigo).await
    }
    pub async fn revocar(&self, invitado: &str) -> Result<(), Error> {
        // Si falla el servicio, este equipo ya ha cortado el acceso y no lo restaura.
        self.nodo.revocar(invitado).await?;
        self.servicio
            .revocar_acceso(&self.origen.publicacion, invitado)
            .await
    }
    pub async fn accesos(&self) -> Result<Vec<String>, Error> {
        self.servicio.autorizados(&self.origen.publicacion).await
    }
    pub fn actualizar_revision(&self, nueva: Revision) -> Result<(), Error> {
        let mut actual = self.revision.write().map_err(|_| Error::Disco)?;
        nueva.manifiesto.validar(&Limites::default())?;
        if nueva.manifiesto.origen != self.origen
            || nueva.manifiesto.perfil != actual.manifiesto.perfil
            || nueva.manifiesto.revision <= actual.manifiesto.revision
        {
            return Err(Error::Datos("revisión de otra publicación o anterior"));
        }
        *actual = nueva;
        Ok(())
    }
    /// Nombre de jugador con que se presentó cada equipo, por su clave.
    pub fn nombres(&self) -> std::collections::BTreeMap<String, String> {
        self.nombres.todos()
    }
    /// Equipos invitados con una conexión abierta ahora.
    pub fn conectados(&self) -> std::collections::BTreeSet<String> {
        self.nodo.conectados()
    }
    pub fn transferencias(&self) -> usize {
        self.nodo.contar_transferencias()
    }
    pub fn transferencias_activas(&self) -> Vec<super::transporte::TransferenciaActiva> {
        self.nodo.transferencias_activas()
    }
    pub async fn drenar(&self) -> Result<(), Error> {
        self.nodo.drenar().await
    }
    /// Cerrar Ícaro conserva autorizaciones; dejar de compartir las revoca.
    pub async fn cerrar(&self) {
        self.nodo.parar().await;
    }
    pub async fn detener(&self) -> Result<(), Error> {
        let accesos = self.accesos.clone();
        let guardado = tokio::task::spawn_blocking(move || accesos.detener())
            .await
            .map_err(|_| Error::Disco);
        self.nodo.parar().await;
        guardado??;
        self.servicio.detener(&self.origen.publicacion).await
    }
    /// Sobre un nodo recién iniciado tras detener: crea un código nuevo y solo
    /// entonces limpia permisos locales antiguos. El servicio exige otra aceptación.
    pub async fn volver_a_compartir(
        &self,
        caducidad: Caducidad,
        cupo: u32,
    ) -> Result<InvitacionCreada, Error> {
        // Reintentar primero la revocación remota, incluso si el cierre anterior
        // ocurrió sin red. Crear otro código sobre una publicación aún activa
        // conservaría sus accesos anteriores.
        self.servicio.detener(&self.origen.publicacion).await?;
        let invitacion = self.invitar(caducidad, cupo).await?;
        let accesos = self.accesos.clone();
        tokio::task::spawn_blocking(move || accesos.reiniciar())
            .await
            .map_err(|_| Error::Disco)??;
        Ok(invitacion)
    }
    pub fn origen(&self) -> &Origen {
        &self.origen
    }
    /// Publica (o deja de publicar) el servidor local por el túnel de invitados.
    pub fn abrir_servidor(&self, puerto: Option<u16>) {
        match puerto {
            Some(p) => super::transporte::servidor::abrir(&self.origen.publicacion, p),
            None => super::transporte::servidor::cerrar(&self.origen.publicacion),
        }
    }
    pub async fn conectar_servidor(
        &self,
        encuentro: &EncuentroFirmado,
        origen: &Origen,
    ) -> Result<super::transporte::servidor::Conexion, Error> {
        self.nodo
            .conectar_servidor(
                encuentro.direccion(&origen.anfitrion, &origen.publicacion)?,
                origen,
            )
            .await
    }
    /// Comparte el endpoint de este equipo para no abrir otra identidad simultánea.
    pub async fn conectar(
        &self,
        encuentro: &EncuentroFirmado,
        origen: &Origen,
    ) -> Result<Cliente, Error> {
        self.nodo
            .conectar(
                encuentro.direccion(&origen.anfitrion, &origen.publicacion)?,
                origen,
            )
            .await
    }
}
