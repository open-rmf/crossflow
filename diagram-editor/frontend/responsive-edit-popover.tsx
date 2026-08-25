import {
  Popover,
  type PopoverActions,
  type PopoverProps,
  useTheme,
} from '@mui/material';
import { useEffect, useRef, useState } from 'react';
import { EditPopoverWidth } from './diagram-side-panel-layout';
import { useResponsiveEditPopoverPosition } from './use-responsive-edit-popover-position';

type ResponsiveEditPopoverProps = Omit<PopoverProps, 'slotProps'>;

export function ResponsiveEditPopover({
  anchorPosition,
  ...props
}: ResponsiveEditPopoverProps) {
  const theme = useTheme();
  const responsiveAnchorPosition = useResponsiveEditPopoverPosition(
    anchorPosition ?? undefined,
  );
  const positionTransition = theme.transitions.create('left', {
    easing: theme.transitions.easing.easeInOut,
    duration: theme.transitions.duration.standard,
  });
  const preservePositionTransition = (node: HTMLElement) => {
    if (!node.style.transition.includes('left')) {
      node.style.transition = [positionTransition, node.style.transition]
        .filter(Boolean)
        .join(',');
    }
  };

  // A popover only fits itself to the viewport as it opens, so content that
  // grows afterwards - expanding a section, switching on an optional field -
  // runs off the bottom of the screen. Re-run the fit whenever it resizes.
  const actions = useRef<PopoverActions>(null);
  const [paper, setPaper] = useState<HTMLElement | null>(null);
  useEffect(() => {
    if (!paper) {
      return;
    }
    const observer = new ResizeObserver(() =>
      actions.current?.updatePosition(),
    );
    observer.observe(paper);
    return () => observer.disconnect();
  }, [paper]);

  return (
    <Popover
      {...props}
      action={actions}
      disableEnforceFocus
      anchorPosition={responsiveAnchorPosition}
      anchorOrigin={{ vertical: 'bottom', horizontal: 'left' }}
      slotProps={{
        transition: {
          onEnter: preservePositionTransition,
          onExit: preservePositionTransition,
        },
        paper: {
          ref: setPaper,
          style: {
            width: `min(${EditPopoverWidth}px, calc(100vw - 32px))`,
          },
        },
      }}
    />
  );
}
