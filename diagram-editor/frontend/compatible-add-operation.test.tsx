import { render, screen, waitFor } from '@testing-library/react';
import { CompatibleAddOperation } from './compatible-add-operation';

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
const mockCheckConnections = jest.fn(
  async () =>
    new Map([
      [
        'candidate',
        { id: 'candidate', status: 'compatible' as const, reason: '' },
      ],
    ]),
);
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

jest.mock('./utils/connection', () => ({
  createConnectionFromHandles: () => ({
    source: 'source-node',
    target: 'candidate-node',
  }),
}));

describe('CompatibleAddOperation', () => {
  test('reports when asynchronous operation results resize its popup', async () => {
    const onContentChange = jest.fn();

    render(
      <CompatibleAddOperation
        newNodePosition={{ x: 0, y: 0 }}
        sourceConnection={{
          sourceNodeId: 'source-node',
          sourceHandle: null,
          sourceHandleType: 'source',
        }}
        onContentChange={onContentChange}
      />,
    );

    expect(
      screen.getByText('Checking compatible operations...'),
    ).toBeInTheDocument();
    expect(
      await screen.findByRole('button', { name: /Candidate operation/ }),
    ).toBeInTheDocument();
    await waitFor(() => {
      expect(onContentChange).toHaveBeenCalled();
    });
  });
});
