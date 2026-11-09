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
    /// Abre la publicación guardada de esa instancia (ID, nunca el nombre).
    VerPublicacionDe(String),
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
    /// Casilla de una fila: marcada se publica, vacía no.
    MarcarPublicacion(String, bool),
    /// Casilla de tres estados de un tipo: todos o ninguno.
    MarcarTipo(TipoContenido, bool),
    AtajoPublicacion(AtajoPublicacion),
    FiltroPublicacion(FiltroPublicacion),
    BuscarPublicacion(String),
    /// Abre o pliega la sección de un tipo.
    AlternarTipo(TipoContenido),
    /// Abre la sección del tipo y la lleva a la vista.
    IrATipo(TipoContenido),
    VerTodosPublicacion(TipoContenido, GrupoPublicacion),
    SiguenSuMod(bool),
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
/// Subgrupo por cambio dentro de cada tipo de contenido.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GrupoPublicacion {
    Nuevo,
    Actualizaciones,
    Retiradas,
    /// Activación, desactivación y orden de resourcepacks.
    Cambios,
    /// Privado desde una revisión anterior.
    Privado,
}
impl GrupoPublicacion {
    pub const TODOS: [Self; 5] = [
        Self::Nuevo,
        Self::Actualizaciones,
        Self::Retiradas,
        Self::Cambios,
        Self::Privado,
    ];
    pub fn titulo(self) -> &'static str {
        match self {
            Self::Nuevo => "Nuevos",
            Self::Actualizaciones => "Actualizados",
            Self::Retiradas => "Retirados",
            Self::Cambios => "Cambios",
            Self::Privado => "Privados desde antes",
        }
    }
}

/// Tipo de contenido, asignado por carpeta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TipoContenido {
    Mod,
    Config,
    Resourcepack,
    Shader,
    Script,
}
impl TipoContenido {
    pub const TODOS: [Self; 5] = [
        Self::Mod,
        Self::Config,
        Self::Resourcepack,
        Self::Shader,
        Self::Script,
    ];
    /// `mods/`, `config/`, `resourcepacks/`, `shaderpacks/` y `kubejs/`.
    pub fn de_ruta(ruta: &str) -> Self {
        match ruta.split('/').next().unwrap_or_default() {
            "mods" => Self::Mod,
            "resourcepacks" | "@orden-resourcepacks" => Self::Resourcepack,
            "shaderpacks" => Self::Shader,
            "kubejs" => Self::Script,
            _ => Self::Config,
        }
    }
    pub fn titulo(self) -> &'static str {
        match self {
            Self::Mod => "Mods",
            Self::Config => "Configuraciones",
            Self::Resourcepack => "Resourcepacks",
            Self::Shader => "Shaders",
            Self::Script => "Scripts de KubeJS",
        }
    }
    pub fn descripcion(self) -> &'static str {
        match self {
            Self::Mod => "Lo que tus amigos necesitan para entrar a tu mundo.",
            Self::Config => "Ajustes de cada mod. Siguen a su mod salvo que los cambies.",
            Self::Resourcepack => "Texturas: activación, desactivación y orden.",
            Self::Shader => "Solo cambian cómo se ve el juego en cada equipo.",
            Self::Script => "Recetas y reglas del servidor.",
        }
    }
    pub fn icono(self) -> crate::iconos::Icono {
        use crate::iconos::Icono;
        match self {
            Self::Mod => Icono::Instancias,
            Self::Config => Icono::Ajustes,
            Self::Resourcepack => Icono::Imagen,
            Self::Shader => Icono::Sol,
            Self::Script => Icono::Codigo,
        }
    }
    /// Singular y plural para la frase del pie.
    pub fn unidades(self) -> (&'static str, &'static str) {
        match self {
            Self::Mod => ("mod", "mods"),
            Self::Config => ("configuración", "configuraciones"),
            Self::Resourcepack => ("cambio de resourcepacks", "cambios de resourcepacks"),
            Self::Shader => ("shader", "shaders"),
            Self::Script => ("script", "scripts"),
        }
    }
}

/// Atajos de «Empieza por aquí».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtajoPublicacion {
    /// Mods que no son solo de cliente, sus configuraciones y los scripts del servidor.
    Necesario,
    Todo,
    NadaNuevo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FiltroPublicacion {
    #[default]
    Todo,
    SePublica,
    NoSePublica,
    ConProblemas,
}

/// Estado de la interfaz de la revisión; sobrevive a cada recálculo del borrador.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VistaRevision {
    pub busqueda: String,
    pub filtro: FiltroPublicacion,
    pub abiertos: std::collections::BTreeSet<TipoContenido>,
    pub ver_todos: std::collections::BTreeSet<(TipoContenido, GrupoPublicacion)>,
    /// «Las configuraciones siguen a su mod».
    pub siguen_mod: bool,
    pub atajo: Option<AtajoPublicacion>,
    /// Línea de éxito tras aplicar un atajo.
    pub aplicado: String,
    /// La selección cambió en esta revisión: cerrar pide confirmación.
    pub sucio: bool,
}
impl Default for VistaRevision {
    fn default() -> Self {
        Self {
            busqueda: String::new(),
            filtro: FiltroPublicacion::Todo,
            abiertos: [TipoContenido::Mod].into(),
            ver_todos: Default::default(),
            siguen_mod: true,
            atajo: None,
            aplicado: String::new(),
            sucio: false,
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
/// Una fila de la revisión: una casilla, marcada se publica y vacía no.
#[derive(Debug, Clone)]
pub struct CambioPublicacion {
    /// `archivo.nombre` es el nombre legible (metadatos del mod), no el del archivo.
    pub archivo: ArchivoCompartido,
    pub tipo: TipoContenido,
    pub grupo: GrupoPublicacion,
    pub marca: Marca,
    pub eleccion: Eleccion,
    /// Elección que corresponde a la casilla marcada.
    pub si: Eleccion,
    /// Elección que corresponde a la casilla vacía.
    pub no: Eleccion,
    /// `0.6.5`, `0.14.3 → 0.14.5`, Modificado, Activado.
    pub version: String,
    /// Versión que siguen recibiendo los invitados si se pospone.
    pub version_publicada: String,
    /// Identificador y nombre del mod al que sigue una configuración.
    pub de_mod: Option<(String, String)>,
    pub propio: bool,
    pub autorizado: bool,
    pub solo_cliente: bool,
    pub integrado: bool,
    pub problema: Option<Problema>,
    /// Detalle con la casilla marcada; en retiradas, el motivo.
    pub estado: String,
}
impl CambioPublicacion {
    pub fn marcado(&self) -> bool {
        self.eleccion == self.si
    }
    pub fn eleccion_para(&self, marcado: bool) -> Eleccion {
        if marcado {
            self.si
        } else {
            self.no
        }
    }
    /// Cuenta como privado en la frase del pie.
    pub fn queda_privado(&self) -> bool {
        matches!(self.grupo, GrupoPublicacion::Nuevo | GrupoPublicacion::Privado) && !self.marcado()
    }
    /// Lo que se elegiría con un atajo; `None` deja la fila como está.
    pub fn con_atajo(&self, atajo: AtajoPublicacion, mod_solo_cliente: Option<bool>) -> Option<bool> {
        if !matches!(self.grupo, GrupoPublicacion::Nuevo | GrupoPublicacion::Privado) {
            return None;
        }
        Some(match atajo {
            AtajoPublicacion::Todo => true,
            AtajoPublicacion::NadaNuevo => false,
            AtajoPublicacion::Necesario => {
                let si = match self.tipo {
                    TipoContenido::Mod | TipoContenido::Script => !self.solo_cliente,
                    TipoContenido::Config => !mod_solo_cliente.unwrap_or(false),
                    TipoContenido::Resourcepack | TipoContenido::Shader => false,
                };
                si && self.problema.is_none()
            }
        })
    }
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
    /// Guardando la selección privada: las casillas esperan sin cambiar de estado.
    pub guardando: bool,
    pub vista: VistaRevision,
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
    Error(Box<ErrorCompartir>),
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
    /// ID de la instancia; la tarjeta lo envía en `Accion::VerPublicacionDe`.
    pub id: String,
    pub instancia: String,
    pub revision: u64,
    pub sesiones: usize,
    pub transferencias: usize,
}
