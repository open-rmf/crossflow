import { fireEvent, render, screen } from '@testing-library/react';
import { useEffect } from 'react';
import { of } from 'rxjs';
import { ApiClient } from '../api-client';
import { ApiClientProvider } from '../api-client-provider';
import {
  DiagramPropertiesProvider,
  useDiagramProperties,
} from '../diagram-properties-provider';
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

function SeedEnvironments() {
  const [, setDiagramProperties] = useDiagramProperties();

  useEffect(() => {
    setDiagramProperties({
      script_environments: {
        analysis: { builder: 'process-bound-python', config: {} },
        staging: { builder: 'process-bound-python', config: {} },
      },
    });
  }, [setDiagramProperties]);

  return null;
}

function renderScriptForm({
  onChange = jest.fn(),
  seedEnvironments = false,
}: {
  onChange?: jest.Mock;
  seedEnvironments?: boolean;
} = {}) {
  const apiClient = new ApiClient();
  jest.spyOn(apiClient, 'getRegistry').mockReturnValue(of(registry));

  return render(
    <ApiClientProvider value={apiClient}>
      <RegistryProvider>
        <NodeManagerProvider value={new NodeManager([scriptNode])}>
          <DiagramPropertiesProvider>
            <DiagramSidePanelProvider>
              <NotificationProvider>
                {seedEnvironments && <SeedEnvironments />}
                <ScriptForm
                  node={scriptNode}
                  onChange={onChange}
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

  test('the settings button toggles the assigned environment drawer', () => {
    renderScriptForm();

    const settings = screen.getByRole('button', {
      name: 'Toggle script environment panel',
    });
    fireEvent.click(settings);

    expect(screen.getByTestId('panel-state')).toHaveTextContent(
      '"tab":"environments"',
    );
    expect(screen.getByTestId('panel-state')).toHaveTextContent(
      '"selectedEnvironmentName":"analysis"',
    );

    fireEvent.click(settings);

    expect(screen.getByTestId('panel-state')).toHaveTextContent('"open":false');
  });

  test('selecting an environment updates the node and shared drawer', () => {
    const onChange = jest.fn();
    renderScriptForm({ onChange, seedEnvironments: true });

    fireEvent.mouseDown(
      screen.getByRole('combobox', { name: 'Script Environment' }),
    );
    fireEvent.click(screen.getByRole('option', { name: 'staging' }));

    expect(onChange).toHaveBeenCalledWith(
      expect.objectContaining({
        item: expect.objectContaining({
          data: expect.objectContaining({
            op: expect.objectContaining({ environment: 'staging' }),
          }),
        }),
      }),
    );
    expect(screen.getByTestId('panel-state')).toHaveTextContent(
      '"selectedEnvironmentName":"staging"',
    );
  });

  test('offers environment creation at the end without changing the node', () => {
    const onChange = jest.fn();
    renderScriptForm({ onChange, seedEnvironments: true });

    fireEvent.mouseDown(
      screen.getByRole('combobox', { name: 'Script Environment' }),
    );
    const options = screen.getAllByRole('option');
    expect(options.at(-1)).toHaveTextContent('Create new environment');
    fireEvent.click(options.at(-1) as HTMLElement);

    expect(onChange).not.toHaveBeenCalled();
    expect(screen.getByTestId('panel-state')).toHaveTextContent(
      '"mode":"create"',
    );
    expect(screen.getByTestId('panel-state')).toHaveTextContent(
      '"expanded":true',
    );
  });
});
