//! Vertical slice: los tres frentes conectados de punta a punta.
//!
//! Este test es el contrato vivo del proyecto. Mientras pase, los tres frentes
//! siguen encajando; cuando se rompe, alguien cambio un contrato sin avisar.
//!
//! **No lo borres ni lo ignores para que pase el CI.** Si falla, o el cambio
//! esta mal, o hay que discutir el contrato en un PR contra `src/contracts/`.
//!
//! Caso: el alumno ejecuta `senior debug` en un proyecto Java con un
//! `NullPointerException` (seccion 5.1 de la guia).

use std::path::Path;

use seniorcli::app::session::{SessionState, Turn};
use seniorcli::app::storage::{ProjectConfig, Storage};
use seniorcli::contracts::{
    ContextEngine, ContextFocus, Intervention, MockContextEngine, ProfileStore, TutorRequest,
};
use seniorcli::learning::{Evidence, TeachingPolicy, build_engine, record_evidence};

/// Prepara un proyecto temporal ya inicializado.
fn proyecto_listo() -> (tempfile::TempDir, Storage) {
    let dir = tempfile::tempdir().expect("tempdir");
    let storage = Storage::new(dir.path());
    storage.init(&ProjectConfig::default()).expect("init");
    (dir, storage)
}

#[tokio::test]
async fn flujo_completo_frente_1_a_frente_3() {
    let (_dir, storage) = proyecto_listo();

    // --- Frente 1: el proyecto real se convierte en contexto ---------------
    // Aqui va el mock; cuando el detector de Java este listo, se cambia por
    // `ContextCollector` y este test no se toca.
    let context = MockContextEngine::java_null_pointer()
        .collect(Path::new("."), ContextFocus::LastError)
        .await
        .expect("el frente 1 entrega contexto");

    assert!(context.has_evidence(), "el contexto debe traer evidencia");
    let error = context.first_error().expect("hay un error");
    assert_eq!(error.line, Some(23));

    // --- Frente 2: contexto + perfil -> intervencion educativa -------------
    let mut profile = storage.load_profile().expect("carga perfil");
    let engine = build_engine("mock", TeachingPolicy::default()).expect("motor");

    let decision = engine
        .decide(TutorRequest::new(
            "me sale NullPointerException en UserService linea 23, por que?",
            context.clone(),
            profile.clone(),
        ))
        .await
        .expect("el frente 2 decide");

    assert!(decision.validate().is_ok(), "la decision debe ser valida");
    assert_eq!(decision.intervention, Intervention::Hint);
    // Lo pedagogico: da una pista y devuelve la pelota, no resuelve el ejercicio.
    assert!(decision.question.is_some());
    assert!(decision.intervention.help_level() < Intervention::GuidedSolution.help_level());

    // --- Frente 3: presentacion y persistencia ------------------------------
    let render = seniorcli::app::ui::render_decision(&decision);
    // El alumno siempre ve que tipo de ayuda recibio y sobre que concepto. Como
    // se maqueta eso es del Frente 3; que aparezca, no.
    assert!(render.contains(decision.intervention.as_str()));
    assert!(render.contains(&decision.concept));

    let mut session = SessionState::start(storage.new_session_path().expect("ruta de sesion"));
    session
        .record(Turn::new("me sale NullPointerException", decision.clone()).with_latency(12))
        .expect("guarda el turno");

    record_evidence(
        &mut profile,
        &decision.concept,
        Evidence::from_intervention(decision.intervention),
    );
    storage.save_profile(&profile).expect("guarda el perfil");

    // --- El estado sobrevive a un cierre -----------------------------------
    let recargado = storage.load_profile().expect("recarga el perfil");
    assert!(recargado.topic("null-safety").is_some());

    let retomada = SessionState::resume(session.path()).expect("retoma la sesion");
    assert_eq!(retomada.len(), 1);
    assert_eq!(
        retomada.last_turn().expect("hay turno").decision.concept,
        "null-safety"
    );
}

#[tokio::test]
async fn una_peticion_vaga_termina_en_una_pregunta_y_no_en_una_solucion() {
    let (_dir, storage) = proyecto_listo();
    let profile = storage.load_profile().expect("carga perfil");

    let context = MockContextEngine::ambiguous()
        .collect(Path::new("."), ContextFocus::Unspecified)
        .await
        .expect("contexto");

    let engine = build_engine("mock", TeachingPolicy::default()).expect("motor");
    let decision = engine
        .decide(TutorRequest::new("no funciona", context, profile))
        .await
        .expect("decide");

    assert_eq!(decision.intervention, Intervention::Clarify);
    assert!(decision.question.is_some());
    assert!(decision.validate().is_ok());
}

#[tokio::test]
async fn el_harness_no_entrega_la_solucion_aunque_el_modelo_insista() {
    use seniorcli::contracts::TutorEngine;
    use seniorcli::learning::{LearningEngine, MockProvider};

    let (_dir, storage) = proyecto_listo();
    let profile = storage.load_profile().expect("carga perfil");

    let context = MockContextEngine::java_null_pointer()
        .collect(Path::new("."), ContextFocus::LastError)
        .await
        .expect("contexto");

    // El proveedor devuelve una solucion completa, sin que nadie se la pidiera.
    let engine =
        LearningEngine::new(MockProvider::over_helpful()).with_policy(TeachingPolicy::socratica());

    let decision = engine
        .decide(TutorRequest::new(
            "por que truena en la linea 23?",
            context,
            profile,
        ))
        .await
        .expect("decide");

    assert!(decision.intervention.help_level() <= Intervention::Hint.help_level());
    assert!(
        !decision.message.contains("Optional.ofNullable"),
        "la solucion literal nunca debe llegar al alumno"
    );
}

#[tokio::test]
async fn una_caida_del_proveedor_deja_la_sesion_utilizable() {
    use seniorcli::contracts::TutorEngine;
    use seniorcli::learning::{LearningEngine, MockProvider};

    let (_dir, storage) = proyecto_listo();
    let profile = storage.load_profile().expect("carga perfil");

    let context = MockContextEngine::java_null_pointer()
        .collect(Path::new("."), ContextFocus::LastError)
        .await
        .expect("contexto");

    let engine = LearningEngine::new(MockProvider::failing("504 gateway timeout"));
    let decision = engine
        .decide(TutorRequest::new("por que truena?", context, profile))
        .await
        .expect("un fallo del proveedor no debe romper el turno");

    assert!(decision.validate().is_ok());
    assert_eq!(decision.intervention, Intervention::Clarify);
}
