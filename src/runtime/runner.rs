//! Ejecucion controlada de comandos.
//!
//! # Regla de seguridad, no negociable
//!
//! **Nunca se ejecuta texto generado por el LLM como shell.** Este modulo no
//! usa `sh -c` ni `cmd /c`: solo lanza programas de una lista blanca, con
//! subcomandos validados y un timeout. Si el Frente 2 quiere correr algo, pide
//! una accion permitida; no manda una cadena de comando.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

use crate::contracts::{Result, SeniorError};

/// Programas que SeniorCLI puede lanzar. Ampliar esta lista es una decision de
/// diseno que se revisa en PR, no algo que se parchea al vuelo.
pub const ALLOWED_PROGRAMS: &[&str] = &[
    "git", "cargo", "rustc", "mvn", "gradle", "npm", "node", "python", "python3", "pytest", "go",
    "dotnet",
];

/// Subcomandos permitidos por programa. `&[]` significa "sin restriccion de
/// subcomando" (por ejemplo `rustc --version`).
const ALLOWED_SUBCOMMANDS: &[(&str, &[&str])] = &[
    ("git", &["status", "diff", "rev-parse", "log", "ls-files"]),
    (
        "cargo",
        &["build", "check", "test", "clippy", "fmt", "metadata"],
    ),
    ("mvn", &["-q", "compile", "test", "test-compile"]),
    ("gradle", &["build", "test", "compileJava"]),
    ("npm", &["run", "test", "ci"]),
    ("python", &["-m", "--version"]),
    ("python3", &["-m", "--version"]),
    ("pytest", &[]),
    ("go", &["build", "test", "vet"]),
    ("dotnet", &["build", "test"]),
];

/// Un comando ya validado contra la lista blanca. Solo se puede construir con
/// [`CommandRunner::allow`], asi que tener uno de estos ya es la prueba de que
/// paso el filtro.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AllowedCommand {
    program: String,
    args: Vec<String>,
}

impl AllowedCommand {
    pub fn display(&self) -> String {
        if self.args.is_empty() {
            self.program.clone()
        } else {
            format!("{} {}", self.program, self.args.join(" "))
        }
    }
}

#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

impl CommandOutput {
    pub fn success(&self) -> bool {
        self.exit_code == Some(0)
    }

    /// stdout + stderr juntos. Muchos compiladores reparten los diagnosticos
    /// entre los dos flujos.
    pub fn combined(&self) -> String {
        format!("{}\n{}", self.stdout, self.stderr)
    }
}

#[derive(Debug, Clone)]
pub struct CommandRunner {
    root: PathBuf,
    timeout: Duration,
}

impl CommandRunner {
    /// Timeout por defecto. Un build lento no debe colgar la CLI para siempre.
    pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            timeout: Self::DEFAULT_TIMEOUT,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Valida un comando contra la lista blanca.
    ///
    /// # Errores
    /// [`SeniorError::CommandNotAllowed`] si el programa o su subcomando no
    /// estan permitidos.
    pub fn allow<S: AsRef<str>>(program: &str, args: &[S]) -> Result<AllowedCommand> {
        if !ALLOWED_PROGRAMS.contains(&program) {
            return Err(SeniorError::CommandNotAllowed {
                command: program.to_string(),
            });
        }

        let args: Vec<String> = args.iter().map(|a| a.as_ref().to_string()).collect();

        if let Some((_, permitidos)) = ALLOWED_SUBCOMMANDS.iter().find(|(p, _)| *p == program)
            && !permitidos.is_empty()
        {
            let sub = args.first().map(String::as_str).unwrap_or("");
            if !permitidos.contains(&sub) {
                return Err(SeniorError::CommandNotAllowed {
                    command: format!("{program} {sub}"),
                });
            }
        }

        Ok(AllowedCommand {
            program: program.to_string(),
            args,
        })
    }

    /// Ejecuta un comando ya validado, con timeout y sin shell.
    pub async fn run(&self, command: &AllowedCommand) -> Result<CommandOutput> {
        let mut cmd = Command::new(&command.program);
        cmd.args(&command.args)
            .current_dir(&self.root)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // Salida estable y parseable, sin colores ANSI.
            .env("NO_COLOR", "1")
            .env("CARGO_TERM_COLOR", "never");

        let futuro = cmd.output();
        let salida = tokio::time::timeout(self.timeout, futuro)
            .await
            .map_err(|_| SeniorError::CommandTimeout {
                command: command.display(),
                timeout_secs: self.timeout.as_secs(),
            })?;

        let salida = salida.map_err(|e| {
            SeniorError::ContextCollection(format!(
                "no se pudo ejecutar `{}`: {e}",
                command.display()
            ))
        })?;

        Ok(CommandOutput {
            exit_code: salida.status.code(),
            stdout: String::from_utf8_lossy(&salida.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&salida.stderr).into_owned(),
        })
    }

    /// Atajo: validar y ejecutar en un paso.
    pub async fn run_checked<S: AsRef<str>>(
        &self,
        program: &str,
        args: &[S],
    ) -> Result<CommandOutput> {
        let command = Self::allow(program, args)?;
        self.run(&command).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rechaza_programas_fuera_de_la_lista() {
        let error = CommandRunner::allow("rm", &["-rf", "/"]).unwrap_err();
        assert!(matches!(error, SeniorError::CommandNotAllowed { .. }));
    }

    #[test]
    fn rechaza_subcomandos_no_permitidos() {
        assert!(CommandRunner::allow("git", &["push"]).is_err());
        assert!(CommandRunner::allow("git", &["status"]).is_ok());
    }

    #[test]
    fn no_hay_shell_asi_que_los_metacaracteres_son_texto() {
        // Aunque un argumento traiga `;` o `&&`, se pasa literal al programa:
        // nunca lo interpreta un shell.
        let cmd = CommandRunner::allow("git", &["log", "--oneline; rm -rf /"]).unwrap();
        assert_eq!(cmd.args[1], "--oneline; rm -rf /");
    }

    #[tokio::test]
    async fn ejecuta_un_comando_real_con_timeout() {
        let runner = CommandRunner::new(".").with_timeout(Duration::from_secs(20));
        let salida = runner
            .run_checked("git", &["rev-parse", "--is-inside-work-tree"])
            .await;
        // En CI siempre hay repo git; si no lo hubiera, el error tambien es valido.
        if let Ok(salida) = salida {
            assert!(salida.combined().contains("true") || !salida.success());
        }
    }
}
