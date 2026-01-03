import {
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

const DrawerWidth = '20vw';

export interface DiagramPropertiesDrawerProps {
  open: boolean;
  onClose: () => void;
}

function DiagramPropertiesDrawer({ open, onClose }: DiagramPropertiesDrawerProps) {
  const [diagramProperties, setDiagramProperties] = useDiagramProperties();
  const loadContext = useLoadContext();
  const theme = useTheme();
  const [copyTooltipText, setCopyTooltipText] =
    React.useState('Copy this example input into clipboard');

  React.useEffect(() => {
    setDiagramProperties({
      description: loadContext?.diagram.description ?? '',
      example_inputs: loadContext?.diagram.example_inputs ?? []
    });
  }, [loadContext]);

  return (
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
            <Typography variant='h4'>Diagram properties</Typography>
            <Tooltip
              title='Properties that may describe the diagram and provide more
              information about its inputs and execution.'
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
        <Divider />
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
            <Typography variant='h6'>Example inputs</Typography>
            <Tooltip
              title='Input examples that can be executed with this workflow'
            >
              <MaterialSymbol symbol='info' fontSize='large' />
            </Tooltip>
          </Stack>
          <Stack direction='row' sx={{ marginLeft: 'auto' }}>
            <Tooltip title='Add example input'>
              <IconButton onClick={() => {}}>
                <MaterialSymbol symbol='add' />
              </IconButton>
            </Tooltip>
          </Stack>
        </Stack>
        <Paper>
          <List>
            {diagramProperties && diagramProperties.example_inputs &&
            diagramProperties.example_inputs.length > 0 ? (
              diagramProperties.example_inputs.map((input, index) => (
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
                            <Stack direction='column' spacing={2}>
                              <Tooltip title={input.description}>
                                <IconButton onClick={() => {}}>
                                  <MaterialSymbol symbol='info' fontSize='large'/>
                                </IconButton>
                              </Tooltip>
                              <Tooltip title={copyTooltipText}>
                                <IconButton
                                  onClick={async () => {
                                    const content =
                                      typeof input.value === 'string'
                                        ? input.value
                                        : JSON.stringify(input.value, null, 2) || '';
                                    await navigator.clipboard.writeText(content);
                                    setCopyTooltipText('Copied into clipboard!');
                                    setTimeout(() => {
                                      setCopyTooltipText('Copy this example input into clipboard');
                                    }, 3000);
                                  }}
                                >
                                  <MaterialSymbol symbol='content_copy' fontSize='large'/>
                                </IconButton>
                              </Tooltip>
                              <Tooltip title='Run diagram with this example input'>
                                <IconButton onClick={() => {}}>
                                  <MaterialSymbol symbol='send' fontSize='large'/>
                                </IconButton>
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
                  primary='No example inputs available'
                />
              </ListItem>
            )}
          </List>
        </Paper>
      </Stack>
    </Drawer>
  );
}

export default DiagramPropertiesDrawer;
