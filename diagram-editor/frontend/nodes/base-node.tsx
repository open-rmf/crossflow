import {
  alpha,
  Box,
  Button,
  type ButtonProps,
  Paper,
  Stack,
  Typography,
} from '@mui/material';
import type { NodeProps } from '@xyflow/react';
import { type JSX, memo } from 'react';
import { useDebugVisualization } from '../debug-visualization-provider';
import { LAYOUT_OPTIONS } from '../utils/layout';

export interface BaseNodeProps extends NodeProps {
  color?: ButtonProps['color'];
  icon?: React.JSX.Element | string;
  label: string;
  caption?: string;
  handles?: JSX.Element;
  highlight?: boolean;
}

function BaseNode({
  color,
  icon: materialIconOrSymbol,
  label,
  caption,
  handles,
  selected,
  highlight,
  id,
}: BaseNodeProps) {
  const { activeNodeIds, latestNodeId } = useDebugVisualization();
  const debugLatest = latestNodeId === id;
  const debugVisited = activeNodeIds.has(id) && !debugLatest;
  const icon =
    typeof materialIconOrSymbol === 'string' ? (
      <span className={`material-symbols-${materialIconOrSymbol}`} />
    ) : (
      materialIconOrSymbol
    );

  return (
    <Paper
      sx={(theme) => ({
        border: highlight ? '2px solid' : undefined,
        borderColor: highlight ? 'warning.main' : undefined,
        outline: debugLatest
          ? `2px solid ${theme.palette.success.main}`
          : debugVisited
            ? `1px solid ${alpha(theme.palette.info.main, 0.35)}`
            : undefined,
        boxShadow: debugLatest
          ? [
              `0 0 0 4px ${alpha(theme.palette.success.main, 0.28)}`,
              `0 0 18px 6px ${alpha(theme.palette.success.main, 0.35)}`,
            ].join(', ')
          : undefined,
        transition: theme.transitions.create(['box-shadow', 'outline-color'], {
          duration: theme.transitions.duration.shortest,
        }),
      })}
    >
      <Button
        title={label}
        color={color}
        fullWidth
        startIcon={icon}
        variant={selected ? 'contained' : 'outlined'}
        sx={{
          textTransform: 'none',
          width: LAYOUT_OPTIONS.nodeWidth,
          height: LAYOUT_OPTIONS.nodeHeight,
        }}
      >
        <Stack>
          <Box
            component="span"
            sx={{
              minWidth: 0,
              overflow: 'hidden',
              textOverflow: 'ellipsis',
              whiteSpace: 'nowrap',
            }}
          >
            {label}
          </Box>
          {caption && (
            <Typography
              variant="caption"
              fontSize={8}
              sx={{
                minWidth: 0,
                overflow: 'hidden',
                textOverflow: 'ellipsis',
                whiteSpace: 'nowrap',
              }}
            >
              {caption}
            </Typography>
          )}
        </Stack>
      </Button>
      {handles}
    </Paper>
  );
}

export default memo(BaseNode);
