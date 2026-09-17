//! Binario `senior`.
//!
//! Aqui solo va el arranque: parseo de argumentos, logging y traduccion del
//! error final a algo que el alumno pueda entender. La logica vive en la
//! biblioteca, que es lo que prueban los tests.

use clap::Parser;

use seniorcli::app::{cli::Cli, run, ui};

#[tokio::main]
async fn main() -> std::process::ExitCode {
    let cli = Cli::parse();
    init_logging(cli.verbose);

    match run(cli).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            // Nunca un stack trace crudo: el alumno merece un mensaje util.
            eprintln!("{}", ui::render_error(&error));
            tracing::debug!("detalle tecnico: {error:?}");
            std::process::ExitCode::FAILURE
        }
    }
}

/// Logs a stderr, controlados por `SENIOR_LOG` o por `--verbose`.
fn init_logging(verbose: bool) {
    let por_defecto = if verbose { "debug" } else { "warn" };
    let filtro = tracing_subscriber::EnvFilter::try_from_env("SENIOR_LOG")
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(por_defecto));

    tracing_subscriber::fmt()
        .with_env_filter(filtro)
        .with_target(false)
        .with_writer(std::io::stderr)
        .init();
}
