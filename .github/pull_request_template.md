## Que cambia

<!-- Una o dos frases. Que hace este PR y por que. -->

## Frente

- [ ] Frente 1 - Runtime & Context
- [ ] Frente 2 - AI & Learning Harness
- [ ] Frente 3 - CLI, Producto & Persistencia
- [ ] Contratos compartidos (`src/contracts/`)
- [ ] Infraestructura / CI

## Contratos

- [ ] **No** toco `src/contracts/`
- [ ] Toco `src/contracts/` y ya lo hable con los otros dos frentes

> Cambiar un contrato rompe a los demas. Si marcaste la segunda casilla,
> explica aqui el cambio y a quien afecta:

## Como lo probaste

<!-- Comandos, casos cubiertos, que no cubriste. -->

- [ ] `cargo test` pasa en local
- [ ] `cargo clippy --all-targets -- -D warnings` sin avisos
- [ ] `cargo fmt --all --check` limpio
- [ ] Agregue tests para lo nuevo (o explico abajo por que no aplica)

## Riesgos del harness

<!-- Marca lo que aplique y di como lo manejaste. -->

- [ ] Puede afectar el contexto que se envia al proveedor (privacidad / tamano)
- [ ] Puede afectar la politica pedagogica (cuanta ayuda se entrega)
- [ ] Puede afectar el estado en disco (perfil o sesiones)
- [ ] Ninguno de los anteriores

## Notas para quien revisa

<!-- Lo que te gustaria que miraran con lupa. -->
