import { render, screen, waitFor } from '@testing-library/react';
import { CompatibleAddOperation } from './compatible-add-operation';

const mockCandidates = [
  {
    key: 'transform',
    label: 'Transform',
    createChanges: () => [
      {
        type: 'add',
        item: { id: 'transform-node' },
      },
    ],
  },
  {
    key: 'fork_clone',
    label: 'Fork Clone',
    createChanges: () => [
      {
        type: 'add',
        item: { id: 'fork-clone-node' },
      },
    ],
  },
  {
    key: 'node:calculator',
    label: 'Calculator',
    createChanges: () => [
      {
        type: 'add',
        item: { id: 'calc-node' },
      },
    ],
  },
];

const mockCheckConnections = jest.fn(
  async () =>
    new Map([
      [
        'transform',
        { id: 'transform', status: 'compatible' as const, reason: '' },
      ],
      [
        'fork_clone',
        { id: 'fork_clone', status: 'compatible' as const, reason: '' },
      ],
      [
        'node:calculator',
        { id: 'node:calculator', status: 'compatible' as const, reason: '' },
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
  getAddOperationCandidates: () => mockCandidates,
  getVisibleAddOperations: () => [],
}));

jest.mock('./utils/connection', () => ({
  createConnectionFromHandles: (_source: unknown, targetId: string) => ({
    source: 'source-node',
    target: targetId,
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
      await screen.findByRole('button', { name: /Transform/ }),
    ).toBeInTheDocument();
    await waitFor(() => {
      expect(onContentChange).toHaveBeenCalled();
    });
  });

  test('renders specific icons for compatible operations and registry builders', async () => {
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

    const transformButton = await screen.findByRole('button', {
      name: /Transform/,
    });
    const forkCloneButton = await screen.findByRole('button', {
      name: /Fork Clone/,
    });
    const calculatorButton = await screen.findByRole('button', {
      name: /Calculator/,
    });

    const transformIcon = transformButton.querySelector(
      '.material-symbols-outlined',
    );
    const forkCloneIcon = forkCloneButton.querySelector(
      '.material-symbols-outlined',
    );
    const calculatorIcon = calculatorButton.querySelector(
      '.material-symbols-outlined',
    );

    expect(transformIcon).toBeInTheDocument();
    expect(forkCloneIcon).toBeInTheDocument();
    expect(calculatorIcon).toBeInTheDocument();

    expect(transformIcon?.textContent).toBe('change_circle');
    expect(forkCloneIcon?.textContent).toBe('content_copy');
    expect(calculatorIcon?.textContent).toBe('line_start_circle');
  });
});
