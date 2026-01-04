# Nodes

You will typically need to supply your [registry][DiagramElementRegistry] with node builders in order to execute any useful JSON diagrams.
The registry will maintain a dictionary of the node builders that you give to it.
When a diagram is built into a workflow, the registered node builders will be used to construct the workflow.

This page will teach you all the details of how to register node builders.

### Node Builder Options

Each time you register a node builder you will need to set its [`NodeBuilderOptions`][NodeBuilderOptions].
The only *required* field in [`NodeBuilderOptions`][NodeBuilderOptions] is the `id`:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/diagram.rs:minimal_multiply_by_example}}
```

#### Display Text

This ID must be unique for each node builder added to the registry.
Registering a second node builder with the same ID as an earlier one will remove the earlier one from the registry.

When you only provide the ID, graphical editors will typically use that ID as display text for its associated nodes when visualizing a diagram.
This isn't always a good idea since the unique ID could be an unintelligible UUID, or it could be mangled with namespaces or version numbers.
Instead you can add a default display text to the node builder options:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/diagram.rs:default_display_text}}
```

The [diagram element registry][DiagramElementRegistry] can be serialized into JSON and exported from the executor.
The serialized registry data can be provided to a diagram editor or visualization frontend, which can then look up the display text and render it to the user.

#### Description

When someone is manually editing or viewing a workflow diagram, the purpose of a node might not be obvious from the ID or the display text.
Therefore your node builder options can also include a description for the nodes that will be built:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/diagram.rs:node_builder_description}}
```

Similar to the display text, the description will be included in the serialized [registry][DiagramElementRegistry], allowing diagram editors or visualizers to render the description for each node.

### Closure

Along with the node builder options, you need to provide a closure when registering a node builder.
The closure is what does the heavy lifting of creating the node for the workflow.
It will be provided with two arguments: a [`&mut Builder`][Builder] and a `config`.

The [`Builder`][Builder] API allows your closure to create *any* kind of node.
You can use it to call [`create_map_block`][Builder::create_map_block] or [`create_map_async`][Builder::create_map_async] for simple functions.
For services or callbacks you can call [`create_node`][Builder::create_node].

If you need to spawn a service within the closure, you can use [`Builder::commands`][Builder::commands] to get a [`&mut Commands`][Commands].
Just keep in mind that **services do *not* get automatically despawned when no longer used**, so you should avoid spawning a new service for each node that gets created.
Unless you do something to periodically clean up those services, you could end up with an arbitrarily growing number of services in your executor.

> [!TIP]
> The closure is expected to be [`FnMut`][FnMut], which means you can cache data inside of it that can be reused or updated each time the closure gets run.

The return type of your closure must be [`Node<Request, Response, Streams>`][Node].
That happens to be the return type of [`create_map_block`][Builder::create_map_block], [`create_map_async`][Builder::create_map_async], and [`create_node`][Builder::create_node], so your entire closure could be as simple as calling one of those methods on [`Builder`][Builder]:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/diagram.rs:minimal_add_example}}
```

The `Request`, `Response`, and `Streams` of the [`Node`][Node] that you return will be automatically detected by the registry.
They will be recorded as the input, output, and stream message types for nodes created by your node builder.

Technically the closure is not limited to only creating a single node.
You could have the closure additionally create buffers or anything else, as long as the final return value is a [`Node`][Node].
But it falls on you to ensure that the connected collection of elements that your closure builds can operate similar to a node, or else it may inflict confusing behavior on users and visualization tools.

> [!TIP]
> If you need to generate something more complex than a simple [`Node`][Node], consider registering a [section builder](./diagram-sections.md) instead.

#### Config

Each node builder can decide on its own `config` data structure.
This `config` will be the second argument passed to the closure.
In the earlier example the `config` data structure is a simple floating point value, `f32`.
Any type that implements [`Deserialize`][Deserialize] and [`JsonSchema`][JsonSchema] can be used as a `config` type.

For complex node configurations, you can define your own custom struct and derive the necessary traits:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/diagram.rs:custom_config}}
```

Deriving [`JsonSchema`][JsonSchema] for the config type allows us to **save a schema for the config in the registry**.
This helps graphical editing tools and diagram generation tools to ensure that all the nodes have valid configurations.
You could even auto-generate UIs tailored to the config of each node builder.

> [!TIP]
> If your node builder doesn't need any `config` information then just use the unit-type `()` as the config type.

##### Examples

Despite being provided with a config schema, human users may still struggle to figure out how to correctly configure a node to get what they want out of it.
To mitigate this problem, you can provide example configurations in your node builder options:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/diagram.rs:custom_config_with_examples}}
```

Crossflow graphical editing tools are encouraged to make these examples visible to users, and allow users to copy/paste the examples.

> [!TIP]
> If you want something more flexible than a static struct for your config, you can always just use [`JsonMessage`][JsonMessage] as the config type.
> Just be sure to include comprehensive examples so human users know what a valid config would be.

#### Fallible

There might be cases where a node builder cannot successfully build a node.
Maybe there is a semantic error in the `config` (even though the parsing was successful), or maybe some resource needed by the builder has become unavailable.

The node builder API above assumes that building the node will always be successful.
To allow the node building to fail, you can use the fallible API instead:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/diagram.rs:division_example}}
```

You can return any error that can be converted into an [`anyhow::Error`][anyhow::Error].
The easiest option is to use the [`anyhow!`][anyhow-macro] macro.

> [!NOTE]
> If the `"config"` in the diagram cannot be successfully deserialized into the data structure of your closure's `config`, this will be automatically caught by the registry, and a [`ConfigError`][ConfigError] will be returned instead of a `Service`.
> Your node builder does not have to handle this error mode.

### Message Operation Support

When you use [`register_node_builder`][DiagramElementRegistry::register_node_builder] from `DiagramElementRegistry`, the message types of `Request`, `Response`, and `Streams` will automatically be registered.
By default all registered messages will also register the ability to serialize, deserialize, and clone, which allows those message types to support operations like [fork-clone](./parallelism.md#clone) and conversion to/from [`JsonMessage`][JsonMessage].

For certain message types there may be additional operations that can be performed on them.
For example if a node returns a [`Result`][Result] type then users should be able to apply a [fork-result](./branching.md) to it.
A tuple message should be able to [unzip](./parallelism.md#unzip).
Unfortunately the Rust programming language does not yet support [specialization] as a stable feature.
To get around this, the [`register_node_builder`][DiagramElementRegistry] returns a [`NodeRegistrationBuilder`][NodeRegistrationBuilder] that lets you register support for additional operations.

At the end of registering a node, you can chain support for additional operators:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/diagram.rs:get_url_header_example}}
```

In the above example `WebPageQuery` implements the `Joined` trait, which means it can be the output of a [join](./using-buffers.md#joining) operation.
To register that ability for the message, we chain `.with_join()` after `.register_node_builder()`.

At the same time the return type of node is a [`Result`][Result].
We can also chain `.with_result()` to register [fork-result](./branching.md) support for the node's output message type.

When adding support for an operation, the message we are adding support for must be compatible with the operation.
This is ensured by the compiler.
If the node in the example above were not returning a [`Result`][Result] type then the Rust compiler would emit a compilation error when we try to register `.with_result()` for it.

Each operation support that we add using [`NodeRegistrationBuilder`][NodeRegistrationBuilder] will only apply to either the input message type or the output message type depending on whether the operation would produce the message or consume the message, respectively.

For finer grain control over exactly what operations are registered for each message type, continue to the next section.

### Special-case Message Registration

In most cases your messages types and their supported operations can be registered as described above---by registering a node and then chaining on any additional operations needed by the messages.
However there are some special cases that don't quite fit that pattern.

#### opt-out

Crossflow diagrams ***can*** support message types that don't implement clone or support serialization/deserialization.
By default the node registration API will assume that your node's messages ***do*** support all of those traits, because those are very typical traits for "[plain old data]" types.
We think if those operations weren't registered by default, there is a high likelihood that users would forget to register the operations, so by default we try to register them.

If your node produces or consumes messages that are not [plain old data], you will need to explicitly opt out of some traits in order to register the node builder.
If you forget to opt-out of the default operations for message types that don't support them, you'll see a compilation error.

Here's an example of a node with an input value that supports none of the default operations:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/diagram.rs:opt_out_example}}
```

The [`UnboundedReceiver`][UnboundedReceiver] does not support serialization, deserialization, or even cloning.
To register this node builder, we first need to use [`.opt_out()`][DiagramElementRegistry::opt_out] and then specify
* [`.no_cloning()`][no_cloning]
* [`.no_serializing()`][no_serializing]
* [`.no_deserializing()`][no_deserializing]

With those opted out, we can use the [`UnboundedReceiver`][UnboundedReceiver] as a message in the node.

To opt back into the default operations for the response, we apply [`.with_common_response()`][with_common_response].
A similar method exists if we were opting back in for the [request type instead][with_common_request].

#### Streams

Registering additional operations for request and response message types is fairly straightforward, but there isn't a clean way to do this for the message types that are present in output streams.
If you have output streams that produce custom data structures, they will be registered with the default message operations (unless you [opted out](#opt-out)), but to register any additional operations for those types, you will need to do it directly.

Instead of using the [`NodeRegistrationBuilder`][NodeRegistrationBuilder] API, you will need to use [`register_message`][DiagramElementRegistry::register_message]:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/diagram.rs:state_update_example}}
```

[DiagramElementRegistry]: https://docs.rs/crossflow/latest/crossflow/diagram/struct.DiagramElementRegistry.html
[DiagramElementRegistry::register_node_builder]: https://docs.rs/crossflow/latest/crossflow/diagram/struct.DiagramElementRegistry.html#method.register_node_builder
[DiagramElementRegistry::opt_out]: https://docs.rs/crossflow/latest/crossflow/diagram/struct.DiagramElementRegistry.html#method.opt_out
[DiagramElementRegistry::register_message]: https://docs.rs/crossflow/latest/crossflow/diagram/struct.DiagramElementRegistry.html#method.register_message
[no_cloning]: https://docs.rs/crossflow/latest/crossflow/diagram/struct.CommonOperations.html#method.no_cloning
[no_serializing]: https://docs.rs/crossflow/latest/crossflow/diagram/struct.CommonOperations.html#method.no_serializing
[no_deserializing]: https://docs.rs/crossflow/latest/crossflow/diagram/struct.CommonOperations.html#method.no_deserializing
[NodeBuilderOptions]: https://docs.rs/crossflow/latest/crossflow/diagram/struct.NodeBuilderOptions.html
[Builder]: https://docs.rs/crossflow/latest/crossflow/builder/struct.Builder.html
[Builder::create_map_block]: https://docs.rs/crossflow/latest/crossflow/builder/struct.Builder.html#method.create_map_block
[Builder::create_map_async]: https://docs.rs/crossflow/latest/crossflow/builder/struct.Builder.html#method.create_map_async
[Builder::create_node]: https://docs.rs/crossflow/latest/crossflow/builder/struct.Builder.html#method.create_node
[Builder::commands]: https://docs.rs/crossflow/latest/crossflow/builder/struct.Builder.html#method.commands
[Commands]: https://docs.rs/bevy/latest/bevy/prelude/struct.Commands.html
[FnMut]: https://doc.rust-lang.org/std/ops/trait.FnMut.html
[Node]: https://docs.rs/crossflow/latest/crossflow/node/struct.Node.html
[Serialize]: https://docs.rs/serde/latest/serde/trait.Serialize.html
[Deserialize]: https://docs.rs/serde/latest/serde/trait.Deserialize.html
[JsonSchema]: https://docs.rs/schemars/latest/schemars/trait.JsonSchema.html
[JsonMessage]: https://docs.rs/crossflow/latest/crossflow/buffer/enum.JsonMessage.html
[ConfigError]: https://docs.rs/crossflow/latest/crossflow/diagram/enum.DiagramErrorCode.html#variant.ConfigError
[anyhow::Error]: https://docs.rs/anyhow/latest/anyhow/struct.Error.html
[anyhow-macro]: https://docs.rs/anyhow/latest/anyhow/macro.anyhow.html
[Result]: https://doc.rust-lang.org/std/result/
[specialization]: https://std-dev-guide.rust-lang.org/policy/specialization.html
[NodeRegistrationBuilder]: https://docs.rs/crossflow/latest/crossflow/diagram/struct.NodeRegistrationBuilder.html
[plain old data]: https://en.wikipedia.org/wiki/Passive_data_structure
[UnboundedReceiver]: https://docs.rs/tokio/latest/tokio/sync/mpsc/struct.UnboundedReceiver.html
[with_common_response]: https://docs.rs/crossflow/latest/crossflow/diagram/struct.NodeRegistrationBuilder.html#method.with_common_response
[with_common_request]: https://docs.rs/crossflow/latest/crossflow/diagram/struct.NodeRegistrationBuilder.html#method.with_common_request
