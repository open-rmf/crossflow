# Middlewares

When using the native Rust API of crossflow, it's straightforward to tap into whatever middlewares you need, as long as a Rust library exists for it.
Just add the relevant library to your `Cargo.toml` and spawn a service or callback that uses the middleware's API.

For JSON diagrams, you'll need your executor to register node builders for each middleware that you want to use.
Registering node builders that can provide comprehensive coverage of all of a middleware's capabilities is a daunting task, so we provide out-of-the-box support for the middlewares that we anticipate will be most useful for our community of users.

The number of middlewares that crossflow provides out-of-the-box support will grow over time as our user base grows.
Moreover, downstream users are always welcome to write third-party libraries that register node builders for any middlewares we haven't covered yet.
Our out-of-the-box support uses public APIs of crossflow, so there would be no difference between first-party and third-party support.

Support for these middlewares is not turned on by default.
We use Rust's [feature system] to allow downstream users to toggle the support on or off.
Since each middleware brings a potentially large number of dependencies with it, making the features opt-in will spare users from taking on a large volume of depenencies that they don't need.

Here are the names of the features for activing various middlewares:
* [`grpc`](./diagram-grpc.md) - uses [tonic] and [prost-reflect] to provide clients for dynamically loaded [gRPC] services.
* [`zenoh`](./diagram-zenoh.md) - provides full support for [zenoh] publishers, subscribers, and queriers. Payloads can be [protobuf] via [prost-reflect] or JSON strings.
* [`ros2`](./diagram-ros2.md) (only on the [`ros2` branch]) - provides registration helpers for ROS 2 subscriptions, publishers, service clients, and action clients via [rclrs].
  For now each message/service/action type needs to be compiled in, but this requirement will relax when [rclrs] supports runtime loading of message definitions.

[feature system]: https://doc.rust-lang.org/cargo/reference/features.html
[tonic]: https://docs.rs/tonic/latest/tonic/
[prost-reflect]: https://docs.rs/prost-reflect/latest/prost_reflect/
[zenoh]: https://docs.rs/zenoh/latest/zenoh/
[gRPC]: https://grpc.io/
[protobuf]: https://protobuf.dev/
[`ros2` branch]: https://github.com/open-rmf/crossflow/tree/ros2
[rclrs]: https://github.com/ros2-rust/ros2_rust
