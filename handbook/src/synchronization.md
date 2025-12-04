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

### Join into struct

There are two ways to join buffers depending on whether you want to produce a
struct or a collection. To join into a struct, you will need to specify a key
(name) for each buffer connection. Each key will put the buffer value into a
matching field in the struct.

![join-keyed](./assets/figures/join-keyed.svg)

Each key can only be used once per join operation. If a buffer is used in
multiple join operations, it can have a different key for each join.

> [!NOTE]
> A keyed join can be used to produce more than static structs. The values can
> also be gathered into a map, such as [`HashMap`][HashMap], [`BTreeMap`][BTreeMap],
> or [JSON][JsonMessage] where the key name of the connection will be used as the
> buffer value's key in the map.

No matter what struct you intend to create through the join, the workflow builder
will ensure that every necessary field has a corresponding buffer connected with
a matching key name and matching data type. When building workflows with the native
Rust API, any mismatch will produce a compilation error. When building a workflow
from a JSON diagram, you will get an [`IncompatibleLayout`][IncompatibleLayout] error.

### Joint into sequence

While the keyed join is generally the recommended way to do a join (explicit key
names have more semantic value), it is also possible to join into a sequence,
such as an [array][array], [`Vec`][Vec], [tuple][tuple], or [JSON][JsonMessage].
For each connected buffer, specify a sequencing index that will determine where
in the sequence that element belongs.

![join-sequence](./assets/figures/join-sequence.svg)

When joining into an array or `Vec`, all the buffer values must have the same
data type. Joining into a tuple allows their data types to be mixed. Joining
into JSON requires the data to be serialized. Similar to error handling for the
keyed join, any incompatibility will produce a compilation error for the native
Rust API and an [`IncompatibleLayout`][IncompatibleLayout] error when building
from a JSON diagram.

### Fetch by clone

When the conditions are met for a join operation to activate (at least one message
is present in every connected buffer), the join operation will construct its
output message by "fetching" the oldest message from each of its connected buffers.
By default, "fetching" means to pull the message, removing it from the buffer and
moving it into the output message.

Sometimes there may be a reason to clone the message out of the buffer instead of
pulling it. For example if the buffer represents a checkpoint in your process
that doesn't need to be repeated, you can clone from the buffer to retain the
checkpoint.

Suppose we want to make many apple pies with the same oven.

![fetch-by-clone](./assets/figures/fetch-by-clone.svg)

We can't bake any pies without preheating the oven, but once the oven has reached
the right temperature, we do not need preheat it again. We can have our ingredient
preparation branch repeatedly prepare the pie pans for baking, and those pans will
be put in the oven as soon as the oven is finished preheating. Any new pans that
are prepared after the preheating is finished can go into the oven right away.

To express this behavior, we use fetch-by-clone for the buffer that tracks whether
the oven is preheated, while we do the regular fetch-by-pull for the prepared pan.
The "finished preheating" buffer will be able to retain its knowledge that the
preheating has finished, while the prepared pan buffer will have its pans consumed
(moved into the oven) each time that it can happen.

## Listen

## Buffer Access

## Collect

## Channels

[PetriNet]: https://en.wikipedia.org/wiki/Petri_net
[RetentionPolicy]: https://docs.rs/crossflow/latest/crossflow/buffer/enum.RetentionPolicy.html
[Join]: https://docs.rs/crossflow/latest/crossflow/builder/struct.Builder.html#method.join
[HashMap]: https://doc.rust-lang.org/std/collections/struct.HashMap.html
[BTreeMap]: https://doc.rust-lang.org/std/collections/struct.BTreeMap.html
[JsonMessage]: https://docs.rs/serde_json/latest/serde_json/enum.Value.html
[IncompatibleLayout]: https://docs.rs/crossflow/latest/crossflow/buffer/struct.IncompatibleLayout.html
[array]: https://doc.rust-lang.org/std/primitive.array.html
[Vec]: https://doc.rust-lang.org/std/vec/struct.Vec.html
[tuple]: https://doc.rust-lang.org/std/primitive.tuple.html
