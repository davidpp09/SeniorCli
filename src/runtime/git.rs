//! Integracion con Git: que cambio y que archivos tocar primero.
//!
//! Git es la mejor senal barata de "donde esta trabajando el alumno ahora".
//! Todo pasa por [`CommandRunner`], nunca por un shell.

use std::path::PathBuf;

use crate::contracts::Result;

use super::runner::CommandRunner;

/// Limite del diff enviado al modelo. Un diff gigante cuesta tokens y anade
/// ruido sin mejorar la respuesta.
pub const MAX_DIFF_CHARS: usize = 8_000;

#[derive(Debug, Clone, Default)]
pub struct GitSnapshot {
    /// `false` si el proyecto no esta bajo control de versiones.
    pub is_repo: bool,
    /// Archivos modificados o agregados, relativos a la raiz.
    pub modified_files: Vec<PathBuf>,
    /// Diff unificado, ya recortado a [`MAX_DIFF_CHARS`].
    pub diff: Option<String>,
    /// `true` si el diff se recorto.
    pub diff_truncated: bool,
}

/// Toma una foto del estado de git. Si el proyecto no es un repo, devuelve un
/// snapshot vacio en lugar de un error: trabajar sin git es valido.
pub async fn snapshot(runner: &CommandRunner) -> Result<GitSnapshot> {
    let dentro = runner
        .run_checked("git", &["rev-parse", "--is-inside-work-tree"])
        .await;
    let es_repo = matches!(&dentro, Ok(salida) if salida.success());
    if !es_repo {
        return Ok(GitSnapshot::default());
    }

    let modified_files = archivos_modificados(runner).await?;
    let (diff, diff_truncated) = diff_recortado(runner).await?;

    Ok(GitSnapshot {
        is_repo: true,
        modified_files,
        diff,
        diff_truncated,
    })
}

async fn archivos_modificados(runner: &CommandRunner) -> Result<Vec<PathBuf>> {
    let salida = runner
        .run_checked("git", &["status", "--porcelain"])
        .await?;
    Ok(salida
        .stdout
        .lines()
        .filter_map(parsear_linea_status)
        .collect())
}

/// Parsea una linea de `git status --porcelain`: `XY ruta` o `XY vieja -> nueva`.
fn parsear_linea_status(linea: &str) -> Option<PathBuf> {
    if linea.len() < 4 {
        return None;
    }
    let ruta = linea[3..].trim();
    // Renombrado: nos interesa el destino.
    let ruta = ruta.rsplit(" -> ").next().unwrap_or(ruta);
    let ruta = ruta.trim_matches('"');
    if ruta.is_empty() {
        None
    } else {
        Some(PathBuf::from(ruta))
    }
}

async fn diff_recortado(runner: &CommandRunner) -> Result<(Option<String>, bool)> {
    let salida = runner.run_checked("git", &["diff", "--unified=3"]).await?;
    let diff = salida.stdout;

    if diff.trim().is_empty() {
        return Ok((None, false));
    }
    if diff.len() <= MAX_DIFF_CHARS {
        return Ok((Some(diff), false));
    }

    // Cortar en un limite de caracter valido para no romper UTF-8.
    let mut corte = MAX_DIFF_CHARS;
    while corte > 0 && !diff.is_char_boundary(corte) {
        corte -= 1;
    }
    Ok((Some(diff[..corte].to_string()), true))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parsea_lineas_de_status() {
        assert_eq!(
            parsear_linea_status(" M src/main.rs"),
            Some(PathBuf::from("src/main.rs"))
        );
        assert_eq!(
            parsear_linea_status("?? nuevo.txt"),
            Some(PathBuf::from("nuevo.txt"))
        );
        assert_eq!(
            parsear_linea_status("R  viejo.rs -> nuevo.rs"),
            Some(PathBuf::from("nuevo.rs"))
        );
        assert_eq!(parsear_linea_status(""), None);
    }

    #[tokio::test]
    async fn un_directorio_sin_git_devuelve_snapshot_vacio() {
        let dir = tempfile::tempdir().expect("tempdir");
        let runner = CommandRunner::new(dir.path());
        let snap = snapshot(&runner).await.expect("snapshot");
        // Fuera de un repo, git falla y devolvemos vacio en lugar de error.
        if !snap.is_repo {
            assert!(snap.modified_files.is_empty());
            assert!(snap.diff.is_none());
        }
    }
}
