import { indentWithTab } from '@codemirror/commands';
import { python } from '@codemirror/lang-python';
import { indentUnit } from '@codemirror/language';
import { keymap } from '@codemirror/view';
import {
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Divider,
  IconButton,
  MenuItem,
  Paper,
  Stack,
  TextField,
  Tooltip,
  Typography,
} from '@mui/material';
import CodeMirror from '@uiw/react-codemirror';
import { useEffect, useMemo, useRef, useState } from 'react';
import { useDiagramProperties } from '../diagram-properties-provider';
import { useNodeManager } from '../node-manager';
import { MaterialSymbol } from '../nodes';
import { useNotification } from '../notification-provider';

export interface ScriptEnvironmentManagerDialogProps {
  open: boolean;
  onClose: () => void;
  initialEnvName?: string;
}

export function ScriptEnvironmentManagerDialog({
  open,
  onClose,
  initialEnvName,
}: ScriptEnvironmentManagerDialogProps) {
  const [diagramProperties, setDiagramProperties] = useDiagramProperties();
  const nodeManager = useNodeManager();
  const showNotification = useNotification();
  const environments = diagramProperties.script_environments || {};

  const [selectedEnvName, setSelectedEnvName] = useState('');
  const [mode, setMode] = useState<'view' | 'edit' | 'create'>('view');
  const [isExpanded, setIsExpanded] = useState(false);

  const [envName, setEnvName] = useState('');
  const [builder, setBuilder] = useState('');
  const [config, setConfig] = useState('{}');
  const [scriptText, setScriptText] = useState('');
  const [selectedCodeField, setSelectedCodeField] = useState('');
  const [language, setLanguage] = useState('');

  const [configError, setConfigError] = useState<string | null>(null);
  const [nameError, setNameError] = useState<string | null>(null);
  const wasOpen = useRef(false);

  useEffect(() => {
    if (open && !wasOpen.current) {
      if (initialEnvName && environments[initialEnvName]) {
        setSelectedEnvName(initialEnvName);
      } else {
        setSelectedEnvName('');
      }
    }
    wasOpen.current = open;
  }, [open, initialEnvName, environments]);

  useEffect(() => {
    if (open && selectedEnvName && environments[selectedEnvName]) {
      const env = environments[selectedEnvName];
      setEnvName(selectedEnvName);
      setBuilder(env.builder);

      const configVal = env.config || {};
      setConfig(JSON.stringify(configVal, null, 2));

      const configObj = configVal as Record<string, unknown>;
      if (
        configObj &&
        'script' in configObj &&
        typeof configObj.script === 'string'
      ) {
        setSelectedCodeField('script');
        setScriptText(configObj.script);
      } else {
        setSelectedCodeField('');
        setScriptText('');
      }

      setLanguage((env as any).language || 'python');
      setMode('view');
    }
  }, [open, selectedEnvName, environments]);

  const getEnvUsageCount = (name: string) => {
    return nodeManager.nodes.filter(
      (node) => node.type === 'script' && node.data.op.environment === name,
    ).length;
  };

  const configKeys = useMemo(() => {
    try {
      const obj = JSON.parse(config);
      if (obj && typeof obj === 'object') {
        return Object.keys(obj).filter((key) => typeof obj[key] === 'string');
      }
    } catch {}
    return [];
  }, [config]);

  const handleScriptTextChange = (newVal: string) => {
    setScriptText(newVal);
    if (!selectedCodeField) return;
    try {
      const obj = JSON.parse(config);
      if (obj && typeof obj === 'object') {
        obj[selectedCodeField] = newVal;
        setConfig(JSON.stringify(obj, null, 2));
      }
    } catch {}
  };

  const handleConfigChange = (newVal: string) => {
    setConfig(newVal);
    try {
      const obj = JSON.parse(newVal);
      if (obj && typeof obj === 'object') {
        setConfigError(null);
        if (selectedCodeField) {
          const currentCodeVal = obj[selectedCodeField];
          if (typeof currentCodeVal === 'string') {
            setScriptText(currentCodeVal);
          } else if (currentCodeVal === undefined || currentCodeVal === null) {
            setScriptText('');
          } else {
            setScriptText(JSON.stringify(currentCodeVal));
          }
        }
      }
    } catch (err) {
      if (err instanceof Error) {
        setConfigError(err.message);
      } else {
        setConfigError('Invalid JSON');
      }
    }
  };

  const handleCodeFieldChange = (newField: string) => {
    setSelectedCodeField(newField);
    if (!newField) {
      setScriptText('');
      return;
    }
    try {
      const obj = JSON.parse(config);
      if (obj && typeof obj === 'object') {
        const val = obj[newField];
        setScriptText(
          typeof val === 'string' ? val : JSON.stringify(val || ''),
        );
      }
    } catch {}
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
      setSelectedEnvName(envName);
      if (mode === 'create') {
        showNotification(
          `Environment '${envName}' created successfully`,
          'success',
        );
      } else if (mode === 'edit') {
        showNotification(
          `Environment '${envName}' saved successfully`,
          'success',
        );
      }
      setMode('view');
    } catch (err) {
      setConfigError('Invalid JSON');
    }
  };

  const handleCreate = () => {
    setEnvName('');
    setBuilder('');
    setConfig('{}');
    setScriptText('');
    setSelectedCodeField('');
    setLanguage('python');
    setMode('create');
  };

  const handleDelete = () => {
    const count = getEnvUsageCount(selectedEnvName);
    if (count > 0) return;

    setDiagramProperties((prev) => {
      const { [selectedEnvName]: _, ...rest } = prev.script_environments || {};
      return {
        ...prev,
        script_environments: rest,
      };
    });
    showNotification(
      `Environment '${selectedEnvName}' deleted successfully`,
      'success',
    );
    setSelectedEnvName('');
    setMode('view');
  };

  const isSaveDisabled = useMemo(() => {
    return !!configError || !!nameError || !envName || !builder;
  }, [configError, nameError, envName, builder]);

  return (
    <Dialog
      open={open}
      onClose={onClose}
      fullWidth
      maxWidth={isExpanded ? 'lg' : 'md'}
      sx={
        isExpanded
          ? {
              '& .MuiDialog-paper': {
                width: '90vw',
                height: '90vh',
                maxWidth: 'none',
                border: '2px solid',
                borderColor: 'primary.main',
              },
            }
          : {}
      }
    >
      <DialogTitle>
        <Stack
          direction="row"
          justifyContent="space-between"
          alignItems="center"
        >
          <Typography variant="h6">Script Environment Manager</Typography>
          <IconButton onClick={() => setIsExpanded(!isExpanded)}>
            <MaterialSymbol
              symbol={isExpanded ? 'fullscreen_exit' : 'fullscreen'}
            />
          </IconButton>
        </Stack>
      </DialogTitle>
      <Divider />
      <DialogContent>
        <Stack spacing={2} sx={{ mt: 1 }}>
          <Stack direction="row" spacing={1} alignItems="center">
            {mode === 'view' ? (
              <TextField
                select
                label="Select Environment"
                value={selectedEnvName}
                onChange={(e) => setSelectedEnvName(e.target.value)}
                fullWidth
              >
                {Object.keys(environments).length === 0 ? (
                  <MenuItem disabled value="">
                    <Typography variant="caption" color="text.disabled">
                      No environments available, please create one
                    </Typography>
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
              <TextField
                label="Environment Name"
                value={envName}
                onChange={(e) => {
                  setEnvName(e.target.value);
                  if (
                    mode === 'create' &&
                    Object.keys(environments).includes(e.target.value)
                  ) {
                    setNameError('Duplicated name');
                  } else {
                    setNameError(null);
                  }
                }}
                fullWidth
                disabled={mode === 'edit'}
                error={!!nameError}
                helperText={nameError}
              />
            )}

            {mode === 'view' && (
              <>
                <Tooltip title="Create new environment">
                  <IconButton onClick={handleCreate}>
                    <MaterialSymbol symbol="add" />
                  </IconButton>
                </Tooltip>
                {selectedEnvName && (
                  <>
                    <Tooltip
                      title={
                        diagramProperties.highlightedEnvironment ===
                        selectedEnvName
                          ? 'Hide nodes'
                          : 'Show nodes'
                      }
                    >
                      <IconButton
                        onClick={() => {
                          setDiagramProperties((prev) => ({
                            ...prev,
                            highlightedEnvironment:
                              prev.highlightedEnvironment === selectedEnvName
                                ? undefined
                                : selectedEnvName,
                          }));
                        }}
                      >
                        <MaterialSymbol
                          symbol={
                            diagramProperties.highlightedEnvironment ===
                            selectedEnvName
                              ? 'visibility_off'
                              : 'visibility'
                          }
                        />
                      </IconButton>
                    </Tooltip>
                    <Tooltip title="Edit environment">
                      <IconButton onClick={() => setMode('edit')}>
                        <MaterialSymbol symbol="edit" />
                      </IconButton>
                    </Tooltip>
                    <Tooltip
                      title={
                        getEnvUsageCount(selectedEnvName) > 0
                          ? `Cannot delete: used by ${getEnvUsageCount(selectedEnvName)} nodes`
                          : 'Delete environment'
                      }
                    >
                      <span>
                        <IconButton
                          disabled={getEnvUsageCount(selectedEnvName) > 0}
                          onClick={handleDelete}
                        >
                          <MaterialSymbol symbol="delete" />
                        </IconButton>
                      </span>
                    </Tooltip>
                  </>
                )}
              </>
            )}

            {mode !== 'view' && (
              <>
                <Button
                  onClick={handleSave}
                  variant="contained"
                  disabled={isSaveDisabled}
                >
                  Save
                </Button>
                <Button
                  onClick={() => {
                    if (mode === 'create') {
                      setSelectedEnvName('');
                      setMode('view');
                    } else {
                      setMode('view');
                      if (selectedEnvName && environments[selectedEnvName]) {
                        const env = environments[selectedEnvName];
                        setEnvName(selectedEnvName);
                        setBuilder(env.builder);

                        const configVal = env.config || {};
                        setConfig(JSON.stringify(configVal, null, 2));

                        const configObj = configVal as Record<string, unknown>;
                        if (
                          configObj &&
                          'script' in configObj &&
                          typeof configObj.script === 'string'
                        ) {
                          setSelectedCodeField('script');
                          setScriptText(configObj.script);
                        } else {
                          setSelectedCodeField('');
                          setScriptText('');
                        }

                        setLanguage((env as any).language || 'python');
                      }
                    }
                  }}
                >
                  Cancel
                </Button>
              </>
            )}
          </Stack>

          {(mode !== 'view' || selectedEnvName) && (
            <>
              <Stack direction="row" spacing={2}>
                <TextField
                  label="Builder"
                  value={builder}
                  onChange={(e) => setBuilder(e.target.value)}
                  fullWidth
                  disabled={mode === 'view'}
                  sx={{ flex: 1 }}
                />
                <TextField
                  select
                  label="Code Field"
                  value={selectedCodeField}
                  onChange={(e) => handleCodeFieldChange(e.target.value)}
                  fullWidth
                  disabled={mode === 'view'}
                  sx={{ flex: 1 }}
                >
                  {configKeys.length === 0 ? (
                    <MenuItem disabled value="">
                      <Typography variant="caption" color="text.disabled">
                        No string fields defined
                      </Typography>
                    </MenuItem>
                  ) : (
                    configKeys.map((key) => (
                      <MenuItem key={key} value={key}>
                        {key}
                      </MenuItem>
                    ))
                  )}
                </TextField>
                <Tooltip title="Support for other languages will be added in the future">
                  <span style={{ flex: 1 }}>
                    <TextField
                      select
                      label="Scripting Language"
                      value={language || 'python'}
                      fullWidth
                      disabled={true}
                    >
                      <MenuItem value="python">Python</MenuItem>
                    </TextField>
                  </span>
                </Tooltip>
              </Stack>

              <TextField
                label="Builder Config (JSON)"
                multiline
                rows={4}
                value={config}
                onChange={(e) => handleConfigChange(e.target.value)}
                fullWidth
                disabled={mode === 'view'}
                error={!!configError}
                helperText={configError}
                slotProps={{
                  htmlInput: {
                    sx: { fontFamily: 'monospace' },
                  },
                }}
              />

              {selectedCodeField && configKeys.includes(selectedCodeField) ? (
                <>
                  <Stack
                    direction="row"
                    justifyContent="space-between"
                    alignItems="center"
                  >
                    <Typography variant="caption" color="text.secondary">
                      Script ({selectedCodeField})
                    </Typography>
                    <IconButton
                      size="small"
                      onClick={() => setIsExpanded(!isExpanded)}
                    >
                      <MaterialSymbol
                        symbol={isExpanded ? 'fullscreen_exit' : 'fullscreen'}
                      />
                    </IconButton>
                  </Stack>
                  <CodeMirror
                    value={scriptText}
                    height={isExpanded ? '50vh' : '300px'}
                    extensions={[
                      python(),
                      indentUnit.of('    '),
                      keymap.of([indentWithTab]),
                    ]}
                    onChange={handleScriptTextChange}
                    theme="dark"
                    readOnly={mode === 'view'}
                  />
                </>
              ) : (
                <Paper
                  variant="outlined"
                  sx={{
                    p: 4,
                    textAlign: 'center',
                    backgroundColor: 'action.hover',
                    borderStyle: 'dashed',
                  }}
                >
                  <Typography color="text.secondary">
                    {configKeys.length === 0
                      ? "Define string fields (like 'script') inside the JSON config above to enable the code editor."
                      : "Select a string field from the 'Code Field' dropdown to open the code editor."}
                  </Typography>
                </Paper>
              )}
            </>
          )}
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>Close</Button>
      </DialogActions>
    </Dialog>
  );
}
