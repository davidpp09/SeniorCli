//! `TutorRequest` / `TutorDecision`: el idioma comun entre el Frente 2 y el
//! Frente 3.
//!
//! El nivel de ayuda es un campo **externo al prompt**: se decide en codigo,
//! se le pide al modelo y despues se verifica contra la salida. Asi una pista
//! no termina convirtiendose en la solucion completa.

use serde::{Deserialize, Serialize};

use super::{ProjectContext, StudentProfile};

/// Tipo de intervencion educativa, ordenada de menos a mas ayuda.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intervention {
    /// Falta intencion o evidencia: hay que preguntar antes de ensenar.
    Clarify,
    SocraticQuestion,
    Hint,
    Explanation,
    Example,
    Pseudocode,
    PartialCode,
    GuidedSolution,
}

impl Intervention {
    /// Cuanta ayuda entrega, de 0 (solo pregunta) a 7 (solucion guiada).
    /// La politica pedagogica compara este numero con el techo permitido.
    pub fn help_level(self) -> u8 {
        match self {
            Self::Clarify => 0,
            Self::SocraticQuestion => 1,
            Self::Hint => 2,
            Self::Explanation => 3,
            Self::Example => 4,
            Self::Pseudocode => 5,
            Self::PartialCode => 6,
            Self::GuidedSolution => 7,
        }
    }

    /// La intervencion inmediatamente menos generosa. Se usa para recortar una
    /// respuesta que se paso del techo en vez de descartarla entera.
    pub fn downgrade(self) -> Option<Self> {
        match self {
            Self::Clarify => None,
            Self::SocraticQuestion => Some(Self::Clarify),
            Self::Hint => Some(Self::SocraticQuestion),
            Self::Explanation => Some(Self::Hint),
            Self::Example => Some(Self::Explanation),
            Self::Pseudocode => Some(Self::Example),
            Self::PartialCode => Some(Self::Pseudocode),
            Self::GuidedSolution => Some(Self::PartialCode),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Clarify => "aclaracion",
            Self::SocraticQuestion => "pregunta socratica",
            Self::Hint => "pista",
            Self::Explanation => "explicacion",
            Self::Example => "ejemplo",
            Self::Pseudocode => "pseudocodigo",
            Self::PartialCode => "codigo parcial",
            Self::GuidedSolution => "solucion guiada",
        }
    }

    /// `true` si la intervencion puede incluir codigo ejecutable del proyecto.
    pub fn may_contain_code(self) -> bool {
        matches!(
            self,
            Self::Example | Self::PartialCode | Self::GuidedSolution
        )
    }
}

/// Todo lo que el Frente 2 necesita para decidir. Se reconstruye **cada turno**
/// desde el contexto, el perfil y el objetivo actual: nada depende de que el
/// historial de chat siga intacto.
#[derive(Debug, Clone, PartialEq)]
pub struct TutorRequest {
    /// Lo que escribio el alumno, tal cual.
    pub student_message: String,
    pub context: ProjectContext,
    pub profile: StudentProfile,
    /// Objetivo declarado de la sesion, si existe.
    pub goal: Option<String>,
    /// Techo de ayuda para este turno. Lo fija la politica, no el modelo.
    pub max_help_level: u8,
}

impl TutorRequest {
    pub fn new(
        student_message: impl Into<String>,
        context: ProjectContext,
        profile: StudentProfile,
    ) -> Self {
        Self {
            student_message: student_message.into(),
            context,
            profile,
            goal: None,
            max_help_level: Intervention::Hint.help_level(),
        }
    }

    pub fn with_goal(mut self, goal: impl Into<String>) -> Self {
        self.goal = Some(goal.into());
        self
    }

    pub fn with_max_help_level(mut self, level: u8) -> Self {
        self.max_help_level = level;
        self
    }

    /// `true` si el mensaje del alumno no dice nada accionable por si solo.
    /// Junto con `context.ambiguity`, empuja la decision hacia `Clarify`.
    pub fn looks_vague(&self) -> bool {
        let msg = self.student_message.trim().to_lowercase();
        const VAGAS: [&str; 6] = [
            "no funciona",
            "no sirve",
            "no jala",
            "arregla esto",
            "hazlo funcionar",
            "ayuda",
        ];
        msg.len() < 12 || VAGAS.iter().any(|v| msg == *v || msg.starts_with(v))
    }
}

/// La unica salida del Frente 2. El Frente 3 la trata como **fuente de verdad
/// del turno**: no re-decide el nivel de ayuda por su cuenta.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TutorDecision {
    /// Concepto que se esta trabajando, p. ej. `null-safety`, `ownership`.
    pub concept: String,
    pub intervention: Intervention,
    /// Texto para el alumno.
    pub message: String,
    /// Pregunta de vuelta. Obligatoria cuando la intervencion es `Clarify`.
    pub question: Option<String>,
    /// Confianza declarada, de 0.0 a 1.0. Es una senal, no una verdad:
    /// la confianza del modelo nunca se convierte en autoridad del sistema.
    pub confidence: f32,
}

impl TutorDecision {
    /// Respuesta segura cuando el modelo falla o su salida es inutilizable.
    /// Prefiere preguntar antes que inventar.
    pub fn safe_fallback(motivo: &str) -> Self {
        Self {
            concept: "indeterminado".to_string(),
            intervention: Intervention::Clarify,
            message: format!(
                "No pude analizar esto con confianza ({motivo}). Prefiero no adivinar."
            ),
            question: Some("Cuentame que esperabas que pasara y que paso en su lugar.".to_string()),
            confidence: 0.0,
        }
    }

    /// Reglas minimas que toda decision debe cumplir, venga del modelo o no.
    pub fn validate(&self) -> Result<(), String> {
        if self.concept.trim().is_empty() {
            return Err("`concept` vacio".to_string());
        }
        if self.message.trim().is_empty() {
            return Err("`message` vacio".to_string());
        }
        if !(0.0..=1.0).contains(&self.confidence) {
            return Err(format!("`confidence` fuera de rango: {}", self.confidence));
        }
        if self.intervention == Intervention::Clarify && self.question.is_none() {
            return Err("una intervencion `clarify` debe incluir una pregunta".to_string());
        }
        Ok(())
    }

    // --- Mocks compartidos ---------------------------------------------------
    // El Frente 3 construye pantalla, input y guardado con esto, sin esperar al
    // Frente 2.

    /// Pista sobre `Optional`/null para el caso vertical inicial.
    pub fn fake_hint_optional() -> Self {
        Self {
            concept: "null-safety".to_string(),
            intervention: Intervention::Hint,
            message: concat!(
                "El error dice que `u` es null justo antes de llamar a `getNombre()`. ",
                "Eso significa que `repo.findById(id)` no siempre devuelve un usuario.",
            )
            .to_string(),
            question: Some(
                "Que deberia pasar cuando ese id no existe en la base de datos?".to_string(),
            ),
            confidence: 0.82,
        }
    }

    /// Respuesta esperada ante una peticion vaga.
    pub fn fake_clarify() -> Self {
        Self {
            concept: "indeterminado".to_string(),
            intervention: Intervention::Clarify,
            message: "Todavia no tengo suficiente evidencia para ayudarte bien.".to_string(),
            question: Some("Que comando ejecutaste y que mensaje te aparecio?".to_string()),
            confidence: 0.3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn los_niveles_de_ayuda_estan_ordenados() {
        assert!(Intervention::Clarify.help_level() < Intervention::Hint.help_level());
        assert!(Intervention::Hint.help_level() < Intervention::GuidedSolution.help_level());
    }

    #[test]
    fn bajar_de_nivel_termina_en_clarify() {
        let mut actual = Intervention::GuidedSolution;
        let mut pasos = 0;
        while let Some(menor) = actual.downgrade() {
            actual = menor;
            pasos += 1;
            assert!(pasos < 20, "downgrade no deberia ciclar");
        }
        assert_eq!(actual, Intervention::Clarify);
    }

    #[test]
    fn clarify_sin_pregunta_es_invalida() {
        let mut decision = TutorDecision::fake_clarify();
        decision.question = None;
        assert!(decision.validate().is_err());
    }

    #[test]
    fn el_fallback_seguro_siempre_es_valido() {
        assert!(TutorDecision::safe_fallback("timeout").validate().is_ok());
    }

    #[test]
    fn detecta_peticiones_vagas() {
        let vaga = TutorRequest::new(
            "no funciona",
            ProjectContext::fake_ambiguous(),
            StudentProfile::fake_beginner(),
        );
        assert!(vaga.looks_vague());

        let clara = TutorRequest::new(
            "me tira NullPointerException en UserService linea 23, por que?",
            ProjectContext::fake_java_null_pointer(),
            StudentProfile::fake_beginner(),
        );
        assert!(!clara.looks_vague());
    }
}
