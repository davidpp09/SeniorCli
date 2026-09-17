# Guias por frente

Cada archivo dice, para un frente: que ya funciona, que le toca construir (en
orden), que NO necesita todavia, sus riesgos y su prueba minima.

- [Frente 1 - Runtime & Context](frente-1-runtime.md) -> `src/runtime/`
- [Frente 2 - AI & Learning Harness](frente-2-learning.md) -> `src/learning/`
- [Frente 3 - CLI, Producto & Persistencia](frente-3-cli.md) -> `src/app/`

El documento fuente es
[SeniorCLI_Guia_3_Frentes_Rust_Actualizada.pdf](../SeniorCLI_Guia_3_Frentes_Rust_Actualizada.pdf).

## Checklist de arranque del equipo

- [x] Acordar los campos de `ProjectContext`, `StudentProfile` y `TutorDecision`
- [x] Crear el repo y la estructura de carpetas
- [x] Crear los mocks compartidos antes de los motores reales
- [x] Elegir el caso vertical inicial (debug de Java con NullPointerException)
- [x] Dejar DeepSeek detras de `ModelProvider`
- [x] Integrar el primer caso de punta a punta (`tests/vertical_slice.rs`)
- [ ] Asignar 2 personas por frente y un responsable tecnico en cada uno
- [ ] Reemplazar los marcadores de `.github/CODEOWNERS` por usuarios reales
- [ ] Configurar la proteccion de la rama `main` (ver CONTRIBUTING, seccion 2)
- [ ] Registrar los fallos del harness desde el primer dia (plantilla de issue)

## Como buscar su trabajo en el codigo

```bash
grep -rn "TODO(frente-1)" src/    # o frente-2, frente-3
```

Cada `TODO` dice que falta y por que, no solo que esta incompleto.
