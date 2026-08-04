import { fireEvent, render, screen } from '@testing-library/react';
import { useEffect } from 'react';
import {
  DiagramPropertiesProvider,
  useDiagramProperties,
} from '../diagram-properties-provider';
import {
  DiagramSidePanelProvider,
  useDiagramSidePanel,
} from '../diagram-side-panel-controller';
import { useScriptEnvironmentNavigation } from './use-script-environment-navigation';

function Harness({ hasEnvironments }: { hasEnvironments: boolean }) {
  const [, setProperties] = useDiagramProperties();
  const openForScript = useScriptEnvironmentNavigation();
  const panel = useDiagramSidePanel();

  useEffect(() => {
    setProperties({
      script_environments: hasEnvironments
        ? {
            analysis: {
              builder: 'process-bound-python',
              config: {},
            },
          }
        : {},
    });
  }, [hasEnvironments, setProperties]);

  return (
    <>
      <output data-testid="state">{JSON.stringify(panel.state)}</output>
      <button type="button" onClick={() => openForScript('analysis', false)}>
        existing script
      </button>
      <button type="button" onClick={() => openForScript(undefined, true)}>
        new script
      </button>
    </>
  );
}

function renderHarness(hasEnvironments: boolean) {
  return render(
    <DiagramPropertiesProvider>
      <DiagramSidePanelProvider>
        <Harness hasEnvironments={hasEnvironments} />
      </DiagramSidePanelProvider>
    </DiagramPropertiesProvider>,
  );
}

describe('useScriptEnvironmentNavigation', () => {
  test('existing Script node opens its assigned environment in view mode', () => {
    renderHarness(true);

    fireEvent.click(screen.getByRole('button', { name: 'existing script' }));

    expect(screen.getByTestId('state')).toHaveTextContent(
      '"tab":"environments"',
    );
    expect(screen.getByTestId('state')).toHaveTextContent(
      '"selectedEnvironmentName":"analysis"',
    );
    expect(screen.getByTestId('state')).toHaveTextContent('"mode":"view"');
  });

  test('first new Script node starts environment creation', () => {
    renderHarness(false);

    fireEvent.click(screen.getByRole('button', { name: 'new script' }));

    expect(screen.getByTestId('state')).toHaveTextContent('"mode":"create"');
    expect(screen.getByTestId('state')).toHaveTextContent('"expanded":true');
  });

  test('new Script node uses view mode when environments exist', () => {
    renderHarness(true);

    fireEvent.click(screen.getByRole('button', { name: 'new script' }));

    expect(screen.getByTestId('state')).toHaveTextContent('"mode":"view"');
    expect(screen.getByTestId('state')).toHaveTextContent('"expanded":false');
  });
});
