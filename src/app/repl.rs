//! Modo interactivo de `learn` y `debug`.
//!
//! Version minima a proposito: lee de stdin sin dependencias extra, para que el
//! flujo completo funcione desde el primer dia.
//!
//! TODO(frente-3): cambiar a `rustyline` para tener historial, edicion de linea
//! y navegacion con flechas (ver `docs/frentes/frente-3-cli.md`). La frontera
//! ya esta aislada en [`Prompt`], asi que el cambio no toca el resto del frente.

use std::io::{BufRead, Write};

use crate::contracts::{Result, SeniorError};

/// De donde salen las respuestas del alumno.
///
/// Es un trait para poder guionizar una sesion en los tests sin teclado.
pub trait Prompt {
    /// Devuelve `None` cuando el alumno cierra la entrada (Ctrl+D / Ctrl+Z).
    fn ask(&mut self, etiqueta: &str) -> Result<Option<String>>;
}

/// Prompt real sobre stdin/stdout.
#[derive(Debug, Default)]
pub struct StdinPrompt;

impl Prompt for StdinPrompt {
    fn ask(&mut self, etiqueta: &str) -> Result<Option<String>> {
        print!("{etiqueta} ");
        std::io::stdout().flush().map_err(SeniorError::Io)?;

        let mut linea = String::new();
        let leidos = std::io::stdin()
            .lock()
            .read_line(&mut linea)
            .map_err(SeniorError::Io)?;
        if leidos == 0 {
            return Ok(None); // fin de la entrada
        }

        let linea = linea.trim().to_string();
        if linea.eq_ignore_ascii_case("salir") || linea.eq_ignore_ascii_case("exit") {
            return Ok(None);
        }
        Ok(Some(linea))
    }
}

/// Prompt guionizado para tests: entrega respuestas en orden.
#[derive(Debug, Default)]
pub struct ScriptedPrompt {
    respuestas: std::collections::VecDeque<String>,
}

impl ScriptedPrompt {
    pub fn new(respuestas: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            respuestas: respuestas.into_iter().map(Into::into).collect(),
        }
    }
}

impl Prompt for ScriptedPrompt {
    fn ask(&mut self, _etiqueta: &str) -> Result<Option<String>> {
        Ok(self.respuestas.pop_front())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_prompt_guionizado_entrega_las_respuestas_en_orden() {
        let mut prompt = ScriptedPrompt::new(["primera", "segunda"]);
        assert_eq!(prompt.ask(">").expect("lee"), Some("primera".to_string()));
        assert_eq!(prompt.ask(">").expect("lee"), Some("segunda".to_string()));
        assert_eq!(prompt.ask(">").expect("lee"), None);
    }
}
