# Traffic app example

This is an example that enables users to build workflows via the diagram editor
and watch how their node connections result in different behaviors in a simple
traffic simulator. It is designed to support and demonstrate various `crossflow`
operations via the diagram editor.

## Basic workflow

Basic/utility nodes to get started:

| Node   | Use case    | Input   | Output   |
| ------ |------------ | ------- | -------- |
| `start_engine` | Takes in a float representing the requested trip distance, and toggles the engine on and sets the distance to destination in `VehicleState`. | `f32` | `Result<(), TripRequestError>` |
| `trigger_check` | This is an async node that sleeps for 500ms to allow other parts of the traffic simulator to run. This is useful in workflows that contain all `Blocking` nodes, and prevents the app from being stuck executing the workflow. | - | - |
| `move_vehicle` | Given the input `MoveVehicle` command, attempt to move the simulated vehicle accordingly. | `MoveVehicle` | - |
| `destination_reached` | Checks the current `VehicleState` and whether the vehicle has completed travelling the requested distance. | - | `Result<(), ()>` |
| `stop_engine` | Stops the vehicle, turns its engine off, and reset state parameters. | - | - |
| `trip_error` | A logger node that prints out trip errors. | `TripRequestError` | - |

### Example

Try loading `base_workflow.json` into the diagram editor and observe the vehicle travel the requested distance. Note that in this workflow, the vehicle ignores its environment (e.g. traffic signal, obstacles, etc.) and simply moves forward at the default speed.


## Intermediate workflows

You may wish to create a more complex workflow that accounts for other factors, e.g.
- Respect traffic signals and only move forward when the traffic light is green
- Slow down or stop the vehicle when there are pedestrians/obstacles in front of the vehicle
- Any combination of the above

The example application comes with some additional nodes to experiment with:

| Node   | Use case    | Input   | Output   |
| ------ |------------ | ------- | -------- |
| `begin_vehicle_check` | Outputs the vehicle's current state checklist in the form of a HashMap. This can be connected to a [Split](https://open-rmf.github.io/crossflow-handbook/parallelism.html#split) operation that sends the HashMap's elements down different branches.  | - | `HashMap<String, ReadyState>` |
| `vehicle_check_ready` | Takes in the current `ReadyState` of a single vehicle checklist element, and outputs `ReadyState::Ready`. This node is currently used to represent conducting checks on each checklist item. | `ReadyState` | `ReadyState` |
| `validate_vehicle_check` | Takes in a collection of `ReadyState`, and checks that all the checklist items are ready. This can be preceded by a [Join](https://open-rmf.github.io/crossflow-handbook/join.html) operation to demonstrate combining and synchronizing outputs of various nodes. | `Vec<ReadyState>` | `Result<(), TripRequestError>` |
| `detect_traffic_signal` | A continuous service node that monitors the upcoming traffic signal via events, and streams them out. In more complex workflows, the stream out can be connected to a [Buffer](https://open-rmf.github.io/crossflow-handbook/buffers.html) node to manage data being received at different rates. | - | - |
| `process_traffic_signal` | This node checks the newest `TrafficSignal` message in the buffer to determine the best vehicle move. It only cares about the latest signal. It requires a key to access the `TrafficSignal` buffer. | `((), BufferKey<TrafficSignal>)` | `Result<MoveVehicle, ()>` |
| `configure_obstacles_thresholds` | This node takes in an optional config for users to configure `ObstacleLimits` which affects whether obstacles surrounding the vehicle is considered to be close enough. The configured values are updated to the `WorldLimits` resource, and will be reset in the `stop_engine` node. | - | - |
| `detect_obstacles` | A continuous service node that monitors the current obstacles around the vehicle via query, and streams them out. In more complex workflows, the stream out can be connected to a [Buffer](https://open-rmf.github.io/crossflow-handbook/buffers.html) node to manage data being received at different rates. | - | - |
| `process_obstacles` | This node pulls the newest `Obstacles` message in the buffer to determine the best vehicle move. It only cares about the latest detected obstacles. It requires a key to access the `Obstacles` buffer. | `((), BufferKey<Obstacles>)` | `Result<MoveVehicle, ()>` |
| `filter_arriving` | This node takes in an optional config for users to configure the distance-to-intersection threshold for `ApproachingIntersection` messages. | `ApproachingIntersection` | `Result<ApproachingIntersection, ()>` |
| `approaching_intersection` | A continuous service node that calculates the main vehicle's distance to the next intersection, and streams out `ApproachingIntersection` messages when the vehicle is arriving at the intersection line. | - | - |
| `check_change_lane` | This node takes in the best vehicle move determined by the previous node, and checks for adjacent obstacles to decide whether the vehicle should attempt at changing lane. This only takes effect if the `allow_change_lane` feature is enabled via the simulator UI. | `(MoveVehicle, BufferKey<Obstacles>)` | `MoveVehicle` |
| `follow_speed_limit` | This node checks the current speed limit and slows down the vehicle if the current speed or commanded speed has exceeded the limit. | `MoveVehicle` | `MoveVehicle` |
| `join_traffic_signal_and_obstacles` | This node checks the input `TrafficSignalWithObstacles` constructed by a preceding [Join](https://open-rmf.github.io/crossflow-handbook/join.html) operation to determine the best vehicle move. It accounts for both `TrafficSignal` and `Obstacles` data, and chooses the best move based on both factors. | `TrafficSignalWithObstacles` | `Result<MoveVehicle, TripRequestError>` |
| `listen_traffic_signal_and_obstacles` | This node checks the latest `TrafficSignal` and/or `Obstacles` buffers via a preceding [Listen](https://open-rmf.github.io/crossflow-handbook/listen.html) operation to determine the best vehicle move. Since Listen operations are activated when any of the connected buffers are modified, if either buffer is empty, it will calculate the best move based on the other buffer. If both buffers contain messages, it will choose the best move based on both factors. It requires keys to both `TrafficSignal` and `Obstacles` buffers. | `TrafficSignalWithObstaclesAccessor` | `Result<MoveVehicle, TripRequestError>` |

### Examples

You may consider starting with the ready-made JSON workflows in `traffic_app/diagrams/` and observe how the vehicle behaves differently between them. Try experimenting with the various settings, such as buffer sizes and fetch types (clone vs. pull) to see how they affect the workflows.


## Try it out!

From the current directory, run

```bash
cargo run -- serve
```

Then open http://localhost:3000 to run the diagram editor app from your web browser.
