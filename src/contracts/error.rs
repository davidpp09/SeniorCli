//! Errores compartidos por los tres frentes.
//!
//! Regla: un frente nunca hace `panic!` en su camino normal. Los fallos se
//! modelan aqui para que el Frente 3 pueda traducirlos a un mensaje accionable
//! en lugar de mostrar un stack trace crudo.

use std::path::PathBuf;

/// Alias usado en todo el proyecto.
pub type Result<T> = std::result::Result<T, SeniorError>;

#[derive(Debug, thiserror::Error)]
pub enum SeniorError {
    // --- Frente 1: Runtime & Context ---
    #[error("no se pudo determinar la raiz del proyecto desde {0}")]
    ProjectRootNotFound(PathBuf),

    #[error("no se pudo recolectar el contexto: {0}")]
    ContextCollection(String),

    #[error("el comando `{command}` no esta en la lista de comandos permitidos")]
    CommandNotAllowed { command: String },

    #[error("el comando `{command}` excedio el limite de {timeout_secs}s")]
    CommandTimeout { command: String, timeout_secs: u64 },

    // --- Frente 2: AI & Learning Harness ---
    #[error("fallo del proveedor de modelo: {0}")]
    Provider(String),

    #[error("el proveedor respondio con un formato invalido: {0}")]
    InvalidModelResponse(String),

    /// El modelo entrego mas ayuda de la permitida por la politica pedagogica.
    /// No es un error fatal: el harness recorta o reintenta.
    #[error("la respuesta viola la politica pedagogica: {0}")]
    PolicyViolation(String),

    #[error("se alcanzo el limite de {max} rondas de herramientas en un turno")]
    ToolLoopLimit { max: u32 },

    // --- Frente 3: CLI, Producto & Persistencia ---
    #[error("no se pudo leer o escribir el estado local en {path}: {source}")]
    Storage {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("el estado local en {path} esta corrupto o es incompatible: {reason}")]
    CorruptState { path: PathBuf, reason: String },

    #[error("configuracion invalida: {0}")]
    Config(String),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl SeniorError {
    /// Mensaje corto y accionable para el alumno. El Frente 3 lo usa en lugar
    /// de imprimir el error tecnico completo.
    pub fn user_message(&self) -> String {
        match self {
            Self::ProjectRootNotFound(_) => {
                "No encontre un proyecto aqui. Ejecuta `senior init` en la raiz de tu proyecto."
                    .to_string()
            }
            Self::Provider(_) => {
                "No pude contactar al proveedor de IA. Revisa tu conexion o tu API key y reintenta."
                    .to_string()
            }
            Self::InvalidModelResponse(_) | Self::PolicyViolation(_) => {
                "La respuesta del modelo no fue utilizable. Puedes reintentar tu pregunta."
                    .to_string()
            }
            Self::CommandTimeout { command, .. } => {
                format!("El comando `{command}` tardo demasiado y lo cancele.")
            }
            Self::CorruptState { .. } => {
                "Tu estado local esta danado. Restaure el respaldo mas reciente.".to_string()
            }
            other => other.to_string(),
        }
    }

    /// `true` si tiene sentido ofrecer un reintento al alumno.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::Provider(_)
                | Self::InvalidModelResponse(_)
                | Self::PolicyViolation(_)
                | Self::CommandTimeout { .. }
        )
    }
}
