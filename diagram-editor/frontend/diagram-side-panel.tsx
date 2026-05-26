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
import { v4 as uuidv4 } from 'uuid';
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
  const [localExamples, setLocalExamples] = React.useState<
    (InputExample & { id: string })[]
  >([]);

  React.useEffect(() => {
    const examples = loadContext?.diagram.input_examples ?? [];
    setLocalExamples(examples.map((ex) => ({ ...ex, id: uuidv4() })));
    setDiagramProperties((prev) => ({
      ...prev,
      description: loadContext?.diagram.description ?? '',
      input_examples: examples,
      script_environments: loadContext?.diagram.script_environments ?? {},
    }));
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
                {localExamples.length > 0 ? (
                  localExamples.map((input, index) => (
                    <ListItem key={input.id}>
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
                                        setLocalExamples((prev) => [
                                          ...prev.slice(0, index),
                                          ...prev.slice(index + 1),
                                        ]);
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
                                        await navigator.clipboard.writeText(
                                          getInputExampleRequestJson(input),
                                        );
                                        setCopyTooltipText('Copied');
                                      }}
                                      onMouseLeave={() =>
                                        setCopyTooltipText(
                                          'Copy this input example into clipboard',
                                        )
                                      }
                                    >
                                      <MaterialSymbol
                                        symbol="content_copy"
                                        fontSize="large"
                                      />
                                    </IconButton>
                                  </Tooltip>
                                  <Tooltip title="Use as run request">
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
                  <ListItem>
                    <ListItemText
                      primary="No input examples"
                      secondary="Add an input example to make this workflow easier to run."
                    />
                  </ListItem>
                )}
              </List>
            </Paper>
          </Stack>
        </Box>
      </Drawer>
      <Dialog
        open={openAddExampleDialog}
        onClose={() => setOpenAddExampleDialog(false)}
        maxWidth="sm"
        fullWidth
      >
        <DialogTitle>Add Input Example</DialogTitle>
        <DialogContent>
          <Stack spacing={2} sx={{ mt: 1 }}>
            <TextField
              label="Description"
              value={newInputExample.description}
              onChange={(e) =>
                setNewInputExample((prev) => ({
                  ...prev,
                  description: e.target.value,
                }))
              }
              fullWidth
            />
            <TextField
              label="Value"
              value={
                typeof newInputExample.value === 'string'
                  ? newInputExample.value
                  : JSON.stringify(newInputExample.value, null, 2) || ''
              }
              onChange={(e) => {
                try {
                  setNewInputExample((prev) => ({
                    ...prev,
                    value: JSON.parse(e.target.value),
                  }));
                } catch {
                  setNewInputExample((prev) => ({
                    ...prev,
                    value: e.target.value,
                  }));
                }
              }}
              fullWidth
              multiline
              rows={8}
              slotProps={{
                htmlInput: { sx: { fontFamily: 'monospace' } },
              }}
            />
          </Stack>
        </DialogContent>
        <DialogActions>
          <Button onClick={() => setOpenAddExampleDialog(false)}>Cancel</Button>
          <Button
            variant="contained"
            disabled={inputExampleInvalid}
            onClick={() => {
              const example = { ...newInputExample, id: uuidv4() };
              setLocalExamples((prev) => [...prev, example]);
              setDiagramProperties((prev) => ({
                ...prev,
                input_examples: [
                  ...(prev.input_examples ?? []),
                  {
                    description: newInputExample.description,
                    value: newInputExample.value,
                  },
                ],
              }));
              setNewInputExample(EmptyInputExample);
              setOpenAddExampleDialog(false);
            }}
          >
            Add
          </Button>
        </DialogActions>
      </Dialog>
    </>
  );
}

export default DiagramSidePanel;
