//! Lectura de archivos, limites de tamano y filtrado de secretos.
//!
//! Antes de que un byte del proyecto salga hacia un proveedor remoto pasa por
//! aqui. El escaner de secretos **no confia en la extension del archivo**: un
//! token puede estar en un `.md` o en un comentario de codigo.

use std::path::Path;

use crate::contracts::{LineRange, RelevanceReason, RelevantFile, Result};

/// Maximo de caracteres por archivo incluido en el contexto.
pub const MAX_FILE_CHARS: usize = 12_000;

/// Cuantas lineas de contexto incluir alrededor de la linea de un error.
pub const CONTEXT_LINES: u32 = 12;

/// Nombres y extensiones que nunca se leen para construir contexto.
const NEGADOS: &[&str] = &[
    ".env",
    ".env.local",
    ".env.production",
    "id_rsa",
    "id_ed25519",
    "credentials",
    ".npmrc",
    ".pypirc",
    "secrets.json",
    "serviceaccount.json",
];

const EXTENSIONES_NEGADAS: &[&str] = &["pem", "key", "p12", "pfx", "keystore", "jks"];

/// Patrones de secreto. Deliberadamente simples y sin `regex` para no arrastrar
/// la dependencia todavia; el Frente 1 puede cambiarlos por regex cuando haga
/// falta afinar.
const PISTAS_DE_SECRETO: &[&str] = &[
    "-----BEGIN ",
    "sk-",
    "ghp_",
    "github_pat_",
    "AKIA",
    "xoxb-",
    "AIza",
    "PRIVATE KEY",
];

/// `true` si el archivo esta vetado por su nombre o extension.
pub fn is_denied_path(path: &Path) -> bool {
    let nombre = path
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if NEGADOS
        .iter()
        .any(|d| nombre == *d || nombre.starts_with(&format!("{d}.")))
    {
        return true;
    }
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .is_some_and(|ext| EXTENSIONES_NEGADAS.contains(&ext.as_str()))
}

/// `true` si el contenido parece traer credenciales.
///
/// Prefiere falsos positivos a filtrar un secreto: ante la duda, el archivo no
/// entra al contexto.
pub fn looks_like_secret(content: &str) -> bool {
    PISTAS_DE_SECRETO.iter().any(|p| content.contains(p))
}

/// Recorta el contenido a un rango de lineas alrededor de `line`.
/// Las lineas se cuentan desde 1, como en los diagnosticos.
pub fn extract_range(content: &str, line: u32, context: u32) -> (String, LineRange) {
    let lineas: Vec<&str> = content.lines().collect();
    let total = lineas.len() as u32;
    let inicio = line.saturating_sub(context).max(1);
    let fin = (line + context).min(total.max(1));

    let recorte = lineas
        .iter()
        .skip(inicio.saturating_sub(1) as usize)
        .take((fin + 1 - inicio) as usize)
        .copied()
        .collect::<Vec<_>>()
        .join("\n");

    (
        recorte,
        LineRange {
            start: inicio,
            end: fin,
        },
    )
}

/// Lee un archivo del proyecto para meterlo al contexto, aplicando los filtros
/// de seguridad y de tamano.
///
/// Devuelve `Ok(None)` cuando el archivo existe pero no debe incluirse (vetado,
/// binario o con pinta de secreto). Eso no es un error.
pub fn read_relevant_file(
    root: &Path,
    relative: &Path,
    around_line: Option<u32>,
    reason: RelevanceReason,
) -> Result<Option<RelevantFile>> {
    if is_denied_path(relative) {
        return Ok(None);
    }

    let absoluta = root.join(relative);
    let Ok(bytes) = std::fs::read(&absoluta) else {
        return Ok(None);
    };
    let Ok(contenido) = String::from_utf8(bytes) else {
        return Ok(None); // binario
    };
    if looks_like_secret(&contenido) {
        return Ok(None);
    }

    let (mut contenido, mut range) = match around_line {
        Some(linea) => {
            let (recorte, rango) = extract_range(&contenido, linea, CONTEXT_LINES);
            (recorte, Some(rango))
        }
        None => (contenido, None),
    };

    let mut truncated = false;
    if contenido.len() > MAX_FILE_CHARS {
        let mut corte = MAX_FILE_CHARS;
        while corte > 0 && !contenido.is_char_boundary(corte) {
            corte -= 1;
        }
        contenido.truncate(corte);
        truncated = true;
        range = None;
    }

    Ok(Some(RelevantFile {
        path: relative.to_path_buf(),
        range,
        content: contenido,
        reason,
        truncated,
    }))
}

/// Elige que archivos entran al contexto y en que orden.
///
/// TODO(frente-1): esta version solo ordena por la razon de relevancia. Falta:
///   - recorrer el repo respetando `.gitignore` (crate `ignore`),
///   - resolver simbolos relacionados (`RelevanceReason::RelatedSymbol`),
///   - un top-K real por relevancia en vez de un corte fijo.
///
/// Ver `docs/frentes/frente-1-runtime.md`.
pub fn select_relevant_files(mut candidatos: Vec<RelevantFile>, max: usize) -> Vec<RelevantFile> {
    candidatos.sort_by_key(|f| match f.reason {
        RelevanceReason::Diagnostic => 0,
        RelevanceReason::Requested => 1,
        RelevanceReason::GitModified => 2,
        RelevanceReason::RelatedSymbol => 3,
    });
    candidatos.dedup_by(|a, b| a.path == b.path);
    candidatos.truncate(max);
    candidatos
}

/// Rutas que el Frente 1 considera "codigo del alumno" y no ruido de build.
pub fn is_source_file(path: &Path) -> bool {
    const EXTENSIONES: &[&str] = &[
        "rs", "java", "py", "js", "ts", "jsx", "tsx", "go", "cs", "kt", "rb", "c", "cpp", "h",
    ];
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .is_some_and(|ext| EXTENSIONES.contains(&ext.as_str()))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn veta_archivos_de_credenciales() {
        assert!(is_denied_path(Path::new(".env")));
        assert!(is_denied_path(Path::new("config/server.pem")));
        assert!(is_denied_path(Path::new("id_rsa")));
        assert!(!is_denied_path(Path::new("src/main.rs")));
    }

    #[test]
    fn detecta_secretos_aunque_la_extension_sea_inocente() {
        assert!(looks_like_secret("# notas\nAWS_KEY=AKIAIOSFODNN7EXAMPLE\n"));
        assert!(looks_like_secret("-----BEGIN RSA PRIVATE KEY-----"));
        assert!(!looks_like_secret("fn main() { println!(\"hola\"); }"));
    }

    #[test]
    fn extrae_el_rango_alrededor_de_una_linea() {
        let contenido = (1..=50)
            .map(|i| format!("linea {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let (recorte, rango) = extract_range(&contenido, 25, 3);
        assert_eq!(rango, LineRange { start: 22, end: 28 });
        assert!(recorte.starts_with("linea 22"));
        assert!(recorte.ends_with("linea 28"));
    }

    #[test]
    fn el_rango_no_se_sale_del_archivo() {
        let contenido = "a\nb\nc";
        let (_, rango) = extract_range(contenido, 1, 10);
        assert_eq!(rango.start, 1);
        assert_eq!(rango.end, 3);
    }

    #[test]
    fn no_lee_un_archivo_con_secretos() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::write(dir.path().join("notas.md"), "token: ghp_abc123").expect("escribe");
        let resultado = read_relevant_file(
            dir.path(),
            Path::new("notas.md"),
            None,
            RelevanceReason::Requested,
        )
        .expect("lee");
        assert!(resultado.is_none());
    }

    #[test]
    fn prioriza_archivos_de_diagnostico() {
        let archivo = |path: &str, reason| RelevantFile {
            path: PathBuf::from(path),
            range: None,
            content: String::new(),
            reason,
            truncated: false,
        };
        let seleccion = select_relevant_files(
            vec![
                archivo("a.rs", RelevanceReason::GitModified),
                archivo("b.rs", RelevanceReason::Diagnostic),
            ],
            5,
        );
        assert_eq!(seleccion[0].path, PathBuf::from("b.rs"));
    }
}
