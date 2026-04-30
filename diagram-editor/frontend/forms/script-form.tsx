import { useState } from 'react';
import { IconButton, TextField, MenuItem, Stack, Tooltip, Typography } from '@mui/material';
import CodeMirror from '@uiw/react-codemirror';
import { python } from '@codemirror/lang-python';
import { indentUnit } from '@codemirror/language';
import { indentWithTab } from '@codemirror/commands';
import { keymap } from '@codemirror/view';
import BaseEditOperationForm, {
  type BaseEditOperationFormProps,
} from './base-edit-operation-form';
import { MaterialSymbol } from '../nodes';
import { ScriptEnvironmentDialog } from './script-environment-dialog';
import { useDiagramProperties } from '../diagram-properties-provider';

export type ScriptFormProps = BaseEditOperationFormProps<'script'>;

function ScriptForm(props: ScriptFormProps) {
  const [diagramProperties, setDiagramProperties] = useDiagramProperties();
  const environments = diagramProperties.script_environments || {};

  const [isExpanded, setIsExpanded] = useState(false);
  const [dialogOpen, setDialogOpen] = useState(false);

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

  const handleScriptChange = (text: string) => {
    const updatedNode = {
      ...props.node,
      data: {
        ...props.node.data,
        op: {
          ...props.node.data.op,
          run: {
             ...props.node.data.op.run,
             text: text,
          }
        },
      },
    };
    props.onChange?.({
      type: 'replace',
      id: props.node.id,
      item: updatedNode,
    });
  };

  return (
    <BaseEditOperationForm {...props}>
      <Stack
        spacing={2}
        sx={
          isExpanded
            ? {
                position: 'fixed',
                top: '5%',
                left: '5%',
                width: '90%',
                height: '90%',
                zIndex: 2000,
                bgcolor: 'background.paper',
                p: 3,
                boxShadow: 24,
                overflow: 'auto',
                border: '2px solid',
                borderColor: 'primary.main',
              }
            : { width: 800 }
        }
      >
        <Stack direction="row" spacing={1} alignItems="center">
          <TextField
            select
            label="Script Environment"
            value={props.node.data.op.environment || ''}
            onChange={(e) => handleEnvChange(e.target.value)}
            fullWidth
          >
            {Object.keys(environments).map((envName) => (
              <MenuItem key={envName} value={envName}>
                {envName}
              </MenuItem>
            ))}
          </TextField>

          <TextField
            select
            label="Scripting Language"
            value="python"
            fullWidth
            disabled={!props.node.data.op.environment}
          >
            <MenuItem value="python">Python</MenuItem>
            <MenuItem disabled>
              <Typography variant="caption" color="text.disabled">
                Open an issue ticket to request for support for more languages
              </Typography>
            </MenuItem>
          </TextField>

          <Tooltip title={"Add a new script environment"}>
            <IconButton
              onClick={() => {
                setDialogOpen(true);
              }}
            >
              <MaterialSymbol symbol='add'/>
            </IconButton>
          </Tooltip>

          <Tooltip title={isExpanded ? "Shrink form" : "Expand form"}>
            <IconButton onClick={() => setIsExpanded(!isExpanded)}>
              <MaterialSymbol symbol={isExpanded ? 'fullscreen_exit' : 'fullscreen'} />
            </IconButton>
          </Tooltip>
        </Stack>

        <ScriptEnvironmentDialog
          open={dialogOpen}
          onClose={() => setDialogOpen(false)}
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
            handleEnvChange(builder);
          }}
          mode="create"
          existingBuilders={Object.keys(environments)}
        />

        {props.node.data.op.environment && (
          <Stack spacing={0.5}>
            <Typography variant="caption" color="text.secondary">
              Python Script
            </Typography>
            <CodeMirror
              value={props.node.data.op.run?.text || ''}
              height={isExpanded ? "70vh" : "400px"}
              extensions={[
                python(),
                indentUnit.of("    "),
                keymap.of([indentWithTab]),
              ]}
              onChange={(value) => handleScriptChange(value)}
              theme="dark"
            />
          </Stack>
        )}
      </Stack>
    </BaseEditOperationForm>
  );
}

export default ScriptForm;
