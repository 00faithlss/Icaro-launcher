// SPDX-License-Identifier: GPL-3.0-only
use super::tareas::{PublicacionLocal, Trabajo};
use icaro_core::compartir::anfitrion::PublicacionActiva;
use icaro_ui::compartir as v;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Etapa {
    #[default]
    Cerrada,
    Selector,
    Analizando,
    Revision,
    Propios,
    /// CompartidoListo, justo después de publicar.
    Listo,
    Publicando,
    Guardando,
    Panel,
    Cierre,
    Entrada,
    Identidad,
    Destino,
    RevisionInvitado,
    InstalandoInvitado,
    ResultadoInvitado,
    ConsultandoVinculo,
    RevisionVinculo,
    AplicandoVinculo,
    ErrorVinculo,
}
pub struct Estado {
    pub biblioteca: super::biblioteca::Biblioteca,
    pub sincronizar: super::sincronizar::Estado,
    pub invitado: super::invitado::Invitado,
    pub habilitado: bool,
    pub activado_por_usuario: bool,
    pub etapa: Etapa,
    pub solicitud: u64,
    pub seleccionada: Option<String>,
    pub cancel: CancellationToken,
    pub cerrar_al_terminar: bool,
    pub red_ocupada: bool,
    pub permiso_propios: bool,
    pub trabajo: Option<Trabajo>,
    pub publicacion: Option<PublicacionLocal>,
    pub revision: Option<v::RevisionPublicacion>,
    pub panel: Option<v::PanelPublicacion>,
    pub anfitrion: Option<Arc<PublicacionActiva>>,
    pub error: Option<String>,
    pub caducidad: v::Caducidad,
    pub cupo: u32,
    pub opcion_cierre: v::OpcionCierre,
    pub codigo_entrada: String,
    /// Servidores › Instancias compartidas abierta; sobrevive a los flujos.
    pub compartidas: Option<v::PestanaCompartidas>,
    pub menu_compartida: Option<String>,
    /// Publicación en pausa que pide confirmar «Quitar de compartidas».
    pub confirmar_quitar: Option<String>,
    pub recibidas: Vec<super::compartidas::Recibida>,
    pub recibidas_generacion: u64,
    pub copiado_de: Option<String>,
    pub selector: super::selector::Selector,
    pub listo: super::listo::Listo,
}
impl Default for Estado {
    fn default() -> Self {
        Self {
            biblioteca: Default::default(),
            sincronizar: Default::default(),
            invitado: Default::default(),
            habilitado: std::env::var("ICARO_COMPARTIR_DESARROLLO").as_deref() == Ok("1"),
            activado_por_usuario: false,
            etapa: Etapa::Cerrada,
            solicitud: 0,
            seleccionada: None,
            cancel: CancellationToken::new(),
            cerrar_al_terminar: false,
            red_ocupada: false,
            permiso_propios: false,
            trabajo: None,
            publicacion: None,
            revision: None,
            panel: None,
            anfitrion: None,
            error: None,
            caducidad: Default::default(),
            cupo: 10,
            opcion_cierre: Default::default(),
            codigo_entrada: String::new(),
            compartidas: None,
            menu_compartida: None,
            confirmar_quitar: None,
            recibidas: Vec::new(),
            recibidas_generacion: 0,
            copiado_de: None,
            selector: Default::default(),
            listo: Default::default(),
        }
    }
}
impl Estado {
    pub fn comenzar(&mut self, instancia: String) -> u64 {
        if self.etapa == Etapa::Publicando || self.red_ocupada || self.etapa == Etapa::Guardando {
            return self.solicitud;
        }
        self.cancel.cancel();
        self.cancel = CancellationToken::new();
        self.solicitud = self.solicitud.wrapping_add(1);
        self.seleccionada = Some(instancia);
        self.trabajo = None;
        self.revision = None;
        self.error = None;
        self.permiso_propios = false;
        self.invitado = Default::default();
        self.cerrar_al_terminar = false;
        self.etapa = Etapa::Analizando;
        self.solicitud
    }
    pub fn vigente(&self, id: u64) -> bool {
        self.solicitud == id && self.etapa != Etapa::Cerrada
    }
    pub fn cerrar(&mut self) {
        self.cancel.cancel();
        if self.etapa == Etapa::Publicando || self.red_ocupada || self.etapa == Etapa::Guardando {
            self.cerrar_al_terminar = true;
            return;
        }
        self.solicitud = self.solicitud.wrapping_add(1);
        self.etapa = Etapa::Cerrada;
        self.biblioteca.cancel_contadores.cancel();
        self.sincronizar = Default::default();
        self.invitado = Default::default();
        self.trabajo = None;
        self.revision = None;
        self.error = None;
        self.permiso_propios = false;
    }
    pub fn modal(&self) -> bool {
        self.etapa != Etapa::Cerrada
    }
    /// Hay una fase que mostrar en Servidores. Consultar si una instancia
    /// está vinculada a una compartida (al pulsar Jugar) no lo es: casi
    /// siempre termina en jugar sin nada que revisar.
    pub fn visible(&self) -> bool {
        !matches!(self.etapa, Etapa::Cerrada | Etapa::ConsultandoVinculo)
    }
}
impl Drop for Estado {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}
