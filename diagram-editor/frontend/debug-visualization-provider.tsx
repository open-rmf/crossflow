import { createContext, type PropsWithChildren, useContext } from 'react';

export interface DebugVisualizationContext {
  activeNodeIds: Set<string>;
  visitedNodeIds: Set<string>;
  clearDebugVisualization: () => void;
  markDebugFinished: () => void;
  markDebugOperationFinished: (operationId: string) => void;
  markDebugOperationStarted: (operationId: string) => void;
}

const DefaultDebugVisualizationContext: DebugVisualizationContext = {
  activeNodeIds: new Set(),
  visitedNodeIds: new Set(),
  clearDebugVisualization: () => {},
  markDebugFinished: () => {},
  markDebugOperationFinished: () => {},
  markDebugOperationStarted: () => {},
};

const DebugVisualizationContextComp = createContext<DebugVisualizationContext>(
  DefaultDebugVisualizationContext,
);

export function DebugVisualizationProvider({
  value,
  children,
}: PropsWithChildren<{ value: DebugVisualizationContext }>) {
  return (
    <DebugVisualizationContextComp.Provider value={value}>
      {children}
    </DebugVisualizationContextComp.Provider>
  );
}

export function useDebugVisualization() {
  return useContext(DebugVisualizationContextComp);
}
