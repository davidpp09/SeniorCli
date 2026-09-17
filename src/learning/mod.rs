//! # Frente 2 - AI & Learning Harness
//!
//! Controla al LLM: que contexto recibe, que puede responder y cuanta ayuda
//! puede dar. Su unica salida publica es una
//! [`TutorDecision`](crate::contracts::TutorDecision).
//!
//! Este frente existe para que SeniorCLI **no sea un chat con una API**. Todo
//! lo que puede decidirse con reglas se decide en codigo:
//!
//! - [`policy`]     cuanta ayuda se permite, y verificacion de la salida;
//! - [`validation`] parseo del JSON del modelo, con una reparacion controlada;
//! - [`provider`]   la frontera con cualquier LLM;
//! - [`deepseek`]   la primera implementacion concreta;
//! - [`mastery`]    como se mueve el dominio del alumno;
//! - [`engine`]     el orquestador del turno.
//!
//! Lo que este frente nunca hace: leer el repositorio por su cuenta (eso es del
//! Frente 1) ni imprimir nada en pantalla (eso es del Frente 3).

pub mod deepseek;
pub mod engine;
pub mod mastery;
pub mod policy;
pub mod provider;
pub mod validation;

pub use deepseek::DeepSeekProvider;
pub use engine::{LearningEngine, TurnMetrics};
pub use mastery::{Evidence, record_evidence};
pub use policy::TeachingPolicy;
pub use provider::{MockProvider, ModelProvider, ModelRequest, ModelResponse};

use crate::contracts::{Result, TutorEngine};

/// Construye el motor educativo segun la configuracion del entorno.
///
/// `SENIOR_PROVIDER=mock` (el valor por defecto mientras el transporte HTTP no
/// existe) permite a todo el equipo trabajar sin API key.
pub fn build_engine(provider_name: &str, policy: TeachingPolicy) -> Result<Box<dyn TutorEngine>> {
    match provider_name {
        "deepseek" => {
            let provider = DeepSeekProvider::from_env()?;
            Ok(Box::new(LearningEngine::new(provider).with_policy(policy)))
        }
        // TODO(frente-2): agregar "local" cuando exista un proveedor on-device.
        _ => Ok(Box::new(
            LearningEngine::new(MockProvider::hint_optional()).with_policy(policy),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_proveedor_por_defecto_es_el_mock_y_no_pide_api_key() {
        assert!(build_engine("mock", TeachingPolicy::default()).is_ok());
        assert!(build_engine("cualquier-cosa", TeachingPolicy::default()).is_ok());
    }
}
