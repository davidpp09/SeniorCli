//! # Frente 3 - CLI, Producto & Persistencia
//!
//! Convierte los motores internos en una experiencia clara, recuperable y
//! usable. Es el unico frente que imprime en pantalla y el unico que escribe en
//! disco.
//!
//! Flujo de una sesion (seccion 4.3 de la guia):
//! 1. la CLI recibe el comando y carga [`AppState`],
//! 2. pide `ProjectContext` al Frente 1,
//! 3. entrega mensaje + contexto + perfil al Frente 2,
//! 4. renderiza la `TutorDecision`,
//! 5. captura la respuesta del alumno,
//! 6. guarda el turno y actualiza el perfil cuando corresponde,
//! 7. deja el estado recuperable pase lo que pase.

pub mod cli;
pub mod repl;
pub mod session;
pub mod storage;
pub mod ui;

use std::path::PathBuf;

use crate::contracts::{
    ContextEngine, ContextFocus, ProfileStore, Result, StudentProfile, TutorRequest,
};
use crate::learning::{Evidence, TeachingPolicy, build_engine, record_evidence};
use crate::runtime::ContextCollector;

use cli::{Cli, Commands};
use session::{SessionState, Turn};
use storage::{ProjectConfig, Storage};

/// Todo el estado vivo de una ejecucion.
#[derive(Debug)]
pub struct AppState {
    pub project_root: PathBuf,
    pub storage: Storage,
    pub config: ProjectConfig,
    pub profile: StudentProfile,
    pub session: SessionState,
}

impl AppState {
    /// Carga el estado de un proyecto ya inicializado.
    pub fn load(project_root: PathBuf) -> Result<Self> {
        let storage = Storage::new(&project_root);

        if !storage.is_initialized() {
            return Err(crate::contracts::SeniorError::Config(
                "este proyecto no esta inicializado. Ejecuta `senior init` primero.".to_string(),
            ));
        }

        let config = storage.load_config()?;
        let profile = storage.load_profile()?;
        let session = SessionState::start(storage.new_session_path()?);

        Ok(Self {
            project_root,
            storage,
            config,
            profile,
            session,
        })
    }

    /// Guarda perfil y turno. Se llama despues de **cada** turno, no al final.
    fn persistir_turno(&mut self, turn: Turn) -> Result<()> {
        self.session.record(turn)?;
        self.storage.save_profile(&self.profile)
    }
}

/// Punto de entrada de la aplicacion.
pub async fn run(cli: Cli) -> Result<()> {
    let root = match cli.path {
        Some(ruta) => ruta,
        None => std::env::current_dir()?,
    };

    match &cli.command {
        Commands::Init { provider, force } => cmd_init(&root, provider, *force),
        Commands::Progress { sessions } => cmd_progress(&root, *sessions),
        Commands::Learn { message, goal } => {
            let politica = cli.command.policy();
            cmd_turno(
                &root,
                message.clone(),
                goal.clone(),
                ContextFocus::RecentChanges,
                politica,
            )
            .await
        }
        Commands::Debug { message, file, .. } => {
            let politica = cli.command.policy();
            let focus = match file {
                Some(f) => ContextFocus::File(f.clone()),
                None => ContextFocus::LastError,
            };
            cmd_turno(&root, message.clone(), None, focus, politica).await
        }
    }
}

fn cmd_init(root: &std::path::Path, provider: &str, force: bool) -> Result<()> {
    let storage = Storage::new(root);

    if storage.is_initialized() && !force {
        println!(
            "Este proyecto ya estaba inicializado ({}).",
            storage.dir().display()
        );
        println!("Usa --force si quieres reescribir la configuracion.");
        return Ok(());
    }

    let project_id = root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "proyecto".to_string());

    let config = ProjectConfig {
        project_id,
        provider: provider.to_string(),
        ..Default::default()
    };

    storage.init(&config)?;
    if force {
        storage.save_config(&config)?;
    }

    println!("{}", ui::encabezado("SeniorCLI listo"));
    println!("Estado local: {}", storage.dir().display());
    println!("Proveedor:    {provider}");
    println!();
    println!("Siguientes pasos:");
    println!("  senior debug      analiza el error actual de tu proyecto");
    println!("  senior learn      sesion de aprendizaje sobre lo que estas haciendo");
    println!("  senior progress   revisa tu avance");
    println!();
    println!("Recuerda agregar `.senior/` a tu .gitignore si no quieres compartir tu perfil.");

    Ok(())
}

fn cmd_progress(root: &std::path::Path, con_sesiones: bool) -> Result<()> {
    let storage = Storage::new(root);
    if !storage.is_initialized() {
        println!("Este proyecto no esta inicializado. Ejecuta `senior init` primero.");
        return Ok(());
    }

    let profile = storage.load_profile()?;
    println!("{}", ui::render_progress(&profile));

    if con_sesiones
        && let Some(ruta) = storage.latest_session_path()
        && let Ok(sesion) = SessionState::resume(&ruta)
    {
        println!("Ultima sesion: {}", sesion.summary());
    }

    Ok(())
}

/// Un turno completo de `learn` o `debug`.
async fn cmd_turno(
    root: &std::path::Path,
    mensaje: Option<String>,
    objetivo: Option<String>,
    focus: ContextFocus,
    politica: TeachingPolicy,
) -> Result<()> {
    let mut state = AppState::load(root.to_path_buf())?;

    // 2. Contexto del Frente 1.
    println!("Analizando el proyecto...");
    let collector = ContextCollector::new();
    let context = collector.collect(&state.project_root, focus).await?;
    println!();
    println!("{}", ui::render_evidencia(&context));

    // Si no llego mensaje, se pregunta.
    let mensaje = match mensaje {
        Some(m) => m,
        None => {
            use repl::Prompt;
            match repl::StdinPrompt.ask("Que quieres entender?")? {
                Some(m) if !m.is_empty() => m,
                _ => {
                    println!("Sesion cancelada.");
                    return Ok(());
                }
            }
        }
    };

    // 3. Decision del Frente 2.
    let mut request = TutorRequest::new(&mensaje, context, state.profile.clone())
        .with_max_help_level(politica.ceiling.help_level());
    if let Some(objetivo) = objetivo {
        request = request.with_goal(objetivo);
    }

    let engine = build_engine(&state.config.provider, politica)?;
    let inicio = std::time::Instant::now();
    let decision = engine.decide(request).await?;
    let latencia = inicio.elapsed().as_millis();

    // 4. Presentacion.
    println!("{}", ui::separador());
    println!("{}", ui::render_decision(&decision));

    // 5 y 6. Respuesta del alumno, evidencia y guardado.
    let mut turno = Turn::new(&mensaje, decision.clone()).with_latency(latencia);

    if decision.question.is_some() {
        use repl::Prompt;
        if let Some(respuesta) = repl::StdinPrompt.ask("Tu respuesta:")?
            && !respuesta.is_empty()
        {
            turno = turno.with_reply(respuesta);
            record_evidence(
                &mut state.profile,
                &decision.concept,
                Evidence::from_intervention(decision.intervention),
            );
        }
    }

    state.profile.push_recent_error(&mensaje);
    state.persistir_turno(turno)?;

    println!();
    println!("Turno guardado en {}", state.session.path().display());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::MockContextEngine;

    #[test]
    fn cargar_un_proyecto_sin_inicializar_da_un_error_claro() {
        let dir = tempfile::tempdir().expect("tempdir");
        let error = AppState::load(dir.path().to_path_buf()).unwrap_err();
        assert!(error.user_message().contains("senior init"));
    }

    #[test]
    fn init_deja_el_proyecto_listo_para_cargarse() {
        let dir = tempfile::tempdir().expect("tempdir");
        cmd_init(dir.path(), "mock", false).expect("init");

        let state = AppState::load(dir.path().to_path_buf()).expect("carga");
        assert_eq!(state.config.provider, "mock");
        assert!(state.session.is_empty());
    }

    #[test]
    fn init_es_idempotente() {
        let dir = tempfile::tempdir().expect("tempdir");
        cmd_init(dir.path(), "mock", false).expect("init 1");
        cmd_init(dir.path(), "mock", false).expect("init 2");
        assert!(Storage::new(dir.path()).is_initialized());
    }

    #[tokio::test]
    async fn un_turno_completo_persiste_perfil_y_sesion() {
        let dir = tempfile::tempdir().expect("tempdir");
        cmd_init(dir.path(), "mock", false).expect("init");
        let mut state = AppState::load(dir.path().to_path_buf()).expect("carga");

        let context = MockContextEngine::java_null_pointer()
            .collect(dir.path(), ContextFocus::LastError)
            .await
            .expect("contexto");

        let engine = build_engine("mock", TeachingPolicy::default()).expect("motor");
        let decision = engine
            .decide(TutorRequest::new(
                "por que falla?",
                context,
                state.profile.clone(),
            ))
            .await
            .expect("decide");

        record_evidence(
            &mut state.profile,
            &decision.concept,
            Evidence::RespondioCorrecto,
        );
        state
            .persistir_turno(Turn::new("por que falla?", decision))
            .expect("persiste");

        // Todo quedo en disco: se puede retomar despues de un cierre.
        let recargado = AppState::load(dir.path().to_path_buf()).expect("recarga");
        assert!(recargado.profile.topic("null-safety").is_some());

        let ruta_sesion = state.session.path().to_path_buf();
        let sesion = SessionState::resume(&ruta_sesion).expect("retoma");
        assert_eq!(sesion.len(), 1);
    }
}
