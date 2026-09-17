//! Contratos compartidos: el idioma comun de los tres frentes.
//!
//! **Regla de oro del repositorio:** un frente solo depende de este modulo,
//! nunca de los modulos de otro frente. Si el Frente 3 necesita algo de
//! `runtime::`, no lo importa directamente: se agrega al trait
//! [`ContextEngine`] y se discute en un PR contra este archivo.
//!
//! Cambiar un tipo de aqui rompe a los otros dos frentes. Por eso
//! `src/contracts/` tiene revisores obligatorios en `.github/CODEOWNERS`.
//!
//! # Trabajar aislado
//!
//! Cada trait tiene una implementacion falsa (`Mock*`) para que un frente
//! pueda avanzar antes de que los otros existan:
//!
//! ```
//! use seniorcli::contracts::{ContextEngine, MockContextEngine, ContextFocus};
//! # async fn demo() -> seniorcli::Result<()> {
//! let engine = MockContextEngine::java_null_pointer();
//! let ctx = engine.collect(std::path::Path::new("."), ContextFocus::LastError).await?;
//! assert!(ctx.has_evidence());
//! # Ok(()) }
//! ```

mod error;
mod project_context;
mod student_profile;
mod tutor_decision;

use std::path::Path;

pub use error::{Result, SeniorError};
pub use project_context::{
    Ambiguity, Diagnostic, DiagnosticSource, Language, LineRange, ProjectContext, RelevanceReason,
    RelevantFile, Severity, StackComponent, StackInfo,
};
pub use student_profile::{PROFILE_SCHEMA_VERSION, SkillLevel, StudentProfile, TopicMastery};
pub use tutor_decision::{Intervention, TutorDecision, TutorRequest};

/// Sobre que quiere el alumno que miremos. Acota el trabajo del Frente 1 para
/// no recolectar de mas.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextFocus {
    /// `senior debug`: el ultimo error de compilacion o de tests.
    LastError,
    /// `senior learn`: lo que cambio recientemente segun git.
    RecentChanges,
    /// El alumno senalo un archivo concreto.
    File(std::path::PathBuf),
    /// Sin pista: recolectar lo minimo y marcar ambiguedad.
    Unspecified,
}

/// Frontera Frente 1 -> Frente 2.
#[async_trait::async_trait]
pub trait ContextEngine: Send + Sync {
    /// Mira el proyecto real y devuelve contexto minimo y suficiente.
    ///
    /// No decide la respuesta pedagogica: si falta informacion, lo senala en
    /// [`ProjectContext::ambiguity`] y deja que el Frente 2 pregunte.
    async fn collect(&self, root: &Path, focus: ContextFocus) -> Result<ProjectContext>;
}

/// Frontera Frente 2 -> Frente 3.
#[async_trait::async_trait]
pub trait TutorEngine: Send + Sync {
    /// Convierte contexto + perfil + mensaje en una intervencion educativa.
    ///
    /// Nunca debe devolver `Err` por un fallo del modelo: eso se resuelve con
    /// [`TutorDecision::safe_fallback`]. Reserva `Err` para fallos del sistema.
    async fn decide(&self, request: TutorRequest) -> Result<TutorDecision>;
}

/// Persistencia del Frente 3, vista por los demas frentes.
pub trait ProfileStore: Send + Sync {
    fn load_profile(&self) -> Result<StudentProfile>;
    /// Debe ser atomica: nunca dejar un perfil a medio escribir.
    fn save_profile(&self, profile: &StudentProfile) -> Result<()>;
}

// --- Implementaciones falsas -------------------------------------------------

/// `ContextEngine` de mentira, para desarrollar los Frentes 2 y 3 en aislamiento.
#[derive(Debug, Clone)]
pub struct MockContextEngine {
    context: ProjectContext,
}

impl MockContextEngine {
    pub fn new(context: ProjectContext) -> Self {
        Self { context }
    }

    /// Caso vertical inicial de la guia.
    pub fn java_null_pointer() -> Self {
        Self::new(ProjectContext::fake_java_null_pointer())
    }

    /// Caso sin evidencia, para probar el camino `Clarify`.
    pub fn ambiguous() -> Self {
        Self::new(ProjectContext::fake_ambiguous())
    }
}

#[async_trait::async_trait]
impl ContextEngine for MockContextEngine {
    async fn collect(&self, _root: &Path, _focus: ContextFocus) -> Result<ProjectContext> {
        Ok(self.context.clone())
    }
}

/// `TutorEngine` de mentira, para desarrollar el Frente 3 sin tocar la API.
#[derive(Debug, Clone)]
pub struct MockTutorEngine {
    decision: TutorDecision,
}

impl MockTutorEngine {
    pub fn new(decision: TutorDecision) -> Self {
        Self { decision }
    }

    pub fn hint() -> Self {
        Self::new(TutorDecision::fake_hint_optional())
    }

    pub fn clarify() -> Self {
        Self::new(TutorDecision::fake_clarify())
    }
}

#[async_trait::async_trait]
impl TutorEngine for MockTutorEngine {
    async fn decide(&self, _request: TutorRequest) -> Result<TutorDecision> {
        Ok(self.decision.clone())
    }
}
