# How to Run a Series

Very often when running a service you may want to feed its output directly into
another service in order to emulate a more complex service or a multi-stage action.
For one-off requests you can chain services together by building a [`Series`][Series].

> [!TIP]
> If you need to assemble services into a more complex structure than a linear
> sequence (e.g. parallel threads, conditional branching, loops), you can
> build a [workflow](./introduction-to-workflows.md) instead.
>
> The advantage of using a `Series` over a workflow is that you can run a series
> once and forget about it (cleanup is automatic), whereas when you build a workflow
> you will spawn a [`Service`](https://docs.rs/crossflow/latest/crossflow/service/struct.Service.html)
> that needs to be managed afterwards.

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

The following example shows a simple series that feeds the output of one service
into another service with a `.map_block` between them to transform the first
service's output into a data type that can be consumed by the second service.

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:simple_series}}
```

## Dependencies and Detachment

You may notice the [`.detach()`][detach] at the end of the previous example series.
Ordinarily a series will automatically stop executing if its result is no longer
needed, which we detect based on whether the [`Promise`][Promise] of the final
response gets dropped. This allows us to avoid running services needlessly. However
sometimes you want services to run even if you won't be observing its final result,
because you are interested in side-effects from the service rather than the final
response of the service. You can insert `.detach()` anywhere in a series to ensure
that everything before the `.detach()` gets run even if the part of the series after
the `.detach()` gets dropped.

There are several ways to terminate a series, and each has drop conditions that
affect what happens to the series when it gets dropped. You can find a table of
the different terminating operations and their drop conditions in the
[`Series::detach()`][detach] documentation.

[Series]: https://docs.rs/crossflow/latest/crossflow/series/struct.Series.html
[Promise]: https://docs.rs/crossflow/latest/crossflow/promise/struct.Promise.html
[detach]: https://docs.rs/crossflow/latest/crossflow/series/struct.Series.html#method.detach
