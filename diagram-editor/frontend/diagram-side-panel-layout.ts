export interface SidePanelLayoutState {
  open: boolean;
  expanded: boolean;
}

export interface EditorAnchorPosition {
  left: number;
  top: number;
}

export const NormalSidePanelWidth = 420;
export const ExpandedSidePanelWidth = 900;
export const MinimumVisibleCanvasWidth = 56;
export const EditPopoverWidth = 420;
export const EditPopoverMargin = 16;
export const EditPopoverSidePanelGap = 16;

export function getDiagramSidePanelWidth(
  viewportWidth: number,
  sidePanel: SidePanelLayoutState,
): number {
  if (!sidePanel.open) {
    return 0;
  }

  return Math.min(
    sidePanel.expanded ? ExpandedSidePanelWidth : NormalSidePanelWidth,
    Math.max(0, viewportWidth - MinimumVisibleCanvasWidth),
  );
}

export function constrainEditPopoverPosition({
  anchorPosition,
  viewportWidth,
  sidePanel,
}: {
  anchorPosition: EditorAnchorPosition;
  viewportWidth: number;
  sidePanel: SidePanelLayoutState;
}): EditorAnchorPosition {
  const sidePanelWidth = getDiagramSidePanelWidth(viewportWidth, sidePanel);
  const rightBoundary =
    viewportWidth -
    sidePanelWidth -
    (sidePanelWidth > 0 ? EditPopoverSidePanelGap : EditPopoverMargin);
  const maximumLeft = rightBoundary - EditPopoverWidth;

  return {
    left: Math.max(
      EditPopoverMargin,
      Math.min(anchorPosition.left, maximumLeft),
    ),
    top: anchorPosition.top,
  };
}
