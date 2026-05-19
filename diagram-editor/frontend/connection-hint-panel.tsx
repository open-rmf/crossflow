import { Paper, Stack, Typography } from '@mui/material';
import { Panel, useConnection } from '@xyflow/react';
import type { NodeManager } from './node-manager';
import { useEdges } from './use-edges';
import {
  filterCompatibleAddOperations,
  getVisibleAddOperations,
} from './utils/add-operation-catalog';
import {
  createConnectionFromDraggedHandle,
  validateConnectionSimple,
  validateSourceOutputCapacity,
} from './utils/connection';
import { ROOT_NAMESPACE } from './utils/namespace';

export interface ConnectionHintPanelProps {
  nodeManager: NodeManager;
}

export function ConnectionHintPanel({ nodeManager }: ConnectionHintPanelProps) {
  const connection = useConnection();
  const edges = useEdges();

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
  const compatibleOperations = sourceOutputCapacity.valid
    ? filterCompatibleAddOperations(
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
        connection.fromHandle.type,
      )
    : [];

  let message = !sourceOutputCapacity.valid
    ? sourceOutputCapacity.error
    : connection.fromHandle.type === 'target'
      ? 'Drop on a compatible output, or release on empty space to add a compatible previous operation.'
      : 'Drop on a compatible input, or release on empty space to add a compatible next operation.';
  let tone: 'info' | 'success' | 'error' = sourceOutputCapacity.valid
    ? 'info'
    : 'error';

  if (connection.toHandle && connection.toNode) {
    const result = validateConnectionSimple(
      createConnectionFromDraggedHandle({
        fromNodeId: connection.fromHandle.nodeId,
        fromHandleId: connection.fromHandle.id,
        fromHandleType: connection.fromHandle.type,
        otherNodeId: connection.toHandle.nodeId,
        otherHandleId: connection.toHandle.id,
      }),
      nodeManager,
      edges,
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
            {connection.fromHandle.type === 'target'
              ? 'Compatible previous operations available: '
              : 'Compatible next operations available: '}
            {compatibleOperations.length}
          </Typography>
        </Stack>
      </Paper>
    </Panel>
  );
}
