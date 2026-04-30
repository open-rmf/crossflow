import {
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Divider,
  Drawer,
  IconButton,
  InputAdornment,
  List,
  ListItem,
  ListItemText,
  Paper,
  Stack,
  TextField,
  Tooltip,
  Typography,
  useTheme,
} from '@mui/material';
import { MaterialSymbol } from './nodes';
import React from 'react';
import { ScriptEnvironmentDialog } from './forms/script-environment-dialog';
import { useDiagramProperties } from './diagram-properties-provider';
import { useLoadContext } from './load-context-provider';
import { useNodeManager } from './node-manager';
import { RunButton } from './run-button';
import type { InputExample } from './types/api';

const DrawerWidth = '20vw';
const EmptyInputExample: InputExample = {
  description: '',
  value: undefined,
};

export interface DiagramPropertiesDrawerProps {
  open: boolean;
  onClose: () => void;
}

function DiagramPropertiesDrawer({
  open,
  onClose
}: DiagramPropertiesDrawerProps) {
  const [diagramProperties, setDiagramProperties] = useDiagramProperties();
  const loadContext = useLoadContext();
  const theme = useTheme();
  const nodeManager = useNodeManager();

  const getEnvUsageCount = (envBuilder: string) => {
    return nodeManager.nodes.filter(
      (node) => node.type === 'script' && node.data.op.environment === envBuilder
    ).length;
  };
  const [copyTooltipText, setCopyTooltipText] =
    React.useState('Copy this input example into clipboard');
  const [openAddExampleDialog, setOpenAddExampleDialog] =
    React.useState(false);
  const [newInputExample, setNewInputExample] =
    React.useState<InputExample>(EmptyInputExample);

  const [openAddEnvDialog, setOpenAddEnvDialog] = React.useState(false);
  const [openEditEnvDialog, setOpenEditEnvDialog] = React.useState(false);
  const [editingEnvBuilder, setEditingEnvBuilder] = React.useState('');

  React.useEffect(() => {
    setDiagramProperties({
      description: loadContext?.diagram.description ?? '',
      input_examples: loadContext?.diagram.input_examples ?? [],
      script_environments: loadContext?.diagram.script_environments ?? {},
    });
  }, [loadContext]);

  const inputExampleInvalid = React.useMemo(() => {
    return newInputExample.description === '' ||
      newInputExample.value === undefined ||
      newInputExample.value === '';
  }, [newInputExample]);

  return (
    <>
      <Drawer
        sx={{
          width: DrawerWidth,
          flexShrink: 0,
          '& .MuiDrawer-paper': {
            width: DrawerWidth,
          },
        }}
        variant='persistent'
        anchor='right'
        open={open}
      >
        <Stack spacing={2} sx={{ m: 2 }}>
          <Stack direction='row'>
            <Stack direction='row' spacing={2} sx={{ alignItems: 'center' }}>
              <Typography variant='h6'>Description</Typography>
              <Tooltip
                title='General description of what this diagram achieves, as well as
                any other relevant information that may help a user understand its
                execution.'
              >
                <MaterialSymbol symbol='info' fontSize='large' />
              </Tooltip>
            </Stack>
            <Stack direction='row' sx={{ marginLeft: 'auto' }}>
              <Tooltip title='Hide this panel'>
                <IconButton onClick={onClose}>
                  <MaterialSymbol symbol='close' />
                </IconButton>
              </Tooltip>
            </Stack>
          </Stack>
          <TextField
            fullWidth
            multiline
            rows={10}
            maxRows={10}
            variant='outlined'
            value={diagramProperties?.description ?? ''}
            slotProps={{
              htmlInput: { sx: { fontFamily: 'monospace' } },
            }}
            onChange={(d) => setDiagramProperties((prev) =>
              ({ ...prev, description: d.target.value }))}
            sx={{ backgroundColor: theme.palette.background.paper }}
          />
          <Divider />

          {/* Script Environments Section */}
          <Stack direction='row'>
            <Stack direction='row' spacing={2} sx={{ alignItems: 'center' }}>
              <Typography variant='h6'>Script Environments</Typography>
              <Tooltip
                title='Available script environments for this diagram'
              >
                <MaterialSymbol symbol='info' fontSize='large' />
              </Tooltip>
            </Stack>
            <Stack direction='row' sx={{ marginLeft: 'auto' }}>
              <Tooltip title='Add script environment'>
                <IconButton onClick={() => {
                  setOpenAddEnvDialog(true);
                }}>
                  <MaterialSymbol symbol='add' />
                </IconButton>
              </Tooltip>
            </Stack>
          </Stack>
          <Paper>
            <List>
              {diagramProperties && diagramProperties.script_environments &&
              Object.keys(diagramProperties.script_environments).length > 0 ? (
                Object.keys(diagramProperties.script_environments).map((envBuilder) => (
                  <ListItem key={envBuilder}>
                    <ListItemText primary={envBuilder} />

                    <Tooltip title={diagramProperties.highlightedEnvironment === envBuilder ? 'Hide nodes' : 'Show nodes'}>
                      <IconButton
                        onClick={() => {
                          setDiagramProperties((prev) => ({
                            ...prev,
                            highlightedEnvironment: prev.highlightedEnvironment === envBuilder ? undefined : envBuilder,
                          }));
                        }}
                      >
                        <MaterialSymbol
                          symbol={diagramProperties.highlightedEnvironment === envBuilder ? 'visibility_off' : 'visibility'}
                          fontSize='large'
                        />
                      </IconButton>
                    </Tooltip>

                    <Tooltip title='Edit script environment'>
                      <IconButton
                        onClick={() => {
                          setEditingEnvBuilder(envBuilder);
                          setOpenEditEnvDialog(true);
                        }}
                      >
                        <MaterialSymbol symbol='edit' fontSize='large' />
                      </IconButton>
                    </Tooltip>

                    <Tooltip title={getEnvUsageCount(envBuilder) > 0 ? `Cannot delete: used by ${getEnvUsageCount(envBuilder)} nodes` : 'Delete script environment'}>
                      <span>
                        <IconButton
                          disabled={getEnvUsageCount(envBuilder) > 0}
                          onClick={() => {
                            setDiagramProperties((prev) => {
                              const prevEnvs = prev.script_environments ?? {};
                              const { [envBuilder]: _, ...rest } = prevEnvs;
                              return {
                                ...prev,
                                script_environments: rest,
                              };
                            });
                          }}
                        >
                          <MaterialSymbol symbol='delete' fontSize='large' />
                        </IconButton>
                      </span>
                    </Tooltip>
                  </ListItem>
                ))
              ) : (
                <ListItem sx={{ textAlign: 'center' }}>
                  <ListItemText
                    slotProps={{
                      primary: { color: theme.palette.text.disabled },
                    }}
                    primary='No script environments available'
                  />
                </ListItem>
              )}
            </List>
          </Paper>

          <Divider />

          <Stack direction='row'>
            <Stack direction='row' spacing={2} sx={{ alignItems: 'center' }}>
              <Typography variant='h6'>Input Examples</Typography>
              <Tooltip
                title='Input examples that can be executed with this workflow'
              >
                <MaterialSymbol symbol='info' fontSize='large' />
              </Tooltip>
            </Stack>
            <Stack direction='row' sx={{ marginLeft: 'auto' }}>
              <Tooltip title='Add input example'>
                <IconButton onClick={() => setOpenAddExampleDialog(true)}>
                  <MaterialSymbol symbol='add' />
                </IconButton>
              </Tooltip>
            </Stack>
          </Stack>
          <Paper>
            <List>
              {diagramProperties && diagramProperties.input_examples &&
              diagramProperties.input_examples.length > 0 ? (
                diagramProperties.input_examples.map((input, index) => (
                  <ListItem key={index}>
                    <TextField
                      id='input-with-icon-textfield'
                      fullWidth
                      multiline
                      variant='outlined'
                      value={input.value}
                      rows='6'
                      slotProps={{
                        htmlInput: {
                          sx: { fontFamily: 'monospace' },
                        },
                        input: {
                          endAdornment: (
                            <InputAdornment position='end'>
                              <Stack direction='column'>
                                <Tooltip title={input.description}>
                                  <IconButton>
                                    <MaterialSymbol
                                      symbol='info'
                                      fontSize='large'
                                    />
                                  </IconButton>
                                </Tooltip>
                                <Tooltip title='Delete input example'>
                                  <IconButton
                                    onClick={() => {
                                      setDiagramProperties((prev) => {
                                        const prevInputExamples = prev.input_examples ?? [];
                                        if (index >= prevInputExamples.length) {
                                          return prev;
                                        }
                                        return {
                                          ...prev,
                                          input_examples: [
                                            ...prevInputExamples.slice(0, index),
                                            ...prevInputExamples.slice(index + 1),
                                          ],
                                        };
                                      });
                                    }}
                                  >
                                    <MaterialSymbol
                                      symbol='delete'
                                      fontSize='large'
                                    />
                                  </IconButton>
                                </Tooltip>
                                <Tooltip title={copyTooltipText}>
                                  <IconButton
                                    onClick={async () => {
                                      const content =
                                        typeof input.value === 'string'
                                          ? input.value
                                          : JSON.stringify(
                                            input.value, null, 2) || '';
                                      await navigator.clipboard.writeText(content);
                                      setCopyTooltipText('Copied into clipboard!');
                                      setTimeout(() => {
                                        setCopyTooltipText(
                                          'Copy this input example');
                                      }, 3000);
                                    }}
                                  >
                                    <MaterialSymbol
                                      symbol='content_copy'
                                      fontSize='large'
                                    />
                                  </IconButton>
                                </Tooltip>
                                <Tooltip
                                  title='Run diagram with this input example'
                                >
                                  <RunButton
                                    requestJsonString={input.value as string}
                                  />
                                </Tooltip>
                              </Stack>
                            </InputAdornment>
                          )
                        },
                      }}
                    />
                  </ListItem>
                ))
              ) : (
                <ListItem sx={{ textAlign: 'center' }}>
                  <ListItemText
                    slotProps={{
                      primary: { color: theme.palette.text.disabled },
                    }}
                    primary='No input examples available'
                  />
                </ListItem>
              )}
            </List>
          </Paper>
        </Stack>
      </Drawer>
      <Dialog
        onClose={() => {
          setOpenAddExampleDialog(false);
          setNewInputExample(EmptyInputExample);
        }}
        open={openAddExampleDialog}
        fullWidth
        maxWidth='sm'
        keepMounted={false}
      >
        <DialogTitle>Add input example</DialogTitle>
        <Divider />
        <DialogContent>
          <Stack spacing={2}>
            <Typography variant="body1">Value</Typography>
            <TextField
              fullWidth
              multiline
              rows={6}
              variant='outlined'
              value={newInputExample.value}
              onChange={(e) => setNewInputExample((prev) =>
                ({ ...prev, value: e.target.value as string}))}
            />
            <Typography variant="body1">Description</Typography>
            <TextField
              fullWidth
              multiline
              rows={3}
              variant='outlined'
              value={newInputExample.description}
              onChange={(e) => setNewInputExample((prev) =>
                ({ ...prev, description: e.target.value}))}
            />
          </Stack>
        </DialogContent>
        <DialogActions>
          <Button
            variant='contained'
            onClick={() => {
              setDiagramProperties((prev) => {
                const prevInputExamples = prev.input_examples ?? [];
                return {
                  ...prev,
                  input_examples: [
                    ...prevInputExamples,
                    newInputExample
                  ],
                };
              });
              setOpenAddExampleDialog(false);
              setNewInputExample(EmptyInputExample);
            }}
            startIcon={<MaterialSymbol symbol="add" />}
            disabled={inputExampleInvalid}
          >
            Add
          </Button>
        </DialogActions>
      </Dialog>

      {/* Add Script Environment Dialog */}
      <ScriptEnvironmentDialog
        open={openAddEnvDialog}
        onClose={() => setOpenAddEnvDialog(false)}
        onSave={(builder, config) => {
          setDiagramProperties((prev) => ({
            ...prev,
            script_environments: {
              ...prev.script_environments,
              [builder]: {
                builder: builder,
                config: config,
              }
            }
          }));
        }}
        mode="create"
        existingBuilders={Object.keys(diagramProperties.script_environments || {})}
      />

      {/* Edit Script Environment Dialog */}
      <ScriptEnvironmentDialog
        open={openEditEnvDialog}
        onClose={() => setOpenEditEnvDialog(false)}
        onSave={(builder, config) => {
          setDiagramProperties((prev) => ({
            ...prev,
            script_environments: {
              ...prev.script_environments,
              [builder]: {
                builder: builder,
                config: config,
              }
            }
          }));
        }}
        mode="edit"
        initialData={
          editingEnvBuilder
            ? {
                builder: editingEnvBuilder,
                config: diagramProperties.script_environments?.[editingEnvBuilder]?.config || {},
              }
            : undefined
        }
      />
    </>
  );
}

export default DiagramPropertiesDrawer;
