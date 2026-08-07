import {
  Box,
  Checkbox,
  FormControlLabel,
  FormHelperText,
  MenuItem,
  Stack,
  TextField,
  Typography,
} from '@mui/material';
import equal from 'fast-deep-equal';
import { useEffect, useMemo, useState } from 'react';

type JsonConfigObject = Record<string, unknown>;

type JsonSchema = Record<string, unknown>;
type EnumValue = string | number;

/**
 * A schema paired with the control that edits it. `raw` is the bottom case:
 * whatever this form cannot express as a typed control is edited as JSON, so an
 * unsupported corner of a schema never limits what the user can enter, and a
 * node without a schema is just the case where the whole config is raw.
 */
type ResolvedSchema = {
  description?: string;
  defaultValue?: unknown;
} & (
  | { type: 'raw' }
  | { type: 'string'; enumValues?: EnumValue[] }
  | {
      type: 'number' | 'integer';
      enumValues?: EnumValue[];
      minimum?: number;
      maximum?: number;
    }
  | { type: 'boolean' }
  | {
      type: 'object';
      properties: Record<string, ResolvedSchema>;
      required: Set<string>;
    }
);

type ResolvedObjectSchema = Extract<ResolvedSchema, { type: 'object' }>;
type ResolvedNumberSchema = Extract<
  ResolvedSchema,
  { type: 'number' | 'integer' }
>;

interface SchemaConfigFormProps {
  schema: unknown;
  definitions: Record<string, unknown>;
  value?: unknown;
  onChange: (value: unknown) => void;
}

function schemaObject(value: unknown): JsonSchema | null {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? (value as JsonSchema)
    : null;
}

// `Object.hasOwn` needs a newer lib target than this package compiles against,
// and biome rewrites a `hasOwnProperty` call into it.
function hasOwn(object: object, key: PropertyKey): boolean {
  return Object.getOwnPropertyDescriptor(object, key) !== undefined;
}

function resolveRef(
  ref: string,
  definitions: Record<string, unknown>,
): JsonSchema | null {
  if (!ref.startsWith('#/')) {
    return null;
  }
  const name = ref.split('/').at(-1)?.replace(/~1/g, '/').replace(/~0/g, '~');
  return name ? schemaObject(definitions[name]) : null;
}

function isNullSchema(option: unknown): boolean {
  const type = schemaObject(option)?.type;
  return Array.isArray(type) ? type.includes('null') : type === 'null';
}

function schemaType(schema: JsonSchema): string | null {
  if (typeof schema.type === 'string') {
    return schema.type;
  }
  if (Array.isArray(schema.type)) {
    const nonNullTypes = schema.type.filter((type) => type !== 'null');
    return schema.type.includes('null') &&
      nonNullTypes.length === 1 &&
      typeof nonNullTypes[0] === 'string'
      ? nonNullTypes[0]
      : null;
  }
  if (Array.isArray(schema.enum) && schema.enum.length > 0) {
    return schema.enum.every((value) => typeof value === 'string')
      ? 'string'
      : null;
  }
  return null;
}

const unsupportedKeywords = [
  'allOf',
  'oneOf',
  'not',
  'if',
  'then',
  'else',
  'items',
  'prefixItems',
  'contains',
  'patternProperties',
  'dependentSchemas',
  'dependencies',
];

/**
 * Resolves the parts of a schema this form has a typed control for, returning
 * `null` for anything it cannot type. Callers go through {@link resolveSchema},
 * which turns that `null` into a raw JSON field.
 */
function resolveTypedSchema(
  schema: JsonSchema,
  definitions: Record<string, unknown>,
  resolvingRefs: Set<string>,
): ResolvedSchema | null {
  if (typeof schema.$ref === 'string') {
    // A cycle resolves to a raw field at the point it closes, leaving the rest
    // of the schema typed.
    if (resolvingRefs.has(schema.$ref)) {
      return null;
    }
    const referenced = resolveRef(schema.$ref, definitions);
    if (!referenced) {
      return null;
    }
    const { $ref: _ref, ...overrides } = schema;
    return resolveTypedSchema(
      { ...referenced, ...overrides },
      definitions,
      new Set(resolvingRefs).add(schema.$ref),
    );
  }

  // `Option<T>` serializes as a two-branch `anyOf`; unwrap it to edit the `T`.
  if (Array.isArray(schema.anyOf)) {
    const nonNull = schema.anyOf.filter((option) => !isNullSchema(option));
    const unwrapped = schemaObject(nonNull[0]);
    if (nonNull.length !== schema.anyOf.length - 1 || !unwrapped) {
      return null;
    }
    const { anyOf: _anyOf, ...overrides } = schema;
    return resolveTypedSchema(
      { ...unwrapped, ...overrides },
      definitions,
      resolvingRefs,
    );
  }

  if (unsupportedKeywords.some((key) => key in schema)) {
    return null;
  }

  const type = schemaType(schema);
  const base = {
    description:
      typeof schema.description === 'string' ? schema.description : undefined,
    defaultValue: schema.default,
  };

  if (type === 'boolean') {
    return schema.enum === undefined ? { ...base, type } : null;
  }

  if (type === 'string' || type === 'number' || type === 'integer') {
    let enumValues: EnumValue[] | undefined;
    if (schema.enum !== undefined) {
      if (
        !Array.isArray(schema.enum) ||
        schema.enum.length === 0 ||
        !schema.enum.every(
          (value) => typeof value === 'string' || typeof value === 'number',
        )
      ) {
        return null;
      }
      enumValues = schema.enum as EnumValue[];
    }
    if (type === 'string') {
      return { ...base, type, enumValues };
    }
    return {
      ...base,
      type,
      enumValues,
      minimum: typeof schema.minimum === 'number' ? schema.minimum : undefined,
      maximum: typeof schema.maximum === 'number' ? schema.maximum : undefined,
    };
  }

  if (type === 'object') {
    // A map type has no fixed set of fields to lay out, so edit it as raw JSON.
    if (
      schema.additionalProperties !== undefined &&
      schema.additionalProperties !== false
    ) {
      return null;
    }
    const rawProperties = schemaObject(schema.properties ?? {});
    const requiredValues = schema.required ?? [];
    if (
      !rawProperties ||
      !Array.isArray(requiredValues) ||
      !requiredValues.every((value) => typeof value === 'string')
    ) {
      return null;
    }
    const properties: Record<string, ResolvedSchema> = {};
    for (const [name, propertySchema] of Object.entries(rawProperties)) {
      properties[name] = resolveSchema(
        propertySchema,
        definitions,
        resolvingRefs,
      );
    }
    return {
      ...base,
      type,
      properties,
      required: new Set(requiredValues as string[]),
    };
  }

  return null;
}

function resolveSchema(
  rawSchema: unknown,
  definitions: Record<string, unknown>,
  resolvingRefs = new Set<string>(),
): ResolvedSchema {
  const schema = schemaObject(rawSchema);
  const typed =
    schema && resolveTypedSchema(schema, definitions, resolvingRefs);
  return (
    typed ?? {
      type: 'raw',
      description:
        typeof schema?.description === 'string'
          ? schema.description
          : undefined,
      defaultValue: schema?.default,
    }
  );
}

function schemaDefault(schema: ResolvedSchema): unknown {
  if (schema.defaultValue !== undefined) {
    // Config values are JSON, so a round trip is enough to detach the default
    // from the registry.
    return JSON.parse(JSON.stringify(schema.defaultValue));
  }
  if (schema.type !== 'object') {
    return undefined;
  }
  const value: JsonConfigObject = {};
  for (const [name, property] of Object.entries(schema.properties)) {
    const propertyDefault = schemaDefault(property);
    if (propertyDefault !== undefined) {
      value[name] = propertyDefault;
    }
  }
  return Object.keys(value).length > 0 ? value : undefined;
}

/**
 * The config a newly created node starts with: schema defaults, plus the
 * required keys that have none, so the node is already the shape the form would
 * give it.
 */
export function getSchemaConfigDefaults(
  schema: unknown,
  definitions: Record<string, unknown>,
): unknown {
  const resolvedSchema = resolveSchema(schema, definitions);
  return completeRequired(resolvedSchema, schemaDefault(resolvedSchema));
}

/** The value written when an optional property is switched on. */
function initialValue(schema: ResolvedSchema): unknown {
  if (schema.defaultValue !== undefined) {
    return JSON.parse(JSON.stringify(schema.defaultValue));
  }
  switch (schema.type) {
    case 'raw':
      return null;
    case 'string':
      return schema.enumValues?.[0] ?? '';
    case 'number':
    case 'integer':
      return schema.enumValues?.[0] ?? 0;
    case 'boolean':
      return false;
    case 'object': {
      // Seed required children too, otherwise switching on an object writes a
      // `{}` the schema rejects.
      const value: JsonConfigObject = {};
      for (const [name, property] of Object.entries(schema.properties)) {
        if (schema.required.has(name)) {
          value[name] = initialValue(property);
        } else {
          const propertyDefault = schemaDefault(property);
          if (propertyDefault !== undefined) {
            value[name] = propertyDefault;
          }
        }
      }
      return value;
    }
  }
}

/**
 * Fills in every required key the config is missing, so that a field the form
 * renders always corresponds to a key that exists. Returns `value` itself when
 * there was nothing to add, which is what stops {@link SchemaConfigForm} from
 * reporting a change on every render.
 *
 * Only absent keys are filled: a key that is present but holds the wrong shape
 * is the user's data and is left for them to see and correct.
 */
function completeRequired(schema: ResolvedSchema, value: unknown): unknown {
  if (schema.type !== 'object') {
    return value;
  }
  const object = schemaObject(value);
  if (!object) {
    // Nothing to complete unless the schema insists on keys; a config-less node
    // whose fields are all optional stays config-less.
    return schema.required.size > 0 ? initialValue(schema) : value;
  }

  let completed = object;
  const fill = (name: string, propertyValue: unknown) => {
    if (completed === object) {
      completed = { ...object };
    }
    completed[name] = propertyValue;
  };

  for (const [name, property] of Object.entries(schema.properties)) {
    if (!hasOwn(object, name)) {
      if (schema.required.has(name)) {
        fill(name, initialValue(property));
      }
      continue;
    }
    const child = completeRequired(property, object[name]);
    if (child !== object[name]) {
      fill(name, child);
    }
  }
  return completed;
}

function formatJson(value: unknown): string {
  return value === undefined ? '' : JSON.stringify(value);
}

function parseJson(draft: string): { value: unknown } | null {
  if (draft.trim() === '') {
    return { value: undefined };
  }
  try {
    return { value: JSON.parse(draft) };
  } catch {
    return null;
  }
}

function formatNumber(value: unknown): string {
  return typeof value === 'number' ? String(value) : '';
}

function parseNumber(
  draft: string,
  type: ResolvedNumberSchema['type'],
): number | undefined {
  if (draft.trim() === '') {
    return undefined;
  }
  const parsed = Number(draft);
  if (
    !Number.isFinite(parsed) ||
    (type === 'integer' && !Number.isInteger(parsed))
  ) {
    return undefined;
  }
  return parsed;
}

interface FieldProps {
  label: string;
  schema: ResolvedSchema;
  value: unknown;
  required: boolean;
  disabled: boolean;
  onChange: (value: unknown) => void;
}

function RawField({
  label,
  schema,
  value,
  required,
  disabled,
  onChange,
}: FieldProps) {
  const [draft, setDraft] = useState(() => formatJson(value));
  // Only adopt an incoming value that the box does not already say. Re-encoding
  // on every keystroke would strip the user's whitespace and move the cursor.
  useEffect(() => {
    setDraft((current) => {
      const parsed = parseJson(current);
      return parsed && equal(parsed.value, value) ? current : formatJson(value);
    });
  }, [value]);

  return (
    <TextField
      multiline
      rows={4}
      label={label}
      required={required}
      disabled={disabled}
      value={draft}
      error={parseJson(draft) === null}
      helperText={schema.description}
      onChange={(event) => {
        setDraft(event.target.value);
        const parsed = parseJson(event.target.value);
        if (parsed) {
          onChange(parsed.value);
        }
      }}
      slotProps={{
        htmlInput: {
          sx: { fontFamily: 'monospace', whiteSpace: 'nowrap' },
        },
      }}
    />
  );
}

function NumberField({
  label,
  schema,
  value,
  required,
  disabled,
  onChange,
}: FieldProps & { schema: ResolvedNumberSchema }) {
  const [draft, setDraft] = useState(() => formatNumber(value));
  // As in RawField: keep a draft that already means `value`, so a trailing `.`
  // or `0` survives the round trip through the config.
  useEffect(() => {
    setDraft((current) =>
      parseNumber(current, schema.type) === value
        ? current
        : formatNumber(value),
    );
  }, [value, schema.type]);

  return (
    <TextField
      type="number"
      label={label}
      required={required}
      disabled={disabled}
      value={draft}
      // Anything the draft says that the config does not hold: unparseable
      // input, or a field cleared without a value to replace it.
      error={parseNumber(draft, schema.type) !== value}
      helperText={schema.description}
      slotProps={{
        htmlInput: {
          min: schema.minimum,
          max: schema.maximum,
          step: schema.type === 'integer' ? 1 : 'any',
        },
      }}
      onChange={(event) => {
        setDraft(event.target.value);
        const parsed = parseNumber(event.target.value, schema.type);
        if (parsed !== undefined) {
          onChange(parsed);
        }
      }}
    />
  );
}

function SchemaField(props: FieldProps) {
  const { label, schema, value, required, disabled, onChange } = props;

  if (schema.type === 'raw') {
    return <RawField {...props} />;
  }

  if (schema.type === 'object') {
    return (
      <Box
        component="fieldset"
        disabled={disabled}
        sx={{ m: 0, p: 2, border: 1, borderColor: 'divider', borderRadius: 1 }}
      >
        <Typography component="legend" variant="subtitle2">
          {label}
        </Typography>
        {schema.description && (
          <Typography color="text.secondary" variant="body2" sx={{ mb: 1 }}>
            {schema.description}
          </Typography>
        )}
        <ObjectFields
          schema={schema}
          value={schemaObject(value) ?? {}}
          onChange={onChange}
        />
      </Box>
    );
  }

  if (schema.type === 'boolean') {
    return (
      <Box>
        <FormControlLabel
          disabled={disabled}
          control={
            <Checkbox
              checked={value === true}
              onChange={(event) => onChange(event.target.checked)}
            />
          }
          label={label}
        />
        {schema.description && (
          <FormHelperText>{schema.description}</FormHelperText>
        )}
      </Box>
    );
  }

  if (schema.enumValues) {
    // Selected by index so that numeric and string options round trip alike.
    const selectedIndex = schema.enumValues.findIndex(
      (option) => option === value,
    );
    return (
      <TextField
        select
        label={label}
        required={required}
        disabled={disabled}
        value={selectedIndex < 0 ? '' : String(selectedIndex)}
        helperText={schema.description}
        onChange={(event) => {
          const selected = schema.enumValues?.[Number(event.target.value)];
          if (selected !== undefined) {
            onChange(selected);
          }
        }}
      >
        {schema.enumValues.map((option, index) => (
          <MenuItem key={JSON.stringify(option)} value={String(index)}>
            {String(option)}
          </MenuItem>
        ))}
      </TextField>
    );
  }

  if (schema.type === 'string') {
    return (
      <TextField
        label={label}
        required={required}
        disabled={disabled}
        value={typeof value === 'string' ? value : ''}
        helperText={schema.description}
        onChange={(event) => onChange(event.target.value)}
      />
    );
  }

  return <NumberField {...props} schema={schema} />;
}

interface ObjectFieldsProps {
  schema: ResolvedObjectSchema;
  value: JsonConfigObject;
  onChange: (value: JsonConfigObject) => void;
}

function ObjectFields({ schema, value, onChange }: ObjectFieldsProps) {
  return (
    <Stack spacing={2}>
      {Object.entries(schema.properties).map(([name, propertySchema]) => {
        const required = schema.required.has(name);
        const present = hasOwn(value, name);
        const setProperty = (propertyValue: unknown) => {
          const nextValue = { ...value };
          if (propertyValue === undefined) {
            delete nextValue[name];
          } else {
            nextValue[name] = propertyValue;
          }
          onChange(nextValue);
        };
        return (
          <Stack key={name} spacing={0.5}>
            {!required && (
              <FormControlLabel
                control={
                  <Checkbox
                    checked={present}
                    onChange={(event) =>
                      setProperty(
                        event.target.checked
                          ? initialValue(propertySchema)
                          : undefined,
                      )
                    }
                  />
                }
                label={`Set ${name}`}
              />
            )}
            <SchemaField
              label={name}
              schema={propertySchema}
              value={value[name]}
              required={required}
              disabled={!required && !present}
              onChange={setProperty}
            />
          </Stack>
        );
      })}
    </Stack>
  );
}

export default function SchemaConfigForm({
  schema,
  definitions,
  value,
  onChange,
}: SchemaConfigFormProps) {
  const resolvedSchema = useMemo(
    () => resolveSchema(schema, definitions),
    [schema, definitions],
  );

  // The form defines the shape of the config, so every field it renders has a
  // key behind it whether or not the user has touched that field. Editing is
  // done against the completed config, and the parent is told about the keys
  // that were filled in.
  const completed = useMemo(
    () => completeRequired(resolvedSchema, value),
    [resolvedSchema, value],
  );
  useEffect(() => {
    if (completed !== value) {
      onChange(completed);
    }
  }, [completed, value, onChange]);

  // The top level is the form itself, so its fields go straight into the
  // surrounding layout rather than into a labelled group.
  if (resolvedSchema.type === 'object') {
    return (
      <ObjectFields
        schema={resolvedSchema}
        value={schemaObject(completed) ?? {}}
        onChange={onChange}
      />
    );
  }

  return (
    <SchemaField
      label="Config"
      schema={resolvedSchema}
      value={completed}
      required={false}
      disabled={false}
      onChange={onChange}
    />
  );
}
