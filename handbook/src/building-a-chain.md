# Building a Chain

Many workflows involve chaining services together rather than building complex graphs.
The API examples on the previous page use the [Builder] API which is the most general API, able to build any kind of directed (cyclic) graph.
However it is also maximally verbose, which---besides requiring more typing---can cause your workflow implementation to be scattered and difficult to follow.

We provide the [Chain] API as a streamlined alternative that suits cases where cycles aren't needed.
It allows you to build workflows by [chaining methods][chaining] together, a popular idiom in Rust.
This can save typing time and also allows your code to express the structure of the workflow.
Here we will recreate the workflows of the previous page, simplifying them using the [Chain] API.

#### simple sequence

Sequences of services can be chained together with a simple `.then(_)`:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:chain_services}}
```

A few notes about this example:
* [`Builder::chain(input)`][Builder::chain] allows us to begin creating a chain.
  In this case we begin the chain from `scope.start`, but you can begin a chain from *any* [`Output`][Output].
* [`Chain::then`][Chain::then] supports the same arguments as [`Builder::create_node`][Builder::create_node], meaning you can pass in [Services][Service] or [Callbacks][Callback].
* [`Chain::connect`][Chain::connect] takes in an [`InputSlot`][InputSlot] and ends the chain by feeding it into that input slot.
  This is useful for connecting the end of a chain into the terminate operation or looping it back to an earlier operation to create a cycle.

If you are dealing with [maps](./maps.md) and want to define in them inline when building the workflow, you can use [`Chain::map_block`][Chain::map_block] or [`Chain::map_async`][Chain::map_async] for that:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:chain_maps}}
```

The [`Chain::map_block`][Chain::map_block] and [`Chain::map_async`][Chain::map_async] methods are just syntactic sugar around [`Chain::then`][Chain::then] that makes it easier to put maps into the chain.
You can interleave calls to `.then`, `.map_block`, and `.map_async` however you would like when creating a sequence of nodes.

#### recreating [fork-result](./connecting-nodes.md#fork-result)

The [`Chain::fork_result`][Chain::fork_result] method allows you to create two diverging branches when an output message has a [`Result`][Result] type.
It takes two closures as arguments, where each closure builds one of the diverging branches.

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:fork_result_chain}}
```

The chain operation has special methods for certain message types that can further simplify how you express the chain.
For example the [`Result`][Result] type gets access to [`Chain::branch_for_err`][Chain::branch_for_err] that isolates the err branch of a fork-result and allows the rest of the chain to proceed with the ok branch:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:branch_for_err}}
```

This is typically used if the error handling branch is relatively small while the ok branch continues on for a long time.

#### recreating [fork-option](./connecting-nodes.md#fork-option)

It's less obvious how to create cycles when using chains, but it is still completely possible!
The key is to first create a [Node] so you can refer to the [`InputSlot`][InputSlot] later.
Here's an example of creating a cycle using a chain:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:fork_option_chain}}
```

A few notes about this example:
* [`Chain::map_block_node`][Chain::map_block_node] is the same as [`Chain::map_block`][Chain::map_block] except it ends the chain and gives back the [Node] of the map, allowing you to reuse its [`InputSlot`][InputSlot] and decide what to do with its [`Output`][Output] later.
  Each of the node chaining operations has a similar variant, e.g. [`Chain::map_async_node`][Chain::map_async_node] and [`Chain::then_node`][Chain::then_node].
* Similar to [`Chain::branch_for_err`][Chain::branch_for_err] for [`Result`][Result] types, there is also a [`Chain::branch_for_none`][Chain::branch_for_none] for [`Option`][Option] types.

#### recreating [racing](./connecting-nodes.md#racing)

When you have diverging parallel branches, an easy way to create one of those branches is with a [`Chain::branch_clone`][Chain::branch_clone].
You just pass in a closure that builds off a new chain that will be fed a clone of the original message.
The return value of `branch_clone(_)` will be a continuation of the original [`Chain`][Chain] that the `branch_clone` forked off of.

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:emergency_stop_chain}}
```

#### recreating [joining](./connecting-nodes.md#joining)

Chains even support synchronization operations like [join](./join.md).
We can structure our fork-clone a bit differently than we did in the racing example to set it up for an easy join:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:use_elevator_chain}}
```

A few notes about this example:
* [`Chain::fork_clone`][Chain::fork_clone] takes in a tuple of any number of closures.
  Each closure takes a [`Chain<T>`][Chain] as an input argument where `T` is the message type of the original chain.
* You will have to explicitly specify the `: Chain<_>` type of the closure arguments.
  The Rust compiler cannot infer this automatically due to limitations in what Traits can express, but you can always use the `_` filler for the generic parameter of the chain.
* The return value of [`Chain::fork_clone`][Chain::fork_clone] will be a tuple wrapping up all the return values of the closures.
  We have each closure end with [`Chain::output`][Chain::output] so we can gather up the plain outputs of all the branches into one tuple.
* The [join method][join-method] can be applied to tuples of [Outputs][Output], allowing us to apply a join operation at the end of these branches to synchronize them.
* The join method gives back a chain of the joined value that we can continue to build off of.

There are many ways to use [`Chain`][Chain] to structure a workflow.
Sometimes you will find it more concise and intuitive, but other times you might find it messy and confusing.
You can mix and match uses of [`Chain`][Chain] with uses of [`Builder`][Builder] however you would like.
Ultimately both APIs boil down to [InputSlots][InputSlot], [Outputs][Output], and [Buffers](./using-buffers.md) (which will be covered later),
making these APIs fully interoperable. Use whichever allows your workflow to be as understandable as possible.

#### recreating [unzipping](./connecting-nodes.md#unzipping)

Chains can also create forks using the unzip operation and join them together ergonomically:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:unzip_chain}}
```

[Builder]: https://docs.rs/crossflow/latest/crossflow/builder/struct.Builder.html
[Builder::chain]: https://docs.rs/crossflow/latest/crossflow/builder/struct.Builder.html#method.chain
[Builder::create_node]: https://docs.rs/crossflow/latest/crossflow/builder/struct.Builder.html#method.create_node
[Output]: https://docs.rs/crossflow/latest/crossflow/node/struct.Output.html
[Chain]: https://docs.rs/crossflow/latest/crossflow/chain/struct.Chain.html
[Chain::then]: https://docs.rs/crossflow/latest/crossflow/chain/struct.Chain.html#method.then
[Chain::then_node]: https://docs.rs/crossflow/latest/crossflow/chain/struct.Chain.html#method.then_node
[Chain::connect]: https://docs.rs/crossflow/latest/crossflow/chain/struct.Chain.html#method.connect
[Chain::map_block]: https://docs.rs/crossflow/latest/crossflow/chain/struct.Chain.html#method.map_block
[Chain::map_block_node]: https://docs.rs/crossflow/latest/crossflow/chain/struct.Chain.html#method.map_block_node
[Chain::map_async]: https://docs.rs/crossflow/latest/crossflow/chain/struct.Chain.html#method.map_async
[Chain::map_async_node]: https://docs.rs/crossflow/latest/crossflow/chain/struct.Chain.html#method.map_async_node
[Chain::fork_result]: https://docs.rs/crossflow/latest/crossflow/chain/struct.Chain.html#method.fork_result
[Chain::branch_for_err]: https://docs.rs/crossflow/latest/crossflow/chain/struct.Chain.html#method.branch_for_err
[Chain::branch_for_none]: https://docs.rs/crossflow/latest/crossflow/chain/struct.Chain.html#method.branch_for_none
[Chain::fork_clone]: https://docs.rs/crossflow/latest/crossflow/chain/struct.Chain.html#method.fork_clone
[Chain::branch_clone]: https://docs.rs/crossflow/latest/crossflow/chain/struct.Chain.html#method.branch_clone
[Chain::output]: https://docs.rs/crossflow/latest/crossflow/chain/struct.Chain.html#method.output
[chaining]: https://dhghomon.github.io/easy_rust/Chapter_35.html
[Result]: https://doc.rust-lang.org/std/result/
[Option]: https://doc.rust-lang.org/std/option/
[Service]: https://docs.rs/crossflow/latest/crossflow/service/struct.Service.html
[Callback]: https://docs.rs/crossflow/latest/crossflow/callback/struct.Callback.html
[InputSlot]: https://docs.rs/crossflow/latest/crossflow/node/struct.InputSlot.html
[Node]: https://docs.rs/crossflow/latest/crossflow/node/struct.Node.html
[join-method]: https://docs.rs/crossflow/latest/crossflow/buffer/trait.Joinable.html#tymethod.join
