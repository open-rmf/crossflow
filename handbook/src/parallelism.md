# Parallelism

## Clone

Besides conditionally running one branch or another, some operations can run
multiple branches in parallel. For example, the
[fork-clone](https://docs.rs/crossflow/latest/crossflow/builder/struct.Builder.html#method.create_fork_clone)
operation takes in any cloneable message and then sends one copy down each each
of the branches coming out of it:

![fork-clone](./assets/figures/fork-clone.svg)

Each branch coming out of the fork-clone will run independently and in parallel.
Unlike in a behavior tree, these branches are completely decoupled from here on
out, unless you choose to [synchronize](#synchronization) them later. You are
free to do any other kind of branching, cycling, connecting for each of these
branches, without needing to consider what any of the other branches are doing.

## Unzip

Another way to create parallel branches is to unzip a [tuple](https://doc.rust-lang.org/std/primitive.tuple.html)
message into its individual elements:

![fork-unzip](./assets/figures/fork-unzip.svg)

The tuple can have any number of elements (up to 12), and the fork-unzip will
have as many output branches as its tuple had elements. Just like fork-clone,
each branch will be fully independent.

## Spread

Parallelism in workflows is not limited to forking into parallel branches. A
single branch can support any amount of parallel activity. In other words, a single
node can be activated multiple times simultaneously, with each run being processed
independently. One easy way to parallelize a single branch is with the [spread][spread] operation:

![spread](./assets/figures/spread.svg)

The [spread][spread] operation takes an iterable message type (e.g. [`Vec`][Vec],
[array], or anything that implements [`IntoIterator`][IntoIterator]) and spreads
it out into `N` separate messages where `N` is however many elements were in the
collection.

All of the messages coming out of the spread will be sent to the same input slot.
In principle they are all sent "at the same time" although the exact details will
vary based on what kind of operation they are being sent to:
* If they are sent to a [blocking service](./spawn_a_service.md#spawn-a-blocking-service) then
  they will be processed one-at-a-time but all of them will be processed within the same "flush"
  of the workflow (i.e. within one update frame of the Bevy [schedule][Schedule]).
* If they are sent to an [async](./spawn_async_service.md) service then the
  service will be activated with one message at a time, but the Futures from each
  call will be processed in parallel by the
  [async compute task pool](https://docs.rs/bevy/latest/bevy/tasks/struct.AsyncComputeTaskPool.html).
* If they are sent to a [continuous](./spawn_continuous_service.md) service then
  they will all be queued up for the continuous service together, and the
  continuous service will see all of them in its queue the next time it gets run.

The inverse of the spread operation is the [collect](./synchronization#collect) operation.

## Split

[spread]: https://docs.rs/crossflow/latest/crossflow/chain/struct.Chain.html#method.spread
[Vec]: https://doc.rust-lang.org/std/vec/struct.Vec.html
[array]: https://doc.rust-lang.org/std/primitive.array.html
[IntoIterator]: [https://doc.rust-lang.org/std/iter/trait.IntoIterator.html]
[Schedule]: https://docs.rs/bevy/latest/bevy/prelude/struct.Schedule.html
