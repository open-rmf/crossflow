import type { Connection, NodeAddChange } from '@xyflow/react';
import { firstValueFrom } from 'rxjs';
import type { BaseApiClient } from '../api-client';
import type { DiagramProperties } from '../diagram-properties-provider';
import type { DiagramEditorEdge } from '../edges';
import { NodeManager } from '../node-manager';
import {
  type DiagramEditorNode,
  isBuiltinNode,
  isOperationNode,
} from '../nodes';
import type {
  CompatibilityCandidate,
  CompatibilityRequest,
  CompatibilityResult,
  Diagram,
  DiagramElementMetadata,
  NamespaceList,
  OperationRef,
  OutputKey,
  OutputRef,
  PortRef,
  SectionTemplate,
} from '../types/api';
import {
  createEdgeFromConnection,
  validateConnectionSimple,
} from './connection';
import { exportDiagram } from './export-diagram';
import { ROOT_NAMESPACE, splitNamespaces } from './namespace';

export interface BuiltCompatibilityCandidate {
  id: string;
  connection: Connection;
  edge: DiagramEditorEdge;
  diagram: Diagram;
  focusPorts: PortRef[];
  sourcePort?: PortRef;
  targetPort?: PortRef;
}

export interface LocalCompatibilityFailure {
  id: string;
  status: 'incompatible';
  reason: string;
}

export type CompatibilityBuildResult =
  | { ok: true; candidate: BuiltCompatibilityCandidate }
  | { ok: false; result: LocalCompatibilityFailure };

function incompatibleBuildResult(
  id: string,
  reason: string,
): CompatibilityBuildResult {
  return {
    ok: false,
    result: {
      id,
      status: 'incompatible',
      reason,
    },
  };
}

function cloneJson<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

function namespaceList(namespace: string): NamespaceList {
  return splitNamespaces(namespace).filter((part) => part !== ROOT_NAMESPACE);
}

function namedOperation(
  namespaces: NamespaceList,
  name: string,
  exposedNamespace?: string | null,
): OperationRef {
  return {
    named: {
      namespaces,
      exposed_namespace: exposedNamespace ?? null,
      name,
    },
  };
}

function namedOutput(
  namespaces: NamespaceList,
  operation: string,
  key: OutputKey,
): OutputRef {
  return {
    Named: {
      namespaces,
      operation,
      key,
    },
  };
}

function inputPort(operation: OperationRef): PortRef {
  return { Input: operation };
}

function outputPort(output: OutputRef): PortRef {
  return { Output: output };
}

function operationInputPort(
  node: DiagramEditorNode,
  edge?: DiagramEditorEdge,
): PortRef | null {
  if (isBuiltinNode(node)) {
    switch (node.type) {
      case 'terminate': {
        return inputPort({ terminate: namespaceList(node.data.namespace) });
      }
      case 'start': {
        return null;
      }
    }
  }

  if (isOperationNode(node)) {
    if (
      node.type === 'section' &&
      (edge?.data.input.type === 'sectionInput' ||
        edge?.data.input.type === 'sectionBuffer')
    ) {
      return inputPort(
        namedOperation(
          namespaceList(node.data.namespace),
          edge.data.input.inputId,
          node.data.opId,
        ),
      );
    }

    return inputPort(
      namedOperation(namespaceList(node.data.namespace), node.data.opId),
    );
  }

  if (node.type === 'sectionOutput') {
    return inputPort(namedOperation([], node.data.outputId));
  }

  return null;
}

function forkCloneOutputIndex(
  edge: DiagramEditorEdge,
  edges: DiagramEditorEdge[],
): number {
  return edges
    .filter((candidate) => candidate.source === edge.source)
    .findIndex((candidate) => candidate.id === edge.id);
}

function operationOutputKey(
  sourceNode: DiagramEditorNode,
  edge: DiagramEditorEdge,
  edges: DiagramEditorEdge[],
): OutputKey | null {
  switch (edge.type) {
    case 'default': {
      if (sourceNode.type === 'fork_clone') {
        return ['next', Math.max(0, forkCloneOutputIndex(edge, edges))];
      }
      return ['next'];
    }
    case 'forkResultOk': {
      return ['ok'];
    }
    case 'forkResultErr': {
      return ['err'];
    }
    case 'splitKey': {
      return ['keyed', edge.data.output.key];
    }
    case 'splitSeq':
    case 'unzip': {
      return ['next', edge.data.output.seq];
    }
    case 'splitRemaining': {
      return ['remaining'];
    }
    case 'streamOut': {
      return ['stream_out', edge.data.output.streamId];
    }
    case 'section': {
      return ['connect', edge.data.output.output];
    }
    case 'buffer': {
      return null;
    }
  }
}

function operationOutputPort(
  node: DiagramEditorNode,
  edge: DiagramEditorEdge,
  edges: DiagramEditorEdge[],
): PortRef | null {
  if (isBuiltinNode(node)) {
    switch (node.type) {
      case 'start': {
        return outputPort({ Start: namespaceList(node.data.namespace) });
      }
      case 'terminate': {
        return null;
      }
    }
  }

  if (isOperationNode(node)) {
    if (edge.type === 'buffer') {
      return inputPort(
        namedOperation(namespaceList(node.data.namespace), node.data.opId),
      );
    }

    const key = operationOutputKey(node, edge, edges);
    if (!key) {
      return null;
    }

    return outputPort(
      namedOutput(namespaceList(node.data.namespace), node.data.opId, key),
    );
  }

  if (node.type === 'sectionInput' || node.type === 'sectionBuffer') {
    return inputPort(namedOperation([], node.data.remappedId));
  }

  return null;
}

function portRefsForEdge(
  nodeManager: NodeManager,
  edge: DiagramEditorEdge,
  edges: DiagramEditorEdge[],
): Pick<
  BuiltCompatibilityCandidate,
  'focusPorts' | 'sourcePort' | 'targetPort'
> {
  const sourceNode = nodeManager.getNode(edge.source);
  const targetNode = nodeManager.getNode(edge.target);

  if (edge.type === 'buffer') {
    const bufferPort = operationOutputPort(sourceNode, edge, edges);
    const focusPorts = bufferPort ? [bufferPort] : [];

    return { focusPorts };
  }

  const sourcePort = operationOutputPort(sourceNode, edge, edges) ?? undefined;
  const targetPort = operationInputPort(targetNode, edge) ?? undefined;
  const focusPorts = [sourcePort, targetPort].filter((port): port is PortRef =>
    Boolean(port),
  );

  return { focusPorts, sourcePort, targetPort };
}

function compatibilityRequestCandidate(
  candidate: BuiltCompatibilityCandidate,
): CompatibilityCandidate {
  return {
    id: candidate.id,
    diagram: candidate.diagram,
    focusPorts: candidate.focusPorts,
    sourcePort: candidate.sourcePort ?? null,
    targetPort: candidate.targetPort ?? null,
  };
}

export function compatibilityCandidateKey(
  candidate: BuiltCompatibilityCandidate,
): string {
  return JSON.stringify(compatibilityRequestCandidate(candidate));
}

export function buildCompatibilityCandidate({
  id,
  registry,
  nodeManager,
  edges,
  templates,
  diagramProperties,
  connection,
  nodeChanges = [],
  edgeId,
}: {
  id: string;
  registry: DiagramElementMetadata;
  nodeManager: NodeManager;
  edges: DiagramEditorEdge[];
  templates: Record<string, SectionTemplate>;
  diagramProperties: DiagramProperties;
  connection: Connection;
  nodeChanges?: NodeAddChange<DiagramEditorNode>[];
  edgeId?: string;
}): CompatibilityBuildResult {
  const candidateNodes = cloneJson(nodeManager.nodes);
  for (const change of nodeChanges) {
    if (change.type === 'add') {
      candidateNodes.push(cloneJson(change.item));
    }
  }

  const candidateManager = new NodeManager(candidateNodes);
  const edgeResult = createEdgeFromConnection(
    connection,
    candidateManager,
    edgeId,
  );
  if (!edgeResult.valid) {
    return incompatibleBuildResult(id, edgeResult.error);
  }
  const { edge } = edgeResult;

  const candidateEdges = [
    ...cloneJson(edges).filter((candidateEdge) => candidateEdge.id !== edge.id),
    edge,
  ];
  const simpleValidation = validateConnectionSimple(
    edge,
    candidateManager,
    candidateEdges.filter((candidateEdge) => candidateEdge.id !== edge.id),
  );
  if (!simpleValidation.valid) {
    return incompatibleBuildResult(id, simpleValidation.error);
  }

  const diagram = exportDiagram(
    registry,
    candidateManager,
    candidateEdges,
    cloneJson(templates),
    cloneJson(diagramProperties),
  );
  const ports = portRefsForEdge(candidateManager, edge, candidateEdges);

  if (ports.focusPorts.length === 0) {
    return incompatibleBuildResult(
      id,
      'connection does not expose compatible message ports',
    );
  }

  return {
    ok: true,
    candidate: {
      id,
      connection,
      edge,
      diagram,
      ...ports,
    },
  };
}

export async function checkCompatibilityCandidates(
  apiClient: BaseApiClient,
  candidates: BuiltCompatibilityCandidate[],
): Promise<Map<string, CompatibilityResult>> {
  if (candidates.length === 0) {
    return new Map();
  }

  const request: CompatibilityRequest = {
    candidates: candidates.map(compatibilityRequestCandidate),
  };
  const response = await firstValueFrom(apiClient.checkCompatibility(request));
  return new Map(response.results.map((result) => [result.id, result]));
}

export function localFailureToCompatibilityResult(
  failure: LocalCompatibilityFailure,
): CompatibilityResult {
  return {
    id: failure.id,
    status: failure.status,
    reason: failure.reason,
  };
}
