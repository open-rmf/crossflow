# gRPC

Support for [gRPC] is implemented using
* [tonic] for client/service communication
* [prost-reflect] for handling dynamic [protobufs][protobuf] and serialization/deserialization

Use the `grpc` feature flag of crossflow to enable this support:

```toml
# Cargo.toml

[dependencies]
crossflow = { version = "*", features = ["diagram", "grpc"] }
```

### Enabling gRPC

Use [`registry.enable_grpc(rt)`] to register premade node builders that can build gRPC clients into a workflow:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/diagram-snippets.rs:grpc_demo}}
```

##### Descriptor Pool

Additionally you will need to use [`decode_global_file_descriptor_set`] to add your proto definitions to the [global descriptor pool].
The node builders will look for service and protobuf definitions in the global descriptor pool.
If a description isn't present in the pool, the node builders will return a [`NodeBuildingError`] when you attempt to build a workflow from the diagram.

We leave it open-ended how descriptors are sent to the executor.
There are many viable ways to do this using various middlewares or compiling the definitions into the executor itself.
Users are welcome to come up with reusable third-party libraries to implement specific approaches and share them with the ecosystem.

### Configuration

Regardless of client type, all gRPC nodes are configured using [`GrpcConfig`].
That config allows you to specify the [service][grpc-service] and method to call, as well as the URI of where the service should be found.
You can optionally specify a timeout that will have the client quit if the response does not arrive by then.

As mentioned [above](#descriptor-pool) whatever service type that you configure your node to use needs to be present within the executor's global descriptor pool before you attempt to build a workflow from the diagram.

### Unary and Server-Streaming Requests

[Unary] and [server-streaming] requests are both covered by the same node builder: `"grcp_request"`.

**Request** --- The `grpc_request` nodes take in a single [`JsonMessage`] which will be used as the single request message sent to the server.
Each [`JsonMessage`] sent in activates a new gRPC client←→server session where the server will receive this single request.
The gRPC client itself is instantiated when the workflow is originally built, and will be reused for all requests sent into this node.
Separate sessions of the workflow will also use the same gRPC client whenever the same node is activated.

**"out"** --- The response messages received from the gRPC server will be sent out of the stream named `"out"`.
This is the case whether the service is unary or server-streaming.
This allows the node to have a single consistent structure regardless of whether the server ends up sending zero, one, or arbitrarily many response messages.

**Response** --- The final response of the node will be a [`Result`] whose value is `Ok` after the server has ended the connection with no errors, or `Err` if an error came up during the request.
The `Err` will contain a string rendering of a [`Status`], based on [gRPC status codes].

**"canceller"** --- In case you want to cancel the gRPC request, you can capture this [`UnboundedSender`], either storing it in a buffer or passing it to a node that can make a decision about when to cancel.
Passing in a `Some(String)` will have the string included inside the string-rendered [`Status`] of the final response's `Err`.

![grpc_request-node](./assets/figures/grpc_request-node.svg)

### Bidirectional and Client-Streaming Requests

[Bidirectional] and [client-streaming] requests are both covered by the `"grpc_client"` node builder.
Technically this node builder can also support unary and server-streaming requests, but the ["grpc_request"](#unary-and-server-streaming-requests) node builder is more ergonomic for those.

What makes `"grpc_client"` different is it takes in an [`UnboundedReceiver`] of [`JsonMessage`] instead of just a single [`JsonMessage`].
This allows any number of messages to be streamed from the client (inside the workflow) out to the server within a single gRPC client←→server session.

Each [`UnboundedReceiver`] sent into this node will active a new independent gRPC client←→server session using the same client.
Every message sent through the [`UnboundedSender`] associated with that receiver will be streamed out over the same gRPC client←→server session.

![grpc_request-node](./assets/figures/grpc_client-node.svg)

[gRPC]: https://grpc.io/
[tonic]: https://docs.rs/tonic/latest/tonic/
[prost-reflect]: https://docs.rs/prost-reflect/latest/prost_reflect/
[`decode_global_file_descriptor_set`]: https://docs.rs/prost-reflect/latest/prost_reflect/struct.DescriptorPool.html#method.decode_global_file_descriptor_set
[global descriptor pool]: https://docs.rs/prost-reflect/latest/prost_reflect/struct.DescriptorPool.html#method.global
[protobuf]: https://protobuf.dev/
[`registry.enable_grpc(rt)`]: https://docs.rs/crossflow/latest/crossflow/diagram/struct.DiagramElementRegistry.html#method.enable_grpc
[unary]: https://grpc.io/docs/what-is-grpc/core-concepts/#unary-rpc
[server-streaming]: https://grpc.io/docs/what-is-grpc/core-concepts/#server-streaming-rpc
[client-streaming]: https://grpc.io/docs/what-is-grpc/core-concepts/#client-streaming-rpc
[Bidirectional]: https://grpc.io/docs/what-is-grpc/core-concepts/#bidirectional-streaming-rpc
[`GrpcConfig`]: https://docs.rs/crossflow/latest/crossflow/diagram/grpc/struct.GrpcConfig.html
[grpc-service]: https://grpc.io/docs/what-is-grpc/core-concepts/#service-definition
[`JsonMessage`]: https://docs.rs/crossflow/latest/crossflow/buffer/enum.JsonMessage.html
[`NodeBuildingError`]: https://docs.rs/crossflow/latest/crossflow/diagram/enum.DiagramErrorCode.html#variant.NodeBuildingError
[`Result`]: https://doc.rust-lang.org/std/result/
[`UnboundedSender`]: https://docs.rs/tokio/latest/tokio/sync/mpsc/struct.UnboundedSender.html
[`UnboundedReceiver`]: https://docs.rs/tokio/latest/tokio/sync/mpsc/struct.UnboundedReceiver.html
[`Status`]: https://docs.rs/tonic/latest/tonic/struct.Status.html
[gRPC status codes]: https://grpc.io/docs/guides/status-codes/
