import { fireEvent, render, screen } from '@testing-library/react';
import { of } from 'rxjs';
import { ApiClient } from '../api-client';
import { ApiClientProvider } from '../api-client-provider';
import { DiagramPropertiesProvider } from '../diagram-properties-provider';
import {
  DiagramSidePanelProvider,
  useDiagramSidePanel,
} from '../diagram-side-panel-controller';
import { NodeManager, NodeManagerProvider } from '../node-manager';
import type { OperationNode } from '../nodes';
import { NotificationProvider } from '../notification-provider';
import { RegistryProvider } from '../registry-provider';
import type { DiagramElementMetadata } from '../types/api';
import ScriptForm from './script-form';

const registry: DiagramElementMetadata = {
  messages: [],
  nodes: {},
  schemas: {},
  sections: {},
  trace_supported: false,
  reverse_message_lookup: {
    result: [],
    split: [],
    unzip: [],
  },
  scripting: {},
};

const scriptNode: OperationNode<'script'> = {
  id: 'script-1',
  type: 'script',
  position: { x: 0, y: 0 },
  data: {
    namespace: '',
    opId: 'script_1',
    op: {
      type: 'script',
      environment: 'analysis',
      run: 'run',
      next: { builtin: 'dispose' },
    },
  },
};

function PanelStateProbe() {
  const { state } = useDiagramSidePanel();
  return <output data-testid="panel-state">{JSON.stringify(state)}</output>;
}

function renderScriptForm() {
  const apiClient = new ApiClient();
  jest.spyOn(apiClient, 'getRegistry').mockReturnValue(of(registry));

  return render(
    <ApiClientProvider value={apiClient}>
      <RegistryProvider>
        <NodeManagerProvider value={new NodeManager([scriptNode])}>
          <DiagramPropertiesProvider>
            <DiagramSidePanelProvider>
              <NotificationProvider>
                <ScriptForm
                  node={scriptNode}
                  onChange={jest.fn()}
                  onDelete={jest.fn()}
                />
                <PanelStateProbe />
              </NotificationProvider>
            </DiagramSidePanelProvider>
          </DiagramPropertiesProvider>
        </NodeManagerProvider>
      </RegistryProvider>
    </ApiClientProvider>,
  );
}

describe('ScriptForm', () => {
  test('keeps a missing assigned environment visible in the selector', () => {
    renderScriptForm();

    fireEvent.mouseDown(
      screen.getByRole('combobox', { name: 'Script Environment' }),
    );

    expect(
      screen.getByRole('option', { name: 'analysis (missing)' }),
    ).toHaveAttribute('aria-disabled', 'true');
  });

  test('Manage opens the assigned environment in the shared drawer', () => {
    renderScriptForm();

    fireEvent.click(
      screen.getByRole('button', {
        name: 'Manage or add script environment',
      }),
    );

    expect(screen.getByTestId('panel-state')).toHaveTextContent(
      '"tab":"environments"',
    );
    expect(screen.getByTestId('panel-state')).toHaveTextContent(
      '"selectedEnvironmentName":"analysis"',
    );
  });
});
