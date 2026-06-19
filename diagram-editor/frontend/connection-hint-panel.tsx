import { Paper, Stack, Typography } from '@mui/material';
import { Panel, useConnection } from '@xyflow/react';
import React from 'react';
import { useConnectionCompatibility } from './connection-compatibility-provider';
import type { NodeManager } from './node-manager';
import { useEdges } from './use-edges';
import {
  createConnectionFromDraggedHandle,
  validateSourceOutputCapacity,
} from './utils/connection';

export interface ConnectionHintPanelProps {
  nodeManager: NodeManager;
}

export function ConnectionHintPanel({ nodeManager }: ConnectionHintPanelProps) {
  const connection = useConnection();
  const edges = useEdges();
  const candidateConnection = React.useMemo(() => {
    if (
      !connection.inProgress ||
      !connection.fromHandle ||
      !connection.toHandle
    ) {
      return null;
    }

    return createConnectionFromDraggedHandle({
      fromNodeId: connection.fromHandle.nodeId,
      fromHandleId: connection.fromHandle.id,
      fromHandleType: connection.fromHandle.type,
      otherNodeId: connection.toHandle.nodeId,
      otherHandleId: connection.toHandle.id,
    });
  }, [connection]);
  const compatibility = useConnectionCompatibility(
    candidateConnection,
    'hovered-handle',
  );

  if (!connection.inProgress || !connection.fromHandle) {
    return null;
  }

  const sourceNode = nodeManager.tryGetNode(connection.fromHandle.nodeId);
  if (!sourceNode) {
    return null;
  }

  const sourceOutputCapacity =
    connection.fromHandle.type === 'source'
      ? validateSourceOutputCapacity(
          sourceNode,
          connection.fromHandle.id,
          edges,
        )
      : { valid: true as const };

  let message = !sourceOutputCapacity.valid
    ? sourceOutputCapacity.error
    : connection.fromHandle.type === 'target'
      ? 'Drop on a compatible output, or release on empty space to add a compatible previous operation.'
      : 'Drop on a compatible input, or release on empty space to add a compatible next operation.';
  let tone: 'info' | 'success' | 'error' = sourceOutputCapacity.valid
    ? 'info'
    : 'error';

  if (connection.toHandle && connection.toNode && compatibility) {
    if (compatibility.status === 'compatible') {
      tone = 'success';
      message = compatibility.provisional
        ? `${compatibility.reason} This connection is allowed provisionally; final compilation may still need more type context.`
        : compatibility.reason;
    } else {
      tone = 'error';
      message = compatibility.reason;
    }
  }

  return (
    <Panel position="top-left">
      <Paper
        elevation={3}
        sx={{
          px: 2,
          py: 1.5,
          width: 320,
          border: 1,
          borderColor:
            tone === 'success'
              ? 'success.main'
              : tone === 'error'
                ? 'error.main'
                : 'divider',
        }}
      >
        <Stack spacing={0.5}>
          <Typography variant="subtitle2">Connection Helper</Typography>
          <Typography variant="body2">{message}</Typography>
          {compatibility?.sourceType && (
            <Typography variant="caption" color="text.secondary">
              Source: {compatibility.sourceType}
            </Typography>
          )}
          {compatibility?.targetType && (
            <Typography variant="caption" color="text.secondary">
              Target: {compatibility.targetType}
            </Typography>
          )}
        </Stack>
      </Paper>
    </Panel>
  );
}
