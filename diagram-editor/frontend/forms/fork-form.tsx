import { MenuItem, TextField } from '@mui/material';
import type { OperationNode } from '../nodes';
import type { DiagramOperation } from '../types/api';
import BaseEditOperationForm, {
  type BaseEditOperationFormProps,
} from './base-edit-operation-form';

export type ForkFormProps = BaseEditOperationFormProps<
  'fork_clone' | 'fork_result'
>;

function ForkForm(props: ForkFormProps) {
  const { node, onChange } = props;
  const behavior = node.data.op.type === 'fork_clone' ? 'clone' : 'result';

  return (
    <BaseEditOperationForm {...props}>
      <TextField
        select
        label="Behavior"
        value={behavior}
        onChange={(event) => {
          const op: Extract<
            DiagramOperation,
            { type: 'fork_clone' | 'fork_result' }
          > =
            event.target.value === 'clone'
              ? { type: 'fork_clone', next: [] }
              : {
                  type: 'fork_result',
                  ok: { builtin: 'dispose' },
                  err: { builtin: 'dispose' },
                };
          const updatedNode: OperationNode = {
            ...node,
            type: op.type,
            data: { ...node.data, op },
          };
          onChange?.({
            type: 'replace',
            id: node.id,
            item: updatedNode,
          });
        }}
      >
        <MenuItem value="clone">Clone</MenuItem>
        <MenuItem value="result">Result</MenuItem>
      </TextField>
    </BaseEditOperationForm>
  );
}

export default ForkForm;
