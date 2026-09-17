# Como trabajamos en SeniorCLI

Somos siete personas en tres frentes sobre el mismo repositorio. Estas reglas
existen para que eso no termine en conflictos de merge y contratos rotos.

## 1. Antes de escribir codigo

```bash
rustup update stable          # necesitamos Rust 1.85 o mas (edition 2024)
cargo test                    # debe pasar todo antes de que empieces
cp .env.example .env          # opcional: solo si vas a usar un proveedor real
```

Si `cargo test` no pasa en tu maquina recien clonado el repo, eso es un bug y va
a un issue antes de cualquier otra cosa.

## 2. Ramas

```text
main                 siempre desplegable, protegida
  frente-1/detector-java
  frente-2/transporte-http
  frente-3/repl-rustyline
  fix/...            correcciones
  chore/...          infraestructura, dependencias, CI
```

Nunca se hace push directo a `main`. Se abre PR.

### Proteccion de `main` (lo configura quien coordina)

> Esto solo se puede hacer **despues** del primer push, y los status checks
> solo aparecen en la lista una vez que el CI corrio al menos una vez.

En *Settings > Branches > Add branch protection rule*, patron `main`:

- Require a pull request before merging (1 aprobacion minimo)
- Require review from Code Owners
- Require status checks to pass: `Formato`, `Clippy`, `Tests (ubuntu-latest)`,
  `Tests (windows-latest)`, `Tests (macos-latest)`
- Require branches to be up to date before merging
- Do not allow bypassing the above settings

Si GitHub no te deja crear la regla, revisa el plan: en repositorios privados
la proteccion de ramas suele requerir plan de pago. Alternativas: hacer el repo
publico, usar una organizacion con plan Team, o quedarse solo con la disciplina
de PR + CI en verde.

## 3. Commits

Formato [Conventional Commits](https://www.conventionalcommits.org/):

```text
feat(runtime): parsear stack traces de Java
fix(app): no sobrescribir el perfil cuando el JSON es parcial
test(learning): cubrir el recorte por sobre-ayuda
docs(contracts): explicar por que Ambiguity vive en ProjectContext
chore(ci): agregar macOS a la matriz de tests
```

Ambitos: `runtime`, `learning`, `app`, `contracts`, `ci`, `docs`.

El mensaje explica **por que**, no que lineas cambiaron: eso ya esta en el diff.

## 4. El contrato es sagrado

`src/contracts/` es el idioma comun de los tres frentes. Cambiar un tipo de ahi
rompe a los otros dos equipos, posiblemente sin que lo noten hasta el merge.

Reglas:

1. Un frente **solo** importa de `crate::contracts`, nunca de otro frente.
   Si el Frente 3 necesita algo de `runtime::`, no lo importa directamente: se
   agrega al trait `ContextEngine`.
2. Cambiar `src/contracts/` requiere aprobacion de los tres responsables
   (configurado en `.github/CODEOWNERS`).
3. Habla el cambio **antes** de escribirlo. Un PR de contratos que llega sin
   aviso se cierra sin revisar.

## 5. El vertical slice no se toca

`tests/vertical_slice.rs` conecta los tres frentes de punta a punta. Es la
prueba de que el proyecto sigue siendo uno y no tres proyectos distintos.

Si falla: o tu cambio esta mal, o hay que discutir el contrato. **Nunca** se
resuelve con `#[ignore]` ni borrando el assert.

## 6. Trabajar aislado, integrar temprano

No esperes a que tu frente este terminado al 100% para integrarlo: eso acumula
incompatibilidades. Cada frontera tiene un mock listo para usar:

```rust
use seniorcli::contracts::{MockContextEngine, MockTutorEngine};
use seniorcli::learning::MockProvider;

let context  = MockContextEngine::java_null_pointer();  // finge el Frente 1
let engine   = MockTutorEngine::hint();                 // finge el Frente 2
let provider = MockProvider::over_helpful();            // finge al LLM
```

Integra un caso pequeno desde temprano y mantenlo funcionando.

## 7. Antes de abrir el PR

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

El CI corre exactamente esto en Linux, Windows y macOS. Si falla en tu maquina,
va a fallar alla.

Ademas:

- Tests para lo que agregaste. Un modulo nuevo sin tests no se aprueba.
- Documenta el **por que** en los comentarios `//!` del modulo, no solo el que.
- Si dejaste algo a medias, marcalo `TODO(frente-N)` con una frase de que falta.

## 8. Revisar un PR

Quien revisa busca, en este orden:

1. **Contratos.** Cambia algo que otro frente usa?
2. **Los principios del producto.** Se le esta delegando al modelo algo que
   podia resolverse con reglas? Se puede filtrar un secreto? Se puede ejecutar
   texto del LLM como comando? Se entrega mas ayuda de la permitida?
3. **Recuperabilidad.** Si esto falla a la mitad, el alumno pierde su perfil o
   su sesion?
4. **Tests.** Cubren el caso feo, no solo el feliz?
5. Legibilidad.

Aprobar un PR es hacerse corresponsable de lo que entra.

## 9. Registrar los fallos del harness

Cuando el modelo alucine, se pase de ayuda, rompa el JSON o pierda el hilo:
abre un issue con la plantilla **Fallo del harness**. No es burocracia, es la
unica forma de saber despues si el harness mejoro.

## 10. Decisiones de arquitectura

Van en [ARCHITECTURE.md](ARCHITECTURE.md). Si tomas una decision que alguien
podria querer revertir en tres meses, escribela ahi con el motivo.
