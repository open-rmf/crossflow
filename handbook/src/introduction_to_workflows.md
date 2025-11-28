# Introduction to Workflows

If you need to assemble services in a more complex way than a [series](./run_a_series.md),
you can build a workflow. Building a workflow will ultimately leave you with a
[`Service`][Service] which you can use to [run the workflow](./run_a_service.md).

Fundamentally, workflows define how the output of one [service](./spawn_a_service.md)
should connect to the input of another service. Along the way, the data that is
being passed might undergo transformations.

![service-chain](./assets/figures/service-chain.svg)

## Branching

What distinguishes a workflow from a [series](./run_a_series.md) is that workflows
support additional **operations** besides just chaining services together. For
example, fork-result creates a fork (a point with diverging branches) in the
workflow where one of two branches will be run based on the value of the input
message:

![fork-result](./assets/figures/fork-result.svg)

## Parallelism



### Clone

Besides conditionally running one branch or another, some operations can run
multiple branches in parallel. For example, the
[fork-clone](https://docs.rs/crossflow/latest/crossflow/builder/struct.Builder.html#method.create_fork_clone)
operation takes in any cloneable message and then sends one copy down each each
branch coming out of it:

![fork-clone](./assets/figures/fork-clone.svg)

Each branch coming out of the fork-clone will run independently and in parallel.
Unlike in a behavior tree, these branches are completely decoupled from here on
out, unless you choose to [synchronize](#synchronization) them later. You are
free to do any other kind of branching, cycling, connecting for each of these
branches, without needing to consider any of the other branches.

### Unzip

Another way to create parallel branches is to unzip a [tuple](https://doc.rust-lang.org/std/primitive.tuple.html)
message into its individual elements:

![fork-unzip](./assets/figures/fork-unzip.svg)

The tuple can have any number of elements (up to 12), and it will have as many
output branches as it had elements. Just like fork-clone, each branch will be
fully independent.



## Synchronization

[Service]: https://docs.rs/crossflow/latest/crossflow/service/struct.Service.html
