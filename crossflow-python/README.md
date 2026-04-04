# crossflow_python

Experimental Python bindings for `crossflow`.

This crate exposes a small PyO3 module with an `Executor` class that can:

- inspect registry metadata
- run diagrams using JSON-compatible Python values
- register synchronous Python callback nodes with JSON input/output

For local development, build or install it with `maturin` from this directory.

