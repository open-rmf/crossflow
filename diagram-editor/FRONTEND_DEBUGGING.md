# Frontend Debugging Coordination Memory

Last updated: 2026-05-13

This file is the working memory and coordination point for adding frontend
debugging support to the diagram editor. Keep it current when backend debug APIs,
frontend state models, or UX plans change.

## Current Findings

Crossflow has meaningful backend debugging support, but the diagram-editor API
only exposes a small slice of it today.

Runtime support in `src/`:

- `src/debug.rs` defines the `Debug` resource, breakpoint storage, active debug
  sessions, paused sessions, pause/unpause notifications, and stepping commands.
- `DebugStepExt` exposes `commands.debug_step(session)` and
  `commands.debug_step_for_operation(session, operation)`.
- `DebugRoster` holds deferred paused inputs and releases one deferred request
  when a step command is issued.
- `src/input.rs` integrates debugging into input consumption: when tracing is
  enabled, inputs can be held in storage if their session is paused or a
  breakpoint was hit.
- `src/trace.rs` defines the trace event model used by debugging:
  `MessageSent`, `BufferEvent`, `SessionEvent`, `OutputDisposed`, and `Broken`.
  `SessionEvent` includes spawned, despawned, cancelled, cleanup, paused, and
  unpaused changes.
- `UniversalTraceToggle` can force tracing on, including message tracing, which
  is useful for debug runs even if the diagram was not configured for tracing.
- Public re-exports for frontend-relevant native debugging are behind the
  `trace` feature: `Debug`, `DebugStepExt`, and `UniversalTraceToggle`.

Diagram model support:

- `Diagram.default_trace` exists and can be `off`, `on`, or `messages`.
- Operation-level `trace` exists on diagram operations and can override the
  diagram default.
- The API schema and generated TypeScript already include `TraceToggle` and
  `DebugSessionMessage`.

Diagram-editor backend support:

- `diagram-editor/Cargo.toml` has a `debug` feature that enables `router` and
  `axum/ws`.
- `diagram-editor/server/api/executor.rs` exposes `/api/executor/debug` when
  the `debug` feature is enabled.
- The WebSocket protocol currently expects the first client message to be a
  `PostRunRequest`, then starts the workflow.
- The server attaches `FeedbackSender` to the root session and broadcasts trace
  events that match that session.
- The current public WebSocket response is lossy: operation-start trace events
  are converted to `DebugSessionMessage::Feedback(OperationStarted(op_id))`.
- The session ends with `DebugSessionMessage::Finish(Ok(value))` or
  `DebugSessionMessage::Finish(Err(message))`.
- The server does not currently expose commands to set breakpoints, pause,
  unpause, step, or stop a debug session through the WebSocket.
- The existing `debug` feature needed small repairs before it could serve the
  current frontend slice: the WebSocket response path now awaits the `Outcome`,
  trace feedback is registered as a Bevy observer because trace records are
  emitted with `world.trigger(...)`, and finish is delayed briefly so already
  emitted feedback can reach the socket first.

Frontend support:

- `diagram-editor/frontend/api-client/debug-session.ts` wraps a WebSocket and
  validates incoming `DebugSessionMessage` objects with AJV. It now buffers
  early messages with a bounded `ReplaySubject`, completes on socket close, and
  exposes `close()` for UI cleanup.
- `diagram-editor/frontend/api-client/base-api-client.ts` exposes
  `wsDebugWorkflow` as an optional capability. The REST client implements it;
  the WASM client remains out of scope and does not provide the method.
- `RunButton` now has both Run and Debug actions. Debug uses the existing
  request/response UI, sends a trace-enabled copy of the exported diagram, shows
  a compact timeline, keeps the debug session running when the popover is
  hidden, and keeps normal Run behavior unchanged.
- `DebugVisualizationProvider` holds transient debug highlight state. Nodes use
  that context for visual highlighting without writing debug state into diagram
  data. The latest executing node now uses the primary green progress glow; older
  visited nodes remain visually subdued.
- `diagram-editor/rsbuild.config.ts` now enables WebSocket proxying for `/api`
  so the local dev server can proxy `/api/executor/debug`.
- `DiagramPropertiesProvider` does not currently track `default_trace`, so
  frontend export paths may need review before using diagram-level trace
  settings from the UI.

## Implemented Slice 1

The first deliverable slice is now implemented as a minimal frontend
visualization over the existing backend protocol.

- The REST API client opens `ws://.../api/executor/debug` or
  `wss://.../api/executor/debug` on the current origin and sends a
  `PostRunRequest`.
- Debug support is optional at the `BaseApiClient` level. If an active client
  does not expose `wsDebugWorkflow`, the Debug action reports that debug
  sessions are not supported by that backend.
- Debug runs clone the exported diagram and set `default_trace: "on"` plus
  operation-level `trace: "on"` recursively. This is request-local and does not
  mutate editor state, persisted diagrams, or normal export output.
- Incoming `operationStarted` events are appended to the run popover timeline
  and mapped to existing React Flow nodes for transient highlighting.
- The final `finish` message is rendered in the existing response panel.
- Backend changes were limited to making the already-existing `debug` feature
  compile and emit its current `operationStarted`/`finish` protocol.
- The embedded frontend archive `diagram-editor/dist.tar.gz` was regenerated
  with `BUILD_FRONTEND=1` after frontend changes.

## Implemented Slice 2

The second deliverable slice improves progress visibility without changing the
backend protocol.

- The diagram editor treats the latest `operationStarted` event as the current
  execution point.
- The current node is highlighted directly in the graph with a green glow using
  the success theme color.
- Previously visited nodes are kept quieter so the current execution point is
  easier to see.
- When a debug session finishes or errors, the frontend clears the current-node
  glow while preserving the visited-node context.
- Closing the Run popover no longer closes the active debug WebSocket, clears
  the timeline, or removes graph highlighting. The debug run continues in the
  background until it finishes, errors, a new run starts, or the editor
  unmounts.
- Loading a new diagram clears debug visualization so stale node highlights do
  not carry across diagrams.
- The popover timeline is bounded to the most recent 200 events.
- The backend drains queued debug feedback before sending `finish` so already
  received operation-start events are less likely to be hidden by completion.
- Breakpoints and stepping remain out of scope for this slice because the
  WebSocket protocol still has no client command messages.

Verification performed:

- `pnpm --dir diagram-editor check:ts`
- `pnpm --dir diagram-editor exec jest --runInBand`
- `env BUILD_FRONTEND=1 cargo run --manifest-path examples/diagram/calculator/Cargo.toml --features crossflow_diagram_editor/debug -- serve --port 3001`
- WebSocket smoke test through `ws://127.0.0.1:3000/api/executor/debug` using
  the calculator `multiply_by_3` diagram produced:
  `operationStarted: "mul3"`, `operationStarted: "(terminate)"`, then
  `finish ok: 369.0`.

## Working Assumptions

- The first frontend milestone should use the existing WebSocket stream before
  expanding the backend protocol.
- Operation IDs in debug feedback should map to React Flow node IDs whenever
  possible.
- The frontend should treat the debug API as optional because it is feature
  gated and unavailable in the WASM backend today.
- A useful UI can start as a run/debug panel and timeline without full stepping,
  but true debugging needs backend protocol additions.
- Backend protocol expansion should remain separate from frontend visualization
  unless an existing route is broken or cannot support the current slice.

## Long-Term Plan

1. Expose the debug capability in the frontend API layer.
   - Done for the REST path as an optional API client capability.
   - WASM remains unsupported and intentionally out of scope for now.
   - Future work may add explicit capability metadata instead of checking for an
     optional method.

2. Build a minimal debug run UI.
   - Done for `operationStarted` timeline, transient node highlighting, and
     final response/error rendering.
   - Current graph progress is now emphasized with a green glow on the latest
     executing node.

3. Preserve trace settings through diagram export.
   - Confirm whether `exportDiagram` currently retains `default_trace` and
     operation-level `trace`.
   - Add UI controls for `default_trace` if the debug flow needs users to opt
     into `on` or `messages`.
   - Prefer debug-session-specific trace forcing in the backend if ordinary
     diagram trace settings should remain untouched.

4. Expand the backend WebSocket protocol.
   - Add client-to-server command messages for pause, unpause, step, step a
     specific operation, set/clear breakpoint, and stop session.
   - Include a stable frontend operation identifier in feedback messages instead
     of only an entity-derived string when possible.
   - Expose session lifecycle events directly, including paused by breakpoint,
     paused by user, and unpaused.
   - Expose richer trace payloads for message routing and buffer activity.

5. Add frontend debugger state.
   - Maintain session status: starting, running, paused, stepping, finishing,
     finished, errored.
   - Maintain breakpoint state keyed by diagram operation ID.
   - Maintain current/last-active operation IDs for graph highlighting.
   - Keep a bounded timeline to avoid unbounded memory growth on long runs.

6. Add verification.
   - Unit-test `DebugSession` message validation and completion behavior.
   - Add component tests for debug button state and timeline rendering.
   - Add backend tests for the expanded WebSocket command protocol.
   - Revisit or replace the ignored `test_ws_debug` once trace event timing is
     deterministic enough for the expected assertions.

## Open Questions

- Should debugging force `UniversalTraceToggle::on()` or
  `UniversalTraceToggle::with_messages()` for debug sessions, independent of
  diagram trace settings? Current slice forces trace in the debug request copy.
- What frontend identifier should the backend send for operations: diagram path,
  operation ID, namespace-qualified ID, or React Flow node ID?
- Should breakpoints be persisted in diagram extensions, local editor state, or
  only per debug session?
- How should WASM debugging work: unsupported initially, implemented through a
  local in-process stream, or parity with the WebSocket protocol?
- Should the user be able to debug multiple workflow sessions at the same time
  from the editor, or should the first implementation enforce one active session?

## Next Practical Slice

The next slice should be one of these, depending on desired product direction:

1. Frontend polish without protocol changes:
   - Add bounded timeline retention.
   - Add clearer debug status labels: connecting, running, finished, errored.
   - Add visible unsupported-backend affordance before the user clicks Debug.
   - Improve operation ID display for builtins such as `(terminate)`.

2. Progress visualization polish:
   - Add an explicit clear/reset control for completed debug visualization.
   - Decide whether the final node should keep glowing after finish or switch to
     a completed state.
   - Consider edge/path highlighting once backend feedback includes enough
     message-routing detail.

3. Protocol groundwork for real debugging:
   - Add client-to-server command messages for pause, unpause, step, and stop.
   - Add breakpoint set/clear messages.
   - Add session lifecycle feedback for paused/unpaused and paused by
     breakpoint.

4. Richer trace visualization:
   - Expose message-routing feedback instead of only `operationStarted`.
   - Add buffer event rows and eventually message payload inspection when trace
     mode is `"messages"`.

Do not add frontend-only breakpoint or step controls until the backend
WebSocket command protocol exists.
