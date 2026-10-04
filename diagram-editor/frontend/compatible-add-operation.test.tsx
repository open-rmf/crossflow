import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react';
import { CompatibleAddOperation } from './compatible-add-operation';
import type { CompatibilityResult } from './types/api';

const mockCandidate = {
  key: 'candidate',
  label: 'Candidate operation',
  createChanges: () => [
    {
      type: 'add',
      item: { id: 'candidate-node' },
    },
  ],
};
const mockCheckConnections = jest.fn<
  Promise<Map<string, CompatibilityResult>>,
  []
>();
const mockChecker = { checkConnections: mockCheckConnections };
const mockEditorMode = [{ mode: 0 }];
const mockNodeManager = {
  tryGetNode: () => ({ id: 'source-node' }),
};
const mockRegistry = {};

jest.mock('./connection-compatibility-provider', () => ({
  useCompatibilityChecker: () => mockChecker,
}));

jest.mock('./editor-mode', () => ({
  EditorMode: { Normal: 0, Template: 1 },
  useEditorMode: () => mockEditorMode,
}));

jest.mock('./node-manager', () => ({
  useNodeManager: () => mockNodeManager,
}));

jest.mock('./registry-provider', () => ({
  useRegistry: () => mockRegistry,
}));

jest.mock('./utils/add-operation-catalog', () => ({
  filterCompatibleAddOperations: (candidates: unknown[]) => candidates,
  getAddOperationCandidates: () => [mockCandidate],
  getVisibleAddOperations: () => [],
}));

describe('CompatibleAddOperation', () => {
  beforeEach(() => {
    mockCheckConnections.mockReset();
    mockCheckConnections.mockImplementation(() => new Promise(() => {}));
  });

  test('makes locally eligible operations selectable while inference is pending', async () => {
    const onAdd = jest.fn();
    const onContentChange = jest.fn();

    render(
      <CompatibleAddOperation
        newNodePosition={{ x: 0, y: 0 }}
        sourceConnection={{
          sourceNodeId: 'source-node',
          sourceHandle: null,
          sourceHandleType: 'source',
        }}
        onAdd={onAdd}
        onContentChange={onContentChange}
      />,
    );

    expect(
      screen.getByRole('button', { name: /Candidate operation/ }),
    ).toBeInTheDocument();
    expect(screen.getByText('Add next operation')).toBeInTheDocument();
    expect(
      screen.queryByText('Checking compatible operations...'),
    ).not.toBeInTheDocument();
    fireEvent.click(
      screen.getByRole('button', { name: /Candidate operation/ }),
    );
    expect(onAdd).toHaveBeenCalledWith({
      changes: [{ type: 'add', item: { id: 'candidate-node' } }],
      primaryNodeId: 'candidate-node',
    });
    await waitFor(() => {
      expect(onContentChange).toHaveBeenCalled();
    });
  });

  test.each(['compatible', 'unknown', 'incompatible', 'missing', 'failure'])(
    'only hides definitive incompatibility after result: %s',
    async (status) => {
      mockCheckConnections.mockImplementationOnce(async () => {
        if (status === 'failure') {
          throw new Error('compatibility unavailable');
        }
        return new Map(
          status === 'missing'
            ? []
            : [
                [
                  'candidate',
                  {
                    id: 'candidate',
                    status: status as CompatibilityResult['status'],
                    reason: '',
                  },
                ],
              ],
        );
      });

      await act(async () => {
        render(
          <CompatibleAddOperation
            newNodePosition={{ x: 0, y: 0 }}
            sourceConnection={{
              sourceNodeId: 'source-node',
              sourceHandle: null,
              sourceHandleType: 'source',
            }}
          />,
        );
      });

      const operation = screen.queryByRole('button', {
        name: /Candidate operation/,
      });
      if (status === 'incompatible') {
        expect(operation).not.toBeInTheDocument();
      } else {
        expect(operation).toBeInTheDocument();
      }
    },
  );

  test('describes reverse additions as previous operations', () => {
    render(
      <CompatibleAddOperation
        newNodePosition={{ x: 0, y: 0 }}
        sourceConnection={{
          sourceNodeId: 'source-node',
          sourceHandle: null,
          sourceHandleType: 'target',
        }}
      />,
    );

    expect(screen.getByText('Add previous operation')).toBeInTheDocument();
    expect(
      screen.getByRole('group', { name: 'Add operation button group' }),
    ).toBeInTheDocument();
  });
});
