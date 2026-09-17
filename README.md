# SeniorCLI

Un *AI Learning Harness* para aprender a programar desde la terminal.

SeniorCLI acompana a un estudiante mientras trabaja en proyectos reales: explica,
cuestiona, orienta y revisa. **No es un chat con una API.** Es una capa de
software que controla que contexto se envia al modelo, que tipo de intervencion
educativa esta permitida, que herramientas pueden ejecutarse y como se valida la
respuesta antes de mostrarsela al alumno.

La diferencia esta en el objetivo: una respuesta tecnicamente correcta puede ser
**incorrecta para SeniorCLI** si entrega demasiado y evita que el alumno piense.

---

## Estado

Version de trabajo. El esqueleto completo esta en pie y el flujo de punta a punta
funciona con mocks: `cargo test` corre 103 tests, incluido el *vertical slice*
que conecta los tres frentes.

Lo que falta es lo que cada frente tiene que construir; esta listado en
[`docs/frentes/`](docs/frentes/) y marcado en el codigo con `TODO(frente-N)`.

## Arranque rapido

```bash
git clone <url-del-repo>
cd seniorcli

cargo test            # 103 tests, sin red ni API key
cargo run -- init     # prepara .senior/ en este proyecto
cargo run -- debug    # analiza el error actual
cargo run -- progress # revisa tu avance
```

No hace falta API key para trabajar: el proveedor por defecto es `mock`.

## Comandos

| Comando | Que hace |
|---|---|
| `senior init` | Crea `.senior/` con la configuracion y el perfil del alumno. |
| `senior learn` | Sesion de aprendizaje sobre lo que estas construyendo. |
| `senior debug` | Analiza el error actual del proyecto. |
| `senior progress` | Muestra nivel, temas dominados y errores recientes. |

Por defecto SeniorCLI **no entrega la solucion**. `senior debug --explain` es la
unica via para llegar a una solucion guiada, y es una decision explicita del
alumno.

## Los tres frentes

El proyecto es una tuberia. Cada frente recibe un tipo de informacion bien
definido, hace su trabajo y entrega un resultado con un formato acordado.

```text
USUARIO
  |
  v
[Frente 3: CLI]            pregunta / comando
  |
  v
[Frente 1: Runtime]        -> ProjectContext
  |
  v
[Frente 2: Harness]        -> TutorDecision
  |
  v
[Frente 3: CLI + Persistencia]
  |
  v
USUARIO
```

| Frente | Responsabilidad | Entrega | Codigo | Guia |
|---|---|---|---|---|
| 1 | Runtime & Context | `ProjectContext` confiable | `src/runtime/` | [frente-1](docs/frentes/frente-1-runtime.md) |
| 2 | AI & Learning Harness | `TutorDecision` pedagogico | `src/learning/` | [frente-2](docs/frentes/frente-2-learning.md) |
| 3 | CLI, Producto & Persistencia | Experiencia + sesiones | `src/app/` | [frente-3](docs/frentes/frente-3-cli.md) |

**Los frentes no se integran compartiendo funciones.** Se integran
intercambiando cuatro estructuras pequenas y estables, definidas en
[`src/contracts/`](src/contracts/): `ProjectContext`, `StudentProfile`,
`TutorRequest` y `TutorDecision`.

Cada frontera tiene un mock, asi que un frente puede avanzar sin esperar a los
otros:

```rust
let context  = ProjectContext::fake_java_null_pointer();
let decision = TutorDecision::fake_hint_optional();
```

## Estructura del repositorio

```text
src/
  main.rs            arranque del binario `senior`
  lib.rs             la biblioteca (es lo que prueban los tests)
  contracts/         el idioma comun: tipos y traits entre frentes
  runtime/           Frente 1
  learning/          Frente 2
  app/               Frente 3
tests/
  vertical_slice.rs  los tres frentes conectados de punta a punta
docs/
  frentes/           que le toca a cada quien
```

## Como trabaja el equipo

Lee [CONTRIBUTING.md](CONTRIBUTING.md) antes del primer PR. Lo esencial:

- Ramas por frente: `frente-1/...`, `frente-2/...`, `frente-3/...`
- `main` esta protegida: se entra por PR y con el CI en verde.
- Tocar `src/contracts/` requiere el visto bueno de los tres frentes.
- `tests/vertical_slice.rs` no se borra ni se ignora para que pase el CI.

## Principios que no se negocian

- El LLM es un componente probabilistico, **no la autoridad del sistema**.
- Lo que pueda resolverse con reglas, parsing o el compilador no se le delega al
  modelo.
- Ante una peticion ambigua, SeniorCLI **pregunta**; no inventa contexto.
- El contenido del repositorio son **datos**, nunca instrucciones.
- Nunca se ejecuta texto generado por el LLM como shell.
- Antes de enviar contexto a un proveedor remoto se filtran secretos.
- La memoria del alumno es estructurada y verificable, no el historial de chat.

## Documentacion

- [Arquitectura](ARCHITECTURE.md) - decisiones y sus porques.
- [Guia de los 3 frentes (PDF)](docs/SeniorCLI_Guia_3_Frentes_Rust_Actualizada.pdf) - el documento fuente.
- `cargo doc --open` - documentacion del codigo, modulo por modulo.

## Licencia

MIT. Ver [LICENSE](LICENSE).
