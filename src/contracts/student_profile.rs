//! `StudentProfile`: la memoria estructurada del alumno.
//!
//! Regla del proyecto: la memoria del estudiante es **estructurada y
//! verificable**; no depende del historial de conversacion. Si el historial se
//! pierde o se resume, el perfil sigue siendo la fuente de verdad.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Version del esquema en disco. Subirla obliga a escribir una migracion en
/// `app::storage`. Nunca sobrescribir un perfil valido con uno parcial.
pub const PROFILE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SkillLevel {
    Beginner,
    Intermediate,
    Advanced,
}

impl SkillLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Beginner => "principiante",
            Self::Intermediate => "intermedio",
            Self::Advanced => "avanzado",
        }
    }
}

/// Dominio del alumno sobre un tema concreto.
///
/// `mastery` va de 0.0 a 1.0 y **nunca** se mueve por una sola respuesta:
/// hacen falta varias evidencias, y puede bajar si aparecen fallos posteriores.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopicMastery {
    pub topic: String,
    pub mastery: f32,
    /// Cuantas evidencias respaldan este valor.
    pub evidence_count: u32,
    pub last_seen: DateTime<Utc>,
}

impl TopicMastery {
    pub fn new(topic: impl Into<String>) -> Self {
        Self {
            topic: topic.into(),
            mastery: 0.0,
            evidence_count: 0,
            last_seen: Utc::now(),
        }
    }

    /// `true` si el valor descansa en suficientes evidencias como para confiar.
    pub fn is_reliable(&self) -> bool {
        self.evidence_count >= 3
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StudentProfile {
    pub schema_version: u32,
    pub student_id: String,
    pub level: SkillLevel,
    pub topics: Vec<TopicMastery>,
    /// Errores recientes, del mas nuevo al mas viejo. Acotado.
    pub recent_errors: Vec<String>,
    pub updated_at: DateTime<Utc>,
}

impl StudentProfile {
    /// Cuantos errores recientes conservamos antes de olvidar los viejos.
    pub const MAX_RECENT_ERRORS: usize = 20;

    pub fn new(student_id: impl Into<String>) -> Self {
        Self {
            schema_version: PROFILE_SCHEMA_VERSION,
            student_id: student_id.into(),
            level: SkillLevel::Beginner,
            topics: Vec::new(),
            recent_errors: Vec::new(),
            updated_at: Utc::now(),
        }
    }

    pub fn topic(&self, topic: &str) -> Option<&TopicMastery> {
        self.topics.iter().find(|t| t.topic == topic)
    }

    pub fn topic_mut(&mut self, topic: &str) -> Option<&mut TopicMastery> {
        self.topics.iter_mut().find(|t| t.topic == topic)
    }

    /// Registra un error visto, sin duplicar el ultimo y sin crecer sin limite.
    pub fn push_recent_error(&mut self, error: impl Into<String>) {
        let error = error.into();
        if self.recent_errors.first() == Some(&error) {
            return;
        }
        self.recent_errors.insert(0, error);
        self.recent_errors.truncate(Self::MAX_RECENT_ERRORS);
        self.updated_at = Utc::now();
    }

    /// Validacion al cargar desde disco. Un perfil que no pasa esto se trata
    /// como corrupto y se restaura desde el respaldo.
    pub fn validate(&self) -> Result<(), String> {
        if self.student_id.trim().is_empty() {
            return Err("student_id vacio".to_string());
        }
        if self.schema_version == 0 || self.schema_version > PROFILE_SCHEMA_VERSION {
            return Err(format!(
                "schema_version desconocida: {}",
                self.schema_version
            ));
        }
        if let Some(t) = self
            .topics
            .iter()
            .find(|t| !(0.0..=1.0).contains(&t.mastery))
        {
            return Err(format!("mastery fuera de rango en el tema `{}`", t.topic));
        }
        Ok(())
    }

    // --- Mocks compartidos ---------------------------------------------------

    /// Alumno principiante sin historial. Perfil por defecto de un `senior init`.
    pub fn fake_beginner() -> Self {
        Self::new("alumno-demo")
    }

    /// Alumno intermedio que ya vio el tema `null-safety` pero aun falla.
    pub fn fake_intermediate_null_safety() -> Self {
        let mut profile = Self::new("alumno-demo");
        profile.level = SkillLevel::Intermediate;
        profile.topics = vec![TopicMastery {
            topic: "null-safety".to_string(),
            mastery: 0.45,
            evidence_count: 4,
            last_seen: Utc::now(),
        }];
        profile.recent_errors = vec!["java.lang.NullPointerException".to_string()];
        profile
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn un_perfil_nuevo_es_valido() {
        assert!(StudentProfile::fake_beginner().validate().is_ok());
    }

    #[test]
    fn rechaza_mastery_fuera_de_rango() {
        let mut profile = StudentProfile::fake_beginner();
        profile.topics.push(TopicMastery {
            topic: "ownership".to_string(),
            mastery: 1.5,
            evidence_count: 1,
            last_seen: Utc::now(),
        });
        assert!(profile.validate().is_err());
    }

    #[test]
    fn los_errores_recientes_no_crecen_sin_limite_ni_se_duplican() {
        let mut profile = StudentProfile::fake_beginner();
        profile.push_recent_error("E0308");
        profile.push_recent_error("E0308");
        assert_eq!(profile.recent_errors.len(), 1);

        for i in 0..50 {
            profile.push_recent_error(format!("error-{i}"));
        }
        assert_eq!(
            profile.recent_errors.len(),
            StudentProfile::MAX_RECENT_ERRORS
        );
    }
}
