import { fireEvent, render, screen, waitFor } from '@testing-library/react';
import {
  DiagramSidePanelProvider,
  useDiagramSidePanel,
} from './diagram-side-panel-controller';
import { ResponsiveEditPopover } from './responsive-edit-popover';

function Harness() {
  const panel = useDiagramSidePanel();

  return (
    <>
      <ResponsiveEditPopover
        open
        anchorReference="anchorPosition"
        anchorPosition={{ left: 900, top: 300 }}
      >
        editor
      </ResponsiveEditPopover>
      <button
        type="button"
        onClick={() => panel.openEnvironments({ create: true })}
      >
        expand drawer
      </button>
    </>
  );
}

describe('ResponsiveEditPopover', () => {
  beforeEach(() => {
    Object.defineProperty(window, 'innerWidth', {
      configurable: true,
      writable: true,
      value: 1440,
    });
  });

  test('preserves a left transition while moving with the drawer', async () => {
    render(
      <DiagramSidePanelProvider>
        <Harness />
      </DiagramSidePanelProvider>,
    );

    const paper = document.querySelector('.MuiPopover-paper') as HTMLElement;

    await waitFor(() => {
      expect(paper.style.left).toBe('584px');
      expect(getComputedStyle(paper).transition).toContain('left');
    });

    fireEvent.click(
      screen.getByRole('button', { name: 'expand drawer', hidden: true }),
    );

    await waitFor(() => {
      expect(paper.style.left).toBe('104px');
      expect(getComputedStyle(paper).transition).toContain('left');
    });
  });
});
