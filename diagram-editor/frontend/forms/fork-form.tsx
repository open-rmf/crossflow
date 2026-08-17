import { MenuItem, TextField } from '@mui/material';
import BaseEditOperationForm, {
  type BaseEditOperationFormProps,
} from './base-edit-operation-form';

export type ForkFormProps = BaseEditOperationFormProps<
  'fork_clone' | 'fork_result' | 'split'
>;

function ForkForm(props: ForkFormProps) {
  const { node, onChange } = props;

  return (
    <BaseEditOperationForm {...props}>
      <TextField
        select
        label="Behavior"
        value={node.data.op.type}
        onChange={(event) => {
          const behavior = event.target.value;
          const op: ForkFormProps['node']['data']['op'] =
            behavior === 'fork_clone'
              ? { type: 'fork_clone', next: [] }
              : behavior === 'fork_result'
                ? {
                    type: 'fork_result',
                    ok: { builtin: 'dispose' },
                    err: { builtin: 'dispose' },
                  }
                : { type: 'split' };
          onChange?.({
            type: 'replace',
            id: node.id,
            item: { ...node, type: op.type, data: { ...node.data, op } },
          });
        }}
      >
        <MenuItem value="fork_clone">Clone</MenuItem>
        <MenuItem value="fork_result">Result</MenuItem>
        <MenuItem value="split">Split</MenuItem>
      </TextField>
    </BaseEditOperationForm>
  );
}

export default ForkForm;
