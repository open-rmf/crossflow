import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import { type ReactNode, useEffect } from 'react';
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
import { NotificationProvider } from '../notification-provider';
import { RegistryProvider } from '../registry-provider';
import type { DiagramElementMetadata } from '../types/api';
import { ScriptEnvironmentWorkspace } from './script-environment-workspace';

jest.mock('@uiw/react-codemirror', () => ({
  __esModule: true,
  default: ({
    value,
    onChange,
  }: {
    value: string;
    onChange: (value: string) => void;
  }) => (
    <textarea
      aria-label="Script editor"
      value={value}
      onChange={(event) => onChange(event.target.value)}
    />
  ),
}));

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
    'generic-script-builder': {
      language: 'bash',
      interpreter: '/bin/bash',
      display_text: 'Bash Script',
      description: 'Run bash scripts',
      config_schema: {
        properties: {
          script_content: {
            type: 'string',
          },
        },
      },
      config_examples: [],
    },
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
          },
          run: 'execute',
        },
      ],
    },
  },
};

function OpenWorkspace() {
  const { openEnvironments } = useDiagramSidePanel();

  useEffect(() => {
    openEnvironments({ create: true });
  }, [openEnvironments]);

  return <ScriptEnvironmentWorkspace />;
}

function SeededWorkspace() {
  const [properties, setProperties] = useDiagramProperties();
  const { openEnvironments } = useDiagramSidePanel();

  useEffect(() => {
    setProperties({
      script_environments: {
        analysis: {
          builder: 'process-bound-python',
          config: {
            ownership: 'persistent',
            script: 'def run(): pass',
          },
        },
      },
    });
    openEnvironments({ environmentName: 'analysis' });
  }, [openEnvironments, setProperties]);

  return (
    <>
      <output data-testid="highlight">
        {properties.highlightedEnvironment || 'none'}
      </output>
      <ScriptEnvironmentWorkspace />
    </>
  );
}

function MissingWorkspace() {
  const { openEnvironments } = useDiagramSidePanel();

  useEffect(() => {
    openEnvironments({ environmentName: 'missing-env' });
  }, [openEnvironments]);

  return <ScriptEnvironmentWorkspace />;
}

function PanelStateProbe() {
  const { state } = useDiagramSidePanel();
  return <output data-testid="panel-state">{JSON.stringify(state)}</output>;
}

function renderWorkspace(
  ui: ReactNode,
  registry: DiagramElementMetadata = testRegistry,
) {
  const apiClient = new ApiClient();
  jest.spyOn(apiClient, 'getRegistry').mockReturnValue(of(registry));

  return render(
    <ApiClientProvider value={apiClient}>
      <RegistryProvider>
        <NodeManagerProvider value={new NodeManager([])}>
          <DiagramPropertiesProvider>
            <DiagramSidePanelProvider>
              <NotificationProvider>{ui}</NotificationProvider>
            </DiagramSidePanelProvider>
          </DiagramPropertiesProvider>
        </NodeManagerProvider>
      </RegistryProvider>
    </ApiClientProvider>,
  );
}

describe('ScriptEnvironmentWorkspace', () => {
  test('defaults new environments to process-bound-python', async () => {
    renderWorkspace(<OpenWorkspace />);

    expect(
      await screen.findByRole('combobox', { name: 'Builder' }),
    ).toHaveTextContent('process-bound-python');
    expect(screen.getByLabelText('Environment Ownership')).toBeInTheDocument();
  });

  test('keeps the builder selectable for future runtimes', async () => {
    renderWorkspace(<OpenWorkspace />);

    const builder = await screen.findByRole('combobox', { name: 'Builder' });
    fireEvent.mouseDown(builder);
    fireEvent.click(
      screen.getByRole('option', { name: 'generic-script-builder' }),
    );

    await waitFor(() => {
      expect(
        screen.getByLabelText('Builder Config (JSON)'),
      ).toBeInTheDocument();
    });
  });

  test('selecting an environment does not highlight its nodes', async () => {
    renderWorkspace(<SeededWorkspace />);

    expect(
      await screen.findByRole('heading', { name: 'analysis' }),
    ).toBeInTheDocument();
    expect(screen.getByTestId('highlight')).toHaveTextContent('none');
  });

  test('the eye button explicitly highlights the selected environment', async () => {
    renderWorkspace(<SeededWorkspace />);

    fireEvent.click(await screen.findByRole('button', { name: 'Show nodes' }));

    expect(screen.getByTestId('highlight')).toHaveTextContent('analysis');
    expect(
      screen.getByRole('button', { name: 'Hide nodes' }),
    ).toBeInTheDocument();
  });

  test('offers recovery when a node references a missing environment', async () => {
    renderWorkspace(<MissingWorkspace />);

    expect(
      await screen.findByText('Environment “missing-env” is not defined.'),
    ).toBeInTheDocument();
    expect(
      screen.getByRole('button', { name: 'Create environment' }),
    ).toBeInTheDocument();
  });

  test('save returns the drawer to normal width', async () => {
    renderWorkspace(
      <>
        <OpenWorkspace />
        <PanelStateProbe />
      </>,
    );

    fireEvent.change(await screen.findByLabelText('Environment Name'), {
      target: { value: 'analysis' },
    });
    fireEvent.click(screen.getByRole('button', { name: 'Save' }));

    await waitFor(() => {
      expect(screen.getByTestId('panel-state')).toHaveTextContent(
        '"expanded":false',
      );
      expect(screen.getByTestId('panel-state')).toHaveTextContent(
        '"mode":"view"',
      );
    });
  });

  test('disables creation when no builders are registered', async () => {
    renderWorkspace(<OpenWorkspace />, {
      ...testRegistry,
      scripting: {},
    });

    expect(
      await screen.findByText('No script environment builders are available.'),
    ).toBeInTheDocument();
  });
});
