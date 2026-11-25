import { Tooltip, Typography, type TypographyProps } from '@mui/material';
import { useRef, useState } from 'react';

export default function TypeLabel({
  children,
  component = 'span',
  sx,
}: {
  children: string | undefined;
  component?: 'dd' | 'span';
  sx?: TypographyProps['sx'];
}) {
  const label = useRef<HTMLElement>(null);
  const [open, setOpen] = useState(false);
  return (
    <Tooltip
      title={children}
      open={open}
      onOpen={() => {
        const el = label.current;
        if (el) setOpen(el.scrollWidth > el.clientWidth);
      }}
      onClose={() => setOpen(false)}
    >
      <Typography
        ref={label}
        component={component}
        variant="body2"
        color="text.secondary"
        align="right"
        maxWidth="12ch"
        justifySelf="end"
        noWrap
        sx={sx}
      >
        {children}
      </Typography>
    </Tooltip>
  );
}
