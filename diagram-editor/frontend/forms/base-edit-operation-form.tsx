import {
  Box,
  Card,
  CardContent,
  CardHeader,
  IconButton,
  Stack,
  TextField,
} from '@mui/material';
import type { NodeChange, NodeRemoveChange } from '@xyflow/react';
import type React from 'react';
import type { OperationNode, OperationNodeTypes } from '../nodes';
import { MaterialSymbol } from '../nodes';

export interface BaseEditOperationFormProps<
  NodeType extends OperationNodeTypes = OperationNodeTypes,
> {
  node: OperationNode<NodeType>;
  onChange?: (changes: NodeChange<OperationNode>) => void;
  onDelete?: (change: NodeRemoveChange) => void;
  sidePanel?: React.ReactNode;
}

function BaseEditOperationForm({
  node,
  onChange,
  onDelete,
  sidePanel,
  children,
}: React.PropsWithChildren<BaseEditOperationFormProps>) {
  return (
    <Card>
      <CardHeader
        title="Edit Operation"
        action={
          <IconButton
            color="error"
            onClick={() => onDelete?.({ type: 'remove', id: node.id })}
          >
            <MaterialSymbol symbol="delete" />
          </IconButton>
        }
      />
      <CardContent>
        <Box
          sx={{
            display: 'grid',
            gridTemplateColumns: sidePanel
              ? { xs: 'minmax(0, 1fr)', md: '360px minmax(320px, 420px)' }
              : 'minmax(0, 1fr)',
            gap: sidePanel ? 3 : 0,
            maxWidth: 'calc(100vw - 32px)',
          }}
        >
          <Stack spacing={2}>
            <TextField
              required
              label="id"
              fullWidth
              value={node.data.opId}
              onChange={(ev) => {
                const updatedNode = { ...node };
                updatedNode.data.opId = ev.target.value;
                onChange?.({
                  type: 'replace',
                  id: node.id,
                  item: updatedNode,
                });
              }}
            />
            {children}
          </Stack>
          {sidePanel}
        </Box>
      </CardContent>
    </Card>
  );
}

export default BaseEditOperationForm;
