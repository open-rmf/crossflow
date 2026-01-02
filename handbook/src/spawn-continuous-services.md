# Continuous Services

Most of the time a service can be written as a [blocking service](./spawn-services.md#spawn-a-blocking-service)
or an [async service](./spawn-async-services.md#spawn-an-async-service) depending on
whether the service is known to be short-lived or long-lived and whether it needs
to use the async feature of Rust. But there's one more shape a service can take
on: continuous.

One thing that blocking and async services share in common is that they are each
defined by a function that takes in a request as an argument and eventually yields
a response. On the other hand a **continuous service** is defined by a Bevy system
that runs continuously in the [system schedule][schedules]. Each time the
continuous service is woken up in the schedule, it can make some incremental
progress on the set of active requests assigned to it---which we call the order queue.

### What is a Continuous Service?

Each service---whether blocking, async, or continuous---is associated with a
unique [Entity][Entity] that we refer to as the **service provider**. For blocking
and async services the main purpose of the service provider is to
1. store the system that implements the service, and
2. allow users to configure the service by inserting components on the *service provider* entity.

For continuous services the service provider has one additional purpose: to store
the **order queue** component. This component is used to keep track of its active
requests---i.e. the requests aimed at the continuous service which have not received
a response yet. When a request for the continuous service shows up, `flush_execution`
will send that request into the *order queue* component of the service that it's
meant for.

![continuous-service-schedule](./assets/figures/continuous-service-schedule.svg)

Each continuous service will be run in the regular Bevy [system schedule][schedules]
just like every regular Bevy system in the application. This means continuous
services can run in parallel to each other, as well as run in parallel to all
other regular Bevy systems, as long as there are no read/write conflicts in what
the systems need to access.

Each time a continuous service is woken up by the schedule, it should check its
*order queue* component and look through any active requests it has. It should
try to make progress on those tasks according to whatever its intended behavior
is---perhaps try to complete one at a time or try to complete all of them at once.
There is no requirement to complete any number of the orders on any given wakeup.
There is no time limit or iteration limit for completing any orders. Any orders
that do not receive a response will simply carry over to the next schedule update.

Once an order *is* complete, the continuous service can issue a response to it.
After issuing a response, the request of the order will immediately become
inaccessible, even within the current update cycle. Responding to an order fully
drops the order from the perspective of the continuous service. This prevents
confusion that might lead to sending two responses for the same order. Nevertheless
the continuous service can respond to *any number* of unique orders withing a single
update cycle.

### Spawn a Continuous Service

Spawning a continuous service has some crucial differences from spawning blocking
or async services. These differences come from two facts:
1. The request information (order queue) persists between service updates as a component
2. The service needs to be added to the app's system schedule

This first difference means that **continuous services *must* query for their order queue**.
In the `hello_continuous_service` example below you can see the `query` argument
takes in a `ContinuousQuery` whose generic parameters perfectly match the generic
parameters of `ContinuousServiceInput` above it. This pattern is mandatory for
continuous services so they can access their order queue.

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:hello_continuous_service}}
```

Unlike other service types, the `srv` input arguments contains nothing but a `key`
that must be used by `query` to access the orders belonging to this service. This
access is fallible because it's possible for a user or a system to despawn the
provider of this service while the service is still inside the system schedule.
In most cases when the order queue can no longer be accessed, your continuous
service can just return immediately. Any requests sent to it will be automatically
cancelled.

When you do get access---in this case the `orders` variable---you can iterate
through each order, using `.request()` to borrow the request value and then
`.respond(_)` to send your response for the order.

#### Inside the System Schedule

The other important difference with continuous services is that they need to be
added to the app's [system schedule][schedules]. While blocking and async services
can be spawned anywhere that you have access to a `Commands`, continuous services
can only be spawned when you have access to an `App`:

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:spawn_hello_continuous_service}}
```

Instead of `spawn_service` which gets used by blocking and async services,
continuous services require you to call `spawn_continuous_service`. You'll also
notice that an additional argument is needed here: the label for which schedule
the continuous service should run in.

> [!NOTE]
> Currently Bevy does not support dynamically adding systems to the schedule,
> so continuous services will generally need to be added during App startup.

Despite these differences in how continuous services get spawned, you will still
ultimately receive a `Service<Request, Response>`, exactly the same as blocking
and async services. From the outside, continuous services appear no different
from other kinds of service; the differences in how it behaves are just
implementation details.

### Serial Orders

In the `hello_continuous_service` [example above](#spawn-a-continuous-service)
we respond to each request on the same update frame that the request arrives.
Very often continuous services are managing a process that needs to be spread out
across multiple frames. The example below shows an example of responding to a
sequence of orders, one at a time, where each order might need multiple frames
before it's complete.

```rust,no_run,noplayground
{{#include ./examples/native/src/continuous_service_example.rs:move_base_vehicle_to_target_example}}
```

On each update of the continuous service we only look at the oldest request in
the order queue, ignoring all others. This effectively makes the service "serial"
meaning no matter how many requests are coming in to it, they will only be handled
one at a time.

### Parallel Orders

One of the advantages that continuous services have over blocking and async is
the ability to handle many requests at once, potentially with interactions
happening between them. The example below shows how `orders.for_each(_)` can be
used to iterate over all active requests in one update frame, making incremental
progress on all of them at once.

```rust,no_run,noplayground
{{#include ./examples/native/src/continuous_service_example.rs:send_drone_to_target_example}}
```

[schedules]: https://bevy-cheatbook.github.io/programming/schedules.html
[Entity]: https://docs.rs/bevy/latest/bevy/prelude/struct.Entity.html

### Full Example

Here is an example that incorporates everything above and also demonstrates how
continuous systems can be configured in the schedule just like regular Bevy systems.
Notice the use of `.configure(_)` when spawning `move_base` and `send_drone`.
This allows us to make sure the base vehicle is up to date when the drone service
is updated.

```rust,no_run,noplayground
{{#include ./examples/native/src/continuous_service_example.rs:example}}
```
