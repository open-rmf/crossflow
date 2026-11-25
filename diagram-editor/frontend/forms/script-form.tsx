import {
  Autocomplete,
  IconButton,
  MenuItem,
  Stack,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material';
import { useMemo } from 'react';
import { useDiagramProperties } from '../diagram-properties-provider';
import { useDiagramSidePanel } from '../diagram-side-panel-controller';
import { MaterialSymbol } from '../nodes';
import BaseEditOperationForm, {
  type BaseEditOperationFormProps,
  operationUpdater,
} from './base-edit-operation-form';
import OperationConfigForm from './operation-config-form';
import { useScriptEnvironmentNavigation } from './use-script-environment-navigation';

export type ScriptFormProps = BaseEditOperationFormProps<'script'>;

const CREATE_ENVIRONMENT_VALUE = '__create_script_environment__';

function ScriptForm(props: ScriptFormProps) {
  const { op } = props.node.data;
  const replaceOp = operationUpdater(props);
  const [diagramProperties] = useDiagramProperties();
  const environments = diagramProperties.script_environments || {};
  const openScriptEnvironment = useScriptEnvironmentNavigation();
  const {
    state: { open: panelOpen, tab: panelTab },
    close: closePanel,
    startEnvironmentCreate,
    requestEnvironmentLeave,
  } = useDiagramSidePanel();

  const handleEnvChange = (envName: string) =>
    requestEnvironmentLeave(() => {
      replaceOp({ ...op, environment: envName });
      openScriptEnvironment(envName, false);
    });

  // Real-time parse python entrypoint functions from selected environment
  const selectedEnvName = op.environment;
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
    <BaseEditOperationForm {...props}>
      <TextField
        label="Display Text"
        value={op.display_text || ''}
        onChange={(ev) =>
          replaceOp({ ...op, display_text: ev.target.value || undefined })
        }
      />

      <Stack direction="row" spacing={1} alignItems="center" minWidth={0}>
        <TextField
          select
          label="Script Environment"
          value={selectedEnvName || ''}
          onChange={(e) => {
            if (e.target.value === CREATE_ENVIRONMENT_VALUE) {
              requestEnvironmentLeave(startEnvironmentCreate);
              return;
            }
            handleEnvChange(e.target.value);
          }}
          fullWidth
        >
          {selectedEnvName && !selectedEnv && (
            <MenuItem disabled value={selectedEnvName}>
              {selectedEnvName} (missing)
            </MenuItem>
          )}
          {Object.keys(environments).length === 0 && !selectedEnvName ? (
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
          <MenuItem value={CREATE_ENVIRONMENT_VALUE}>
            <Stack direction="row" spacing={1} alignItems="center">
              <MaterialSymbol symbol="add" />
              <span>Create new environment</span>
            </Stack>
          </MenuItem>
        </TextField>

        <Tooltip title="Toggle script environment panel">
          <IconButton
            aria-label="Toggle script environment panel"
            onClick={() => {
              if (panelOpen && panelTab === 'environments') {
                closePanel();
                return;
              }
              openScriptEnvironment(selectedEnvName || undefined, false);
            }}
          >
            <MaterialSymbol symbol="settings" />
          </IconButton>
        </Tooltip>
      </Stack>

      {/* Entrypoint dropdown populated via real-time script parser */}
      <Autocomplete
        freeSolo
        autoSelect
        options={availableFunctions}
        value={op.run || ''}
        onChange={(_, val) => replaceOp({ ...op, run: val ?? '' })}
        onInputChange={(_, val, reason) => {
          if (reason === 'input') replaceOp({ ...op, run: val });
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

      <OperationConfigForm
        {...props}
        schema={undefined}
        label="Node Config (JSON)"
      />
    </BaseEditOperationForm>
  );
}

export default ScriptForm;
