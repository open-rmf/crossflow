import {
  constrainEditPopoverPosition,
  getDiagramSidePanelWidth,
} from './diagram-side-panel-layout';

describe('diagram side-panel layout', () => {
  test('uses the normal and expanded drawer widths when space permits', () => {
    expect(
      getDiagramSidePanelWidth(1440, { open: true, expanded: false }),
    ).toBe(420);
    expect(getDiagramSidePanelWidth(1440, { open: true, expanded: true })).toBe(
      900,
    );
  });

  test('leaves 56 pixels of canvas visible on narrow viewports', () => {
    expect(getDiagramSidePanelWidth(800, { open: true, expanded: true })).toBe(
      744,
    );
  });

  test('moves an overlapping editor left of the normal drawer', () => {
    expect(
      constrainEditPopoverPosition({
        anchorPosition: { left: 900, top: 300 },
        viewportWidth: 1440,
        sidePanel: { open: true, expanded: false },
      }),
    ).toEqual({ left: 584, top: 300 });
  });

  test('moves the editor farther left when the drawer expands', () => {
    expect(
      constrainEditPopoverPosition({
        anchorPosition: { left: 900, top: 300 },
        viewportWidth: 1440,
        sidePanel: { open: true, expanded: true },
      }),
    ).toEqual({ left: 104, top: 300 });
  });

  test('keeps a safe node anchor unchanged', () => {
    expect(
      constrainEditPopoverPosition({
        anchorPosition: { left: 80, top: 300 },
        viewportWidth: 1440,
        sidePanel: { open: true, expanded: true },
      }),
    ).toEqual({ left: 80, top: 300 });
  });

  test('restores the original safe anchor when the drawer closes', () => {
    expect(
      constrainEditPopoverPosition({
        anchorPosition: { left: 900, top: 300 },
        viewportWidth: 1440,
        sidePanel: { open: false, expanded: false },
      }),
    ).toEqual({ left: 900, top: 300 });
  });

  test('clamps to the viewport margin when side-by-side space is exhausted', () => {
    expect(
      constrainEditPopoverPosition({
        anchorPosition: { left: 700, top: 300 },
        viewportWidth: 800,
        sidePanel: { open: true, expanded: true },
      }),
    ).toEqual({ left: 16, top: 300 });
  });
});
