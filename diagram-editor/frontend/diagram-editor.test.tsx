import { Snackbar } from '@mui/material';
import { act, render, screen, waitFor } from '@testing-library/react';
import type { Connection, ReactFlowProps } from '@xyflow/react';
import React from 'react';
import { of, throwError } from 'rxjs';
import type { CompatibleAddOperationProps } from './compatible-add-operation';
import DiagramEditor from './diagram-editor';
import { DiagramPropertiesProvider } from './diagram-properties-provider';
import { DiagramSidePanelProvider } from './diagram-side-panel-controller';
import type { DiagramEditorEdge } from './edges';
import { createOperationNode, type DiagramEditorNode } from './nodes';
import { TemplatesProvider } from './templates-provider';
import { TransientEditorDraftProvider } from './transient-editor-drafts';
import type { CompatibilityRequest, CompatibilityResult } from './types/api';
import { ROOT_NAMESPACE } from './utils/namespace';

type CapturedReactFlowProps = ReactFlowProps<
  DiagramEditorNode,
  DiagramEditorEdge
> & {
  nodes: DiagramEditorNode[];
  edges: DiagramEditorEdge[];
};

let mockReactFlowProps!: CapturedReactFlowProps;
let mockCompatibility: CompatibilityResult['status'] | 'failure' = 'unknown';
let mockHoveredConnection: Connection | null = null;

beforeEach(() => {
  mockHoveredConnection = null;
});

jest.mock('./api-client-provider', () => ({
  useApiClient: () => ({
    checkCompatibility: (request: CompatibilityRequest) =>
      mockCompatibility === 'failure'
        ? throwError(() => new Error('offline'))
        : of({
            results: request.candidates.map(({ id }) => ({
              id,
              status: mockCompatibility,
              reason: 'test compatibility result',
            })),
          }),
  }),
}));

jest.mock('@xyflow/react', () => {
  const actual = jest.requireActual('@xyflow/react');
  return {
    ...actual,
    ReactFlow: (props: CapturedReactFlowProps) => {
      mockReactFlowProps = props;
      const { useConnectionCompatibility } = jest.requireActual(
        './connection-compatibility-provider',
      );
      const result = useConnectionCompatibility(mockHoveredConnection);
      return (
        <output data-testid="compatibility-preview">{result?.status}</output>
      );
    },
  };
});

jest.mock('./registry-provider', () => ({
  useRegistry: () => ({}),
}));

jest.mock('./forms', () => ({
  EditEdgeForm: () => null,
  EditNodeForm: () => null,
}));

type CompatibleAddElement = React.ReactElement<
  React.PropsWithChildren<CompatibleAddOperationProps>
>;

function findCompatibleAddElement(
  node: React.ReactNode,
): CompatibleAddElement | null {
  for (const child of React.Children.toArray(node)) {
    if (
      !React.isValidElement<
        React.PropsWithChildren<CompatibleAddOperationProps>
      >(child)
    ) {
      continue;
    }
    if (child.props.sourceConnection && child.props.onAdd) {
      return child;
    }
    const nested = findCompatibleAddElement(child.props.children);
    if (nested) {
      return nested;
    }
  }
  return null;
}

function renderEditor() {
  render(
    <TemplatesProvider>
      <DiagramPropertiesProvider>
        <TransientEditorDraftProvider>
          <DiagramSidePanelProvider>
            <DiagramEditor />
          </DiagramSidePanelProvider>
        </TransientEditorDraftProvider>
      </DiagramPropertiesProvider>
    </TemplatesProvider>,
  );
}

test.each(['compatible', 'unknown', 'incompatible', 'failure'] as const)(
  'connection admission allows all but proven incompatibility: %s',
  async (status) => {
    mockCompatibility = status;
    renderEditor();
    const start = mockReactFlowProps.nodes.find(
      (node) => node.type === 'start',
    )!;
    const terminate = mockReactFlowProps.nodes.find(
      (node) => node.type === 'terminate',
    )!;
    await act(async () => {
      mockReactFlowProps.onConnect?.({
        source: start.id,
        sourceHandle: null,
        target: terminate.id,
        targetHandle: null,
      });
    });
    expect(mockReactFlowProps.edges).toHaveLength(
      status === 'incompatible' ? 0 : 1,
    );
  },
);

test.each([false, true])(
  'reconnect preview excludes only the moving wire until drag ends (drop: %s)',
  async (drop) => {
    mockCompatibility = 'compatible';
    renderEditor();
    const start = mockReactFlowProps.nodes.find(
      (node) => node.type === 'start',
    )!;
    const terminate = mockReactFlowProps.nodes.find(
      (node) => node.type === 'terminate',
    )!;
    const target = createOperationNode(
      ROOT_NAMESPACE,
      undefined,
      { x: 200, y: 200 },
      { type: 'fork_clone', next: [] },
      'target',
    );
    await act(async () => {
      mockReactFlowProps.onConnect?.({
        source: start.id,
        sourceHandle: null,
        target: terminate.id,
        targetHandle: null,
      });
    });
    const originalEdge = mockReactFlowProps.edges[0];
    const connection: Connection = {
      source: start.id,
      sourceHandle: null,
      target: target.id,
      targetHandle: null,
    };
    mockHoveredConnection = connection;
    act(() => {
      mockReactFlowProps.onNodesChange?.([{ type: 'add', item: target }]);
    });
    const preview = screen.getByTestId('compatibility-preview');
    await waitFor(() => expect(preview).toHaveTextContent(/^incompatible$/));

    // React Flow captures this callback before starting the reconnect.
    const onConnectEnd = mockReactFlowProps.onConnectEnd;
    act(() => {
      mockReactFlowProps.onReconnectStart?.(
        new MouseEvent('mousedown') as unknown as React.MouseEvent,
        originalEdge,
        'target',
      );
    });
    await waitFor(() => expect(preview).toHaveTextContent(/^compatible$/));
    await act(async () => {
      if (drop) mockReactFlowProps.onReconnect?.(originalEdge, connection);
      else {
        onConnectEnd?.(new MouseEvent('mouseup'), {
          fromHandle: { nodeId: start.id, id: null, type: 'source' },
          toHandle: null,
          isValid: false,
        } as never);
      }
      mockReactFlowProps.onReconnectEnd?.(
        new MouseEvent('mouseup'),
        originalEdge,
        'target',
      );
    });
    await waitFor(() => expect(preview).toHaveTextContent(/^incompatible$/));
    expect(mockReactFlowProps.edges).toHaveLength(1);
    expect(mockReactFlowProps.edges[0]).toMatchObject({
      source: start.id,
      target: drop ? target.id : terminate.id,
    });
    const errorToast = React.Children.toArray(mockReactFlowProps.children).find(
      (child): child is React.ReactElement<{ open: boolean }> =>
        React.isValidElement(child) && child.type === Snackbar,
    );
    expect(errorToast?.props.open).toBe(false);
    expect(findCompatibleAddElement(mockReactFlowProps.children)).toBeNull();
    await act(async () => {
      onConnectEnd?.(new MouseEvent('mouseup'), {
        fromHandle: { nodeId: terminate.id, id: null, type: 'target' },
        toHandle: null,
        isValid: false,
      } as never);
    });
    expect(
      findCompatibleAddElement(mockReactFlowProps.children),
    ).not.toBeNull();
  },
);

test('unknown types allow connect, reconnect, and add-and-connect', async () => {
  mockCompatibility = 'unknown';
  renderEditor();

  const start = mockReactFlowProps.nodes.find((node) => node.type === 'start')!;
  const terminate = mockReactFlowProps.nodes.find(
    (node) => node.type === 'terminate',
  )!;

  const unfinished = createOperationNode(
    ROOT_NAMESPACE,
    undefined,
    { x: 200, y: 200 },
    { type: 'node', builder: '', next: { builtin: 'dispose' } },
    'unfinished',
  );
  act(() => {
    mockReactFlowProps.onNodesChange?.([{ type: 'add', item: unfinished }]);
  });

  const startToTerminate: Connection = {
    source: start.id,
    sourceHandle: null,
    target: terminate.id,
    targetHandle: null,
  };
  await act(async () => {
    mockReactFlowProps.onConnect?.(startToTerminate);
  });
  expect(mockReactFlowProps.edges).toHaveLength(1);
  expect(mockReactFlowProps.edges[0]).toMatchObject({
    source: start.id,
    target: terminate.id,
  });

  const originalEdge = mockReactFlowProps.edges[0];
  const reconnected: Connection = {
    ...startToTerminate,
    target: unfinished.id,
  };
  await act(async () => {
    mockReactFlowProps.onReconnect?.(originalEdge, reconnected);
  });
  expect(mockReactFlowProps.edges).toHaveLength(1);
  expect(mockReactFlowProps.edges[0]).toMatchObject({
    source: start.id,
    target: unfinished.id,
  });

  act(() => {
    mockReactFlowProps.onConnectEnd?.(
      new MouseEvent('mouseup', { clientX: 40, clientY: 40 }),
      {
        fromHandle: { nodeId: terminate.id, id: null, type: 'target' },
        toHandle: null,
        isValid: false,
      } as never,
    );
  });
  const compatibleAdd = findCompatibleAddElement(mockReactFlowProps.children);
  if (!compatibleAdd?.props.onAdd) {
    throw new Error('add-and-connect operation was not opened');
  }

  const added: DiagramEditorNode = {
    ...unfinished,
    id: 'added',
    position: { x: 400, y: 200 },
    data: { ...unfinished.data, opId: 'added' },
  };
  await act(async () => {
    compatibleAdd.props.onAdd?.({
      changes: [{ type: 'add', item: added }],
      primaryNodeId: added.id,
    });
  });

  expect(mockReactFlowProps.nodes).toContainEqual(added);
  expect(mockReactFlowProps.edges).toEqual(
    expect.arrayContaining([
      expect.objectContaining({ source: added.id, target: terminate.id }),
    ]),
  );

  const edgeCount = mockReactFlowProps.edges.length;
  await act(async () => {
    mockReactFlowProps.onConnect?.(startToTerminate);
  });
  expect(mockReactFlowProps.edges).toHaveLength(edgeCount);
});
