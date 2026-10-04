import { render, screen } from '@testing-library/react';
import { Position, ReactFlowProvider } from '@xyflow/react';
import { ConnectionHintPanel } from './connection-hint-panel';
import { Handle, HandleType } from './handles';
import { NodeManager } from './node-manager';
import { createOperationNode } from './nodes';
import type { CompatibilityResult } from './types/api';
import { ROOT_NAMESPACE } from './utils/namespace';

let mockCompatibility: CompatibilityResult['status'] = 'unknown';

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
    toHandle: { nodeId: 'target', id: null, type: 'target' },
    toNode: { id: 'target' },
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
