# Stream Out

In crossflow all services are able to have [output streams](./output_streams.md),
which allow services to produce output messages while still running. These are
different from the *final output* in two ways:
* A service can have multiple output streams with different names and message
  types, whereas its final output has one fixed message type.
* Each output stream can produce ***any number*** of output messages per activation
  of the service, including 0---outputting no message at all---whereas the final
  output always produces exactly one output message per service activation,
  unless the service gets cancelled.

When you spawn a workflow it will be encapsulated as a service. The **stream out**
operation is how your workflow can provide output streams as a service. This
operation will take any messages passed to it and forward them out of the
workflow session.

For example suppose you have a workflow that takes in a basket of apples and
chops them one at a time:

![scope-stream](./assets/figures/scope-stream.svg)

You can design the workflow so that each time an apple is chopped, the slices
are sent out through an output stream, and then the workflow continues on to
chop the next apple. The workflow ends when there are no apples left, and the
final output of the workflow is just a [trigger](./branching.md#trigger) message.
