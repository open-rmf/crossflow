# Cycles

Cycles are commonly used in programming to define routines that need to repeat
until some condition is met. Similarly a workflow may need to contain a cycle of
operations if some subroutine needs to keep running until a condition is met.

![cycle](./assets/figures/cycle.svg)

**Cycles are not a special or specific operation in crossflow.** A cycle is simply
what happens when the output of an operation is connected to the input slot of
an operation upstream of it.

When building a workflow, you are free to connect an output to the input slot of
***any*** operation that expects a compatible message type. There are no
restrictions on how these connections are laid out, except that each output
connects to exactly one input slot---but each input slot can take in any number
of outputs, leaving an opening for downstream operations to connect their outputs
back upstream.
