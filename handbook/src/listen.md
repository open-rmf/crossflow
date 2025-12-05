# Listen

While [join](./join.md) can handle basic synchronization between branches, sometimes
the buffers need to be managed with more nuanced logic than simply pulling out
their oldest values. This is especially true if your workflow expresses a state
machine where different state transitions may need to take place based on the
combined values of multiple buffers.

Suppose a vehicle is approaching an intersection with a traffic signal. While we
approach the intersection we'll monitor the traffic signal, streaming the latest
detection into a buffer. At the same time, the robot will approach the intersection.

![listen-stoplight](./assets/figures/listen-stoplight.svg)

Once the vehicle is close enough to the intersection, a decision must be made:
Should the vehicle stop before reaching the intersection or drive through it?
If the traffic signal is red, we will ask it to stop, but then once it turns
green we will need to tell the vehicle to proceed.

To express this in a workflow we create two buffers: `latest_signal` and `arriving`.
We create a listen operation (listener) that connects to both buffers. Every time
a change is made to either buffer, the listener will output a message containing
a [key][BufferKey] for each buffer. Those buffer keys allow a service to freely
[access][BufferAccess] the contents of the buffers and even make changes to the
contents of each.

With that in mind, let's translate this into how the buffers should be manipulated
under different circumstances:
* If the `arriving` buffer is empty then do nothing because the vehicle is not ready yet.
* If the `arriving` buffer has a value and `latest_signal` is red, leave the `arriving`
  buffer alone and signal the vehicle to come to a stop. Continue listening for
  `latest_signal` to turn green.
* If the `arriving` buffer has a value and `latest_signal` is green, drain the `arriving`
  buffer and signal the vehicle to proceed. With the `arriving` buffer now empty,
  the listener will no longer react to any updates to `latest_signal`.
* (Edge case) If the `arriving` buffer has a value and `latest_signal` is empty,
  treat `latest_signal` as though it were red (come to a stop) to be cautious.

If we had tried to use the [join](./join.md) operation for this logic, we would
have drained the `arriving` buffer the first time that both buffers had a value.
If the value in `latest_signal` were red then we would be prematurely emptying
the `arriving` buffer, and then we would no longer be waiting for the green traffic
signal.

## Multi-Agent State Machine

![layout-multi-robot](./assets/figures/layout-multi-robot.svg)

![listen-multi-robot](./assets/figures/listen-multi-robot.svg)


[BufferKey]: https://docs.rs/crossflow/latest/crossflow/buffer/struct.BufferKey.html
[BufferAccess]: https://docs.rs/crossflow/latest/crossflow/buffer/struct.BufferAccess.html
