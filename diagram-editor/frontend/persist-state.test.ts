/**
 * @jest-environment node
 */

import { loadState, type PersistedState, saveState } from './persist-state';
import type { Diagram } from './types/api';
import { loadDiagram } from './utils/load-diagram';

test('save and load state', async () => {
  const diagram: Diagram = {
    version: '0.1.0',
    start: 'op1',
    ops: {
      op1: {
        type: 'node',
        builder: 'test-builder',
        next: { builtin: 'terminate' },
      },
    },
  };
  const { graph, isRestored } = await loadDiagram(diagram);
  expect(isRestored).toBe(false);
  const state: PersistedState = {
    nodes: graph.nodes,
    edges: graph.edges,
  };
  await saveState(diagram, state);
  const loadedState = await loadState(diagram);
  expect(state).toEqual(loadedState);
});

test('detects when a saved state is stale', async () => {
  const diagram: Diagram = {
    version: '0.1.0',
    start: 'op1',
    ops: {
      op1: {
        type: 'node',
        builder: 'test-builder',
        next: { builtin: 'terminate' },
      },
    },
  };
  const { graph, isRestored } = await loadDiagram(diagram);
  expect(isRestored).toBe(false);
  const state: PersistedState = {
    nodes: graph.nodes,
    edges: graph.edges,
  };
  await saveState(diagram, state);
  diagram.ops.op1.builder = 'other-builder';
  expect(loadState(diagram)).resolves.toBe(null);
});
