# Callbacks

When you [spawn a service](./spawn-services.md) you get back a [`Service`] instance.
The implementation of that [`Service`] will be stored inside the Bevy ECS until you choose to despawn it.

In some cases you might not want the implementation to exist inside the Bevy ECS.
You might prefer an object that you can pass around and which will have [RAII]---freeing its memory when it is no longer needed.

**Callbacks** are an alternative to services whose lifecycle is outside of the Bevy ECS but still act as [Bevy systems]---able to interact with entities, components, and resources.
They also fulfill the role of services---taking in a Request message and passing back a Response message, potentially with [output streams](./output-streams.md) as well.

There are three key differences between a service and a callback:
* Callbacks do **not** need to be spawned with [`Commands`].
* Callbacks are not associated with any [`Entity`] and therefore do not have any [provider] that you can store components on.
* A callback will automatically deallocate when all references to it are dropped.

> [!TIP]
> The more general term that we use to refer to services and service-like things---such as callbacks---is [provider](./provider-overview.md).

### How to use

To use a callback, simply define the callback either as a `fn` or a closure, and then apply `.as_callback()` to its name.
Note the use of [`BlockingCallbackInput`] instead of [`BlockingServiceInput`]:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:callback_example}}
```

##### Async

Async callbacks are implemented in much the same way as [async services](./spawn-async-services.md), just replacing [`AsyncServiceInput`] with [`AsyncCallbackInput`]:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:async_callback_example}}
```

Same as for blocking callbacks, you turn the `fn` definition into a callback by applying `.as_callback()` to it.

> [!NOTE]
> There is no callback equivalent to continuous services.
> Continuous services **must** exist inside the Bevy ECS.
> There is currently no way around this.

##### Closures

You can also turn a closure into a callback.
Sometimes the syntax for this is confusing, but the easiest way to make it work is to first define the closure as a variable and then convert that variable into a callback:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:closure_callback_example}}
```

This also works for async callbacks, but you need to use the async block syntax:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:async_closure_callback_example}}
```

### Service/Callback Agnostic Implementation

In some cases you might want to implement a Bevy system [provider](./provider-overview.md) but don't want to commit to choosing a service or a callback.
For that you can create a regular Bevy system that takes an [input] and convert it into a service or callback later.

Whether you make it a service or a callback, the `Request` message type will match the [input] of the Bevy system, and the `Response` message type will match either its return value (for blocking) or the output of its Future (for async).


Here's an example of converting a blocking function into a service and a closure:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:agnostic_blocking_example}}
```

And here's an example for an async function:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:agnostic_async_example}}
```

> [!CAUTION]
> `.into_async_callback()` does not work for systems whose only system parameter is the [input].
> Trying to convert such a function into a callback will result in a compilation error.
> This problem is being tracked by [#159](https://github.com/open-rmf/crossflow/issues/159).

[`Service`]: https://docs.rs/crossflow/latest/crossflow/service/struct.Service.html
[RAII]: https://en.wikipedia.org/wiki/Resource_acquisition_is_initialization
[Bevy systems]: https://bevy-cheatbook.github.io/programming/systems.html
[`Commands`]: https://docs.rs/bevy/latest/bevy/prelude/struct.Commands.html
[`Entity`]: https://docs.rs/bevy/latest/bevy/prelude/struct.Entity.html
[provider]: https://docs.rs/crossflow/latest/crossflow/service/struct.Service.html#method.provider
[`BlockingCallbackInput`]: https://docs.rs/crossflow/latest/crossflow/type.BlockingCallbackInput.html
[`BlockingServiceInput`]: https://docs.rs/crossflow/latest/crossflow/type.BlockingServiceInput.html
[`AsyncServiceInput`]: https://docs.rs/crossflow/latest/crossflow/type.AsyncServiceInput.html
[`AsyncCallbackInput`]: https://docs.rs/crossflow/latest/crossflow/type.AsyncCallbackInput.html
[input]: https://docs.rs/bevy/latest/bevy/ecs/system/struct.In.html
