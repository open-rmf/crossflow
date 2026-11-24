import {
  Box,
  Chip,
  IconButton,
  Menu,
  MenuItem,
  Stack,
  Tooltip,
} from '@mui/material';
import { Templates } from '@rjsf/mui';
import type { FieldTemplateProps } from '@rjsf/utils';
import {
  type ComponentType,
  createContext,
  type PropsWithChildren,
  useContext,
  useState,
} from 'react';
import { MaterialSymbol } from '../nodes';
import {
  templatePropertyNames,
  useActiveTemplate,
} from '../templates-provider';
import { useTransientEditorDrafts } from '../transient-editor-drafts';
import type { BaseEditOperationFormProps } from './base-edit-operation-form';
import SchemaConfigForm, {
  type SchemaConfigFormProps,
} from './schema-config-form';

type Bindings = Record<string, string>;
const BindingContext = createContext<{
  properties: string[];
  bindings: Bindings;
  update: (bindings: Bindings) => void;
} | null>(null);
const ParentPointer = createContext<string | undefined>(undefined);
const MuiField = Templates.FieldTemplate as ComponentType<FieldTemplateProps>;

function BindableField({
  pointer,
  label,
  children,
}: PropsWithChildren<{
  pointer: string;
  label: string;
}>) {
  const context = useContext(BindingContext);
  const parent = useContext(ParentPointer);
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  if (!context || parent === pointer) return children;

  const linked = context.bindings[pointer];
  const select = (property?: string) => {
    const next = { ...context.bindings };
    if (property === undefined) {
      delete next[pointer];
    } else {
      for (const child of Object.keys(next)) {
        if (child.startsWith(`${pointer}/`)) delete next[child];
      }
      next[pointer] = property;
    }
    context.update(next);
    setAnchor(null);
  };
  return (
    <ParentPointer.Provider value={pointer}>
      {linked !== undefined ? (
        <Stack spacing={0.5} alignItems="flex-start">
          <Box
            component="span"
            sx={{ typography: 'caption', color: 'text.secondary' }}
          >
            {label}
          </Box>
          <Chip
            label={linked}
            color="primary"
            icon={<MaterialSymbol symbol="link" />}
            onDelete={() => select()}
          />
        </Stack>
      ) : (
        <Stack direction="row" alignItems="flex-start" spacing={1}>
          <Box sx={{ flex: 1, minWidth: 0 }}>{children}</Box>
          <Tooltip title={`Link ${label} to a section property`}>
            <IconButton
              aria-label={`Link ${label} to a section property`}
              sx={{ mt: 1 }}
              onClick={(event) => setAnchor(event.currentTarget)}
            >
              <MaterialSymbol symbol="link" />
            </IconButton>
          </Tooltip>
        </Stack>
      )}
      <Menu
        anchorEl={anchor}
        open={Boolean(anchor)}
        onClose={() => setAnchor(null)}
      >
        {!context.properties.length && (
          <MenuItem disabled>No section properties</MenuItem>
        )}
        {context.properties.map((name) => (
          <MenuItem key={name} onClick={() => select(name)}>
            {name}
          </MenuItem>
        ))}
      </Menu>
    </ParentPointer.Provider>
  );
}

function OperationField(props: FieldTemplateProps) {
  const path = props.fieldPathId.path;
  const field = <MuiField {...props} />;
  // Reordering a list would make an index binding point to a different item.
  if (
    props.hidden ||
    path.some((segment) => typeof segment === 'number') ||
    (!path.length && props.schema.type === 'object')
  )
    return field;
  const pointer = path
    .map((key) => `/${String(key).replace(/~/g, '~0').replace(/\//g, '~1')}`)
    .join('');
  return (
    <BindableField pointer={pointer} label={props.label}>
      {field}
    </BindableField>
  );
}

type OperationProps = BaseEditOperationFormProps<
  'node' | 'script' | 'section'
> &
  Pick<SchemaConfigFormProps, 'schema' | 'definitions' | 'label'>;

export default function OperationConfigForm({
  node,
  onChange,
  schema,
  definitions,
  label = 'Config',
}: OperationProps) {
  const template = useActiveTemplate();
  const { drafts, setOperationConfigDraft } = useTransientEditorDrafts();
  const op = node.data.op;
  const key = `${op.type}:${node.id}:config`;
  const replace = (next: typeof op) =>
    onChange?.({
      type: 'replace',
      id: node.id,
      item: { ...node, data: { ...node.data, op: next } },
    });
  const binding = template && {
    bindings: op.config_bindings ?? {},
    properties: templatePropertyNames(template.config_schema),
    update: (next: Bindings) =>
      replace({
        ...op,
        config_bindings: Object.keys(next).length ? next : undefined,
      }),
  };
  const form = (
    <SchemaConfigForm
      schema={schema}
      definitions={definitions}
      label={label}
      allowJson
      nodeDefaults={op.type === 'node'}
      value={op.config}
      onChange={(config) => replace({ ...op, config })}
      fieldTemplate={binding ? OperationField : undefined}
      rawDraft={drafts.operationConfigs[key]}
      onRawDraftChange={(text) => setOperationConfigDraft(key, text)}
    />
  );
  return (
    <BindingContext.Provider value={binding ?? null}>
      {binding && (!schema || typeof schema === 'boolean') ? (
        <BindableField pointer="" label={label}>
          {form}
        </BindableField>
      ) : (
        form
      )}
    </BindingContext.Provider>
  );
}
