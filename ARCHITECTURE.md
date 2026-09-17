# Arquitectura de SeniorCLI

Este documento explica **por que** el codigo esta como esta. Para el que, lee el
codigo o corre `cargo doc --open`.

## La forma del sistema

SeniorCLI es una tuberia de tres etapas. Cada una tiene una entrada y una salida
bien definidas, y no conoce el interior de las demas.

```text
mensaje del alumno
      |
      v
Frente 1: Runtime & Context   --> ProjectContext
      |
      v
Frente 2: AI & Learning       --> TutorDecision
      |
      v
Frente 3: CLI & Persistencia  --> pantalla + estado en disco
```

La frontera entre etapas son traits, no funciones sueltas:

| Trait | Frontera | Doble para tests |
|---|---|---|
| `ContextEngine` | Frente 1 -> Frente 2 | `MockContextEngine` |
| `TutorEngine` | Frente 2 -> Frente 3 | `MockTutorEngine` |
| `ModelProvider` | Frente 2 -> LLM | `MockProvider` |
| `ProfileStore` | cualquiera -> disco | `Storage` |

Con traits en las fronteras, cada frente puede desarrollarse y probarse contra
un doble sin esperar a nadie, y la integracion consiste en reemplazar el mock
por la implementacion real.

## Decisiones y sus porques

### Por que Rust

SeniorCLI tiene que ejecutarse como herramienta nativa de terminal, lanzar
procesos, leer archivos y distribuirse como un binario unico en tres sistemas
operativos. Rust da rendimiento, seguridad de memoria y un ecosistema solido
para CLIs asincronas. Usar Rust no implica que el modelo corra en la maquina del
alumno: la inferencia puede ser remota.

### Por que un solo crate y no un workspace

La guia define modulos, no paquetes. Un solo crate mantiene la compilacion
rapida y los refactors de contratos baratos. Si algun frente crece lo suficiente
como para tener su propio ciclo de release, ahi se separa; antes, no.

### Por que la logica vive en `lib.rs` y no en `main.rs`

`main.rs` solo arranca: parsea argumentos, configura logs y traduce el error
final. Todo lo demas esta en la biblioteca, que es lo que los tests pueden
importar. Un binario no se puede testear; una biblioteca si.

### Por que `TutorDecision` es una estructura y no texto libre

Si el Frente 2 devolviera un `String`, el Frente 3 no podria saber que tipo de
ayuda esta mostrando, ni la politica podria verificar nada. El tipo de
intervencion tiene que ser un dato, no una interpretacion de la prosa.

### Por que el nivel de ayuda se calcula fuera del prompt

Los modelos desobedecen instrucciones. Si el techo de ayuda viviera solo en el
prompt, una pista podria terminar siendo la solucion completa sin que nadie se
entere.

En `learning::policy` el nivel se calcula en codigo deterministico, se le
comunica al modelo **y ademas se verifica contra la salida**. Si la respuesta se
paso, se recorta (`TeachingPolicy::enforce`) y se registra como sobre-ayuda.

### Por que una peticion vaga no llega al modelo

Cuando falta intencion o evidencia, `LearningEngine` responde `Clarify` sin
llamar a la API. Gastar tokens para que el modelo nos devuelva una pregunta que
ya sabemos hacer no tiene sentido, y ademas evita que el modelo adivine el
objetivo del alumno.

### Por que el contexto del proyecto va en el mensaje de usuario y delimitado

Un README o un comentario del repositorio puede contener "ignora las
instrucciones anteriores". El contenido del proyecto es **material inerte**: va
en el mensaje de usuario, dentro de `<contexto_del_proyecto>`, y el system
prompt dice explicitamente que ahi no hay ordenes.

### Por que el runner tiene lista blanca y no acepta cadenas de comando

`runtime::runner` no usa `sh -c` ni `cmd /c`. Solo lanza programas de una lista
blanca, con subcomandos validados y timeout. Como no hay shell, un argumento con
`;` o `&&` es texto literal, no una inyeccion.

Esto es lo que hace seguro que el Frente 2 pueda pedir diagnosticos.

### Por que el escaner de secretos no confia en la extension

Un token puede estar en un `.md`, en un comentario o en un archivo sin
extension. `runtime::files` filtra por nombre, por extension **y** por contenido,
y ante la duda excluye el archivo. Un falso positivo cuesta un poco de contexto;
un falso negativo filtra una credencial.

### Por que el mastery no sube con una sola respuesta

Un perfil inflado hace que el harness ayude de menos y frustre al alumno. En
`learning::mastery` los pasos positivos son pequenos, los negativos mas grandes,
y un tema no cuenta como confiable hasta tener varias evidencias. El nivel
global solo se mueve con varios temas confiables.

### Por que las escrituras son atomicas y el perfil tiene respaldo

Un Ctrl+C a media escritura no debe dejar un JSON partido ni borrar el progreso
del alumno. `app::storage` escribe a un temporal y renombra, guarda un `.bak` del
ultimo perfil valido, y se **niega** a guardar un perfil que no valida.

Si el archivo activo esta corrupto, se restaura el respaldo. Si tampoco sirve,
se avisa en vez de empezar de cero en silencio.

### Por que las sesiones son JSONL y se escriben turno a turno

Nada se guarda al final. Cada turno se escribe apenas ocurre, una linea por
turno. Si la CLI muere, lo anterior sigue ahi, y una linea corrupta solo cuesta
un turno, no la sesion entera.

### Por que la salida es ASCII y no depende del color

SeniorCLI corre en la terminal de Windows, en un pipe de CI y en emuladores
raros. Ningun dato critico puede depender de un color o de un glifo: el tipo de
intervencion y la confianza van en texto.

### Por que `DeepSeekProvider` existe aunque no haga HTTP todavia

Tener la estructura puesta (endpoint, cuerpo de la peticion, parseo de la
respuesta, todo ya probado) hace que implementar el transporte sea rellenar un
metodo. Ni el motor ni la CLI cambian cuando se conecte de verdad.

## Lo que deliberadamente no hicimos todavia

| No esta | Por que |
|---|---|
| Indexacion semantica / AST | El flujo central tiene que estar estable antes. |
| Multi-model routing | Un proveedor bien hecho vale mas que tres a medias. |
| SQLite | JSON alcanza hasta que el historial lo justifique. |
| Dashboard web | El producto es la terminal. |
| Mas de dos stacks | Ampliar antes de que el caso vertical funcione acumula deuda. |

## Registrar decisiones nuevas

Si tomas una decision que alguien podria querer revertir en tres meses,
agregala aqui con su motivo. El formato es el de este archivo: un titulo que
empiece con "Por que" y la razon, no la implementacion.
