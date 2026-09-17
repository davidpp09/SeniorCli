//! Entrada del alumno en la sesion interactiva.
//!
//! Dos piezas: [`Prompt`], que es de donde salen las lineas, e [`interpretar`],
//! que decide si una linea es un comando o un mensaje para el tutor.
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

/// Lo que escribio el alumno, ya interpretado.
///
/// Los comandos empiezan por `/` para que nunca choquen con una duda real:
/// "salir del bucle" es una pregunta valida de programacion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entrada {
    /// Linea en blanco: se ignora sin ruido.
    Vacia,
    Salir,
    Ayuda,
    /// Volver a mostrar sobre que esta razonando SeniorCLI.
    Contexto,
    Progreso,
    /// Volver a mirar el proyecto, porque el alumno ya cambio codigo.
    Analizar,
    /// Un comando que no existe. Se guarda para poder nombrarlo en el aviso.
    Desconocida(String),
    /// Todo lo demas: una duda para el tutor.
    Mensaje(String),
}

/// Traduce una linea cruda a una [`Entrada`].
pub fn interpretar(linea: &str) -> Entrada {
    let linea = linea.trim();
    if linea.is_empty() {
        return Entrada::Vacia;
    }

    // Cortesia: las dos palabras que todo el mundo teclea para irse.
    if linea.eq_ignore_ascii_case("salir") || linea.eq_ignore_ascii_case("exit") {
        return Entrada::Salir;
    }

    let Some(comando) = linea.strip_prefix('/') else {
        return Entrada::Mensaje(linea.to_string());
    };

    match comando.trim().to_lowercase().as_str() {
        "salir" | "exit" | "q" => Entrada::Salir,
        "ayuda" | "help" | "?" => Entrada::Ayuda,
        "contexto" => Entrada::Contexto,
        "progreso" => Entrada::Progreso,
        "analizar" => Entrada::Analizar,
        otro => Entrada::Desconocida(otro.to_string()),
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
    fn una_duda_normal_no_se_confunde_con_un_comando() {
        // El caso que obliga a usar `/`: una pregunta que empieza como comando.
        assert_eq!(
            interpretar("salir del bucle sin romper el for"),
            Entrada::Mensaje("salir del bucle sin romper el for".to_string())
        );
    }

    #[test]
    fn reconoce_los_comandos_de_la_sesion() {
        assert_eq!(interpretar("/salir"), Entrada::Salir);
        assert_eq!(interpretar("  /Ayuda  "), Entrada::Ayuda);
        assert_eq!(interpretar("/analizar"), Entrada::Analizar);
        assert_eq!(interpretar("salir"), Entrada::Salir);
        assert_eq!(interpretar(""), Entrada::Vacia);
    }

    #[test]
    fn un_comando_inventado_se_nombra_en_vez_de_ignorarse() {
        assert_eq!(
            interpretar("/compilar"),
            Entrada::Desconocida("compilar".to_string())
        );
    }

    #[test]
    fn el_prompt_guionizado_entrega_las_respuestas_en_orden() {
        let mut prompt = ScriptedPrompt::new(["primera", "segunda"]);
        assert_eq!(prompt.ask(">").expect("lee"), Some("primera".to_string()));
        assert_eq!(prompt.ask(">").expect("lee"), Some("segunda".to_string()));
        assert_eq!(prompt.ask(">").expect("lee"), None);
    }
}
