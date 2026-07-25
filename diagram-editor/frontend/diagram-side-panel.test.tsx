import { createTheme, ThemeProvider } from '@mui/material';
import { render, screen } from '@testing-library/react';
import { useEffect } from 'react';
import { DiagramPropertiesProvider } from './diagram-properties-provider';
import DiagramSidePanel from './diagram-side-panel';
import {
  DiagramSidePanelProvider,
  useDiagramSidePanel,
} from './diagram-side-panel-controller';

jest.mock('./run-button', () => ({
  RunPanel: () => null,
}));

jest.mock('./forms/script-environment-workspace', () => ({
  ScriptEnvironmentWorkspace: () => null,
}));

function OpenCreateMode() {
  const { openEnvironments } = useDiagramSidePanel();

  useEffect(() => {
    openEnvironments({ create: true });
  }, [openEnvironments]);

  return null;
}

function renderSidePanel({ create = false }: { create?: boolean } = {}) {
  return render(
    <ThemeProvider theme={createTheme({ palette: { mode: 'dark' } })}>
      <DiagramPropertiesProvider>
        <DiagramSidePanelProvider>
          {create && <OpenCreateMode />}
          <DiagramSidePanel
            runRequestJson=""
            onRunRequestJsonChange={jest.fn()}
          />
        </DiagramSidePanelProvider>
      </DiagramPropertiesProvider>
    </ThemeProvider>,
  );
}

describe('DiagramSidePanel', () => {
  test('renders Environments beside Properties and Run', () => {
    renderSidePanel();

    expect(
      screen.getByRole('tab', { name: /Properties/i }),
    ).toBeInTheDocument();
    expect(screen.getByRole('tab', { name: /Run/i })).toBeInTheDocument();
    expect(
      screen.getByRole('tab', { name: /Environments/i }),
    ).toBeInTheDocument();
  });

  test('marks the drawer expanded and applies a width transition', async () => {
    renderSidePanel({ create: true });

    const paper = await screen.findByTestId('diagram-side-panel-paper');
    expect(paper).toHaveAttribute('data-expanded', 'true');
    expect(paper.style.transition).toContain('width');
  });
});
