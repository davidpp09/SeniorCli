# Frente 2 - AI & Learning Harness

**2 personas + 1 responsable tecnico.**
**Entrega principal:** una `TutorDecision` pedagogica.

Controlan al LLM: que contexto recibe, que puede responder y cuanta ayuda puede
dar. Este frente es la razon de que SeniorCLI no sea un chat con una API.

## Su codigo

```text
src/learning/
  mod.rs          build_engine: arma el motor segun la config
  engine.rs       orquestador del turno                [funcionando]
  policy.rs       cuanta ayuda se permite y verificacion [funcionando]
  validation.rs   parseo del JSON del modelo           [funcionando]
  mastery.rs      como se mueve el dominio del alumno  [funcionando]
  provider.rs     el trait ModelProvider + MockProvider [funcionando]
  deepseek.rs     proveedor real                        [falta el HTTP]
```

Su unica salida publica es `TutorDecision`. Lo que **no** les toca: leer el
repositorio (eso es del Frente 1) ni imprimir nada (eso es del Frente 3).

## Que ya funciona

```bash
cargo test learning::
```

- `policy.rs` calcula el techo de ayuda segun evidencia, nivel y dominio, y
  **recorta** una respuesta que se paso (`enforce`), sustituyendo el mensaje
  para que la solucion literal no llegue al alumno.
- `validation.rs` extrae el JSON aunque venga con prosa alrededor, en un bloque
  ```` ```json ````, o con llaves dentro de las cadenas. Un JSON truncado se
  rechaza en vez de adivinarse.
- `engine.rs` resuelve las peticiones vagas **sin llamar al modelo**, arma los
  prompts separando instrucciones de datos, valida, verifica la politica y cae
  en un fallback seguro cuando algo falla. Ya emite `TurnMetrics`.
- `mastery.rs` mueve el dominio con evidencias acumuladas, nunca con una sola
  respuesta.
- `MockProvider` simula cuatro escenarios: respuesta buena, JSON roto,
  sobre-ayuda y caida del proveedor.

## Que les toca construir

### 1. Transporte HTTP de DeepSeek (`deepseek.rs::complete`) - primero esto

Es lo unico que bloquea al producto entero. Todo lo demas ya esta probado:
`endpoint()`, `build_body()` y `parse_body()` tienen tests.

Pasos, en el propio `TODO(frente-2)` del archivo:

1. Descomentar `reqwest` en `Cargo.toml` (seccion Frente 2).
2. POST a `self.endpoint()` con `Authorization: Bearer {api_key}`.
3. Medir la latencia real y ponerla en `ModelResponse`.
4. Reintentar hasta `MAX_REINTENTOS` **solo** ante 429/5xx/timeout, con espera
   creciente. Nunca ante un 4xx de validacion. Nunca un loop infinito.
5. Mapear fallos a `SeniorError::Provider` con un mensaje que el alumno entienda
   y **sin filtrar la API key**.

**Hecho cuando:** `SENIOR_PROVIDER=deepseek cargo run -- debug` produce una
intervencion valida contra la API real, y `cargo test` sigue pasando sin red.

> Usen `secrecy` (comentado en `Cargo.toml`) si quieren endurecer el manejo de
> la key. El `Debug` manual que ya evita imprimirla debe seguir ahi.

### 2. Afinar la politica pedagogica (`policy.rs`)

Lo que hay es una primera version razonable. Falta:

- Usar `intentos_antes_de_subir`: hoy se define pero no se aplica. Si el alumno
  pregunta tres veces sobre el mismo concepto, el techo deberia subir un paso.
- Pasarle el concepto real a `max_help_level`: hoy se llama con `None` desde
  `apply_to`, asi que el ajuste por dominio nunca entra.

**Hecho cuando:** un alumno que insiste en el mismo concepto recibe mas ayuda
que en su primer intento, y hay un test que lo demuestra.

### 3. Deteccion de ambiguedad mas fina (`TutorRequest::looks_vague`)

Hoy es una lista de frases y un largo minimo. Suficiente para arrancar, pobre
para produccion. Piensen en preguntas largas pero igual de vacias.

### 4. Reparacion de JSON truncado (`validation.rs`)

Ahora un JSON cortado a la mitad se rechaza. Una opcion es un segundo intento
pidiendole al modelo solo el JSON. Si lo hacen: **un intento, no un bucle**, y
que quede registrado en `TurnMetrics`.

### 5. Limite de rondas de herramientas

`SeniorError::ToolLoopLimit` ya existe pero nadie lo usa. En cuanto el modelo
pueda pedir acciones (correr tests, leer un archivo), hay que limitar rondas,
costo y herramientas por turno.

### 6. Exportar las metricas

`TurnMetrics` se calcula y se tira. Habria que persistirlo junto a la sesion
para poder responder: cuanto tarda, cuantos tokens cuesta, cuantas veces falla
el parseo, cuantas veces el modelo se pasa de ayuda.

Esto se coordina con el Frente 3, que es quien escribe en disco.

## Lo que NO necesitan todavia

Multi-model routing complejo, fine-tuning, modelo local.

## Riesgos que les tocan a ustedes

| Riesgo | Como lo evitamos |
|---|---|
| Alucinacion tecnica | Preferir evidencia del compilador. Bajar `confidence`, nunca convertir la seguridad del modelo en verdad. |
| Peticion vaga | `Clarify`, no adivinar. Ya esta resuelto sin llamar al modelo. |
| El harness se pierde | Reconstruir cada turno desde contexto + perfil + objetivo, no desde el historial. |
| Perdida del objetivo pedagogico | El techo de ayuda vive fuera del prompt y se verifica contra la salida. |
| JSON invalido | `validation.rs` + fallback seguro. |
| Loop de herramientas | Limitar rondas (pendiente, punto 5). |
| Prompt injection desde el repo | El contenido del proyecto va delimitado como datos en el mensaje de usuario. |
| Perfil equivocado | Varias evidencias, con degradacion. Ya esta en `mastery.rs`. |

## Prueba minima del frente

> Peticion vaga -> `Clarify`. Peticion clara -> `Hint` o pregunta socratica.
> Ambas con DeepSeek real.

Los dos casos ya estan cubiertos con `MockProvider` en `engine.rs`; falta
repetirlos contra la API.

## Como integrarse con los demas

No necesitan al Frente 1: usen `ProjectContext::fake_java_null_pointer()` y
`fake_ambiguous()`. No necesitan al Frente 3: el motor devuelve datos, no texto
de pantalla.
