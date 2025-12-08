# What is Crossflow?

Crossflow is a general-purpose Rust library for reactive and async programming.
It simplifies the challenges of implementing highly async software systems that
may have parallel activities with interdependencies, conditional branching,
and/or cycles. Its specialty is implementing event-driven state machines.

Implemented in Rust on the [Bevy](https://bevy.org/) [ECS](https://en.wikipedia.org/wiki/Entity_component_system),
crossflow has high performance and safe parallelism, making it suitable for
highly responsive low-level state machines, just as well as high-level visually
programmed event orchestrators.

## Workflows

![sense-think-act](./assets/figures/sense-think-act_workflow.svg)

A state machine can be defined by creating a workflow, and then the workflow can
be executed with a simple

```rust
let response = commands.request(input, workflow).take_response();
```

In the above example `workflow` is a [`Service`](https://docs.rs/crossflow/latest/crossflow/service/struct.Service.html)
created by [`spawn_workflow`](https://docs.rs/crossflow/latest/crossflow/workflow/trait.SpawnWorkflowExt.html#tymethod.spawn_workflow).
`input` must match the [`Request`](https://docs.rs/crossflow/latest/crossflow/service/struct.Service.html)
type of `workflow`. `commands` is a Bevy [`Commands`](https://docs.rs/bevy/latest/bevy/prelude/struct.Commands.html)
struct. The `take_response()` method allows you to eventually receive the final
response of the workflow. This line of code is non-blocking, so the request will
be processed in the "background" as your application runs, and you can periodically
check `response` to see if the request is complete or use Rust's built-in `.await`
language feature on `response` in an async code block to receive the final output
of the request. You can find more options for handling responses in the
[Building a series](./building_a_series.md) chapter.

You can run as many requests as you want at the same time for as many workflows
or services as you want, including making multiple simultaneous requests for the
same service or workflow. Each time [`request`](https://docs.rs/crossflow/latest/crossflow/request/trait.RequestExt.html#tymethod.request)
is called, a new "session" will be spawned that executes the workflow or service
independently from any other that is running. However, if any of the services or
workflows interact with "the world" (either the Bevy [World](https://docs.rs/bevy/latest/bevy/prelude/struct.World.html),
the actual physical world, or some external resource) then those services or
workflows may indirectly interact with each other.

To get you started with crossflow, the next chapter will teach you how to spawn
a basic service. After that you will see how to run a service, then how to
assemble services into a workflow (which is itself a service) and execute it.
