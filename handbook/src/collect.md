# Collect

> [!WARNING]
> At the time of this writing, the collect operation is not yet available as a JSON
> diagram operation. This is being tracked by [#59](https://github.com/open-rmf/crossflow/issues/59).
> In the meantime it can be put into a JSON diagram via the [section](./workflow_sections.md) builder operation.

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

## Circularity

It is okay to put a collect operation inside of a cycle. In this case, "upstream"
of the collect **does include** any operations **downstream** *that can lead back
to the collect operation*. When calculating reachability, the
collect operation will simply *prune itself* to prevent an infinite graph search.

![single-collect-cycle](./assets/figures/single-collect-cycle.svg)

However, there is an edge case to be wary of. If you have two collect operations
inside of one loop, then there is no logical way to resolve the reachability for
either of the collect operations.

![double-collect-cycle](./assets/figures/double-collect-cycle.svg)

Suppose all other activity within this cycle comes to a stop. Both collect
operations will want to check their own reachabilities, and each will want to
send out a message if it's not reachable. If the left collect operation assumes
that the right collect operation can still send out a message then the left
collect operation should consider itself reachable. At the same time if the right
collect operation assumes that the left can still send out a message then the right
should consider itself reachable. If both see themselves as reachable then neither
will ever decide to send out a message, but this directly contradicts the assumption
that both made. The result will be that neither ever sends out a message, which means
both collect operations are failing to do their jobs.

On the other hand if the left collect operation decides that the right will **not**
send out a message then the left will decide that it should send a message. At
the same time, the right collect operation would make the same assumption about
the left and also decide to send out a message. Now both collect operations will
produce messages every time the activity in the cycle settles down, meaning there
will be potentially infinite empty messages being produced by these operations
over the life of the workflow. Churning out infinite empty messages despite no
actual new activity would also violate the purpose of the collect operation.

No matter which assumption is used to implement the collect operation, there is
no way to get meaningful behavior. Instead the workflow will detect this when
it attempts to run and cancel with a [`CircularCollect`][CircularCollect] error.
Currently there is no mechanism to detect this type of error when compiling or
spawning the workflow. This issue is tracked by [#148](https://github.com/open-rmf/crossflow/issues/148).

### Scoping

It's still possible to have two collect operations in the same cycle, but you
need something to disrupt the circular dependency. You can use the [scope](./scopes.md#scope-operation)
operation to isolate one of the collect operations to focus exclusively on one
portion of the cycle, e.g. the portion between the left and right collect operations:

![scoped-collect-cycle](./assets/figures/scoped-collect-cycle.svg)

This effectively makes both collect operations invisible to each other. The left
collect operation will only check if the scope to its right has any active sessions.
The right collect operation will only check if there is any activity inside of its
own scope.

Strictly speaking this structure is not logically equal to the original circular
dependency structure---and good thing, because there is no way to resolve that
structure!---but it does allow us to have a pattern of multiple **spread**🠚**collect** or
**stream**🠚**collect** sections within one cycle of a workflow. We just need to
isolate each collect to a specific scope of activity.

[Vec]: https://doc.rust-lang.org/std/vec/struct.Vec.html
[Unreachability]: https://docs.rs/crossflow/latest/crossflow/cancel/struct.Unreachability.html
[CircularCollect]: https://docs.rs/crossflow/latest/crossflow/cancel/struct.CircularCollect.html
