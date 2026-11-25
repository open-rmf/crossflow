import { Chip, MenuItem, Stack, Tooltip, Typography } from '@mui/material';
import { Templates } from '@rjsf/mui';
import type { FieldTemplateProps } from '@rjsf/utils';
import {
  type ComponentType,
  createContext,
  type PropsWithChildren,
  useContext,
} from 'react';
import { MaterialSymbol } from '../nodes';
import { useRegistry } from '../registry-provider';
import { useActiveTemplate } from '../templates-provider';
import { useTransientEditorDrafts } from '../transient-editor-drafts';
import { bindingProperties, schemaDocument } from '../utils/schema-types';
import {
  type BaseEditOperationFormProps,
  operationUpdater,
} from './base-edit-operation-form';
import { FieldActions, FieldMenu } from './field-actions';
import SchemaConfigForm, {
  type SchemaConfigFormProps,
} from './schema-config-form';

type Bindings = Record<string, string>;
const BindingContext = createContext<{
  schema: ReturnType<typeof schemaDocument>;
  pointer?: string;
  bindings: Bindings;
  update: (bindings: Bindings) => void;
} | null>(null);
const MuiField = Templates.FieldTemplate as ComponentType<FieldTemplateProps>;

function BindableField({
  field,
  pointer = '',
  label = field?.label ?? 'Config',
  children,
}: PropsWithChildren<{
  field?: FieldTemplateProps;
  pointer?: string;
  label?: string;
}>) {
  const context = useContext(BindingContext);
  if (!context || context.pointer === pointer) return children;

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
  };
  if (linked !== undefined) {
    return (
      <Stack spacing={0.5} alignItems="flex-start">
        <Typography variant="caption" color="text.secondary">
          {label}
        </Typography>
        <Chip
          label={linked}
          color="primary"
          icon={<MaterialSymbol symbol="link" />}
          onDelete={() => select()}
        />
      </Stack>
    );
  }

  const menu = (
    <FieldMenu
      symbol="link"
      label={`Link ${label} to a section property`}
      menuListSx={{
        display: 'grid',
        gridTemplateColumns: '1fr auto auto',
        columnGap: 1,
        '& > li': {
          display: 'grid',
          gridColumn: '1 / -1',
          gridTemplateColumns: 'subgrid',
        },
      }}
    >
      {() => {
        const properties = bindingProperties(context.schema, field);
        if (!properties.length)
          return <MenuItem disabled>No section properties</MenuItem>;
        return properties.map(({ name, type, warning }) => (
          <MenuItem
            key={name}
            aria-label={name}
            aria-description={warning ? `${type}, ${warning}` : type}
            onClick={() => select(name)}
          >
            <span>{name}</span>
            {warning && (
              <Tooltip title={warning}>
                <MaterialSymbol
                  symbol="warning"
                  sx={{ fontSize: 18, color: 'warning.main' }}
                />
              </Tooltip>
            )}
            <Typography
              variant="body2"
              color="text.secondary"
              align="right"
              sx={{ gridColumn: 3 }}
            >
              {type}
            </Typography>
          </MenuItem>
        ));
      }}
    </FieldMenu>
  );
  return (
    <BindingContext.Provider value={{ ...context, pointer }}>
      <FieldActions id={field?.id ?? ''} actions={menu}>
        {children}
      </FieldActions>
    </BindingContext.Provider>
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
    <BindableField field={props} pointer={pointer}>
      {field}
    </BindableField>
  );
}

type OperationProps = BaseEditOperationFormProps<
  'node' | 'script' | 'section'
> &
  Pick<SchemaConfigFormProps, 'schema' | 'label'>;

export default function OperationConfigForm(props: OperationProps) {
  const { node, schema, label = 'Config' } = props;
  const { schemas: definitions } = useRegistry();
  const template = useActiveTemplate();
  const { drafts, setOperationConfigDraft } = useTransientEditorDrafts();
  const op = node.data.op;
  const replaceOp = operationUpdater(props);
  const key = `${op.type}:${node.id}:config`;
  const binding = template && {
    bindings: op.config_bindings ?? {},
    schema: schemaDocument(template.config_schema, definitions),
    update: (next: Bindings) =>
      replaceOp({
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
      onChange={(config) => replaceOp({ ...op, config })}
      fieldTemplate={binding ? OperationField : undefined}
      rawDraft={drafts.operationConfigs[key]}
      onRawDraftChange={(text) => setOperationConfigDraft(key, text)}
    />
  );
  return (
    <BindingContext.Provider value={binding ?? null}>
      {binding && (!schema || typeof schema === 'boolean') ? (
        <BindableField label={label}>{form}</BindableField>
      ) : (
        form
      )}
    </BindingContext.Provider>
  );
}
