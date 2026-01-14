# Delivery Instructions

A common problem with highly parallelized async systems is when multiple instances of one service are active at the same time, but the service is designed for exclusive usage.
This can happen when a service interacts with global resources like a mouse cursor or physical assets like the joints of a robot.
If multiple conflicting requests are being made for the same set of resources, there could be deadlocks, misbehaviors, or even critical failures.

Resource contention is a broad and challenging problem with no silver bullet solution, but crossflow offers a **delivery instructions** mechanism that can help to handle simple cases.
With the help of delivery instructions, you can implement your service to not worry about exclusive access, and then wrap the service in instructions that tell crossflow to only execute the service once-at-a-time.

More advanced use of delivery instructions allow you to have specific sets of requests for a service run in serial while others run in parallel at the same time.
You can have specific queues where requests in the same queue run in serial while the queues themselves run in parallel.

> [!TIP]
> Delivery instructions are only relevant for async and continuous services.
> They have no effect on blocking services since blocking services always run in serial no matter what.

### Always-Serial Services

> [!WARNING]
> Always-serial services are also serial across different [workflows](./introduction-to-workflows.md) and different [sessions](./scopes.md) of the same workflow.
> When a service gets used in multiple different nodes, those nodes will only execute once-at-a-time, even if the nodes belong to different workflows or are being triggered across different sessions.

You can choose for an async or continuous service to always be run in serial, meaning when multiple requests are sent to it, they will always be run once-at-a-time.
This does not block any other services from running at the same time.

To make any service always run serially, just add `.serial()` while spawning it:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:always_serial_example}}
```

> [!NOTE]
> Apply `.serial()` to the name of the service itself, inside the parenthesis of `.spawn_service(_)`.
> You **cannot** chain it outside the parentheses of `.spawn_service(_)`.

### Delivery Label

Always-serial is often too restrictive.
When a service is used across different workflows or in multiple nodes at the same time, the requests being sent to it might be accessing unrelated resources.
In this case, you can assign delivery instructions to the service before making the request.

The first step is to create a delivery label:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:delivery_label}}
```

A delivery label can be any [`struct`] that implements `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`, and `DeliveryLabel`.
The `#[derive(DeliveryLabel)]` will fail at compile time if any of the required traits are missing, so the compiler will help ensure that your struct has all the traits it needs.
Other than those traits, there is no restriction on what can be used as a delivery label.

To use a delivery label, spawn a service as normal, but apply instructions with `.instruct(_)` when passing it into a request:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:set_instructions}}
```

To create the [`DeliveryInstructions`], you must pass in an instance of your delivery label.

Every request made with a matching delivery label value will be put into the same queue and run in serial with each other.
Requests made with different delivery labels will be put in separate queues which can run in parallel.

### Pre-empt

Sometimes waiting for a service to execute once-at-a-time isn't really what you want.
When multiple requests are contending for the same resource, sometimes what you really want is to discard the earlier requests so the newest one can take over.

We call this pre-emption, and provide it as an option for [`DeliveryInstructions`]:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:preempt_example}}
```

When [`.preempt()`] is applied to [`DeliveryInstructions`], that request will discard all requests that were queued up before it.
Even a live task that is in the process of being executed will be cancelled when a pre-empting request arrives.

> [!NOTE]
> Earlier we said that delivery instructions don't have an effect on blocking services, but this is not true in the case of pre-emption.
> If multiple requests with the same delivery label have gotten queued up for a blocking service before it's had a chance to flush, a pre-empting request will clear out the earlier requests before they can ever execute.

> [!WARNING]
> When a service being used by a workflow gets pre-empted, that will result in a [disposal](./scope-cancellation.md#disposal).

### Ensure

Sometimes pre-emption may be too much of a scorched-earth approach.
If you have some requests that should be pre-empted while others are too crucial to pre-empt, you can apply [`.ensure()`]:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:ensure_example}}
```

> [!TIP]
> [`.preempt()`] and [`.ensure()`] can be used independently of each other per request.
> A request can be pre-emptive but not ensured, or ensured but not pre-emptive, or it can be both pre-emptive and ensured.


### Full Example

```rust,no_run,noplayground
{{#include ./examples/native/src/delivery_instructions.rs:full_example}}
```

[`struct`]: https://doc.rust-lang.org/book/ch05-01-defining-structs.html
[`DeliveryInstructions`]: https://docs.rs/crossflow/latest/crossflow/service/struct.DeliveryInstructions.html
[`.preempt()`]: https://docs.rs/crossflow/latest/crossflow/service/struct.DeliveryInstructions.html#method.preempt
[`.ensure()`]: https://docs.rs/crossflow/latest/crossflow/service/struct.DeliveryInstructions.html#method.ensure

> [!TIP]
> To set delivery instructions for the services used in nodes, simply apply `.instruct(_)` to the service before passing it to [`builder.create_node(_)`](./creating-a-node.md)
