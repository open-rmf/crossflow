# Introduction to Workflows

If you need to assemble services in a more complex way than a [series](./run_a_series.md),
you can build a workflow. Building a workflow will ultimately leave you with a
[`Service`][Service] which you can use to [run the workflow](./run_a_service.md).

Fundamentally, workflows define how the output of one [service](./spawn_a_service.md)
should connect to the input of another service. Along the way, the data that is
being passed might undergo transformations.

![service-chain](./assets/figures/service-chain.svg)

There are three strategies available for creating workflows, each with its own
benefits and use-cases:

<style>
table th:first-of-type {
  width: 15%;
}
table th:nth-of-type(2) {
  width: 40%;
}
table th:nth-of-type(3) {
  width: 45%;
}
</style>

|  | Description | Benefits |
|--|-------------|----------|
| [![native Rust API](./assets/figures/binary-matrix.png "Native Rust API")](./build_a_workflow.md) | Write Rust code to define nodes and workflows using the native Rust API of crossflow |  🠚 compile-time validation <br> 🠚 access to all native features <br> 🠚 easily import plugins |
| [![runtime generation](./assets/figures/cyber-brain.png "Runtime Generation")](./json_diagrams.md) | Generate a JSON diagram based on the output of a planner or a description of some process | 🠚 runtime validation of diagram <br> 🠚 visualize generated diagrams <br> 🠚 implement highly dynamic systems |
| [![visual editor](./assets/figures/visual-diagram.png "Visual Editor")](https://open-rmf.github.io/crossflow/) | Visually design and configure a workflow with a graphical editor | 🠚 no-code programming <br> 🠚 validate diagram while editing <br> 🠚 runtime validation of diagram |

This chapter will introduce concepts that are relevant to all three. For building
workflows using the native Rust API, you can go to the [How to Build a Workflow](./build_a_workflow.md)
chapter. To learn about runtime generation and visual (no-code) editing of workflows,
go to [JSON Diagrams](./json_diagrams.md).

> [!TIP]
> See our [live web demo](https://open-rmf.github.io/crossflow/) of the open
> source crossflow diagram editor.

## Node

To put a service into a workflow you [create a node](./build_a_workflow.md#creating-a-node)
by specifying a service that will be run when the node is given an input:

![workflow-node](./assets/figures/workflow-node.svg)

In this case we've created a [node][Node] that will run the `bake_pie` service,
taking an apple and turning it into a pie. You can include the same service any
number of times in a workflow by creating a node for each place that you want to
run the service.

### Input Slots and Outputs

A node has one [input slot][InputSlot] and one final [output][Output].
There must be ***at least one*** output connected into a node's input slot in
order for the node to ever be activated, but there is ***no upper limit*** on how many
outputs can connect to an input slot. The node will simply be activated any time
a message is sent to its input slot, no matter what the origin of the message is.

The [output][Output] of a node must be connected to ***no more than one*** input
slot or operation. If an output is not connected to anything, we call that output
"disposed". If you want to connect your node's output to multiple input slots,
you will need to pass it through an operation like [clone](./parallelism.md#clone),
[unzip](./parallelism.md#unzip), or [split](./parallelism.md#split) depending on how
you want the message's data to be distributed.

To connect an output to an input slot, the data type of the output **must match** the
data type expected by the input slot. When using the native Rust API, you can
use a [map](./maps.md) node to transform an output message into a compatible input
message. When building a JSON diagram, you can use the [transform](./transform.md)
operation. A data type mismatch will either cause a compilation error (native Rust
API) or a workflow building error (JSON Diagram). Either way, the workflow will
not be allowed to run until the mismatch is resolved.

### Streams

If the service used by your node has [output streams](./output_streams.md) then
you will receive a separate [output][Output] for each stream:

![output-streams](./assets/figures/output-streams.svg)

In this example, the `pollinate` service has a side effect of producing more
flowers and producing honey. We can represent these side effects as output streams
of the service.

Each of these streamed outputs can be connected to a separate input slot or operation.
Each stream can carry a different data type, so make sure that you are connecting
each stream to a compatible input slot or operation.

When building a workflow, streamed outputs behave essentially the same as the
regular "final" output. There are just two practical characteristics that make
streamed outputs different:

* Output streams will only produce messages while the node is active. After
  the final output message has been sent, no more stream messages can appear
  until the node is activated again by a new input message.
* For as long as the node is active, an output stream may produce ***any number of
  messages***, including zero. This means you cannot rely on getting any messages
  from a stream unless you know something about the implementation of the service
  that the node is using.

## Operations

Besides just running services, workflows need to manage the flow of data between
those services. That includes [conditional branching](./branching.md),
[parallelism](./parallelism.md), and [synchronization](./synchronization.md). Continue
to the next page to learn about the other kinds of operations that you can use
in a workflow to achieve the behavior that you want.

[Node]: https://docs.rs/crossflow/latest/crossflow/node/struct.Node.html
[InputSlot]: https://docs.rs/crossflow/latest/crossflow/node/struct.InputSlot.html
[Output]: https://docs.rs/crossflow/latest/crossflow/node/struct.Output.html
[Service]: https://docs.rs/crossflow/latest/crossflow/service/struct.Service.html
