# What is Crossflow?

Crossflow is a general-purpose Rust library for reactive and async programming.
It simplifies the challenges of implementing highly async software systems that
may have parallel activities with interdependencies, conditional branching,
and/or cycles. Its specialty is creating event-driven multi-agent state machines.

Implemented in Rust on the [Bevy](https://bevy.org/) [ECS](https://en.wikipedia.org/wiki/Entity_component_system),
crossflow has high performance and guaranteed-safe parallelism, making it suitable
for highly responsive low-level state machines, just as well as high-level visually
programmed device orchestrators.

## Workflows

![sense-think-act](./assets/figures/sense-think-act_workflow.svg)

A state machine can be defined by creating a workflow, and then the workflow can
be executed with a simple

```rust
let response = commands.request(input, workflow).take_response();
```

In the above example `workflow` is a [`Service`][Service] created by
[`spawn_workflow`][spawn_workflow]. `input` must match the [`Request`][Service]
type of `workflow`. `commands` is a Bevy [`Commands`][Commands] struct. The
`take_response()` method allows you to eventually receive the final response of
the workflow. This line of code is non-blocking, so the request will be processed
in the "background" as your application runs, and you can periodically check
`response` to see if the request is complete, or you can use Rust's built-in
[`.await`][await] language feature on `response` in an async code block to receive
the final output of the request. You can find more options for handling responses
in the [How to Run a Series](./run_a_series.md) chapter.

You can run as many requests as you want at the same time for as many workflows
or services as you want, including making multiple simultaneous requests for the
same service or workflow. Each time [`request`][request] is called, a new "session"
will be spawned that executes the workflow or service independently from any other
that is running. However, if any of the services or workflows interact with
"the world" (either the Bevy [World][World], the actual physical world, or some
external resource) then those services or workflows may indirectly interact with
each other.

To get you started with crossflow, the next chapter will teach you how to spawn
a basic service. After that you will see [how to run a service](./run_a_service.md),
then how to [assemble services into a workflow](./build_a_workflow.md) (which is
itself a service) and execute it.

To learn the fundamental concepts around what a "workflow" is in the crossflow
library, see the [Introduction to Workflows](./introduction_to_workflows.md) chapter.

[Service]: https://docs.rs/crossflow/latest/crossflow/service/struct.Service.html
[spawn_workflow]: https://docs.rs/crossflow/latest/crossflow/workflow/trait.SpawnWorkflowExt.html#tymethod.spawn_workflow
[Commands]: https://docs.rs/bevy/latest/bevy/prelude/struct.Commands.html
[await]: https://doc.rust-lang.org/std/keyword.await.html
[request]: https://docs.rs/crossflow/latest/crossflow/request/trait.RequestExt.html#tymethod.request
[World]: https://docs.rs/bevy/latest/bevy/prelude/struct.World.html
