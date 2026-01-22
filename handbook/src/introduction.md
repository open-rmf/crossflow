# What is Crossflow?

[Crossflow] is a general-purpose Rust library for reactive and async programming.
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
let outcome = commands.request(input, service).outcome();
```

This line is non-blocking, meaning the service will be executed concurrently with the rest of the application's activity. The `outcome` is a receiver that can be polled---or better yet [awaited][await] in an async context---until the service has sent its response or has been cancelled.

## Workflows

Best practice when creating complex systems is to encapsulate services into the
simplest possible building blocks and assemble those blocks into increasingly
sophisticated structures. This is where workflows come in.

![sense-think-act](./assets/sense-think-act_workflow.svg)

Workflows allow you to assemble services into a directed graph---cycles *are*
allowed---that form a more complex behavior, feeding the output of each service
as input to another service. Workflows are excellent for defining state machines
that have async state transitions or that have lots of parallel activity that
needs to be managed and synchronized.

When you [create a workflow](./spawn-workflows.md) you will ultimately be
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
a basic service. After that you will see [how to run a service](./run-services.md),
then how to [assemble services into a workflow](./spawn-workflows.md) (which is
itself a service) and execute it.

To learn the fundamental concepts around what a "workflow" is in the crossflow
library, see the [Introduction to Workflows](./introduction-to-workflows.md) chapter.

[Crossflow]: https://github.com/open-rmf/crossflow
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
[live-demo]: https://open-rmf.github.io/crossflow/?diagram=3VhLb%2BM2EP4rhtCjGZMUKYm%2BF2gPPbVAD4uFwcfQVleWvHokGwT57x1K8isbO%2FJigwY9xSKHo%2Fk%2BznwzylP0S2M3sNXRMtq07a5ZLha1frhb5%2B2mM10Dta3KFsr2zlbbRbWDktRbv7B11TS%2BqB4WNfhmsQHtmsVW5%2BXC5Xpd6%2B3d4PXun6Yqo3l0D3WT469lRO%2FYHcWVFra7QrfQRMun53nUtLpucbvZFXmL29UubETfGKXhb%2Fu4A9wtKwe4abq8cFDjwrYr8LmEb%2BFsMCam8x635hHG7fN1tMTFeeRydKwfV%2B3RMgpv7bbXvGvnDt6f%2BvU2DxhaqLd5icEHHy9cB5e4%2Bk8VLA%2Bu%2B8fgOgSHyD69CBaf9g%2BfD3hGV7h3CwWvMfAaAT3%2Bnu2j8z37DXzt8M5zXexDHWKMPg%2FxHF5yPDouDPuXtwNh0Ng637VDPvxd1V9CIs3ajW5nfQDNrNvhI8w4KbutgXqWl7uunRV5085niLfN0QyaYLPFPZff567TRfE4nwWih435TJduhlc42rXVGvBHfYdQen8r%2BKYxB0MGfnp6EdUfx5cwHs%2FM4wxBo4sZ%2BggrmFOngQiZDDZ7E1xAC7xAzJP1Hk4NDR7BcEY3SZYEXu910QWKPqHfeTj5GVm8EFD%2FjhfR0BfBkEPEe5Ow8mY4BG3OosET4SBGgymJaQNlKOG%2BLA%2FlT8ZyJ%2BDytqrJPQvbG91s0EUMID1VRolEaMdErLkCby3LWEohcXGaAZd9OmP5YzXh0ZDew4XkDl0sV6ux7Fa9QqxWQTrGZO0lYx7tqiYfWEK9iJacy3n0iH9lSHAowLbaFHjA66IBTD%2Fdotjhm%2FQWmp22wVVI2y3opqsB3%2FoUPeSuRQQ8SMcG8vUGi0RSTN7vwzpowWloR4G4GF7ynvElUnGnvCOJ9pII4RwxKfOEm0xYTy3IWB%2BjHSXlRaC9bgYe%2B0BfjSrI9O%2FhfaNCVLt30%2BobCUiNSlNnE%2BJTxC4kZySzxmE%2BC6aN4ZiW%2FC0C9jclJxAQpPot%2FD%2FcTW7EHseUUUk5kWAsEVQLkjkniHE80zR1ACY9Yh870wXsYgL20cMZ%2BB%2FvdzeC5TQzaczxorMsRrBCEk2VIqBibTDTBcv8Wxcdy5sy%2FeZEn96RbwSfMdCSakawtikRFijRoCyRVolMSSGd60et8%2BZ%2B4arZlDQ%2FTGe3DQ23XmqsPGWJIrFIUb58IomhJg0algiwzAsfSul8tLgoYPFEATte0xm8k8nlFgzY6RjHDk9iVFsilPVEKZNgYibeSC54qk%2Fu5hKGfWpOxPBzIIR279anbdipNPZOWmIUQ0nheDUm0SmRLpEyppkAGocMqLq6D%2BrVtj3s%2FoZTWeh2ZVcUCF%2FXOHvckMq9%2FbmPra6%2FQP1r2WMaEeu6rh5sUTWIdH4EesTJKZIxGjvwGichNNwTXHUtDojDl0k%2FK5643ls%2Fn9Shy3SW%2BYxox1GEHNeouMgR94JR62LmetU40DOxPV8lbGqNfEjCLPdpJmIgArTC4LkkigOQJAWBZGEyMHNK2MR2fpWwyzPbh6QIG1dKneckSzKOjQ31SUFCCWZYYuMYsCDtKUUT8%2BEqRVMnh%2Fcg7KBcU%2FgajP%2BEr0PbiZbo1ENrN3%2F1%2B6hhRx32kCr85CDMCky1QKHOVEJSXDUGa5X2nz0HHifq9v%2BUR3aRR1T6VHPriEs8apxC4co0%2FrKGqVQqpTw%2FatZ0Dq7yOHWK%2F5AF7IwzPMDG5oUEZF6RjIMjaazxG5gnKfa1swKeNsleJWzq0PEhCUs0%2Fj%2BEUk2okwYrNcuIiWlCqM2YMSaW8lzxJo4MVwmb%2BqH8HoT1c%2FNQe98xNqraNOJonAonHCaNZBpBYEvVMlYkTaj0MmacUfaziZv63fXfEMeuE4cfJM%2FP%2FwI%3D
