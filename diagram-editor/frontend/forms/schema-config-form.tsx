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
import { useEffect, useMemo, useState } from 'react';

export type JsonConfigObject = Record<string, unknown>;

type JsonSchema = Record<string, unknown>;
type EnumValue = string | number;

type ResolvedSchema = {
  description?: string;
  defaultValue?: unknown;
  hasDefault: boolean;
} & (
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

export interface SchemaConfigFormProps {
  schema: unknown;
  definitions: Record<string, unknown>;
  value?: JsonConfigObject;
  onChange: (value: JsonConfigObject) => void;
}

function schemaObject(value: unknown): JsonSchema | null {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? (value as JsonSchema)
    : null;
}

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

function schemaType(schema: JsonSchema): string | null {
  if (typeof schema.type === 'string') {
    return schema.type;
  }
  if (Array.isArray(schema.type)) {
    const nonNullTypes = schema.type.filter((type) => type !== 'null');
    if (
      schema.type.includes('null') &&
      nonNullTypes.length === 1 &&
      typeof nonNullTypes[0] === 'string'
    ) {
      return nonNullTypes[0];
    }
    return null;
  }
  if (
    Array.isArray(schema.enum) &&
    schema.enum.length > 0 &&
    schema.enum.every((value) => typeof value === 'string')
  ) {
    return 'string';
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
] as const;

function resolveSchema(
  rawSchema: unknown,
  definitions: Record<string, unknown>,
  resolvingRefs = new Set<string>(),
): ResolvedSchema | null {
  const schema = schemaObject(rawSchema);
  if (!schema) {
    return null;
  }

  if (typeof schema.$ref === 'string') {
    if (resolvingRefs.has(schema.$ref)) {
      return null;
    }
    const referenced = resolveRef(schema.$ref, definitions);
    if (!referenced) {
      return null;
    }
    const { $ref: _ref, ...overrides } = schema;
    return resolveSchema(
      { ...referenced, ...overrides },
      definitions,
      new Set(resolvingRefs).add(schema.$ref),
    );
  }

  if (Array.isArray(schema.anyOf)) {
    const nonNull = schema.anyOf.filter((option) => {
      const object = schemaObject(option);
      const types = Array.isArray(object?.type) ? object.type : [object?.type];
      return !types.includes('null');
    });
    const hasNull = schema.anyOf.some((option) => {
      const object = schemaObject(option);
      const types = Array.isArray(object?.type) ? object.type : [object?.type];
      return types.includes('null');
    });
    const unwrapped = schemaObject(nonNull[0]);
    if (!hasNull || nonNull.length !== 1 || !unwrapped) {
      return null;
    }
    const { anyOf: _anyOf, ...overrides } = schema;
    return resolveSchema(
      { ...unwrapped, ...overrides },
      definitions,
      resolvingRefs,
    );
  }

  if (unsupportedKeywords.some((key) => key in schema)) {
    return null;
  }

  const type = schemaType(schema);
  const description =
    typeof schema.description === 'string' ? schema.description : undefined;
  const hasDefault = hasOwn(schema, 'default');
  const base = {
    description,
    hasDefault,
    ...(hasDefault && { defaultValue: schema.default }),
  };

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

  if (type === 'boolean') {
    if (schema.enum !== undefined) {
      return null;
    }
    return { ...base, type };
  }

  if (type === 'object') {
    if (
      schema.additionalProperties !== undefined &&
      schema.additionalProperties !== false
    ) {
      return null;
    }
    const rawProperties = schemaObject(schema.properties ?? {});
    if (!rawProperties) {
      return null;
    }
    const properties: Record<string, ResolvedSchema> = {};
    for (const [name, propertySchema] of Object.entries(rawProperties)) {
      const property = resolveSchema(
        propertySchema,
        definitions,
        resolvingRefs,
      );
      if (!property) {
        return null;
      }
      properties[name] = property;
    }
    const requiredValues = schema.required ?? [];
    if (
      !Array.isArray(requiredValues) ||
      !requiredValues.every((value) => typeof value === 'string')
    ) {
      return null;
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

export function resolveSupportedConfigSchema(
  schema: unknown,
  definitions: Record<string, unknown>,
): ResolvedObjectSchema | null {
  const resolved = resolveSchema(schema, definitions);
  return resolved?.type === 'object' ? resolved : null;
}

function cloneDefault(value: unknown): unknown {
  return value === undefined ? undefined : JSON.parse(JSON.stringify(value));
}

function schemaDefault(schema: ResolvedSchema): unknown {
  if (schema.hasDefault) {
    return cloneDefault(schema.defaultValue);
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

export function getSchemaConfigDefaults(
  schema: unknown,
  definitions: Record<string, unknown>,
): JsonConfigObject | undefined {
  const resolvedSchema = resolveSupportedConfigSchema(schema, definitions);
  if (!resolvedSchema) {
    return undefined;
  }
  return schemaObject(schemaDefault(resolvedSchema)) ?? undefined;
}

function initialValue(schema: ResolvedSchema): unknown {
  const defaultValue = schemaDefault(schema);
  if (defaultValue !== undefined) {
    return defaultValue;
  }
  switch (schema.type) {
    case 'string':
      return schema.enumValues?.[0] ?? '';
    case 'number':
    case 'integer':
      return 0;
    case 'boolean':
      return false;
    case 'object':
      return {};
  }
}

interface FieldProps {
  label: string;
  schema: ResolvedSchema;
  value: unknown;
  required: boolean;
  disabled: boolean;
  onChange: (value: unknown) => void;
}

function NumberField({
  label,
  schema,
  value,
  required,
  disabled,
  onChange,
}: FieldProps) {
  const [draft, setDraft] = useState(
    typeof value === 'number' ? String(value) : '',
  );
  useEffect(() => {
    setDraft(typeof value === 'number' ? String(value) : '');
  }, [value]);

  if (schema.type !== 'number' && schema.type !== 'integer') {
    return null;
  }

  return (
    <TextField
      type="number"
      label={label}
      required={required}
      disabled={disabled}
      value={draft}
      helperText={schema.description}
      slotProps={{
        htmlInput: {
          min: schema.minimum,
          max: schema.maximum,
          step: schema.type === 'integer' ? 1 : 'any',
        },
      }}
      onChange={(event) => {
        const nextDraft = event.target.value;
        setDraft(nextDraft);
        if (nextDraft.trim() === '') {
          return;
        }
        const parsed = Number(nextDraft);
        if (
          !Number.isFinite(parsed) ||
          (schema.type === 'integer' && !Number.isInteger(parsed))
        ) {
          return;
        }
        onChange(parsed);
      }}
    />
  );
}

function SchemaField(props: FieldProps) {
  const { label, schema, value, required, disabled, onChange } = props;

  if (schema.type === 'object') {
    const objectValue = schemaObject(value) ?? {};
    return (
      <Box component="fieldset" disabled={disabled} sx={{ m: 0, p: 2 }}>
        <Typography component="legend" variant="subtitle2">
          {label}
        </Typography>
        {schema.description && (
          <Typography color="text.secondary" variant="body2" sx={{ mb: 1 }}>
            {schema.description}
          </Typography>
        )}
        <ObjectFields schema={schema} value={objectValue} onChange={onChange} />
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

  return <NumberField {...props} />;
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
          onChange({ ...value, [name]: propertyValue });
        };
        return (
          <Stack key={name} spacing={0.5}>
            {!required && (
              <FormControlLabel
                control={
                  <Checkbox
                    checked={present}
                    onChange={(event) => {
                      if (event.target.checked) {
                        setProperty(initialValue(propertySchema));
                      } else {
                        const nextValue = { ...value };
                        delete nextValue[name];
                        onChange(nextValue);
                      }
                    }}
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
    () => resolveSupportedConfigSchema(schema, definitions),
    [schema, definitions],
  );

  if (!resolvedSchema) {
    return null;
  }

  return (
    <ObjectFields
      schema={resolvedSchema}
      value={value ?? {}}
      onChange={onChange}
    />
  );
}
