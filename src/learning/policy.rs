//! Politica pedagogica: cuanta ayuda se permite en este turno.
//!
//! Este modulo es la razon de ser de SeniorCLI. Una respuesta tecnicamente
//! correcta puede ser **incorrecta para el producto** si entrega demasiado y
//! evita que el alumno piense.
//!
//! El nivel de ayuda se calcula aqui, en codigo deterministico, y se **verifica**
//! contra lo que devolvio el modelo. Nunca se confia en que el modelo obedezca
//! el prompt.

use crate::contracts::{
    Intervention, ProjectContext, SkillLevel, StudentProfile, TutorDecision, TutorRequest,
};

/// Reglas de cuanta ayuda dar, segun nivel y dominio del tema.
#[derive(Debug, Clone, PartialEq)]
pub struct TeachingPolicy {
    /// Techo absoluto, pase lo que pase. Sube solo si el alumno lo pide
    /// explicitamente (`senior debug --explain`).
    pub ceiling: Intervention,
    /// Cuantas veces seguidas puede pedir ayuda sobre el mismo concepto antes
    /// de que subamos el nivel.
    pub intentos_antes_de_subir: u32,
}

impl Default for TeachingPolicy {
    fn default() -> Self {
        Self {
            ceiling: Intervention::Pseudocode,
            intentos_antes_de_subir: 2,
        }
    }
}

impl TeachingPolicy {
    /// Politica para cuando el alumno pide explicitamente la solucion.
    pub fn permisiva() -> Self {
        Self {
            ceiling: Intervention::GuidedSolution,
            intentos_antes_de_subir: 1,
        }
    }

    /// Politica estricta: nunca pasa de la pista.
    pub fn socratica() -> Self {
        Self {
            ceiling: Intervention::Hint,
            intentos_antes_de_subir: 3,
        }
    }

    /// Calcula el techo de ayuda para este turno.
    ///
    /// Se apoya en tres senales, en este orden:
    /// 1. si falta intencion o evidencia, solo se puede preguntar;
    /// 2. el nivel declarado del alumno;
    /// 3. cuanto domina ya el concepto (menos dominio, un poco mas de ayuda).
    pub fn max_help_level(
        &self,
        context: &ProjectContext,
        profile: &StudentProfile,
        mensaje_vago: bool,
        concepto: Option<&str>,
    ) -> u8 {
        // 1. Sin intencion clara o sin evidencia: preguntar, no ensenar.
        if mensaje_vago || context.ambiguity.is_some() || !context.has_evidence() {
            return Intervention::Clarify.help_level();
        }

        // 2. Base por nivel del alumno.
        let base = match profile.level {
            SkillLevel::Beginner => Intervention::Explanation,
            SkillLevel::Intermediate => Intervention::Hint,
            SkillLevel::Advanced => Intervention::SocraticQuestion,
        };

        // 3. Ajuste por dominio del concepto.
        let ajuste = concepto
            .and_then(|c| profile.topic(c))
            .filter(|t| t.is_reliable())
            .map_or(0, |t| if t.mastery < 0.4 { 1 } else { 0 });

        (base.help_level() + ajuste).min(self.ceiling.help_level())
    }

    /// Verifica la decision del modelo contra el techo del turno.
    ///
    /// No descarta la respuesta: la **recorta**. Bajar de `GuidedSolution` a
    /// `Hint` conserva el trabajo util y respeta la politica.
    ///
    /// Devuelve la decision ajustada y `true` si hubo que recortarla (eso se
    /// registra como metrica de "sobre-ayuda" del modelo).
    pub fn enforce(
        &self,
        mut decision: TutorDecision,
        max_help_level: u8,
    ) -> (TutorDecision, bool) {
        let mut recortada = false;

        while decision.intervention.help_level() > max_help_level {
            let Some(menor) = decision.intervention.downgrade() else {
                break;
            };
            decision.intervention = menor;
            recortada = true;
        }

        if recortada {
            // Si bajamos el nivel, el mensaje original ya no corresponde: puede
            // seguir conteniendo la solucion completa. Se sustituye por algo
            // acorde y se obliga a preguntar de vuelta.
            decision.message = format!(
                "Voy a ir mas despacio para que lo resuelvas tu. Trabajemos sobre `{}`.",
                decision.concept
            );
            if decision.question.is_none() {
                decision.question =
                    Some("Que crees que esta pasando en la linea del error?".to_string());
            }
            // La confianza del modelo no sobrevive a una respuesta recortada.
            decision.confidence = decision.confidence.min(0.5);
        }

        (decision, recortada)
    }

    /// Techo de ayuda ya resuelto para una peticion concreta.
    pub fn apply_to(&self, request: &TutorRequest) -> u8 {
        self.max_help_level(
            &request.context,
            &request.profile,
            request.looks_vague(),
            None,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peticion_clara() -> TutorRequest {
        TutorRequest::new(
            "me sale NullPointerException en la linea 23, que significa?",
            ProjectContext::fake_java_null_pointer(),
            StudentProfile::fake_intermediate_null_safety(),
        )
    }

    #[test]
    fn sin_evidencia_solo_se_puede_preguntar() {
        let politica = TeachingPolicy::default();
        let nivel = politica.max_help_level(
            &ProjectContext::fake_ambiguous(),
            &StudentProfile::fake_beginner(),
            true,
            None,
        );
        assert_eq!(nivel, Intervention::Clarify.help_level());
    }

    #[test]
    fn un_avanzado_recibe_menos_ayuda_que_un_principiante() {
        let politica = TeachingPolicy::default();
        let contexto = ProjectContext::fake_java_null_pointer();

        let mut avanzado = StudentProfile::fake_beginner();
        avanzado.level = SkillLevel::Advanced;

        let nivel_principiante =
            politica.max_help_level(&contexto, &StudentProfile::fake_beginner(), false, None);
        let nivel_avanzado = politica.max_help_level(&contexto, &avanzado, false, None);

        assert!(nivel_avanzado < nivel_principiante);
    }

    #[test]
    fn nunca_se_pasa_del_techo() {
        let politica = TeachingPolicy::socratica();
        let nivel = politica.max_help_level(
            &ProjectContext::fake_java_null_pointer(),
            &StudentProfile::fake_beginner(),
            false,
            None,
        );
        assert!(nivel <= Intervention::Hint.help_level());
    }

    #[test]
    fn recorta_una_solucion_completa_hasta_el_techo() {
        let politica = TeachingPolicy::default();
        let excesiva = TutorDecision {
            concept: "null-safety".to_string(),
            intervention: Intervention::GuidedSolution,
            message: "return Optional.ofNullable(u)...".to_string(),
            question: None,
            confidence: 0.95,
        };

        let (ajustada, recortada) = politica.enforce(excesiva, Intervention::Hint.help_level());

        assert!(recortada);
        assert_eq!(ajustada.intervention, Intervention::Hint);
        // Lo importante: la solucion literal ya no llega al alumno.
        assert!(!ajustada.message.contains("Optional.ofNullable"));
        assert!(ajustada.question.is_some());
        assert!(ajustada.validate().is_ok());
    }

    #[test]
    fn una_decision_dentro_del_techo_pasa_intacta() {
        let politica = TeachingPolicy::default();
        let original = TutorDecision::fake_hint_optional();
        let (ajustada, recortada) =
            politica.enforce(original.clone(), Intervention::Pseudocode.help_level());
        assert!(!recortada);
        assert_eq!(ajustada, original);
    }

    #[test]
    fn una_peticion_clara_permite_algo_mas_que_preguntar() {
        let politica = TeachingPolicy::default();
        assert!(politica.apply_to(&peticion_clara()) > Intervention::Clarify.help_level());
    }
}
