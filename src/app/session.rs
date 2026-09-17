//! Sesiones de aprendizaje, guardadas turno a turno.
//!
//! Nada se guarda "al final": cada turno se escribe apenas ocurre, en formato
//! JSONL. Si la CLI muere a media sesion, lo anterior sigue ahi y se puede
//! retomar.

use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::contracts::{Result, SeniorError, TutorDecision};

/// Un turno completo: lo que dijo el alumno y lo que respondio SeniorCLI.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Turn {
    pub at: DateTime<Utc>,
    pub student_message: String,
    pub decision: TutorDecision,
    /// Respuesta del alumno a la pregunta de vuelta, si la dio.
    pub student_reply: Option<String>,
    /// Latencia del turno, para las metricas.
    pub latency_ms: u128,
}

impl Turn {
    pub fn new(student_message: impl Into<String>, decision: TutorDecision) -> Self {
        Self {
            at: Utc::now(),
            student_message: student_message.into(),
            decision,
            student_reply: None,
            latency_ms: 0,
        }
    }

    pub fn with_latency(mut self, latency_ms: u128) -> Self {
        self.latency_ms = latency_ms;
        self
    }

    pub fn with_reply(mut self, reply: impl Into<String>) -> Self {
        self.student_reply = Some(reply.into());
        self
    }
}

/// Estado de una sesion viva.
#[derive(Debug, Clone)]
pub struct SessionState {
    path: PathBuf,
    started_at: DateTime<Utc>,
    turns: Vec<Turn>,
}

impl SessionState {
    /// Abre una sesion nueva sobre el archivo dado.
    pub fn start(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            started_at: Utc::now(),
            turns: Vec::new(),
        }
    }

    /// Retoma una sesion existente leyendo su JSONL.
    ///
    /// Una linea corrupta se descarta con un aviso: perder un turno es mejor
    /// que perder la sesion entera.
    pub fn resume(path: impl Into<PathBuf>) -> Result<Self> {
        let path = path.into();
        let texto = std::fs::read_to_string(&path).map_err(|e| SeniorError::Storage {
            path: path.clone(),
            source: e,
        })?;

        let mut turns = Vec::new();
        for (numero, linea) in texto.lines().enumerate() {
            if linea.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<Turn>(linea) {
                Ok(turno) => turns.push(turno),
                Err(e) => tracing::warn!("turno ilegible en la linea {}: {e}", numero + 1),
            }
        }

        let started_at = turns.first().map_or_else(Utc::now, |t| t.at);
        Ok(Self {
            path,
            started_at,
            turns,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn started_at(&self) -> DateTime<Utc> {
        self.started_at
    }

    pub fn turns(&self) -> &[Turn] {
        &self.turns
    }

    pub fn len(&self) -> usize {
        self.turns.len()
    }

    pub fn is_empty(&self) -> bool {
        self.turns.is_empty()
    }

    pub fn last_turn(&self) -> Option<&Turn> {
        self.turns.last()
    }

    /// Agrega un turno y lo escribe a disco de inmediato.
    pub fn record(&mut self, turn: Turn) -> Result<()> {
        let linea = serde_json::to_string(&turn)?;
        self.append_line(&linea)?;
        self.turns.push(turn);
        Ok(())
    }

    fn append_line(&self, linea: &str) -> Result<()> {
        if let Some(padre) = self.path.parent() {
            std::fs::create_dir_all(padre).map_err(|e| SeniorError::Storage {
                path: padre.to_path_buf(),
                source: e,
            })?;
        }

        let mut archivo = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|e| SeniorError::Storage {
                path: self.path.clone(),
                source: e,
            })?;

        writeln!(archivo, "{linea}").map_err(|e| SeniorError::Storage {
            path: self.path.clone(),
            source: e,
        })?;
        // Bajar a disco ya: un Ctrl+C no debe llevarse el ultimo turno.
        archivo.flush().map_err(|e| SeniorError::Storage {
            path: self.path.clone(),
            source: e,
        })
    }

    /// Resumen corto para `senior progress`.
    pub fn summary(&self) -> String {
        if self.turns.is_empty() {
            return "sesion vacia".to_string();
        }
        let conceptos: Vec<&str> = self
            .turns
            .iter()
            .map(|t| t.decision.concept.as_str())
            .collect();
        format!(
            "{} turnos, conceptos: {}",
            self.turns.len(),
            conceptos.join(", ")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cada_turno_se_escribe_al_momento() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ruta = dir.path().join("sesion.jsonl");

        let mut sesion = SessionState::start(&ruta);
        sesion
            .record(Turn::new(
                "por que falla?",
                TutorDecision::fake_hint_optional(),
            ))
            .expect("guarda turno");

        // El archivo ya tiene el turno, sin haber cerrado la sesion.
        let contenido = std::fs::read_to_string(&ruta).expect("lee");
        assert_eq!(contenido.lines().count(), 1);
        assert!(contenido.contains("null-safety"));
    }

    #[test]
    fn una_sesion_se_puede_retomar_despues_de_un_cierre() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ruta = dir.path().join("sesion.jsonl");

        let mut original = SessionState::start(&ruta);
        original
            .record(Turn::new("uno", TutorDecision::fake_hint_optional()))
            .expect("t1");
        original
            .record(Turn::new("dos", TutorDecision::fake_clarify()))
            .expect("t2");
        drop(original); // simula el cierre de la CLI

        let retomada = SessionState::resume(&ruta).expect("retoma");
        assert_eq!(retomada.len(), 2);
        assert_eq!(
            retomada.last_turn().expect("hay turno").student_message,
            "dos"
        );
    }

    #[test]
    fn una_linea_corrupta_no_tumba_la_sesion() {
        let dir = tempfile::tempdir().expect("tempdir");
        let ruta = dir.path().join("sesion.jsonl");

        let mut sesion = SessionState::start(&ruta);
        sesion
            .record(Turn::new("bueno", TutorDecision::fake_hint_optional()))
            .expect("t1");
        std::fs::OpenOptions::new()
            .append(true)
            .open(&ruta)
            .and_then(|mut f| writeln!(f, "{{ basura"))
            .expect("escribe basura");

        let retomada = SessionState::resume(&ruta).expect("retoma");
        assert_eq!(retomada.len(), 1);
    }

    #[test]
    fn el_resumen_lista_los_conceptos() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut sesion = SessionState::start(dir.path().join("s.jsonl"));
        assert_eq!(sesion.summary(), "sesion vacia");

        sesion
            .record(Turn::new("x", TutorDecision::fake_hint_optional()))
            .expect("t1");
        assert!(sesion.summary().contains("null-safety"));
    }
}
