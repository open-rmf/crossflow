import {
  Alert,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
} from '@mui/material';
import { useCallback, useRef } from 'react';

interface NewDiagramDialogProps {
  open: boolean;
  canSave: boolean;
  onCancel: () => void;
  onDiscard: () => void;
  onSave: () => void;
}

export function useNewDiagramAfterExport(
  onExportCompleted: (filename: string) => void,
  onStartNew: () => void,
) {
  const pending = useRef(false);

  const begin = useCallback(() => {
    pending.current = true;
  }, []);
  const cancel = useCallback(() => {
    pending.current = false;
  }, []);
  const complete = useCallback(
    (filename: string) => {
      onExportCompleted(filename);
      if (pending.current) {
        pending.current = false;
        onStartNew();
      }
    },
    [onExportCompleted, onStartNew],
  );

  return { begin, cancel, complete };
}

export function NewDiagramDialog({
  open,
  canSave,
  onCancel,
  onDiscard,
  onSave,
}: NewDiagramDialogProps) {
  return (
    <Dialog open={open} onClose={onCancel}>
      <DialogTitle>Start a new diagram?</DialogTitle>
      <DialogContent>
        <DialogContentText>
          Your current diagram has changes. Save it to a file before starting a
          new diagram?
        </DialogContentText>
        {!canSave && (
          <Alert severity="warning" sx={{ mt: 2 }}>
            Some editor changes cannot be exported yet. Fix or discard them
            before saving to a file.
          </Alert>
        )}
      </DialogContent>
      <DialogActions>
        <Button onClick={onCancel}>Cancel</Button>
        <Button color="error" onClick={onDiscard}>
          Discard
        </Button>
        <Button variant="contained" disabled={!canSave} onClick={onSave}>
          Save &amp; New
        </Button>
      </DialogActions>
    </Dialog>
  );
}
