# crossflow_python

Experimental Python bindings for `crossflow`.

This crate exposes a small PyO3 module with an `Executor` class that can:

- inspect registry metadata
- run diagrams using JSON-compatible Python values
- register synchronous Python callback nodes with JSON input/output

## Requirements

- Python 3.9+
- Rust and Cargo
- maturin

## Quickstart

From the repository root:

```bash
python -m venv .venv-crossflow-python
source .venv-crossflow-python/bin/activate
python -m pip install --upgrade pip maturin
maturin develop --manifest-path crossflow-python/Cargo.toml
python crossflow-python/examples/basic_usage.py
```

`maturin develop` builds the Rust extension and installs `crossflow_python` into the active virtual environment.

## Example

See `[examples/basic_usage.py](examples/basic_usage.py)` for a runnable version.

```bash
python crossflow-python/examples/basic_usage.py
```

## Notes

- `Executor.run(...)` accepts either a Python dictionary/list structure or a JSON string for the diagram, as long as it matches Crossflow's diagram schema.
- Requests, configs, and responses must be JSON-compatible Python values.

