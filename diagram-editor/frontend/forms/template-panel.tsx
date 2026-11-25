import { Box, Divider, Stack, Typography } from '@mui/material';
import { Fragment, type ReactNode } from 'react';
import { useCompatibilityChecker } from '../connection-compatibility-provider';
import { useNodeManager } from '../node-manager';
import { useTemplates } from '../templates-provider';
import type { Schema } from '../types/api';
import { useEdges } from '../use-edges';
import { useReactFlow } from '../use-react-flow';
import { schemaDocument } from '../utils/schema-types';
import SchemaEditor, { schemaSummary } from './schema-editor';

export default function TemplatePanel({ templateId }: { templateId: string }) {
  const [templates, setTemplates] = useTemplates();
  const template = templates[templateId];
  const { nodes } = useNodeManager();
  const { updateNodeData } = useReactFlow();
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
        <dd>{type}</dd>
      </Fragment>,
    );
  }

  function updateSchema(config_schema: Schema | undefined) {
    const names = Object.keys(schemaDocument(config_schema).properties ?? {});
    const removed = Object.keys(
      schemaDocument(template.config_schema).properties ?? {},
    ).filter((name) => !names.includes(name));
    for (const node of nodes) {
      if (
        node.type !== 'node' &&
        node.type !== 'section' &&
        node.type !== 'script'
      )
        continue;
      const op = node.data.op;
      if (!op.config_bindings) continue;
      const bindings = Object.entries(op.config_bindings);
      const kept = bindings.filter(([, name]) => !removed.includes(name));
      if (kept.length !== bindings.length) {
        updateNodeData(node.id, {
          op: {
            ...op,
            config_bindings: kept.length ? Object.fromEntries(kept) : undefined,
          },
        });
      }
    }
    setTemplates((prev) => ({
      ...prev,
      [templateId]: { ...prev[templateId], config_schema },
    }));
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
                    gridTemplateColumns: 'minmax(0, 1fr) auto',
                    columnGap: 1,
                    m: 0,
                    typography: 'body2',
                    overflowWrap: 'anywhere',
                    '& dd': {
                      m: 0,
                      color: 'text.secondary',
                      textAlign: 'right',
                    },
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
        onChange={updateSchema}
      />
    </Stack>
  );
}
