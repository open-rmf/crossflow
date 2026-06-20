import {
  Box,
  Button,
  ButtonGroup,
  CircularProgress,
  Stack,
  styled,
  TextField,
  Typography,
} from '@mui/material';
import type { XYPosition } from '@xyflow/react';
import React from 'react';
import type { AddOperationSelection } from './add-operation';
import { useCompatibilityChecker } from './connection-compatibility-provider';
import { EditorMode, useEditorMode } from './editor-mode';
import { useNodeManager } from './node-manager';
import { isOperationNode, NodeIcon } from './nodes';
import { useRegistry } from './registry-provider';
import {
  type AddOperationCandidate,
  filterCompatibleAddOperations,
  getAddOperationCandidates,
  getVisibleAddOperations,
} from './utils/add-operation-catalog';
import { createConnectionFromDraggedHandle } from './utils/connection';
import { joinNamespaces, ROOT_NAMESPACE } from './utils/namespace';

const StyledOperationButton = styled(Button)({
  justifyContent: 'flex-start',
});

export interface CompatibleAddOperationProps {
  parentId?: string;
  newNodePosition: XYPosition;
  sourceConnection: {
    sourceNodeId: string;
    sourceHandle: string | null;
    sourceHandleType: 'source' | 'target';
  };
  onAdd?: (selection: AddOperationSelection) => void;
}

export function CompatibleAddOperation({
  parentId,
  newNodePosition,
  sourceConnection,
  onAdd,
}: CompatibleAddOperationProps) {
  const registry = useRegistry();
  const nodeManager = useNodeManager();
  const checker = useCompatibilityChecker();
  const [editorMode] = useEditorMode();
  const [search, setSearch] = React.useState('');
  const [compatibleCandidates, setCompatibleCandidates] = React.useState<
    AddOperationCandidate[] | null
  >(null);

  const namespace = React.useMemo(() => {
    const parentNode = parentId && nodeManager.tryGetNode(parentId);
    if (!parentNode || !isOperationNode(parentNode)) {
      return ROOT_NAMESPACE;
    }
    return joinNamespaces(parentNode.data.namespace, parentNode.data.opId);
  }, [parentId, nodeManager]);

  const candidates = React.useMemo(() => {
    const sourceNode = nodeManager.tryGetNode(sourceConnection.sourceNodeId);
    if (!sourceNode) {
      return [];
    }

    const visibleBuiltins = getVisibleAddOperations({
      isTemplateMode: editorMode.mode === EditorMode.Template,
      namespace,
    });
    const allCandidates = getAddOperationCandidates(registry, {
      includeGenericNode: false,
      includeRegistryNodes: true,
      includeBuiltins: true,
      builtins: visibleBuiltins,
    });

    return filterCompatibleAddOperations(
      allCandidates,
      sourceNode,
      sourceConnection.sourceHandle,
      { namespace, parentId },
      sourceConnection.sourceHandleType,
    );
  }, [
    editorMode.mode,
    namespace,
    nodeManager,
    parentId,
    registry,
    sourceConnection.sourceHandle,
    sourceConnection.sourceHandleType,
    sourceConnection.sourceNodeId,
  ]);

  React.useEffect(() => {
    let active = true;
    setCompatibleCandidates(null);

    const checks = candidates.flatMap((candidate) => {
      const changes = candidate.createChanges({
        namespace,
        parentId,
        newNodePosition,
        nodeManager,
      });
      const primaryNode = changes[0]?.item;
      if (!primaryNode) {
        return [];
      }

      return [
        {
          id: candidate.key,
          connection: createConnectionFromDraggedHandle({
            fromNodeId: sourceConnection.sourceNodeId,
            fromHandleId: sourceConnection.sourceHandle,
            fromHandleType: sourceConnection.sourceHandleType,
            otherNodeId: primaryNode.id,
            otherHandleId: null,
          }),
          nodeChanges: changes,
        },
      ];
    });

    checker
      .checkConnections(checks)
      .then((results) => {
        if (!active) {
          return;
        }
        setCompatibleCandidates(
          candidates.filter(
            (candidate) => results.get(candidate.key)?.status === 'compatible',
          ),
        );
      })
      .catch(() => {
        if (active) {
          setCompatibleCandidates([]);
        }
      });

    return () => {
      active = false;
    };
  }, [
    candidates,
    checker,
    namespace,
    nodeManager,
    parentId,
    newNodePosition,
    sourceConnection,
  ]);

  const operations = React.useMemo(() => {
    if (!compatibleCandidates) {
      return null;
    }

    const trimmedSearch = search.trim().toLowerCase();
    if (!trimmedSearch) {
      return compatibleCandidates;
    }

    return compatibleCandidates.filter((operation) =>
      operation.label.toLowerCase().includes(trimmedSearch),
    );
  }, [compatibleCandidates, search]);

  const title =
    sourceConnection.sourceHandleType === 'target'
      ? 'Compatible previous operations'
      : 'Compatible next operations';

  if (!operations) {
    return (
      <Box sx={{ p: 1.5, display: 'flex', alignItems: 'center', gap: 1 }}>
        <CircularProgress size={16} />
        <Typography variant="body2">
          Checking compatible operations...
        </Typography>
      </Box>
    );
  }

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
          aria-label="Add compatible operation button group"
          sx={{ width: '100%' }}
        >
          {operations.map((operation) => (
            <StyledOperationButton
              key={operation.key}
              startIcon={<NodeIcon />}
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
      {operations.length === 0 && (
        <Typography variant="body2">
          {search.trim()
            ? 'No compatible operations match this filter.'
            : 'No compatible operations are available here yet.'}
        </Typography>
      )}
    </Stack>
  );
}
