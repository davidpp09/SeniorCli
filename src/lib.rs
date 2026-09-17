//! SeniorCLI - AI Learning Harness para aprender a programar desde la terminal.
//!
//! El proyecto esta dividido en tres frentes que se comunican **solo** a traves
//! de los tipos y traits de [`contracts`]. Esa es la regla de integracion: los
//! frentes no comparten funciones sueltas, intercambian estructuras pequenas y
//! estables.
//!
//! ```text
//! USUARIO -> [app] -> [runtime] -> ProjectContext
//!                  -> [learning] -> TutorDecision
//!                  -> [app] -> USUARIO
//! ```
//!
//! - [`runtime`]  Frente 1: convierte el proyecto real en un `ProjectContext`.
//! - [`learning`] Frente 2: convierte contexto + perfil en una `TutorDecision`.
//! - [`app`]      Frente 3: CLI, presentacion y persistencia local.

pub mod app;
pub mod contracts;
pub mod learning;
pub mod runtime;

pub use contracts::{Result, SeniorError};
