# Reflection

[Reflective programming](https://en.wikipedia.org/wiki/Reflective_programming)---also
referred to as *reflection*---is when a program can introspect and modify its own
structure or behavior. Crossflow does not currently support generalized reflection,
which would imply that a workflow could change its connections or add new nodes
and operations at runtime. However, it does support a few *reflective* operations
which are able to respond to the overall state of the workflow.

Most operations in crossflow are "localized", meaning they don't know anything
about the workflow that they are in, except for the immediate neighbors that
they are connected to. The reflective operations covered in this chapter have a
broader view of their parent workflow. They can be used to assess or modify the
execution of the workflow at runtime.

> [!NOTE]
> Theoretically it is possible to implement generalized reflection in crossflow.
> The main challenge is how to design an API that does not leave loose ends
> dangling while modifications are being made, or an API that can protect the
> user from unintuitive race conditions.

## Trim

> [!CAUTION]
> 🚧 Under Construction 🚧

## Gate

> [!CAUTION]
> 🚧 Under Construction 🚧

## Inject

> [!CAUTION]
> 🚧 Under Construction 🚧

## Collect

Collect was [already covered](./collect.md) under synchronization, but it can
also be considered a reflective operation. It creates a point in the workflow
where no further progress will be made until all upstream activity has finished.
