import { useState, useEffect } from 'react';
import {
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Divider,
  Stack,
  TextField,
} from '@mui/material';
import { MaterialSymbol } from '../nodes';

export interface ScriptEnvironmentDialogProps {
  open: boolean;
  onClose: () => void;
  onSave: (builder: string, config: object) => void;
  mode: 'create' | 'edit';
  initialData?: { builder: string; config: object };
  existingBuilders?: string[];
}

export function ScriptEnvironmentDialog({
  open,
  onClose,
  onSave,
  mode,
  initialData,
  existingBuilders = [],
}: ScriptEnvironmentDialogProps) {
  const [builder, setBuilder] = useState('');
  const [builderError, setBuilderError] = useState<string | null>(null);
  const [config, setConfig] = useState('{}');
  const [configError, setConfigError] = useState<string | null>(null);

  useEffect(() => {
    if (open) {
      if (mode === 'edit' && initialData) {
        setBuilder(initialData.builder);
        setConfig(JSON.stringify(initialData.config, null, 2));
      } else {
        setBuilder('');
        setConfig('{}');
      }
      setBuilderError(null);
      setConfigError(null);
    }
  }, [open, mode, initialData]);

  const handleSave = () => {
    try {
      const parsedConfig = JSON.parse(config);
      onSave(builder, parsedConfig);
      onClose();
    } catch (err) {
      setConfigError('Invalid JSON');
    }
  };

  return (
    <Dialog open={open} onClose={onClose} fullWidth maxWidth="sm">
      <DialogTitle>
        {mode === 'create' ? 'Add Script Environment' : 'Edit Script Environment'}
      </DialogTitle>
      <Divider />
      <DialogContent>
        <Stack spacing={2} sx={{ mt: 1 }}>
          <TextField
            label="Builder"
            value={builder}
            onChange={(e) => {
              setBuilder(e.target.value);
              if (mode === 'create') {
                if (existingBuilders.includes(e.target.value)) {
                  setBuilderError('Duplicated Script Environment builder');
                } else if (e.target.value.length === 0) {
                  setBuilderError('Script Environment builder cannot be empty');
                } else {
                  setBuilderError(null);
                }
              }
            }}
            fullWidth
            disabled={mode === 'edit'}
            error={!!builderError}
            helperText={builderError}
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
            error={!!configError}
            helperText={configError}
          />
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>Cancel</Button>
        <Button
          variant="contained"
          onClick={handleSave}
          startIcon={<MaterialSymbol symbol={mode === 'create' ? 'add' : 'save'} />}
          disabled={!!configError || !!builderError || !builder}
        >
          {mode === 'create' ? 'Add' : 'Save'}
        </Button>
      </DialogActions>
    </Dialog>
  );
}
