# What is Crossflow?

Crossflow is a general-purpose Rust library for reactive and async programming.
It simplifies the challenges of implementing highly async software systems that
may have parallel activities with interdependencies, conditional branching,
and/or cycles. Its specialty is creating event-driven multi-agent state machines.

Implemented in Rust on the [Bevy](https://bevy.org/) [ECS](https://en.wikipedia.org/wiki/Entity_component_system),
crossflow has high performance and guaranteed-safe parallelism, making it suitable
for highly responsive low-level state machines, just as well as high-level visually
programmed device orchestrators.

## Services

The basic unit of work in crossflow is encapsulated by a service. Services take
an input message and eventually yield an output message---perhaps immediately or
perhaps after some long-running routine has finished.

![apple-pie-service](./assets/figures/service.svg)

In crossflow services are defined by [Bevy Systems][systems] that take an [input][In]
and produce an output. As [Bevy Systems][systems], the crossflow services can
integrate into an application by interacting with the [Bevy World][World] through
[entities][Entity], [components][Components], and [resources][Resources] of
[Bevy's ECS](https://en.wikipedia.org/wiki/Entity_component_system). This allows
services to have enormous versatility in how they are implemented, while still
being highly parallelizable, memory-safe, and efficient.

A service can be executed by sending it a request at any time using this line
of code:

```rust
let response = commands.request(input, service).take_response();
```

This line is non-blocking, meaning the service will be executed concurrently with
the rest of the application's activity. The `response` is a promise that can be
polled---or better yet [awaited][await] in an async context---until the service has sent
its response.

## Workflows

Best practice when creating complex systems is to encapsulate services into the
simplest possible building blocks and assemble those blocks into increasingly
sophisticated structures. This is where workflows come in.

![sense-think-act](./assets/figures/sense-think-act_workflow.svg)

Workflows allow you to assemble services into a directed graph---cycles *are*
allowed---that form a more complex behavior, feeding the output of each service
as input to another service. Workflows are excellent for defining state machines
that have async state transitions or that have lots of parallel activity that
needs to be managed and synchronized.

When you [create a workflow](./build_a_workflow.md) you will ultimately be
creating yet another *service* that can be treated exactly the same as a service
created using a Bevy System. This workflow-based service can even be used as a
node inside of another workflow. In other words, you can build hierarchical
workflows.

## Execution

You can run as many service requests as you want at the same time for as many
services or workflows as you want, including making multiple simultaneous requests
for the same service or workflow. Each time [`request`][request] is called, a new
"session" will be spawned that executes the workflow or service independently from
any other that is running. However, if any of the services or workflows interact
with "the world" (either the Bevy [World][World], the actual physical world, or
some external resource) then those services or workflows may indirectly interact
with each other.

> [!TIP]
> Check out the [live web demo][live-demo] to get a sense for what a workflow
> might look like.
>
> Try passing in `[20, 30]` as the request message and run the workflow to see
> the message get split and calculated.

## Getting Started

To get you started with crossflow, the next chapter will teach you how to spawn
a basic service. After that you will see [how to run a service](./run_a_service.md),
then how to [assemble services into a workflow](./build_a_workflow.md) (which is
itself a service) and execute it.

To learn the fundamental concepts around what a "workflow" is in the crossflow
library, see the [Introduction to Workflows](./introduction_to_workflows.md) chapter.

[Service]: https://docs.rs/crossflow/latest/crossflow/service/struct.Service.html
[spawn_workflow]: https://docs.rs/crossflow/latest/crossflow/workflow/trait.SpawnWorkflowExt.html#tymethod.spawn_workflow
[Commands]: https://docs.rs/bevy/latest/bevy/prelude/struct.Commands.html
[await]: https://rust-lang.github.io/async-book/part-guide/async-await.html#await
[request]: https://docs.rs/crossflow/latest/crossflow/request/trait.RequestExt.html#tymethod.request
[World]: https://docs.rs/bevy/latest/bevy/prelude/struct.World.html
[systems]: https://bevy-cheatbook.github.io/programming/systems.html
[In]: https://docs.rs/bevy/latest/bevy/ecs/system/struct.In.html
[Entity]: https://docs.rs/bevy/latest/bevy/prelude/struct.Entity.html
[Components]: https://docs.rs/bevy/latest/bevy/ecs/component/trait.Component.html
[Resources]: https://bevy.org/learn/quick-start/getting-started/resources/
[live-demo]: https://open-rmf.github.io/crossflow/?diagram=lZDRboMwDEX%2FxdojJfSVX5mqKSWmpCIJi521FeLf50BH0VRV6lNi3%2BT4%2Bo7wQU2HTkMNHfNAtVJRX8qT5S4dE2Fsgmf0XDbBqTCg30XXqiYGorYPFxWxJdWhNqSctl4Zq09Ru3KhlmcKHgr4wUhWbjVU5b6spMPohl4zEtTjVACxjiwyDb1lkcOQBbjuq90xtS3GXPFtQHlzb8gvkV%2FqC%2B2h%2FNEJv5PsZHUP9edMkaYccFigmy8%2BGBTxmGxv8hRwqZfa4zXb3fgrQJJq7QnqfVWAsTJL3754fZf9nIOVDFb2XGZ2Bsi%2Bi5UHcEM%2FrCMpuXk1OV641MasLse5z3m0xB6d9ZJ7ZvwzeSfPcbwRwNMEnkYgGUzTLw%3D%3D
