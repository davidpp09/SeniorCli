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
    ContextEngine, ContextFocus, ProfileStore, ProjectContext, Result, StudentProfile,
    TutorDecision, TutorEngine, TutorRequest,
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

    // `senior` a secas abre la sesion interactiva: es la forma normal de usar
    // la herramienta. Los subcomandos quedan para scripts y para entrar directo
    // a un modo concreto.
    let Some(command) = &cli.command else {
        return cmd_sesion(&root, TeachingPolicy::default()).await;
    };

    match command {
        Commands::Init { provider, force } => cmd_init(&root, provider, *force),
        Commands::Progress { sessions } => cmd_progress(&root, *sessions),
        Commands::Learn { message, goal } => {
            let politica = command.policy();
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
            let politica = command.policy();
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

/// Sesion interactiva: lo que pasa al escribir `senior` a secas.
///
/// Dos diferencias con `cmd_turno`, y son justo las que cambian la sensacion de
/// la herramienta:
///
/// 1. el proyecto se analiza **una vez** al entrar, no en cada pregunta;
/// 2. la respuesta del alumno a la pregunta del tutor es simplemente su
///    siguiente mensaje, como en una conversacion.
async fn cmd_sesion(root: &std::path::Path, politica: TeachingPolicy) -> Result<()> {
    let mut state = AppState::load(root.to_path_buf())?;

    println!(
        "{}",
        ui::bienvenida(
            &state.config.project_id,
            &state.project_root,
            &state.config.provider,
            politica.ceiling,
        )
    );

    let collector = ContextCollector::new();
    println!("Analizando el proyecto...");
    let mut context = collector
        .collect(&state.project_root, ContextFocus::LastError)
        .await?;
    println!();
    println!("{}", ui::render_evidencia(&context));

    let engine = build_engine(&state.config.provider, politica)?;
    let mut prompt = repl::StdinPrompt;

    bucle(
        &mut state,
        &mut context,
        engine.as_ref(),
        &collector,
        &mut prompt,
    )
    .await?;

    println!();
    println!(
        "{}",
        ui::despedida(state.session.len(), state.session.path())
    );

    Ok(())
}

/// El bucle de la sesion.
///
/// Esta separado de [`cmd_sesion`] a proposito: recibe el `Prompt` por
/// parametro, asi que los tests pueden guionizar una conversacion entera sin
/// teclado, con [`repl::ScriptedPrompt`].
async fn bucle<P: repl::Prompt>(
    state: &mut AppState,
    context: &mut ProjectContext,
    engine: &dyn TutorEngine,
    collector: &ContextCollector,
    prompt: &mut P,
) -> Result<()> {
    // La pregunta que quedo abierta en el turno anterior. Cuando el alumno
    // vuelve a escribir, esa linea es su respuesta: no hay que pedirsela aparte.
    let mut pendiente: Option<TutorDecision> = None;

    // El bucle termina cuando el alumno escribe `/salir` o cierra la entrada
    // con Ctrl+D / Ctrl+Z.
    while let Some(linea) = prompt.ask(">")? {
        match repl::interpretar(&linea) {
            repl::Entrada::Vacia => continue,
            repl::Entrada::Salir => break,
            repl::Entrada::Ayuda => println!("{}", ui::ayuda()),
            repl::Entrada::Progreso => println!("{}", ui::render_progress(&state.profile)),
            repl::Entrada::Contexto => println!("{}", ui::render_evidencia(context)),
            repl::Entrada::Analizar => {
                println!("Volviendo a mirar el proyecto...");
                *context = collector
                    .collect(&state.project_root, ContextFocus::LastError)
                    .await?;
                println!();
                println!("{}", ui::render_evidencia(context));
            }
            repl::Entrada::Desconocida(comando) => {
                println!("No conozco `/{comando}`. Escribe /ayuda para ver la lista.");
            }
            repl::Entrada::Mensaje(mensaje) => {
                // Si habia una pregunta abierta, este mensaje la contesta: eso
                // dice cuanto le costo el concepto y se registra como evidencia.
                if let Some(previa) = pendiente.take() {
                    record_evidence(
                        &mut state.profile,
                        &previa.concept,
                        Evidence::from_intervention(previa.intervention),
                    );
                }

                println!();
                println!("Pensando...");

                let inicio = std::time::Instant::now();
                let decision = engine
                    .decide(TutorRequest::new(
                        &mensaje,
                        context.clone(),
                        state.profile.clone(),
                    ))
                    .await?;
                let latencia = inicio.elapsed().as_millis();

                println!();
                println!("{}", ui::render_decision(&decision));

                state.profile.push_recent_error(&mensaje);
                state.persistir_turno(
                    Turn::new(&mensaje, decision.clone()).with_latency(latencia),
                )?;

                if decision.question.is_some() {
                    pendiente = Some(decision);
                }
            }
        }
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
    async fn la_sesion_encadena_turnos_sin_volver_a_analizar() {
        let dir = tempfile::tempdir().expect("tempdir");
        cmd_init(dir.path(), "mock", false).expect("init");
        let mut state = AppState::load(dir.path().to_path_buf()).expect("carga");

        let mut context = MockContextEngine::java_null_pointer()
            .collect(dir.path(), ContextFocus::LastError)
            .await
            .expect("contexto");

        let engine = build_engine("mock", TeachingPolicy::default()).expect("motor");
        let collector = ContextCollector::new().without_diagnostics();
        let mut prompt = repl::ScriptedPrompt::new([
            "por que falla findById?",
            "/ayuda",
            "creo que devuelve null cuando el id no existe",
            "/salir",
        ]);

        bucle(
            &mut state,
            &mut context,
            engine.as_ref(),
            &collector,
            &mut prompt,
        )
        .await
        .expect("la sesion corre entera");

        // Dos mensajes = dos turnos: los comandos no ensucian la sesion.
        assert_eq!(state.session.len(), 2);

        // La segunda linea contesto la pregunta del primer turno, y eso dejo
        // huella en el perfil sin que el alumno hiciera nada especial.
        assert!(state.profile.topic("null-safety").is_some());

        let sesion = SessionState::resume(state.session.path()).expect("retoma");
        assert_eq!(sesion.len(), 2);
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
