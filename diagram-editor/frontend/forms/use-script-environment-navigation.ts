import { useCallback } from 'react';
import { useDiagramProperties } from '../diagram-properties-provider';
import { useDiagramSidePanel } from '../diagram-side-panel-controller';

export function useScriptEnvironmentNavigation() {
  const [diagramProperties] = useDiagramProperties();
  const { openEnvironments } = useDiagramSidePanel();

  return useCallback(
    (environmentName?: string, isNew = false) => {
      const hasEnvironments =
        Object.keys(diagramProperties.script_environments || {}).length > 0;

      openEnvironments({
        environmentName,
        create: isNew && !hasEnvironments,
      });
    },
    [diagramProperties.script_environments, openEnvironments],
  );
}
