import { Box, Button, Divider, Stack, Typography } from '@mui/material';
import type {
  ScriptEnvironmentMetadata,
  ScriptEnvironmentSchema,
} from '../types/api';

export interface ScriptEnvironmentPanelProps {
  environmentName: string;
  environment?: ScriptEnvironmentSchema;
  metadata?: ScriptEnvironmentMetadata;
  onEdit: () => void;
}

function ConfigPreview({ value }: { value: unknown }) {
  return (
    <Box
      component="pre"
      sx={{
        m: 0,
        p: 1.5,
        maxHeight: 220,
        overflow: 'auto',
        bgcolor: 'action.hover',
        border: '1px solid',
        borderColor: 'divider',
        borderRadius: 1,
        fontFamily: 'monospace',
        fontSize: '0.75rem',
        lineHeight: 1.5,
        whiteSpace: 'pre-wrap',
        overflowWrap: 'anywhere',
      }}
    >
      {typeof value === 'string' ? value : JSON.stringify(value, null, 2)}
    </Box>
  );
}

export function ScriptEnvironmentPanel({
  environmentName,
  environment,
  metadata,
  onEdit,
}: ScriptEnvironmentPanelProps) {
  if (!environmentName || !environment) {
    return (
      <Box
        component="aside"
        sx={{
          pl: { xs: 0, md: 3 },
          pt: { xs: 3, md: 0 },
          borderLeft: { xs: 0, md: '1px solid' },
          borderTop: { xs: '1px solid', md: 0 },
          borderColor: 'divider',
        }}
      >
        <Typography variant="h6">Environment</Typography>
        <Typography color="text.secondary" sx={{ mt: 1 }}>
          Select an environment to inspect its runtime and configuration.
        </Typography>
      </Box>
    );
  }

  const config =
    typeof environment.config === 'object' && environment.config !== null
      ? (environment.config as Record<string, unknown>)
      : {};
  const script = typeof config.script === 'string' ? config.script : '';
  const configWithoutScript = Object.fromEntries(
    Object.entries(config).filter(([key]) => key !== 'script'),
  );

  return (
    <Box
      component="aside"
      aria-label="Script environment details"
      sx={{
        pl: { xs: 0, md: 3 },
        pt: { xs: 3, md: 0 },
        borderLeft: { xs: 0, md: '1px solid' },
        borderTop: { xs: '1px solid', md: 0 },
        borderColor: 'divider',
        minWidth: 0,
      }}
    >
      <Stack spacing={2}>
        <Stack
          direction="row"
          alignItems="center"
          justifyContent="space-between"
        >
          <Box sx={{ minWidth: 0 }}>
            <Typography variant="overline" color="text.secondary">
              Environment
            </Typography>
            <Typography variant="h6" noWrap title={environmentName}>
              {environmentName}
            </Typography>
          </Box>
          <Button size="small" onClick={onEdit}>
            Edit
          </Button>
        </Stack>

        <Divider />

        <Stack spacing={1}>
          <Typography variant="subtitle2">Runtime</Typography>
          <Box>
            <Typography variant="caption" color="text.secondary">
              Builder
            </Typography>
            <Typography variant="body2">{environment.builder}</Typography>
          </Box>
          <Box>
            <Typography variant="caption" color="text.secondary">
              Language
            </Typography>
            <Typography variant="body2">
              {metadata?.language ?? 'Unknown'}
            </Typography>
          </Box>
          <Box>
            <Typography variant="caption" color="text.secondary">
              Interpreter version
            </Typography>
            <Typography
              variant="body2"
              sx={{ fontFamily: 'monospace', overflowWrap: 'anywhere' }}
            >
              {metadata?.interpreter ?? 'Unavailable'}
            </Typography>
          </Box>
        </Stack>

        <Stack spacing={1}>
          <Typography variant="subtitle2">Configuration</Typography>
          <ConfigPreview value={configWithoutScript} />
        </Stack>

        {script && (
          <Stack spacing={1}>
            <Typography variant="subtitle2">Environment script</Typography>
            <ConfigPreview value={script} />
          </Stack>
        )}
      </Stack>
    </Box>
  );
}
