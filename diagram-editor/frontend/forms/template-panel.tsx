import { Divider, Stack, Typography } from '@mui/material';
import { useCompatibilityChecker } from '../connection-compatibility-provider';
import { useNodeManager } from '../node-manager';
import { templatePropertyNames, useTemplates } from '../templates-provider';
import type { Schema } from '../types/api';
import { useEdges } from '../use-edges';
import { useReactFlow } from '../use-react-flow';
import SchemaEditor, { schemaSummary } from './schema-editor';

export default function TemplatePanel({ templateId }: { templateId: string }) {
  const [templates, setTemplates] = useTemplates();
  const { nodes } = useNodeManager();
  const { updateNodeData } = useReactFlow();
  const edges = useEdges();
  const { edgeResults } = useCompatibilityChecker();
  const ports = nodes.flatMap((node) => {
    if (node.type !== 'sectionInput' && node.type !== 'sectionOutput')
      return [];
    const input = node.type === 'sectionInput';
    const edge = edges.find((e) => (input ? e.source : e.target) === node.id);
    const result = edge && edgeResults.get(edge.id);
    const inferredType = input ? result?.targetType : result?.sourceType;
    return [
      {
        id: node.id,
        name: input ? node.data.remappedId : node.data.outputId,
        direction: input ? 'Inputs' : 'Outputs',
        type:
          node.data.schema === undefined
            ? (inferredType ?? 'unknown')
            : schemaSummary(node.data.schema),
      },
    ];
  });

  function updateSchema(config_schema: Schema | undefined) {
    const next = templatePropertyNames(config_schema);
    const removed = templatePropertyNames(
      templates[templateId].config_schema,
    ).filter((name) => !next.includes(name));
    if (removed.length) {
      for (const node of nodes) {
        if (
          node.type !== 'node' &&
          node.type !== 'section' &&
          node.type !== 'script'
        )
          continue;
        const op = node.data.op;
        const bindings = Object.entries(op.config_bindings ?? {});
        const retained = bindings.filter(([, name]) => !removed.includes(name));
        if (retained.length !== bindings.length) {
          updateNodeData(node.id, {
            op: {
              ...op,
              config_bindings: retained.length
                ? Object.fromEntries(retained)
                : undefined,
            },
          });
        }
      }
    }
    setTemplates((prev) => ({
      ...prev,
      [templateId]: { ...prev[templateId], config_schema },
    }));
  }

  return (
    <Stack spacing={2} sx={{ p: 2, overflowY: 'auto' }}>
      {ports.length > 0 && (
        <>
          <Stack direction="row" spacing={2}>
            {['Inputs', 'Outputs'].map((direction) => (
              <Stack key={direction} sx={{ flex: 1, minWidth: 0 }}>
                <Typography variant="subtitle2">{direction}</Typography>
                {ports
                  .filter((port) => port.direction === direction)
                  .map((port) => (
                    <Stack
                      key={port.id}
                      direction="row"
                      justifyContent="space-between"
                      spacing={1}
                    >
                      <Typography
                        variant="body2"
                        sx={{ overflowWrap: 'anywhere' }}
                      >
                        {port.name}
                      </Typography>
                      <Typography
                        variant="body2"
                        color="text.secondary"
                        sx={{ overflowWrap: 'anywhere', textAlign: 'right' }}
                      >
                        {port.type}
                      </Typography>
                    </Stack>
                  ))}
              </Stack>
            ))}
          </Stack>
          <Divider />
        </>
      )}
      <SchemaEditor
        object
        label="Configuration schema"
        value={templates[templateId].config_schema ?? undefined}
        onChange={updateSchema}
      />
    </Stack>
  );
}
