# How to Run a Series

Very often when running a service you may want to feed its output directly into
another service in order to emulate a more complex service. For one-off requests
that can be done by building a [`Series`][Series].
If you need to assemble services with a more complex structure than a linear
sequence, you can [build a workflow instead](./build_a_workflow.md). The advantage
of using a `Series` over a workflow is that you can run a series once and forget
about it, whereas building a workflow spawns a
[`Service`](https://docs.rs/crossflow/latest/crossflow/service/struct.Service.html)
that needs to be managed afterwards.

Every time you run `.request(_, _)`, you immediately receive a [`Series`][Series].
When you use `.request(_, _).take_response()` you are actually calling
[`Series::take_response`](https://docs.rs/crossflow/latest/crossflow/series/struct.Series.html#method.take_response)
which terminates the construction of the series after a single service call.

To build a more complex series you can use the chaining methods provided by the
[`Series`][Series] struct:

* [`.then(_)`](https://docs.rs/crossflow/latest/crossflow/series/struct.Series.html#method.then):
  Specify a service (or other kind of provider) to pass the last response into,
  getting back a new response.
* [`.map_block(_)`](https://docs.rs/crossflow/latest/crossflow/series/struct.Series.html#method.map_block):
  Specify a `FnOnce(T) -> U` to transform the last response into a new value.
  The function you provide will block the system schedule from running, so it
  should be short-lived.
* [`.map_async(_)`](https://docs.rs/crossflow/latest/crossflow/series/struct.Series.html#method.map_async):
  Specify a `FnOnce(T) -> impl Future<Output=U>` to transform the last response
  into a new value. The Future will be evaluated in the async task pool, so put
  all long-running routines into the async block.

The following example shows a simple series that sequences two services with a
`.map_block` between them to transform the first service's output into a data type
that can be consumed by the second service.

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:simple_series}}
```



[Series]: https://docs.rs/crossflow/latest/crossflow/series/struct.Series.html
