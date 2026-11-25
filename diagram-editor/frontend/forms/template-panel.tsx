import { Box, Divider, Stack, Typography } from '@mui/material';
import { Fragment, type ReactNode } from 'react';
import { useCompatibilityChecker } from '../connection-compatibility-provider';
import { useNodeManager } from '../node-manager';
import { useTemplates } from '../templates-provider';
import type { Schema } from '../types/api';
import { useEdges } from '../use-edges';
import SchemaEditor, { schemaSummary } from './schema-editor';
import TypeLabel from './type-label';

export default function TemplatePanel({
  templateId,
  onChangeSchema,
}: {
  templateId: string;
  onChangeSchema: (
    schema: Schema | undefined,
    rename?: { from: string; to: string },
  ) => string | undefined;
}) {
  const [templates] = useTemplates();
  const template = templates[templateId];
  const { nodes } = useNodeManager();
  const edges = useEdges();
  const { edgeResults } = useCompatibilityChecker();
  const ports: Record<'Inputs' | 'Outputs', ReactNode[]> = {
    Inputs: [],
    Outputs: [],
  };
  for (const node of nodes) {
    if (node.type !== 'sectionInput' && node.type !== 'sectionOutput') continue;
    const input = node.type === 'sectionInput';
    const edge = edges.find((e) => (input ? e.source : e.target) === node.id);
    const result = edge && edgeResults.get(edge.id);
    const type =
      node.data.schema === undefined
        ? ((input ? result?.targetType : result?.sourceType) ?? 'unknown')
        : schemaSummary(node.data.schema);
    ports[input ? 'Inputs' : 'Outputs'].push(
      <Fragment key={node.id}>
        <dt>{input ? node.data.remappedId : node.data.outputId}</dt>
        <TypeLabel component="dd" sx={{ width: '100%' }}>
          {type}
        </TypeLabel>
      </Fragment>,
    );
  }

  return (
    <Stack spacing={2} sx={{ p: 2, overflowY: 'auto' }}>
      {(ports.Inputs.length > 0 || ports.Outputs.length > 0) && (
        <>
          <Stack direction="row" spacing={2}>
            {Object.entries(ports).map(([direction, entries]) => (
              <Stack key={direction} sx={{ flex: 1, minWidth: 0 }}>
                <Typography variant="subtitle2">{direction}</Typography>
                <Box
                  component="dl"
                  sx={{
                    display: 'grid',
                    gridTemplateColumns: 'fit-content(50%) minmax(0, 1fr)',
                    columnGap: 1,
                    m: 0,
                    typography: 'body2',
                    overflowWrap: 'anywhere',
                  }}
                >
                  {entries}
                </Box>
              </Stack>
            ))}
          </Stack>
          <Divider />
        </>
      )}
      <SchemaEditor
        object
        label="Configuration schema"
        value={template.config_schema ?? undefined}
        onChange={onChangeSchema}
        onRenameProperty={(from, to, schema) =>
          onChangeSchema(schema, { from, to })
        }
      />
    </Stack>
  );
}
