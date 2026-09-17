//! Estado local en `.senior/`.
//!
//! ```text
//! .senior/
//!   config.toml            configuracion del proyecto
//!   profile.json           perfil del alumno
//!   profile.json.bak       ultimo perfil valido conocido
//!   sessions/
//!     2026-09-16T1234.jsonl  un turno por linea
//! ```
//!
//! Dos reglas que no se negocian:
//!
//! 1. **Escrituras atomicas.** Se escribe a un temporal y se renombra. Un
//!    Ctrl+C a medio guardado no deja un JSON partido.
//! 2. **Nunca sobrescribir un perfil valido con uno invalido.** Si el perfil en
//!    disco no valida, se restaura el respaldo en vez de empezar de cero.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::contracts::{PROFILE_SCHEMA_VERSION, ProfileStore, Result, SeniorError, StudentProfile};

pub const DIR_ESTADO: &str = ".senior";
pub const ARCHIVO_CONFIG: &str = "config.toml";
pub const ARCHIVO_PERFIL: &str = "profile.json";
pub const ARCHIVO_PERFIL_BAK: &str = "profile.json.bak";
pub const DIR_SESIONES: &str = "sessions";

/// Configuracion del proyecto, versionada para poder migrarla.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub schema_version: u32,
    /// Identificador estable del proyecto, para no mezclar perfiles.
    pub project_id: String,
    /// `mock`, `deepseek`, ...
    pub provider: String,
    /// Techo de ayuda: `socratica`, `default` o `permisiva`.
    pub policy: String,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            schema_version: 1,
            project_id: "proyecto".to_string(),
            provider: "mock".to_string(),
            policy: "default".to_string(),
        }
    }
}

/// Acceso al estado local de un proyecto.
#[derive(Debug, Clone)]
pub struct Storage {
    root: PathBuf,
}

impl Storage {
    pub fn new(project_root: impl Into<PathBuf>) -> Self {
        Self {
            root: project_root.into(),
        }
    }

    pub fn dir(&self) -> PathBuf {
        self.root.join(DIR_ESTADO)
    }

    pub fn config_path(&self) -> PathBuf {
        self.dir().join(ARCHIVO_CONFIG)
    }

    pub fn profile_path(&self) -> PathBuf {
        self.dir().join(ARCHIVO_PERFIL)
    }

    pub fn sessions_dir(&self) -> PathBuf {
        self.dir().join(DIR_SESIONES)
    }

    /// `true` si el proyecto ya fue inicializado con `senior init`.
    pub fn is_initialized(&self) -> bool {
        self.config_path().exists()
    }

    /// Crea `.senior/` con configuracion y perfil por defecto.
    /// Es idempotente: no pisa lo que ya existe.
    pub fn init(&self, config: &ProjectConfig) -> Result<()> {
        std::fs::create_dir_all(self.sessions_dir())
            .map_err(|e| self.error_io(self.sessions_dir(), e))?;

        if !self.config_path().exists() {
            self.save_config(config)?;
        }
        if !self.profile_path().exists() {
            self.save_profile(&StudentProfile::new(&config.project_id))?;
        }
        Ok(())
    }

    pub fn load_config(&self) -> Result<ProjectConfig> {
        let ruta = self.config_path();
        let texto = std::fs::read_to_string(&ruta).map_err(|e| self.error_io(ruta.clone(), e))?;

        let config: ProjectConfig =
            toml::from_str(&texto).map_err(|e| SeniorError::CorruptState {
                path: ruta,
                reason: e.to_string(),
            })?;
        Ok(config)
    }

    pub fn save_config(&self, config: &ProjectConfig) -> Result<()> {
        let texto = toml::to_string_pretty(config)
            .map_err(|e| SeniorError::Config(format!("no pude serializar la config: {e}")))?;
        escribir_atomico(&self.config_path(), texto.as_bytes())
    }

    /// Ruta del archivo de sesion de hoy, creando el directorio si hace falta.
    pub fn new_session_path(&self) -> Result<PathBuf> {
        let dir = self.sessions_dir();
        std::fs::create_dir_all(&dir).map_err(|e| self.error_io(dir.clone(), e))?;
        let marca = chrono::Utc::now().format("%Y-%m-%dT%H%M%S");
        Ok(dir.join(format!("{marca}.jsonl")))
    }

    /// La sesion mas reciente, si existe. Es lo que permite retomar despues de
    /// un cierre inesperado.
    pub fn latest_session_path(&self) -> Option<PathBuf> {
        let mut sesiones: Vec<PathBuf> = std::fs::read_dir(self.sessions_dir())
            .ok()?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "jsonl"))
            .collect();
        sesiones.sort();
        sesiones.pop()
    }

    fn error_io(&self, path: PathBuf, source: std::io::Error) -> SeniorError {
        SeniorError::Storage { path, source }
    }
}

impl ProfileStore for Storage {
    /// Carga el perfil, validandolo.
    ///
    /// Si el archivo esta corrupto intenta el respaldo; si tampoco sirve,
    /// devuelve [`SeniorError::CorruptState`] en vez de inventar un perfil
    /// nuevo y perder el progreso del alumno en silencio.
    fn load_profile(&self) -> Result<StudentProfile> {
        let ruta = self.profile_path();

        match leer_perfil(&ruta) {
            Ok(perfil) => Ok(perfil),
            Err(motivo_principal) => {
                let respaldo = self.dir().join(ARCHIVO_PERFIL_BAK);
                match leer_perfil(&respaldo) {
                    Ok(perfil) => {
                        tracing::warn!(
                            "perfil corrupto ({motivo_principal}); restaurado desde el respaldo"
                        );
                        // Dejar el respaldo restaurado como perfil activo.
                        self.save_profile(&perfil)?;
                        Ok(perfil)
                    }
                    Err(_) => Err(SeniorError::CorruptState {
                        path: ruta,
                        reason: motivo_principal,
                    }),
                }
            }
        }
    }

    /// Guarda el perfil de forma atomica, dejando antes un respaldo del
    /// anterior. Rechaza guardar un perfil que no valida.
    fn save_profile(&self, profile: &StudentProfile) -> Result<()> {
        profile
            .validate()
            .map_err(|reason| SeniorError::CorruptState {
                path: self.profile_path(),
                reason: format!("me negue a guardar un perfil invalido: {reason}"),
            })?;

        let ruta = self.profile_path();
        if let Some(padre) = ruta.parent() {
            std::fs::create_dir_all(padre).map_err(|e| self.error_io(padre.to_path_buf(), e))?;
        }

        // Respaldo del perfil anterior, solo si el que hay es valido.
        if leer_perfil(&ruta).is_ok() {
            let _ = std::fs::copy(&ruta, self.dir().join(ARCHIVO_PERFIL_BAK));
        }

        let json = serde_json::to_vec_pretty(profile)?;
        escribir_atomico(&ruta, &json)
    }
}

fn leer_perfil(ruta: &Path) -> std::result::Result<StudentProfile, String> {
    let texto = std::fs::read_to_string(ruta).map_err(|e| e.to_string())?;
    let perfil: StudentProfile = serde_json::from_str(&texto).map_err(|e| e.to_string())?;
    perfil.validate()?;
    if perfil.schema_version > PROFILE_SCHEMA_VERSION {
        return Err(format!(
            "el perfil viene de una version mas nueva de SeniorCLI (v{})",
            perfil.schema_version
        ));
    }
    Ok(perfil)
}

/// Escritura atomica: temporal + rename.
///
/// En Windows `rename` falla si el destino existe, asi que se borra primero.
/// Es la unica diferencia de plataforma y esta contenida aqui.
pub fn escribir_atomico(destino: &Path, datos: &[u8]) -> Result<()> {
    if let Some(padre) = destino.parent() {
        std::fs::create_dir_all(padre).map_err(|e| SeniorError::Storage {
            path: padre.to_path_buf(),
            source: e,
        })?;
    }

    let temporal = destino.with_extension("tmp");
    std::fs::write(&temporal, datos).map_err(|e| SeniorError::Storage {
        path: temporal.clone(),
        source: e,
    })?;

    if destino.exists() {
        let _ = std::fs::remove_file(destino);
    }
    std::fs::rename(&temporal, destino).map_err(|e| SeniorError::Storage {
        path: destino.to_path_buf(),
        source: e,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn storage_temporal() -> (tempfile::TempDir, Storage) {
        let dir = tempfile::tempdir().expect("tempdir");
        let storage = Storage::new(dir.path());
        (dir, storage)
    }

    #[test]
    fn init_crea_la_estructura_y_es_idempotente() {
        let (_dir, storage) = storage_temporal();
        let config = ProjectConfig::default();

        storage.init(&config).expect("init");
        assert!(storage.is_initialized());
        assert!(storage.profile_path().exists());
        assert!(storage.sessions_dir().exists());

        // Un segundo init no debe pisar nada.
        let mut perfil = storage.load_profile().expect("carga");
        perfil.push_recent_error("E0308");
        storage.save_profile(&perfil).expect("guarda");

        storage.init(&config).expect("segundo init");
        assert_eq!(
            storage.load_profile().expect("carga").recent_errors.len(),
            1
        );
    }

    #[test]
    fn el_perfil_sobrevive_al_ciclo_de_guardado() {
        let (_dir, storage) = storage_temporal();
        storage.init(&ProjectConfig::default()).expect("init");

        let mut perfil = StudentProfile::fake_intermediate_null_safety();
        perfil.push_recent_error("NullPointerException");
        storage.save_profile(&perfil).expect("guarda");

        let cargado = storage.load_profile().expect("carga");
        assert_eq!(cargado.level, perfil.level);
        assert_eq!(cargado.recent_errors, perfil.recent_errors);
    }

    #[test]
    fn se_niega_a_guardar_un_perfil_invalido() {
        let (_dir, storage) = storage_temporal();
        storage.init(&ProjectConfig::default()).expect("init");

        let mut malo = StudentProfile::fake_beginner();
        malo.student_id = String::new();
        assert!(storage.save_profile(&malo).is_err());

        // El perfil bueno sigue en disco.
        assert!(storage.load_profile().is_ok());
    }

    #[test]
    fn un_perfil_corrupto_se_recupera_desde_el_respaldo() {
        let (_dir, storage) = storage_temporal();
        storage.init(&ProjectConfig::default()).expect("init");

        // Primer guardado bueno, segundo guardado para generar el .bak.
        let mut bueno = StudentProfile::fake_intermediate_null_safety();
        storage.save_profile(&bueno).expect("guarda 1");
        bueno.push_recent_error("otro error");
        storage.save_profile(&bueno).expect("guarda 2");

        // Alguien corrompe el archivo activo.
        std::fs::write(storage.profile_path(), "{ esto no es json").expect("corrompe");

        let recuperado = storage.load_profile().expect("recupera del respaldo");
        assert_eq!(recuperado.level, bueno.level);
    }

    #[test]
    fn la_config_va_y_viene_por_toml() {
        let (_dir, storage) = storage_temporal();
        let config = ProjectConfig {
            provider: "deepseek".to_string(),
            project_id: "seniorcli".to_string(),
            ..Default::default()
        };

        storage.init(&config).expect("init");
        let cargada = storage.load_config().expect("carga config");
        assert_eq!(cargada, config);
    }

    #[test]
    fn la_escritura_atomica_no_deja_temporales() {
        let (dir, _storage) = storage_temporal();
        let destino = dir.path().join("dato.json");

        escribir_atomico(&destino, b"{\"a\":1}").expect("escribe");
        escribir_atomico(&destino, b"{\"a\":2}").expect("sobrescribe");

        assert_eq!(std::fs::read_to_string(&destino).expect("lee"), "{\"a\":2}");
        assert!(!destino.with_extension("tmp").exists());
    }

    #[test]
    fn encuentra_la_sesion_mas_reciente() {
        let (_dir, storage) = storage_temporal();
        storage.init(&ProjectConfig::default()).expect("init");
        assert!(storage.latest_session_path().is_none());

        std::fs::write(storage.sessions_dir().join("2026-01-01T000000.jsonl"), "").expect("a");
        std::fs::write(storage.sessions_dir().join("2026-09-16T120000.jsonl"), "").expect("b");

        let ultima = storage.latest_session_path().expect("hay sesion");
        assert!(ultima.to_string_lossy().contains("2026-09-16"));
    }
}
