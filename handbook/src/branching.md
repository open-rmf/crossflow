# Branching

What distinguishes a workflow from a [series](./run_a_series.md) is that workflows
support additional **operations** besides just chaining services together. For
example, fork-result creates a fork (a point with diverging branches) in the
workflow where one of two branches will be run based on the value of the input
message:

![fork-result](./assets/figures/fork-result.svg)

The input message of a fork-result must be the [`Result<T, E>`](https://doc.rust-lang.org/std/result/)
type. `Result` is an [enum](https://doc.rust-lang.org/book/ch06-01-defining-an-enum.html)
with two variants: `Ok` and `Err`. The `Ok` variant will contain an object of
type `T` while `Err` will contain a type `E`. `T` and `E` can be any two data
structures of your choosing.

Each variant gets its own branch coming out of the fork-result operation. Each
time a message is sent into the fork-result operation, **exactly one branch** will
activate depending on whether the message is an `Ok` and `Err` variant. In Rust,
an instance of an enum can only have a value of exactly one of its variants.

We say that fork-result does "conditional branching" because it creates a fork
in the workflow where one or more branches might not be activated when a message
arrives.

### Trigger

Another conditional branching operation is fork-option. Similar to `Result<T, E>`,
the [`Option<T>`](https://doc.rust-lang.org/std/option/) type in Rust is an enum
with two variants: `Some` and `None`. What makes `Option` different from `Result`
is that the `None` variant has no inner value. An `Option<T>` either contains some
value `T` or it contains no value at all (similar to
[`std::optional`](https://en.cppreference.com/w/cpp/utility/optional.html)
in C++).

When we fork an `Option` we still produce two possible branches:

![fork-option](./assets/figures/fork-option.svg)

The branch for `Some` will carry the inner `T` message that the `Option` contained,
but the `None` branch has no value to carry. Instead it contains a message of
the [unit type][UnitType] represented in Rust code as an empty tuple `()`.

The [unit type][UnitType] concept is a useful one in workflows. It's a way of
sending a signal that some branch in the workflow should activate but there is
no meaningful data to transmit. We refer to this pattern as a "trigger", and
operations that produce a unit type are called triggers.

In the above example we check whether an apple is available. If it is, we send
it to the kitchen to be baked into a pie. If no apple was available then we go
to the supermarket and bring back an apple to send to the kitchen. The context
of the workflow is enough for us to know that we need to pick up an apple from
the supermarket (this information could be embedded in the service configuration),
so a trigger with no message data is sufficient to activate that service.

> [!NOTE]
> If you have a custom enum type that you would like to use for forking, we have
> an [open issue ticket](https://github.com/open-rmf/crossflow/issues/144) to add
> support for this.

[UnitType]: https://doc.rust-lang.org/std/primitive.unit.html
