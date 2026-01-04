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



[gRPC]: https://grpc.io/
[tonic]: https://docs.rs/tonic/latest/tonic/
[prost-reflect]: https://docs.rs/prost-reflect/latest/prost_reflect/
[protobuf]: https://protobuf.dev/
