import {
  Button,
  Checkbox,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  Divider,
  FormControlLabel,
  IconButton,
  MenuItem,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import {
  type FieldProps,
  isObject,
  type RJSFSchema,
  type UiSchema,
} from '@rjsf/utils';
import type { NodeChange } from '@xyflow/react';
import { createContext, useContext, useMemo, useState } from 'react';
import {
  MaterialSymbol,
  type SectionInputNode,
  type SectionOutputNode,
} from '../nodes';
import { useRegistry } from '../registry-provider';
import ajv from '../utils/ajv';
import { pointerSegment } from '../utils/schema-types';
import SchemaConfigForm from './schema-config-form';
import TypeLabel from './type-label';

type Schema = RJSFSchema | boolean;
const initial: RJSFSchema = { type: 'string' };
const schemasByKind: Record<string, RJSFSchema> = {
  string: initial,
  integer: { type: 'integer' },
  number: { type: 'number' },
  boolean: { type: 'boolean' },
  null: { type: 'null' },
  array: { type: 'array', items: initial },
  object: { type: 'object' },
  enum: { enum: ['option'] },
  anyOf: { anyOf: [initial] },
  oneOf: { oneOf: [initial] },
  any: {},
};
const choiceRef = { $ref: '#/$defs/choice' };
const nullConstraints = [
  'const',
  '$ref',
  '$dynamicRef',
  'allOf',
  'anyOf',
  'oneOf',
  'not',
  'if',
];
const choice: RJSFSchema = {
  default: '',
  oneOf: [
    { type: 'string', default: '' },
    { type: 'number', default: 0 },
    { type: 'boolean', enum: [true, false], default: false },
    { type: 'null' },
    { type: 'array', items: choiceRef, default: [] },
    { type: 'object', additionalProperties: choiceRef, default: {} },
  ],
};

function concreteType(type: unknown) {
  if (Array.isArray(type) && type.length === 2 && type.includes('null'))
    type = type.find((t) => t !== 'null');
  return typeof type === 'string' && schemasByKind[type]?.type === type
    ? type
    : undefined;
}

function withNull(schema: RJSFSchema, allow: boolean): RJSFSchema {
  const next = { ...schema };
  const type = concreteType(schema.type);
  if (type && type !== 'null') next.type = allow ? [type, 'null'] : type;
  if (Array.isArray(schema.enum)) {
    const values = schema.enum.filter((v) => v !== null);
    next.enum = allow ? [...values, null] : values;
  }
  if (!allow && schema.default === null) delete next.default;
  return next;
}

function kindOf(schema: Schema) {
  if (typeof schema === 'boolean') return 'JSON';
  if (Object.keys(schema).every((key) => key === 'title')) return 'any';
  if (Array.isArray(schema.enum)) return 'enum';
  return (
    concreteType(schema.type) ??
    ['anyOf', 'oneOf', '$ref'].find((key) => key in schema) ??
    'JSON'
  );
}

export function schemaSummary(schema: Schema) {
  if (typeof schema === 'boolean') return schema ? 'any' : 'never';
  if (typeof schema.title === 'string') return schema.title;
  if (typeof schema.$ref === 'string') return schema.$ref.split('/').pop();
  return Array.isArray(schema.type) ? schema.type.join(' | ') : kindOf(schema);
}

function readSchema(draft: Schema | string) {
  try {
    const schema: Schema =
      typeof draft === 'string' ? JSON.parse(draft) : draft;
    if (typeof schema !== 'boolean' && !isObject(schema))
      return { error: 'A JSON Schema must be an object or a boolean.' };
    if (!ajv.validateSchema(schema))
      return { schema, error: ajv.errorsText(ajv.errors) };
    return { schema, kind: kindOf(schema) };
  } catch (err) {
    return { error: String(err) };
  }
}

export type RenameProperty = (
  from: string,
  to: string,
  schema: RJSFSchema,
) => string | undefined;

const PortSchemaContext = createContext(false);

export function PortSchemaEditor<
  N extends SectionInputNode | SectionOutputNode,
>({ node, onChange }: { node: N; onChange?: (change: NodeChange<N>) => void }) {
  return (
    <PortSchemaContext.Provider value>
      <SchemaEditor
        label={node.type === 'sectionInput' ? 'Input schema' : 'Output schema'}
        value={node.data.schema}
        onChange={(schema) =>
          onChange?.({
            type: 'replace',
            id: node.id,
            item: { ...node, data: { ...node.data, schema } },
          })
        }
      />
    </PortSchemaContext.Provider>
  );
}

function SchemaItem({ formData, onChange, fieldPathId }: FieldProps<Schema>) {
  const index = fieldPathId.path.at(-1);
  return (
    <SchemaEditor
      alternative={typeof index === 'number'}
      label={
        typeof index === 'number' ? `Alternative ${index + 1}` : 'Item schema'
      }
      removable={false}
      value={formData ?? initial}
      onChange={(schema) => onChange(schema, fieldPathId.path)}
    />
  );
}

interface SchemaFieldsProps {
  value: RJSFSchema;
  onChange: (value: RJSFSchema) => void;
}

function schemaList(items: UiSchema): UiSchema {
  // RJSF provides the add button and item toolbar. Share the title row with
  // the add button, keeping a single column of items below it.
  return {
    'ui:mui': {
      rjsfSlotProps: {
        arrayPaper: { elevation: 0, sx: { bgcolor: 'transparent' } },
        arrayBox: { sx: { p: 0, display: 'grid' } },
        titleBox: {
          sx: {
            gridArea: '1 / 1',
            m: 0,
            alignContent: 'center',
            borderBottom: 1,
            borderColor: 'divider',
          },
        },
        titleTypography: { variant: 'subtitle2', sx: { width: 'fit-content' } },
        titleDivider: { sx: { display: 'none' } },
        arrayAddButtonGridContainer: {
          sx: { gridArea: '1 / 1', justifySelf: 'end' },
        },
        arrayAddButtonBox: { sx: { mt: 0 } },
      },
    },
    items,
  };
}

const schemaItem = { 'ui:field': SchemaItem };
const alternatives = schemaList(schemaItem);
const schemaUi: UiSchema = {
  default: { 'ui:placeholder': 'No default' },
  enum: schemaList({ 'ui:emptyValue': '' }),
  items: schemaItem,
  anyOf: alternatives,
  oneOf: alternatives,
};

// RJSF edits the schema document itself. For example, its standard array
// control adds and reorders the schema objects saved under anyOf or oneOf.
function SchemaFields({
  value,
  onChange,
  inlineTitle,
}: SchemaFieldsProps & { inlineTitle?: boolean }) {
  const registry = useRegistry();
  const port = useContext(PortSchemaContext);
  const kind = kindOf(value);
  const type = concreteType(value.type);
  const choices = value.enum;
  // A sibling constraint can reject null even when the type admits it.
  // Those schemas retain their explicit alternatives or JSON editor.
  const canAllowNull =
    (type ? type !== 'null' : choices && value.type === undefined) &&
    !nullConstraints.some((key) => key in value);
  const nullable =
    (value.type === undefined ||
      (Array.isArray(value.type) && value.type.includes('null'))) &&
    (!choices || choices.includes(null));
  const enumType = useMemo(
    () =>
      kind === 'enum' && value.type
        ? ajv.compile({ type: value.type })
        : undefined,
    [kind, value.type],
  );
  const refs = Object.keys(registry.schemas).map(
    (name) => `#/schemas/${pointerSegment(name)}`,
  );
  const fields: Record<string, RJSFSchema> = {};
  if (!inlineTitle) fields.title = { type: 'string', title: 'Title' };
  if (kind === '$ref')
    fields.$ref = {
      type: 'string',
      title: 'Registered type',
      enum: [...new Set([...refs, String(value.$ref)])],
    };
  if (choices || kind === 'anyOf' || kind === 'oneOf') {
    fields[kind] = {
      type: 'array',
      minItems: 1,
      title: choices ? 'Choices' : 'Alternatives',
      items: choices
        ? { ...choiceRef, title: 'Choice type' }
        : { type: 'object', default: initial },
    };
    if (choices) fields.enum.uniqueItems = true;
  }
  if (kind === 'array') fields.items = { type: 'object', title: 'Items' };
  if (['number', 'integer'].includes(kind)) {
    fields.minimum = { type: 'number', title: 'Minimum' };
    fields.maximum = { type: 'number', title: 'Maximum' };
  }
  if (
    !port &&
    (choices?.length ||
      ['string', 'integer', 'number', 'boolean'].includes(kind))
  ) {
    fields.default = { title: 'Default value' };
    if (choices) fields.default.enum = choices;
    else if (kind === 'boolean')
      fields.default.enum = nullable ? [true, false, null] : [true, false];
    else fields.default.type = value.type;
  }
  const kinds = { ...schemasByKind };
  if (refs.length || kind === '$ref') kinds.$ref = { $ref: refs[0] };
  return (
    <Stack spacing={2}>
      <Stack direction="row" spacing={2} alignItems="center">
        <TextField
          select
          fullWidth
          label="Type"
          value={kind}
          onChange={({ target }) => {
            const next = kinds[target.value];
            onChange({
              ...(canAllowNull && nullable ? withNull(next, true) : next),
              ...(value.title !== undefined && { title: value.title }),
            });
          }}
        >
          {Object.keys(kinds).map((k) => (
            <MenuItem key={k} value={k}>
              {k === '$ref' ? 'Registered type' : k}
            </MenuItem>
          ))}
        </TextField>
        {canAllowNull && (
          <FormControlLabel
            sx={{ flexShrink: 0 }}
            label="Allow null"
            control={<Checkbox />}
            checked={nullable}
            onChange={(_, checked) => onChange(withNull(value, checked))}
          />
        )}
      </Stack>
      <SchemaConfigForm
        label=""
        schema={{
          type: 'object',
          properties: fields,
          ...(kind === 'enum' && { $defs: { choice } }),
        }}
        value={value}
        uiSchema={schemaUi}
        onChange={(data) => {
          const next = data as RJSFSchema;
          // Older enums declared a redundant type. Widen it when the author
          // adds a choice of another type, preserving other schema constraints.
          if (
            enumType &&
            choices?.every((item) => enumType(item)) &&
            next.enum?.some((item) => !enumType(item))
          ) {
            const { type: _oldType, ...widened } = next;
            onChange(widened);
          } else onChange(next);
        }}
      />
      {kind === 'object' && <Properties value={value} onChange={onChange} />}
    </Stack>
  );
}

interface DraftProps {
  label: string;
  value: Schema;
  name?: string;
  required?: boolean;
  names?: string[];
  json?: boolean;
  inlineTitle?: boolean;
  onSave: (
    schema: Schema,
    name: string,
    required: boolean,
  ) => string | undefined;
  onClose: () => void;
}

function SchemaDialog(props: DraftProps) {
  const [name, setName] = useState(props.name ?? '');
  const [nameError, setNameError] = useState<string>();
  const [required, setRequired] = useState(props.required ?? false);
  const [draft, setDraft] = useState<Schema | string>(() => {
    const { kind } = readSchema(props.value);
    return props.json || !kind || kind === 'JSON'
      ? JSON.stringify(props.value, null, 2)
      : props.value;
  });
  const json = typeof draft === 'string';
  const { schema = props.value, kind, error } = readSchema(draft);
  const duplicate =
    name !== props.name && props.names?.includes(name)
      ? 'A property with this name already exists.'
      : undefined;
  return (
    <Dialog open onClose={props.onClose} fullWidth maxWidth="sm">
      <DialogTitle>{props.label}</DialogTitle>
      <DialogContent>
        <Stack spacing={2} sx={{ pt: 1 }}>
          {props.names && (
            <TextField
              autoFocus
              label="Name"
              required
              value={name}
              error={!!duplicate || !!nameError}
              helperText={duplicate ?? nameError}
              onChange={({ target }) => {
                setName(target.value);
                setNameError(undefined);
              }}
            />
          )}
          {json ? (
            <TextField
              fullWidth
              multiline
              minRows={4}
              maxRows={16}
              label="JSON Schema"
              value={draft}
              onChange={({ target }) => setDraft(target.value)}
              error={!!error}
              helperText={error}
              slotProps={{ input: { sx: { fontFamily: 'monospace' } } }}
            />
          ) : (
            <SchemaFields
              value={schema as RJSFSchema}
              onChange={setDraft}
              inlineTitle={props.inlineTitle}
            />
          )}
          {props.name !== undefined && (
            <FormControlLabel
              sx={{ ml: 0 }}
              label="Required"
              control={<Checkbox />}
              checked={required}
              onChange={(_, checked) => setRequired(checked)}
            />
          )}
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button
          sx={{ mr: 'auto' }}
          disabled={json && (!kind || kind === 'JSON')}
          onClick={() =>
            setDraft(json ? schema : JSON.stringify(schema, null, 2))
          }
        >
          {json ? 'Edit form' : 'Edit JSON'}
        </Button>
        <Button onClick={props.onClose}>Cancel</Button>
        <Button
          disabled={
            !!error || !!duplicate || (props.name !== undefined && name === '')
          }
          onClick={() => {
            const message = props.onSave(schema, name, required);
            if (message) setNameError(message);
            else props.onClose();
          }}
        >
          Apply
        </Button>
      </DialogActions>
    </Dialog>
  );
}

function iconButton(label: string, symbol: string, onClick: () => void) {
  return (
    <IconButton size="small" aria-label={label} onClick={onClick}>
      <MaterialSymbol symbol={symbol} />
    </IconButton>
  );
}

function Properties({
  value,
  onChange,
  onRenameProperty,
}: SchemaFieldsProps & { onRenameProperty?: RenameProperty }) {
  const fields = value.properties ?? {};
  const required = value.required ?? [];
  const [editing, setEditing] = useState<string | null>();
  function update(
    schema: Schema | undefined,
    name: string,
    isRequired = false,
  ) {
    const previous = editing ?? name;
    const rename = (key: string) => (key === previous ? name : key);
    const properties = new Map(
      Object.entries(fields).map(([key, field]) => [rename(key), field]),
    );
    if (schema === undefined) properties.delete(name);
    else properties.set(name, schema);
    const keys = new Set(required.map(rename));
    if (isRequired) keys.add(name);
    else keys.delete(name);
    const next: RJSFSchema = {
      ...value,
      properties: Object.fromEntries(properties),
    };
    if (keys.size) next.required = [...keys];
    else delete next.required;
    if (previous !== name && onRenameProperty)
      return onRenameProperty(previous, name, next);
    onChange(next);
  }
  return (
    <Stack spacing={1}>
      {Object.entries(fields).map(([name, schema]) => (
        <Stack
          key={name}
          direction="row"
          spacing={1}
          alignItems="center"
          sx={{ borderBottom: 1, borderColor: 'divider', py: 0.5 }}
        >
          <Stack
            direction="row"
            spacing={1}
            alignItems="baseline"
            sx={{ flex: 1, minWidth: 0 }}
          >
            <Typography
              variant="body2"
              sx={{ flex: 1, overflowWrap: 'anywhere' }}
            >
              {name}
              {required.includes(name) && ' *'}
            </Typography>
            <TypeLabel>{schemaSummary(schema)}</TypeLabel>
          </Stack>
          {iconButton(`Edit ${name}`, 'edit', () => setEditing(name))}
          {iconButton(`Delete ${name}`, 'delete', () => {
            if (window.confirm(`Delete property “${name}”?`))
              update(undefined, name);
          })}
        </Stack>
      ))}
      <Button
        aria-label="Add property"
        sx={{ alignSelf: 'start' }}
        startIcon={<MaterialSymbol symbol="add" />}
        onClick={() => setEditing(null)}
      >
        Add property
      </Button>
      {editing !== undefined && (
        <SchemaDialog
          label={editing === null ? 'Add property' : `Edit ${editing}`}
          name={editing ?? ''}
          value={editing === null ? initial : fields[editing]}
          names={
            editing === null || onRenameProperty
              ? Object.keys(fields)
              : undefined
          }
          required={editing !== null && required.includes(editing)}
          onClose={() => setEditing(undefined)}
          onSave={update}
        />
      )}
    </Stack>
  );
}

export default function SchemaEditor({
  label,
  value,
  onChange,
  onRenameProperty,
  object = false,
  removable = true,
  alternative = false,
}: {
  label: string;
  value?: Schema;
  onChange: (value: Schema | undefined) => void;
  onRenameProperty?: RenameProperty;
  object?: boolean;
  removable?: boolean;
  alternative?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const schema: Schema = value ?? (object ? { type: 'object' } : initial);
  const inlineTitle = alternative && typeof schema === 'object';
  const { title, ...definition } = inlineTitle ? schema : {};
  const name = typeof title === 'string' ? title : '';
  return (
    <Stack spacing={1} component="section" aria-label={label}>
      <Stack
        direction="row"
        spacing={1}
        alignItems={removable || inlineTitle ? 'center' : 'baseline'}
      >
        {inlineTitle ? (
          <TextField
            variant="standard"
            placeholder={label}
            slotProps={{ htmlInput: { 'aria-label': label } }}
            sx={{ flex: 1 }}
            value={name}
            onChange={({ target }) =>
              onChange(
                target.value
                  ? { ...definition, title: target.value }
                  : definition,
              )
            }
          />
        ) : (
          <Typography variant="subtitle2" sx={{ flex: 1 }}>
            {label}
          </Typography>
        )}
        {!object && value !== undefined && (
          <TypeLabel>
            {schemaSummary(inlineTitle ? definition : value)}
          </TypeLabel>
        )}
        <Button size="small" onClick={() => setOpen(true)}>
          {object ? 'Edit JSON' : value === undefined ? 'Declare' : 'Edit'}
        </Button>
        {removable &&
          value !== undefined &&
          iconButton(`Remove ${label}`, 'delete', () => {
            if (!object || window.confirm('Remove the configuration schema?'))
              onChange(undefined);
          })}
      </Stack>
      {object && readSchema(schema).kind === 'object' && (
        <>
          <Divider />
          <Properties
            value={schema as RJSFSchema}
            onChange={onChange}
            onRenameProperty={onRenameProperty}
          />
        </>
      )}
      {open && (
        <SchemaDialog
          label={name || label}
          inlineTitle={inlineTitle}
          value={schema}
          json={object}
          onClose={() => setOpen(false)}
          onSave={(next) => {
            onChange(next);
            return undefined;
          }}
        />
      )}
    </Stack>
  );
}
