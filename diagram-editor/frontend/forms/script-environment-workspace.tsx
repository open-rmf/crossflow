import { indentWithTab } from '@codemirror/commands';
import { indentUnit } from '@codemirror/language';
import { keymap } from '@codemirror/view';
import {
  Alert,
  Box,
  Button,
  Divider,
  IconButton,
  List,
  ListItemButton,
  ListItemText,
  MenuItem,
  Paper,
  Stack,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material';
import CodeMirror from '@uiw/react-codemirror';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useDiagramProperties } from '../diagram-properties-provider';
import { useDiagramSidePanel } from '../diagram-side-panel-controller';
import { useNodeManager } from '../node-manager';
import { MaterialSymbol } from '../nodes';
import { useNotification } from '../notification-provider';
import { useRegistry } from '../registry-provider';
import type { DiagramElementMetadata } from '../types/api';
import { ScriptEnvironmentPanel } from './script-environment-panel';
import { scriptEnvironmentPlugins } from './script-environments/registry';

function objectConfig(value: unknown): Record<string, unknown> {
  return typeof value === 'object' && value !== null
    ? (value as Record<string, unknown>)
    : {};
}

export function ScriptEnvironmentWorkspace() {
  const [diagramProperties, setDiagramProperties] = useDiagramProperties();
  const nodeManager = useNodeManager();
  const showNotification = useNotification();
  const rawRegistry = useRegistry();
  const registry = rawRegistry as unknown as DiagramElementMetadata;
  const environments = diagramProperties.script_environments || {};
  const {
    state: { selectedEnvironmentName, mode, expanded },
    selectEnvironment,
    startEnvironmentCreate,
    startEnvironmentEdit,
    finishEnvironmentEdit,
    setExpanded,
  } = useDiagramSidePanel();

  const registeredBuilders = useMemo(
    () => (registry.scripting ? Object.keys(registry.scripting) : []),
    [registry],
  );
  const defaultBuilder = registeredBuilders.includes('process-bound-python')
    ? 'process-bound-python'
    : registeredBuilders[0] || '';

  const [envName, setEnvName] = useState('');
  const [builder, setBuilder] = useState('');
  const [config, setConfig] = useState('{}');
  const [scriptText, setScriptText] = useState('');
  const [selectedCodeField, setSelectedCodeField] = useState('');
  const [language, setLanguage] = useState('');
  const [configError, setConfigError] = useState<string | null>(null);
  const [nameError, setNameError] = useState<string | null>(null);
  const previousMode = useRef(mode);

  const plugin = scriptEnvironmentPlugins[builder];
  const activeCodeField = plugin?.defaultCodeField || selectedCodeField;

  const applyBuilder = useCallback(
    (selectedBuilder: string, bootstrap: boolean) => {
      setBuilder(selectedBuilder);
      const metadata = registry.scripting?.[selectedBuilder];
      const selectedPlugin = scriptEnvironmentPlugins[selectedBuilder];
      setLanguage(metadata?.language || 'python');
      setSelectedCodeField(selectedPlugin?.defaultCodeField || '');

      if (!bootstrap) {
        return;
      }

      const example = metadata?.config_examples?.[0]?.config;
      const bootstrapConfig =
        example && typeof example === 'object'
          ? example
          : selectedPlugin?.bootstrapConfig
            ? JSON.parse(selectedPlugin.bootstrapConfig())
            : {};
      const configObject = objectConfig(bootstrapConfig);
      const codeField = selectedPlugin?.defaultCodeField || '';
      const code =
        codeField && typeof configObject[codeField] === 'string'
          ? (configObject[codeField] as string)
          : '';

      setConfig(JSON.stringify(configObject, null, 2));
      setScriptText(code);
      setConfigError(null);
    },
    [registry.scripting],
  );

  const loadEnvironmentDraft = useCallback(
    (name: string) => {
      const environment = environments[name];
      if (!environment) {
        return;
      }

      const configObject = objectConfig(environment.config);
      const environmentPlugin = scriptEnvironmentPlugins[environment.builder];
      const codeField = environmentPlugin?.defaultCodeField || '';
      setEnvName(name);
      setBuilder(environment.builder);
      setLanguage(
        registry.scripting?.[environment.builder]?.language || 'python',
      );
      setConfig(JSON.stringify(configObject, null, 2));
      setSelectedCodeField(codeField);
      setScriptText(
        codeField && typeof configObject[codeField] === 'string'
          ? (configObject[codeField] as string)
          : '',
      );
      setConfigError(null);
      setNameError(null);
    },
    [environments, registry.scripting],
  );

  useEffect(() => {
    if (mode === 'create' && previousMode.current !== 'create') {
      setEnvName('');
      setNameError(null);
      setConfigError(null);
      applyBuilder(defaultBuilder, true);
    } else if (
      mode !== 'create' &&
      selectedEnvironmentName &&
      environments[selectedEnvironmentName]
    ) {
      loadEnvironmentDraft(selectedEnvironmentName);
    }
    previousMode.current = mode;
  }, [
    applyBuilder,
    defaultBuilder,
    environments,
    loadEnvironmentDraft,
    mode,
    selectedEnvironmentName,
  ]);

  const startFreshEnvironment = useCallback(() => {
    setEnvName('');
    setNameError(null);
    setConfigError(null);
    applyBuilder(defaultBuilder, true);
    startEnvironmentCreate();
  }, [applyBuilder, defaultBuilder, startEnvironmentCreate]);

  const getEnvUsageCount = useCallback(
    (name: string) =>
      nodeManager.nodes.filter(
        (node) => node.type === 'script' && node.data.op.environment === name,
      ).length,
    [nodeManager.nodes],
  );

  const configKeys = useMemo(() => {
    try {
      const parsed = JSON.parse(config);
      if (parsed && typeof parsed === 'object') {
        return Object.keys(parsed).filter(
          (key) => typeof parsed[key] === 'string',
        );
      }
    } catch {
      return [];
    }
    return [];
  }, [config]);

  const handleConfigChange = (newValue: string) => {
    setConfig(newValue);
    try {
      const parsed = JSON.parse(newValue);
      setConfigError(null);
      if (
        selectedCodeField &&
        parsed &&
        typeof parsed === 'object' &&
        typeof parsed[selectedCodeField] === 'string'
      ) {
        setScriptText(parsed[selectedCodeField]);
      }
    } catch (error) {
      setConfigError(error instanceof Error ? error.message : 'Invalid JSON');
    }
  };

  const handleScriptTextChange = (newValue: string) => {
    setScriptText(newValue);
    if (!activeCodeField) {
      return;
    }
    try {
      const parsed = JSON.parse(config);
      if (parsed && typeof parsed === 'object') {
        parsed[activeCodeField] = newValue;
        setConfig(JSON.stringify(parsed, null, 2));
      }
    } catch {
      return;
    }
  };

  const handleCodeFieldChange = (newField: string) => {
    setSelectedCodeField(newField);
    if (!newField) {
      setScriptText('');
      return;
    }
    try {
      const parsed = JSON.parse(config);
      const value = parsed?.[newField];
      setScriptText(typeof value === 'string' ? value : '');
    } catch {
      setScriptText('');
    }
  };

  const handleSave = () => {
    try {
      const parsedConfig = JSON.parse(config);
      setDiagramProperties((prev) => ({
        ...prev,
        script_environments: {
          ...prev.script_environments,
          [envName]: {
            builder,
            config: parsedConfig,
          },
        },
      }));
      selectEnvironment(envName);
      showNotification(
        mode === 'create'
          ? `Environment '${envName}' created successfully`
          : `Environment '${envName}' saved successfully`,
        'success',
      );
      finishEnvironmentEdit();
    } catch {
      setConfigError('Invalid JSON');
    }
  };

  const handleCancel = () => {
    if (selectedEnvironmentName && environments[selectedEnvironmentName]) {
      loadEnvironmentDraft(selectedEnvironmentName);
    }
    finishEnvironmentEdit();
  };

  const handleDelete = () => {
    if (
      !selectedEnvironmentName ||
      getEnvUsageCount(selectedEnvironmentName) > 0
    ) {
      return;
    }

    setDiagramProperties((prev) => {
      const { [selectedEnvironmentName]: _, ...rest } =
        prev.script_environments || {};
      return {
        ...prev,
        script_environments: rest,
        highlightedEnvironment:
          prev.highlightedEnvironment === selectedEnvironmentName
            ? undefined
            : prev.highlightedEnvironment,
      };
    });
    showNotification(
      `Environment '${selectedEnvironmentName}' deleted successfully`,
      'success',
    );
    selectEnvironment(undefined);
  };

  const isSaveDisabled =
    !!configError ||
    !!nameError ||
    !envName ||
    !builder ||
    !registeredBuilders.includes(builder);

  const selectedEnvironment = selectedEnvironmentName
    ? environments[selectedEnvironmentName]
    : undefined;
  const selectedMetadata = selectedEnvironment
    ? registry.scripting?.[selectedEnvironment.builder]
    : undefined;

  const renderEnvironmentActions = () => (
    <Stack direction="row" spacing={0.5} alignItems="center">
      <Tooltip title="Create new environment">
        <span>
          <IconButton
            aria-label="Create new environment"
            onClick={startFreshEnvironment}
            disabled={registeredBuilders.length === 0}
          >
            <MaterialSymbol symbol="add" />
          </IconButton>
        </span>
      </Tooltip>
      {selectedEnvironmentName && selectedEnvironment && (
        <>
          <Tooltip
            title={
              diagramProperties.highlightedEnvironment ===
              selectedEnvironmentName
                ? 'Hide nodes'
                : 'Show nodes'
            }
          >
            <IconButton
              aria-label={
                diagramProperties.highlightedEnvironment ===
                selectedEnvironmentName
                  ? 'Hide nodes'
                  : 'Show nodes'
              }
              onClick={() => {
                setDiagramProperties((prev) => ({
                  ...prev,
                  highlightedEnvironment:
                    prev.highlightedEnvironment === selectedEnvironmentName
                      ? undefined
                      : selectedEnvironmentName,
                }));
              }}
            >
              <MaterialSymbol
                symbol={
                  diagramProperties.highlightedEnvironment ===
                  selectedEnvironmentName
                    ? 'visibility_off'
                    : 'visibility'
                }
              />
            </IconButton>
          </Tooltip>
          <Tooltip title="Edit environment">
            <IconButton
              aria-label="Edit environment"
              onClick={startEnvironmentEdit}
            >
              <MaterialSymbol symbol="edit" />
            </IconButton>
          </Tooltip>
          <Tooltip
            title={
              getEnvUsageCount(selectedEnvironmentName) > 0
                ? `Cannot delete: used by ${getEnvUsageCount(
                    selectedEnvironmentName,
                  )} nodes`
                : 'Delete environment'
            }
          >
            <span>
              <IconButton
                aria-label="Delete environment"
                disabled={getEnvUsageCount(selectedEnvironmentName) > 0}
                onClick={handleDelete}
              >
                <MaterialSymbol symbol="delete" />
              </IconButton>
            </span>
          </Tooltip>
        </>
      )}
    </Stack>
  );

  if (mode === 'view' && selectedEnvironmentName && !selectedEnvironment) {
    return (
      <Stack spacing={2} sx={{ p: 2 }}>
        <Alert
          severity="warning"
          action={
            <Button onClick={startFreshEnvironment}>Create environment</Button>
          }
        >
          Environment “{selectedEnvironmentName}” is not defined.
        </Alert>
      </Stack>
    );
  }

  return (
    <Stack spacing={2} sx={{ p: 2, minWidth: 0, overflowY: 'auto' }}>
      <Stack direction="row" spacing={1} alignItems="center">
        {mode === 'view' ? (
          <TextField
            select
            label="Select Environment"
            value={selectedEnvironmentName || ''}
            onChange={(event) =>
              selectEnvironment(event.target.value || undefined)
            }
            fullWidth
          >
            {Object.keys(environments).length === 0 ? (
              <MenuItem disabled value="">
                No environments available
              </MenuItem>
            ) : (
              Object.keys(environments).map((name) => (
                <MenuItem key={name} value={name}>
                  {name}
                </MenuItem>
              ))
            )}
          </TextField>
        ) : (
          <Typography variant="h6" sx={{ flex: 1 }}>
            {mode === 'create' ? 'Create Environment' : 'Edit Environment'}
          </Typography>
        )}
        {mode === 'view' ? (
          renderEnvironmentActions()
        ) : (
          <>
            <Tooltip
              title={
                expanded
                  ? 'Collapse environment editor'
                  : 'Expand environment editor'
              }
            >
              <IconButton
                aria-label={
                  expanded
                    ? 'Collapse environment editor'
                    : 'Expand environment editor'
                }
                onClick={() => setExpanded(!expanded)}
              >
                <MaterialSymbol
                  symbol={expanded ? 'fullscreen_exit' : 'fullscreen'}
                />
              </IconButton>
            </Tooltip>
            <Button
              variant="contained"
              disabled={isSaveDisabled}
              onClick={handleSave}
            >
              Save
            </Button>
            <Button onClick={handleCancel}>Cancel</Button>
          </>
        )}
      </Stack>

      {mode === 'view' ? (
        selectedEnvironmentName && selectedEnvironment ? (
          <ScriptEnvironmentPanel
            environmentName={selectedEnvironmentName}
            environment={selectedEnvironment}
            metadata={selectedMetadata}
          />
        ) : (
          <Paper
            variant="outlined"
            sx={{ p: 3, textAlign: 'center', borderStyle: 'dashed' }}
          >
            <Typography color="text.secondary">
              Select an environment to inspect its runtime and configuration.
            </Typography>
          </Paper>
        )
      ) : registeredBuilders.length === 0 ? (
        <Alert severity="warning">
          No script environment builders are available.
        </Alert>
      ) : (
        <Box
          sx={{
            display: 'grid',
            gridTemplateColumns: expanded
              ? {
                  xs: 'minmax(0, 1fr)',
                  md: '260px minmax(0, 1fr)',
                }
              : 'minmax(0, 1fr)',
            gap: 2,
            minWidth: 0,
          }}
        >
          {expanded && (
            <Paper variant="outlined" sx={{ minWidth: 0 }}>
              <Stack sx={{ p: 1 }}>
                <Button
                  startIcon={<MaterialSymbol symbol="add" />}
                  onClick={startFreshEnvironment}
                >
                  New environment
                </Button>
              </Stack>
              <Divider />
              <List dense>
                {Object.keys(environments).map((name) => (
                  <ListItemButton
                    key={name}
                    selected={name === selectedEnvironmentName}
                    onClick={() => {
                      selectEnvironment(name);
                      startEnvironmentEdit();
                      loadEnvironmentDraft(name);
                    }}
                  >
                    <ListItemText primary={name} />
                  </ListItemButton>
                ))}
              </List>
            </Paper>
          )}

          <Stack spacing={2} minWidth={0}>
            <TextField
              label="Environment Name"
              value={envName}
              onChange={(event) => {
                const nextName = event.target.value;
                setEnvName(nextName);
                setNameError(
                  mode === 'create' && environments[nextName]
                    ? 'Duplicated name'
                    : null,
                );
              }}
              disabled={mode === 'edit'}
              error={!!nameError}
              helperText={nameError}
              fullWidth
            />
            <Stack direction={{ xs: 'column', md: 'row' }} spacing={2}>
              <TextField
                select
                label="Builder"
                value={builder}
                onChange={(event) => applyBuilder(event.target.value, true)}
                fullWidth
              >
                {registeredBuilders.map((registeredBuilder) => (
                  <MenuItem key={registeredBuilder} value={registeredBuilder}>
                    {registeredBuilder}
                  </MenuItem>
                ))}
              </TextField>
              {!plugin?.defaultCodeField && (
                <TextField
                  select
                  label="Code Field"
                  value={selectedCodeField}
                  onChange={(event) =>
                    handleCodeFieldChange(event.target.value)
                  }
                  fullWidth
                >
                  {configKeys.length === 0 ? (
                    <MenuItem disabled value="">
                      No string fields defined
                    </MenuItem>
                  ) : (
                    configKeys.map((key) => (
                      <MenuItem key={key} value={key}>
                        {key}
                      </MenuItem>
                    ))
                  )}
                </TextField>
              )}
              <TextField
                label="Scripting Language"
                value={language || 'python'}
                disabled
                fullWidth
              />
            </Stack>

            {plugin ? (
              <plugin.PropertiesForm
                config={config}
                onChange={setConfig}
                onValidationError={setConfigError}
                mode={mode}
                registry={registry}
                scriptText={scriptText}
              />
            ) : (
              <TextField
                label="Builder Config (JSON)"
                multiline
                rows={4}
                value={config}
                onChange={(event) => handleConfigChange(event.target.value)}
                error={!!configError}
                helperText={configError}
                fullWidth
                slotProps={{
                  htmlInput: { sx: { fontFamily: 'monospace' } },
                }}
              />
            )}

            {activeCodeField &&
            (plugin || configKeys.includes(activeCodeField)) ? (
              <>
                <Typography variant="caption" color="text.secondary">
                  Script ({activeCodeField})
                </Typography>
                <CodeMirror
                  value={scriptText}
                  height={expanded ? '50vh' : '300px'}
                  extensions={[
                    ...(plugin?.codeExtensions?.() || []),
                    indentUnit.of('    '),
                    keymap.of([indentWithTab]),
                  ]}
                  onChange={handleScriptTextChange}
                  theme="dark"
                />
              </>
            ) : (
              <Paper
                variant="outlined"
                sx={{
                  p: 3,
                  textAlign: 'center',
                  backgroundColor: 'action.hover',
                  borderStyle: 'dashed',
                }}
              >
                <Typography color="text.secondary">
                  Define and select a string config field to edit its script.
                </Typography>
              </Paper>
            )}
          </Stack>
        </Box>
      )}
    </Stack>
  );
}
