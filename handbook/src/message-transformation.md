# Message Transformation

It's not often that a message can be trivially fed directly from the output of one service to the input of another service.
Most of the time the services that you want to connect to each other will have slight differences between the `Response` type that the first produces and the `Request` type that you're trying to pass it along to.

Data transformation is something that Rust excels at, and there are many utilities in the language to assist with it.
For crossflow, many of the builtin workflow operations are aimed at helping you to transform or manipulate data so that information can flow through your workflow in the exact right way.
Better yet, the compiler will validate these transformations, so you can catch almost all workflow errors before you even run your program.

We already covered a few operations that help to transform messages.
[fork-result](./building-a-chain.md#recreating-fork-result) and [fork-option](./building-a-chain.md) help you manipulate [`Result`][Result] and [`Option`][Option] types, respectively, by extracting the inner values of their variants and sending them down different branches.
This page will take you through a few other useful tools for manipulating messages within a workflow.
We will be making extensive use of [blocking maps](./creating-a-node.md#create_map_block) since they are the most efficient and ergonomic way to apply a quick data transformation closure to a message.

> [!TIP]
> We will be using the [`Chain`][Chain] API in this page, but all of these operations have functionally equivalent methods available in the [`Builder`][Builder] API as well.
> Use whichever suits your workflow building style best.
>
> The only difference is that the [`Builder`][Builder] API will provide both the [InputSlots][InputSlot] and [Outputs][Output] for every operation and leave it up to you to connect them later, whereas the [`Chain`][Chain] API connects these as you create the operations.

#### unzip

On the previous page we showed how [fork-clone](./building-a-chain.md#recreating-racing) can be used to activate multiple branches at once.
It's not often that both branches actually need the same message data, so using fork-clone will likely send more information than needed down each branch.

The [unzip](./parallelism.md#unzip) operation allows you to organize exactly what will go down each branch before forking.
Transform the message into a tuple where each element of the tuple goes down a separate branch, and then use unzip:




[Result]: https://doc.rust-lang.org/std/result/
[Option]: https://doc.rust-lang.org/std/option/
[Chain]: https://docs.rs/crossflow/latest/crossflow/chain/struct.Chain.html
[Builder]: https://docs.rs/crossflow/latest/crossflow/builder/struct.Builder.html
[InputSlot]: https://docs.rs/crossflow/latest/crossflow/node/struct.InputSlot.html
[Output]: https://docs.rs/crossflow/latest/crossflow/node/struct.Output.html
