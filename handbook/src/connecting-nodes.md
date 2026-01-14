# Connecting Nodes

The examples on the [previous page](./creating-a-node.md) are completely trivial workflows.
In each one we're just starting the workflow, running one service (or [provider](./provider-overview.md)), and then terminating the workflow with the output of the service.
The real value of a workflow is being able to assemble multiple services together into something more complex.

Here is an example of creating two nodes and chaining them:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:basic_connect_nodes}}
```

A common pattern when building workflows is to declare the nodes or operations at the top and then connect them together below.

The above workflow still doesn't accomplish anything that we couldn't get from running a [Series](./run-series.md) instead.
It's just a sequence of actions that feed into each other.
What makes workflows interesting is their ability to branch, cycle, and flow freely through a directed graph structure.

### Conditional Branching

One useful kind of control flow is [conditional branching](./branching.md).
Conditional branching is when the activity in a workflow reaches a "[fork in the road][fork-in-the-road]"
where a message must go down one branch or another but not both.

#### fork-result

Commonly used for error handling in workflows, the fork-result operation will take in [`Result`][Result] messages and activate one of two branches---
the `ok` branch or the `err` branch---depending on whether the input message had an [`Ok`][Ok] or [`Err`][Err] value.

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:fork_result_workflow}}
```

Note how in this example both branches converge back to the same terminate operation.

#### fork-option

Another common branching operation is fork-option.
Similar to fork-result, this creates two branches.
One sends the value contained inside `Some` inputs while the other will produce a [trigger](./branching.md#trigger) `()` when the input value was `None`.

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:fork_option_workflow}}
```

### Parallel Branches

Some kinds of forks result in [parallel](./parallelism.md) activity instead of conditionally activating branches.
This is one of the most powerful features of workflows: being able to easily juggle large amounts of parallel activity.

#### racing

The [fork-clone](./parallelism.md#clone) operation allows a message to be cloned and simultaneously activate multiple branches.
Once activated, each branch will run independently and concurrently.
This is useful if you have multiple separate concerns that need to be handled simultaneously.

Here is an example of a pick-and-place workflow where we have a parallel node that monitors the safety of the operation.
While the pick-and-place sequence is being executed, the `emergency_stop` service will watch the state of the workcell and issue an output if anything threatens the safety of the operation.
The pick-and-place operation and the emergency stop both connect to the [terminate](./scopes.md#terminate) operation.
Whichever yields an output first will end the workflow. This is known as a [race](./scope-operation.md#racing).

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:emergency_stop_workflow}}
```

#### joining

Another way to use fork-clone is to activate parallel branches that each control a different agent.
This is often done when a process needs two agents to work independently until they both reach a synchronization point is reached.
In that case instead of racing the two branches we will [join](./join.md) them.

Here is an example of a robot that needs to use an elevator.
The robot will start moving to the elevator lobby, and at the same time we will run a branch that watches the robot's progress.
When the robot is close enough to the elevator lobby, we will summon the elevator to come pick up the robot.
When the robot and elevator both arrive in the elevator lobby, we will have the robot use the elevator.

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:use_elevator_workflow}}
```

#### unzipping

It's not often that a message can be trivially fed directly from the output of one service to the input of another service.
Most of the time the services that you want to connect to each other will have slight differences between the `Response` type that the first produces and the `Request` type that you're trying to pass it along to.
If you have two or more parallel branches that expect different inputs from each other, fork-clone might not seem like a good fit because every branch will receive the same message.

We've seen how blocking maps can be used to perform quick data transforms.
If we combine a blocking map with the [unzip operation](./parallelism.md#unzip), we can perform parallel branching where a specific message is sent down each branch:

```rust,no_run,noplayground
{{#include ./examples/handbook_snippets/src/native-snippets.rs:unzip_workflow}}
```


[Result]: https://doc.rust-lang.org/std/result/
[Ok]: https://doc.rust-lang.org/std/result/enum.Result.html#variant.Ok
[Err]: https://doc.rust-lang.org/std/result/enum.Result.html#variant.Err
[fork-in-the-road]: https://en.wiktionary.org/wiki/fork_in_the_road
