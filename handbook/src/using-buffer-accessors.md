# Using an Accessor

[Buffers](./buffers.md) can act similar to the "blackboard" concept in behavior trees---a repository of data shared across the different services in your workflow.
To achieve thread-safe high-performance access to buffer data, we rely on Bevy's ECS.

Buffers are implemented as [entities][Entity] with certain components attached.
This means their data is stored in Bevy's highly optimized memory management system, that can automatically provide safe parallel access if you use [continuous services](./spawn-continuous-services.md).
The catch is, to access that data you need to use [services](./spawn-services.md) or [callbacks](./callbacks.md) which can support system parameters.
In particular you need to use the [`BufferAccess`][BufferAccess] system parameter for read-only parallelizable access, or [`BufferAccessMut`][BufferAccessMut] for exclusive read/write access.


> [!NOTE]
> [Maps](./maps.md) cannot access buffer data.
> In general maps cannot use system parameters, making them slightly more efficient than [services](./spawn-services.md) or [callbacks](./callbacks.md), but leaving them unable to do as much.

### With Access

To grant a service access to a buffer, you must provide it with a [`BufferKey`][BufferKey].
The two typical ways to obtain a buffer key are through the [listen](./listen.md) operation or the [buffer access](./buffer-access.md) operation.
* The [listen operation](./listen.md) will send out a message containing one or more buffer keys when the buffers attached to it are modified.
* The [buffer access operation](./buffer-access.md) will append a buffer key to a message that's in flight from one node to another.

The important thing is that a [`BufferKey`][BufferKey] is somehow present inside the message data that gets passed into the service.
The following example shows the [`Chain::with_access`][Chain::with_access] method adding a buffer key to messages that are being passed into some services.
It also shows how those services use [`BufferAccess`][BufferAccess] and [`BufferAccessMut`][BufferAccessMut] to view and modify the contents of a buffer.

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:buffer_access_example}}
```

### Listen

The [buffer access operation](#with-access) ([with_access][Chain::with_access]) only gets activated when a message is emitted by some operation output.
To monitor the contents of one or more buffers directly, you need to use [listen](./listen.md).
The listen operation connects to one or more buffers and gets activated any time a modification is made to the contents of any one of the buffers connected to it.

When listen is activated, is passes along an [accessor](./listen.md#accessor) as a message.
Unlike the [buffer access operation](#with-access), the message produced by a listener is nothing but a collection of buffer keys.
For the listen operation to be useful, you need to send its message to a service that will do something with those keys.

The following code example recreates a simple intersection crossing workflow:

![listen-accessor](./assets/figures/listen-accessor.svg)

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:listen_example}}
```

### Gate

Besides being able to access the data in buffers, you can also use a buffer key to open and close the [gate](./workflow-reflection.md#gate) of the buffer.
When a the gate of a buffer is closed, listeners will ***not*** be notified when the content of the buffer gets modified.
However ***this does not prevent*** the buffer from being accessed by any services that have a key for it.

> [!NOTE]
> Closing a buffer gate has no effect on the [buffer access operation](#with-access).

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:gate_example}}
```

[Entity]: https://docs.rs/bevy/latest/bevy/prelude/struct.Entity.html
[BufferAccess]: https://docs.rs/crossflow/latest/crossflow/buffer/struct.BufferAccess.html
[BufferAccessMut]: https://docs.rs/crossflow/latest/crossflow/buffer/struct.BufferAccessMut.html
[BufferKey]: https://docs.rs/crossflow/latest/crossflow/buffer/struct.BufferKey.html
[Chain::with_access]: https://docs.rs/crossflow/latest/crossflow/chain/struct.Chain.html#method.with_access
