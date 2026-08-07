import {
  Autocomplete,
  ListItem,
  Stack,
  TextField,
  Tooltip,
} from '@mui/material';
import { MaterialSymbol } from '../nodes';
import { useRegistry } from '../registry-provider';
import BaseEditOperationForm, {
  type BaseEditOperationFormProps,
} from './base-edit-operation-form';
import SchemaConfigForm, {
  getSchemaConfigDefaults,
} from './schema-config-form';

export type NodeFormProps = BaseEditOperationFormProps<'node'>;

function NodeForm(props: NodeFormProps) {
  const registry = useRegistry();
  const nodes = Object.keys(registry.nodes).sort();
  const op = props.node.data.op;
  const existingConfig = op.config;

  const replaceOp = (updatedOp: typeof op) => {
    props.onChange?.({
      type: 'replace',
      id: props.node.id,
      item: { ...props.node, data: { ...props.node.data, op: updatedOp } },
    });
  };

  const updateConfig = (config: unknown) => {
    const { config: _config, ...rest } = op;
    replaceOp(config === undefined ? rest : { ...rest, config });
  };

  return (
    <BaseEditOperationForm {...props}>
      <TextField
        label="Display Text"
        value={op.display_text || ''}
        onChange={(ev) => {
          replaceOp({ ...op, display_text: ev.target.value || undefined });
        }}
      />
      <Autocomplete
        freeSolo
        autoSelect
        options={nodes}
        getOptionLabel={(option) => option}
        value={op.builder}
        onChange={(_, value) => {
          const builder = value ?? '';
          // Only a node that has no config yet gets the new builder's defaults;
          // an existing config is the user's, so it is left alone.
          const defaults =
            existingConfig === undefined
              ? getSchemaConfigDefaults(
                  registry.nodes[builder]?.config_schema,
                  registry.schemas,
                )
              : undefined;
          replaceOp({
            ...op,
            builder,
            ...(defaults !== undefined && { config: defaults }),
          });
        }}
        renderInput={(params) => (
          <TextField {...params} required label="Builder" />
        )}
        renderOption={({ key, ...otherProps }, value) => {
          const nodeMetadata = registry.nodes[value];
          return (
            <ListItem key={key} {...otherProps}>
              <Stack
                direction="row"
                justifyContent="space-between"
                width="100%"
              >
                <span>{value}</span>
                {nodeMetadata?.description && (
                  <Tooltip title={nodeMetadata.description}>
                    <MaterialSymbol symbol="info" />
                  </Tooltip>
                )}
              </Stack>
            </ListItem>
          );
        }}
      />
      <SchemaConfigForm
        schema={registry.nodes[op.builder]?.config_schema}
        definitions={registry.schemas}
        value={existingConfig}
        onChange={updateConfig}
      />
    </BaseEditOperationForm>
  );
}

export default NodeForm;
