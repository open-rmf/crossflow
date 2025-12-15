# Scopes

Every time a workflow is run, a **session** is started. That session is contained
within some **scope**. Each scope has a **start** operation (green play symbol)
and a **terminate** operation (golden star).

![workflow-scope](./assets/figures/workflow-scope.svg)

## Start

The **start** operation has one simple purpose: deliver the user's request message
into this session. Each time a new session starts, exactly one message will be
produced by the start operation, and it will be whatever request message started
this run of the workflow. The start operation will never output a second message
for the same session.

## Terminate

Naturally the **terminate** operation serves the opposite purpose of the start
operation. The terminate operation has an input slot and no output slots, so it
will never output a message within its own scope. Instead it takes the first
message that gets passed to it for each session and then sets that as the final
response of that workflow session. Any additional messages passed to it for the
same session will simply be discarded.

Immediately after the terminate operation is activated, it will trigger the
[scope cleanup](./scope-cleanup.md) to begin. This ensures that all activity
happening inside the workflow is brought to a clean and complete finish before
the outside world is told that the workflow is done.

As soon as the cleanup process is finished, the ***first*** output that was passed to
the terminate operation will be sent out as the ***final*** output of the workflow
session.

