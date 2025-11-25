# Crossflow Handbook

This directory contains the source code for the official Crossflow Handbook.
The handbook provides a step-by-step introduction into the key concepts and
usage of Crossflow.

## Building

This uses [mdbook](https://rust-lang.github.io/mdBook/) to generate the document.
To install the dependencies run

```shell
cargo install mdbook
```

Then to build and view the handbook run this command from inside this directory:

```shell
mdbook serve --open
```

The document should open in your default web browser. When you make changes to
the source code, you can refresh the web page to see your updates take effect.
