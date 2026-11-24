import {
  Box,
  IconButton,
  Menu,
  MenuItem,
  Stack,
  TextField,
  Tooltip,
} from '@mui/material';
import { getDefaultRegistry } from '@rjsf/core';
import Form, { Templates } from '@rjsf/mui';
import {
  type FieldProps,
  type FieldTemplateProps,
  getDefaultFormState,
  getSchemaType,
  isObject,
  type MultiSchemaFieldTemplateProps,
  type ObjectFieldTemplateProps,
  RJSF_REF_CYCLE_KEY,
  type RJSFSchema,
  retrieveSchema,
  type UiSchema,
  useDeepCompareMemo,
} from '@rjsf/utils';
import { customizeValidator } from '@rjsf/validator-ajv8';
import Ajv2020 from 'ajv/dist/2020';
import equal from 'fast-deep-equal';
import {
  type ComponentType,
  useEffect,
  useId,
  useMemo,
  useRef,
  useState,
} from 'react';
import { MaterialSymbol } from '../nodes';

const validator = customizeValidator({
  AjvClass: Ajv2020,
  ajvOptionsOverrides: { strict: false, validateFormats: false },
  ajvFormatOptions: false,
});

const MuiObjectField =
  Templates.ObjectFieldTemplate as ComponentType<ObjectFieldTemplateProps>;

function ObjectLayout(props: ObjectFieldTemplateProps) {
  const rjsfSlotProps = {
    titleBox: { sx: { m: 0 } },
    titleTypography: { variant: 'subtitle2' },
    objectGridItem: { sx: { mb: 0 } },
    objectGridContainer: {
      sx: { mt: props.title || props.description ? 2 : 0 },
    },
  };
  return (
    <MuiObjectField
      {...props}
      uiSchema={{
        ...props.uiSchema,
        'ui:options': {
          ...props.uiSchema?.['ui:options'],
          mui: { rjsfSlotProps },
        },
      }}
    />
  );
}

function AlternativesLayout({
  selector,
  optionSchemaField,
}: MultiSchemaFieldTemplateProps) {
  return (
    <Stack spacing={2}>
      {selector}
      {optionSchemaField}
    </Stack>
  );
}

const editingDefaults = {
  emptyObjectFields: 'skipDefaults',
  requiredBooleanDefault: 'skip',
  arrayMinItems: { populate: 'never' },
  constAsDefaults: 'skipOneOf',
} as const;

export interface SchemaConfigFormProps {
  schema: unknown;
  definitions?: Record<string, unknown>;
  value?: unknown;
  onChange: (value: unknown) => void;
  label?: string;
  disabled?: boolean;
  nodeDefaults?: boolean;
  allowJson?: boolean;
  uiSchema?: UiSchema;
  fieldTemplate?: ComponentType<FieldTemplateProps>;
  rawDraft?: string;
  onRawDraftChange?: (value: string | undefined) => void;
}

// Registry references point outside the operation's schema. Give RJSF the same
// document root it would have had when resolving the schema in the registry.
function schemaRoot(schema: unknown, definitions?: Record<string, unknown>) {
  if (!schema || typeof schema !== 'object' || !Object.keys(schema).length) {
    return undefined;
  }
  const root = {
    schemas: definitions,
    $defs: definitions,
    definitions,
    ...schema,
  } as RJSFSchema;
  // Compilation checks every reachable branch, including a map's value type
  // before its first key exists. Unresolvable imported schemas stay editable.
  return validator.rawValidation(root, undefined).validationError
    ? undefined
    : root;
}

// Existing node builders expect their required fields to exist even before the
// first edit. Section instances and schema authors edit their actual input.
function fillNodeFields(
  schema: RJSFSchema,
  root: RJSFSchema,
  value: unknown,
  required = false,
): unknown {
  if (RJSF_REF_CYCLE_KEY in schema) return value ?? null;
  if (required && value === undefined)
    value = getDefaultFormState(validator, schema, undefined, root);
  const resolved = retrieveSchema(validator, schema, root, value);
  const type = getSchemaType(resolved);
  if (type !== 'object') {
    if (value !== undefined || !required) return value;
    if ('default' in resolved) return resolved.default;
    if (
      (Array.isArray(resolved.type) && resolved.type.includes('null')) ||
      resolved.anyOf?.some((s) => isObject(s) && s.type === 'null')
    )
      return null;
    if (resolved.enum?.length) return resolved.enum[0];
    return type === 'string'
      ? ''
      : type === 'boolean'
        ? false
        : type === 'integer' || type === 'number'
          ? 0
          : null;
  }
  if (value !== undefined && !isObject(value)) return value;
  if (value === undefined && !required && !resolved.required?.length)
    return undefined;
  const object = { ...(value as Record<string, unknown>) };
  for (const [key, child] of Object.entries(resolved.properties ?? {})) {
    const needed = resolved.required?.includes(key) ?? false;
    if (needed || key in object) {
      const entry = fillNodeFields(
        typeof child === 'boolean' ? {} : child,
        root,
        object[key],
        needed,
      );
      if (entry !== undefined) object[key] = entry;
    }
  }
  return equal(object, value) ? value : object;
}

export function getSchemaConfigDefaults(
  schema: unknown,
  definitions: Record<string, unknown>,
): unknown {
  const root = schemaRoot(schema, definitions);
  if (!root) return undefined;
  const defaults = getDefaultFormState(validator, root, undefined, root);
  return fillNodeFields(root, root, equal(defaults, {}) ? undefined : defaults);
}

function parseJson(text: string) {
  return text.trim() === '' ? undefined : JSON.parse(text);
}

function RawJsonField({
  label = 'Config',
  value,
  onChange,
  rawDraft,
  onRawDraftChange,
  disabled,
}: Omit<SchemaConfigFormProps, 'schema'>) {
  const [draft, setDraft] = useState(
    () => rawDraft ?? JSON.stringify(value) ?? '',
  );
  const [invalid, setInvalid] = useState(rawDraft !== undefined);
  useEffect(() => {
    if (rawDraft !== undefined) {
      setDraft(rawDraft);
      setInvalid(true);
      return;
    }
    setInvalid(false);
    setDraft((current) => {
      try {
        if (equal(parseJson(current), value)) return current;
      } catch {}
      return JSON.stringify(value) ?? '';
    });
  }, [value, rawDraft]);

  return (
    <TextField
      fullWidth
      multiline
      rows={4}
      label={label}
      value={draft}
      disabled={disabled}
      error={invalid}
      slotProps={{ htmlInput: { sx: { fontFamily: 'monospace' } } }}
      onChange={(event) => {
        const text = event.target.value;
        setDraft(text);
        try {
          const parsed = parseJson(text);
          setInvalid(false);
          onChange(parsed);
          onRawDraftChange?.(undefined);
        } catch {
          setInvalid(true);
          onRawDraftChange?.(text);
        }
      }}
    />
  );
}

const nativeFields = getDefaultRegistry().fields;

// RJSF's NullField silently sets missing properties to null on mount.
function NullPropertyField(props: FieldProps) {
  if (typeof props.fieldPathId.path.at(-1) !== 'string')
    return <nativeFields.NullField {...props} />;
  return (
    <nativeFields.StringField
      {...props}
      schema={{
        title: props.schema.title,
        enum: [null],
        ...(props.required && { default: null }),
      }}
      uiSchema={{ ...props.uiSchema, 'ui:placeholder': 'Unset' }}
    />
  );
}

function EnumField(props: FieldProps) {
  const { enum: choices = [], title, default: defaultValue } = props.schema;
  const nullable =
    choices.includes(null) &&
    validator.isValid(props.schema, null, props.registry.rootSchema);
  const values = nullable ? choices : choices.filter((v) => v !== null);
  const canUnset = nullable && !props.required;
  const indexes = values.map((_, i) => String(i));
  const indexOf = (value: unknown) =>
    indexes[values.findIndex((v) => equal(v, value))];
  const strings = values.every((v) => typeof v === 'string');
  // RJSF's single-select treats an array value as multiple selections and an
  // empty string as an unset value. Give the widget indexes into the literals.
  return (
    <nativeFields.StringField
      {...props}
      schema={{
        type: 'string',
        title,
        enum: indexes,
        default: canUnset ? undefined : indexOf(defaultValue),
      }}
      uiSchema={{
        ...(canUnset && { 'ui:placeholder': 'Unset' }),
        'ui:enumNames': values.map((v) =>
          strings && v ? String(v) : JSON.stringify(v),
        ),
        ...props.uiSchema,
        'ui:field': undefined,
      }}
      formData={indexOf(props.formData)}
      onChange={(index, ...rest) =>
        props.onChange(values[Number(index)], ...rest)
      }
    />
  );
}

function FieldContents({ children }: FieldTemplateProps) {
  return children;
}

function NullableField(props: FieldProps) {
  const { schema, formData, fieldPathId, registry } = props;
  const container = useRef<HTMLDivElement>(null);
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const label = props.uiSchema?.['ui:title'] ?? schema.title ?? props.name;
  const type = getSchemaType(schema) as RJSFSchema['type'];
  const concrete = { ...schema, type };
  if (concrete.default === null) delete concrete.default;
  const set = (value: unknown) => {
    setAnchor(null);
    props.onChange(value, fieldPathId.path);
  };
  return (
    <Stack ref={container} direction="row" spacing={1} alignItems="flex-start">
      <Box sx={{ flex: 1, minWidth: 0 }}>
        {formData === null ? (
          <TextField
            fullWidth
            label={label}
            required={props.required}
            disabled={props.disabled}
            value="null"
            slotProps={{ input: { readOnly: true } }}
          />
        ) : (
          <nativeFields.SchemaField
            {...props}
            schema={concrete}
            uiSchema={{
              ...props.uiSchema,
              'ui:field': undefined,
              'ui:FieldTemplate': FieldContents,
            }}
          />
        )}
      </Box>
      <Tooltip title={`${label} value options`}>
        <span>
          <IconButton
            aria-label={`${label} value options`}
            aria-haspopup="menu"
            disabled={props.disabled || props.readonly}
            onClick={(e) => setAnchor(e.currentTarget)}
            sx={{ mt: 1 }}
          >
            <MaterialSymbol symbol="more_vert" />
          </IconButton>
        </span>
      </Tooltip>
      <Menu
        container={() => container.current}
        disableScrollLock
        anchorEl={anchor}
        open={Boolean(anchor)}
        onClose={() => setAnchor(null)}
      >
        {formData === null ? (
          <MenuItem
            onClick={() =>
              set(
                fillNodeFields(concrete, registry.rootSchema, undefined, true),
              )
            }
          >
            Edit value
          </MenuItem>
        ) : (
          <MenuItem onClick={() => set(null)}>Set to null</MenuItem>
        )}
        {!props.required && typeof fieldPathId.path.at(-1) !== 'number' && (
          <MenuItem
            disabled={formData === undefined}
            onClick={() => set(undefined)}
          >
            Unset
          </MenuItem>
        )}
      </Menu>
    </Stack>
  );
}

function SchemaField(props: FieldProps) {
  const schema = props.schema.$ref
    ? props.registry.schemaUtils.retrieveSchema(props.schema, props.formData)
    : props.schema;
  const nullable =
    Array.isArray(schema.type) &&
    schema.type.length === 2 &&
    schema.type.includes('null') &&
    !schema.enum &&
    !schema.anyOf &&
    !schema.oneOf &&
    validator.isValid(props.schema, null, props.registry.rootSchema);
  const field = nullable
    ? NullableField
    : schema.enum?.some((v) => typeof v !== 'string' || v === '')
      ? EnumField
      : undefined;
  return (
    <nativeFields.SchemaField
      {...props}
      uiSchema={
        field ? { 'ui:field': field, ...props.uiSchema } : props.uiSchema
      }
    />
  );
}

function AlternativesField(props: FieldProps) {
  // RJSF otherwise reports undefined when the author selects a null branch.
  const options = (props.options as RJSFSchema[]).map((option) => ({
    ...option,
    ...(option.type === 'null' && { default: null }),
    title:
      option.title ??
      (typeof option.type === 'string' ? option.type : undefined),
  }));
  return <nativeFields.AnyOfField {...props} options={options} />;
}

// A node keeps its last usable number while an integer or required number is
// incomplete. RJSF still controls the numeric widget and its formatting.
function NodeNumberField(props: FieldProps) {
  const [draft, setDraft] = useState<{ input: unknown; previous: unknown }>();
  const invalid = draft !== undefined && draft.previous === props.formData;
  return (
    <nativeFields.NumberField
      {...props}
      formData={invalid ? draft.input : props.formData}
      rawErrors={invalid ? [''] : props.rawErrors}
      onChange={(input, ...rest) => {
        if (
          (input === undefined &&
            (props.required || !props.fieldPathId.path.length)) ||
          (input !== undefined &&
            getSchemaType(props.schema) === 'integer' &&
            !Number.isInteger(input))
        ) {
          setDraft({ input, previous: props.formData });
          return;
        }
        setDraft(undefined);
        props.onChange(input, ...rest);
      }}
    />
  );
}

export default function SchemaConfigForm(props: SchemaConfigFormProps) {
  const id = useId();
  const [editJson, setEditJson] = useState(props.rawDraft !== undefined);
  const schema = useDeepCompareMemo(props.schema);
  const root = useMemo(
    () => schemaRoot(schema, props.definitions),
    [schema, props.definitions],
  );
  const value = useMemo(
    () =>
      root && props.nodeDefaults
        ? fillNodeFields(root, root, props.value)
        : props.value,
    [root, props.nodeDefaults, props.value],
  );
  useEffect(() => {
    if (value !== props.value) props.onChange(value);
  }, [value, props.value, props.onChange]);
  if (!root) return <RawJsonField {...props} />;

  const raw = editJson || props.rawDraft !== undefined;
  const title = props.label ?? 'Config';
  const editor = (
    <Form
      tagName="div"
      idPrefix={id}
      schema={root}
      uiSchema={{
        'ui:title': title,
        'ui:submitButtonOptions': { norender: true },
        ...props.uiSchema,
      }}
      fields={{
        SchemaField,
        AnyOfField: AlternativesField,
        OneOfField: AlternativesField,
        NullField: NullPropertyField,
        ...(props.nodeDefaults && { NumberField: NodeNumberField }),
      }}
      formData={value}
      disabled={props.disabled}
      validator={validator}
      templates={{
        ObjectFieldTemplate: ObjectLayout,
        MultiSchemaFieldTemplate: AlternativesLayout,
        ...(props.fieldTemplate && { FieldTemplate: props.fieldTemplate }),
      }}
      experimental_defaultFormStateBehavior={editingDefaults}
      liveValidate="onChange"
      showErrorList={false}
      noHtml5Validate
      onChange={({ formData }, fieldId) => {
        // RJSF also reports suggested defaults when it mounts. Only user edits
        // have a field id. Opening a panel must not change a saved diagram.
        if (fieldId) props.onChange(formData);
      }}
    />
  );
  if (!props.allowJson) return editor;
  return (
    <Stack spacing={2}>
      {raw ? <RawJsonField {...props} /> : editor}
      <Stack direction="row" justifyContent="flex-end">
        <Tooltip title={raw ? 'Edit as form' : 'Edit as JSON'}>
          <span>
            <IconButton
              size="small"
              aria-label={raw ? 'Edit as form' : 'Edit as JSON'}
              disabled={props.rawDraft !== undefined}
              onClick={() => setEditJson(!raw)}
            >
              <MaterialSymbol symbol={raw ? 'list' : 'code'} />
            </IconButton>
          </span>
        </Tooltip>
      </Stack>
    </Stack>
  );
}
