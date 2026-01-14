# Connecting Output Streams

Up until now all the examples of spawning a workflow have used [`spawn_io_workflow`][spawn_io_workflow] which spawns a service that takes in a single request and yields a single response with no [output streams](./output-streams.md).
Just as services support output streams, so do workflows.
To stream messages out of your workflow while it is still running, you can use the [stream out](./scope-stream-out.md) operation.

For starters, to stream out of a workflow you need to use the more general [`spawn_workflow`][spawn_workflow] method.
Unlike the `Request` and `Response` generic parameters, it is ***never*** possible for the Rust compiler to infer what type you want for the `Streams` generic parameter.
This is why the `spawn_io_workflow` exists: In cases where you don't need to stream out of your workflow, all the generic parameters can usually be inferred.

The easiest way to specify the streams is to do it in the `Scope`.
The `Request` and `Response` parameters can still be inferred by putting a placeholder `_` in for them:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:minimal_workflow_stream_example}}
```

### Single Stream

If you workflow has a single output stream, you can use the [`StreamOf<T>`][StreamOf] struct to have a stream that produces messages of type `T`.

Here is how the [stream out](./scope-stream-out.md) conceptual example of slicing apples would be written with the native Rust API:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:apple_stream_out}}
```

The workflow has a single stream output that produces `AppleSlice` objects.
You can also see how streams produced by the `deposit_apples` and `chop_apple` services are connected to other operations.
The `Node` struct has a [`streams`][Node::streams] field which, for these services, is a single [`Output`][Output], because the services are using [`StreamOf`][StreamOf].

### Stream Pack

If your workflow needs multiple stream outputs, you can use a custom [`StreamPack`][StreamPack].
Here's an example of a stream pack that might be provided by a navigation workflow:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:navigation_streams}}
```

With that defined, we can emit multiple output streams from our workflow.
Here is an example of a robot navigating through a doorway and streaming out information while it goes along:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:navigation_streams_workflow}}
```

> [!TIP]
> In the above example we are connecting node stream outputs to the scope stream input slots, but this is an illustration, not a limitation.
> The fields in `Node::streams` are all regular [Outputs][Output] that can be connected into any [`InputSlot`][InputSlot], and the fields in `Scope::streams` are regular [InputSlots][InputSlot] that can receive messages from any [`Output`][Output].


[spawn_io_workflow]: https://docs.rs/crossflow/latest/crossflow/workflow/trait.SpawnWorkflowExt.html#method.spawn_io_workflow
[spawn_workflow]: https://docs.rs/crossflow/latest/crossflow/workflow/trait.SpawnWorkflowExt.html#tymethod.spawn_workflow
[StreamOf]: https://docs.rs/crossflow/latest/crossflow/stream/struct.StreamOf.html
[Node::streams]: https://docs.rs/crossflow/latest/crossflow/node/struct.Node.html#structfield.streams
[Output]: https://docs.rs/crossflow/latest/crossflow/node/struct.Output.html
[StreamPack]: https://docs.rs/crossflow/latest/crossflow/stream/trait.StreamPack.html
[InputSlot]: https://docs.rs/crossflow/latest/crossflow/node/struct.InputSlot.html
