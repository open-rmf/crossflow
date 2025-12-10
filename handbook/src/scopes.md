# Scopes

Every time a workflow is run, a **session** is started. That session is contained
within some **scope**. Each scope has a start operation (green play symbol) and
a terminate operation (golden star).

![workflow-scope](./assets/figures/workflow-scope.svg)

## Start

The start operation has one simple purpose: deliver the user's request message
into this session. Each time a new session starts, exactly one message will be
produced by the start operation, and it will be whatever request message started
this run of the workflow. The start operation will never output a second message
for the same session.

## Terminate

Naturally the terminate operation serves the opposite purpose of the start
operation. The terminate operation has an input slot and no output slots, so it
will never output a message within its own scope. Instead it takes the first
message that gets passed to it for each session and then sets that as the final
response of that workflow session. Any additional messages passed to it for the
same session will simply be discarded.

But terminate carries out one other important responsibility: scope cleanup.

### Operation Cleanup

There are many kinds of operations that can exist within a workflow. Once the
terminate operation is reached, we want to wind down that session as quickly as
possible to avoid doing useless work---once the final output of the workflow is
determined, no other work should matter in principle. This process of winding
down is called the operation cleanup. There are three different ways that
operations get cleaned depending on the kind of operation:

* **Blocking**: The input message storage of the operation is cleared out for
  this session. Even if the operation awakens, it will quit early when it sees
  that it has no more input messages.
* **Async**: The input message storage of the operation is cleared out, **and**
  any [Tasks][Task] that were spawned for this operation are [cancelled][cancel].
  The cleanup of this operation is considered finished when we are notified that
  the Future of the task was successfully dropped. At that point, there cannot
  be any side-effects that take place from the Future.
* **Continuous**: The order queue of the operation is cleared out for this
  session. The next time the service runs, it will no longer see any orders
  related to this session.
* **Workflow**: The input message storage of the operation is cleared out, **and**
  the inner workflow is sent a cancellation signal. Any uninterruptible scopes
  within the workflow will be brought to a finish, and cleanup will be performed
  on the workflow's operations and buffers. The cleanup of this operation is
  considered finished when we are notified that the inner workflow's cleanup has
  finished.

### Buffer Cleanup

> [!WARNING]
> At the time of this writing, buffer cleanup is not yet available for JSON
> diagrams. This is being tracked by [#59](https://github.com/open-rmf/crossflow/issues/59).

After operation cleanup is finished, there may still be data lingering in the
buffers for this session. Often it would be fine to just discard that data without
any further action, but sometimes the lingering buffer data is significant. Maybe
some buffer data represents ownership of a resource that needs to be released, or
contains an error that needs to be resolved before the session should end, or
maybe there is a sign-off that should be performed before dropping the whole
workflow session.

The buffer cleanup phase acts like a user-defined destructor for your workflow.
You can define any number of buffer cleanup workflows for your workflow---you
read that right, you can define workflows to clean up the data in the buffers of
your workflow.

The input message for each cleanup workflow is an [Accessor](./listen.md#accessor)
containing keys of buffers in the scope that is being cleaned up. You can choose
any in-scope buffers that you would like the Accessor to contain when you set the
cleanup workflow. You can also specify if each cleanup workflow should be run
only when the parent workflow was prematurely cancelled, successfully terminated,
or either.

> [!NOTE]
> You can use the same buffer across multiple cleanup workflows, but be mindful
> of how those separate workflows might interfere with each other. All the
> cleanup workflows will run in parallel.

As soon as operation cleanup is finished, all the buffer cleanup workflows will
be started at once, given access to whichever buffers they requested. The cleanup
workflows are allowed to have cleanup workflows themselves, and so can the cleanup
workflows of your cleanup workflows, etc. It is not possible to build a workflow
with infinitely recursive cleanup workflows, because the attempt to build such a
workflow would require infinite memory.

The buffer cleanup phase is finished once **all workflows** have terminated or
cancelled, including any inner cleanup workflows that they may contain. Any data
still lingering for this session in any of the buffers will simply be discarded.

### Final Output

Once all the cleanup is finished, the terminate operation will deliver the first
message it received to whoever started this session.

[Task]: https://docs.rs/bevy/latest/bevy/tasks/struct.Task.html
[cancel]: https://docs.rs/bevy/latest/bevy/tasks/struct.Task.html#method.cancel
