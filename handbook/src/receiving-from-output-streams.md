# Receiving from Output Streams

Some services also have [output streams](./output-streams.md) that you may want
to receive data from. In that case you will need to take a [`Recipient`][Recipient]
instead of only taking the response:

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:take_recipient}}
```

The `parsing_service` provides this `ParsedStreams` stream output pack:

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:parsed_streams_struct}}
```

The string value `"3.14"` should be able to parse to `f32` but not `u32` or `i32`.
We can receive whichever values were produced with the following code:

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:receive_streams}}
```

The `streams` field of [`Recipient`][Recipient] will itself contain one field for
each field of the service's stream pack, and the names those fields will match
the field names in the stream pack.

Each field in `Recipient::streams` will be a
[receiver](https://docs.rs/tokio/latest/tokio/sync/mpsc/struct.UnboundedReceiver.html)
that can be used to receive values streamed out by the requested service, either in a
[sync](https://docs.rs/tokio/latest/tokio/sync/mpsc/struct.UnboundedReceiver.html#method.try_recv)
or [async](https://docs.rs/tokio/latest/tokio/sync/mpsc/struct.UnboundedReceiver.html#method.recv)
context.

In the snippet above, `.recv()` will provide a "Future" that can efficiently
wait for the next value from the stream receiver that it's called on. Calling
`.await` on that Future will let the async task rest until the future is available.
Calling `.recv().await` in a while-loop expression ensures that we keep draining
the streams until all messages have been received.

The `Recipient` also has a `response` field which the above snippet uses to
detect that the service is finished running. In general `Recipient::response`
will provide the final output message of the service, but for `parsing_service`
that is just a unit trigger `()`.

You are not required to await the response or the streams in any particular order.
You could await or poll the streams while the request is still being processed in
order to receive live updates from long-running requests. There are many tools
in the Rust ecosystem that allow for sophisticated async programming patterns,
such as tokio's [select](https://docs.rs/tokio/latest/tokio/macro.select.html#examples)
macro that allows you to await on multiple streams at once and immediately receive
the next message that emerges from any one of them:

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:receive_streams_parallel}}
```

[Recipient]: https://docs.rs/crossflow/latest/crossflow/series/struct.Recipient.html

## Collecting Streams

If you are not interested in managing stream channels and just want to store the
stream outputs for later use, you can use
[`Series::collect_streams`](https://docs.rs/crossflow/latest/crossflow/series/struct.Series.html#method.collect_streams)
to store all the stream outputs in an entity of your choosing:

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:collect_streams}}
```

Then you can schedule a system to query the [`Collection`](https://docs.rs/crossflow/latest/crossflow/series/struct.Collection.html)
component of that entity to inspect the streams. When using a stream pack with
named streams make sure to use [`NamedValue<T>`](https://docs.rs/crossflow/latest/crossflow/stream/struct.NamedValue.html)
for the inner type of the `Collection`:

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:query_stream_storage}}
```
