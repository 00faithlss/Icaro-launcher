// SPDX-License-Identifier: GPL-3.0-only
// Copyright (C) 2026 00faithlss

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Accion {
    Cerrar,
    Cancelar,
    Atras,
    Continuar,
    Compartir,
    ElegirInstancia(String),
    IncluirArchivo(String),
    ExcluirArchivo(String),
    VerIncluidos,
    ElegirCarpetas,
    AbrirCarpeta(String),
    RevisarPropios,
    PermisoPropios(bool),
    AutorizarPropios,
    ExcluirPropios,
    CopiarCodigo,
    GenerarInvitacion,
    RevocarCodigo,
    Caducidad(Caducidad),
    Cupo(u32),
    RevisarPublicacion,
    VerPublicacion,
    CopiarHuella,
    ConfirmarRevocacion(String),
    CancelarRevocacion,
    RevocarAcceso(String),
    DejarDeCompartir,
    VolverACompartir,
    OpcionCierre(OpcionCierre),
    ConfirmarCierre,
    Codigo(String),
    PegarCodigo,
    ConfiarAnfitrion,
    ConfirmarHuella(bool),
    NombreDestino(String),
    RevisarInstalacion,
    EleccionPublicacion(String, Eleccion),
    PublicarRevision,
    ConfirmarCancelarPublicacion,
    SeguirRevisando,
    CerrarSinPublicar,
    ResolverConflicto(String, Resolucion),
    Sincronizar,
    Jugar,
    VerCambios,
    Reintentar,
    VerDetalle,
    Esperar,
    ElegirUnidad,
    Desvincular,
    AbrirCompartir,
    AceptarInvitacion,
    AgregarServidor,
}

#[derive(Debug, Clone, Default)]
pub struct InstanciaCompartida {
    pub id: String,
    pub nombre: String,
    pub minecraft: String,
    pub loader: String,
    pub mods: usize,
    pub en_uso: bool,
}

#[derive(Debug, Clone, Default)]
pub struct ArchivoCompartido {
    pub id: String,
    pub nombre: String,
    pub detalle: String,
    pub tamano: String,
    pub procedencia: String,
    pub sha256: String,
    pub carpeta: String,
}

#[derive(Debug, Clone)]
pub enum EstadoContenido {
    Analizando { hechos: u64, total: u64 },
    Listo,
    Bloqueado,
    Preparando { revision: u64, porcentaje: f32 },
}
#[derive(Debug, Clone)]
pub struct CompartirInstancia {
    pub instancias: Vec<InstanciaCompartida>,
    pub seleccionada: Option<String>,
    pub estado: EstadoContenido,
    pub incluidos: Vec<ArchivoCompartido>,
    pub total_incluidos: usize,
    pub excluidos: Vec<ArchivoCompartido>,
    pub propios: Vec<ArchivoCompartido>,
    pub carpetas: String,
    pub tamano_carpetas: String,
    pub problemas: Vec<Problema>,
}
#[derive(Debug, Clone)]
pub struct ConfirmarPropios {
    pub instancia: String,
    pub revision: u64,
    pub archivos: Vec<ArchivoCompartido>,
    pub autorizado: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Caducidad {
    #[default]
    Horas24,
    Dias2,
    Semana,
}
impl Caducidad {
    pub const TODAS: [Self; 3] = [Self::Horas24, Self::Dias2, Self::Semana];
    pub fn texto(self) -> &'static str {
        match self {
            Self::Horas24 => "24 horas",
            Self::Dias2 => "2 días",
            Self::Semana => "1 semana",
        }
    }
}
#[derive(Debug, Clone)]
pub enum EstadoInvitacion {
    Generando,
    Vigente { restante: String },
    Completa,
    Vencida { fecha: String },
    Revocada,
}
#[derive(Debug, Clone)]
pub struct Invitacion {
    pub codigo: String,
    pub estado: EstadoInvitacion,
    pub caducidad: Caducidad,
    pub cupo: u32,
    pub aceptados: u32,
    pub copiado: bool,
    /// Capacidad validada por el servicio; no se deduce del número de aceptados.
    pub ajustes_editables: bool,
}
impl Default for Invitacion {
    fn default() -> Self {
        Self {
            codigo: String::new(),
            estado: EstadoInvitacion::Generando,
            caducidad: Caducidad::Horas24,
            cupo: 10,
            aceptados: 0,
            copiado: false,
            ajustes_editables: true,
        }
    }
}
#[derive(Debug, Clone)]
pub struct Acceso {
    pub id: String,
    pub nombre: String,
    pub huella: String,
    pub estado: String,
    pub conectado: bool,
    pub aceptado: String,
}
#[derive(Debug, Clone, Default)]
pub struct Accesos {
    pub personas: Vec<Acceso>,
    pub confirmar: Option<String>,
    pub instancia: String,
}
#[derive(Debug, Clone)]
pub enum EstadoPublicacion {
    Preparando { revision: u64, porcentaje: f32 },
    Compartiendo,
    Cambios { resumen: String },
    PreparandoNueva { revision: u64, porcentaje: f32 },
    Publicada { revision: u64, resumen: String },
    Detenida,
    Error(String),
}
#[derive(Debug, Clone)]
pub struct PanelPublicacion {
    pub instancia: InstanciaCompartida,
    pub revision: u64,
    pub fecha_revision: String,
    pub hash_revision: String,
    pub disponibilidad: String,
    pub transferencias: String,
    pub huella: String,
    pub contenido_privado: usize,
    pub estado: EstadoPublicacion,
    pub invitacion: Option<Invitacion>,
    pub accesos: Accesos,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OpcionCierre {
    #[default]
    Esperar,
    Ahora,
}
#[derive(Debug, Clone)]
pub struct Transferencia {
    pub nombre: String,
    pub fase: String,
    pub porcentaje: f32,
}
#[derive(Debug, Clone)]
pub struct CierreTransferencias {
    pub cerrar_icaro: bool,
    pub instancia: String,
    pub transferencias: Vec<Transferencia>,
    pub accesos: usize,
    pub opcion: OpcionCierre,
}

#[derive(Debug, Clone)]
pub enum EstadoEntrada {
    Vacio,
    Validando,
    Invalido,
    Vencido { fecha: String, anfitrion: String },
    Completo { anfitrion: String },
    Valido { anfitrion: String, disponibles: u32 },
    Inaccesible(String),
}
#[derive(Debug, Clone)]
pub struct EntradaInvitacion {
    pub codigo: String,
    pub estado: EstadoEntrada,
}
#[derive(Debug, Clone)]
pub enum Confianza {
    PrimeraVez,
    Conocido { fecha: String },
    Distinto { fecha: String },
}
#[derive(Debug, Clone)]
pub struct IdentidadAnfitrion {
    pub anfitrion: String,
    pub instancia: InstanciaCompartida,
    pub revision: u64,
    pub huella: String,
    pub confianza: Confianza,
    pub confirmar_nueva: bool,
    /// Matriz simétrica 8×8 calculada por el servicio de identidad.
    pub sello: [[bool; 8]; 8],
}
#[derive(Debug, Clone)]
pub struct DestinoNuevo {
    pub recuperando: bool,
    pub nombre: String,
    pub error_nombre: Option<String>,
    pub anfitrion: String,
    pub instancia: InstanciaCompartida,
    pub revision: u64,
    pub mods: String,
    pub carpetas: String,
    pub descarga: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Marca {
    Alta,
    Actualizacion,
    Retirada,
    Conflicto,
    Conservado,
}
impl Marca {
    pub fn texto(self) -> &'static str {
        match self {
            Self::Alta => "+",
            Self::Actualizacion => "~",
            Self::Retirada => "−",
            Self::Conflicto => "!",
            Self::Conservado => "=",
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrupoPublicacion {
    Nuevo,
    Actualizaciones,
    Retiradas,
    Resourcepacks,
    Privado,
}
impl GrupoPublicacion {
    pub const TODOS: [Self; 5] = [
        Self::Nuevo,
        Self::Actualizaciones,
        Self::Retiradas,
        Self::Resourcepacks,
        Self::Privado,
    ];
    pub fn titulo(self) -> &'static str {
        match self {
            Self::Nuevo => "Contenido nuevo",
            Self::Actualizaciones => "Actualizaciones",
            Self::Retiradas => "Retiradas",
            Self::Resourcepacks => "Resourcepacks",
            Self::Privado => "Contenido privado",
        }
    }
    pub fn nota(self) -> &'static str {
        match self {
        Self::Nuevo => "Empieza privado. Si no decides, se queda en tu equipo y no bloquea la publicación.",
        Self::Actualizaciones => "Ya estaba compartido y cambió. Posponer mantiene la versión publicada.",
        Self::Retiradas => "Retirarlo propone quitarlo a tus amigos en su próxima sincronización. Si ellos lo cambiaron, lo resuelven como conflicto.",
        Self::Resourcepacks => "Activación, desactivación y orden. Un cambio de orden se publica aunque no cambien archivos.",
        Self::Privado => "Se queda en tu equipo en esta y en las próximas revisiones, hasta que lo incluyas.",
    }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Eleccion {
    SinDecidir,
    Compartir,
    Privado,
    Incluir,
    Posponer,
    Retirar,
}
impl Eleccion {
    pub fn publica(self) -> bool {
        matches!(self, Self::Compartir | Self::Incluir | Self::Retirar)
    }
}
#[derive(Debug, Clone)]
pub struct Problema {
    pub archivo: String,
    pub titulo: String,
    pub detalle: String,
    pub accion: Accion,
    pub rotulo: String,
}
#[derive(Debug, Clone)]
pub struct CambioPublicacion {
    pub archivo: ArchivoCompartido,
    pub grupo: GrupoPublicacion,
    pub marca: Marca,
    pub eleccion: Eleccion,
    pub propio: bool,
    pub autorizado: bool,
    pub solo_cliente: bool,
    pub integrado: bool,
    pub problema: Option<Problema>,
    pub estado: String,
}
#[derive(Debug, Clone)]
pub enum EstadoRevisionPublicacion {
    Analizando { hechos: u64, total: u64 },
    SinCambios,
    Disponible,
    Preparando { porcentaje: f32 },
    Publicada { resumen: String },
    Error(String),
}
#[derive(Debug, Clone)]
pub struct RevisionPublicacion {
    pub instancia: InstanciaCompartida,
    pub base: u64,
    pub nueva: u64,
    pub cambios: Vec<CambioPublicacion>,
    pub estado: EstadoRevisionPublicacion,
    pub confirmar_cancelacion: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolucion {
    Anfitrion,
    ConservarMia,
}
#[derive(Debug, Clone)]
pub struct Diferencia {
    pub archivo: ArchivoCompartido,
    pub marca: Marca,
    pub del_anfitrion: bool,
    pub resolucion: Option<Resolucion>,
}
#[derive(Debug, Clone)]
pub enum EstadoRevisionInvitado {
    Revisando,
    SinCambios,
    Diferencias,
}
#[derive(Debug, Clone)]
pub struct RevisionSincronizacion {
    pub instancia: String,
    pub anfitrion: String,
    pub revision: u64,
    pub estado: EstadoRevisionInvitado,
    pub diferencias: Vec<Diferencia>,
    pub conexion: Option<ResumenConexion>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fase {
    Preparacion,
    Descarga,
    Verificacion,
    Aplicacion,
}
#[derive(Debug, Clone)]
pub enum EstadoProgreso {
    Activo(Fase),
    Cancelado,
    Recuperando,
    Completado { resumen: String },
}
#[derive(Debug, Clone)]
pub struct ProgresoSincronizacion {
    pub instancia: String,
    pub anfitrion: String,
    pub revision: u64,
    pub estado: EstadoProgreso,
    pub porcentaje: f32,
    pub detalle: String,
    pub archivo: String,
}
#[derive(Debug, Clone)]
pub enum EstadoAnfitrion {
    Buscando,
    Disponible,
    Inaccesible,
}
#[derive(Debug, Clone)]
pub enum ConexionSync {
    Conectando,
    Conectada,
    SinConexion,
    Interrumpida,
    Reconectando { intento: u32, espera: u32 },
}
#[derive(Debug, Clone)]
pub enum ConexionJuego {
    SinComprobar,
    Alcanzable { direccion: String, ping: u64 },
    SinServidor,
    RedDesconectada,
}
#[derive(Debug, Clone)]
pub struct ResumenConexion {
    pub anfitrion: EstadoAnfitrion,
    pub sincronizacion: ConexionSync,
    pub juego: ConexionJuego,
}
#[derive(Debug, Clone)]
pub enum TipoError {
    Espacio,
    ArchivoCambiado,
    Hash,
    Limite,
    RevisionNueva,
    Revocado,
}
#[derive(Debug, Clone)]
pub struct ErrorCompartir {
    pub tipo: TipoError,
    pub titulo: String,
    pub explicacion: String,
    pub conservacion: String,
    pub detalle: Option<String>,
}
#[derive(Debug, Clone)]
pub struct ResumenPublicacion {
    pub instancia: String,
    pub revision: u64,
    pub sesiones: usize,
    pub transferencias: usize,
}
