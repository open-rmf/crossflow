# Diagram Editor UX Roadmap

This page tracks a long-term UX roadmap for the Crossflow diagram editor. The goal is to grow the editor in stages instead of treating all UX work as one large delivery.

## Near-Term Goal

The immediate target is a small demo with visible improvements:

- real-time connection feedback while dragging edges
- clear compatible and incompatible connection hints
- at least one faster path for adding a compatible next operation
- a polished calculator demo flow that is easy to show live

This milestone is intentionally small. It is meant to show momentum and produce an obvious before/after improvement for users building workflows.

## Mid-Term Editor UX Track

After the demo milestone, the main editor track should continue with:

- stronger real-time diagram validation
- better add-operation flow
- config example browser
- improved JSON and config authoring
- better diagnostics presentation

This is the primary product direction because it matches the current open
diagram editor issues and the existing frontend architecture.

## Long-Term Advanced Tooling Track

Tracing and debugging should stay in a later phase:

- execution overlays on nodes and edges
- pause and resume
- breakpoints
- step controls
- multi-session views
- hot start from saved state

These features should build on the tracing and debugging backend support that
landed recently, but they should not block simpler authoring improvements.

## Demo Slice

The current demo slice should stay narrow and frontend-only:

- reuse the local validation logic in the editor
- avoid adding backend validation routes yet
- make drag-time connection affordances clearer
- allow dragging from an output into empty space to open a compatibility-aware
  add-operation helper

Success for this slice means a user can:

1. Start a connection from an existing operation.
2. See which targets are viable before dropping.
3. Drop on empty space and discover a compatible next operation faster.
4. Run the updated calculator demo successfully.
