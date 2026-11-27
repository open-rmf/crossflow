import { Chip, MenuItem, TextField, Tooltip } from '@mui/material';
import { Templates } from '@rjsf/mui';
import type { FieldTemplateProps } from '@rjsf/utils';
import {
  type ComponentType,
  createContext,
  type PropsWithChildren,
  useContext,
} from 'react';
import { EditPopoverWidth } from '../diagram-side-panel-layout';
import { MaterialSymbol } from '../nodes';
import { useRegistry } from '../registry-provider';
import { useActiveTemplate } from '../templates-provider';
import { useTransientEditorDrafts } from '../transient-editor-drafts';
import {
  bindingProperties,
  fieldPointer,
  schemaDocument,
  withinField,
} from '../utils/schema-types';
import {
  type BaseEditOperationFormProps,
  operationUpdater,
} from './base-edit-operation-form';
import { FieldActions, FieldMenu } from './field-actions';
import SchemaConfigForm, {
  type SchemaConfigFormProps,
} from './schema-config-form';
import TypeLabel from './type-label';

type Bindings = Record<string, string>;
const BindingContext = createContext<{
  schema: ReturnType<typeof schemaDocument>;
  pointer?: string;
  bindings: Bindings;
  setLink: (pointer: string, property?: string) => void;
} | null>(null);
const MuiField = Templates.FieldTemplate as ComponentType<FieldTemplateProps>;

// RJSF supplies the field template for a known schema. A raw config supplies
// its editor directly, and can bind the root value in the same way.
function BindableField(
  props: FieldTemplateProps | PropsWithChildren<{ label: string }>,
) {
  const field = 'fieldPathId' in props ? props : undefined;
  const children = field ? <MuiField {...field} /> : props.children;
  const { label } = props;
  const context = useContext(BindingContext);
  const path = field?.fieldPathId.path ?? [];
  // Reordering a list would make an index binding point to a different item.
  if (
    !context ||
    field?.hidden ||
    path.some((segment) => typeof segment === 'number')
  )
    return children;
  const pointer = fieldPointer(path);
  if (context.pointer === pointer) return children;

  const linked = context.bindings[pointer];
  if (linked !== undefined) {
    return (
      <TextField
        fullWidth
        label={label}
        required={field?.required}
        slotProps={{
          htmlInput: { inert: true },
          input: {
            readOnly: true,
            startAdornment: (
              <Chip
                className="nokey"
                label={linked}
                aria-label={`${label} linked to ${linked}`}
                aria-keyshortcuts="Backspace Delete"
                color="primary"
                icon={<MaterialSymbol symbol="link" />}
                onDelete={() => context.setLink(pointer)}
              />
            ),
          },
        }}
      />
    );
  }
  if (!path.length && field?.schema.type === 'object') return children;

  const menu = (
    <FieldMenu
      symbol="link"
      label={`Link ${label} to a section property`}
      menuListSx={{
        display: 'grid',
        maxWidth: EditPopoverWidth,
        gridTemplateColumns: 'minmax(0, 1fr) auto fit-content(50%)',
        columnGap: 1,
        '& > li': {
          display: 'grid',
          gridColumn: '1 / -1',
          gridTemplateColumns: 'subgrid',
          whiteSpace: 'normal',
          overflowWrap: 'anywhere',
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
            onClick={() => context.setLink(pointer, name)}
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
            <TypeLabel sx={{ gridColumn: 3 }}>{type}</TypeLabel>
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
  function updateConfig(
    config: unknown,
    removed: string[] = [],
    link?: [string, string],
  ) {
    const links = new Map(Object.entries(op.config_bindings ?? {}));
    for (const path of removed) links.delete(path);
    if (link) links.set(...link);
    replaceOp({
      ...op,
      config,
      config_bindings: links.size ? Object.fromEntries(links) : undefined,
    });
  }
  const trackedFields = Object.keys(op.config_bindings ?? {});
  const binding = template && {
    bindings: op.config_bindings ?? {},
    schema: schemaDocument(template.config_schema, definitions),
    setLink: (pointer: string, property?: string) =>
      updateConfig(
        op.config,
        trackedFields.filter((path) => withinField(path, pointer)),
        property === undefined ? undefined : [pointer, property],
      ),
  };
  const form = (
    <SchemaConfigForm
      schema={schema}
      definitions={definitions}
      label={label}
      allowJson
      nodeDefaults={op.type === 'node'}
      value={op.config}
      trackedFields={trackedFields}
      onChange={updateConfig}
      fieldTemplate={binding ? BindableField : undefined}
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
