import { of } from 'rxjs';
import type { BaseApiClient } from '../api-client/base-api-client';
import { createBufferEdge, createSectionEdge } from '../edges';
import { NodeManager } from '../node-manager';
import { createOperationNode } from '../nodes';
import type { DiagramElementMetadata, SectionTemplate } from '../types/api';
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
  test.each([
    [
      'template arrays',
      { inputs: ['request'], outputs: ['response'] },
      'request',
      'response',
    ],
    [
      'template remapping',
      { inputs: { request: 'internal' }, outputs: ['response'] },
      'request',
      'response',
    ],
    ['registered section', null, 'request', 'response'],
    [
      'ambiguous ports',
      { inputs: ['left', 'right'], outputs: ['yes', 'no'] },
      '',
      '',
    ],
    ['empty interface', {}, '', ''],
    ['missing definition', undefined, '', ''],
  ] as const)(
    'selects only unambiguous section ports: %s',
    (_, definition, inputId, output) => {
      const op =
        definition === null
          ? { type: 'section' as const, builder: 'example' }
          : { type: 'section' as const, template: 'example' };
      const source = createOperationNode(
        ROOT_NAMESPACE,
        undefined,
        { x: 0, y: 0 },
        op,
        'source',
      );
      const target = createOperationNode(
        ROOT_NAMESPACE,
        undefined,
        { x: 0, y: 0 },
        op,
        'target',
      );
      const registry: DiagramElementMetadata = {
        ...stubRegistry,
        sections: {
          example: {
            config_examples: [],
            config_schema: {},
            default_display_text: 'Example',
            interface: {
              inputs: { request: { message_type: 0 } },
              outputs: { response: { message_type: 0 } },
              buffers: {},
            },
          },
        },
      };
      const templates: Record<string, SectionTemplate> = definition
        ? { example: JSON.parse(JSON.stringify({ ...definition, ops: {} })) }
        : {};
      const result = buildCompatibilityCandidate({
        id: 'section-connection',
        registry,
        templates,
        nodeManager: new NodeManager([source, target]),
        edges: [],
        diagramProperties: {},
        connection: {
          source: source.id,
          sourceHandle: null,
          target: target.id,
          targetHandle: null,
        },
      });
      expect(result).toMatchObject({
        ok: true,
        candidate: {
          edge: {
            data: {
              input: { type: 'sectionInput', inputId },
              output: { output },
            },
          },
          diagram: {
            ops: { source: { connect: { [output]: { target: inputId } } } },
          },
        },
      });
    },
  );

  test.each([
    [['used'], undefined, ''],
    [['used', 'free'], undefined, 'free'],
    [['used'], 'existing', 'used'],
  ])(
    'selects only available section outputs %j when replacing %s',
    (outputs, edgeId, output) => {
      const source = createOperationNode(
        ROOT_NAMESPACE,
        undefined,
        { x: 0, y: 0 },
        { type: 'section', template: 'example' },
        'source',
      );
      const target = createOperationNode(
        ROOT_NAMESPACE,
        undefined,
        { x: 0, y: 0 },
        { type: 'buffer' },
        'target',
      );
      const existing = createSectionEdge(source.id, null, target.id, null, {
        output: 'used',
      });
      existing.id = 'existing';
      const result = buildCompatibilityCandidate({
        id: 'section-connection',
        registry: stubRegistry,
        templates: { example: { outputs, ops: {} } },
        nodeManager: new NodeManager([source, target]),
        edges: [existing],
        diagramProperties: {},
        edgeId,
        connection: {
          source: source.id,
          sourceHandle: null,
          target: target.id,
          targetHandle: null,
        },
      });
      expect(result).toMatchObject({
        ok: true,
        candidate: { edge: { data: { output: { output } } } },
      });
    },
  );

  test('selects a section buffer input separately from its data inputs', () => {
    const source = createOperationNode(
      ROOT_NAMESPACE,
      undefined,
      { x: 0, y: 0 },
      { type: 'buffer' },
      'source',
    );
    const target = createOperationNode(
      ROOT_NAMESPACE,
      undefined,
      { x: 0, y: 0 },
      { type: 'section', template: 'example' },
      'target',
    );
    const result = buildCompatibilityCandidate({
      id: 'buffer-connection',
      registry: stubRegistry,
      templates: {
        example: {
          inputs: ['request'],
          buffers: { storage: 'internal' },
          ops: {},
        },
      },
      nodeManager: new NodeManager([source, target]),
      edges: [],
      diagramProperties: {},
      connection: {
        source: source.id,
        sourceHandle: null,
        target: target.id,
        targetHandle: null,
      },
    });
    const edge = result.ok ? result.candidate.edge : result.edge;
    expect(edge?.data.input).toEqual({
      type: 'sectionBuffer',
      inputId: 'storage',
    });
  });

  test('an unrelated export failure keeps a structurally valid edge available', () => {
    const source = createOperationNode(
      ROOT_NAMESPACE,
      undefined,
      { x: 0, y: 0 },
      { type: 'node', builder: '', next: { builtin: 'dispose' } },
      'source',
    );
    const target = {
      ...source,
      id: 'target',
      data: { ...source.data, opId: 'target' },
    };
    const unfinished = {
      ...source,
      id: 'unfinished',
      data: { ...source.data, namespace: ':missing_scope' },
    };
    const result = buildCompatibilityCandidate({
      id: 'connection',
      registry: stubRegistry,
      nodeManager: new NodeManager([source, target, unfinished]),
      edges: [],
      templates: {},
      diagramProperties: {},
      connection: {
        source: source.id,
        sourceHandle: null,
        target: target.id,
        targetHandle: null,
      },
    });
    expect(result).toMatchObject({
      ok: false,
      result: { status: 'unknown' },
      edge: { source: source.id, target: target.id },
    });
  });

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
