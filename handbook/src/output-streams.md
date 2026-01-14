# Output Streams

So far we've seen services that return a single output as a final response to each request it receives.
This is enough for chaining together sequential atomic services, but some actions are carried out over a period of time and involve stages or incremental progress that you may want to track.

Crossflow services have the option of using **output streams** to transmit messages *while processing a request*.
Before the final response is sent out, a service can stream out any number of messages over any number of output streams.
Each stream can have its own message type.

Streams can be anonymous---having no name, identified simply by what its output type is---but best practice is to use named streams.
Giving names to streams adds clarity to the purpose or meaning of each stream.

> [!TIP]
> If you want to know how to receive data from output streams, see [Receiving from Output Streams](./receiving-from-output-streams.md).
>
> To see how to connect output streams inside a workflow, see [Connecting Output Streams](./connecting-output-streams.md).

### Anonymous Streams

While it's not the recommended approach, we'll start by explaining how anonymous streams work.
If you are confident that your service will only ever produce one stream or that the type information is enough to make the purpose of the stream clear, then anonymous streams may be acceptable for your use case.

In this example we implement a Fibonacci sequence service that streams out the values of the sequence and then ultimately just returns a trigger `()` to indicate that the service is finished running:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:fibonacci_example}}
```

We use `StreamOf<u32>` to define an anonymous (unnamed) stream of [`u32`].
This stream allows us to emit a stream of outputs where each message is a new item in the Fiboanacci sequence, as opposed to the conventional "final response" that would require us to return a single message of `Vec<u32>`.
While a Fibonacci sequence is a trivial use case, this same approach can be used to transform any single input into a stream of any number of outputs.

##### Multiple Anonymous Streams

In the previous example you might have noticed

```rust,no_run,noplayground
let stream = input.streams;
```

The plural `input.streams` was renamed to a singular `stream` variable.
This is because in general services expect that multiple streams need to be supported.
In the case of `StreamOf<T>` the potentially multiple streams get reduced to one stream.

To get multiple anonymous streams, you can use a tuple.
Here's a slightly tweaked version of the previous example that additionally streams out a stringified version of the sequence:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:fibonacci_string_example}}
```

Now `input.streams` contains two streams, packed together as a tuple.
Since they are packed into a tuple, they continue to be anonymous (unnamed), but they can be accessed separately and send different outputs.

> [!WARNING]
> When you use multiple anonymous output streams, you must not include the same message type multiple times, e.g. `(StreamOf<u32>, StreamOf<u32>)`.
> This cannot be caught at compile time and will lead to confusing behavior, because when this service is spawned it will appear to have multiple output streams, but only one of the `Output`s will receive all of the messages, even if the service sends messages to both.

### Stream Pack

While anonymous streams are an option, it's best to always use named streams, especially if you are creating services for a [diagram](./json-diagrams.md).

To create named streams, you should define a [`struct`] where each field represents a different output stream.
The data type of each field will be the message type of its associated stream.
Simply apply `#[derive(StreamPack)]` to that struct, and now you can use those streams in your system:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:fibonacci_stream_pack_example}}
```

Now `input.streams` contains a separate named field for each of the streams in the stream pack.
Each of those fields lets you send output messages for each stream.
The compiler will ensure that the messages you send are compatible with the stream's type.

> [!TIP]
> Unlike with anonymous streams, you can have multiple named streams with the same message type.

### Async Streams

The previous examples show how to use streams in a blocking service to generate a stream of data from a single input.
While that is a valid use of streams, there is another conceptually important use: streaming updates or products out of an ongoing live service.
The streams that come out of a blocking service will be delivered at the same time at the final response due to the nature of blocking services, so live updates are only relevant for async and continuous services.

Below is a stub of a navigation service.
It takes in a `NavigationRequest` that includes a destination and a [`BufferKey`] for the robot's position.
While the robot makes progress towards its destination, the service will emit `NavigationStreams` including location data, log data, and any errors that come up along the way.

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:async_streams_example}}
```

A few important details of the above example:
* We use the `fn _(AsyncServiceInput) -> impl Future<Output = _>` syntax so we can have an async service that accesses the `NavigationGraph` resource at startup.
* We use the `async move { ... }` syntax to create the long-running Future that will run in the AsyncComputeTaskPool.
* All relevant input data is unpacked and then moved into the async block.
* We create a [callback](./callbacks.md) to periodically fetch data from the position buffer while our async block is running using the [async channel]. This is the most effective way for async tasks to access Bevy ECS data.
* The streams provided to an async service can be moved into an async block, allowing messages to be streamed out while the async task is still being processed.

### Continuous Streams

The output streams of blocking and async services both have a similar ergonomics, even if their behavior has some notable differences.
For continuous services, output streams allow you to stream out messages while a request is still in flight, similar to async services.

However the API has an important difference: a continuous service can see all its active requests (orders) at once, and its streams are isolated per order:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:continuous_streams_example}}
```

In the above example you can see that the output streams are accessed through `order.streams()` instead of `input.streams`.
Each order comes from a different request message passed into the continuous service---potentially from many different workflows or sessions at once.
Forcing you to send streams through the [`OrderMut`] API ensures that the messages you stream out are only going to the specific order that they're meant for.

[`u32`]: https://doc.rust-lang.org/std/primitive.u32.html
[`struct`]: https://doc.rust-lang.org/book/ch05-01-defining-structs.html
[`BufferKey`]: https://docs.rs/crossflow/latest/crossflow/buffer/struct.BufferKey.html
[async channel]: https://docs.rs/crossflow/latest/crossflow/channel/struct.Channel.html
[`OrderMut`]: https://docs.rs/crossflow/latest/crossflow/service/struct.OrderMut.html
