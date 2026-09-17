//! Parseo de errores del compilador, los tests y el linter.
//!
//! Esto es lo que convierte "no compila" en evidencia concreta: archivo, linea
//! y mensaje. Cuanto mejor sea este modulo, menos tiene que adivinar el modelo.
//!
//! Estado actual: parser de Rust implementado; Java y Python son el siguiente
//! paso del Frente 1 (ver `docs/frentes/frente-1-runtime.md`).

use std::path::PathBuf;

use crate::contracts::{Diagnostic, DiagnosticSource, Language, Severity};

/// Cuantos diagnosticos como maximo entran al contexto. El primer error suele
/// ser la causa; los siguientes suelen ser consecuencia.
pub const MAX_DIAGNOSTICS: usize = 5;

/// Parsea la salida de un build segun el lenguaje.
pub fn parse(language: &Language, output: &str) -> Vec<Diagnostic> {
    let mut diagnosticos = match language {
        Language::Rust => parse_rust(output),
        // TODO(frente-1): implementar estos dos para cerrar el caso vertical
        // inicial (la guia recomienda empezar por Java o Python).
        Language::Java => parse_java(output),
        Language::Python => parse_python(output),
        _ => Vec::new(),
    };
    diagnosticos.truncate(MAX_DIAGNOSTICS);
    diagnosticos
}

/// Parsea la salida de `cargo build` / `cargo test`.
///
/// Formato esperado:
/// ```text
/// error[E0308]: mismatched types
///   --> src/main.rs:4:20
/// ```
fn parse_rust(output: &str) -> Vec<Diagnostic> {
    let lineas: Vec<&str> = output.lines().collect();
    let mut diagnosticos = Vec::new();

    for (i, linea) in lineas.iter().enumerate() {
        let linea = linea.trim();
        let (severity, resto) = if let Some(resto) = linea.strip_prefix("error") {
            (Severity::Error, resto)
        } else if let Some(resto) = linea.strip_prefix("warning") {
            (Severity::Warning, resto)
        } else {
            continue;
        };

        // `[E0308]: mensaje` o `: mensaje`
        let (code, mensaje) = match resto.strip_prefix('[') {
            Some(con_codigo) => match con_codigo.split_once(']') {
                Some((codigo, resto)) => (Some(codigo.to_string()), resto),
                None => (None, resto),
            },
            None => (None, resto),
        };
        let Some(mensaje) = mensaje.strip_prefix(':') else {
            continue;
        };
        let mensaje = mensaje.trim();
        if mensaje.is_empty() {
            continue;
        }

        // La ubicacion viene en la linea siguiente, tras `-->`.
        let ubicacion = lineas
            .get(i + 1)
            .and_then(|l| l.trim().strip_prefix("--> "));
        let (file, line, column) = ubicacion.map_or((None, None, None), parse_ubicacion_rust);

        diagnosticos.push(Diagnostic {
            severity,
            source: DiagnosticSource::Compiler,
            file,
            line,
            column,
            code,
            message: mensaje.to_string(),
        });
    }

    // Los errores primero: el warning casi nunca es lo que trae al alumno.
    diagnosticos.sort_by_key(|d| match d.severity {
        Severity::Error => 0,
        Severity::Warning => 1,
        Severity::Note => 2,
    });
    diagnosticos
}

/// `src/main.rs:4:20` -> (archivo, linea, columna)
fn parse_ubicacion_rust(ubicacion: &str) -> (Option<PathBuf>, Option<u32>, Option<u32>) {
    let partes: Vec<&str> = ubicacion.trim().rsplitn(3, ':').collect();
    match partes.as_slice() {
        [col, linea, archivo] => (
            Some(PathBuf::from(*archivo)),
            linea.parse().ok(),
            col.parse().ok(),
        ),
        _ => (Some(PathBuf::from(ubicacion.trim())), None, None),
    }
}

/// TODO(frente-1): parsear salida de `mvn`/`gradle` y stack traces de la JVM.
/// Formatos objetivo:
///   `[ERROR] /ruta/Archivo.java:[23,15] mensaje`
///   `at demo.UserService.nombreDe(UserService.java:23)`
fn parse_java(_output: &str) -> Vec<Diagnostic> {
    Vec::new()
}

/// TODO(frente-1): parsear tracebacks de Python y salida de `pytest`.
/// Formato objetivo:
///   `  File "app.py", line 23, in nombre_de`
fn parse_python(_output: &str) -> Vec<Diagnostic> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SALIDA_CARGO: &str = "\
   Compiling demo v0.1.0 (/proyecto/demo)
error[E0308]: mismatched types
  --> src/main.rs:4:20
   |
 4 |     let x: u32 = \"hola\";
   |                    ^^^^^^ expected `u32`, found `&str`
warning: unused variable: `y`
  --> src/main.rs:6:9
error: could not compile `demo` (bin \"demo\") due to 1 previous error";

    #[test]
    fn parsea_un_error_de_cargo_con_codigo_y_ubicacion() {
        let diagnosticos = parse(&Language::Rust, SALIDA_CARGO);
        let primero = &diagnosticos[0];
        assert_eq!(primero.severity, Severity::Error);
        assert_eq!(primero.code.as_deref(), Some("E0308"));
        assert_eq!(primero.message, "mismatched types");
        assert_eq!(primero.file, Some(PathBuf::from("src/main.rs")));
        assert_eq!(primero.line, Some(4));
        assert_eq!(primero.column, Some(20));
    }

    #[test]
    fn los_errores_van_antes_que_los_warnings() {
        let diagnosticos = parse(&Language::Rust, SALIDA_CARGO);
        assert!(diagnosticos.len() >= 2);
        assert_eq!(diagnosticos[0].severity, Severity::Error);
        assert!(diagnosticos.iter().any(|d| d.severity == Severity::Warning));
    }

    #[test]
    fn no_inventa_diagnosticos_cuando_todo_compila() {
        let diagnosticos = parse(&Language::Rust, "    Finished dev profile in 0.4s");
        assert!(diagnosticos.is_empty());
    }

    #[test]
    fn nunca_devuelve_mas_del_maximo() {
        let ruidosa = "error: algo\n".repeat(50);
        assert!(parse(&Language::Rust, &ruidosa).len() <= MAX_DIAGNOSTICS);
    }
}
