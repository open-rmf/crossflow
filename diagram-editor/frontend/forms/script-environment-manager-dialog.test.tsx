import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { of } from 'rxjs';
import { ApiClient } from '../api-client';
import { ApiClientProvider } from '../api-client-provider';
import { DiagramPropertiesProvider } from '../diagram-properties-provider';
import { NodeManager, NodeManagerProvider } from '../node-manager';
import { NotificationProvider } from '../notification-provider';
import { RegistryProvider } from '../registry-provider';
import type { DiagramElementMetadata } from '../types/api';
import { ScriptEnvironmentManagerDialog } from './script-environment-manager-dialog';

const testRegistry: DiagramElementMetadata = {
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
  scripting: {
    'process-bound-python': {
      language: 'python',
      interpreter: 'python3',
      display_text: 'Python',
      description: 'Process-bound Python interpreter',
      config_schema: {
        properties: {
          ownership: {
            enum: ['shared', 'persistent', 'isolated'],
          },
          script: {
            type: 'string',
          },
        },
      },
      config_examples: [
        {
          name: 'Shared Python Environment',
          description: 'Reused environment with shared state',
          config: {
            ownership: 'shared',
            script: 'print("Hello from example")',
          } as any,
          run: 'execute',
        },
      ],
    },
    'generic-script-builder': {
      language: 'bash',
      interpreter: '/bin/bash',
      display_text: 'Bash Script',
      description: 'Run bash scripts',
      config_schema: {
        properties: {
          command: {
            type: 'string',
          },
          script_content: {
            type: 'string',
          },
        },
      },
      config_examples: [],
    },
  },
};

function renderDialog(
  ui: ReactNode,
  registry: DiagramElementMetadata = testRegistry,
) {
  const apiClient = new ApiClient();
  jest.spyOn(apiClient, 'getRegistry').mockReturnValue(of(registry));
  const nodeManager = new NodeManager([]);

  return render(
    <ApiClientProvider value={apiClient}>
      <RegistryProvider>
        <NodeManagerProvider value={nodeManager}>
          <DiagramPropertiesProvider>
            <NotificationProvider>{ui}</NotificationProvider>
          </DiagramPropertiesProvider>
        </NodeManagerProvider>
      </RegistryProvider>
    </ApiClientProvider>,
  );
}

describe('ScriptEnvironmentManagerDialog', () => {
  test('opens and displays creation form when creating', async () => {
    renderDialog(
      <ScriptEnvironmentManagerDialog open={true} onClose={jest.fn()} />,
    );

    // Click Create New button
    const createButton = screen.getByRole('button', {
      name: /Create new environment/i,
    });
    fireEvent.click(createButton);

    // Expect Environment Name input to appear
    expect(screen.getByLabelText('Environment Name')).toBeTruthy();
  });

  test('renders custom properties form for process-bound-python', async () => {
    renderDialog(
      <ScriptEnvironmentManagerDialog open={true} onClose={jest.fn()} />,
    );

    const createButton = screen.getByRole('button', {
      name: /Create new environment/i,
    });
    fireEvent.click(createButton);

    // Select 'process-bound-python' builder
    const builderSelect = screen.getByRole('combobox', { name: /builder/i });
    fireEvent.mouseDown(builderSelect);

    const option = screen.getByRole('option', { name: 'process-bound-python' });
    fireEvent.click(option);

    // Verifies that 'Environment Ownership' select dropdown is rendered
    await waitFor(() => {
      expect(screen.getByLabelText('Environment Ownership')).toBeTruthy();
    });

    // Verifies that 'Builder Config (JSON)' and 'Code Field' are NOT rendered
    expect(screen.queryByLabelText('Builder Config (JSON)')).toBeNull();
    expect(screen.queryByLabelText('Code Field')).toBeNull();
  });

  test('renders raw JSON editor and Code Field selection for generic script builders', async () => {
    renderDialog(
      <ScriptEnvironmentManagerDialog open={true} onClose={jest.fn()} />,
    );

    const createButton = screen.getByRole('button', {
      name: /Create new environment/i,
    });
    fireEvent.click(createButton);

    // Select 'generic-script-builder'
    const builderSelect = screen.getByRole('combobox', { name: /builder/i });
    fireEvent.mouseDown(builderSelect);

    const option = screen.getByRole('option', {
      name: 'generic-script-builder',
    });
    fireEvent.click(option);

    // Verifies that generic controls are rendered
    await waitFor(() => {
      expect(screen.getByLabelText('Builder Config (JSON)')).toBeTruthy();
      expect(screen.getByLabelText('Code Field')).toBeTruthy();
    });

    // Verifies custom ownership form is NOT rendered
    expect(screen.queryByLabelText('Environment Ownership')).toBeNull();
  });

  test('automatically populates example script and config on builder selection', async () => {
    renderDialog(
      <ScriptEnvironmentManagerDialog open={true} onClose={jest.fn()} />,
    );

    const createButton = screen.getByRole('button', {
      name: /Create new environment/i,
    });
    fireEvent.click(createButton);

    // Select 'process-bound-python'
    const builderSelect = screen.getByRole('combobox', { name: /builder/i });
    fireEvent.mouseDown(builderSelect);

    const option = screen.getByRole('option', { name: 'process-bound-python' });
    fireEvent.click(option);

    // Verifies ownership is pre-populated with 'shared' (from the first example config)
    await waitFor(() => {
      expect(screen.getByText('Shared')).toBeTruthy();
    });

    // Verifies script description text is visible in CodeMirror context
    expect(screen.getByText(/Script \(script\)/i)).toBeTruthy();
  });
});
