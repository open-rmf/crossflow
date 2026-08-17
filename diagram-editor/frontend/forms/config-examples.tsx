import {
  Accordion,
  AccordionDetails,
  AccordionSummary,
  Box,
  Divider,
  IconButton,
  Stack,
  Tooltip,
  Typography,
} from '@mui/material';
import { MaterialSymbol } from '../nodes';
import type { ConfigExample } from '../types/api';

export interface ConfigExamplesProps {
  examples: ConfigExample[];
  onApply: (config: unknown) => void;
}

/**
 * The configurations a builder ships as examples. Collapsed by default: they
 * are reference material, and they matter most for the builders whose schema
 * the form can only edit as JSON, where the shape is the thing being learnt.
 */
export default function ConfigExamples({
  examples,
  onApply,
}: ConfigExamplesProps) {
  if (examples.length === 0) {
    return null;
  }

  return (
    <Accordion
      disableGutters
      elevation={0}
      square
      slotProps={{ transition: { unmountOnExit: true } }}
    >
      <AccordionSummary expandIcon={<MaterialSymbol symbol="expand_more" />}>
        <Typography variant="body2">Examples ({examples.length})</Typography>
      </AccordionSummary>
      <AccordionDetails>
        <Stack spacing={2} divider={<Divider flexItem />}>
          {examples.map((example) => (
            <Stack key={example.description} spacing={1}>
              <Typography color="text.secondary" variant="body2">
                {example.description}
              </Typography>
              <Stack
                direction="row"
                alignItems="center"
                spacing={1}
                sx={{
                  border: 1,
                  borderColor: 'divider',
                  borderRadius: 1,
                  bgcolor: 'action.hover',
                  pl: 1.5,
                  pr: 0.5,
                  py: 0.5,
                }}
              >
                <Box
                  component="pre"
                  sx={{
                    flex: 1,
                    m: 0,
                    fontFamily: 'monospace',
                    fontSize: '0.8125rem',
                    whiteSpace: 'pre-wrap',
                    wordBreak: 'break-word',
                  }}
                >
                  {JSON.stringify(example.config)}
                </Box>
                <Tooltip title="Use this config">
                  <IconButton
                    size="small"
                    aria-label="Use this config"
                    onClick={() => onApply(example.config)}
                  >
                    <MaterialSymbol symbol="arrow_forward" />
                  </IconButton>
                </Tooltip>
              </Stack>
            </Stack>
          ))}
        </Stack>
      </AccordionDetails>
    </Accordion>
  );
}
