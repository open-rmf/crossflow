import { useState, useEffect, useMemo } from 'react';
import {
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Divider,
  Stack,
  TextField,
  MenuItem,
  Tooltip,
  IconButton,
  Typography,
} from '@mui/material';
import CodeMirror from '@uiw/react-codemirror';
import { python } from '@codemirror/lang-python';
import { indentUnit } from '@codemirror/language';
import { indentWithTab } from '@codemirror/commands';
import { keymap } from '@codemirror/view';
import { MaterialSymbol, DEFAULT_PYTHON_SCRIPT } from '../nodes';
import { useDiagramProperties } from '../diagram-properties-provider';
import { useNodeManager } from '../node-manager';

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
  const environments = diagramProperties.script_environments || {};

  const [selectedEnvName, setSelectedEnvName] = useState('');
  const [mode, setMode] = useState<'view' | 'edit' | 'create'>('view');
  const [isExpanded, setIsExpanded] = useState(false);

  const [envName, setEnvName] = useState('');
  const [builder, setBuilder] = useState('');
  const [config, setConfig] = useState('{}');
  const [scriptText, setScriptText] = useState('');
  const [language, setLanguage] = useState('');

  const [configError, setConfigError] = useState<string | null>(null);
  const [nameError, setNameError] = useState<string | null>(null);

  useEffect(() => {
    if (open) {
      if (initialEnvName && environments[initialEnvName]) {
        setSelectedEnvName(initialEnvName);
      } else {
        setSelectedEnvName('');
      }
    }
  }, [open, initialEnvName, environments]);

  useEffect(() => {
    if (open && selectedEnvName && environments[selectedEnvName]) {
      const env = environments[selectedEnvName];
      setEnvName(selectedEnvName);
      setBuilder(env.builder);
      setConfig(JSON.stringify(env.config || {}, null, 2));
      setScriptText((env as any).script || '');
      setLanguage((env as any).language || 'python');
      setMode('view');
    }
  }, [open, selectedEnvName, environments]);

  const getEnvUsageCount = (name: string) => {
    return nodeManager.nodes.filter(
      (node) => node.type === 'script' && node.data.op.environment === name
    ).length;
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
            script: scriptText,
            language,
          },
        },
      }));
      setSelectedEnvName(envName);
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
    setLanguage('');
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
      sx={isExpanded ? {
        '& .MuiDialog-paper': {
          width: '90vw',
          height: '90vh',
          maxWidth: 'none',
          border: '2px solid',
          borderColor: 'primary.main',
        }
      } : {}}
    >
      <DialogTitle>
        <Stack direction="row" justifyContent="space-between" alignItems="center">
          <Typography variant="h6">Script Environment Manager</Typography>
          <IconButton onClick={() => setIsExpanded(!isExpanded)}>
            <MaterialSymbol symbol={isExpanded ? 'fullscreen_exit' : 'fullscreen'} />
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
                {Object.keys(environments).map((name) => (
                  <MenuItem key={name} value={name}>
                    {name}
                  </MenuItem>
                ))}
              </TextField>
            ) : (
              <TextField
                label="Environment Name"
                value={envName}
                onChange={(e) => {
                  setEnvName(e.target.value);
                  if (mode === 'create' && Object.keys(environments).includes(e.target.value)) {
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
                    <Tooltip title="Edit environment">
                      <IconButton onClick={() => setMode('edit')}>
                        <MaterialSymbol symbol="edit" />
                      </IconButton>
                    </Tooltip>
                    <Tooltip title={getEnvUsageCount(selectedEnvName) > 0 ? `Cannot delete: used by ${getEnvUsageCount(selectedEnvName)} nodes` : 'Delete environment'}>
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
                <Button onClick={handleSave} variant="contained" disabled={isSaveDisabled}>
                  Save
                </Button>
                <Button onClick={() => {
                  if (mode === 'create') {
                    setSelectedEnvName('');
                    setMode('view');
                  } else {
                    setMode('view');
                    // Reset fields to selected env
                    if (selectedEnvName && environments[selectedEnvName]) {
                      const env = environments[selectedEnvName];
                      setEnvName(selectedEnvName);
                      setBuilder(env.builder);
                      setConfig(JSON.stringify(env.config || {}, null, 2));
                      setScriptText((env as any).script || '');
                      setLanguage((env as any).language || 'python');
                    }
                  }
                }}>
                  Cancel
                </Button>
              </>
            )}
          </Stack>

          {(mode !== 'view' || selectedEnvName) && (
            <>
              <TextField
                label="Builder"
                value={builder}
                onChange={(e) => setBuilder(e.target.value)}
                fullWidth
                disabled={mode === 'view'}
              />

              <TextField
                label="Config (JSON)"
                multiline
                rows={4}
                value={config}
                onChange={(e) => {
                  setConfig(e.target.value);
                  try {
                    JSON.parse(e.target.value);
                    setConfigError(null);
                  } catch (err) {
                    if (err instanceof Error) {
                      setConfigError(err.message);
                    } else {
                      setConfigError('Invalid JSON');
                    }
                  }
                }}
                fullWidth
                disabled={mode === 'view'}
                error={!!configError}
                helperText={configError}
              />

              <Stack direction="row" spacing={2} alignItems="center">
                <TextField
                  select
                  label="Scripting Language"
                  value={language}
                  onChange={(e) => {
                    const newLang = e.target.value;
                    setLanguage(newLang);
                    if (newLang === 'python' && !scriptText) {
                      setScriptText(DEFAULT_PYTHON_SCRIPT);
                    }
                  }}
                  fullWidth
                  disabled={mode === 'view'}
                >
                  <MenuItem value="python">Python</MenuItem>
                  <MenuItem disabled>
                    <Typography variant="caption" color="text.disabled">
                      Open an issue ticket for more languages
                    </Typography>
                  </MenuItem>
                </TextField>
              </Stack>

              {language === 'python' && (
                <>
                  <Typography variant="caption" color="text.secondary">
                    Script
                  </Typography>
                  <CodeMirror
                    value={scriptText}
                    height={isExpanded ? "50vh" : "300px"}
                    extensions={[
                      python(),
                      indentUnit.of("    "),
                      keymap.of([indentWithTab]),
                    ]}
                    onChange={(value) => setScriptText(value)}
                    theme="dark"
                    readOnly={mode === 'view'}
                  />
                </>
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
