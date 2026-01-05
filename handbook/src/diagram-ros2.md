# ROS 2

Support for ROS 2 is implemented using [rclrs].
We provide generic registration functions that allow you to register node builders for individual messages, services, and actions.
Currently this means message, service, and action definitions need to be registered into the executor at compilation time.
This restriction will be lifted once `rclrs` supports dynamic messages.

Use the `ros2` feature flag of crossflow to enable this support:

```toml
# Cargo.toml

[dependencies]
crossflow = { version = "*", features = ["diagram", "ros2"] }
```

### Setup

> [!CAUTION]
> The ROS 2 support currently involves some complicated [colcon] setup and therefore is being kept to the [`ros2` branch] of crossflow for now.
> We should be able to merge it into `main` once `rclrs` supports dynamic messages.

Follow the instructions on the [README] of the `ros2` branch to set up a colcon workspace with the necessary dependencies.

> [!NOTE]
> This currently [uses forks] for `rosidl_rust` and `rosidl_runtime_rs` to have [`schemars`] support.
> There will be an effort to move this support upstream to the original repos.

### Registering Primitives

To register support for a specific message, service, or action, start by creating the node that will be used by the ROS 2 primitives.

Pass that node along to `registry.enable_ros2(node)`, which will provide an API that you can chain message, service, and action registrations onto.

<!-- TODO: Move this code into handbook_snippets once we merge the ros2 branch into main -->
```rust,no_run,noplayground
use crossflow::prelude::*;
use rclrs::*;

use nav_msgs::{
    msg::Path,
    srv::GetPlan,
    action::GetMap,
};

// Create an rclrs executor and node.
let context = Context::default_from_env().unwrap();
let mut executor = context.create_basic_executor();
let node = executor.create_node("crossflow_executor").unwrap();

// Use the node to register the specific message, service, and action types.
let mut registry = DiagramElementRegistry::new();
registry
    .enable_ros2(node)
    .register_ros2_message::<Path>()
    .register_ros2_service::<GetPlan>()
    .register_ros2_action::<GetMap>();

// Spin the executor on a separate thread since it needs to run alongside Bevy.
std::thread::spawn(move || {
    executor.spin(Default::default());
});
```

### Configuration

Subscriptions, publishers, and service clients are configured using [`PrimitiveOptions`] in JSON format.

Since action clients have multiple independent QoS to consider, they are configured using [`ActionClientConfig`] in JSON format.

In both cases, the topic/service/action name is the only mandatory field.
All the QoS settings are optional.
Any unset QoS will use the default setting for whatever primitive it's being applied to.

### Subscription

Subscriptions allow your workflow to receive messages from anonymous publishers that publish to the same topic that you've subscribed to.
Once the node gets activated, it will stream out any incoming messages until it gets cancelled or until the scope it's in finishes.

**Request** --- A simple [trigger](./branching.md#trigger) that just prompts the node to begin listening for messages and streaming them out.
Note that unlike the zenoh subscription it's possible to activate redundant instances of this node.
This issue is being tracked by [#158](https://github.com/open-rmf/crossflow/issues/158).

**"out"** --- Any messages received by subscription will be sent out of the `"out"` stream.
The message type will be the rosidl message struct that was registered rather than a [`JsonMessage`].

**"canceller"** --- In case you want to cancel the subscription, you can capture this [`UnboundedSender`], either storing it in a buffer or passing it to a node that can make a decision about when to cancel.
Triggering this will cause the final response of the node to be `Ok(msg)` where `msg` is whatever [`JsonMessage`] you pass into this sender.
Since subscriptions can last indefinitely, this is the only way to stop the node from running before the scope terminates.

**Response** ---  The final response of the node is a [`Result`] whose value is `Ok` if the subscription was cancelled using the `"canceller"`, or `Err` if an error occurred.

![ros2-subscription-node](./assets/figures/ros2-subscription-node.svg)

### Publisher

Publishers allow your workflow to send messages to anonymous subscribers whose topics match the topic of your publisher.

The ROS 2 publisher will be created when your workflow is created and reused across all sessions of the workflow.
Note that this has some potential side-effects.
If you configure the node to support late joiners then the late joiners will receive old messages even if the workflow session that sent those messages is no longer active.

**Request** --- Pass in a message of type `T` to publish, where `T` is the message type that the node builder is meant for.

**Response** --- A [`Result`] which will be `Ok` if publishing the message was successful, or `Err` if the message failed to publish.

![ros2-publisher-node](./assets/figures/ros2-publisher-node.svg)

### Client

Clients (for services) allow your workflow to send a request to a service and get back a response.
Clients and services have a 1-to-1 relationship with each other: For every one request you send, you get exactly one response back (in the absence of errors).

**Request** --- Pass in the `Srv::Request` type associated with the service `Srv` that was registered for this node builder.

**Response** --- A [`Result`] which will be `Ok` containing the `Srv::Response` from the server when the response gets delivered successfully.
If the canceller is triggered before the response arrives, this will be `Err` containing the cancellation message.
If any other error happens that interferes with the service, this will return an `Err(JsonMessage::String(_))` describing the error.

**"canceller"** --- Similar to the `"canceller"` for [subscriptions](#subscription), this allows the service to be cancelled before the response has finished arriving.

![ros2-service_client-node](./assets/figures/ros2-service_client-node.svg)

### Action Client

ROS 2 actions represent a combination of topics and services that altogether describe an "action" (usually a physical process) which takes place over a period of time and involves incremental updates while it makes progress.

Anyone unfamiliar with ROS 2 actions is encouraged to read through the [action tutorial].

**Request** --- Pass in the `A::Request` type associated with action `A` that was registered for this node builder.

**Response** --- A [`Result`] which will be `Ok` containing the `A::Response` and [`GoalStatusCode`] from the action server when the response gets delivered successfully.
If an error happens internally that makes the result undeliverable, a string message describing the error will be returned in `Err`.

**"feedback"** --- Stream of the action's feedback messages.

**"status"** --- Stream of the action's goal status updates.

**"canceller"** --- Send a request to cancel the action.
Unlike subscriptions and services, an action server gets notified about a cancellation and can respond to it accordingly.
Therefore triggering this does not immediately cancel the node.
Instead the cancellation request will be sent to the action server, and `"cancellation_response"` will stream out the response from the action server, along with the [`JsonMessage`] of the cancellation request that it's responding to.
Once the action is successfully cancelled by the action server, the result and status will be sent out of the node's final response wrapped in `Ok`.

**"cancellation_response"** --- Stream of the action server's responses to any cancellation requests sent to `"canceller"`.
The message that was passed into `"canceller"` will be included in the output.
If the action's communication is working correctly, there will be one message sent from this stream for every message sent through the `"canceller"` of an active action.

![ros2-action_client-node](./assets/figures/ros2-action_client-node.svg)

[colcon]: https://colcon.readthedocs.io/en/released/
[rclrs]: https://github.com/ros2-rust/ros2_rust
[`ros2` branch]: https://github.com/open-rmf/crossflow/tree/ros2
[README]: https://github.com/open-rmf/crossflow/blob/ros2/README.md
[uses forks]: https://github.com/open-rmf/crossflow/blob/3a0c92e21dfcd4e56ab533f6443881389e04ec4f/ros2-feature.repos#L30-L37
[`schemars`]: https://docs.rs/schemars/latest/schemars/
[`PrimitiveOptions`]: https://github.com/mxgrey/ros2_rust/blob/a8de85bdf50cea1467cfe1eed2ba08554ec2e21e/rclrs/src/node/primitive_options.rs#L17-L42
[`ActionClientConfig`]: https://github.com/open-rmf/crossflow/blob/3a0c92e21dfcd4e56ab533f6443881389e04ec4f/src/diagram/ros2.rs#L183-L197
[`JsonMessage`]: https://docs.rs/crossflow/latest/crossflow/buffer/enum.JsonMessage.html
[`Result`]: https://doc.rust-lang.org/std/result/
[`GoalStatusCode`]: https://github.com/ros2-rust/ros2_rust/blob/ed7bca63e7f5396582c8b881286f8b1d30e2c379/rclrs/src/action.rs#L173-L196
[action tutorial]: https://docs.ros.org/en/kilted/Tutorials/Beginner-CLI-Tools/Understanding-ROS2-Actions/Understanding-ROS2-Actions.html
