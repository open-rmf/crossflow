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
import type { FieldProps, RJSFSchema, UiSchema } from '@rjsf/utils';
import { useMemo, useState } from 'react';
import { MaterialSymbol } from '../nodes';
import { useRegistry } from '../registry-provider';
import type { Schema } from '../types/api';
import ajv from '../utils/ajv';
import SchemaConfigForm from './schema-config-form';

type SchemaObject = Exclude<Schema, boolean>;
const types = [
  'string',
  'integer',
  'number',
  'boolean',
  'null',
  'array',
  'object',
];
const kinds = [...types, 'enum', 'anyOf', 'oneOf', '$ref', 'any'];
const initial = { type: 'string' };
const choiceRef = { $ref: '#/$defs/choice' };
const choice: RJSFSchema = {
  default: '',
  anyOf: [
    { type: 'string', default: '' },
    { type: 'number', default: 0 },
    { type: 'boolean', default: false },
    { type: 'null' },
    { type: 'array', items: choiceRef, default: [] },
    { type: 'object', additionalProperties: choiceRef, default: {} },
  ],
};

function concreteType(type: unknown) {
  if (Array.isArray(type) && type.length === 2 && type.includes('null'))
    type = type.find((t) => t !== 'null');
  return typeof type === 'string' && types.includes(type) ? type : undefined;
}

function withNull(schema: SchemaObject, allow: boolean): SchemaObject {
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
  if (schema && typeof schema === 'object') {
    if (Object.keys(schema).every((key) => key === 'title')) return 'any';
    if (Array.isArray(schema.enum)) return 'enum';
    const type = concreteType(schema.type);
    if (type) return type;
    for (const key of ['anyOf', 'oneOf', '$ref']) if (key in schema) return key;
  }
  return 'JSON';
}

function schemaForKind(
  kind: string,
  registeredRef: string | undefined,
): SchemaObject {
  switch (kind) {
    case 'any':
      return {};
    case '$ref':
      return { $ref: registeredRef };
    case 'enum':
      return { enum: ['option'] };
    case 'anyOf':
    case 'oneOf':
      return { [kind]: [initial] };
    case 'array':
      return { type: kind, items: initial };
    default:
      return { type: kind };
  }
}

export function schemaSummary(schema: Schema) {
  if (typeof schema === 'boolean') return schema ? 'any' : 'never';
  if (typeof schema.title === 'string') return schema.title;
  if (typeof schema.$ref === 'string') return schema.$ref.split('/').pop();
  return Array.isArray(schema.type) ? schema.type.join(' | ') : kindOf(schema);
}

function schemaError(value: unknown) {
  if (
    typeof value !== 'boolean' &&
    (!value || typeof value !== 'object' || Array.isArray(value))
  )
    return 'A JSON Schema must be an object or a boolean.';
  try {
    if (!ajv.validateSchema(value)) return ajv.errorsText(ajv.errors);
  } catch (err) {
    return String(err);
  }
}

interface EditorProps {
  label: string;
  value?: Schema;
  onChange: (value: Schema | undefined) => void;
  object?: boolean;
  removable?: boolean;
  alternative?: boolean;
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

function schemaList(items: UiSchema): UiSchema {
  const paper = { elevation: 0, sx: { bgcolor: 'transparent' } };
  const slots = (rjsfSlotProps: Record<string, unknown>) => ({
    'ui:options': { mui: { rjsfSlotProps } },
  });
  return {
    ...slots({
      arrayPaper: paper,
      arrayBox: { sx: { p: 0 } },
      titleBox: { sx: { mt: 0, mb: 2 } },
      titleTypography: { variant: 'subtitle2' },
    }),
    items: {
      ...items,
      ...slots({
        arrayItemPaper: paper,
        arrayItemInnerBox: { sx: { p: 0 } },
        arrayItemOuterBox: { sx: { mb: 0 } },
        arrayItemGridContainer: { sx: { gap: 1, mb: 2 } },
        arrayItemGridItem: { size: 'grow', sx: { overflow: 'visible' } },
        arrayItemToolbarGrid: { sx: { mt: 0 } },
      }),
    },
  };
}

// RJSF edits the schema document itself. For example, its standard array
// control adds and reorders the schema objects saved under anyOf or oneOf.
function SchemaFields({
  value,
  onChange,
  inlineTitle,
}: {
  value: SchemaObject;
  onChange: (value: SchemaObject) => void;
  inlineTitle?: boolean;
}) {
  const registry = useRegistry();
  const kind = kindOf(value);
  const type = concreteType(value.type);
  const choices = Array.isArray(value.enum) ? value.enum : undefined;
  // A sibling constraint can reject null even when the type admits it.
  // Those schemas retain their explicit alternatives or JSON editor.
  const canAllowNull =
    (type ? type !== 'null' : choices && value.type === undefined) &&
    ![
      'const',
      '$ref',
      '$dynamicRef',
      'allOf',
      'anyOf',
      'oneOf',
      'not',
      'if',
    ].some((key) => key in value);
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
    (name) => `#/schemas/${name.replace(/~/g, '~0').replace(/\//g, '~1')}`,
  );
  const fields: Record<string, RJSFSchema> = {};
  if (!inlineTitle) fields.title = { type: 'string', title: 'Title' };
  const nested = { 'ui:field': SchemaItem };
  if (kind === '$ref')
    fields.$ref = {
      type: 'string',
      title: 'Registered type',
      enum: [...new Set([...refs, String(value.$ref)])],
    };
  if (kind === 'enum') {
    const values = value.enum as NonNullable<RJSFSchema['enum']>;
    fields.enum = {
      type: 'array',
      title: 'Choices',
      minItems: 1,
      uniqueItems: true,
      items: { ...choiceRef, title: 'Choice type' },
    };
    if (values.length)
      fields.default = { title: 'Default value', enum: values };
  }
  if (kind === 'array') fields.items = { type: 'object', title: 'Items' };
  if (kind === 'anyOf' || kind === 'oneOf')
    fields[kind] = {
      type: 'array',
      title: 'Alternatives',
      minItems: 1,
      items: { type: 'object', default: initial },
    };
  if (['number', 'integer'].includes(kind)) {
    fields.minimum = { type: 'number', title: 'Minimum' };
    fields.maximum = { type: 'number', title: 'Maximum' };
  }
  if (['string', 'integer', 'number', 'boolean'].includes(kind))
    fields.default = {
      title: 'Default value',
      ...(kind === 'boolean'
        ? { enum: nullable ? [true, false, null] : [true, false] }
        : { type: value.type as RJSFSchema['type'] }),
    };
  const data = Object.fromEntries(
    Object.keys(fields)
      .filter((k) => k in value)
      .map((k) => [k, value[k]]),
  );
  return (
    <Stack spacing={2}>
      <Stack direction="row" spacing={2} alignItems="center">
        <TextField
          select
          fullWidth
          label="Type"
          value={kind}
          onChange={({ target }) => {
            const next = schemaForKind(target.value, refs[0]);
            onChange({
              ...(canAllowNull && nullable ? withNull(next, true) : next),
              ...(value.title !== undefined && { title: value.title }),
            });
          }}
        >
          {kinds
            .filter((k) => k !== '$ref' || refs.length || kind === '$ref')
            .map((k) => (
              <MenuItem key={k} value={k}>
                {k === '$ref' ? 'Registered type' : k}
              </MenuItem>
            ))}
        </TextField>
        {canAllowNull && (
          <FormControlLabel
            sx={{ flexShrink: 0 }}
            label="Allow null"
            control={
              <Checkbox
                checked={nullable}
                onChange={(_, checked) => onChange(withNull(value, checked))}
              />
            }
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
        value={data}
        uiSchema={{
          default: { 'ui:placeholder': 'No default' },
          enum: schemaList({ 'ui:emptyValue': '' }),
          items: nested,
          anyOf: schemaList(nested),
          oneOf: schemaList(nested),
        }}
        onChange={(next) => {
          const kept = Object.fromEntries(
            Object.entries(value).filter(([key]) => !(key in fields)),
          );
          // Older enums declared a redundant type. Widen it when the author
          // adds a choice of another type, preserving other schema constraints.
          if (
            enumType &&
            (value.enum as unknown[]).every((item) => enumType(item)) &&
            (next as { enum: unknown[] }).enum.some((item) => !enumType(item))
          )
            delete kept.type;
          onChange({ ...kept, ...(next as SchemaObject) });
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
  onSave: (schema: Schema, name: string, required: boolean) => void;
  onClose: () => void;
}

function SchemaDialog(props: DraftProps) {
  const [name, setName] = useState(props.name ?? '');
  const [required, setRequired] = useState(props.required ?? false);
  const [value, setValue] = useState(props.value);
  const [json, setJson] = useState<string | undefined>(
    props.json || schemaError(value) || kindOf(value) === 'JSON'
      ? JSON.stringify(value, null, 2)
      : undefined,
  );
  let schema: unknown = value;
  let error: string | undefined;
  if (json !== undefined) {
    try {
      schema = JSON.parse(json);
    } catch (err) {
      error = String(err);
    }
  }
  error ??= schemaError(schema);
  const duplicate = props.name === '' && props.names?.includes(name);
  return (
    <Dialog open onClose={props.onClose} fullWidth maxWidth="sm">
      <DialogTitle>{props.label}</DialogTitle>
      <DialogContent>
        <Stack spacing={2} sx={{ pt: 1 }}>
          {props.name === '' && (
            <TextField
              autoFocus
              label="Name"
              required
              value={name}
              error={!!duplicate}
              helperText={
                duplicate
                  ? 'A property with this name already exists.'
                  : undefined
              }
              onChange={({ target }) => setName(target.value)}
            />
          )}
          {json !== undefined ? (
            <TextField
              fullWidth
              multiline
              minRows={4}
              maxRows={16}
              label="JSON Schema"
              value={json}
              onChange={({ target }) => setJson(target.value)}
              error={!!error}
              helperText={error}
              slotProps={{ input: { sx: { fontFamily: 'monospace' } } }}
            />
          ) : (
            <SchemaFields
              value={value as SchemaObject}
              onChange={setValue}
              inlineTitle={props.inlineTitle}
            />
          )}
          {props.name !== undefined && (
            <FormControlLabel
              label="Required"
              control={
                <Checkbox
                  checked={required}
                  onChange={(_, checked) => setRequired(checked)}
                />
              }
            />
          )}
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button
          sx={{ mr: 'auto' }}
          disabled={
            json !== undefined &&
            (!!error || kindOf(schema as Schema) === 'JSON')
          }
          onClick={() => {
            if (json === undefined) setJson(JSON.stringify(value, null, 2));
            else {
              setValue(schema as Schema);
              setJson(undefined);
            }
          }}
        >
          {json !== undefined ? 'Form' : 'JSON'}
        </Button>
        <Button onClick={props.onClose}>Cancel</Button>
        <Button
          disabled={
            !!error || !!duplicate || (props.name !== undefined && name === '')
          }
          onClick={() => props.onSave(schema as Schema, name, required)}
        >
          Apply
        </Button>
      </DialogActions>
    </Dialog>
  );
}

function Properties({
  value,
  onChange,
}: {
  value: SchemaObject;
  onChange: (schema: SchemaObject) => void;
}) {
  const fields = (value.properties ?? {}) as Record<string, Schema>;
  const required = (value.required ?? []) as string[];
  const [editing, setEditing] = useState<{ key?: string } | null>(null);
  function update(name: string, schema?: Schema, isRequired = false) {
    const properties = { ...fields, [name]: schema };
    if (schema === undefined) delete properties[name];
    const keys = isRequired
      ? [...new Set([...required, name])]
      : required.filter((key) => key !== name);
    const { required: _previous, ...rest } = value;
    onChange({
      ...rest,
      properties,
      ...(keys.length > 0 && { required: keys }),
    });
    setEditing(null);
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
          <Typography
            variant="body2"
            sx={{ flex: 1, overflowWrap: 'anywhere' }}
          >
            {name}
            {required.includes(name) && ' *'}
          </Typography>
          <Typography variant="body2" color="text.secondary">
            {schemaSummary(schema)}
          </Typography>
          <IconButton
            size="small"
            aria-label={`Edit ${name}`}
            onClick={() => setEditing({ key: name })}
          >
            <MaterialSymbol symbol="edit" />
          </IconButton>
          <IconButton
            size="small"
            aria-label={`Delete ${name}`}
            onClick={() => {
              if (window.confirm(`Delete property “${name}”?`)) update(name);
            }}
          >
            <MaterialSymbol symbol="delete" />
          </IconButton>
        </Stack>
      ))}
      <Button
        aria-label="Add property"
        sx={{ alignSelf: 'start' }}
        startIcon={<MaterialSymbol symbol="add" />}
        onClick={() => setEditing({})}
      >
        Add property
      </Button>
      {editing && (
        <SchemaDialog
          label={
            editing.key === undefined ? 'Add property' : `Edit ${editing.key}`
          }
          name={editing.key ?? ''}
          value={editing.key === undefined ? initial : fields[editing.key]}
          names={Object.keys(fields)}
          required={required.includes(editing.key ?? '')}
          onClose={() => setEditing(null)}
          onSave={(schema, name, required) => update(name, schema, required)}
        />
      )}
    </Stack>
  );
}

export default function SchemaEditor({
  label,
  value,
  onChange,
  object = false,
  removable = true,
  alternative = false,
}: EditorProps) {
  const [open, setOpen] = useState(false);
  const schema: Schema = value ?? (object ? { type: 'object' } : initial);
  const inlineTitle = alternative && typeof schema === 'object';
  const { title, ...definition } = inlineTitle ? schema : {};
  const name = typeof title === 'string' ? title : '';
  return (
    <Stack spacing={1} component="section" aria-label={label}>
      <Stack direction="row" spacing={1} alignItems="center">
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
          <Typography variant="body2" color="text.secondary">
            {schemaSummary(inlineTitle ? definition : value)}
          </Typography>
        )}
        <Button size="small" onClick={() => setOpen(true)}>
          {object ? 'Edit JSON' : value === undefined ? 'Declare' : 'Edit'}
        </Button>
        {removable && value !== undefined && (
          <IconButton
            size="small"
            aria-label={`Remove ${label}`}
            onClick={() => {
              if (!object || window.confirm('Remove the configuration schema?'))
                onChange(undefined);
            }}
          >
            <MaterialSymbol symbol="delete" />
          </IconButton>
        )}
      </Stack>
      {object && !schemaError(schema) && kindOf(schema) === 'object' && (
        <>
          <Divider />
          <Properties value={schema as SchemaObject} onChange={onChange} />
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
            setOpen(false);
          }}
        />
      )}
    </Stack>
  );
}
