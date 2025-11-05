# What is Crossflow?

Crossflow is a general-purpose Rust library for reactive and async programming.
It simplifies the challenges of implementing highly async software systems that
may have parallel activities with interdependencies, conditional branching,
and/or cycles. It specialty is implementing event-driven state machines.

Implemented in Rust on the [Bevy](https://bevy.org/) [ECS](https://en.wikipedia.org/wiki/Entity_component_system),
crossflow has high performance and safe parallelism, making it suitable for
high-frequency low-level state machines, just as well as low-frequency high-level
state machines.

## Workflows

![sense-think-act](../../assets/figures/sense-think-act_workflow.svg)

State machines can be defined as workflows, and then those workflows can
be executed in whatever way suits their use case. The same workflow can have
multiple independent sessions executing at once. Any number of workflows (same
or different) can execute simultaneously in one application.

