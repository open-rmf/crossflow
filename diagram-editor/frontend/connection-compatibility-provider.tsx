import type { Connection, NodeAddChange } from '@xyflow/react';
import { useConnection } from '@xyflow/react';
import React from 'react';
import { useApiClient } from './api-client-provider';
import { useDiagramProperties } from './diagram-properties-provider';
import type { DiagramEditorEdge } from './edges';
import type { NodeManager } from './node-manager';
import type { DiagramEditorNode } from './nodes';
import { useRegistry } from './registry-provider';
import { useTemplates } from './templates-provider';
import type { CompatibilityResult } from './types/api';
import {
  createConnectionFromHandles,
  validateDraggedHandlePair,
} from './utils/connection';
import {
  type BuiltCompatibilityCandidate,
  buildCompatibilityCandidate,
  checkCompatibilityCandidates,
  compatibilityCandidateKey,
  localFailureToCompatibilityResult,
} from './utils/compatibility';

interface CompatibilityCheckInput {
  id: string;
  connection: Connection;
  nodeChanges?: NodeAddChange<DiagramEditorNode>[];
}

interface ConnectionCompatibilityContextValue {
  checkConnections: (
    inputs: CompatibilityCheckInput[],
  ) => Promise<Map<string, CompatibilityResult>>;
}

const ConnectionCompatibilityContext =
  React.createContext<ConnectionCompatibilityContextValue | null>(null);

const MAX_COMPATIBILITY_CACHE_ENTRIES = 500;

function setCachedCompatibilityResult(
  cache: Map<string, Promise<CompatibilityResult>>,
  key: string,
  value: Promise<CompatibilityResult>,
) {
  if (!cache.has(key) && cache.size >= MAX_COMPATIBILITY_CACHE_ENTRIES) {
    const oldestKey = cache.keys().next().value;
    if (oldestKey) {
      cache.delete(oldestKey);
    }
  }
  cache.set(key, value);
}

function compatibilityErrorResult(
  id: string,
  error: unknown,
): CompatibilityResult {
  return {
    id,
    status: 'unknown',
    reason:
      error instanceof Error ? error.message : 'compatibility check failed',
  };
}

function unknownCompatibilityResult(
  id: string,
  reason: string,
): CompatibilityResult {
  return {
    id,
    status: 'unknown',
    reason,
  };
}

function incompatibleCompatibilityResult(
  id: string,
  reason: string,
): CompatibilityResult {
  return {
    id,
    status: 'incompatible',
    reason,
  };
}

export function ConnectionCompatibilityProvider({
  nodeManager,
  edges,
  children,
}: React.PropsWithChildren<{
  nodeManager: NodeManager;
  edges: DiagramEditorEdge[];
}>) {
  const apiClient = useApiClient();
  const registry = useRegistry();
  const [templates] = useTemplates();
  const [diagramProperties] = useDiagramProperties();
  const cache = React.useRef(new Map<string, Promise<CompatibilityResult>>());

  const checkConnections = React.useCallback(
    async (inputs: CompatibilityCheckInput[]) => {
      const results = new Map<string, CompatibilityResult>();
      const misses: {
        id: string;
        key: string;
        candidate: BuiltCompatibilityCandidate;
      }[] = [];

      for (const input of inputs) {
        let built: ReturnType<typeof buildCompatibilityCandidate>;
        try {
          built = buildCompatibilityCandidate({
            id: input.id,
            registry,
            nodeManager,
            edges,
            templates,
            diagramProperties,
            connection: input.connection,
            nodeChanges: input.nodeChanges,
          });
        } catch (error) {
          results.set(
            input.id,
            incompatibleCompatibilityResult(
              input.id,
              error instanceof Error
                ? error.message
                : 'failed to build compatibility candidate',
            ),
          );
          continue;
        }

        if (!built.ok) {
          results.set(
            input.id,
            localFailureToCompatibilityResult(built.result),
          );
          continue;
        }

        const key = compatibilityCandidateKey(built.candidate);
        const cached = cache.current.get(key);
        if (cached) {
          results.set(input.id, await cached);
          continue;
        }

        misses.push({
          id: input.id,
          key,
          candidate: built.candidate,
        });
      }

      if (misses.length > 0) {
        const batch = checkCompatibilityCandidates(
          apiClient,
          misses.map((miss) => miss.candidate),
        );
        const pendingResults = misses.map((miss) => {
          const pending = batch
            .then(
              (batchResults) =>
                batchResults.get(miss.id) ??
                unknownCompatibilityResult(
                  miss.id,
                  'compatibility result was not returned',
                ),
            )
            .catch((error) => {
              cache.current.delete(miss.key);
              return compatibilityErrorResult(miss.id, error);
            });
          setCachedCompatibilityResult(cache.current, miss.key, pending);
          return pending.then((result) => [miss.id, result] as const);
        });

        for (const [id, result] of await Promise.all(pendingResults)) {
          results.set(id, result);
        }
      }

      return results;
    },
    [apiClient, registry, nodeManager, edges, templates, diagramProperties],
  );

  const value = React.useMemo(() => ({ checkConnections }), [checkConnections]);

  return (
    <ConnectionCompatibilityContext.Provider value={value}>
      {children}
    </ConnectionCompatibilityContext.Provider>
  );
}

export function useConnectionCompatibility(
  connection: Connection | null,
  id = 'connection',
): CompatibilityResult | null {
  const context = React.useContext(ConnectionCompatibilityContext);
  const [result, setResult] = React.useState<CompatibilityResult | null>(null);

  React.useEffect(() => {
    if (!connection || !context) {
      setResult(null);
      return;
    }

    let active = true;
    setResult(null);
    context
      .checkConnections([{ id, connection }])
      .then((results) => {
        if (active) {
          setResult(results.get(id) ?? null);
        }
      })
      .catch((error) => {
        if (active) {
          setResult(compatibilityErrorResult(id, error));
        }
      });

    return () => {
      active = false;
    };
  }, [context, connection, id]);

  return result;
}

export function useDraggedConnectionCompatibility({
  id,
  otherNodeId,
  otherHandleId,
  otherHandleType,
  skipSelf = false,
}: {
  id: string;
  otherNodeId: string | null | undefined;
  otherHandleId: string | null | undefined;
  otherHandleType: 'source' | 'target';
  skipSelf?: boolean;
}): CompatibilityResult | null {
  const connection = useConnection();
  const fromHandle = connection.fromHandle;
  const fromNodeId = fromHandle?.nodeId;
  const fromHandleId = fromHandle?.id;
  const fromHandleType = fromHandle?.type;

  const candidate = React.useMemo((): {
    connection: Connection | null;
    localCompatibility: CompatibilityResult | null;
  } => {
    if (
      !connection.inProgress ||
      !fromNodeId ||
      !fromHandleType ||
      !otherNodeId
    ) {
      return { connection: null, localCompatibility: null };
    }

    if (
      skipSelf &&
      fromNodeId === otherNodeId &&
      (fromHandleId || null) === (otherHandleId || null) &&
      fromHandleType === otherHandleType
    ) {
      return { connection: null, localCompatibility: null };
    }

    const direction = validateDraggedHandlePair({
      fromHandleType,
      otherHandleType,
    });
    if (!direction.valid) {
      return {
        connection: null,
        localCompatibility: incompatibleCompatibilityResult(
          id,
          direction.error,
        ),
      };
    }

    return {
      connection: createConnectionFromHandles(
        {
          nodeId: fromNodeId,
          id: fromHandleId,
          type: fromHandleType,
        },
        otherNodeId,
        otherHandleId,
      ),
      localCompatibility: null,
    };
  }, [
    connection.inProgress,
    fromHandleId,
    fromHandleType,
    fromNodeId,
    id,
    otherHandleId,
    otherHandleType,
    otherNodeId,
    skipSelf,
  ]);

  const remoteCompatibility = useConnectionCompatibility(
    candidate.connection,
    id,
  );
  return candidate.localCompatibility ?? remoteCompatibility;
}

export function useCompatibilityChecker() {
  const context = React.useContext(ConnectionCompatibilityContext);
  if (!context) {
    throw new Error(
      'useCompatibilityChecker must be used within ConnectionCompatibilityProvider',
    );
  }
  return context;
}
