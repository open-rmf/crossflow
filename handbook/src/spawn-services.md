# How to Spawn a Service

For crossflow to be useful, you first need to have a [`Service`][Service]
to run. You may be able to find thirdparty libraries that provide some services
for you to use, but this chapter will teach you how to spawn them yourself just
in case you need to start from scratch.

### What is a service?

In its most distilled essence, a service is something that can take an input
message (request) and produce an output message (response):

![apple-pie-service](./assets/figures/service.svg)

Each [service][Service] expresses its request and response types as generic parameters in
the [`Service`][Service] struct. These `Request` and `Response` parameters can be
***any*** data structures that can be passed between threads.

> [!TIP]
> We mean "data structures" in the broadest possible sense, not only "plain data".
> For example, you can pass around utilities like [channels](https://tokio.rs/tokio/tutorial/channels)
> and [publishers](https://docs.rs/rclrs/latest/rclrs/type.Publisher.html) as
> messages, or use them as fields inside of messages. **Any valid Rust `struct`** that
> can be safely moved between threads can be used as a message.

The [`Service`][Service] data structure itself is much like a function pointer. It contains
nothing besides an [`Entity`][Entity]
(a lightweight identifier that points to the service's location in the
[`World`](https://docs.rs/bevy/latest/bevy/prelude/struct.World.html)) and the
type information that ensures you send it the correct `Request` type and that you
know what `Response` type to expect from it. When you copy or clone a `Service`,
you are really just making a copy of this identifier. The underlying service
implementation that will be called for the copy is the exact same as the original.

### Spawn a Blocking Service

The simplest type of service to spawn is called a "blocking" service. These
services are much like ordinary functions: They receive an input, run once for
that input, and immediately return an output value. Much as the name implies, a
blocking service will block all other activity in the [schedule](https://docs.rs/bevy/latest/bevy/prelude/struct.Schedule.html)
until it is done running. Therefore blocking services must be short-lived.

To define a blocking service, create a function whose input argument is a `BlockingServiceInput`:

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:sum_fn}}
```

This function will define the behavior of our service: The request (input)
message is passed in through the `BlockingServiceInput` argument. The request
type is a `Vec<f32>`, and the purpose of the function is to sum up the elements
in that vector. The output of the service is a simple `f32`.

Before we can run this function as a service, we need to spawn an instance of it.
We can use the [`AddServicesExt`](https://docs.rs/crossflow/latest/crossflow/service/trait.AddServicesExt.html#tymethod.spawn_service) trait for this:

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:spawn_sum}}
```

We can spawn the service while building our Bevy [`App`](https://docs.rs/bevy/latest/bevy/app/struct.App.html) to make sure that it's
available whenever we need it. A common practice is to save your services into
[Components](https://docs.rs/bevy/latest/bevy/ecs/component/trait.Component.html) or [Resources](https://bevy.org/learn/quick-start/getting-started/resources/) so
they can be accessed at runtime when needed.

Now that you've seen how to spawn a system, you could move on to
[How to Run a Service](./run-services.md). Or you can continue on this page to
learn about the more sophisticated abilities of services.

### Services as Bevy Systems

A crucial concept in Bevy is [systems](https://bevy-cheatbook.github.io/programming/systems.html).
Bevy uses an [Entity-Component-System (ECS)](https://en.wikipedia.org/wiki/Entity_component_system)
architecture for structuring applications. Systems are how an ECS architecture
queries and modifies the data in an application. In crossflow, services ***are***
Bevy systems, except instead of being scheduled like most systems, a service only
gets run when needed, taking in an input argument (request) and returning an output
value (response). Since they are Bevy systems, services can query and modify the
entities, components, and resources in the `World`.

In the example from the previous section, you may have noticed `In(input): ...`.
That [`In`](https://docs.rs/bevy_ecs/latest/bevy_ecs/system/struct.In.html) is a
special type of [`SystemParam`](https://docs.rs/bevy/latest/bevy/ecs/system/trait.SystemParam.html#derive)
for a value that is being directly passed into the system rather than being
accessed from the world. For blocking services we pass in a
[`BlockingService`][BlockingService] as the input, which contains the request data,
[output streams](./output-streams.md), and some other fields that represent metadata about the service.

Just like any other Bevy system, you can add as many system params to your service
as you would like. Here is an example of a blocking service that includes a
[`Query`](https://docs.rs/bevy/latest/bevy/prelude/struct.Query.html):

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:apply_offset_fn}}
```

First we define a component struct named `Offset` which simply stores a
[`Vec2`](https://docs.rs/glam/latest/glam/f32/struct.Vec2.html). The job of our
`apply_offset` service is to query for `Offset` stored for this service and apply
it to the incoming `Vec2` of the request.

To query the `Offset` we add `offsets: Query<&Offset>` as an argument (a.k.a.
system param) for our service function (a.k.a. system). One of the fields in
[`BlockingService`][BlockingService] is `provider`, which is the [`Entity`][Entity]
that *provides* this service. You can use this entity to store data that allows
the behavior of the service to be configured externally. In this case we will
store an `Offset` component in the `provider` to externally configure what kind
of `Offset` is being applied.

When spawning the service, you can use `.with` to initialize the provider entity:

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:spawn_apply_offset}}
```

In general you can use [`Service::provider`](https://docs.rs/crossflow/latest/crossflow/service/struct.Service.html#method.provider)
to access the provider entity at any time, and use all the normal Bevy mechanisms
for managing the components of an entity.

[Entity]: https://docs.rs/bevy/latest/bevy/prelude/struct.Entity.html
[Service]: https://docs.rs/crossflow/latest/crossflow/service/struct.Service.html
[BlockingService]: https://docs.rs/crossflow/latest/crossflow/struct.BlockingService.html

### Full Example

```rust,no_run,noplayground
{{#include ./examples/native/src/blocking_service_example.rs:example}}
```

### More kinds of services

If you are interested in non-blocking service types, continue on to [Async Services](./spawn-async-services.md) and [Continuous Services](./spawn-continuous-services.md).

If you need your service to be a portable object that isn't associated with an
entity, take a look at [Callbacks](./callbacks.md). If you don't care about your
service being a Bevy system at all (i.e. it should just be a plain function with
a single input argument) then take a look at [Maps](./maps.md).

If blocking services are enough to get you started, then you can skip ahead to
[How to Run a Service](./run-services.md).
