# Creating a Node

Most of the useful work that happens inside a workflow is done by [nodes][Node].
A node can be implemented by any [provider](./provider_overview.md).
Providers will often be services---including [blocking](./spawn_a_service.md#spawn-a-blocking-service), [async](./spawn_async_service.md), and [continuous](./spawn_continuous_service.md) services---but could also be [callbacks](./callbacks.md) or [maps](./maps.md).

### From a Service

Suppose we have a sum function defined to be a blocking service:

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:sum_fn}}
```

We can spawn this as a service and then use it to create a node inside a workflow:

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:sum_service_workflow}}
```

A few things to note:
* We spawn the service outside of the workflow and then use the [`move`][move] keyword to move it into the closure that builds the workflow.
* We use [Builder::create_node][create_node] to create a workflow node that will run the `sum` service.
* After the node is created, we can access its input through `node.input` and its output through `node.output`.
* `builder.connect(ouput, input)` will connect an `Output` to an `InputSlot`
  * Somewhat counter-intuitively we connect `scope.input` to `node.input` because `scope.input` is actually an `Output`. This means whatever is input to the workflow will be sent directly to the `sum` service.
  * To pass back the sum as the output of the workflow, we connect `node.output` to `scope.terminate`.
* We don't need to explicitly specify the `Request` and `Response` types of the workflow because the compiler can infer those from the two `builder.connect(_, _)` calls.

[move]: https://doc.rust-lang.org/std/keyword.move.html
[create_node]: https://docs.rs/crossflow/latest/crossflow/builder/struct.Builder.html#method.create_node

### From a Callback

Using a [callback](./callbacks.md) instead of a service looks much the same.
The only difference is that callbacks don't need to be spawned:

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:sum_callback_workflow}}
```

The fact that it was implemented as a callback instead of as a service makes no difference to the workflow builder.
It still gets created as a [`Node`][Node] with `InputSlot` and `Output` types that match the Request and Response types, respectively, of the callback.

### From a Map

[Node]: https://docs.rs/crossflow/latest/crossflow/node/struct.Node.html

### Multiple Nodes

