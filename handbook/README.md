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

## For Maintainers

To publish an updated version of the handbook, run

```bash
./scripts/publish.sh
```

This renders the book and then copies it over to the `gh-pages` branch of the [crossflow-handbook](https://github.com/open-rmf/crossflow-handbook) repo.
This will require you to have write permissions for that repo.
