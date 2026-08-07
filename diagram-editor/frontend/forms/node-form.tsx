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
  const configSchema =
    registry.nodes[props.node.data.op.builder]?.config_schema;
  const existingConfig = props.node.data.op.config;

  const updateConfig = (config: unknown) => {
    const updatedNode = {
      ...props.node,
      data: {
        ...props.node.data,
        op: {
          ...props.node.data.op,
          config,
        },
      },
    };
    if (config === undefined) {
      delete updatedNode.data.op.config;
    }
    props.onChange?.({
      type: 'replace',
      id: props.node.id,
      item: updatedNode,
    });
  };

  return (
    <BaseEditOperationForm {...props}>
      <TextField
        label="Display Text"
        value={props.node.data.op.display_text || ''}
        onChange={(ev) => {
          const updatedNode = {
            ...props.node,
            data: {
              ...props.node.data,
              op: {
                ...props.node.data.op,
                display_text: ev.target.value || undefined,
              },
            },
          };
          props.onChange?.({
            type: 'replace',
            id: props.node.id,
            item: updatedNode,
          });
        }}
      />
      <Autocomplete
        freeSolo
        autoSelect
        options={nodes}
        getOptionLabel={(option) => option}
        value={props.node.data.op.builder}
        onChange={(_, value) => {
          const builder = value ?? '';
          const defaults =
            existingConfig === undefined
              ? getSchemaConfigDefaults(
                  registry.nodes[builder]?.config_schema,
                  registry.schemas,
                )
              : undefined;
          const updatedNode = {
            ...props.node,
            data: {
              ...props.node.data,
              op: {
                ...props.node.data.op,
                builder,
                ...(defaults !== undefined && { config: defaults }),
              },
            },
          };
          props.onChange?.({
            type: 'replace',
            id: props.node.id,
            item: updatedNode,
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
        schema={configSchema}
        definitions={registry.schemas}
        value={existingConfig}
        onChange={updateConfig}
      />
    </BaseEditOperationForm>
  );
}

export default NodeForm;
