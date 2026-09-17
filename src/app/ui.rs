//! Presentacion en terminal.
//!
//! Dos reglas de compatibilidad:
//!
//! 1. **Ningun dato critico depende del color ni de glifos.** Si la terminal no
//!    los soporta, el texto sigue siendo completo y legible.
//! 2. SeniorCLI no debe parecer una ventana de chat: siempre muestra que
//!    evidencia uso y que tipo de intervencion esta dando.
//!
//! Las funciones devuelven `String` en vez de imprimir, para poder probarlas.

use crate::contracts::{ProjectContext, StudentProfile, TutorDecision};

const ANCHO: usize = 72;

/// Linea separadora.
pub fn separador() -> String {
    "-".repeat(ANCHO)
}

/// Encabezado de una seccion.
pub fn encabezado(titulo: &str) -> String {
    format!("{}\n{titulo}\n{}", separador(), separador())
}

/// Resumen de la evidencia que SeniorCLI esta usando.
///
/// Esto es lo que lo separa de un chat: el alumno ve **sobre que** se esta
/// razonando antes de leer la respuesta.
pub fn render_evidencia(context: &ProjectContext) -> String {
    let mut salida = String::from("Contexto que estoy usando:\n");

    salida.push_str(&format!("  Stack: {}\n", context.stack.active.as_str()));

    match context.first_error() {
        Some(d) => {
            let ubicacion = match (&d.file, d.line) {
                (Some(f), Some(l)) => format!("{}:{l}", f.display()),
                (Some(f), None) => f.display().to_string(),
                _ => "sin ubicacion".to_string(),
            };
            salida.push_str(&format!(
                "  Error: [{ubicacion}] {}\n",
                primera_linea(&d.message)
            ));
        }
        None => salida.push_str("  Error: ninguno detectado\n"),
    }

    if context.relevant_files.is_empty() {
        salida.push_str("  Archivos: ninguno\n");
    } else {
        let archivos: Vec<String> = context
            .relevant_files
            .iter()
            .map(|f| f.path.display().to_string())
            .collect();
        salida.push_str(&format!("  Archivos: {}\n", archivos.join(", ")));
    }

    if context.truncated {
        salida.push_str("  (se recorto parte del contexto por tamano)\n");
    }
    if let Some(ambiguedad) = &context.ambiguity {
        salida.push_str(&format!("  Falta informacion: {}\n", ambiguedad.reason));
    }

    salida
}

/// Renderiza la intervencion del tutor.
pub fn render_decision(decision: &TutorDecision) -> String {
    let mut salida = String::new();

    // El tipo de intervencion y la confianza van en texto, no en color.
    salida.push_str(&format!(
        "[{}] concepto: {} | confianza: {:.0}%\n",
        decision.intervention.as_str(),
        decision.concept,
        decision.confidence * 100.0
    ));
    salida.push_str(&separador());
    salida.push('\n');
    salida.push_str(&envolver(&decision.message, ANCHO));
    salida.push('\n');

    if let Some(pregunta) = &decision.question {
        salida.push('\n');
        salida.push_str(&envolver(&format!("Pregunta: {pregunta}"), ANCHO));
        salida.push('\n');
    }

    if decision.confidence < 0.5 {
        salida.push_str("\nNota: no estoy seguro de esto. Verificalo contra el compilador.\n");
    }

    salida
}

/// Vista de `senior progress`.
pub fn render_progress(profile: &StudentProfile) -> String {
    let mut salida = encabezado("Tu progreso");
    salida.push('\n');
    salida.push_str(&format!("Nivel: {}\n", profile.level.as_str()));

    if profile.topics.is_empty() {
        salida.push_str("\nTodavia no hay temas registrados. Usa `senior learn` para empezar.\n");
        return salida;
    }

    salida.push_str("\nTemas:\n");
    let mut temas: Vec<_> = profile.topics.iter().collect();
    temas.sort_by(|a, b| {
        b.mastery
            .partial_cmp(&a.mastery)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for tema in temas {
        let confiable = if tema.is_reliable() {
            ""
        } else {
            " (pocas evidencias)"
        };
        salida.push_str(&format!(
            "  {:<24} {:>3}%  {}{confiable}\n",
            tema.topic,
            (tema.mastery * 100.0).round() as i32,
            barra(tema.mastery),
        ));
    }

    if !profile.recent_errors.is_empty() {
        salida.push_str("\nErrores recientes:\n");
        for error in profile.recent_errors.iter().take(5) {
            salida.push_str(&format!("  - {}\n", primera_linea(error)));
        }
    }

    salida
}

/// Traduce un error tecnico a algo accionable para el alumno.
pub fn render_error(error: &crate::contracts::SeniorError) -> String {
    let mut salida = format!(
        "No pude completar la operacion.\n\n{}\n",
        error.user_message()
    );
    if error.is_retryable() {
        salida.push_str("\nPuedes reintentar con el mismo comando.\n");
    }
    salida
}

/// Barra de progreso en ASCII puro: legible en cualquier terminal.
fn barra(valor: f32) -> String {
    const ANCHO_BARRA: usize = 20;
    let lleno = (valor.clamp(0.0, 1.0) * ANCHO_BARRA as f32).round() as usize;
    format!("[{}{}]", "#".repeat(lleno), ".".repeat(ANCHO_BARRA - lleno))
}

fn primera_linea(texto: &str) -> &str {
    texto.lines().next().unwrap_or(texto)
}

/// Ajuste de linea por palabras, respetando los saltos que ya trae el texto.
fn envolver(texto: &str, ancho: usize) -> String {
    let mut salida = Vec::new();

    for parrafo in texto.split('\n') {
        let mut linea = String::new();
        for palabra in parrafo.split_whitespace() {
            if !linea.is_empty() && linea.chars().count() + 1 + palabra.chars().count() > ancho {
                salida.push(std::mem::take(&mut linea));
            }
            if !linea.is_empty() {
                linea.push(' ');
            }
            linea.push_str(palabra);
        }
        salida.push(linea);
    }

    salida.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn la_decision_muestra_el_tipo_de_intervencion() {
        let salida = render_decision(&TutorDecision::fake_hint_optional());
        assert!(salida.contains("[pista]"));
        assert!(salida.contains("null-safety"));
        assert!(salida.contains("Pregunta:"));
    }

    #[test]
    fn avisa_cuando_la_confianza_es_baja() {
        let salida = render_decision(&TutorDecision::safe_fallback("timeout"));
        assert!(salida.contains("no estoy seguro"));
    }

    #[test]
    fn la_evidencia_se_muestra_antes_de_la_respuesta() {
        let salida = render_evidencia(&ProjectContext::fake_java_null_pointer());
        assert!(salida.contains("Java"));
        assert!(salida.contains("UserService.java:23"));
    }

    #[test]
    fn la_evidencia_avisa_cuando_falta_informacion() {
        let salida = render_evidencia(&ProjectContext::fake_ambiguous());
        assert!(salida.contains("Falta informacion"));
    }

    #[test]
    fn la_salida_es_ascii_puro() {
        // Requisito de compatibilidad: nada critico depende de glifos raros.
        let salida = render_progress(&StudentProfile::fake_intermediate_null_safety());
        assert!(
            salida.is_ascii(),
            "la salida debe ser legible en cualquier terminal"
        );
    }

    #[test]
    fn el_progreso_marca_los_temas_con_pocas_evidencias() {
        let mut profile = StudentProfile::fake_beginner();
        profile
            .topics
            .push(crate::contracts::TopicMastery::new("ownership"));
        let salida = render_progress(&profile);
        assert!(salida.contains("pocas evidencias"));
    }

    #[test]
    fn el_ajuste_de_linea_respeta_el_ancho() {
        let largo = "palabra ".repeat(40);
        for linea in envolver(&largo, 30).lines() {
            assert!(linea.chars().count() <= 30);
        }
    }

    #[test]
    fn el_error_se_traduce_a_algo_accionable() {
        let error = crate::contracts::SeniorError::Provider("429".to_string());
        let salida = render_error(&error);
        assert!(salida.contains("reintentar"));
        assert!(!salida.contains("429"));
    }
}
