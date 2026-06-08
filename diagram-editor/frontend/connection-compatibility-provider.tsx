import type { Connection, NodeAddChange } from '@xyflow/react';
import React from 'react';
import { useApiClient } from './api-client-provider';
import { useDiagramProperties } from './diagram-properties-provider';
import type { DiagramEditorEdge } from './edges';
import { NodeManager } from './node-manager';
import type { DiagramEditorNode } from './nodes';
import { useRegistry } from './registry-provider';
import { useTemplates } from './templates-provider';
import type { CompatibilityResult } from './types/api';
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
        const built = buildCompatibilityCandidate({
          id: input.id,
          registry,
          nodeManager,
          edges,
          templates,
          diagramProperties,
          connection: input.connection,
          nodeChanges: input.nodeChanges,
        });

        if (!built.ok) {
          results.set(input.id, localFailureToCompatibilityResult(built.result));
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
        for (const miss of misses) {
          cache.current.set(
            miss.key,
            batch.then(
              (batchResults) =>
                batchResults.get(miss.id) ?? {
                  id: miss.id,
                  status: 'unknown',
                  reason: 'compatibility result was not returned',
                },
            ),
          );
        }

        const batchResults = await batch;
        for (const miss of misses) {
          results.set(
            miss.id,
            batchResults.get(miss.id) ?? {
              id: miss.id,
              status: 'unknown',
              reason: 'compatibility result was not returned',
            },
          );
        }
      }

      return results;
    },
    [apiClient, registry, nodeManager, edges, templates, diagramProperties],
  );

  const value = React.useMemo(
    () => ({ checkConnections }),
    [checkConnections],
  );

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
  const key = JSON.stringify(connection);

  React.useEffect(() => {
    if (!connection || !context) {
      setResult(null);
      return;
    }

    let active = true;
    setResult(null);
    context.checkConnections([{ id, connection }]).then((results) => {
      if (active) {
        setResult(results.get(id) ?? null);
      }
    });

    return () => {
      active = false;
    };
  }, [context, key, id]);

  return result;
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
