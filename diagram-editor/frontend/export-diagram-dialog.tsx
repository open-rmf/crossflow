import {
  Button,
  ButtonGroup,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Stack,
  TextField,
  Typography,
  useTheme,
} from '@mui/material';
import { deflateSync, strToU8 } from 'fflate';
import React, { Suspense, use, useMemo } from 'react';
import { useLoadContext } from './load-context-provider';
import { useNodeManager } from './node-manager';
import { MaterialSymbol } from './nodes';
import { saveState } from './persist-state';
import { useRegistry } from './registry-provider';
import { useTemplates } from './templates-provider';
import { useEdges } from './use-edges';
import { exportDiagram } from './utils/export-diagram';

export interface ExportDiagramDialogProps {
  open: boolean;
  onClose: () => void;
}

interface DialogData {
  shareLink: string;
  diagramJson: string;
}

function ExportDiagramDialogInternal({
  open,
  onClose,
}: ExportDiagramDialogProps) {
  const nodeManager = useNodeManager();
  const edges = useEdges();
  const [templates] = useTemplates();
  const registry = useRegistry();
  const loadContext = useLoadContext();
  const theme = useTheme();

  const dialogDataPromise = useMemo(async () => {
    const diagram = exportDiagram(registry, nodeManager, edges, templates);
    if (loadContext?.diagram.extensions) {
      diagram.extensions = loadContext.diagram.extensions;
    }
    await saveState(diagram, {
      nodes: [...nodeManager.nodes],
      edges: [...edges],
    });
    const diagramJsonMin = JSON.stringify(diagram);
    // Compress the JSON string to Uint8Array
    const compressedData = deflateSync(strToU8(diagramJsonMin));
    // Convert Uint8Array to a binary string for btoa
    let binaryString = '';
    for (let i = 0; i < compressedData.length; i++) {
      binaryString += String.fromCharCode(compressedData[i]);
    }
    const base64Diagram = btoa(binaryString);

    const shareLink = `${window.location.origin}${window.location.pathname}?diagram=${encodeURIComponent(base64Diagram)}`;

    const diagramJsonPretty = JSON.stringify(diagram, null, 2);

    const dialogData = {
      shareLink,
      diagramJson: diagramJsonPretty,
    } satisfies DialogData;

    return dialogData;
  }, [registry, nodeManager, edges, templates, loadContext]);

  const dialogData = use(dialogDataPromise);

  const [downloaded, setDownloaded] = React.useState(false);

  const handleDownload = async () => {
    if (!dialogData) {
      return;
    }

    const blob = new Blob([dialogData.diagramJson], {
      type: 'application/json',
    });

    if ('showSaveFilePicker' in window) {
      try {
        const handle = await (window as any).showSaveFilePicker({
          suggestedName: 'diagram.json',
          types: [
            {
              description: 'JSON File',
              accept: { 'application/json': ['.json'] },
            },
          ],
        });
        const writable = await handle.createWritable();
        await writable.write(blob);
        await writable.close();
        setDownloaded(true);
        setTimeout(() => { setDownloaded(false); }, 5000);
        return;
      } catch (err) {
        if ((err as Error).name === 'AbortError') {
          return;
        }
      }
    }

    // The showSaveFilePicker API might not be supported in some browsers,
    // fallback to the default method of downloading if it fails.
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'diagram.json';
    document.body.appendChild(a);
    a.click();
    document.body.removeChild(a);
    URL.revokeObjectURL(url);

    setDownloaded(true);
    setTimeout(() => { setDownloaded(false); }, 5000);
  };

  const [copiedShareLink, setCopiedShareLink] = React.useState(false);

  return (
    <Dialog
      onClose={() => {
        onClose();
        setCopiedShareLink(false);
      }}
      open={open}
      fullWidth
      maxWidth="md"
      keepMounted={false}
    >
      <DialogTitle>Export Diagram</DialogTitle>
      <DialogContent>
        <Stack spacing={2}>
          <Typography variant="h6">Share</Typography>
          <ButtonGroup>
            <TextField
              variant="outlined"
              value={dialogData?.shareLink}
              fullWidth
              size="small"
            />
            <Button
              variant="contained"
              aria-label="copy share link"
              onClick={() => {
                if (!dialogData || copiedShareLink) {
                  return;
                }
                navigator.clipboard.writeText(dialogData.shareLink);
                setCopiedShareLink(true);
              }}
            >
              {copiedShareLink ? (
                <MaterialSymbol symbol="check" />
              ) : (
                <MaterialSymbol symbol="content_copy" />
              )}
            </Button>
          </ButtonGroup>
          <Stack direction="row" justifyContent="space-between">
            <Typography variant="h6">Export JSON</Typography>
            <Button
              variant="contained"
              onClick={handleDownload}
              startIcon={downloaded ? (
                <MaterialSymbol symbol="check_circle" />
              ) : (
                <MaterialSymbol symbol="download" />
              )}
              sx={{
                backgroundColor: downloaded ? theme.palette.success.main : null,
              }}
            >
              {downloaded ? 'Downloaded' : 'Download'}
            </Button>
          </Stack>
          <TextField
            multiline
            maxRows={20}
            variant="outlined"
            value={dialogData?.diagramJson}
            slotProps={{
              htmlInput: { sx: { fontFamily: 'monospace' } },
            }}
          />
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>Close</Button>
      </DialogActions>
    </Dialog>
  );
}

export const ExportDiagramDialog = (props: ExportDiagramDialogProps) => (
  <Suspense>
    <ExportDiagramDialogInternal {...props} />
  </Suspense>
);
