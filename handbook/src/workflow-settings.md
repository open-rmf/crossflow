# Workflow Setings

In general, services allow you to specify [delivery settings](./delivery_instructions.md) which affect whether the service can run in parallel across one or more sessions, or whether the service can only be run once at a time across all sessions.
There are also [scope settings][ScopeSettings] which determine whether a given scope is "interruptible", meaning it cannot be cancelled from the outside---only an internal cancellation or successful termination can end the scope session.

Both of these settings are relevant to a workflow.
A workflow is ultimately a service, and therefore supports delivery settings.
A workflow has a root scope, and that root scope can have scope settings.
Both of these types of settings are bundled into [`WorkflowSettings`][WorkflowSettings].

For blocking, async, and continuous services, you would set delivery instructions via the [`ServiceBuilder`][ServiceBuilder] API, which allows you to chain `.serial()` or `.parallel()` onto the service name while spawning it.
Instead of this chaining approach, workflows allow you to specify their settings by returning one of these from the closure that you use to spawn the workflow:
* [`WorkflowSettings`][WorkflowSettings]: Specify all the workflow settings that you want.
* [`DeliverySettings`][DeliverySettings]: Specify the delivery settings and use the default scope settings (interruptible).
* [`ScopeSettings`][ScopeSettings]: Specify the scope settings and use the default delivery settings (parallel).
* `()`: Use the default delivery settings (parallel) and scope settings (interruptible).
  This is what your closure will return if you don't explicitly return anything.

Here are examples of each:

#### `WorkflowSettings`

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:explicit_workflow_settings}}
```

#### `DeliverySettings`

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:explicit_delivery_settings}}
```

#### `ScopeSettings`

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:explicit_scope_settings}}
```

#### Default

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:default_workflow_settings}}
```

## Inner Scope

When you use the scope operation inside of a workflow, that inner scope can have its own scope settings, independent from the rest of the workflow.
This allows you to set specific clusters of operations as uninterruptible.

```rust,no_run,noplayground
{{#include ./examples/native/src/handbook_snippets.rs:inner_scope_settings}}
```


[ScopeSettings]: https://docs.rs/crossflow/latest/crossflow/workflow/struct.ScopeSettings.html
[WorkflowSettings]: https://docs.rs/crossflow/latest/crossflow/workflow/struct.WorkflowSettings.html
[ServiceBuilder]: https://docs.rs/crossflow/latest/crossflow/service/struct.ServiceBuilder.html
[DeliverySettings]: https://docs.rs/crossflow/latest/crossflow/workflow/enum.DeliverySettings.html
