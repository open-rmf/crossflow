# Synchronization

Unfettered parallelism empowers your workflow to aggressively carry out its tasks
without being concerned about what other activities it's carrying out simultaneously.
However, sometimes those parallel threads of activity relate to each other in
important ways. Sometimes a service needs to obtain results from two different
branches before it can run.

When multiple separate branches need to weave back together, or concurrent
activity needs to be gathered up, we call that **synchronization**.

## Buffer

When services are connected together in a workflow, the next service in a chain
will receive input messages as soon as they're produced. But sometimes when a
service is finished, the message that it produces can't be used right away. It
may need to wait for some other service to finish first, so their results can be
combined.

For example, if we want to bake an apple pie, we don't need to wait for the oven
to preheat before we chop the apples, or vice versa. These can be done in parallel,
but we need both to be done before we can begin baking the pie:

![buffer-bake](./assets/figures/buffer-bake.svg)

To make this work, we capture the output of each service—`chop_apple` and
`preheat_oven`—in a separate **buffer**. A buffer is a workflow element that
can capture messages for later use. There are numerous ways and reasons to use a
buffer, but a spme examples include:
* Hold messages until the workflow is ready to consume them (similar to "places" in [Petri Nets][PetriNet])
* Store information about the state of workflow (similar to the "blackboard" in behavior trees)

In our apple pie use case, we want to see that the "apple chopped" and "temperature ready"
buffers both have a message before we pull those messages out of the buffers and
pass the apples along to be baked.

> [!TIP]
> Buffer settings allow you to specify how many messages a buffer can [retain][RetentionPolicy].
> You can also specify whether to discard the oldest or the newest messages when
> the limit is reached.
>
> This allows buffers to handle buildups of data if one branch is generating
> messages at a higher frequency than another branch that it needs to sync with.

Certain operations take buffers instead of messages as inputs. Those operations
will be activated on any change in any of the buffers connected to them, although
the exact behavior depends on the operation. Some examples are [join](#join) and
[listen](#listen).

## Join

A common use of buffers is to join together the results of two or more parallel
branches. Crossflow provides a builtin [**join**][Join] operation that can have
two or more buffers connected into it. As soon as ***at least one*** message is
present in ***each and every*** buffer connected to the join operation, the oldest message
will be pulled from each buffer and combined into a single message that gets sent
as the output of the join operation.

## Listen

## Buffer Access

## Collect

## Channels

[PetriNet]: https://en.wikipedia.org/wiki/Petri_net
[RetentionPolicy]: https://docs.rs/crossflow/latest/crossflow/buffer/enum.RetentionPolicy.html
[Join]: https://docs.rs/crossflow/latest/crossflow/builder/struct.Builder.html#method.join
