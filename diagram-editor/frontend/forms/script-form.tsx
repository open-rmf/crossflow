import {
  Autocomplete,
  IconButton,
  MenuItem,
  Stack,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material';
import { useMemo, useState } from 'react';
import { useDiagramProperties } from '../diagram-properties-provider';
import { MaterialSymbol } from '../nodes';
import { useRegistry } from '../registry-provider';
import BaseEditOperationForm, {
  type BaseEditOperationFormProps,
} from './base-edit-operation-form';
import { ScriptEnvironmentManagerDialog } from './script-environment-manager-dialog';
import { ScriptEnvironmentPanel } from './script-environment-panel';

export type ScriptFormProps = BaseEditOperationFormProps<'script'>;

function ScriptForm(props: ScriptFormProps) {
  const [diagramProperties] = useDiagramProperties();
  const registry = useRegistry();
  const environments = diagramProperties.script_environments || {};

  const [openManager, setOpenManager] = useState(false);

  // Track raw string of node config to support JSON editing
  const [configValue, setConfigValue] = useState(() =>
    props.node.data.op.config
      ? JSON.stringify(props.node.data.op.config, null, 2)
      : '',
  );

  const configError = useMemo(() => {
    if (configValue === '') return false;
    try {
      JSON.parse(configValue);
      return false;
    } catch {
      return true;
    }
  }, [configValue]);

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

  // Real-time parse python entrypoint functions from selected environment
  const selectedEnvName = props.node.data.op.environment;
  const selectedEnv = selectedEnvName
    ? environments[selectedEnvName]
    : undefined;

  const scriptText = useMemo(() => {
    if (!selectedEnv) return '';
    if (selectedEnv.builder === 'process-bound-python') {
      const pbConfig = selectedEnv.config as
        | Record<string, unknown>
        | undefined;
      return (pbConfig?.script as string) || '';
    }
    const envRecord = selectedEnv as unknown as Record<string, unknown>;
    return (envRecord.script as string) || '';
  }, [selectedEnv]);

  const availableFunctions = useMemo(() => {
    if (!scriptText) return [];
    const regex = /^[ \t]*(?:async\s+)?def\s+([a-zA-Z_][a-zA-Z0-9_]*)\s*\(/gm;
    const functions: string[] = [];
    regex.lastIndex = 0;
    while (true) {
      const match = regex.exec(scriptText);
      if (match === null) {
        break;
      }
      functions.push(match[1]);
    }
    return functions;
  }, [scriptText]);

  return (
    <BaseEditOperationForm
      {...props}
      sidePanel={
        <ScriptEnvironmentPanel
          environmentName={selectedEnvName}
          environment={selectedEnv}
          metadata={
            selectedEnv ? registry.scripting[selectedEnv.builder] : undefined
          }
          onEdit={() => setOpenManager(true)}
        />
      }
    >
      <TextField
        label="Display Text"
        value={props.node.data.op.display_text || ''}
        onChange={(ev) => {
          const updatedNode = { ...props.node };
          updatedNode.data.op.display_text = ev.target.value || undefined;
          props.onChange?.({
            type: 'replace',
            id: props.node.id,
            item: updatedNode,
          });
        }}
      />

      <Stack direction="row" spacing={1} alignItems="center" minWidth={0}>
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
          <IconButton onClick={() => setOpenManager(true)}>
            <MaterialSymbol symbol="settings" />
          </IconButton>
        </Tooltip>
      </Stack>

      {/* Entrypoint dropdown populated via real-time script parser */}
      <Autocomplete
        freeSolo
        autoSelect
        options={availableFunctions}
        value={props.node.data.op.run || ''}
        onChange={(_, val) => {
          const updatedNode = { ...props.node };
          updatedNode.data.op.run = val ?? '';
          props.onChange?.({
            type: 'replace',
            id: props.node.id,
            item: updatedNode,
          });
        }}
        onInputChange={(_, val) => {
          const updatedNode = { ...props.node };
          updatedNode.data.op.run = val;
          props.onChange?.({
            type: 'replace',
            id: props.node.id,
            item: updatedNode,
          });
        }}
        renderInput={(params) => (
          <TextField
            {...params}
            required
            label="Entrypoint Function (run)"
            placeholder="Select or type the script function"
          />
        )}
      />

      <TextField
        multiline
        rows={4}
        label="Node Config (JSON)"
        value={configValue}
        onChange={(ev) => {
          setConfigValue(ev.target.value);
          try {
            const updatedNode = { ...props.node };
            updatedNode.data.op.config =
              ev.target.value === '' ? undefined : JSON.parse(ev.target.value);
            props.onChange?.({
              type: 'replace',
              id: props.node.id,
              item: updatedNode,
            });
          } catch {}
        }}
        error={configError}
        slotProps={{
          htmlInput: {
            sx: { fontFamily: 'monospace', whiteSpace: 'nowrap' },
          },
        }}
      />

      <ScriptEnvironmentManagerDialog
        open={openManager}
        onClose={() => setOpenManager(false)}
        initialEnvName={props.node.data.op.environment}
      />
    </BaseEditOperationForm>
  );
}

export default ScriptForm;
