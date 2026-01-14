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
import { useDiagramProperties } from './diagram-properties-provider';
import { useLoadContext } from './load-context-provider';
import { RunButton } from './run-button';
import type { ExampleInput } from './types/api';

const DrawerWidth = '20vw';
const EmptyExampleInput: ExampleInput = {
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
  const [copyTooltipText, setCopyTooltipText] =
    React.useState('Copy this input example into clipboard');
  const [openAddExampleDialog, setOpenAddExampleDialog] =
    React.useState(false);
  const [newExampleInput, setNewExampleInput] =
    React.useState<ExampleInput>(EmptyExampleInput);

  React.useEffect(() => {
    setDiagramProperties({
      description: loadContext?.diagram.description ?? '',
      input_examples: loadContext?.diagram.input_examples ?? []
    });
  }, [loadContext]);

  const exampleInputInvalid = React.useMemo(() => {
    return newExampleInput.description === '' ||
      newExampleInput.value === undefined ||
      newExampleInput.value === '';
  }, [newExampleInput]);

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
                                        const prevExampleInputs = prev.input_examples ?? [];
                                        if (index >= prevExampleInputs.length) {
                                          return prev;
                                        }
                                        return {
                                          ...prev,
                                          input_examples: [
                                            ...prevExampleInputs.slice(0, index),
                                            ...prevExampleInputs.slice(index + 1),
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
          setNewExampleInput(EmptyExampleInput);
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
              value={newExampleInput.value}
              onChange={(e) => setNewExampleInput((prev) =>
                ({ ...prev, value: e.target.value as string}))}
            />
            <Typography variant="body1">Description</Typography>
            <TextField
              fullWidth
              multiline
              rows={3}
              variant='outlined'
              value={newExampleInput.description}
              onChange={(e) => setNewExampleInput((prev) =>
                ({ ...prev, description: e.target.value}))}
            />
          </Stack>
        </DialogContent>
        <DialogActions>
          <Button
            variant='contained'
            onClick={() => {
              setDiagramProperties((prev) => {
                const prevExampleInputs = prev.input_examples ?? [];
                return {
                  ...prev,
                  input_examples: [
                    ...prevExampleInputs,
                    newExampleInput
                  ],
                };
              });
              setOpenAddExampleDialog(false);
              setNewExampleInput(EmptyExampleInput);
            }}
            startIcon={<MaterialSymbol symbol="add" />}
            disabled={exampleInputInvalid}
          >
            Add
          </Button>
        </DialogActions>
      </Dialog>
    </>
  );
}

export default DiagramPropertiesDrawer;
