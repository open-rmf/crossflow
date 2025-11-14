import type { DiagramEditorEdge } from './edges';
import type { DiagramEditorNode } from './nodes';
import type { Diagram } from './types/api';

const EXTENSION_KEY = 'crossflow-diagram-editor-v1';

export interface PersistedState {
  nodes: DiagramEditorNode[];
  edges: DiagramEditorEdge[];
}

interface ExtensionData {
  state: PersistedState;
}

export function saveState(diagram: Diagram, state: PersistedState) {
  if (!diagram.extensions) {
    diagram.extensions = {};
  }
  diagram.extensions[EXTENSION_KEY] = {
    state,
  } satisfies ExtensionData;
}

export function loadState(diagram: Diagram): PersistedState | null {
  return diagram.extensions?.[EXTENSION_KEY]
    ? ((diagram.extensions[EXTENSION_KEY] as ExtensionData)
        .state as PersistedState)
    : null;
}
