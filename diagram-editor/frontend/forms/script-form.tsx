import { useState } from 'react';
import { IconButton, TextField, MenuItem, Stack, Tooltip, Typography } from '@mui/material';
import BaseEditOperationForm, {
  type BaseEditOperationFormProps,
} from './base-edit-operation-form';
import { MaterialSymbol } from '../nodes';
import { ScriptEnvironmentManagerDialog } from './script-environment-manager-dialog';
import { useDiagramProperties } from '../diagram-properties-provider';

export type ScriptFormProps = BaseEditOperationFormProps<'script'>;

function ScriptForm(props: ScriptFormProps) {
  const [diagramProperties] = useDiagramProperties();
  const environments = diagramProperties.script_environments || {};

  const [openManager, setOpenManager] = useState(false);

  const handleEnvChange = (envName: string) => {
    const updatedNode = {
      ...props.node,
      data: {
        ...props.node.data,
        op: {
          ...props.node.data.op,
          environment: envName,
        },
      },
    };
    props.onChange?.({
      type: 'replace',
      id: props.node.id,
      item: updatedNode,
    });
  };

  return (
    <BaseEditOperationForm {...props}>

      <Stack direction="row" spacing={1} alignItems="center" minWidth={300}>
        <TextField
          select
          label="Script Environment"
          value={props.node.data.op.environment || ''}
          onChange={(e) => handleEnvChange(e.target.value)}
          fullWidth
        >
          {Object.keys(environments).length === 0 ? (
            <MenuItem disabled value="">
              <Typography variant="caption" color="text.disabled">
                No environments available, please create one
              </Typography>
            </MenuItem>
          ) : (
            Object.keys(environments).map((envName) => (
              <MenuItem key={envName} value={envName}>
                {envName}
              </MenuItem>
            ))
          )}
        </TextField>

        <Tooltip title="Manage or add script environment">
          <IconButton
            onClick={() => {
              setOpenManager(true);
            }}
          >
            <MaterialSymbol symbol='settings'/>
          </IconButton>
        </Tooltip>
      </Stack>

      <ScriptEnvironmentManagerDialog
        open={openManager}
        onClose={() => setOpenManager(false)}
        initialEnvName={props.node.data.op.environment}
      />

    </BaseEditOperationForm>
  );
}

export default ScriptForm;
