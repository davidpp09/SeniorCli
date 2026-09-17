//! `ProjectContext`: lo que el Frente 1 entrega al Frente 2.
//!
//! Este tipo es el resultado de mirar el proyecto real. Debe ser **suficiente
//! para responder, pero no innecesariamente grande**: nada de volcar el
//! repositorio completo.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Language {
    Rust,
    Java,
    Python,
    Node,
    Go,
    CSharp,
    Other(String),
    Unknown,
}

impl Language {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Rust => "Rust",
            Self::Java => "Java",
            Self::Python => "Python",
            Self::Node => "Node",
            Self::Go => "Go",
            Self::CSharp => "C#",
            Self::Other(name) => name,
            Self::Unknown => "desconocido",
        }
    }
}

/// Un componente de build dentro del proyecto. Un monorepo puede tener varios.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StackComponent {
    pub language: Language,
    /// Raiz de este componente, relativa a la raiz del proyecto.
    pub root: PathBuf,
    /// `cargo`, `maven`, `gradle`, `npm`, `poetry`, ...
    pub build_tool: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StackInfo {
    /// Raiz absoluta del proyecto.
    pub root: PathBuf,
    /// Componente sobre el que estamos trabajando ahora mismo.
    pub active: Language,
    /// Todos los componentes detectados (un monorepo tiene mas de uno).
    pub components: Vec<StackComponent>,
}

impl StackInfo {
    pub fn unknown(root: PathBuf) -> Self {
        Self {
            root,
            active: Language::Unknown,
            components: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineRange {
    pub start: u32,
    pub end: u32,
}

/// Por que este archivo entro al contexto. Sirve para depurar el selector y
/// para que el Frente 3 pueda explicarle al alumno que esta viendo SeniorCLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelevanceReason {
    /// Aparece en un diagnostico (error de compilacion, test fallido).
    Diagnostic,
    /// Modificado segun git.
    GitModified,
    /// El alumno lo menciono explicitamente.
    Requested,
    /// Define un simbolo usado por otro archivo relevante.
    RelatedSymbol,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelevantFile {
    /// Ruta relativa a la raiz del proyecto.
    pub path: PathBuf,
    /// Rango incluido. `None` = el archivo completo.
    pub range: Option<LineRange>,
    pub content: String,
    pub reason: RelevanceReason,
    /// `true` si el contenido se recorto por limites de tamano.
    pub truncated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
    Note,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSource {
    Compiler,
    Test,
    Lint,
    Runtime,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Diagnostic {
    pub severity: Severity,
    pub source: DiagnosticSource,
    pub file: Option<PathBuf>,
    pub line: Option<u32>,
    pub column: Option<u32>,
    /// Codigo del compilador/linter, p. ej. `E0308`.
    pub code: Option<String>,
    pub message: String,
}

/// Marca de ambiguedad. El Frente 1 **no** inventa la intencion del alumno:
/// cuando la evidencia no alcanza lo senala aqui, y el Frente 2 decide
/// preguntar en lugar de adivinar.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ambiguity {
    pub reason: String,
    /// Que le falta al sistema para poder responder.
    pub missing: Vec<String>,
}

/// El idioma comun entre el Frente 1 y el Frente 2.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectContext {
    pub stack: StackInfo,
    pub relevant_files: Vec<RelevantFile>,
    pub diagnostics: Vec<Diagnostic>,
    pub git_diff: Option<String>,
    /// Presente si el Frente 1 no pudo determinar sobre que se pregunta.
    pub ambiguity: Option<Ambiguity>,
    /// `true` si se recorto contexto por limites de tamano.
    pub truncated: bool,
}

impl ProjectContext {
    /// Contexto vacio para una raiz dada. Punto de partida del recolector.
    pub fn empty(root: PathBuf) -> Self {
        Self {
            stack: StackInfo::unknown(root),
            relevant_files: Vec::new(),
            diagnostics: Vec::new(),
            git_diff: None,
            ambiguity: None,
            truncated: false,
        }
    }

    /// `true` si hay evidencia dura sobre la que razonar.
    pub fn has_evidence(&self) -> bool {
        !self.diagnostics.is_empty() || !self.relevant_files.is_empty()
    }

    /// Primer error, si lo hay. Es el ancla habitual de `senior debug`.
    pub fn first_error(&self) -> Option<&Diagnostic> {
        self.diagnostics
            .iter()
            .find(|d| d.severity == Severity::Error)
    }

    /// Tamano aproximado del contexto en caracteres. El Frente 2 lo usa para
    /// vigilar costo y latencia antes de llamar al modelo.
    pub fn approx_size(&self) -> usize {
        self.relevant_files
            .iter()
            .map(|f| f.content.len())
            .sum::<usize>()
            + self
                .diagnostics
                .iter()
                .map(|d| d.message.len())
                .sum::<usize>()
            + self.git_diff.as_ref().map_or(0, String::len)
    }

    // --- Mocks compartidos ---------------------------------------------------
    // Permiten que los Frentes 2 y 3 trabajen antes de que el Frente 1 termine.
    // Ver la seccion 5.1 de la guia y `docs/frentes/`.

    /// Caso vertical inicial: NullPointerException en un proyecto Java.
    pub fn fake_java_null_pointer() -> Self {
        let archivo = PathBuf::from("src/main/java/demo/UserService.java");
        Self {
            stack: StackInfo {
                root: PathBuf::from("/proyecto/demo"),
                active: Language::Java,
                components: vec![StackComponent {
                    language: Language::Java,
                    root: PathBuf::from("."),
                    build_tool: Some("maven".to_string()),
                }],
            },
            relevant_files: vec![RelevantFile {
                path: archivo.clone(),
                range: Some(LineRange { start: 20, end: 26 }),
                content: concat!(
                    "    public String nombreDe(Long id) {\n",
                    "        User u = repo.findById(id);\n",
                    "        return u.getNombre();\n",
                    "    }\n",
                )
                .to_string(),
                reason: RelevanceReason::Diagnostic,
                truncated: false,
            }],
            diagnostics: vec![Diagnostic {
                severity: Severity::Error,
                source: DiagnosticSource::Runtime,
                file: Some(archivo),
                line: Some(23),
                column: None,
                code: None,
                message: concat!(
                    "java.lang.NullPointerException: Cannot invoke ",
                    "demo.User.getNombre() because u is null",
                )
                .to_string(),
            }],
            git_diff: None,
            ambiguity: None,
            truncated: false,
        }
    }

    /// Caso sin evidencia: el alumno dijo "no funciona" y no hay diagnostico.
    /// El Frente 2 debe responder con `Clarify`.
    pub fn fake_ambiguous() -> Self {
        let mut ctx = Self::empty(PathBuf::from("/proyecto/demo"));
        ctx.stack.active = Language::Python;
        ctx.ambiguity = Some(Ambiguity {
            reason: "No hay error reciente ni archivo indicado".to_string(),
            missing: vec![
                "que comportamiento esperabas".to_string(),
                "que archivo o comando estabas ejecutando".to_string(),
            ],
        });
        ctx
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_mock_java_trae_evidencia_utilizable() {
        let ctx = ProjectContext::fake_java_null_pointer();
        assert!(ctx.has_evidence());
        let error = ctx.first_error().expect("el mock debe traer un error");
        assert_eq!(error.line, Some(23));
        assert!(ctx.ambiguity.is_none());
    }

    #[test]
    fn el_mock_ambiguo_no_trae_evidencia() {
        let ctx = ProjectContext::fake_ambiguous();
        assert!(!ctx.has_evidence());
        assert!(ctx.ambiguity.is_some());
    }

    #[test]
    fn el_contexto_sobrevive_al_serializado() {
        let ctx = ProjectContext::fake_java_null_pointer();
        let json = serde_json::to_string(&ctx).expect("serializa");
        let vuelta: ProjectContext = serde_json::from_str(&json).expect("deserializa");
        assert_eq!(ctx, vuelta);
    }
}
