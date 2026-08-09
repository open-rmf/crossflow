import { fireEvent, render, screen } from '@testing-library/react';
import { createOperationNode } from '../nodes';
import { ROOT_NAMESPACE } from '../utils/namespace';
import EditNodeForm from './edit-node-form';

jest.mock('cel-js', () => ({ parse: jest.fn() }));

describe('ForkForm', () => {
  test('switches a clone fork to a result fork', () => {
    const node = createOperationNode(
      ROOT_NAMESPACE,
      undefined,
      { x: 1, y: 2 },
      { type: 'fork_clone', next: [] },
      'fork',
    );
    const onChange = jest.fn();
    render(<EditNodeForm node={node} onChange={onChange} />);

    fireEvent.mouseDown(screen.getByRole('combobox', { name: 'Behavior' }));
    fireEvent.click(screen.getByRole('option', { name: 'Result' }));

    expect(onChange).toHaveBeenCalledWith({
      type: 'replace',
      id: node.id,
      item: {
        ...node,
        type: 'fork_result',
        data: {
          ...node.data,
          op: {
            type: 'fork_result',
            ok: { builtin: 'dispose' },
            err: { builtin: 'dispose' },
          },
        },
      },
    });
  });
});
