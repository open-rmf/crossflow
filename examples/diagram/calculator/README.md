# Calculator example

This is an example that shows how to build a `crossflow` workflow from a diagram
that expresses some calculator operations. This is not a practical use case of
workflows; it is only meant to be illustrative of how to use the tools.

For a quick run this example, open a terminal focused on this folder and run:

```bash
cargo run -- run diagrams/multiply_by_3.json 10
```

You should see `30.0` printed out by the program, because `multiply_by_3.json` is a
very simple workflow that just multiples your input by 3.

You can replace `10` with a different number or you can write a different workflow
diagram to perform a different set of operations on the input value.

### Running a Progress Visualization Workflow

To see diagram editor progress visualization clearly, use the delayed carry
workflow:

```bash
cargo run -- run diagrams/carry_object_progress.json '{"object":"box-1","from":"shelf","to":"station_a"}'
```

### Running a Python Scripting Workflow

For a more complex example demonstrating dynamic Python scripting support, run the following command:

```bash
cargo run -- run diagrams/python_script_nodes.json '[[0, 1, 2, 3, 4, 5], [10, 9, 8, 7, 6, 5]]'
```

This workflow launches two parallel dynamic Python script streams that output values to buffers; a final listener node accesses these buffers and terminates the execution when a matching value is found (which outputs `5` in this example).

## Diagram Editor

To use the diagram editor to create a new calculator workflow, run

```bash
cargo run -- serve
```

Then open http://localhost:3000 to run the diagram editor app from your web browser.
