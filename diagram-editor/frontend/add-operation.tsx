import { Button, ButtonGroup, TextField, Typography, styled } from '@mui/material';
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
  const operations = React.useMemo(() => {
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
      );
    }

    const trimmedSearch = search.trim().toLowerCase();
    if (!trimmedSearch) {
      return visible;
    }

    return visible.filter((operation) =>
      operation.label.toLowerCase().includes(trimmedSearch),
    );
  }, [editorMode.mode, namespace, sourceNode, sourceConnection?.sourceHandle, parentId, search]);

  return (
    <>
      <Typography variant="subtitle2" sx={{ px: 1.5, pt: 1.5, pb: 0.5 }}>
        {sourceConnection ? 'Compatible next operations' : 'Add operation'}
      </Typography>
      <TextField
        size="small"
        placeholder="Filter operations"
        value={search}
        onChange={(event) => setSearch(event.target.value)}
        sx={{ px: 1.5, pb: 1, width: 260 }}
      />
      <ButtonGroup
        orientation="vertical"
        variant="contained"
        size="small"
        aria-label="Add operation button group"
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
      {operations.length === 0 && (
        <Typography variant="body2" sx={{ px: 1.5, py: 1.5, width: 260 }}>
          No compatible operations are available for this output yet.
        </Typography>
      )}
    </>
  );
}

export default AddOperation;
