import {
  Autocomplete,
  Button,
  ListItem,
  Stack,
  TextField,
  Tooltip,
} from '@mui/material';
import { useEffect, useMemo, useState } from 'react';
import { MaterialSymbol } from '../nodes';
import { useRegistry } from '../registry-provider';
import BaseEditOperationForm, {
  type BaseEditOperationFormProps,
} from './base-edit-operation-form';
import SchemaConfigForm, {
  getSchemaConfigDefaults,
  type JsonConfigObject,
  resolveSupportedConfigSchema,
} from './schema-config-form';

export type NodeFormProps = BaseEditOperationFormProps<'node'>;

function configObject(value: unknown): JsonConfigObject | undefined {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? (value as JsonConfigObject)
    : undefined;
}

interface RawConfigEditorProps {
  value: unknown;
  onChange: (value: unknown) => void;
}

function RawConfigEditor({ value, onChange }: RawConfigEditorProps) {
  const [configValue, setConfigValue] = useState(() =>
    value === undefined ? '' : JSON.stringify(value),
  );
  useEffect(() => {
    setConfigValue(value === undefined ? '' : JSON.stringify(value));
  }, [value]);
  const configError = useMemo(() => {
    if (configValue === '') {
      return false;
    }
    try {
      JSON.parse(configValue);
      return false;
    } catch {
      return true;
    }
  }, [configValue]);

  return (
    <TextField
      multiline
      rows={4}
      label="Config"
      value={configValue}
      onChange={(event) => {
        setConfigValue(event.target.value);
        try {
          onChange(
            event.target.value === ''
              ? undefined
              : JSON.parse(event.target.value),
          );
        } catch {}
      }}
      error={configError}
      slotProps={{
        htmlInput: {
          sx: { fontFamily: 'monospace', whiteSpace: 'nowrap' },
        },
      }}
    />
  );
}

function NodeForm(props: NodeFormProps) {
  const registry = useRegistry();
  const nodes = Object.keys(registry.nodes).sort();
  const [showRawConfig, setShowRawConfig] = useState(false);
  const configSchema =
    registry.nodes[props.node.data.op.builder]?.config_schema;
  const supportedSchema = useMemo(
    () => resolveSupportedConfigSchema(configSchema, registry.schemas),
    [configSchema, registry.schemas],
  );
  const existingConfig = props.node.data.op.config;
  const canUseGeneratedForm =
    supportedSchema !== null &&
    (existingConfig === undefined ||
      configObject(existingConfig) !== undefined);

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
          try {
            const updatedNode = { ...props.node };
            updatedNode.data.op.display_text = ev.target.value || undefined;
            props.onChange?.({
              type: 'replace',
              id: props.node.id,
              item: updatedNode,
            });
          } catch {}
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
                ...(defaults && { config: defaults }),
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
      {canUseGeneratedForm && (
        <Button
          size="small"
          sx={{ alignSelf: 'flex-start' }}
          onClick={() => setShowRawConfig((showRaw) => !showRaw)}
        >
          {showRawConfig ? 'Use generated form' : 'Edit raw JSON'}
        </Button>
      )}
      {canUseGeneratedForm && !showRawConfig ? (
        <SchemaConfigForm
          schema={configSchema}
          definitions={registry.schemas}
          value={configObject(existingConfig)}
          onChange={updateConfig}
        />
      ) : (
        <RawConfigEditor value={existingConfig} onChange={updateConfig} />
      )}
    </BaseEditOperationForm>
  );
}

export default NodeForm;
