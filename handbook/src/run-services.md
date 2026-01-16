# How to Run a Service

Once you have [spawned a service](./spawn-services.md) or have some other type
of "provider" available [[1](./callbacks.md)][[2](./maps.md)][[3](./spawn-workflows.md)],
you can run it by passing in a request. This can be done from inside of any Bevy
system by including a [`Commands`](https://docs.rs/bevy/latest/bevy/prelude/struct.Commands.html):

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:request_service_example}}
```

The `.request(_, _)` method comes from the [`RequestExt`](https://docs.rs/crossflow/latest/crossflow/request/trait.RequestExt.html#tymethod.request) trait provided by crossflow. This method takes in a `request_msg` (the input message for the service) and any type of "provider", which is usually a [`Service`](https://docs.rs/crossflow/latest/crossflow/service/struct.Service.html).

The simplest thing to do with a request is to take the outcome using `.outcome()`.
This will provide you with an [`Outcome`][Outcome] which you can use to receive the response of the service once it finishes.

### Sync Outcome

You can use an [`Outcome`][Outcome] in a sync (blocking, non-async) function using [`try_recv`](https://docs.rs/crossflow/latest/crossflow/series/struct.Outcome.html#method.try_recv):

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:try_recv}}
```

> [!WARNING]
> Using outcomes in sync code has a crucial disadvantage that you need to
> repeatedly poll the outcome to know when it has finished. In most cases this
> is inefficient busywork.
>
> **You are recommended to await outcomes in async code instead.**

### Async Outcome

The most efficient and ergonomic way to use an `Outcome` is to `.await` it in an async function. Awaiting the `Outcome` will consume it and return its final result as soon as that final result is available:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:await_outcome}}
```

The result will either be the final response from the service or an error explaining why the request was cancelled.

## More Ways to Manage Requests

There are often times where you'll want to immediately feed the result of one
service into another in a chain of service calls. We call this a `Series`, and
you can continue to the next page to find out how to do this.

Some services have [output streams](./output-streams.md) in addition to a response,
and you may need to receive data from those. You can learn about how to receive
from output streams in [Receiving from Output Streams](./receiving-from-output-streams.md).

If simply receiving the final response of a service is enough for your needs,
then you can move along to the [Introduction to Workflows](./introduction-to-workflows.md) section.

[Outcome]: https://docs.rs/crossflow/latest/crossflow/series/struct.Outcome.html
