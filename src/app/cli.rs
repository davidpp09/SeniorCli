//! Definicion de la linea de comandos.
//!
//! Los cuatro comandos del alcance inicial: `init`, `learn`, `debug`,
//! `progress`. Agregar comandos nuevos es barato; agregarlos antes de que el
//! flujo central funcione, no.

use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::learning::TeachingPolicy;

#[derive(Debug, Parser)]
#[command(
    name = "senior",
    version,
    about = "SeniorCLI - un ingeniero senior que te acompana mientras aprendes",
    long_about = None,
)]
pub struct Cli {
    /// Raiz del proyecto. Por defecto, el directorio actual.
    #[arg(long, short = 'C', global = true)]
    pub path: Option<PathBuf>,

    /// Mas detalle en la salida (equivale a SENIOR_LOG=debug).
    #[arg(long, short, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Prepara el proyecto: crea `.senior/` con la configuracion y el perfil.
    Init {
        /// Proveedor de modelo: `mock` o `deepseek`.
        #[arg(long, default_value = "mock")]
        provider: String,

        /// Vuelve a escribir la configuracion aunque ya exista.
        #[arg(long)]
        force: bool,
    },

    /// Sesion de aprendizaje sobre lo que estas construyendo.
    Learn {
        /// Tu pregunta. Si la omites, se abre el modo interactivo.
        message: Option<String>,

        /// Objetivo de la sesion, para no perder el hilo.
        #[arg(long)]
        goal: Option<String>,
    },

    /// Analiza el error actual del proyecto.
    Debug {
        /// Que quieres entender. Si la omites, SeniorCLI parte del ultimo error.
        message: Option<String>,

        /// Archivo concreto sobre el que quieres trabajar.
        #[arg(long)]
        file: Option<PathBuf>,

        /// Permite llegar hasta la solucion guiada. Usalo cuando de verdad te
        /// atoraste: por defecto SeniorCLI no entrega la solucion.
        #[arg(long)]
        explain: bool,
    },

    /// Muestra tu progreso: nivel, temas y errores recientes.
    Progress {
        /// Incluye el resumen de la ultima sesion.
        #[arg(long)]
        sessions: bool,
    },
}

impl Commands {
    /// Politica pedagogica que corresponde a este comando.
    ///
    /// `debug --explain` es la unica via para llegar a la solucion guiada, y es
    /// una decision explicita del alumno.
    pub fn policy(&self) -> TeachingPolicy {
        match self {
            Self::Debug { explain: true, .. } => TeachingPolicy::permisiva(),
            Self::Learn { .. } => TeachingPolicy::socratica(),
            _ => TeachingPolicy::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::Intervention;
    use clap::CommandFactory;

    #[test]
    fn la_definicion_de_la_cli_es_valida() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parsea_los_cuatro_comandos() {
        for args in [
            vec!["senior", "init"],
            vec!["senior", "learn", "como uso Optional?"],
            vec!["senior", "debug"],
            vec!["senior", "progress"],
        ] {
            assert!(Cli::try_parse_from(args).is_ok());
        }
    }

    #[test]
    fn learn_es_la_modalidad_mas_estricta() {
        let cli = Cli::try_parse_from(["senior", "learn", "hola"]).expect("parsea");
        assert_eq!(cli.command.policy().ceiling, Intervention::Hint);
    }

    #[test]
    fn solo_debug_explain_llega_a_la_solucion() {
        let normal = Cli::try_parse_from(["senior", "debug"]).expect("parsea");
        assert!(normal.command.policy().ceiling < Intervention::GuidedSolution);

        let explicito = Cli::try_parse_from(["senior", "debug", "--explain"]).expect("parsea");
        assert_eq!(
            explicito.command.policy().ceiling,
            Intervention::GuidedSolution
        );
    }

    #[test]
    fn acepta_una_raiz_distinta_del_directorio_actual() {
        let cli =
            Cli::try_parse_from(["senior", "-C", "/otro/proyecto", "progress"]).expect("parsea");
        assert_eq!(cli.path, Some(PathBuf::from("/otro/proyecto")));
    }
}
