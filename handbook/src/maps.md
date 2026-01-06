# Maps

[Services](./spawn-services.md) and [callbacks](./callbacks.md) are two kinds of [providers](./provider-overview.md) that can both function as [Bevy systems].
Bevy systems have the benefit of granting you full access to the Bevy [ECS], but there is a small amount of overhead involved in initializing and running systems.
If you want a provider that **does not need access to Bevy's ECS**, then you should consider using a **map** instead.

**Maps** are [providers](./provider-overview.md) that have the minimum possible overhead.
They are defined with as functions that don't have any [Bevy system parameters].
Maps are good for doing quick transformations of data or for calling non-Bevy async functions.

> [!WARNING]
> Since blocking maps do not have any system parameters, they cannot access data inside of buffers.
>
> Async maps can use the [async channel] to access buffer data, but each query needs to wait until the next execution flush takes place.

### Simple usage

When building a [series of services](./run-series.md) you can use [`Series::map_block(_)`] or [`Series::map_async(_)`] to quickly and easily create a map.

Similarly when [chaining service in a workflow](./building-a-chain.md#simple-sequence) you can use [`Chain::map_block(_)`] or [`Chain::map_async(_)`].

These functions allow you to convert simple blocking or async functions (or closures) into providers with no additional steps.

### Streams and Async Channel

Even though maps cannot directly access any Bevy system params, they can still have output streams.
Async maps even get access to the [async channel] which means they can query for and modify Bevy ECS data while running in the async task pool.

To get access to these, you must define your function explicitly as a blocking or async map:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:fibonacci_map_example}}
```

Before passing your function into `commands.request(_)` you must apply `.as_map()` to it to convert it into a map.

> [!TIP]
> If your map is part of a series or a chain, you can use [`Series::map(_)`] or [`Chain::map(_)`] instead to avoid the need to apply `.as_map()`.
> This is useful if you want to define your map as a closure inline in the series or chain.

Blocking maps do not get to access the [async channel]---neither do blocking services nor blocking callbacks---but async maps do.
Just like accessing streams, you just need to make it explicit that you have an async map:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:navigate_map_example}}
```

With that, `navigate` can be passed into `commands.request(_, navigate.as_map())` or into `series.map(navigate)` or `chain.map(navigate)`.

[Bevy systems]: https://bevy-cheatbook.github.io/programming/systems.html
[ECS]: https://en.wikipedia.org/wiki/Entity_component_system
[Bevy system parameters]: https://docs.rs/bevy/latest/bevy/ecs/system/trait.SystemParam.html#derive
[async channel]: https://docs.rs/crossflow/latest/crossflow/channel/struct.Channel.html
[`Series::map(_)`]: https://docs.rs/crossflow/latest/crossflow/series/struct.Series.html#method.map
[`Series::map_block(_)`]: https://docs.rs/crossflow/latest/crossflow/series/struct.Series.html#method.map_block
[`Series::map_async(_)`]: https://docs.rs/crossflow/latest/crossflow/series/struct.Series.html#method.map_async
[`Chain::map(_)`]: https://docs.rs/crossflow/latest/crossflow/chain/struct.Chain.html#method.map
[`Chain::map_block(_)`]: https://docs.rs/crossflow/latest/crossflow/chain/struct.Chain.html#method.map_block
[`Chain::map_async(_)`]: https://docs.rs/crossflow/latest/crossflow/chain/struct.Chain.html#method.map_async
