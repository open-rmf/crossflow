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

#### Spawning a service inside workflow building closure

Recommended practice is to spawn services outside of the workflow building closure and then move them into the closure.
This allows services to have greater reusability, as they could be copied into multiple different workflows.
However if you do want to spawn the service from inside the workflow building closure, that option is available:

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:sum_nested_service_workflow}}
```

### From a Callback

Using a [callback](./callbacks.md) instead of a service looks much the same.
The only difference is that callbacks don't need to be spawned:

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:sum_callback_workflow}}
```

The fact that it was implemented as a callback instead of as a service makes no difference to the workflow builder.
It still gets created as a [`Node`][Node] with `InputSlot` and `Output` types that match the Request and Response types, respectively, of the callback.

### From a Map

[Maps](./maps.md) are an extremely common element in workflows.
Their ability to perform quick data transformations makes them invaluable for bridging the gap between different services.
They are common enough that crossflow provides two special APIs to make them easier to write:

#### `create_map_block`

A blocking map is a short-lived [closure] that performs a quick calculation or data transformation.
These are very useful for adapting message types in-between services that are chained together.

To create a blocking map you can simply pass a closure into the `Builder::create_map_block` method:

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:sum_map_workflow}}
```

You might notice that maps are often defined within the workflow building closure itself even though Services and Callbacks are usually created outside.
This isn't a requirement, but it is usually the most ergonomic way to add a map.
Reusability is less of a concern with maps than it is for Services or Callbacks.

#### `create_map_async`

An async map is simply used to run a basic async function.
Suppose you have an async function like:

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:get_page_title}}
```

You can use this async function as a map by simply passing its name into `Builder::create_map_async`:

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:async_map_workflow}}
```

Alternatively you can define the async map inline as a closure.
Just take note that Rust [does not currently support async closures][async-closures], so you will need your closure to return an `async move { ... }` block, like this:

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:async_map_nested_workflow}}
```

#### Chain

Creating a map through the [`Builder`][Builder] API is necessary if you need to [connect multiple outputs into the same map node](./building-a-cycle.md), but in most cases you'll want to create a map that just transforms data as it passes from one operation to another.
A more convenient way to do that is with a [Chain](./building-a-chain.md), discussed later.

[move]: https://doc.rust-lang.org/std/keyword.move.html
[create_node]: https://docs.rs/crossflow/latest/crossflow/builder/struct.Builder.html#method.create_node
[Node]: https://docs.rs/crossflow/latest/crossflow/node/struct.Node.html
[closure]: https://doc.rust-lang.org/book/ch13-01-closures.html
[async-closures]: https://github.com/rust-lang/rust/issues/62290
[Builder]: https://docs.rs/crossflow/latest/crossflow/builder/struct.Builder.html
