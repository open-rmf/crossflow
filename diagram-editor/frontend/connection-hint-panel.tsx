import { Paper, Stack, Typography } from '@mui/material';
import { Panel, useConnection } from '@xyflow/react';
import type { NodeManager } from './node-manager';
import {
  filterCompatibleAddOperations,
  getVisibleAddOperations,
} from './utils/add-operation-catalog';
import { validateConnectionQuick } from './utils/connection';
import { ROOT_NAMESPACE } from './utils/namespace';

export interface ConnectionHintPanelProps {
  nodeManager: NodeManager;
}

export function ConnectionHintPanel({ nodeManager }: ConnectionHintPanelProps) {
  const connection = useConnection();

  if (!connection.inProgress || !connection.fromHandle) {
    return null;
  }

  const sourceNode = nodeManager.tryGetNode(connection.fromHandle.nodeId);
  if (!sourceNode) {
    return null;
  }

  const compatibleOperations = filterCompatibleAddOperations(
    getVisibleAddOperations({
      isTemplateMode: false,
      namespace: ROOT_NAMESPACE,
    }),
    sourceNode,
    connection.fromHandle.id,
    {
      namespace: ROOT_NAMESPACE,
      parentId: sourceNode.parentId,
    },
  );

  let message =
    'Drop on a compatible input, or release on empty space to add a compatible next operation.';
  let tone: 'info' | 'success' | 'error' = 'info';

  if (connection.toHandle && connection.toNode) {
    const result = validateConnectionQuick(
      {
        source: connection.fromHandle.nodeId,
        sourceHandle: connection.fromHandle.id || null,
        target: connection.toHandle.nodeId,
        targetHandle: connection.toHandle.id || null,
      },
      nodeManager,
    );

    if (result.valid) {
      tone = 'success';
      message = `Compatible target: ${connection.toNode.type}`;
    } else {
      tone = 'error';
      message = result.error;
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
          <Typography variant="caption" color="text.secondary">
            Compatible next operations available: {compatibleOperations.length}
          </Typography>
        </Stack>
      </Paper>
    </Panel>
  );
}
