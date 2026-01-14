# Execution

Crossflow is first and foremost a library to facilitate the execution of workflows, so there is not a single canonical way for JSON diagrams to be executed.
However, there are certain requirements to meet and recommended code paths to follow for an application to be effective at executing diagrams.

Just like when workflows are [built the native Rust API](./spawn-workflows.md), a crossflow **JSON diagram executor** needs to be built as part of a Bevy [App].
You can either make a Bevy app that is entirely dedicated to executing JSON diagrams, or you can make the execution of JSON diagrams simply one feature within a broader app.
In order for your app to execute diagrams, you will need to create a [system] that can receive JSON diagrams, build the diagrams into executable workflows, and then run those workflows.

The most important piece to understand when implementing an executor app is the [`DiagramElementRegistry`][DiagramElementRegistry].
The registry stores "builders" that allow the operations (`"ops"`) within a JSON diagram to be translated into workflow elements that can actually be executed.

There are three types of registrations present in the registry:

* **Message registration** stores information about the message types supported by the executor, including what operations can be performed on the message type and how to perform it.
* **Node builder registration** stores the [Node Builders](./diagram-nodes.md) that allow `"type": "node"` operations in a diagram to be built into workflow nodes.
  This registration also stores the schema of the `"config"` field for each unique `"builder"` ID.
* **Section builder registration** is similar to the node builder registration, except it stores information on [Section Builders](./diagram-sections.md).

### Creating a registry

Initializing a registry is simple:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/diagram-snippets.rs:new_diagram_element_registry}}
```

This will create a new registry that only contains registrations for the the "builtin" message types:
* Rust primitive types
    * [`()`](https://doc.rust-lang.org/std/primitive.unit.html)
    * [`u8`](https://doc.rust-lang.org/std/primitive.u8.html)
    * [`u16`](https://doc.rust-lang.org/std/primitive.u16.html)
    * [`u32`](https://doc.rust-lang.org/std/primitive.u32.html)
    * [`u64`](https://doc.rust-lang.org/std/primitive.u64.html)
    * [`usize`](https://doc.rust-lang.org/std/primitive.usize.html)
    * [`i8`](https://doc.rust-lang.org/std/primitive.i8.html)
    * [`i16`](https://doc.rust-lang.org/std/primitive.i16.html)
    * [`i32`](https://doc.rust-lang.org/std/primitive.i32.html)
    * [`i64`](https://doc.rust-lang.org/std/primitive.i64.html)
    * [`isize`](https://doc.rust-lang.org/std/primitive.isize.html)
    * [`f32`](https://doc.rust-lang.org/std/primitive.f32.html)
    * [`f64`](https://doc.rust-lang.org/std/primitive.f64.html)
    * [`bool`](https://doc.rust-lang.org/std/primitive.bool.html)
    * [`char`](https://doc.rust-lang.org/std/primitive.char.html)
* [`String`](https://doc.rust-lang.org/std/string/struct.String.html)
* [`JsonMessage`](https://docs.rs/serde_json/latest/serde_json/value/enum.Value.html)

You'll notice that we declared `mut registry` as *mutable* in the above code snippet.
This is because the registry isn't very useful until you start registering your own node builders.
Without any node builders, your executor will only be able to build workflows that exclusively use the builtin message types listed above and the basic builtin operations.

Registering a node builder will allow you to build workflows that use custom services.
Use [`DiagramElementRegistry::register_node_builder`][DiagramElementRegistry::register_node_builder] as shown below to register a new node builder.

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/diagram-snippets.rs:minimal_add_example}}
```

Node builders are covered in more detail on the [next page](./diagram-nodes.md).

> [!TIP]
> When you register a node builder, you will also automatically register any input and output messages types needed by the node builder.

### Building workflows with a registry

Once you've registered all the builders that your executor needs, you can start building workflows from diagrams.
Simply create a valid [`Diagram`][Diagram] instance and then call [`Diagram::spawn_io_workflow`][Diagram::spawn_io_workflow]:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/diagram-snippets.rs:build_workflow_example}}
```

> [!NOTE]
>
> In the above example, `commands` is a [`Commands`][Commands] instance.
> Typically you will need to create a Bevy system that receives JSON diagrams and builds them into workflows.

### Executing built workflows

Once you've used the [Diagram] and [registry][DiagramElementRegistry] to build the workflow, you will be holding a [`Service`][Service] that you can execute.
From there you can follow the same guidance in [How to Run a Service](./run-services.md) or [How to Run a Series](./run-series.md).

> [!TIP]
> To build the [service][Service] and execute the workflow, your executor application will need to know the input and output message types of the diagram at compile time.
>
> In most cases you can't expect all incoming diagrams to have the exact same input and output message types as each other, so instead **you can use [`JsonMessage`][JsonMessage] as both the input and output message types (`Request` and `Response`) of all diagrams.**
>
> As long as the actual input and output message types of the diagrams are deserializable and serializable (respectively), the workflow builder can convert to/from [`JsonMessage`][JsonMessage] to run the workflow and receive its response.

## Premade Executor

If you would like to get started with executing crossflow diagrams with minimal effort, you can use the [`crossflow-diagram-editor`](https://github.com/open-rmf/crossflow/tree/main/diagram-editor) library to quickly make a basic executor.

The [calculator example](https://github.com/open-rmf/crossflow/tree/main/examples/diagram/calculator) shows how to use the `crossflow-diagram-editor` to create a workflow executor that provides simple calculator operations.
For your own custom executor, you can replace the calculator node builders with your own more useful node builders.

```rust,no_run,noplayground
{{#include ./examples/diagram/calculator/src/main.rs:calculator_example}}
```

[App]: https://docs.rs/bevy/latest/bevy/app/struct.App.html
[DiagramElementRegistry]: https://docs.rs/crossflow/latest/crossflow/diagram/struct.DiagramElementRegistry.html
[DiagramElementRegistry::register_node_builder]: https://docs.rs/crossflow/latest/crossflow/diagram/struct.DiagramElementRegistry.html#method.register_node_builder
[Diagram]: https://docs.rs/crossflow/latest/crossflow/diagram/struct.Diagram.html
[Diagram::spawn_io_workflow]: https://docs.rs/crossflow/latest/crossflow/diagram/struct.Diagram.html#method.spawn_io_workflow
[Service]: https://docs.rs/crossflow/latest/crossflow/service/struct.Service.html
[Commands]: https://docs.rs/bevy/latest/bevy/prelude/struct.Commands.html
[JsonMessage]: https://docs.rs/crossflow/latest/crossflow/buffer/enum.JsonMessage.html
[system]: https://bevy-cheatbook.github.io/programming/systems.html
