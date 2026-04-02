import { Button, ButtonGroup, Stack, TextField, Typography, styled } from '@mui/material';
import type { NodeAddChange, XYPosition } from '@xyflow/react';
import React from 'react';
import { EditorMode, useEditorMode } from './editor-mode';
import { useNodeManager } from './node-manager';
import type { DiagramEditorNode } from './nodes';
import {
  BufferAccessIcon,
  BufferIcon,
  ForkCloneIcon,
  ForkResultIcon,
  isOperationNode,
  JoinIcon,
  ListenIcon,
  NodeIcon,
  ScopeIcon,
  SectionBufferIcon,
  SectionIcon,
  SectionInputIcon,
  SectionOutputIcon,
  SplitIcon,
  StreamOutIcon,
  TransformIcon,
  UnzipIcon,
} from './nodes';
import {
  type AddOperationKey,
  filterCompatibleAddOperations,
  getVisibleAddOperations,
} from './utils/add-operation-catalog';
import { joinNamespaces, ROOT_NAMESPACE } from './utils/namespace';

const StyledOperationButton = styled(Button)({
  justifyContent: 'flex-start',
});

const OPERATION_ICONS: Record<AddOperationKey, React.ReactNode> = {
  sectionInput: <SectionInputIcon />,
  sectionOutput: <SectionOutputIcon />,
  sectionBuffer: <SectionBufferIcon />,
  node: <NodeIcon />,
  fork_clone: <ForkCloneIcon />,
  unzip: <UnzipIcon />,
  fork_result: <ForkResultIcon />,
  split: <SplitIcon />,
  join: <JoinIcon />,
  transform: <TransformIcon />,
  buffer: <BufferIcon />,
  buffer_access: <BufferAccessIcon />,
  listen: <ListenIcon />,
  stream_out: <StreamOutIcon />,
  scope: <ScopeIcon />,
  section: <SectionIcon />,
};

export interface AddOperationSelection {
  primaryNodeId: string;
  changes: NodeAddChange<DiagramEditorNode>[];
}

export interface AddOperationProps {
  parentId?: string;
  newNodePosition: XYPosition;
  sourceConnection?: {
    sourceNodeId: string;
    sourceHandle: string | null;
    sourceHandleType: 'source' | 'target';
  } | null;
  onAdd?: (selection: AddOperationSelection) => void;
}

function AddOperation({
  parentId,
  newNodePosition,
  sourceConnection,
  onAdd,
}: AddOperationProps) {
  const [editorMode] = useEditorMode();
  const nodeManager = useNodeManager();
  const [search, setSearch] = React.useState('');
  const namespace = React.useMemo(() => {
    const parentNode = parentId && nodeManager.tryGetNode(parentId);
    if (!parentNode || !isOperationNode(parentNode)) {
      return ROOT_NAMESPACE;
    }
    return joinNamespaces(parentNode.data.namespace, parentNode.data.opId);
  }, [parentId, nodeManager]);
  const sourceNode = sourceConnection
    ? nodeManager.tryGetNode(sourceConnection.sourceNodeId)
    : null;
  const compatibleOperations = React.useMemo(() => {
    let visible = getVisibleAddOperations({
      isTemplateMode: editorMode.mode === EditorMode.Template,
      namespace,
    });

    if (sourceNode) {
      visible = filterCompatibleAddOperations(
        visible,
        sourceNode,
        sourceConnection?.sourceHandle,
        { namespace, parentId },
        sourceConnection?.sourceHandleType,
      );
    }

    return visible;
  }, [
    editorMode.mode,
    namespace,
    sourceNode,
    sourceConnection?.sourceHandle,
    sourceConnection?.sourceHandleType,
    parentId,
  ]);

  const operations = React.useMemo(() => {
    const trimmedSearch = search.trim().toLowerCase();
    if (!trimmedSearch) {
      return compatibleOperations;
    }

    return compatibleOperations.filter((operation) =>
      operation.label.toLowerCase().includes(trimmedSearch),
    );
  }, [compatibleOperations, search]);

  const emptyMessage = React.useMemo(() => {
    if (operations.length > 0) {
      return null;
    }

    if (search.trim()) {
      return sourceConnection
        ? sourceConnection.sourceHandleType === 'target'
          ? 'No compatible input operations match this filter.'
          : 'No compatible output operations match this filter.'
        : 'No operations match this filter.';
    }

    return sourceConnection
      ? sourceConnection.sourceHandleType === 'target'
        ? 'No compatible operations are available for this input yet.'
        : 'No compatible operations are available for this output yet.'
      : 'No operations are available here yet.';
  }, [operations.length, search, sourceConnection]);

  const title = sourceConnection
    ? sourceConnection.sourceHandleType === 'target'
      ? 'Compatible previous operations'
      : 'Compatible next operations'
    : 'Add operation';

  return (
    <Stack spacing={1} sx={{ px: 1.5, pt: 1.5, pb: 1.5, width: 260 }}>
      <Typography variant="subtitle2">{title}</Typography>
      <TextField
        size="small"
        placeholder="Filter operations"
        value={search}
        onChange={(event) => setSearch(event.target.value)}
      />
      {operations.length > 0 && (
        <ButtonGroup
          orientation="vertical"
          variant="contained"
          size="small"
          aria-label="Add operation button group"
          sx={{ width: '100%' }}
        >
          {operations.map((operation) => (
            <StyledOperationButton
              key={operation.key}
              startIcon={OPERATION_ICONS[operation.key]}
              onClick={() => {
                const changes = operation.createChanges({
                  namespace,
                  parentId,
                  newNodePosition,
                  nodeManager,
                });
                const primaryNodeId = changes[0]?.item.id;
                if (!primaryNodeId) {
                  return;
                }
                onAdd?.({ changes, primaryNodeId });
              }}
            >
              {operation.label}
            </StyledOperationButton>
          ))}
        </ButtonGroup>
      )}
      {emptyMessage && (
        <Typography variant="body2">
          {emptyMessage}
        </Typography>
      )}
    </Stack>
  );
}

export default AddOperation;
