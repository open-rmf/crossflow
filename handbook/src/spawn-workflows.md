# How to Build a Workflow

> [!NOTE]
> This chapter will be about building workflows using the **native Rust API**. That
> means writing Rust code to build the workflow, which will be compiled and embeded
> in an application. If you are instead interested in building workflows using
> **JSON diagrams**, then you can skip ahead to the [JSON Diagrams](./json-diagrams.md)
> chapter.

## Spawning

You can spawn a workflow anywhere that you can access Bevy [Commands][Commands] through the trait [`SpawnWorkflowExt`][SpawnWorkflowExt]. This can be done while setting up your application or during runtime.

This is an example of spawning an input/output (i/o) workflow---a workflow that doesn't have any output streams, just one input message and one final output message:

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:trivial_workflow}}
```

Notice some key details:
* The output of `spawn_io_workflow` is a [`Service`][Service]. This is what you will use to refer to the workflow after it has been spawned.
* The input argument of `spawn_io_workflow` is a [closure][closure].
* The input arguments of the closure are a [`Scope`][Scope] and a [`Builder`][Builder].
* The generic parameters of the [`Scope`][Scope] match those of the [`Service`][Service].
* The [`Scope`][Scope] has an [input](./scopes.md#start) and a [terminate](./scopes.md#terminate) field. These represent the input and output of the overall workflow, and therefore match the request and response type of the [`Service`][Service]. In this case `Request` and `Response` are actually aliases of the same type.
* The [`Builder`][Builder] can make a connection between an output and an input.

Very often the Rust compiler can infer the generic types of the service and scope, so the above example can usually be reduced to:

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:trivial_workflow_concise}}
```

The `spawn_io_workflow` command exists to make this inference easy and concise.
When streams are used this kind of inference will not work as easily.
This will be discussed more in the [Output Streams](./workflow-output-streams.md) section.
In the meantime, move on to the next page to see how to put nodes into your workflow.

[Commands]: https://docs.rs/bevy/latest/bevy/prelude/struct.Commands.html
[SpawnWorkflowExt]: https://docs.rs/crossflow/latest/crossflow/workflow/trait.SpawnWorkflowExt.html
[Service]: https://docs.rs/crossflow/latest/crossflow/service/struct.Service.html
[Scope]: https://docs.rs/crossflow/latest/crossflow/workflow/struct.Scope.html
[Builder]: https://docs.rs/crossflow/latest/crossflow/builder/struct.Builder.html
[closure]: https://doc.rust-lang.org/book/ch13-01-closures.html
