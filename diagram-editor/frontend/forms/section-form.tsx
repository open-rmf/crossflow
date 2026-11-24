import {
  Autocomplete,
  Card,
  CardContent,
  CardHeader,
  IconButton,
  Stack,
  TextField,
} from '@mui/material';
import type { EdgeChange, NodeChange, NodeRemoveChange } from '@xyflow/react';
import { type PropsWithChildren, useMemo } from 'react';
import type { SectionEdge } from '../edges';
import { useNodeManager } from '../node-manager';
import {
  isSectionNode,
  MaterialSymbol,
  type SectionBufferNode,
  type SectionInputNode,
  type SectionInterfaceNode,
  type SectionOutputNode,
} from '../nodes';
import { useRegistry } from '../registry-provider';
import { useTemplates } from '../templates-provider';
import { useTransientEditorDrafts } from '../transient-editor-drafts';
import BaseEditOperationForm, {
  type BaseEditOperationFormProps,
} from './base-edit-operation-form';

import OperationConfigForm from './operation-config-form';
import SchemaEditor from './schema-editor';

export interface BaseSectionInterfaceFormProps {
  node: SectionInterfaceNode;
  onDelete?: (change: NodeRemoveChange) => void;
}

function BaseSectionInterfaceForm({
  node,
  onDelete,
  children,
}: PropsWithChildren<BaseSectionInterfaceFormProps>) {
  return (
    <Card>
      <CardHeader
        title="Edit"
        action={
          <IconButton
            color="error"
            onClick={() => onDelete?.({ type: 'remove', id: node.id })}
          >
            <MaterialSymbol symbol="delete" />
          </IconButton>
        }
      />
      <CardContent>{children}</CardContent>
    </Card>
  );
}

export interface SectionInputFormProps extends BaseSectionInterfaceFormProps {
  node: SectionInputNode;
  onChange?: (change: NodeChange<SectionInputNode>) => void;
}

export function SectionInputForm({
  node,
  onChange,
  onDelete,
}: SectionInputFormProps) {
  return (
    <BaseSectionInterfaceForm node={node} onDelete={onDelete}>
      <Stack spacing={2}>
        <TextField
          required
          label="Remapped Id"
          value={node.data.remappedId}
          onChange={(ev) => {
            const updatedNode = { ...node };
            updatedNode.data.remappedId = ev.target.value;
            onChange?.({
              type: 'replace',
              id: node.id,
              item: updatedNode,
            });
          }}
        />
        <SchemaEditor
          label="Input schema"
          value={node.data.schema}
          onChange={(schema) =>
            onChange?.({
              type: 'replace',
              id: node.id,
              item: { ...node, data: { ...node.data, schema } },
            })
          }
        />
      </Stack>
    </BaseSectionInterfaceForm>
  );
}

export interface SectionBufferFormProps extends BaseSectionInterfaceFormProps {
  node: SectionBufferNode;
  onChange?: (change: NodeChange<SectionBufferNode>) => void;
}

export function SectionBufferForm({
  node,
  onChange,
  onDelete,
}: SectionBufferFormProps) {
  return (
    <BaseSectionInterfaceForm node={node} onDelete={onDelete}>
      <Stack spacing={2}>
        <TextField
          required
          label="Remapped Id"
          value={node.data.remappedId}
          onChange={(ev) => {
            const updatedNode = { ...node };
            updatedNode.data.remappedId = ev.target.value;
            onChange?.({
              type: 'replace',
              id: node.id,
              item: updatedNode,
            });
          }}
        />
      </Stack>
    </BaseSectionInterfaceForm>
  );
}

export interface SectionOutputFormProps extends BaseSectionInterfaceFormProps {
  node: SectionOutputNode;
  onChange?: (change: NodeChange<SectionOutputNode>) => void;
}

export function SectionOutputForm({
  node,
  onChange,
  onDelete,
}: SectionOutputFormProps) {
  return (
    <BaseSectionInterfaceForm node={node} onDelete={onDelete}>
      <Stack spacing={2}>
        <TextField
          required
          label="Output Id"
          value={node.data.outputId}
          onChange={(ev) => {
            const updatedNode = { ...node };
            updatedNode.data.outputId = ev.target.value;
            onChange?.({
              type: 'replace',
              id: node.id,
              item: updatedNode,
            });
          }}
        />
        <SchemaEditor
          label="Output schema"
          value={node.data.schema}
          onChange={(schema) =>
            onChange?.({
              type: 'replace',
              id: node.id,
              item: { ...node, data: { ...node.data, schema } },
            })
          }
        />
      </Stack>
    </BaseSectionInterfaceForm>
  );
}

export type SectionFormProps = BaseEditOperationFormProps<'section'>;

export function SectionForm(props: SectionFormProps) {
  const { clearOperationConfigDrafts } = useTransientEditorDrafts();
  const registry = useRegistry();
  const [templates, _setTemplates] = useTemplates();
  const sectionBuilders = Object.keys(registry.sections);
  const sectionTemplates = Object.keys(templates);
  const sections = [...sectionBuilders, ...sectionTemplates].sort();

  const op = props.node.data.op;
  const template =
    typeof op.template === 'string' ? templates[op.template] : undefined;
  const schema =
    typeof op.builder === 'string'
      ? registry.sections[op.builder]?.config_schema
      : template?.config_schema;

  function replaceOp(next: typeof op) {
    props.onChange?.({
      type: 'replace',
      id: props.node.id,
      item: { ...props.node, data: { ...props.node.data, op: next } },
    });
  }

  return (
    <BaseEditOperationForm {...props}>
      <TextField
        label="Display Text"
        value={op.display_text || ''}
        onChange={(ev) =>
          replaceOp({ ...op, display_text: ev.target.value || undefined })
        }
      />
      <Autocomplete
        freeSolo
        autoSelect
        options={sections}
        getOptionLabel={(option) => option}
        value={(op.builder ?? op.template ?? '') as string}
        onChange={(_, value) => {
          if (value === null || value === (op.builder ?? op.template)) return;
          clearOperationConfigDrafts([props.node.id]);
          const {
            builder: _builder,
            template: _template,
            config_bindings: _bindings,
            ...rest
          } = op;
          replaceOp(
            sectionBuilders.includes(value)
              ? { ...rest, builder: value }
              : { ...rest, template: value },
          );
        }}
        renderInput={(params) => (
          <TextField {...params} required label="builder/template" />
        )}
      />
      {(!template || schema != null) && (
        <OperationConfigForm
          {...props}
          schema={schema}
          definitions={registry.schemas}
        />
      )}
    </BaseEditOperationForm>
  );
}

export interface SectionEdgeFormProps {
  edge: SectionEdge;
  onChange?: (changes: EdgeChange<SectionEdge>) => void;
}

export function SectionEdgeForm({ edge, onChange }: SectionEdgeFormProps) {
  const nodeManager = useNodeManager();
  const registry = useRegistry();
  const [templates, _setTemplates] = useTemplates();
  const sourceNode = nodeManager.tryGetNode(edge.source);

  const outputs = useMemo(() => {
    if (sourceNode && isSectionNode(sourceNode)) {
      if (typeof sourceNode.data.op.builder === 'string') {
        const sectionBuilder = registry.sections[sourceNode.data.op.builder];
        return Object.keys(sectionBuilder?.interface.outputs || {});
      } else if (typeof sourceNode.data.op.template === 'string') {
        const template = templates[sourceNode.data.op.template];
        return template?.outputs || [];
      } else {
        return [];
      }
    } else {
      console.error('source node of a section edge is not a section node');
      return [];
    }
  }, [sourceNode, registry, templates]);

  return (
    <Autocomplete
      freeSolo
      autoSelect
      options={outputs}
      getOptionLabel={(option) => option}
      value={edge.data.output.output}
      onChange={(_, value) => {
        onChange?.({
          type: 'replace',
          id: edge.id,
          item: {
            ...edge,
            data: {
              ...edge.data,
              output: { output: value || '' },
            },
          },
        });
      }}
      renderInput={(params) => (
        <TextField {...params} required label="Section Output" />
      )}
    />
  );
}
