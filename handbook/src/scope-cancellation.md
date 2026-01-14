# Cancellation

There are times where it becomes impossible for a workflow to successfully
terminate, or where a condition is met that requires the workflow to stop before
it can successfully terminate. The potential causes of a cancellation are
enumerated in [`CancellationCause`][CancellationCause].

When a workflow is cancelled, the caller will never receive a `Response` message,
instead the [`Promise`][Promise] they hold will contain a [`PromiseState::Cancelled`][PromiseState]
value. Inside will be a [`CancellationCause`][CancellationCause] to give some
information on why the cancellation happened.

There are a few potential causes of cancellation that are worth being mindful of:
* [Unreachability](./reachability.md) - The terminate operation is no longer
  reachable. You will receive a list of the [disposals][disposals] that happened
  during the session which led to the unreachability.
* [Triggered Cancellation][TriggeredCancellation] - A cancel operation was
  explicitly triggered by the workflow itself. You might receive a
  [string that serializes the message][TriggeredCancellationValue] that triggered
  the cancellation from inside the workflow.
* [Filtered][Filtered] - The condition of a filtering operation failed to pass,
  so the workflow was cancelled.

### Disposal

When a scope nested inside of another scope gets cancelled, the parent scope
will see that as a ***disposal***, meaning the node will simply never yield a
final message for that session of the nested scope.

> [!TIP]
> **It is generally discouraged to use cancellation in the happy path of your
> workflow.** The cancellation data received by the caller is meant for debugging
> purposes, not to be used as regular service output. Instead if your workflow
> is known to be fallible it should return a [`Result`][Result] type.

In crossflow, disposals are something that are managed automatically. Currently
there is no operation for users to explicitly react to a disposal, so when the
service of a node gets cancelled, this is generally invisible to the parent
workflow until it escalates into a cancellation itself. The [collect](./collect.md#catching-unreachability)
operation can be used to catch unreachability, but there may be significant
information lost by the time the workflow reaches that point. Therefore it is
best practice to return [`Result`][Result] types for fallible workflows instead of having
them cancel.

[CancellationCause]: https://docs.rs/crossflow/latest/crossflow/cancel/enum.CancellationCause.html
[Promise]: https://docs.rs/crossflow/latest/crossflow/promise/struct.Promise.html
[PromiseState]: https://docs.rs/crossflow/latest/crossflow/promise/enum.PromiseState.html
[disposals]: https://docs.rs/crossflow/latest/crossflow/cancel/struct.Unreachability.html#structfield.disposals
[TriggeredCancellation]: https://docs.rs/crossflow/latest/crossflow/cancel/struct.TriggeredCancellation.html
[TriggeredCancellationValue]: https://docs.rs/crossflow/latest/crossflow/cancel/struct.TriggeredCancellation.html#structfield.value
[Filtered]: https://docs.rs/crossflow/latest/crossflow/disposal/struct.Filtered.html
[Result]: https://doc.rust-lang.org/std/result/
