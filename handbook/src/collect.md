# Collect

Recall from the [parallelism](./parallelism.md) chapter that there are two ways
to have parallel activity within a workflow: [branching](./parallelism.md#clone) and
[spreading](./parallelism.md#spread). [Join](./join.md) and [listen](./listen.md)
allow us to synchronize activity across multiple branches, but they aren't
necessarily suited for managing parallel activity happening along a single branch,
which is what the spread operation does.

![collect](./assets/figures/collect.svg)

The inverse of the spread operation is **collect**. The collect operation will
monitor all workflow activity happening upstream of it. Messages sent to the
collect operation will be held onto until all upstream activity has finished,
then they will be collected into a single message (such as [`Vec`][Vec]) and
sent out.

## Maximum

You can also choose to collect until a certain number of elements has been reached.
Then regardless of the upstream activity, all the gathered elements will be sent
out as one message once the maximum has been reached. If more messages arrive
later then a new collection will start, and they will also be sent out once they
reach the maximum or all upstream activity has finished.

## Minimum

If the collection *needs* to reach a certain number of elements, you can set that
as a minimum. If all upstream activity ceases before the collect operation reaches
the minimum---meaning it is impossible to ever reach the minimum---then all the
gathered messages will be discarded and the collect operation will signal that
it has disposed a message, which could lead to a cancellation.

## Catching Unreachability

In crossflow a "disposal" happens any time one or more outputs of an operation
will ***not*** carry any message after the operation was activated. Whenever a
disposal happens we will check if it is still possible for the workflow to finish.
If a workflow can no longer finish because of a disposal then it will be cancelled
with an [`Unreachability`][Unreachability] error.

Consider the nature of the collect operation: It monitors upstream activity. One
way it identifies when upstream activity has finished is to monitor when disposals
happen. With each disposal, the collect operation will calculate its own
"reachability"---i.e. whether or not it is possible for the collect operation to
be reached by any of the ongoing activity in the workflow. Once the collect
operation is no longer reachable, it will send out its collection, as long as
the collection has more than the [minimum](#minimum) number of elements, otherwise
it will emit a disposal itself.

This means the collect operation has the ability to catch cases where some part
of your workflow may become unreachable. A node becoming unreachable is a natural
thing to happen in a workflow that contains any conditional branching, but sometimes
you may want to respond to that unreachability in a certain way, perhaps perform
a fallback action. By inserting a collect operation with a minimum of 0 and a
maximum of 1, you can catch when that part of the workflow becomes unreachable by
checking whether the output of the collect operation is empty or has 1 element.

> [!WARNING]
> At the time of this writing, the collect operation is not yet available for JSON
> diagrams. This is being tracked by [#59](https://github.com/open-rmf/crossflow/issues/59).

[Vec]: https://doc.rust-lang.org/std/vec/struct.Vec.html
[Unreachability]: https://docs.rs/crossflow/latest/crossflow/cancel/struct.Unreachability.html
