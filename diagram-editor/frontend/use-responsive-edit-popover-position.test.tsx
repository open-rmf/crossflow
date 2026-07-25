import { fireEvent, render, screen } from '@testing-library/react';
import {
  DiagramSidePanelProvider,
  useDiagramSidePanel,
} from './diagram-side-panel-controller';
import { useResponsiveEditPopoverPosition } from './use-responsive-edit-popover-position';

function Harness() {
  const panel = useDiagramSidePanel();
  const position = useResponsiveEditPopoverPosition({
    left: 900,
    top: 300,
  });

  return (
    <>
      <output data-testid="position">{JSON.stringify(position)}</output>
      <button
        type="button"
        onClick={() => panel.openEnvironments({ create: true })}
      >
        expand drawer
      </button>
      <button type="button" onClick={panel.close}>
        close drawer
      </button>
    </>
  );
}

function renderHarness() {
  return render(
    <DiagramSidePanelProvider>
      <Harness />
    </DiagramSidePanelProvider>,
  );
}

describe('useResponsiveEditPopoverPosition', () => {
  beforeEach(() => {
    Object.defineProperty(window, 'innerWidth', {
      configurable: true,
      writable: true,
      value: 1440,
    });
  });

  test('moves with drawer expansion and returns when the drawer closes', () => {
    renderHarness();

    expect(screen.getByTestId('position')).toHaveTextContent(
      '{"left":584,"top":300}',
    );

    fireEvent.click(screen.getByRole('button', { name: 'expand drawer' }));
    expect(screen.getByTestId('position')).toHaveTextContent(
      '{"left":104,"top":300}',
    );

    fireEvent.click(screen.getByRole('button', { name: 'close drawer' }));
    expect(screen.getByTestId('position')).toHaveTextContent(
      '{"left":900,"top":300}',
    );
  });

  test('repositions when the viewport width changes', () => {
    renderHarness();

    window.innerWidth = 1200;
    fireEvent(window, new Event('resize'));

    expect(screen.getByTestId('position')).toHaveTextContent(
      '{"left":344,"top":300}',
    );
  });
});
