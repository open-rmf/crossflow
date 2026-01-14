# Channels

Buffers are a useful to encode the synchronization of workflow activities in the
structure of the workflow itself. However sometimes a deeper coupling between
services is needed.

Recall that "messages" that are passed between operations in a workflow can be
**any data type** that can be safely moved between threads. This includes endpoints
of [channels][channels]---e.g. [`Sender`][Sender] and [`Receiver`][Receiver]. By
streaming out a `Receiver` as a message, the service that gets the `Receiver` can
listen to ongoing activity in your service while both services run in parallel.
Or by streaming out a `Sender`, the service that gets the `Sender` can send data
back to your service while both services run in parallel.

![channel-receiver](./assets/figures/channel-receiver.svg)

Suppose we have **trajectory controller** service and a **motor controller** service in
a workflow. The trajectory controller might update at a frequency of 50Hz while
the motor controller updates at a frequency of 1000Hz. There isn't a natural way
for the structure of the workflow itself to synchronize these services, but the
trajectory controller could send a `Receiver` to the motor controller service,
allowing the motor controller to run its feedback loop at its own rate and
receive new targets as they arrive.

We use a [stream](./output-streams.md) to send out the receiver because streams
can send messages out of the service while the service continues to run. This
means the `trajectory_controller` service and `motor_controller` service can run
simultaneously as async services, and those specific service sessions can
communicate via the channel of the `Receiver`.

This can also be used to draw data into a running service. Suppose we want to
consider environmental hazards that should alter the path of the trajectory or
maybe even bring the robot to a stop. We could introduce a `safety_monitor`
service:

![channel-sender](./assets/figures/channel-sender.svg)

The `trajectory_controller` can send out a [`Sender`][Sender] over a second
output stream. This sender is used by the `safety_monitor` service to feed
moving obstacle information back into the `trajectory_controller`. Rather than
outright killing the `trajectory_controller` service to respond to obstacles,
we can feed the service with the information it needs to respond to obstacles.

> [!CAUTION]
> Channels are a powerful way to set up long-running one-way or two-way
> communication between async services in a workflow that run in parallel, but
> there is a notable drawback. If you want to visualize the execution of the
> workflow, **data sent between services over channels will not be traceable**.
> If you care about traceability, you should consider copying all channel data to
> an output stream for logging.

[channels]: https://doc.rust-lang.org/rust-by-example/std_misc/channels.html
[Sender]: https://docs.rs/tokio/latest/tokio/sync/mpsc/struct.Sender.html
[Receiver]: https://docs.rs/tokio/latest/tokio/sync/mpsc/struct.Receiver.html
