# How to Build a Workflow

> [!NOTE]
> This chapter will be about building workflows using the **native Rust API**. That
> means writing Rust code to build the workflow, which will be compiled and embeded
> in an application. If you are instead interested in building workflows using
> **JSON diagrams**, then you can skip ahead to the [JSON Diagrams](./json_diagrams.md)
> chapter.

## Spawning

You can spawn a workflow anywhere that you can access Bevy [Commands][Commands] through the trait [`SpawnWorkflowExt`][SpawnWorkflowExt]. This can be done while setting up your application or during runtime.

This is an example of spawning an input/output (i/o) workflow, which simply means it's a workflow that doesn't have any additional output streams:

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:trivial_workflow}}
```

Notice a few key details:
* The output of `spawn_io_workflow` is a [`Service`][Service].
* The input argument of `spawn_io_workflow` is a function or closure.
* The input arguments of the closure are a [`Scope`][Scope] and a [`Builder`][Builder].
* The generic parameters of the [`Scope`][Scope] match those of the [`Service`][Service].
* The [`Scope`][Scope] has an [input](./scopes.md#start) and a [terminate](./scopes.md#terminate). These represent the input and output of the overall workflow, and therefore match the request and response type of the [`Service`][Service].
* The [`Builder`][Builder] can make a connection between an output and an input.

[Commands]: https://docs.rs/bevy/latest/bevy/prelude/struct.Commands.html
[SpawnWorkflowExt]: https://docs.rs/crossflow/latest/crossflow/workflow/trait.SpawnWorkflowExt.html
[Service]: https://docs.rs/crossflow/latest/crossflow/service/struct.Service.html
[Scope]: https://docs.rs/crossflow/latest/crossflow/workflow/struct.Scope.html
[Builder]: https://docs.rs/crossflow/latest/crossflow/builder/struct.Builder.html
