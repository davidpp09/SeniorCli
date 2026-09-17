//! Abstraccion del proveedor de modelo.
//!
//! SeniorCLI **no** esta acoplado a DeepSeek. El motor educativo solo conoce
//! este trait; cambiar a un modelo local o a un proveedor institucional es
//! escribir otra implementacion, no reescribir el producto.

use std::time::Duration;

use crate::contracts::{Result, SeniorError};

/// Peticion al modelo. Deliberadamente pobre: aqui no hay pedagogia, solo
/// texto. Las reglas educativas viven en `policy` y se verifican en `validation`.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelRequest {
    /// Instrucciones del sistema. **Nunca** contiene contenido del repositorio.
    pub system: String,
    /// Mensaje del usuario. Aqui si va el contexto del proyecto, claramente
    /// marcado como datos.
    pub user: String,
    pub max_tokens: u32,
    pub temperature: f32,
}

impl ModelRequest {
    pub fn new(system: impl Into<String>, user: impl Into<String>) -> Self {
        Self {
            system: system.into(),
            user: user.into(),
            max_tokens: 1_024,
            // Baja a proposito: queremos respuestas estables y parseables.
            temperature: 0.2,
        }
    }
}

/// Respuesta cruda del modelo, antes de validar nada.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelResponse {
    pub content: String,
    /// Tokens consumidos, si el proveedor los reporta. Alimenta las metricas.
    pub tokens_used: Option<u32>,
    pub latency: Duration,
}

impl ModelResponse {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            tokens_used: None,
            latency: Duration::ZERO,
        }
    }
}

/// La unica frontera entre SeniorCLI y cualquier LLM.
#[async_trait::async_trait]
pub trait ModelProvider: Send + Sync {
    /// Nombre para logs y metricas, p. ej. `deepseek-chat`.
    fn name(&self) -> &str;

    async fn complete(&self, request: ModelRequest) -> Result<ModelResponse>;
}

/// Proveedor falso: devuelve respuestas fijas, sin red y sin costo.
///
/// Es lo que usan los tests y el modo `SENIOR_PROVIDER=mock`, para que el
/// equipo pueda trabajar sin API key.
#[derive(Debug, Clone)]
pub struct MockProvider {
    respuestas: Vec<String>,
    /// Si es `Some`, `complete` falla siempre con este mensaje. Sirve para
    /// probar los caminos de error del harness.
    fallo: Option<String>,
}

impl MockProvider {
    /// Responde siempre con una pista valida sobre null-safety.
    pub fn hint_optional() -> Self {
        Self {
            respuestas: vec![
                r#"{"concept":"null-safety","intervention":"hint","message":"El error dice que u es null antes de llamar a getNombre(). Eso significa que repo.findById(id) no siempre devuelve un usuario.","question":"Que deberia pasar cuando ese id no existe?","confidence":0.82}"#
                    .to_string(),
            ],
            fallo: None,
        }
    }

    /// Responde con JSON roto: ejercita el reparador de `validation`.
    pub fn broken_json() -> Self {
        Self {
            respuestas: vec![
                "Claro! Aqui va:\n```json\n{\"concept\": \"null-safety\",".to_string(),
            ],
            fallo: None,
        }
    }

    /// Responde con mas ayuda de la permitida: ejercita la politica pedagogica.
    pub fn over_helpful() -> Self {
        Self {
            respuestas: vec![
                r#"{"concept":"null-safety","intervention":"guided_solution","message":"Cambia la linea 23 por: return Optional.ofNullable(u).map(User::getNombre).orElse(\"desconocido\");","question":null,"confidence":0.95}"#
                    .to_string(),
            ],
            fallo: None,
        }
    }

    /// Simula una caida del proveedor (timeout, 429, red).
    pub fn failing(motivo: impl Into<String>) -> Self {
        Self {
            respuestas: Vec::new(),
            fallo: Some(motivo.into()),
        }
    }

    pub fn with_responses(respuestas: Vec<String>) -> Self {
        Self {
            respuestas,
            fallo: None,
        }
    }
}

#[async_trait::async_trait]
impl ModelProvider for MockProvider {
    fn name(&self) -> &str {
        "mock"
    }

    async fn complete(&self, _request: ModelRequest) -> Result<ModelResponse> {
        if let Some(motivo) = &self.fallo {
            return Err(SeniorError::Provider(motivo.clone()));
        }
        let contenido = self.respuestas.first().cloned().unwrap_or_default();
        Ok(ModelResponse {
            content: contenido,
            tokens_used: Some(0),
            latency: Duration::ZERO,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn el_mock_responde_sin_red() {
        let provider = MockProvider::hint_optional();
        let respuesta = provider
            .complete(ModelRequest::new("sistema", "usuario"))
            .await
            .expect("responde");
        assert!(respuesta.content.contains("null-safety"));
    }

    #[tokio::test]
    async fn el_mock_puede_simular_una_caida() {
        let provider = MockProvider::failing("429 rate limit");
        let error = provider
            .complete(ModelRequest::new("s", "u"))
            .await
            .unwrap_err();
        assert!(error.is_retryable());
    }
}
