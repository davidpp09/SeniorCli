//! Parseo y validacion de la respuesta del modelo.
//!
//! Los modelos rompen el esquema: agregan texto antes del JSON, lo envuelven en
//! bloques ```json, cortan a medias por limite de tokens. Este modulo intenta
//! **una** reparacion controlada y, si no se puede, devuelve un error para que
//! el motor use el fallback seguro. Nunca se inventa una decision.

use crate::contracts::{Result, SeniorError, TutorDecision};

/// Extrae y valida una [`TutorDecision`] del texto crudo del modelo.
///
/// # Errores
/// [`SeniorError::InvalidModelResponse`] si no hay JSON recuperable o si el
/// contenido no pasa [`TutorDecision::validate`].
pub fn parse_decision(raw: &str) -> Result<TutorDecision> {
    let json = extraer_json(raw)
        .ok_or_else(|| SeniorError::InvalidModelResponse("no encontre un objeto JSON".into()))?;

    let decision: TutorDecision = serde_json::from_str(&json).map_err(|e| {
        SeniorError::InvalidModelResponse(format!("el JSON no coincide con TutorDecision: {e}"))
    })?;

    decision
        .validate()
        .map_err(SeniorError::InvalidModelResponse)?;

    Ok(decision)
}

/// Saca el primer objeto JSON completo del texto.
///
/// Tolera: prosa antes y despues, bloques ```json, y llaves dentro de cadenas.
fn extraer_json(raw: &str) -> Option<String> {
    let texto = quitar_cercas(raw);
    let bytes = texto.as_bytes();
    let inicio = texto.find('{')?;

    let mut profundidad = 0usize;
    let mut en_cadena = false;
    let mut escapado = false;

    for (i, &b) in bytes.iter().enumerate().skip(inicio) {
        if en_cadena {
            match b {
                _ if escapado => escapado = false,
                b'\\' => escapado = true,
                b'"' => en_cadena = false,
                _ => {}
            }
            continue;
        }
        match b {
            b'"' => en_cadena = true,
            b'{' => profundidad += 1,
            b'}' => {
                profundidad -= 1;
                if profundidad == 0 {
                    return Some(texto[inicio..=i].to_string());
                }
            }
            _ => {}
        }
    }

    // JSON truncado por limite de tokens: no lo reparamos a mano, seria adivinar.
    None
}

/// Quita bloques ```json ... ``` dejando solo el contenido.
fn quitar_cercas(raw: &str) -> &str {
    let texto = raw.trim();
    let Some(resto) = texto.strip_prefix("```") else {
        return texto;
    };
    // Saltar el lenguaje declarado (```json) hasta el salto de linea.
    let resto = resto.split_once('\n').map_or(resto, |(_, r)| r);
    resto.strip_suffix("```").unwrap_or(resto).trim()
}

/// Esquema que se le pide al modelo. Vive junto al parser a proposito: si
/// alguien cambia el esquema, ve inmediatamente que tiene que actualizar aqui.
pub const ESQUEMA_JSON: &str = r#"{
  "concept": "string, el concepto de programacion que se esta trabajando",
  "intervention": "clarify | socratic_question | hint | explanation | example | pseudocode | partial_code | guided_solution",
  "message": "string, texto para el alumno",
  "question": "string o null, pregunta de vuelta (obligatoria si intervention es clarify)",
  "confidence": "numero entre 0.0 y 1.0"
}"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::Intervention;

    const VALIDO: &str = r#"{"concept":"null-safety","intervention":"hint","message":"El objeto puede ser null.","question":"Que pasa si el id no existe?","confidence":0.8}"#;

    #[test]
    fn parsea_json_limpio() {
        let decision = parse_decision(VALIDO).expect("parsea");
        assert_eq!(decision.intervention, Intervention::Hint);
        assert_eq!(decision.concept, "null-safety");
    }

    #[test]
    fn tolera_bloques_de_codigo() {
        let envuelto = format!("```json\n{VALIDO}\n```");
        assert!(parse_decision(&envuelto).is_ok());
    }

    #[test]
    fn tolera_prosa_antes_y_despues() {
        let ruidoso = format!("Claro, aqui va mi analisis:\n{VALIDO}\nEspero que ayude!");
        assert!(parse_decision(&ruidoso).is_ok());
    }

    #[test]
    fn tolera_llaves_dentro_de_las_cadenas() {
        let con_llaves = r#"{"concept":"sintaxis","intervention":"hint","message":"Te falta cerrar el bloque { aqui }","question":null,"confidence":0.7}"#;
        let decision = parse_decision(con_llaves).expect("parsea");
        assert!(decision.message.contains('{'));
    }

    #[test]
    fn rechaza_json_truncado_en_vez_de_adivinar() {
        let truncado = r#"{"concept": "null-safety", "intervention": "hi"#;
        assert!(parse_decision(truncado).is_err());
    }

    #[test]
    fn rechaza_una_intervencion_inexistente() {
        let inventada = r#"{"concept":"x","intervention":"resolver_todo","message":"y","question":null,"confidence":0.5}"#;
        assert!(parse_decision(inventada).is_err());
    }

    #[test]
    fn rechaza_clarify_sin_pregunta() {
        let mala = r#"{"concept":"x","intervention":"clarify","message":"y","question":null,"confidence":0.5}"#;
        assert!(parse_decision(mala).is_err());
    }

    #[test]
    fn rechaza_confianza_fuera_de_rango() {
        let mala = r#"{"concept":"x","intervention":"hint","message":"y","question":null,"confidence":9.9}"#;
        assert!(parse_decision(mala).is_err());
    }

    #[test]
    fn rechaza_texto_sin_json() {
        assert!(parse_decision("Lo siento, no puedo ayudarte con eso.").is_err());
    }
}
