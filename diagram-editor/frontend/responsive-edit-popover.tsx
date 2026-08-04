import { Popover, type PopoverProps, useTheme } from '@mui/material';
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

  return (
    <Popover
      {...props}
      disableEnforceFocus
      anchorPosition={responsiveAnchorPosition}
      anchorOrigin={{ vertical: 'bottom', horizontal: 'left' }}
      slotProps={{
        transition: {
          onEnter: preservePositionTransition,
          onExit: preservePositionTransition,
        },
        paper: {
          style: {
            width: `min(${EditPopoverWidth}px, calc(100vw - 32px))`,
          },
        },
      }}
    />
  );
}
