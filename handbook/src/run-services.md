# How to Run a Service

Once you have [spawned a service](./spawn-services.md) or have some other type
of "provider" available [[1](./callbacks.md)][[2](./maps.md)][[3](./spawn-workflows.md)],
you can run it by passing in a request. This can be done from inside of any Bevy
system by including a [`Commands`](https://docs.rs/bevy/latest/bevy/prelude/struct.Commands.html):

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:request_service_example}}
```

The `.request(_, _)` method comes from the [`RequestExt`](https://docs.rs/crossflow/latest/crossflow/request/trait.RequestExt.html#tymethod.request) trait provided by crossflow. This method takes in a `request_msg` (the input message for the service) and any type of "provider", which is usually a [`Service`](https://docs.rs/crossflow/latest/crossflow/service/struct.Service.html).

The simplest thing to do with a request is to take the response using `.take_response()`.
This will provide you with a [`Promise`][Promise]
which you can use to receive the response of the service once it finishes.

### Sync Promise

You can use a [`Promise`][Promise] in a sync (blocking, non-async)
function using [`peek`](https://docs.rs/crossflow/latest/crossflow/promise/struct.Promise.html#method.peek):

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:peek_promise}}
```

Peeking a promise will check if there are any updates for the promise and then
provide a borrow of the [inner state of the promise][PromiseState] which lets
you examine whether the final result is available, cancelled, or anything else.

> [!WARNING]
> Using promises in sync code has a crucial disadvantage that you need to
> repeatedly poll the promise to know when it has finished. In most cases this
> is inefficient busywork.
>
> **You are recommended to await promises in async code instead.**

### Async Promise

The most efficient and ergonomic way to use a Promise is to `.await` it in an
async function. Awaiting the `Promise` will consume it and return its final
[`PromiseState`][PromiseState] as soon as that final state is available:

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:await_promise}}
```

The [`PromiseState`][PromiseState] is an enum that can take on a number of
variants depending on what happened while the request was being processed. The
code snippet provides some description of each variant. Issue
[#17](https://github.com/open-rmf/crossflow/issues/17) is tracking the question
of whether this can be simplified.

[Promise]: https://docs.rs/crossflow/latest/crossflow/promise/struct.Promise.html
[PromiseState]: https://docs.rs/crossflow/latest/crossflow/promise/enum.PromiseState.html

## More Ways to Manage Requests

There are often times where you'll want to immediately feed the result of one
service into another in a chain of service calls. We call this a `Series`, and
you can continue to the next page to find out how to do this.

Some services have [output streams](./output-streams.md) in addition to a response,
and you may need to receive data from those. You can learn about how to receive
from output streams in [Receiving from Output Streams](./receiving-from-output-streams.md).

If simply receiving the final response of a service is enough for your needs,
then you can move along to the [Introduction to Workflows](./introduction-to-workflows.md) section.
