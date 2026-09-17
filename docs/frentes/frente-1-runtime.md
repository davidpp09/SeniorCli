# Frente 1 - Runtime & Context

**2 personas + 1 responsable tecnico.**
**Entrega principal:** un `ProjectContext` confiable.

Hacen que SeniorCLI entienda el proyecto real antes de preguntarle al LLM.

## Su codigo

```text
src/runtime/
  mod.rs          ContextCollector: orquesta el flujo completo
  detector.rs     raiz del proyecto y stack          [funcionando]
  git.rs          estado, diff y archivos tocados     [funcionando]
  runner.rs       ejecucion controlada de comandos    [funcionando]
  files.rs        lectura, limites y secretos         [parcial]
  diagnostics.rs  parseo de errores                   [solo Rust]
```

Su unica salida publica es `ProjectContext`. Lo que **no** les toca: elegir el
tipo de intervencion, hablar con el LLM, imprimir en pantalla.

## Que ya funciona

Corran esto para verlo:

```bash
cargo test runtime::
cargo run -- debug   # sobre este mismo repo
```

- `detector.rs` reconoce Rust, Java (maven/gradle), Python, Node y Go por sus
  marcadores de build, sube hasta la raiz del repo y soporta monorepos.
- `git.rs` da archivos modificados y diff recortado a 8.000 caracteres.
- `runner.rs` ejecuta comandos de una lista blanca, sin shell y con timeout.
- `files.rs` recorta rangos alrededor de un error, aplica limites de tamano y
  filtra secretos por nombre, extension y contenido.
- `diagnostics.rs` parsea la salida de `cargo`.

## Que les toca construir

En orden sugerido. Cada punto tiene el archivo y el marcador `TODO(frente-1)`.

### 1. Parser de Java (`diagnostics.rs::parse_java`)

Es el que desbloquea el caso vertical inicial. Formatos objetivo:

```text
[ERROR] /ruta/UserService.java:[23,15] cannot find symbol
at demo.UserService.nombreDe(UserService.java:23)
```

**Hecho cuando:** un fixture con salida de maven produce un `Diagnostic` con
archivo, linea y mensaje correctos, y el test pasa en los tres sistemas.

### 2. Parser de Python (`diagnostics.rs::parse_python`)

```text
  File "app.py", line 23, in nombre_de
TypeError: unsupported operand type(s)
```

Ojo con `pytest`: su salida es distinta de un traceback suelto.

**Hecho cuando:** un traceback y una salida de `pytest` producen diagnosticos
con ubicacion.

### 3. Explorador de archivos que respete `.gitignore`

Ahora mismo `select_relevant_files` solo ordena candidatos que alguien mas le
dio. Falta recorrer el repo de verdad.

- Descomenten `ignore` y `walkdir` en `Cargo.toml` (seccion Frente 1).
- Respeten `.gitignore` y los limites de tamano que ya estan en `files.rs`.

**Hecho cuando:** en un repo con `node_modules/` y `target/`, el contexto nunca
los incluye.

### 4. Top-K de relevancia real

Hoy el orden es: diagnostico > pedido > modificado > relacionado, y se corta en
6 archivos. Falta una puntuacion de verdad (cercania al error, recencia en git,
tamano).

**Hecho cuando:** ante 30 archivos candidatos, los 6 elegidos son los que un
humano elegiria para ese error.

### 5. Simbolos relacionados (`RelevanceReason::RelatedSymbol`)

Si el error es `u is null` en `UserService`, conviene incluir tambien la
definicion de `User` y de `repo`. Basta con busqueda textual de definiciones al
inicio; el AST no hace falta todavia.

### 6. Mas comandos de diagnostico (`mod.rs::comando_de_diagnostico`)

Agreguen Python y Node **solo cuando su parser exista**. Ejecutar un build que
nadie sabe leer solo gasta tiempo.

### 7. Afinar el escaner de secretos

`files.rs::PISTAS_DE_SECRETO` usa comparacion literal. Si empieza a dar falsos
positivos molestos, cambienlo a `regex` (ya esta comentado en `Cargo.toml`).

Mantengan la regla: **ante la duda, el archivo no entra**.

## Lo que NO necesitan todavia

Indexacion semantica, AST completo, sandbox avanzado, soporte de mas de dos
stacks.

## Riesgos que les tocan a ustedes

| Riesgo | Como lo evitamos |
|---|---|
| Contexto insuficiente | Incluir archivo, rango y simbolos relacionados, no solo el error. |
| Contexto excesivo | Limites por archivo, top-K, prioridad a lo modificado. |
| Peticion vaga | **No inventar el objetivo.** Llenar `ProjectContext::ambiguity` y dejar que el Frente 2 pregunte. |
| Stack mal detectado | `StackInfo` admite varios componentes y una raiz activa. |
| Comando peligroso | Lista blanca en `runner.rs`. Nunca `sh -c`. Nunca texto del LLM. |
| Secretos | Filtrar por nombre, extension y contenido. No confiar en la extension. |

## Prueba minima del frente

> Un fixture de Java o Python produce el `ProjectContext` esperado: stack,
> archivo, linea y diagnostico.

Pongan los fixtures en `tests/fixtures/` y el test en `tests/`.

## Como integrarse con los demas

No necesitan a nadie para avanzar. Cuando su `ContextCollector` este listo para
Java, en `tests/vertical_slice.rs` se cambia `MockContextEngine` por
`ContextCollector` y el resto del test no se toca.
