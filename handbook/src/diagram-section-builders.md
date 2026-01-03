# Section Builders

Sometimes you'll want to provide users with a workflow element that does more than a [node](./diagram-nodes.md).
Maybe you want to encapsulate a complex arrangement of operations as a single unit that users can drop into their workflows without worrying about the details of how it's implemented.
This is what we call a **Section**.

Section builders are able to generate a web of operations connected however necessary to fulfill the purpose of the section.
You can register section builders in much the same way you register [node builders](./diagram-nodes.md).
Once your section builder is registered, any diagram passed to your executor can include the section in its workflow.

> [!CAUTION]
> A section is ***not related*** to [scopes](./scopes.md) even though they superficially appear similar, as they both contain an arrangement of connected operations.
>
> When a section is put into a workflow **all operations in that section will exist in the original scope that the section has been placed in**.
> This has important implications for session and buffer behavior.
> Each message that enters a [scope](./scopes.md) will begin a new session, whereas **no  new session is created** when a message enters a section.

### Section Builder Options

Section builder options are essentially the same as [node builder options](./diagram-nodes.md#node-builder-options).
Refer to the node builder options guide to understand the fields in [`SectionBuilderOptions`][SectionBuilderOptions].

### Closure

Just like the [closure for node builders](./diagram-nodes.md#closure), section builders are implemented through a closure that takes in a [`&mut Builder`][Builder] and a `config`.
The `builder` is used to create and connect whatever elements your section needs.
The `config`---just like for node builders---is any deserializable data structure that provides the information needed to configure a section.

The key difference for a section is that it does ***not*** output a [`Node`][Node].
Instead it outputs any struct that implements the [`Section`][Section] trait:

```rust,no_run,noplayground
{{#include ./examples/diagram/calculator_ops_catalog/src/handbook_snippets.rs:elevator_example}}
```

In the above example we create a custom struct named `UseElevatorSection` to define what the inputs and outputs of our section are.
The `begin` input begins the overall process of having a robot use an elevator.
Each stage of using the elevator provides a signal to indicate if a problem has happened.
Diagrams that use this section have the opportunity to handle each error however they would like, and then signal a `retry_...` input slot to resume the process.

### Message Operation Support

When registering node builders, the `Request`, `Response`, and `Streams` message types also get registered.
You can add support for more operations by chaining them onto the [`NodeRegistrationBuilder`][NodeRegistrationBuilder].

Sections are somewhat similar: The message type of each field in the section will be automatically registered.
However it wouldn't make sense to use chain methods to register additioanl operations for those message types, because there are arbitrary number of messages within the section, and we can't assume that all the message types will support all the operations we want to add.

Instead we use Rust's procedural macro system:

```rust,no_run,noplayground
{{#include ./examples/diagram/calculator_ops_catalog/src/handbook_snippets.rs:section_operation_support}}
```

[SectionBuilderOptions]: https://docs.rs/crossflow/latest/crossflow/diagram/struct.SectionBuilderOptions.html
[Builder]: https://docs.rs/crossflow/latest/crossflow/builder/struct.Builder.html
[Node]: https://docs.rs/crossflow/latest/crossflow/node/struct.Node.html
[Section]: https://docs.rs/crossflow/latest/crossflow/diagram/trait.Section.html
[NodeRegistrationBuilder]: https://docs.rs/crossflow/latest/crossflow/diagram/struct.NodeRegistrationBuilder.html
