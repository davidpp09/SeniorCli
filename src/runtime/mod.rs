//! # Frente 1 - Runtime & Context
//!
//! Hace que SeniorCLI entienda el proyecto real **antes** de preguntarle al
//! modelo. Su unica salida publica es un [`ProjectContext`].
//!
//! Flujo interno (seccion 2.3 de la guia):
//! 1. detectar raiz y stack ([`detector`]),
//! 2. leer cambios recientes y el error actual ([`git`], [`diagnostics`]),
//! 3. ejecutar solo diagnosticos permitidos ([`runner`]),
//! 4. identificar archivos y rangos utiles ([`files`]),
//! 5. filtrar secretos y contenido enorme ([`files`]),
//! 6. entregar el contexto **sin** decidir la respuesta pedagogica.
//!
//! Lo que este frente nunca hace: elegir el tipo de intervencion, hablar con el
//! LLM, o inventar la intencion del alumno.

pub mod detector;
pub mod diagnostics;
pub mod files;
pub mod git;
pub mod runner;

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::contracts::{
    Ambiguity, ContextEngine, ContextFocus, Language, ProjectContext, RelevanceReason,
    RelevantFile, Result, StackInfo,
};

use runner::CommandRunner;

/// Cuantos archivos como maximo viajan en un contexto.
pub const MAX_CONTEXT_FILES: usize = 6;

/// Recolector real de contexto. Implementa [`ContextEngine`], asi que el Frente
/// 3 puede intercambiarlo por `MockContextEngine` sin cambiar una linea.
#[derive(Debug, Clone)]
pub struct ContextCollector {
    timeout: Duration,
    /// Si es `false`, no se ejecuta ningun build (modo rapido / offline).
    run_diagnostics: bool,
}

impl Default for ContextCollector {
    fn default() -> Self {
        Self {
            timeout: CommandRunner::DEFAULT_TIMEOUT,
            run_diagnostics: true,
        }
    }
}

impl ContextCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Desactiva la ejecucion de builds. Util para tests y para `senior learn`,
    /// donde no siempre hace falta compilar.
    pub fn without_diagnostics(mut self) -> Self {
        self.run_diagnostics = false;
        self
    }

    /// Comando de diagnostico permitido para un stack, si existe.
    ///
    /// TODO(frente-1): agregar Python (`pytest -q`) y Node cuando sus parsers
    /// esten listos. No sirve ejecutar un build que nadie sabe leer.
    fn comando_de_diagnostico(stack: &StackInfo) -> Option<(&'static str, Vec<&'static str>)> {
        match stack.active {
            Language::Rust => Some(("cargo", vec!["check", "--message-format=short"])),
            Language::Java => {
                let gradle = stack
                    .components
                    .iter()
                    .any(|c| c.build_tool.as_deref() == Some("gradle"));
                if gradle {
                    Some(("gradle", vec!["compileJava"]))
                } else {
                    Some(("mvn", vec!["-q", "test-compile"]))
                }
            }
            _ => None,
        }
    }
}

#[async_trait::async_trait]
impl ContextEngine for ContextCollector {
    async fn collect(&self, root: &Path, focus: ContextFocus) -> Result<ProjectContext> {
        let root = detector::find_project_root(root)?;
        let mut contexto = ProjectContext::empty(root.clone());

        // 1. Stack.
        contexto.stack = detector::detect_stack(&root)?;

        // 2. Estado de git.
        let runner = CommandRunner::new(&root).with_timeout(self.timeout);
        let snapshot = git::snapshot(&runner).await?;
        contexto.git_diff = snapshot.diff.clone();
        contexto.truncated |= snapshot.diff_truncated;

        // 3. Diagnosticos, solo si el foco los amerita y sabemos leerlos.
        let quiere_build = self.run_diagnostics
            && matches!(focus, ContextFocus::LastError | ContextFocus::Unspecified);
        if quiere_build
            && let Some((programa, args)) = Self::comando_de_diagnostico(&contexto.stack)
        {
            match runner.run_checked(programa, &args).await {
                Ok(salida) if !salida.success() => {
                    contexto.diagnostics =
                        diagnostics::parse(&contexto.stack.active, &salida.combined());
                }
                Ok(_) => {}
                // Que falte la herramienta de build no es fatal: seguimos con
                // el contexto que si tenemos.
                Err(e) => tracing::debug!("diagnostico omitido: {e}"),
            }
        }

        // 4 y 5. Archivos relevantes, ya filtrados por secretos y tamano.
        let candidatos = self.reunir_archivos(&root, &focus, &contexto, &snapshot.modified_files);
        contexto.truncated |= candidatos.iter().any(|f| f.truncated);
        contexto.relevant_files = files::select_relevant_files(candidatos, MAX_CONTEXT_FILES);

        // 6. Marcar ambiguedad en lugar de adivinar la intencion.
        if !contexto.has_evidence() {
            contexto.ambiguity = Some(Ambiguity {
                reason: "No encontre errores recientes ni cambios sin commitear".to_string(),
                missing: vec![
                    "que comando ejecutaste".to_string(),
                    "que archivo estabas editando".to_string(),
                ],
            });
        }

        Ok(contexto)
    }
}

impl ContextCollector {
    fn reunir_archivos(
        &self,
        root: &Path,
        focus: &ContextFocus,
        contexto: &ProjectContext,
        modificados: &[PathBuf],
    ) -> Vec<RelevantFile> {
        let mut candidatos = Vec::new();

        // El archivo que el alumno senalo explicitamente.
        if let ContextFocus::File(ruta) = focus
            && let Ok(Some(archivo)) =
                files::read_relevant_file(root, ruta, None, RelevanceReason::Requested)
        {
            candidatos.push(archivo);
        }

        // Los archivos que aparecen en los diagnosticos, recortados al rango
        // del error: es donde de verdad esta el problema.
        for diagnostico in &contexto.diagnostics {
            let Some(ruta) = &diagnostico.file else {
                continue;
            };
            if let Ok(Some(archivo)) =
                files::read_relevant_file(root, ruta, diagnostico.line, RelevanceReason::Diagnostic)
            {
                candidatos.push(archivo);
            }
        }

        // Lo que el alumno toco desde el ultimo commit.
        for ruta in modificados
            .iter()
            .filter(|ruta| files::is_source_file(ruta.as_path()))
        {
            if let Ok(Some(archivo)) =
                files::read_relevant_file(root, ruta, None, RelevanceReason::GitModified)
            {
                candidatos.push(archivo);
            }
        }

        candidatos
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn recolecta_contexto_de_este_mismo_repo() {
        let collector = ContextCollector::new().without_diagnostics();
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let contexto = collector
            .collect(root, ContextFocus::RecentChanges)
            .await
            .expect("recolecta contexto");

        assert_eq!(contexto.stack.active, Language::Rust);
        assert!(contexto.stack.root.ends_with("seniorcli"));
    }

    #[tokio::test]
    async fn marca_ambiguedad_cuando_no_hay_evidencia() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname=\"x\"").expect("escribe");

        let collector = ContextCollector::new().without_diagnostics();
        let contexto = collector
            .collect(dir.path(), ContextFocus::Unspecified)
            .await
            .expect("recolecta contexto");

        assert!(!contexto.has_evidence());
        assert!(contexto.ambiguity.is_some());
    }

    #[tokio::test]
    async fn el_collector_es_intercambiable_por_el_mock() {
        // La prueba de que el contrato funciona: los dos son `dyn ContextEngine`.
        let motores: Vec<Box<dyn ContextEngine>> = vec![
            Box::new(ContextCollector::new().without_diagnostics()),
            Box::new(crate::contracts::MockContextEngine::java_null_pointer()),
        ];
        for motor in motores {
            let root = Path::new(env!("CARGO_MANIFEST_DIR"));
            assert!(motor.collect(root, ContextFocus::Unspecified).await.is_ok());
        }
    }
}
