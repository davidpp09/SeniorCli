//! Presentacion en terminal.
//!
//! Dos reglas de compatibilidad:
//!
//! 1. **Ningun dato critico depende del color ni de glifos.** Si la terminal no
//!    los soporta, el texto sigue siendo completo y legible.
//! 2. SeniorCLI no debe parecer una ventana de chat: siempre muestra que
//!    evidencia uso y que tipo de intervencion esta dando.
//!
//! La regla 2 no obliga a ser aspero. La evidencia se cuenta en lenguaje llano
//! y la etiqueta de la intervencion va al final, como una nota al pie: informa
//! sin interrumpir la lectura.
//!
//! Las funciones devuelven `String` en vez de imprimir, para poder probarlas.

use std::path::Path;

use crate::contracts::{
    Intervention, ProjectContext, RelevanceReason, Severity, StudentProfile, TutorDecision,
};

const ANCHO: usize = 72;

/// Linea separadora.
pub fn separador() -> String {
    "-".repeat(ANCHO)
}

/// Encabezado de una seccion.
pub fn encabezado(titulo: &str) -> String {
    format!("{}\n{titulo}\n{}", separador(), separador())
}

/// Pantalla de bienvenida de la sesion interactiva.
///
/// Dice de entrada las tres cosas que evitan que el alumno se sienta perdido:
/// sobre que proyecto trabaja, quien le va a responder y hasta donde puede
/// llegar la ayuda.
pub fn bienvenida(proyecto: &str, raiz: &Path, proveedor: &str, techo: Intervention) -> String {
    let mut salida = encabezado(&format!(
        "SeniorCLI {} - sesion interactiva",
        env!("CARGO_PKG_VERSION")
    ));
    salida.push('\n');

    salida.push_str(&format!("Proyecto:   {proyecto}\n"));
    salida.push_str(&format!("Carpeta:    {}\n", raiz.display()));

    // Decirlo claro evita la peor confusion posible: respuestas de ejemplo que
    // parecen venir de una IA de verdad.
    let modelo = if proveedor == "mock" {
        "mock (respuestas de ejemplo: todavia no hay una IA conectada)"
    } else {
        proveedor
    };
    salida.push_str(&format!("Modelo:     {modelo}\n"));
    salida.push_str(&format!("Ayuda max:  {}\n", techo.as_str()));

    salida.push('\n');
    salida.push_str(&envolver(
        "Escribe tu duda y pulsa Enter. Te respondo con pistas y preguntas, no con la solucion hecha: la idea es que la encuentres tu.",
        ANCHO,
    ));
    salida.push_str("\n\n");
    salida.push_str("Escribe /ayuda para ver los comandos, /salir para terminar.\n");

    salida
}

/// Comandos disponibles dentro de la sesion.
pub fn ayuda() -> String {
    let mut salida = String::from("Comandos de la sesion:\n");
    salida.push_str("  /contexto   que estoy mirando de tu proyecto ahora mismo\n");
    salida.push_str("  /analizar   volver a mirarlo (usalo despues de cambiar codigo)\n");
    salida.push_str("  /progreso   tu nivel y los temas que llevas trabajados\n");
    salida.push_str("  /ayuda      esta lista\n");
    salida.push_str("  /salir      terminar la sesion\n");
    salida.push('\n');
    salida.push_str("Cualquier otra cosa que escribas es una duda para el tutor.\n");
    salida.push_str("Cada turno se guarda al momento: si se cierra, no pierdes nada.\n");
    salida
}

/// Cierre de la sesion: donde quedo lo trabajado.
pub fn despedida(turnos: usize, ruta: &Path) -> String {
    if turnos == 0 {
        return "Sesion terminada. No hubo turnos que guardar.\n".to_string();
    }
    let plural = if turnos == 1 { "turno" } else { "turnos" };
    format!(
        "Sesion terminada: {turnos} {plural} en\n  {}\nRetomala cuando quieras escribiendo `senior`.\n",
        ruta.display()
    )
}

/// Resumen de la evidencia que SeniorCLI esta usando.
///
/// Esto es lo que lo separa de un chat: el alumno ve **sobre que** se esta
/// razonando antes de leer la respuesta. Va en lenguaje llano, no como un
/// volcado de estructuras.
pub fn render_evidencia(context: &ProjectContext) -> String {
    let mut salida = String::from("Esto es lo que mire de tu proyecto:\n");

    salida.push_str(&format!("  Lenguaje:  {}\n", context.stack.active.as_str()));

    let errores = context
        .diagnostics
        .iter()
        .filter(|d| d.severity == Severity::Error)
        .count();

    match context.first_error() {
        Some(d) => {
            let ubicacion = match (&d.file, d.line) {
                (Some(f), Some(l)) => format!(" en {}:{l}", f.display()),
                (Some(f), None) => format!(" en {}", f.display()),
                _ => String::new(),
            };
            let plural = if errores == 1 { "error" } else { "errores" };
            salida.push_str(&format!("  Compilar:  {errores} {plural}{ubicacion}\n"));
            salida.push_str(&format!("             {}\n", primera_linea(&d.message)));
        }
        None => salida.push_str("  Compilar:  sin errores\n"),
    }

    if context.relevant_files.is_empty() {
        salida.push_str("  Archivos:  ninguno\n");
    } else {
        salida.push_str("  Archivos:\n");
        for archivo in &context.relevant_files {
            salida.push_str(&format!(
                "             {} ({})\n",
                archivo.path.display(),
                motivo(archivo.reason)
            ));
        }
    }

    if context.truncated {
        salida.push_str("  (recorte parte del contexto por tamano)\n");
    }

    if let Some(ambiguedad) = &context.ambiguity {
        salida.push('\n');
        salida.push_str(&envolver(
            &format!(
                "Todavia no se de que hablarte: {}. Cuentame tu que paso.",
                ambiguedad.reason.to_lowercase()
            ),
            ANCHO,
        ));
        salida.push('\n');
        for falta in &ambiguedad.missing {
            salida.push_str(&format!("  - {falta}\n"));
        }
    }

    salida
}

/// Por que un archivo entro al contexto, dicho en cristiano.
fn motivo(reason: RelevanceReason) -> &'static str {
    match reason {
        RelevanceReason::Diagnostic => "sale en el error",
        RelevanceReason::GitModified => "lo cambiaste y no lo has commiteado",
        RelevanceReason::Requested => "lo pediste tu",
        RelevanceReason::RelatedSymbol => "lo usa el codigo del error",
    }
}

/// Renderiza la intervencion del tutor.
pub fn render_decision(decision: &TutorDecision) -> String {
    let mut salida = envolver(&decision.message, ANCHO);
    salida.push('\n');

    if let Some(pregunta) = &decision.question {
        salida.push('\n');
        salida.push_str(&envolver(&format!("Pregunta: {pregunta}"), ANCHO));
        salida.push('\n');
    }

    if decision.confidence < 0.5 {
        salida.push_str("\nNota: no estoy seguro de esto. Verificalo contra el compilador.\n");
    }

    // La etiqueta va al final, en texto plano: el alumno siempre sabe que tipo
    // de ayuda recibio, pero lo primero que lee es la respuesta.
    salida.push_str(&format!(
        "\n  -- {} sobre `{}`, confianza {:.0}% --\n",
        decision.intervention.as_str(),
        decision.concept,
        decision.confidence * 100.0
    ));

    salida
}

/// Vista de `senior progress`.
pub fn render_progress(profile: &StudentProfile) -> String {
    let mut salida = encabezado("Tu progreso");
    salida.push('\n');
    salida.push_str(&format!("Nivel: {}\n", profile.level.as_str()));

    if profile.topics.is_empty() {
        salida.push_str("\nTodavia no hay temas registrados. Escribe `senior` y pregunta algo.\n");
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
        assert!(salida.contains("pista"));
        assert!(salida.contains("null-safety"));
        assert!(salida.contains("Pregunta:"));
    }

    #[test]
    fn la_respuesta_va_antes_que_la_etiqueta() {
        // El alumno lee primero lo que le sirve, no los metadatos.
        let salida = render_decision(&TutorDecision::fake_hint_optional());
        let mensaje = salida.find("El error dice").expect("esta el mensaje");
        let etiqueta = salida.find("-- pista").expect("esta la etiqueta");
        assert!(mensaje < etiqueta);
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
    fn la_evidencia_explica_por_que_entro_cada_archivo() {
        let salida = render_evidencia(&ProjectContext::fake_java_null_pointer());
        assert!(salida.contains("sale en el error"));
    }

    #[test]
    fn la_evidencia_avisa_cuando_falta_informacion() {
        let salida = render_evidencia(&ProjectContext::fake_ambiguous());
        assert!(salida.contains("Todavia no se de que hablarte"));
        assert!(salida.contains("que comportamiento esperabas"));
    }

    #[test]
    fn la_bienvenida_avisa_que_el_mock_no_es_una_ia() {
        let salida = bienvenida(
            "demo",
            Path::new("/proyecto/demo"),
            "mock",
            Intervention::Pseudocode,
        );
        assert!(salida.contains("todavia no hay una IA conectada"));
        assert!(salida.contains("pseudocodigo"));
    }

    #[test]
    fn la_despedida_dice_donde_quedo_la_sesion() {
        let ruta = Path::new(".senior/sessions/hoy.jsonl");
        assert!(despedida(0, ruta).contains("No hubo turnos"));
        assert!(despedida(1, ruta).contains("1 turno "));
        assert!(despedida(3, ruta).contains("hoy.jsonl"));
    }

    #[test]
    fn la_salida_es_ascii_puro() {
        // Requisito de compatibilidad: nada critico depende de glifos raros.
        let salida = render_progress(&StudentProfile::fake_intermediate_null_safety());
        assert!(
            salida.is_ascii(),
            "la salida debe ser legible en cualquier terminal"
        );

        let sesion = bienvenida("demo", Path::new("/demo"), "mock", Intervention::Hint);
        assert!(sesion.is_ascii(), "la bienvenida debe ser ASCII");
        assert!(ayuda().is_ascii(), "la ayuda debe ser ASCII");
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
