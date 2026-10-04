import { render, screen } from '@testing-library/react';
import { Position, ReactFlowProvider } from '@xyflow/react';
import { ConnectionHintPanel } from './connection-hint-panel';
import { createDefaultEdge } from './edges';
import { Handle, HandleType } from './handles';
import { NodeManager } from './node-manager';
import { createOperationNode } from './nodes';
import type { CompatibilityResult } from './types/api';
import { EdgesProvider } from './use-edges';
import { ROOT_NAMESPACE } from './utils/namespace';

let mockCompatibility: CompatibilityResult['status'] = 'unknown';
let mockHovering = true;

beforeEach(() => {
  mockHovering = true;
});

jest.mock('./connection-compatibility-provider', () => ({
  useDraggedConnectionCompatibility: () => ({
    id: 'hovered-handle',
    status: mockCompatibility,
    reason: 'Compatibility result',
  }),
}));

jest.mock('@xyflow/react', () => ({
  ...jest.requireActual('@xyflow/react'),
  useConnection: () => ({
    inProgress: true,
    fromHandle: { nodeId: 'source', id: null, type: 'source' },
    toHandle: mockHovering
      ? { nodeId: 'target', id: null, type: 'target' }
      : null,
    toNode: mockHovering ? { id: 'target' } : null,
  }),
}));

test.each([
  ['unknown', null, 'rgba(0, 0, 0, 0.12)'],
  ['compatible', 'handle-compatible', '#2e7d32'],
  ['incompatible', 'handle-incompatible', '#d32f2f'],
] as const)(
  'uses %s compatibility feedback',
  (status, className, borderColor) => {
    mockCompatibility = status;
    const source = {
      ...createOperationNode(
        ROOT_NAMESPACE,
        undefined,
        { x: 0, y: 0 },
        { type: 'fork_clone', next: [] },
        'source',
      ),
      id: 'source',
    };
    render(
      <ReactFlowProvider>
        <Handle
          variant={HandleType.Data}
          type="target"
          position={Position.Left}
        />
        <ConnectionHintPanel nodeManager={new NodeManager([source])} />
      </ReactFlowProvider>,
    );

    const handle = document.querySelector('.react-flow__handle');
    if (className) {
      expect(handle).toHaveClass(className);
    } else {
      expect(handle).not.toHaveClass('handle-compatible');
      expect(handle).not.toHaveClass('handle-incompatible');
    }
    const panel = screen
      .getByText('Compatibility result')
      .closest('.MuiPaper-root');
    expect(panel).toHaveStyle({ borderColor });
  },
);

test.each([false, true])(
  'capacity hint excludes the moving wire before hovering a target: %s',
  (reconnecting) => {
    mockHovering = false;
    const source = {
      ...createOperationNode(
        ROOT_NAMESPACE,
        undefined,
        { x: 0, y: 0 },
        { type: 'node', builder: '', next: { builtin: 'dispose' } },
        'source',
      ),
      id: 'source',
    };
    const edge = createDefaultEdge(source.id, null, 'target', null);
    const props = {
      nodeManager: new NodeManager([source]),
      reconnectingEdgeId: reconnecting ? edge.id : undefined,
    };
    render(
      <ReactFlowProvider>
        <EdgesProvider value={[edge]}>
          <ConnectionHintPanel {...props} />
        </EdgesProvider>
      </ReactFlowProvider>,
    );
    const panel = screen
      .getByText('Connection Helper')
      .closest('.MuiPaper-root');
    expect(panel).toHaveStyle({
      borderColor: reconnecting ? 'rgba(0, 0, 0, 0.12)' : '#d32f2f',
    });
    if (reconnecting) {
      expect(panel).toHaveTextContent('keep the original connection');
    }
  },
);
