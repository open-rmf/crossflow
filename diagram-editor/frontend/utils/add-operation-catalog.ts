import type { NodeAddChange, XYPosition } from '@xyflow/react';
import { v4 as uuidv4 } from 'uuid';
import type { NodeManager } from '../node-manager';
import {
  createOperationNode,
  createScopeNode,
  createSectionBufferNode,
  createSectionInputNode,
  createSectionOutputNode,
  type DiagramEditorNode,
  isSectionBufferNode,
  isSectionInputNode,
  isSectionOutputNode,
} from '../nodes';
import type { DiagramOperation, NextOperation } from '../types/api';
import { getValidEdgeTypes } from './connection';
import { ROOT_NAMESPACE } from './namespace';
import { addUniqueSuffix } from './unique-value';

export type AddOperationKey =
  | 'sectionInput'
  | 'sectionOutput'
  | 'sectionBuffer'
  | 'node'
  | 'fork_clone'
  | 'unzip'
  | 'fork_result'
  | 'split'
  | 'join'
  | 'transform'
  | 'buffer'
  | 'buffer_access'
  | 'listen'
  | 'stream_out'
  | 'scope'
  | 'section';

type AddOperationDefinition = {
  key: AddOperationKey;
  label: string;
  templateOnlyRoot?: boolean;
  createPreviewNode: (
    namespace: string,
    parentId: string | undefined,
  ) => DiagramEditorNode;
  createChanges: (args: {
    namespace: string;
    parentId: string | undefined;
    newNodePosition: XYPosition;
    nodeManager: NodeManager;
  }) => NodeAddChange<DiagramEditorNode>[];
};

function createSectionInputChange(
  remappedId: string,
  targetId: NextOperation,
  position: XYPosition,
): NodeAddChange<DiagramEditorNode> {
  return {
    type: 'add',
    item: createSectionInputNode(remappedId, targetId, position),
  };
}

function createSectionOutputChange(
  outputId: string,
  position: XYPosition,
): NodeAddChange<DiagramEditorNode> {
  return {
    type: 'add',
    item: createSectionOutputNode(outputId, position),
  };
}

function createSectionBufferChange(
  remappedId: string,
  targetId: NextOperation,
  position: XYPosition,
): NodeAddChange<DiagramEditorNode> {
  return {
    type: 'add',
    item: createSectionBufferNode(remappedId, targetId, position),
  };
}

function createNodeChange(
  namespace: string,
  parentId: string | undefined,
  newNodePosition: XYPosition,
  op: DiagramOperation,
): NodeAddChange<DiagramEditorNode>[] {
  if (op.type === 'scope') {
    return createScopeNode(
      namespace,
      parentId,
      newNodePosition,
      op,
      uuidv4(),
    ).map((node) => ({ type: 'add', item: node }));
  }

  return [
    {
      type: 'add',
      item: createOperationNode(
        namespace,
        parentId,
        newNodePosition,
        op,
        uuidv4(),
      ),
    },
  ];
}

export const ADD_OPERATION_DEFINITIONS: AddOperationDefinition[] = [
  {
    key: 'sectionInput',
    label: 'Section Input',
    templateOnlyRoot: true,
    createPreviewNode: () =>
      createSectionInputNode('preview_input', { builtin: 'dispose' }, { x: 0, y: 0 }),
    createChanges: ({ newNodePosition, nodeManager }) => {
      const remappedId = addUniqueSuffix(
        'new_input',
        nodeManager.nodes
          .filter(isSectionInputNode)
          .map((n) => n.data.remappedId),
      );
      return [
        createSectionInputChange(
          remappedId,
          { builtin: 'dispose' },
          newNodePosition,
        ),
      ];
    },
  },
  {
    key: 'sectionOutput',
    label: 'Section Output',
    templateOnlyRoot: true,
    createPreviewNode: () => createSectionOutputNode('preview_output', { x: 0, y: 0 }),
    createChanges: ({ newNodePosition, nodeManager }) => {
      const outputId = addUniqueSuffix(
        'new_output',
        nodeManager.nodes
          .filter(isSectionOutputNode)
          .map((n) => n.data.outputId),
      );
      return [createSectionOutputChange(outputId, newNodePosition)];
    },
  },
  {
    key: 'sectionBuffer',
    label: 'Section Buffer',
    templateOnlyRoot: true,
    createPreviewNode: () =>
      createSectionBufferNode('preview_buffer', { builtin: 'dispose' }, { x: 0, y: 0 }),
    createChanges: ({ newNodePosition, nodeManager }) => {
      const remappedId = addUniqueSuffix(
        'new_buffer',
        nodeManager.nodes
          .filter(isSectionBufferNode)
          .map((n) => n.data.remappedId),
      );
      return [
        createSectionBufferChange(
          remappedId,
          { builtin: 'dispose' },
          newNodePosition,
        ),
      ];
    },
  },
  {
    key: 'node',
    label: 'Node',
    createPreviewNode: (namespace, parentId) =>
      createOperationNode(
        namespace,
        parentId,
        { x: 0, y: 0 },
        { type: 'node', builder: '', next: { builtin: 'dispose' } },
        'preview_node',
      ),
    createChanges: ({ namespace, parentId, newNodePosition }) =>
      createNodeChange(namespace, parentId, newNodePosition, {
        type: 'node',
        builder: '',
        next: { builtin: 'dispose' },
      }),
  },
  {
    key: 'fork_clone',
    label: 'Fork Clone',
    createPreviewNode: (namespace, parentId) =>
      createOperationNode(
        namespace,
        parentId,
        { x: 0, y: 0 },
        { type: 'fork_clone', next: [] },
        'preview_fork_clone',
      ),
    createChanges: ({ namespace, parentId, newNodePosition }) =>
      createNodeChange(namespace, parentId, newNodePosition, {
        type: 'fork_clone',
        next: [],
      }),
  },
  {
    key: 'unzip',
    label: 'Unzip',
    createPreviewNode: (namespace, parentId) =>
      createOperationNode(
        namespace,
        parentId,
        { x: 0, y: 0 },
        { type: 'unzip', next: [] },
        'preview_unzip',
      ),
    createChanges: ({ namespace, parentId, newNodePosition }) =>
      createNodeChange(namespace, parentId, newNodePosition, {
        type: 'unzip',
        next: [],
      }),
  },
  {
    key: 'fork_result',
    label: 'Fork Result',
    createPreviewNode: (namespace, parentId) =>
      createOperationNode(
        namespace,
        parentId,
        { x: 0, y: 0 },
        {
          type: 'fork_result',
          err: { builtin: 'dispose' },
          ok: { builtin: 'dispose' },
        },
        'preview_fork_result',
      ),
    createChanges: ({ namespace, parentId, newNodePosition }) =>
      createNodeChange(namespace, parentId, newNodePosition, {
        type: 'fork_result',
        err: { builtin: 'dispose' },
        ok: { builtin: 'dispose' },
      }),
  },
  {
    key: 'split',
    label: 'Split',
    createPreviewNode: (namespace, parentId) =>
      createOperationNode(
        namespace,
        parentId,
        { x: 0, y: 0 },
        { type: 'split' },
        'preview_split',
      ),
    createChanges: ({ namespace, parentId, newNodePosition }) =>
      createNodeChange(namespace, parentId, newNodePosition, {
        type: 'split',
      }),
  },
  {
    key: 'join',
    label: 'Join',
    createPreviewNode: (namespace, parentId) =>
      createOperationNode(
        namespace,
        parentId,
        { x: 0, y: 0 },
        { type: 'join', buffers: [], next: { builtin: 'dispose' } },
        'preview_join',
      ),
    createChanges: ({ namespace, parentId, newNodePosition }) =>
      createNodeChange(namespace, parentId, newNodePosition, {
        type: 'join',
        buffers: [],
        next: { builtin: 'dispose' },
      }),
  },
  {
    key: 'transform',
    label: 'Transform',
    createPreviewNode: (namespace, parentId) =>
      createOperationNode(
        namespace,
        parentId,
        { x: 0, y: 0 },
        { type: 'transform', cel: '', next: { builtin: 'dispose' } },
        'preview_transform',
      ),
    createChanges: ({ namespace, parentId, newNodePosition }) =>
      createNodeChange(namespace, parentId, newNodePosition, {
        type: 'transform',
        cel: '',
        next: { builtin: 'dispose' },
      }),
  },
  {
    key: 'buffer',
    label: 'Buffer',
    createPreviewNode: (namespace, parentId) =>
      createOperationNode(
        namespace,
        parentId,
        { x: 0, y: 0 },
        { type: 'buffer' },
        'preview_buffer_node',
      ),
    createChanges: ({ namespace, parentId, newNodePosition }) =>
      createNodeChange(namespace, parentId, newNodePosition, {
        type: 'buffer',
      }),
  },
  {
    key: 'buffer_access',
    label: 'Buffer Access',
    createPreviewNode: (namespace, parentId) =>
      createOperationNode(
        namespace,
        parentId,
        { x: 0, y: 0 },
        { type: 'buffer_access', buffers: [], next: { builtin: 'dispose' } },
        'preview_buffer_access',
      ),
    createChanges: ({ namespace, parentId, newNodePosition }) =>
      createNodeChange(namespace, parentId, newNodePosition, {
        type: 'buffer_access',
        buffers: [],
        next: { builtin: 'dispose' },
      }),
  },
  {
    key: 'listen',
    label: 'Listen',
    createPreviewNode: (namespace, parentId) =>
      createOperationNode(
        namespace,
        parentId,
        { x: 0, y: 0 },
        { type: 'listen', buffers: [], next: { builtin: 'dispose' } },
        'preview_listen',
      ),
    createChanges: ({ namespace, parentId, newNodePosition }) =>
      createNodeChange(namespace, parentId, newNodePosition, {
        type: 'listen',
        buffers: [],
        next: { builtin: 'dispose' },
      }),
  },
  {
    key: 'stream_out',
    label: 'Stream Out',
    createPreviewNode: (namespace, parentId) =>
      createOperationNode(
        namespace,
        parentId,
        { x: 0, y: 0 },
        { type: 'stream_out', name: '' },
        'preview_stream_out',
      ),
    createChanges: ({ namespace, parentId, newNodePosition }) =>
      createNodeChange(namespace, parentId, newNodePosition, {
        type: 'stream_out',
        name: '',
      }),
  },
  {
    key: 'scope',
    label: 'Scope',
    createPreviewNode: (namespace, parentId) =>
      createScopeNode(
        namespace,
        parentId,
        { x: 0, y: 0 },
        {
          type: 'scope',
          start: { builtin: 'dispose' },
          ops: {},
          next: { builtin: 'dispose' },
        },
        'preview_scope',
      )[0],
    createChanges: ({ namespace, parentId, newNodePosition }) =>
      createNodeChange(namespace, parentId, newNodePosition, {
        type: 'scope',
        start: { builtin: 'dispose' },
        ops: {},
        next: { builtin: 'dispose' },
      }),
  },
  {
    key: 'section',
    label: 'Section',
    createPreviewNode: (namespace, parentId) =>
      createOperationNode(
        namespace,
        parentId,
        { x: 0, y: 0 },
        { type: 'section', template: '' },
        'preview_section',
      ),
    createChanges: ({ namespace, parentId, newNodePosition }) =>
      createNodeChange(namespace, parentId, newNodePosition, {
        type: 'section',
        template: '',
      }),
  },
];

export function getVisibleAddOperations(options: {
  isTemplateMode: boolean;
  namespace: string;
}) {
  return ADD_OPERATION_DEFINITIONS.filter((definition) => {
    if (!definition.templateOnlyRoot) {
      return true;
    }

    return options.isTemplateMode && options.namespace === ROOT_NAMESPACE;
  });
}

export function filterCompatibleAddOperations(
  definitions: AddOperationDefinition[],
  anchorNode: DiagramEditorNode,
  anchorHandle: string | null | undefined,
  options: { namespace: string; parentId: string | undefined },
  anchorHandleType: 'source' | 'target' = 'source',
) {
  return definitions.filter((definition) => {
    const previewNode = definition.createPreviewNode(
      options.namespace,
      options.parentId,
    );
    if (
      anchorHandleType === 'target' &&
      previewNode.type === 'fork_result'
    ) {
      return false;
    }
    return anchorHandleType === 'source'
      ? getValidEdgeTypes(
          anchorNode,
          anchorHandle,
          previewNode,
          null,
        ).length > 0
      : getValidEdgeTypes(
          previewNode,
          null,
          anchorNode,
          anchorHandle,
        ).length > 0;
  });
}
