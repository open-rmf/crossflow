import type { DiagramEditorEdge } from './edges';
import type { DiagramEditorNode } from './nodes';
import type { Diagram } from './types/api';

const EXTENSION_KEY = 'crossflow-diagram-editor-v1';

export interface PersistedState {
  nodes: DiagramEditorNode[];
  edges: DiagramEditorEdge[];
}

interface ExtensionData {
  hash: string;
  state: PersistedState;
}

async function hashDiagram(diagram: Diagram): Promise<string> {
  // do not hash the extensions
  const { extensions: _, ...strippedExt } = diagram;
  const payload = JSON.stringify(strippedExt);

  const textEncoder = new TextEncoder();
  const data = textEncoder.encode(payload);
  const hashBuffer = await crypto.subtle.digest('SHA-1', data);
  const hashArray = Array.from(new Uint8Array(hashBuffer));
  const hashHex = hashArray
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');
  return hashHex;
}

/**
 * Save the render state into the diagram using the diagram's "extensions" field.
 */
export async function saveState(diagram: Diagram, state: PersistedState) {
  if (!diagram.extensions) {
    diagram.extensions = {};
  }
  diagram.extensions[EXTENSION_KEY] = {
    hash: await hashDiagram(diagram),
    state,
  } satisfies ExtensionData;
}

/**
 * Attempt to load a render state from a diagram's "extensions" field.
 * Returns `null` if no state is saved or if the saved state is stale.
 */
export async function loadState(
  diagram: Diagram,
): Promise<PersistedState | null> {
  const extData = diagram.extensions?.[EXTENSION_KEY] as ExtensionData;
  if (!extData) {
    return null;
  }
  const hash = await hashDiagram(diagram);
  if (hash !== extData.hash) {
    return null;
  }
  return extData.state;
}
