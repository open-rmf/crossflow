# How to Run a Service

Once you have [spawned a service](./spawn_a_service.md) or have some other type
of "provider" available [[1](./callbacks.md)][[2](./maps.md)][[3](./make_a_workflow.md)],
you can run it by passing in a request. This can be done from inside of any Bevy
system by including a [`Commands`](https://docs.rs/bevy/latest/bevy/prelude/struct.Commands.html):

```rust
let response = commands.request(request_msg, service).take_response();
```

The `.request(_, _)` method comes from the [`RequestExt`](https://docs.rs/crossflow/latest/crossflow/request/trait.RequestExt.html#tymethod.request) trait provided by crossflow. This method takes in a `request_msg` (the input message for the service) and any type of "provider", which is usually a [`Service`](https://docs.rs/crossflow/latest/crossflow/service/struct.Service.html).

The simplest thing to do with a request is to take the response using `.take_response()`.
This will provide you with a [`Promise`](https://docs.rs/crossflow/latest/crossflow/promise/struct.Promise.html)
which you can use to receive the response of the service once it finishes. You
can use a promise in a regular (blocking, non-async) function using
[`peek`](https://docs.rs/crossflow/latest/crossflow/promise/struct.Promise.html#method.peek):

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:request_service_example}}
```
