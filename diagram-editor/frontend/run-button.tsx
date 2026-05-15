import {
  Box,
  Button,
  DialogActions,
  DialogContent,
  DialogTitle,
  Divider,
  Popover,
  Stack,
  TextField,
  Tooltip,
  Typography,
  useTheme,
} from '@mui/material';
import { useEffect, useMemo, useRef, useState } from 'react';
import type { Subscription } from 'rxjs';
import { useApiClient } from './api-client-provider';
import { useDebugVisualization } from './debug-visualization-provider';
import { useNodeManager } from './node-manager';
import { MaterialSymbol } from './nodes';
import { useRegistry } from './registry-provider';
import { useTemplates } from './templates-provider';
import { useEdges } from './use-edges';
import { exportDiagram } from './utils/export-diagram';
import { useDiagramProperties } from './diagram-properties-provider';
import type { Diagram, DiagramOperation } from './types/api';

type ResponseContent = { raw: string } | { err: string };
type RunningMode = 'run' | 'debug' | null;
interface DebugTimelineEntry {
  seq: number;
  operationId: string;
}

const DefaultResponseContent: ResponseContent = { raw: '' };
const MaxDebugTimelineEntries = 200;

function enableDebugTraceForOps(ops: Record<string, DiagramOperation>) {
  for (const op of Object.values(ops)) {
    op.trace = 'on';
    if (op.type === 'scope') {
      enableDebugTraceForOps(op.ops);
    }
  }
}

function enableDebugTrace(diagram: Diagram): Diagram {
  const debugDiagram = JSON.parse(JSON.stringify(diagram)) as Diagram;
  debugDiagram.default_trace = 'on';
  enableDebugTraceForOps(debugDiagram.ops);
  return debugDiagram;
}

export interface RunButtonProps {
  requestJsonString: string;
}

export function RunButton({ requestJsonString }: RunButtonProps) {
  const nodeManager = useNodeManager();
  const edges = useEdges();
  const [openPopover, setOpenPopover] = useState(false);
  const buttonRef = useRef<HTMLButtonElement>(null);
  const theme = useTheme();
  const [requestJson, setRequestJson] = useState(requestJsonString);
  const [responseContent, setResponseContent] = useState<ResponseContent>(
    DefaultResponseContent
  );
  const apiClient = useApiClient();
  const {
    clearDebugVisualization,
    markDebugFinished,
    markDebugOperationStarted,
  } = useDebugVisualization();
  const [templates, _setTemplates] = useTemplates();
  const registry = useRegistry();
  const [runningMode, setRunningMode] = useState<RunningMode>(null);
  const [debugTimeline, setDebugTimeline] = useState<DebugTimelineEntry[]>([]);
  const debugEventCounter = useRef(0);
  const debugSessionRef = useRef<Awaited<
    ReturnType<NonNullable<typeof apiClient.wsDebugWorkflow>>
  > | null>(null);
  const debugSubscriptionRef = useRef<Subscription | null>(null);
  const [diagramProperties, _] = useDiagramProperties();

  const closeDebugSession = () => {
    debugSubscriptionRef.current?.unsubscribe();
    debugSubscriptionRef.current = null;
    debugSessionRef.current?.close();
    debugSessionRef.current = null;
  };

  useEffect(() => {
    setRequestJson(requestJsonString);
  }, [requestJsonString]);

  useEffect(() => {
    return closeDebugSession;
  }, []);

  const requestError = useMemo(() => {
    try {
      JSON.parse(requestJson);
      return false;
    } catch {
      return true;
    }
  }, [requestJson]);

  const responseError = useMemo(() => {
    return 'err' in responseContent;
  }, [responseContent]);

  const responseValue = useMemo(() => {
    if ('err' in responseContent) {
      return `Error: ${responseContent.err}`;
    } else {
      return responseContent.raw;
    }
  }, [responseContent]);

  const handleRunClick = () => {
    closeDebugSession();
    clearDebugVisualization();
    setDebugTimeline([]);
    try {
      const request = JSON.parse(requestJson);
      const diagram = exportDiagram(registry, nodeManager, edges, templates, diagramProperties);
      apiClient.postRunWorkflow(diagram, request).subscribe({
        next: (response) => {
          setResponseContent({ raw: JSON.stringify(response, null, 2) });
          setRunningMode(null);
        },
        error: (err) => {
          setResponseContent({ err: (err as Error).message });
          setRunningMode(null);
        },
      });
      setRunningMode('run');
    } catch (e) {
      setResponseContent({ err: (e as Error).message });
    }
  };

  const handleDebugClick = async () => {
    closeDebugSession();
    clearDebugVisualization();
    setDebugTimeline([]);
    debugEventCounter.current = 0;

    if (!apiClient.wsDebugWorkflow) {
      setResponseContent({
        err: 'Debug sessions are not supported by this backend.',
      });
      return;
    }

    try {
      const request = JSON.parse(requestJson);
      const diagram = exportDiagram(
        registry,
        nodeManager,
        edges,
        templates,
        diagramProperties,
      );
      setResponseContent(DefaultResponseContent);
      setRunningMode('debug');

      const debugSession = await apiClient.wsDebugWorkflow(
        enableDebugTrace(diagram),
        request,
      );
      debugSessionRef.current = debugSession;
      debugSubscriptionRef.current = debugSession.debugFeedback$.subscribe({
        next: (msg) => {
          if (msg.type === 'feedback' && 'operationStarted' in msg) {
            const operationId = msg.operationStarted;
            markDebugOperationStarted(operationId);
            const entry = {
              seq: ++debugEventCounter.current,
              operationId,
            };
            setDebugTimeline((prev) =>
              [...prev, entry].slice(-MaxDebugTimelineEntries),
            );
            return;
          }

          if (msg.type === 'finish') {
            markDebugFinished();
            if ('ok' in msg) {
              setResponseContent({ raw: JSON.stringify(msg.ok, null, 2) });
            } else {
              setResponseContent({ err: msg.err });
            }
            setRunningMode(null);
            closeDebugSession();
          }
        },
        error: (err) => {
          markDebugFinished();
          setResponseContent({ err: (err as Error).message });
          setRunningMode(null);
          closeDebugSession();
        },
        complete: () => {
          setRunningMode(null);
        },
      });
    } catch (e) {
      setResponseContent({ err: (e as Error).message });
      setRunningMode(null);
      closeDebugSession();
    }
  };

  const handleClosePopover = () => {
    setOpenPopover(false);
  };

  return (
    <>
      <Tooltip title="Run Workflow">
        <Button ref={buttonRef} onClick={() => setOpenPopover(true)}>
          <MaterialSymbol symbol="play_arrow" />
        </Button>
      </Tooltip>
      <Popover
        open={openPopover}
        onClose={handleClosePopover}
        anchorEl={buttonRef.current}
        anchorOrigin={{
          vertical: 'bottom',
          horizontal: 'center',
        }}
        transformOrigin={{
          vertical: 'top',
          horizontal: 'center',
        }}
        slotProps={{
          paper: {
            sx: {
              overflow: 'hidden',
              mt: 0.5,
              width: 'min(560px, calc(100vw - 32px))',
              maxWidth: 'calc(100vw - 32px)',
              maxHeight: 'calc(100vh - 32px)',
              display: 'flex',
              flexDirection: 'column',
              backgroundColor: theme.palette.background.paper,
              border: `1px solid ${theme.palette.divider}`,
              '&:before': {
                content: '""',
                position: 'absolute',
                top: 0,
                left: '50%',
                transform: 'translateY(-50%) translateX(-50%) rotate(45deg)',
                width: 16,
                height: 16,
                backgroundColor: theme.palette.background.paper,
                backgroundImage: 'inherit',
                borderTop: `1px solid ${theme.palette.divider}`,
                borderLeft: `1px solid ${theme.palette.divider}`,
              },
            },
          },
        }}
      >
        <DialogTitle>Run Workflow</DialogTitle>
        <Divider />
        <DialogContent
          sx={{
            width: '100%',
            overflowY: 'auto',
            flex: 1,
          }}
        >
          <Stack spacing={2}>
            <Typography variant="body1">Request:</Typography>
            <TextField
              fullWidth
              multiline
              minRows={6}
              maxRows={10}
              variant="outlined"
              value={requestJson}
              slotProps={{
                htmlInput: {
                  sx: { fontFamily: 'monospace', whiteSpace: 'nowrap' },
                },
              }}
              onChange={(e) => setRequestJson(e.target.value)}
              error={requestError}
              sx={{ backgroundColor: theme.palette.background.paper }}
            />
            <Stack
              direction='row'
              spacing={2}
              sx={{ alignItems: 'center'}}
            >
              <Typography variant="body1">Response:</Typography>
              {'err' in responseContent ? (
                <MaterialSymbol
                  symbol='error'
                  sx={{ color: theme.palette.error.main }}
                />
              ) : 'raw' in responseContent && responseContent.raw.length > 0 ? (
                <MaterialSymbol
                  symbol='check_circle'
                  sx={{ color: theme.palette.success.main }}
                />
              ) : (
                <></>
              )}
            </Stack>
            <TextField
              fullWidth
              multiline
              minRows={6}
              maxRows={10}
              variant="outlined"
              value={responseValue}
              slotProps={{
                htmlInput: {
                  sx: { fontFamily: 'monospace', whiteSpace: 'nowrap' },
                },
              }}
              error={responseError}
            />
            {debugTimeline.length > 0 && (
              <Stack spacing={1}>
                <Typography variant="body1">Debug timeline:</Typography>
                <Box
                  sx={{
                    border: `1px solid ${theme.palette.divider}`,
                    borderRadius: 1,
                    maxHeight: 160,
                    overflowY: 'auto',
                    px: 1,
                    py: 0.5,
                  }}
                >
                  {debugTimeline.map((entry) => (
                    <Stack
                      key={entry.seq}
                      direction="row"
                      spacing={1}
                      sx={{ alignItems: 'baseline' }}
                    >
                      <Typography variant="caption" color="text.secondary">
                        {entry.seq}
                      </Typography>
                      <Typography
                        variant="body2"
                        sx={{
                          fontFamily: 'monospace',
                          overflow: 'hidden',
                          textOverflow: 'ellipsis',
                          whiteSpace: 'nowrap',
                        }}
                      >
                        {entry.operationId}
                      </Typography>
                    </Stack>
                  ))}
                </Box>
              </Stack>
            )}
          </Stack>
        </DialogContent>
        <DialogActions sx={{ flexShrink: 0 }}>
          <Button
            variant="outlined"
            onClick={handleDebugClick}
            disabled={runningMode !== null}
            loading={runningMode === 'debug'}
            startIcon={<MaterialSymbol symbol="bug_report" />}
          >
            Debug
          </Button>
          <Button
            variant="contained"
            onClick={handleRunClick}
            disabled={runningMode !== null}
            loading={runningMode === 'run'}
            startIcon={<MaterialSymbol symbol="play_arrow" />}
          >
            Run
          </Button>
        </DialogActions>
      </Popover>
    </>
  );
}
