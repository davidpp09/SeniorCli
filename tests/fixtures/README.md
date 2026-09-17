# Fixtures

Salidas reales de compiladores y runners, para probar los parsers del Frente 1
sin depender de tener maven, gradle o python instalados en el CI.

Convencion de nombres: `<herramienta>-<caso>.txt`

```text
cargo-error-e0308.txt
maven-nullpointer.txt
pytest-typeerror.txt
```

Peguen la salida **tal cual**, sin limpiarla: los espacios y el ruido son parte
de lo que el parser tiene que aguantar.
