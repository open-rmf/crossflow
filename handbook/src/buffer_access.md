# Buffer Access

There may be times when a service needs more information about what is going on
than what can be provided by the upstream service that connects into it. As we've
seen, workflows store their state information in [buffers](./buffers.md). Typically
we can design our workflow so that any additional information needed by a service
can be sourced from one or more of the workflow's buffers. This can function
similar to a blackboard in a Behavior Tree.

[Buffer access][BufferAccess] refers to a workflow operation that will take any
input and combine it with a buffer [Accessor](./listen.md#accessor) before passing
the combined message along to a node or other operation. For example, suppose
a node that does task planning outputs a target destination for a robot to
reach. For a path planner to determine how the robot should reach the target, it
will also need to know the robot's current location:

![buffer-access](./assets/figures/buffer-access.svg)

With the buffer access operation, we can take the target destination and combine
it with a buffer key or other [accessor](./listen.md#accessor) that the `plan_path`
service can use to check the latest location provided by the `localization` node.
The output type of the buffer access operation is a tuple whose first element is
the original output and the second element is a key or accessor that combines all
buffers connected to the access operation.

Just like listen, buffer access can form its associated buffers into any [Accessor](./listen.md#accessor)
that the downstream node asks for, as long as the data types and buffer key names
are set correctly. The difference between listen and buffer access is that
**the buffer access operation does not get triggered when buffer values are changed**.
It only gets triggered when an input value is passed to it.

> [!TIP]
> To learn how to use an accessor within your service, see
> [Using an Accessor](./using_an_accessor.md).

[BufferAccess]: https://docs.rs/crossflow/latest/crossflow/builder/struct.Builder.html#method.create_buffer_access
