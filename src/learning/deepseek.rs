//! Proveedor DeepSeek: la primera implementacion real de [`ModelProvider`].
//!
//! # Estado
//!
//! El transporte HTTP **todavia no esta implementado**: es la primera tarea del
//! Frente 2 (ver `docs/frentes/frente-2-learning.md`). Hasta entonces
//! `SENIOR_PROVIDER=mock` deja trabajar al resto del equipo sin API key.
//!
//! La estructura ya esta puesta a proposito, para que agregar HTTP sea rellenar
//! [`DeepSeekProvider::complete`] y nada mas: ni el motor ni la CLI cambian.

use std::fmt;

use crate::contracts::{Result, SeniorError};

use super::provider::{ModelProvider, ModelRequest, ModelResponse};

pub const DEFAULT_BASE_URL: &str = "https://api.deepseek.com";
pub const DEFAULT_MODEL: &str = "deepseek-chat";

/// Reintentos ante fallos transitorios (429, 5xx, timeout).
/// Limitado a proposito: nunca un loop infinito.
pub const MAX_REINTENTOS: u32 = 2;

pub struct DeepSeekProvider {
    api_key: String,
    base_url: String,
    model: String,
}

// `Debug` manual: la API key nunca debe aparecer en un log ni en un panic.
impl fmt::Debug for DeepSeekProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeepSeekProvider")
            .field("base_url", &self.base_url)
            .field("model", &self.model)
            .field("api_key", &"<oculta>")
            .finish()
    }
}

impl DeepSeekProvider {
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            base_url: DEFAULT_BASE_URL.to_string(),
            model: DEFAULT_MODEL.to_string(),
        }
    }

    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Construye el proveedor desde el entorno (`DEEPSEEK_API_KEY`, etc.).
    ///
    /// # Errores
    /// [`SeniorError::Config`] si falta la API key.
    pub fn from_env() -> Result<Self> {
        let api_key = std::env::var("DEEPSEEK_API_KEY").map_err(|_| {
            SeniorError::Config(
                "falta DEEPSEEK_API_KEY. Copia .env.example a .env, o usa SENIOR_PROVIDER=mock."
                    .to_string(),
            )
        })?;
        if api_key.trim().is_empty() {
            return Err(SeniorError::Config(
                "DEEPSEEK_API_KEY esta vacia".to_string(),
            ));
        }

        let mut provider = Self::new(api_key);
        if let Ok(url) = std::env::var("DEEPSEEK_BASE_URL")
            && !url.trim().is_empty()
        {
            provider = provider.with_base_url(url);
        }
        if let Ok(model) = std::env::var("DEEPSEEK_MODEL")
            && !model.trim().is_empty()
        {
            provider = provider.with_model(model);
        }
        Ok(provider)
    }

    /// Endpoint de chat completions.
    pub fn endpoint(&self) -> String {
        format!(
            "{}/v1/chat/completions",
            self.base_url.trim_end_matches('/')
        )
    }

    /// Cuerpo de la peticion, ya en el formato que espera la API.
    ///
    /// Esta separado de `complete` para poder probarlo sin red.
    pub fn build_body(&self, request: &ModelRequest) -> serde_json::Value {
        serde_json::json!({
            "model": self.model,
            "messages": [
                { "role": "system", "content": request.system },
                { "role": "user", "content": request.user }
            ],
            "max_tokens": request.max_tokens,
            "temperature": request.temperature,
            "stream": false
        })
    }

    /// Extrae el texto de la respuesta de la API.
    pub fn parse_body(body: &serde_json::Value) -> Result<ModelResponse> {
        let contenido = body
            .get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| {
                SeniorError::InvalidModelResponse(
                    "la respuesta no trae choices[0].message.content".to_string(),
                )
            })?;

        let tokens = body
            .get("usage")
            .and_then(|u| u.get("total_tokens"))
            .and_then(serde_json::Value::as_u64)
            .and_then(|t| u32::try_from(t).ok());

        Ok(ModelResponse {
            content: contenido.to_string(),
            tokens_used: tokens,
            latency: std::time::Duration::ZERO,
        })
    }
}

#[async_trait::async_trait]
impl ModelProvider for DeepSeekProvider {
    fn name(&self) -> &str {
        &self.model
    }

    async fn complete(&self, _request: ModelRequest) -> Result<ModelResponse> {
        // TODO(frente-2): implementar el transporte HTTP.
        //
        // Pasos, en orden:
        //   1. descomentar `reqwest` en Cargo.toml (seccion "Frente 2"),
        //   2. POST a `self.endpoint()` con `self.build_body(&request)`,
        //      cabecera `Authorization: Bearer {api_key}`,
        //   3. medir la latencia y pasarla a `ModelResponse`,
        //   4. reintentar hasta `MAX_REINTENTOS` SOLO ante 429/5xx/timeout,
        //      con espera creciente; nunca ante un 4xx de validacion,
        //   5. leer la respuesta con `Self::parse_body`,
        //   6. mapear los fallos a `SeniorError::Provider` con un mensaje que
        //      el alumno pueda entender (sin filtrar la API key).
        //
        // `build_body` y `parse_body` ya estan probados: solo falta el envio.
        let _ = &self.api_key;
        Err(SeniorError::Provider(format!(
            "el proveedor `{}` todavia no esta implementado. Usa SENIOR_PROVIDER=mock mientras tanto.",
            self.model
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_api_key_no_aparece_en_el_debug() {
        let provider = DeepSeekProvider::new("sk-secreto-de-verdad");
        let texto = format!("{provider:?}");
        assert!(!texto.contains("sk-secreto-de-verdad"));
        assert!(texto.contains("<oculta>"));
    }

    #[test]
    fn arma_el_endpoint_sin_barras_dobles() {
        let provider = DeepSeekProvider::new("k").with_base_url("https://api.deepseek.com/");
        assert_eq!(
            provider.endpoint(),
            "https://api.deepseek.com/v1/chat/completions"
        );
    }

    #[test]
    fn el_cuerpo_lleva_system_y_user_separados() {
        let provider = DeepSeekProvider::new("k");
        let body = provider.build_body(&ModelRequest::new("reglas", "contexto del proyecto"));
        assert_eq!(body["messages"][0]["role"], "system");
        assert_eq!(body["messages"][0]["content"], "reglas");
        assert_eq!(body["messages"][1]["role"], "user");
        assert_eq!(body["stream"], false);
    }

    #[test]
    fn lee_el_contenido_y_los_tokens_de_una_respuesta() {
        let body = serde_json::json!({
            "choices": [{ "message": { "content": "hola" } }],
            "usage": { "total_tokens": 42 }
        });
        let respuesta = DeepSeekProvider::parse_body(&body).expect("parsea");
        assert_eq!(respuesta.content, "hola");
        assert_eq!(respuesta.tokens_used, Some(42));
    }

    #[test]
    fn rechaza_una_respuesta_con_forma_inesperada() {
        let body = serde_json::json!({ "error": "algo salio mal" });
        assert!(DeepSeekProvider::parse_body(&body).is_err());
    }
}
