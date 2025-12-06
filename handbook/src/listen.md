# Listen

While [join](./join.md) can handle basic synchronization between branches, sometimes
the buffers need to be managed with more nuanced logic than simply pulling out
their oldest values. This is especially true if your workflow expresses a state
machine where different state transitions may need to take place based on the
combined values of multiple buffers.

Suppose a vehicle is approaching an intersection with a traffic signal. While we
approach the intersection we'll monitor the traffic signal, streaming the latest
detection into a buffer. At the same time, the robot will approach the intersection.

![listen-stoplight](./assets/figures/listen-stoplight.svg)

Once the vehicle is close enough to the intersection, a decision must be made:
Should the vehicle stop before reaching the intersection, or just drive through
it? If the traffic signal is red, we will ask the vehicle to stop, but then once
the signal turns green we will need to tell the vehicle to proceed.

To express this in a workflow we create two buffers: `latest_signal` and `arriving`.
We create a listen operation (listener) that connects to both buffers. Every time
a change is made to either buffer, the listener will output a message containing
a [key][BufferKey] for each buffer. Those buffer keys allow a service to freely
[access][BufferAccess] the contents of the buffers and even make changes to the
contents of each.

Let's translate these requirements into how `proceed_or_stop` should manipulate
the buffers when activated under different circumstances:
* If the `arriving` buffer is empty then do nothing because the vehicle is not
  near the intersection yet (or has already passed the intersection).
* If the `arriving` buffer has a value and `latest_signal` is red, leave the `arriving`
  buffer alone and command the vehicle to come to a stop. By leaving the `arriving`
  buffer alone, we can continue to listen for `latest_signal` to turn green.
* If the `arriving` buffer has a value and `latest_signal` is green, drain the `arriving`
  buffer and command the vehicle to proceed. With the `arriving` buffer now empty,
  the listener will no longer react to any updates to `latest_signal`.
* *(Edge case)* If the `arriving` buffer has a value and `latest_signal` is empty,
  treat `latest_signal` as though it were red (come to a stop) to err on the
  side of caution.

If we had tried to use the [join](./join.md) operation for this logic, we would
have drained the `arriving` buffer the first time that both buffers had a value.
If the value in `latest_signal` were red then we would be prematurely emptying
the `arriving` buffer, and then we would no longer be waiting for the green traffic
signal.

> [!NOTE]
> A listen operation (listener) will be activated each time ***any one*** of the
> buffers connected to it gets modified. The listener will pass along [buffer keys][BufferKey]
> that allow services to read and write to those connected buffers. **Listeners
> will not be activated when a buffer is modified using one of the listener's own
> buffer keys.** This prevents infinite loops where a listener endlessly gets
> woken up by a downstream modification. You can choose to turn off this safety
> mechanism with [`allow_closed_loops`][allow_closed_loops].

## Multi-Agent State Machines

Listeners are excellent at multiplexing messages coming from many sources at once.
This makes them well equipped to manage state machines that involve multiple
independent agents that need to be orchestrated.

Suppose we have three robots:
1. A **machining robot** that takes raw material and cuts (machines) it into a
   particular shape.
2. A **painting robot** that takes machined material and applies paint to it.
3. A **tending robot** that moves material around the work zone, passing it all
   between the other machines and the supply areas.

![layout-multi-robot](./assets/figures/layout-multi-robot.svg)

We also have three supply areas within the work zone:
1. The **raw material supply** is where raw material gets dropped off to be processed.
2. The **machined material supply** is where machined material is kept until the
   painting robot is available to work on it.
3. The **finished material supply** is where painted (finished) material is placed
   until it can be taken away.

For the overall flow we want the *raw material* to be moved to the **machining robot**,
but only when the **machining robot** is available. We want *machined material*
to be moved to the **painting robot** but only when the **painting robot** is available,
otherwise it should be moved to the `machined material supply`. Finally *painted
material* should be moved to the `finished material supply`. Each time material
is moved to the **machining robot** or the **painting robot**, that robot should begin
working on the material that it received.

We can express that with the following workflow:

![listen-multi-robot](./assets/figures/listen-multi-robot.svg)

We define six buffers in total: one for each of the three robots and one for each
of the three material supplies. Each of these buffers can be thought of as
representing a variable in the multi-agent state machine. Each time a value changes
for one of these variables, a relevant service will be activated to make decisions
about what state transitions should take place:

### 1. send raw material for machining

Connected to the `raw_material_supply`, the `machining_robot_state`, and the
`tending_robot_state`, this service will be activated any time one of these
events happen:
* New material is added to `raw_material_supply`
* The **machining robot** becomes available
* The **tending robot** becomes available

Technically there are other events that can activate the service (e.g. the
tending robot begins a new task), but the service will exit early when evaluating
irrelevant events.

When any of the above events trigger the service, it will check if **all** of the
following conditions are met:
* Raw material is available in the `raw_material_supply`
* The **machining robot** is available
* The **tending robot** is available

When all conditions are met this service will:
* Claim the **tending robot** by setting the `tending_robot_state` buffer to *Busy*
* Claim the **machining robot** by setting the `machining_robot_state` buffer to *Busy*
* Begin this async routine:
  * Command the **tending robot** to pull an item from the raw material supply
  * Reduce the count in the `raw_material_supply` buffer
  * Place the item in the **machining robot** area
  * Release the **tending robot** by setting `tending_robot_state` buffer to available
  * Command the **machining robot** to perform its machining process
  * When the **machining robot** is finished, change the value in `machining_robot_state` to *Finished*

If any one of the conditions is not met, then the service will not do anything.

### 2. send machined material to painting area OR into queue for later

Connected to `machining_robot_state`, `machined_material_supply`, `painted_robot_state`,
and `tending_robot_state`, this service will be activated any time one of these
events happens:
* The **machining robot** finishes a job
* The **tending robot** becomes available
* The **painting robot** becomes available

Technically there are other events that can activate the service (e.g. the
machining robot or tending robot begins a new task), but the service will exit
early when evaluating irrelevant events.

When any of the above events trigger the service, it will check whether the
conditions are met for each of the following scenarios, in order, until it finds
a scenario that is satisfied. The first one satisfied will be executed and the
rest will be skipped:

#### 🠚 move material from **machining robot** to **painting robot**

Conditions:
* `machining_robot_state` is *Finished*
* `painting_robot_state` is *Available*
* `tending_robot_state` is *Available*

Execution:
* Claim the **tending robot** by setting `tending_robot_state` buffer to *Busy*
* Claim the **painting robot** by setting `painted_robot_state` buffer to *Busy*
* Begin this async routine:
  * Command the **tending robot** to pull the material from the **machining robot** area
  * Once the **machining robot** area is clear, set `machining_robot_state` to *Available*
  * Move the material to the **painting robot** area
  * Release the **tending robot** by setting `tending_robot_state` buffer to available
  * Command the **painting robot** to perform its painting process
  * When the **painting robot** is finished, change the value in `painting_robot_state` to *Finished*

This transition has the benefit of efficiently making the machining robot available
and beginning the painting in one fell swoop, skipping the intermediate machined
material supply.

#### 🠚 move material from **machining robot** to the `machined_material_supply`

If the first scenario was skipped then either the **painting robot** is busy or
the **machining robot** had no material available. We should prioritize clearing
the **machining robot** so more material can be machined as soon as possible.

Conditions:
* `machining_robot_state` is *Finished*
* `machined_material_supply` has capacity for more items
* `tending_robot_state` is *Available*

Execution:
* Claim the **tending robot** by setting `tending_robot_state` buffer to *Busy*
* Begin this async routine:
  * Command the **tending robot** to pull the material from the machining robot area
  * Once the **machining robot** area is clear, set `machining_robot_state` to *Available*
  * Move the material to the *machined material* area
  * Increment the `machined_material_supply` value by one
  * Release the **tending robot** by setting `tending_robot_state` buffer to *Available*

#### 🠚 move material from `machined_material_supply` to **painting robot**

If the first two scenarios were skipped then we cannot clear out the **machining
robot** at this time. Now we should check if we can move any *machined material*
from the `machined_material_supply` to the **painting robot**.

Conditions:
* `machined_material_supply` is greater than zero
* `painting_robot_state` is *Available*
* `tending_robot_state` is *Available*

Execution:
* Claim the **tending robot** by setting `tending_robot_state` buffer to *Busy*
* Claim the **painting robot** by setting `painted_robot_state` buffer to *Busy*
* Begin this async routine:
  * Command the **tending robot** to pull material from the `machined_material_supply`
  * Once the material is retrieved, decrement the value in the `machined_material_supply` buffer
  * Move the material to the **painting robot** area
  * Release the **tending robot** by setting `tending_robot_state` buffer to available
  * Command the **painting robot** to perform its painting process
  * When the **painting robot** is finished, change the value in `painting_robot_state` to *Finished*

#### 🠚 else

If none of the above scenarios are satisfied, then this service will do nothing.

### 3. move finished material to pickup area

The final service is responsible for clearing material from the **painting robot**.
Connected to `painting_robot_state`, `finished_material_supply`, and `tending_robot_state`,
this service will be activated any time one of these events happens:

* The **painting robot** finishes a job
* Material is taken from the `finished_material_supply`
* The **tending robot** becomes available

Technically there are other events that can activate the service (e.g. the
tending robot begins a new task), but the service will exit early when evaluating
irrelevant events.

When any of the above events triggers the service, it will check if **all** of the
following conditions are met:
* `finished_material_supply` has capacity for more items
* `painting_robot_state` is *Finished*
* `tending_robot_state` is *Available*

When all conditions are met this service will:
* Claim the **tending robot** by setting the `tending_robot_state` buffer to *Busy*
* Begin this async routine:
  * Command **tending robot** to pull material from the **painting robot** area
  * Once the **painting robot** area is clear, set `painting_robot_state` to *Available*
  * Move the material to the `finished_material_supply`
  * Increment the `finished_material_supply` value by one
  * Release the **tending robot** by setting `tending_robot_state` buffer to *Available*

### Conclusion

The above workflow defines a highly parallelized process involving three agents
that opportunistically keeps material moving whenever the right agents are
available. Underlying this workflow is a state machine that combines the state
information of all three agents and three material supply areas. Transitions for
this state machine are asynchronous and can overlap with each other without
negatively interfering with each other because their activities are orchestrated
through their shared use of the buffers.

The logic of each of the state transitions that can be performed are encapsulated
by three different services that do not need to know about each other's existence.
Each of these services can be implemented as an [async function](./spawn_async_service.md)
*or* by defining a separate lower-level workflow for each of them.

This kind of free-form reactive async system cannot be expressed by most graphical
programming paradigms. Behavior Trees generally cannot express this kind of
open-ended reactivity, and even most Petri Net implementations cannot express async
transitions that gradually modify state variables ("places" in Petri Net terminology)
throughout the execution of the transition.


[BufferKey]: https://docs.rs/crossflow/latest/crossflow/buffer/struct.BufferKey.html
[BufferAccess]: https://docs.rs/crossflow/latest/crossflow/buffer/struct.BufferAccess.html
[allow_closed_loops]: https://docs.rs/crossflow/latest/crossflow/buffer/struct.BufferMut.html#method.allow_closed_loops
