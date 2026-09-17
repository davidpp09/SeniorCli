//! Deteccion de la raiz del proyecto y del stack.
//!
//! Implementado por marcadores de build, no por extensiones de archivo: un
//! `.java` suelto no hace un proyecto Java, un `pom.xml` si.
//!
//! Soporta monorepos: [`detect_stack`] devuelve *todos* los componentes que
//! encuentra y marca uno como activo.

use std::path::{Path, PathBuf};

use crate::contracts::{Language, Result, SeniorError, StackComponent, StackInfo};

/// Marcadores de build, en orden de prioridad para elegir el componente activo.
const MARCADORES: &[(&str, Language, &str)] = &[
    ("Cargo.toml", Language::Rust, "cargo"),
    ("pom.xml", Language::Java, "maven"),
    ("build.gradle", Language::Java, "gradle"),
    ("build.gradle.kts", Language::Java, "gradle"),
    ("package.json", Language::Node, "npm"),
    ("pyproject.toml", Language::Python, "poetry"),
    ("requirements.txt", Language::Python, "pip"),
    ("setup.py", Language::Python, "setuptools"),
    ("go.mod", Language::Go, "go"),
];

/// Carpetas que nunca vale la pena recorrer buscando componentes.
const IGNORADAS: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    "build",
    "dist",
    ".venv",
    "venv",
    "__pycache__",
    ".senior",
];

/// Profundidad maxima al buscar sub-componentes de un monorepo.
const PROFUNDIDAD_MAX: usize = 3;

/// Sube desde `start` hasta encontrar la raiz del proyecto.
///
/// Prioriza `.git`; si no hay repo, usa el directorio mas alto que tenga un
/// marcador de build.
///
/// # Errores
/// [`SeniorError::ProjectRootNotFound`] si no encuentra ninguna senal.
pub fn find_project_root(start: &Path) -> Result<PathBuf> {
    let start = start
        .canonicalize()
        .map_err(|_| SeniorError::ProjectRootNotFound(start.to_path_buf()))?;

    let mut candidato_marcador = None;

    for dir in start.ancestors() {
        if dir.join(".git").exists() {
            return Ok(dir.to_path_buf());
        }
        if candidato_marcador.is_none() && tiene_marcador(dir) {
            candidato_marcador = Some(dir.to_path_buf());
        }
    }

    candidato_marcador.ok_or(SeniorError::ProjectRootNotFound(start))
}

fn tiene_marcador(dir: &Path) -> bool {
    MARCADORES
        .iter()
        .any(|(archivo, _, _)| dir.join(archivo).exists())
}

fn componente_en(dir: &Path, root: &Path) -> Option<StackComponent> {
    let (_, language, build_tool) = MARCADORES
        .iter()
        .find(|(archivo, _, _)| dir.join(archivo).exists())?;

    let relativa = dir
        .strip_prefix(root)
        .unwrap_or(Path::new("."))
        .to_path_buf();
    Some(StackComponent {
        language: language.clone(),
        root: if relativa.as_os_str().is_empty() {
            PathBuf::from(".")
        } else {
            relativa
        },
        build_tool: Some((*build_tool).to_string()),
    })
}

/// Detecta el stack del proyecto, incluyendo sub-componentes de un monorepo.
///
/// Si no reconoce nada devuelve un [`StackInfo`] con `Language::Unknown`: el
/// Frente 1 no adivina, deja que el Frente 2 pregunte.
pub fn detect_stack(root: &Path) -> Result<StackInfo> {
    let mut componentes = Vec::new();
    recolectar(root, root, 0, &mut componentes);

    let active = componentes
        .first()
        .map_or(Language::Unknown, |c| c.language.clone());

    Ok(StackInfo {
        root: root.to_path_buf(),
        active,
        components: componentes,
    })
}

fn recolectar(dir: &Path, root: &Path, profundidad: usize, salida: &mut Vec<StackComponent>) {
    if profundidad > PROFUNDIDAD_MAX {
        return;
    }

    if let Some(componente) = componente_en(dir, root) {
        salida.push(componente);
    }

    let Ok(entradas) = std::fs::read_dir(dir) else {
        return;
    };

    for entrada in entradas.flatten() {
        let ruta = entrada.path();
        if !ruta.is_dir() {
            continue;
        }
        let nombre = entrada.file_name();
        let nombre = nombre.to_string_lossy();
        if nombre.starts_with('.') || IGNORADAS.contains(&nombre.as_ref()) {
            continue;
        }
        recolectar(&ruta, root, profundidad + 1, salida);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detecta_este_mismo_proyecto_como_rust() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let stack = detect_stack(root).expect("detecta el stack");
        assert_eq!(stack.active, Language::Rust);
        assert!(
            stack
                .components
                .iter()
                .any(|c| c.build_tool.as_deref() == Some("cargo"))
        );
    }

    #[test]
    fn un_directorio_sin_marcadores_queda_como_desconocido() {
        let dir = tempfile::tempdir().expect("tempdir");
        let stack = detect_stack(dir.path()).expect("detecta el stack");
        assert_eq!(stack.active, Language::Unknown);
        assert!(stack.components.is_empty());
    }

    #[test]
    fn un_monorepo_reporta_varios_componentes() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join("api")).expect("crea api");
        std::fs::create_dir_all(dir.path().join("web")).expect("crea web");
        std::fs::write(dir.path().join("api/pom.xml"), "<project/>").expect("escribe pom");
        std::fs::write(dir.path().join("web/package.json"), "{}").expect("escribe package");

        let stack = detect_stack(dir.path()).expect("detecta el stack");
        assert_eq!(stack.components.len(), 2);
        assert!(
            stack
                .components
                .iter()
                .any(|c| c.language == Language::Java)
        );
        assert!(
            stack
                .components
                .iter()
                .any(|c| c.language == Language::Node)
        );
    }

    #[test]
    fn encuentra_la_raiz_subiendo_desde_un_subdirectorio() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let encontrada = find_project_root(&root.join("src")).expect("encuentra la raiz");
        assert_eq!(encontrada.canonicalize().ok(), root.canonicalize().ok());
    }
}
