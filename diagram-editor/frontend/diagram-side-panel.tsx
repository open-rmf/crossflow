import {
  Box,
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
  Tab,
  Tabs,
  TextField,
  Tooltip,
  Typography,
  useTheme,
} from '@mui/material';
import React from 'react';
import { useDiagramProperties } from './diagram-properties-provider';
import { useLoadContext } from './load-context-provider';
import { MaterialSymbol } from './nodes';
import { RunPanel } from './run-button';
import type { InputExample } from './types/api';

const DrawerWidth = 'min(420px, calc(100vw - 56px))';
const EmptyInputExample: InputExample = {
  description: '',
  value: undefined,
};

export type DiagramSidePanelTab = 'run' | 'properties';

export interface DiagramSidePanelProps {
  open: boolean;
  tab: DiagramSidePanelTab;
  runRequestJson: string;
  onClose: () => void;
  onRunRequestJsonChange: (requestJson: string) => void;
  onTabChange: (tab: DiagramSidePanelTab) => void;
}

function getInputExampleRequestJson(input: InputExample): string {
  if (typeof input.value === 'string') {
    return input.value;
  }

  return JSON.stringify(input.value, null, 2) || '';
}

function getInputExampleKey(input: InputExample, index: number): string {
  return `${input.description}:${getInputExampleRequestJson(input)}:${index}`;
}

function DiagramSidePanel({
  open,
  tab,
  runRequestJson,
  onClose,
  onRunRequestJsonChange,
  onTabChange,
}: DiagramSidePanelProps) {
  const [diagramProperties, setDiagramProperties] = useDiagramProperties();
  const loadContext = useLoadContext();
  const theme = useTheme();
  const [copyTooltipText, setCopyTooltipText] = React.useState(
    'Copy this input example into clipboard',
  );
  const [openAddExampleDialog, setOpenAddExampleDialog] = React.useState(false);
  const [newInputExample, setNewInputExample] =
    React.useState<InputExample>(EmptyInputExample);

  React.useEffect(() => {
    setDiagramProperties({
      description: loadContext?.diagram.description ?? '',
      input_examples: loadContext?.diagram.input_examples ?? [],
    });
  }, [loadContext, setDiagramProperties]);

  const inputExampleInvalid = React.useMemo(() => {
    return (
      newInputExample.description === '' ||
      newInputExample.value === undefined ||
      newInputExample.value === ''
    );
  }, [newInputExample]);

  return (
    <>
      <Drawer
        sx={{
          width: DrawerWidth,
          flexShrink: 0,
          '& .MuiDrawer-paper': {
            width: DrawerWidth,
            display: 'flex',
            flexDirection: 'column',
          },
        }}
        variant="persistent"
        anchor="right"
        open={open}
      >
        <Stack
          direction="row"
          sx={{
            alignItems: 'center',
            px: 2,
            pt: 1,
          }}
        >
          <Tabs
            value={tab}
            onChange={(_, value: DiagramSidePanelTab) => onTabChange(value)}
            sx={{ minHeight: 40 }}
          >
            <Tab
              value="run"
              label="Run"
              icon={<MaterialSymbol symbol="play_arrow" />}
              iconPosition="start"
            />
            <Tab
              value="properties"
              label="Properties"
              icon={<MaterialSymbol symbol="info" />}
              iconPosition="start"
            />
          </Tabs>
          <Tooltip title="Hide this panel">
            <IconButton onClick={onClose} sx={{ ml: 'auto' }}>
              <MaterialSymbol symbol="close" />
            </IconButton>
          </Tooltip>
        </Stack>
        <Divider />
        <Box
          sx={{
            display: tab === 'run' ? 'flex' : 'none',
            flexDirection: 'column',
            flex: 1,
            minHeight: 0,
          }}
        >
          <RunPanel
            requestJsonString={runRequestJson}
            onRequestJsonStringChange={onRunRequestJsonChange}
          />
        </Box>
        <Box
          sx={{
            display: tab === 'properties' ? 'flex' : 'none',
            flexDirection: 'column',
            flex: 1,
            minHeight: 0,
          }}
        >
          <Stack spacing={2} sx={{ minHeight: 0, overflowY: 'auto', p: 2 }}>
            <Stack direction="row">
              <Stack direction="row" spacing={2} sx={{ alignItems: 'center' }}>
                <Typography variant="h6">Description</Typography>
                <Tooltip title="General description of what this diagram achieves, as well as any other relevant information that may help a user understand its execution.">
                  <MaterialSymbol symbol="info" fontSize="large" />
                </Tooltip>
              </Stack>
            </Stack>
            <TextField
              fullWidth
              multiline
              rows={10}
              maxRows={10}
              variant="outlined"
              value={diagramProperties?.description ?? ''}
              slotProps={{
                htmlInput: { sx: { fontFamily: 'monospace' } },
              }}
              onChange={(d) =>
                setDiagramProperties((prev) => ({
                  ...prev,
                  description: d.target.value,
                }))
              }
              sx={{ backgroundColor: theme.palette.background.paper }}
            />
            <Divider />
            <Stack direction="row">
              <Stack direction="row" spacing={2} sx={{ alignItems: 'center' }}>
                <Typography variant="h6">Input Examples</Typography>
                <Tooltip title="Input examples that can be executed with this workflow">
                  <MaterialSymbol symbol="info" fontSize="large" />
                </Tooltip>
              </Stack>
              <Stack direction="row" sx={{ marginLeft: 'auto' }}>
                <Tooltip title="Add input example">
                  <IconButton onClick={() => setOpenAddExampleDialog(true)}>
                    <MaterialSymbol symbol="add" />
                  </IconButton>
                </Tooltip>
              </Stack>
            </Stack>
            <Paper>
              <List>
                {diagramProperties?.input_examples &&
                diagramProperties.input_examples.length > 0 ? (
                  diagramProperties.input_examples.map((input, index) => (
                    <ListItem key={getInputExampleKey(input, index)}>
                      <TextField
                        fullWidth
                        multiline
                        variant="outlined"
                        value={getInputExampleRequestJson(input)}
                        rows="6"
                        slotProps={{
                          htmlInput: {
                            sx: { fontFamily: 'monospace' },
                          },
                          input: {
                            endAdornment: (
                              <InputAdornment position="end">
                                <Stack direction="column">
                                  <Tooltip title={input.description}>
                                    <IconButton>
                                      <MaterialSymbol
                                        symbol="info"
                                        fontSize="large"
                                      />
                                    </IconButton>
                                  </Tooltip>
                                  <Tooltip title="Delete input example">
                                    <IconButton
                                      onClick={() => {
                                        setDiagramProperties((prev) => {
                                          const prevInputExamples =
                                            prev.input_examples ?? [];
                                          if (
                                            index >= prevInputExamples.length
                                          ) {
                                            return prev;
                                          }
                                          return {
                                            ...prev,
                                            input_examples: [
                                              ...prevInputExamples.slice(
                                                0,
                                                index,
                                              ),
                                              ...prevInputExamples.slice(
                                                index + 1,
                                              ),
                                            ],
                                          };
                                        });
                                      }}
                                    >
                                      <MaterialSymbol
                                        symbol="delete"
                                        fontSize="large"
                                      />
                                    </IconButton>
                                  </Tooltip>
                                  <Tooltip title={copyTooltipText}>
                                    <IconButton
                                      onClick={async () => {
                                        const content =
                                          getInputExampleRequestJson(input);
                                        await navigator.clipboard.writeText(
                                          content,
                                        );
                                        setCopyTooltipText(
                                          'Copied into clipboard!',
                                        );
                                        setTimeout(() => {
                                          setCopyTooltipText(
                                            'Copy this input example',
                                          );
                                        }, 3000);
                                      }}
                                    >
                                      <MaterialSymbol
                                        symbol="content_copy"
                                        fontSize="large"
                                      />
                                    </IconButton>
                                  </Tooltip>
                                  <Tooltip title="Run diagram with this input example">
                                    <IconButton
                                      onClick={() => {
                                        onRunRequestJsonChange(
                                          getInputExampleRequestJson(input),
                                        );
                                        onTabChange('run');
                                      }}
                                    >
                                      <MaterialSymbol
                                        symbol="play_arrow"
                                        fontSize="large"
                                      />
                                    </IconButton>
                                  </Tooltip>
                                </Stack>
                              </InputAdornment>
                            ),
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
                      primary="No input examples available"
                    />
                  </ListItem>
                )}
              </List>
            </Paper>
          </Stack>
        </Box>
      </Drawer>
      <Dialog
        onClose={() => {
          setOpenAddExampleDialog(false);
          setNewInputExample(EmptyInputExample);
        }}
        open={openAddExampleDialog}
        fullWidth
        maxWidth="sm"
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
              variant="outlined"
              value={newInputExample.value}
              onChange={(e) =>
                setNewInputExample((prev) => ({
                  ...prev,
                  value: e.target.value as string,
                }))
              }
            />
            <Typography variant="body1">Description</Typography>
            <TextField
              fullWidth
              multiline
              rows={3}
              variant="outlined"
              value={newInputExample.description}
              onChange={(e) =>
                setNewInputExample((prev) => ({
                  ...prev,
                  description: e.target.value,
                }))
              }
            />
          </Stack>
        </DialogContent>
        <DialogActions>
          <Button
            variant="contained"
            onClick={() => {
              setDiagramProperties((prev) => {
                const prevInputExamples = prev.input_examples ?? [];
                return {
                  ...prev,
                  input_examples: [...prevInputExamples, newInputExample],
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
    </>
  );
}

export default DiagramSidePanel;
