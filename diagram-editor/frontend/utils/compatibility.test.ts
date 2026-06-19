import { of } from 'rxjs';
import type { BaseApiClient } from '../api-client/base-api-client';
import { createBufferEdge } from '../edges';
import { NodeManager } from '../node-manager';
import { createOperationNode } from '../nodes';
import type { DiagramElementMetadata } from '../types/api';
import {
  buildCompatibilityCandidate,
  checkCompatibilityCandidates,
} from './compatibility';
import { ROOT_NAMESPACE } from './namespace';

const stubRegistry: DiagramElementMetadata = {
  messages: [],
  nodes: {},
  reverse_message_lookup: {
    result: [],
    split: [],
    unzip: [],
  },
  schemas: {},
  scripting: {},
  sections: {},
  trace_supported: false,
};

describe('compatibility candidate builder', () => {
  test('buffer edges use the buffer input as an infer-only focused port', () => {
    const buffer = createOperationNode(
      ROOT_NAMESPACE,
      undefined,
      { x: 0, y: 0 },
      { type: 'buffer' },
      'buffer',
    );
    const listen = createOperationNode(
      ROOT_NAMESPACE,
      undefined,
      { x: 0, y: 0 },
      { type: 'listen', buffers: [], next: { builtin: 'terminate' } },
      'listen',
    );

    const result = buildCompatibilityCandidate({
      id: 'buffer-to-listen',
      registry: stubRegistry,
      nodeManager: new NodeManager([buffer, listen]),
      edges: [],
      templates: {},
      diagramProperties: {},
      connection: {
        source: buffer.id,
        sourceHandle: null,
        target: listen.id,
        targetHandle: null,
      },
    });

    expect(result.ok).toBe(true);
    if (!result.ok) {
      return;
    }

    expect(result.candidate.sourcePort).toBeUndefined();
    expect(result.candidate.targetPort).toBeUndefined();
    expect(result.candidate.focusPorts).toEqual([
      {
        Input: {
          named: {
            namespaces: [],
            exposed_namespace: null,
            name: 'buffer',
          },
        },
      },
    ]);
  });

  test('compatibility checks preserve provisional compatible results', async () => {
    const buffer = createOperationNode(
      ROOT_NAMESPACE,
      undefined,
      { x: 0, y: 0 },
      { type: 'buffer' },
      'buffer',
    );
    const listen = createOperationNode(
      ROOT_NAMESPACE,
      undefined,
      { x: 0, y: 0 },
      { type: 'listen', buffers: [], next: { builtin: 'terminate' } },
      'listen',
    );

    const built = buildCompatibilityCandidate({
      id: 'buffer-to-listen',
      registry: stubRegistry,
      nodeManager: new NodeManager([buffer, listen]),
      edges: [],
      templates: {},
      diagramProperties: {},
      connection: {
        source: buffer.id,
        sourceHandle: null,
        target: listen.id,
        targetHandle: null,
      },
    });

    expect(built.ok).toBe(true);
    if (!built.ok) {
      return;
    }

    const apiClient = {
      getRegistry: jest.fn(() => of(stubRegistry)),
      postRunWorkflow: jest.fn(() => of(null)),
      checkCompatibility: jest.fn((request) =>
        of({
          results: request.candidates.map((candidate) => ({
            id: candidate.id,
            status: 'compatible' as const,
            provisional: true,
            reason: 'connection needs more type context',
          })),
        }),
      ),
    } satisfies BaseApiClient;

    const results = await checkCompatibilityCandidates(apiClient, [
      built.candidate,
    ]);

    expect(results.get('buffer-to-listen')).toMatchObject({
      status: 'compatible',
      provisional: true,
      reason: 'connection needs more type context',
    });
  });

  test('reconnect candidates replace the old edge with the same id', () => {
    const buffer = createOperationNode(
      ROOT_NAMESPACE,
      undefined,
      { x: 0, y: 0 },
      { type: 'buffer' },
      'buffer',
    );
    const oldListen = createOperationNode(
      ROOT_NAMESPACE,
      undefined,
      { x: 0, y: 0 },
      { type: 'listen', buffers: [], next: { builtin: 'terminate' } },
      'old_listen',
    );
    const newListen = createOperationNode(
      ROOT_NAMESPACE,
      undefined,
      { x: 0, y: 0 },
      { type: 'listen', buffers: [], next: { builtin: 'terminate' } },
      'new_listen',
    );
    const oldEdge = createBufferEdge(buffer.id, null, oldListen.id, null, {
      type: 'bufferSeq',
      seq: 0,
    });
    oldEdge.id = 'reconnected-edge';

    const result = buildCompatibilityCandidate({
      id: 'buffer-to-new-listen',
      registry: stubRegistry,
      nodeManager: new NodeManager([buffer, oldListen, newListen]),
      edges: [oldEdge],
      templates: {},
      diagramProperties: {},
      connection: {
        source: buffer.id,
        sourceHandle: null,
        target: newListen.id,
        targetHandle: null,
      },
      edgeId: oldEdge.id,
    });

    expect(result.ok).toBe(true);
    if (!result.ok) {
      return;
    }

    expect(result.candidate.diagram.ops.old_listen).toMatchObject({
      type: 'listen',
      buffers: [],
    });
    expect(result.candidate.diagram.ops.new_listen).toMatchObject({
      type: 'listen',
      buffers: ['buffer'],
    });
  });
});
