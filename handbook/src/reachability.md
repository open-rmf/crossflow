# Reachability

An important thing to consider for the lifecycle of a workflow is whether it's
even possible for the workflow to end. If a workflow is allowed to keep running
indefinitely with no possibility of reaching the terminate operation, then the
caller who made the request will be left hanging forever. This can lead to
undesirable program behaviors like deadlocks.

To mitigate this problem, crossflow calculates the ***reachability*** of the
terminate operation each time an event occurs that could influence whether the
terminate operation can be reached. If at any point the terminate operation is
no longer reachable, then the session of the scope will automatically be
[cancelled](./scope-cancellation.md).

> [!NOTE]
> An operation is **reachable** if there exists at least one plausible path to
> the operation from a currently active operation.

## Inherent Unreachability

One kind of unreachability is ***inherent***, meaning the very structure of the
workflow makes it impossible for the terminate operation to ever be reached.

One way to get inherent unreachability is if nothing at all is connected to the
terminate operation:

![inherent-unreachability](./assets/figures/inherent-unreachability.svg)

A slightly more subtle version of inherent unreachability is when there is no
path from the start operation to the terminate operation because each is part of
a separate island of operations:

![inherent-unreachability-island](./assets/figures/inherent-unreachability-island.svg)

In either case, this unreachability will be detected immediately. Before the
initial message is even sent out by the start operation, crossflow will detect
that the terminate operation cannot be reached from the start operation, and the
workflow will be instantly [cancelled](./scope-cancellation.md) without any
operations running.

## Conditional Unreachability

Inherent unreachability fully prevents a workflow from running, but most of the
cases where we need to think about unreachability, it depends on **runtime conditions**.
Depending on which branch(es) are activated by an operation in the workflow, the
terminate operation might become unreachable.

Consider this simple [fork-result](./branching.md) example:

![conditional-unchreachability](./assets/figures/conditional-unreachability.svg)

The branch going to node `B` will lead to the terminate operation whereas the
branch going to node `C` never will. If the message produced by node `A` has an
`Ok` value then the workflow will have no problem reaching the terminate operation.
But for an `Err` value:

![conditional-unreachability-bad](./assets/figures//conditional-unreachability-bad.svg)

In this case crossflow will detect that the `Ok` message was "disposed" by the
fork-result operation and immediately perform a reachability check for the
terminate operation. Before node `C` even receives the message out of `A`, the
workflow will be cancelled because the terminate operation is not reachable.

Fixing this problem is relatively simple. You just need to connect node `D` to
the terminate operation:

![conditional-unreachability-fixed](./assets/figures/conditional-unreachability-fixed.svg)

There may be cases where you ***actually do want*** the `Err` branch to cancel your
workflow because only node `B` can correctly provide the final output of the
workflow, but you nevertheless want nodes `C` and `D` to run before the workflow
finishes running.

One way to accomplish this is by using the [cleanup](./scope-cleanup.md) feature.
You can pass the `Err` result into a buffer and then define a cleanup workflow
for that buffer. Nodes `C` and `D` go into the cleanup workflow to guarantee
that they get run even if the workflow gets cancelled.

![conditional-unreachability-cleanup](./assets/figures/conditional-unreachability-cleanup.svg)

The cleanup workflow can be designed so that nodes `C` and `D` do nothing if
there was no message in the buffer. Alternatively you can set the cleanup settings
so that this cleanup workflow only gets run during a cancellation and not during
a successful termination.

> [!NOTE]
> You might find that this solution is technically what you need, but the use of the
> cleanup feature does not express your workflow the way you would like. To provide
> better support for this use case, ticket [#150](https://github.com/open-rmf/crossflow/issues/150)
> proposes a new cancellation operation that would prevent the unreachability test
> from prematurely cancelling the workflow.
