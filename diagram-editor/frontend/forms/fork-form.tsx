import { MenuItem, TextField } from '@mui/material';
import BaseEditOperationForm, {
  type BaseEditOperationFormProps,
} from './base-edit-operation-form';

export type ForkFormProps = BaseEditOperationFormProps<
  'fork_clone' | 'fork_result'
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
          const op: ForkFormProps['node']['data']['op'] =
            event.target.value === 'fork_clone'
              ? { type: 'fork_clone', next: [] }
              : {
                  type: 'fork_result',
                  ok: { builtin: 'dispose' },
                  err: { builtin: 'dispose' },
                };
          onChange?.({
            type: 'replace',
            id: node.id,
            item: { ...node, type: op.type, data: { ...node.data, op } },
          });
        }}
      >
        <MenuItem value="fork_clone">Clone</MenuItem>
        <MenuItem value="fork_result">Result</MenuItem>
      </TextField>
    </BaseEditOperationForm>
  );
}

export default ForkForm;
