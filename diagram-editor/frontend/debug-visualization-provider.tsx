import {
  createContext,
  type PropsWithChildren,
  useContext,
} from 'react';

export interface DebugVisualizationContext {
  activeNodeIds: Set<string>;
  latestNodeId: string | null;
  clearDebugVisualization: () => void;
  markDebugFinished: () => void;
  markDebugOperationStarted: (operationId: string) => void;
}

const DefaultDebugVisualizationContext: DebugVisualizationContext = {
  activeNodeIds: new Set(),
  latestNodeId: null,
  clearDebugVisualization: () => {},
  markDebugFinished: () => {},
  markDebugOperationStarted: () => {},
};

const DebugVisualizationContextComp =
  createContext<DebugVisualizationContext>(
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
