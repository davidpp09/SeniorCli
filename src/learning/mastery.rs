//! Actualizacion del dominio del alumno.
//!
//! Regla: el mastery **no sube por una sola respuesta**. Se mueve con
//! evidencias acumuladas y puede bajar si el alumno vuelve a fallar en el mismo
//! concepto. Un perfil inflado es peor que un perfil vacio, porque hace que el
//! harness ayude de menos.

use chrono::Utc;

use crate::contracts::{Intervention, SkillLevel, StudentProfile, TopicMastery};

/// Que observamos sobre el alumno en un turno.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evidence {
    /// Resolvio el problema sin que le dieramos la solucion.
    ResolvioSolo,
    /// Contesto bien una pregunta socratica.
    RespondioCorrecto,
    /// Contesto mal o se quedo atorado.
    RespondioIncorrecto,
    /// Necesito que le entregaramos codigo o la solucion guiada.
    NecesitoSolucion,
    /// Volvio a caer en el mismo error poco despues.
    RepitioElError,
}

impl Evidence {
    /// Cuanto mueve el dominio, en la escala 0.0 - 1.0.
    ///
    /// Los pasos positivos son pequenos y los negativos mas grandes a
    /// proposito: es mas barato subestimar al alumno que sobreestimarlo.
    fn delta(self) -> f32 {
        match self {
            Self::ResolvioSolo => 0.15,
            Self::RespondioCorrecto => 0.08,
            Self::RespondioIncorrecto => -0.10,
            Self::NecesitoSolucion => -0.05,
            Self::RepitioElError => -0.20,
        }
    }

    /// Deduce la evidencia a partir del tipo de ayuda que hizo falta.
    pub fn from_intervention(intervention: Intervention) -> Self {
        match intervention {
            Intervention::Clarify | Intervention::SocraticQuestion => Self::RespondioCorrecto,
            Intervention::Hint | Intervention::Explanation | Intervention::Example => {
                Self::RespondioIncorrecto
            }
            Intervention::Pseudocode | Intervention::PartialCode | Intervention::GuidedSolution => {
                Self::NecesitoSolucion
            }
        }
    }
}

/// Cuantas evidencias hacen falta antes de que el valor cuente como confiable.
pub const EVIDENCIAS_MINIMAS: u32 = 3;

/// Registra una evidencia sobre un concepto y devuelve el nuevo dominio.
///
/// Crea el tema si no existia. El valor siempre queda dentro de 0.0 - 1.0.
pub fn record_evidence(profile: &mut StudentProfile, concept: &str, evidence: Evidence) -> f32 {
    if profile.topic(concept).is_none() {
        profile.topics.push(TopicMastery::new(concept));
    }

    let nuevo = {
        let tema = profile.topic_mut(concept).expect("recien insertado");
        tema.mastery = (tema.mastery + evidence.delta()).clamp(0.0, 1.0);
        tema.evidence_count += 1;
        tema.last_seen = Utc::now();
        tema.mastery
    };

    profile.updated_at = Utc::now();
    reevaluar_nivel(profile);
    nuevo
}

/// Ajusta el nivel global del alumno a partir de sus temas confiables.
///
/// Solo cuentan los temas con suficientes evidencias: un tema visto una vez no
/// debe mover el nivel general.
fn reevaluar_nivel(profile: &mut StudentProfile) {
    let confiables: Vec<f32> = profile
        .topics
        .iter()
        .filter(|t| t.is_reliable())
        .map(|t| t.mastery)
        .collect();

    if confiables.len() < 2 {
        return;
    }

    let promedio = confiables.iter().sum::<f32>() / confiables.len() as f32;
    profile.level = if promedio >= 0.75 {
        SkillLevel::Advanced
    } else if promedio >= 0.45 {
        SkillLevel::Intermediate
    } else {
        SkillLevel::Beginner
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn una_sola_respuesta_no_vuelve_confiable_un_tema() {
        let mut profile = StudentProfile::fake_beginner();
        record_evidence(&mut profile, "ownership", Evidence::RespondioCorrecto);
        let tema = profile.topic("ownership").expect("existe");
        assert!(!tema.is_reliable());
        assert_eq!(tema.evidence_count, 1);
    }

    #[test]
    fn el_dominio_sube_con_evidencias_acumuladas() {
        let mut profile = StudentProfile::fake_beginner();
        for _ in 0..EVIDENCIAS_MINIMAS {
            record_evidence(&mut profile, "ownership", Evidence::ResolvioSolo);
        }
        let tema = profile.topic("ownership").expect("existe");
        assert!(tema.is_reliable());
        assert!(tema.mastery > 0.4);
    }

    #[test]
    fn el_dominio_baja_al_repetir_el_error() {
        let mut profile = StudentProfile::fake_beginner();
        record_evidence(&mut profile, "null-safety", Evidence::ResolvioSolo);
        let antes = profile.topic("null-safety").expect("existe").mastery;
        record_evidence(&mut profile, "null-safety", Evidence::RepitioElError);
        let despues = profile.topic("null-safety").expect("existe").mastery;
        assert!(despues < antes);
    }

    #[test]
    fn el_dominio_nunca_se_sale_del_rango() {
        let mut profile = StudentProfile::fake_beginner();
        for _ in 0..50 {
            record_evidence(&mut profile, "loops", Evidence::ResolvioSolo);
        }
        assert!(profile.topic("loops").expect("existe").mastery <= 1.0);

        for _ in 0..50 {
            record_evidence(&mut profile, "loops", Evidence::RepitioElError);
        }
        assert!(profile.topic("loops").expect("existe").mastery >= 0.0);
        assert!(profile.validate().is_ok());
    }

    #[test]
    fn el_nivel_no_se_mueve_con_un_solo_tema() {
        let mut profile = StudentProfile::fake_beginner();
        for _ in 0..10 {
            record_evidence(&mut profile, "loops", Evidence::ResolvioSolo);
        }
        assert_eq!(profile.level, SkillLevel::Beginner);
    }

    #[test]
    fn el_nivel_sube_con_varios_temas_dominados() {
        let mut profile = StudentProfile::fake_beginner();
        for tema in ["loops", "funciones", "colecciones"] {
            for _ in 0..8 {
                record_evidence(&mut profile, tema, Evidence::ResolvioSolo);
            }
        }
        assert_eq!(profile.level, SkillLevel::Advanced);
    }
}
