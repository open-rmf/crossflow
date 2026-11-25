import {
  Autocomplete,
  Button,
  MenuItem,
  Stack,
  Typography,
} from '@mui/material';
import { getDefaultRegistry } from '@rjsf/core';
import Form, { Templates } from '@rjsf/mui';
import {
  type FieldProps,
  type FieldTemplateProps,
  getDefaultFormState,
  getSchemaType,
  guessType,
  isObject,
  type MultiSchemaFieldTemplateProps,
  type ObjectFieldTemplateProps,
  optionsList,
  RJSF_REF_CYCLE_KEY,
  RJSF_REF_KEY,
  type RJSFMarkedSchema,
  type RJSFSchema,
  retrieveSchema,
  setByPath,
  toFieldPathId,
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
import { schemaDocument, schemaTypes } from '../utils/schema-types';
import {
  ActionTextField,
  CheckboxWithActions,
  FieldActionContext,
  FieldActions,
  FieldMenu,
  FieldRow,
  InputWithActions,
  SelectWithActions,
  TitleWithActions,
} from './field-actions';

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
    titleGridContainer: { alignItems: 'center' },
    objectGridItem: { sx: { mb: 0 } },
    objectGridContainer: {
      sx: { mt: props.title || props.description ? 2 : 0 },
    },
  };
  return (
    <MuiObjectField
      {...props}
      uiSchema={{ ...props.uiSchema, 'ui:mui': { rjsfSlotProps } }}
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
      <FieldActionContext.Provider value={null}>
        {optionSchemaField}
      </FieldActionContext.Provider>
    </Stack>
  );
}

const editingDefaults = {
  emptyObjectFields: 'skipDefaults',
  requiredBooleanDefault: 'skip',
  arrayMinItems: { populate: 'never' },
  constAsDefaults: 'skipOneOf',
} as const;

const emptyScalars: Record<string, unknown> = {
  string: '',
  integer: 0,
  number: 0,
  boolean: false,
};

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
  const root = schemaDocument(schema, definitions);
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
    return typeof type === 'string' ? (emptyScalars[type] ?? null) : null;
  }
  if (value !== undefined && !isObject(value)) return value;
  if (value === undefined && !required && !resolved.required?.length)
    return undefined;
  const object = { ...(value as Record<string, unknown>) };
  for (const [key, child] of Object.entries(resolved.properties ?? {})) {
    const needed = resolved.required?.includes(key) ?? false;
    if (needed || key in object) {
      object[key] = fillNodeFields(
        typeof child === 'boolean' ? {} : child,
        root,
        object[key],
        needed,
      );
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

function RawField({
  label = 'Config',
  value,
  onChange,
  rawDraft: draftValue,
  onRawDraftChange: onDraftChange,
  disabled,
}: Omit<SchemaConfigFormProps, 'schema'>) {
  const [draft, setDraft] = useState(() => draftValue ?? formatJson(value));
  // Only adopt an incoming value that the box does not already say. Re-encoding
  // on every keystroke would strip the user's whitespace and move the cursor.
  useEffect(() => {
    setDraft((current) => {
      if (draftValue !== undefined) {
        return draftValue;
      }
      const parsed = parseJson(current);
      return parsed && equal(parsed.value, value) ? current : formatJson(value);
    });
  }, [draftValue, value]);

  return (
    <ActionTextField
      multiline
      rows={4}
      label={label}
      disabled={disabled}
      value={draft}
      error={parseJson(draft) === null}
      onChange={(event) => {
        setDraft(event.target.value);
        const parsed = parseJson(event.target.value);
        if (parsed) {
          onChange(parsed.value);
          onDraftChange?.(undefined);
        } else {
          onDraftChange?.(event.target.value);
        }
      }}
      slotProps={{
        htmlInput: { sx: { fontFamily: 'monospace' } },
      }}
    />
  );
}

const nativeFields = getDefaultRegistry().fields;

function fieldTitle({ name, schema, uiSchema }: FieldProps) {
  return uiSchema?.['ui:title'] ?? schema.title ?? name;
}

function acceptsNull(schema: RJSFSchema, registry: FieldProps['registry']) {
  return validator.isValid(schema, null, registry.rootSchema);
}

function EnumField(props: FieldProps) {
  const { schema, uiSchema, fieldPathId, formData } = props;
  const choices = optionsList(schema, uiSchema) ?? [
    { label: 'null', value: null, schema },
  ];
  const strings = choices.every(({ value }) => typeof value === 'string');
  const options = choices.filter(
    ({ value }) =>
      (!schema.oneOf && value !== null) ||
      validator.isValid(schema, value, props.registry.rootSchema),
  );
  return (
    <Autocomplete
      id={fieldPathId.$id}
      fullWidth
      options={options}
      getOptionKey={({ value }) => JSON.stringify(value)}
      getOptionLabel={({ label, value, schema: option }) =>
        option?.title !== undefined ||
        label !== String(value) ||
        (strings && value)
          ? label
          : JSON.stringify(value)
      }
      value={options.find(({ value }) => equal(value, formData)) ?? null}
      disableClearable={props.required}
      disabled={props.disabled}
      readOnly={props.readonly}
      onChange={(_, choice) => props.onChange(choice?.value, fieldPathId.path)}
      renderInput={(params) => (
        <ActionTextField
          {...params}
          label={fieldTitle(props)}
          required={props.required}
          placeholder={uiSchema?.['ui:placeholder']}
          error={Boolean(props.rawErrors?.length)}
        />
      )}
    />
  );
}

function FieldContents({ children }: FieldTemplateProps) {
  return children;
}

// The outer field already supplies the binding, label and validation errors.
function inlineUiSchema({ 'ui:field': _field, ...rest }: UiSchema = {}) {
  return { ...rest, 'ui:FieldTemplate': FieldContents };
}

function hasEditor(schema: RJSFSchema) {
  return getSchemaType(schema) || schema.oneOf || schema.anyOf;
}

// anyOf validates a single JSON value. Each editor writes to that same path,
// so overlapping choices can match together and object members can coexist.
function ParallelAnyOfField(props: FieldProps) {
  const { formData, fieldPathId, registry, schema } = props;
  const { SchemaField: Field } = registry.fields;
  const { anyOf = [], ...base } = schema;
  const title = fieldTitle(props);
  const current = useRef(formData);
  current.current = formData;
  const branches = anyOf
    .filter((s) => s !== false)
    .map((s) =>
      registry.schemaUtils.retrieveSchema(s === true ? {} : s, formData),
    );
  const rawEditor = (label: string, key?: string) => (
    <RawField
      key={key}
      label={label}
      value={formData}
      disabled={props.disabled || props.readonly}
      onChange={(value) => props.onChange(value, fieldPathId.path)}
    />
  );
  if (!branches.length) return rawEditor(title);
  const concrete = branches.filter(
    (s) => !equal(schemaTypes(s, registry.rootSchema), ['null']),
  );
  if (concrete.length < branches.length && acceptsNull(schema, registry)) {
    if (!concrete.length)
      return <EnumField {...props} schema={{ title, enum: [null] }} />;
    const nonNullSchema = {
      ...base,
      ...(concrete.length === 1 ? concrete[0] : { anyOf: concrete }),
    };
    // A registered nullable type may already provide its own null control.
    if (concrete.length === 1 && acceptsNull(concrete[0], registry)) {
      if (!hasEditor(nonNullSchema)) return rawEditor(title);
      return (
        <Field
          {...props}
          schema={nonNullSchema}
          uiSchema={inlineUiSchema(props.uiSchema)}
        />
      );
    }
    return <NullableField {...props} concrete={nonNullSchema} />;
  }
  return (
    <Stack spacing={2}>
      <FieldRow id={fieldPathId.$id}>
        <Typography variant="subtitle2">{title}</Typography>
      </FieldRow>
      <FieldActionContext.Provider value={null}>
        {base.properties && (
          <Field
            {...props}
            schema={{ ...base, title: '' }}
            uiSchema={{ 'ui:title': '' }}
            name=""
          />
        )}
        {branches.map((option, index) => {
          const optionId = `${registry.globalFormOptions.idPrefix}_anyOf${index}`;
          const type = getSchemaType(option) ?? getSchemaType(base);
          const ref = (option as RJSFMarkedSchema)[RJSF_REF_KEY];
          const name =
            option.title ??
            (typeof ref === 'string' ? ref.split('/').at(-1) : undefined) ??
            (typeof type === 'string' ? type : `Alternative ${index + 1}`);
          const values =
            option.enum ??
            (option.const === undefined ? undefined : [option.const]);
          if (base.properties && !hasEditor(option)) return null; // A required-only branch constrains the shared object above.
          if (!type && !values && !option.anyOf && !option.oneOf)
            return rawEditor(name, optionId);
          const matchesType =
            type === guessType(formData) ||
            (type === 'integer' && typeof formData === 'number');
          const globalFormOptions = {
            ...registry.globalFormOptions,
            idPrefix: optionId,
          };
          return (
            <Field
              {...props}
              key={optionId}
              name={name}
              required={false}
              schema={
                values ? { enum: values } : { type: base.type, ...option }
              }
              uiSchema={{ 'ui:title': name }}
              registry={{ ...registry, globalFormOptions }}
              fieldPathId={toFieldPathId('', globalFormOptions, fieldPathId)}
              formData={matchesType || values || !type ? formData : undefined}
              onChange={(value, path, errors, id) => {
                const child = path.slice(fieldPathId.path.length);
                if (
                  child.length &&
                  (type === 'object' || type === 'array') &&
                  type !== guessType(current.current)
                ) {
                  const container = setByPath(
                    type === 'array' ? [] : {},
                    child,
                    value,
                  );
                  props.onChange(container, fieldPathId.path, errors, id);
                } else {
                  props.onChange(value, path, errors, id);
                }
              }}
            />
          );
        })}
      </FieldActionContext.Provider>
    </Stack>
  );
}

function NullableField({
  concrete: alternative,
  ...props
}: FieldProps & { concrete?: RJSFSchema }) {
  const { schema, formData, fieldPathId, registry } = props;
  const label = fieldTitle(props);
  const type = getSchemaType(schema) as RJSFSchema['type'];
  const concrete = alternative ?? { ...schema, type };
  if (concrete.default === null) delete concrete.default;
  const set = (value: unknown) => props.onChange(value, fieldPathId.path);
  const canUnset =
    !props.required && typeof fieldPathId.path.at(-1) !== 'number';
  const Field = registry.fields.SchemaField;
  const action = formData === null ? 'Edit value' : 'Set to null';
  const first = (concrete.anyOf?.[0] as RJSFSchema) ?? concrete;
  const editValue = () =>
    fillNodeFields(first, registry.rootSchema, undefined, true);
  const menu = (
    <FieldMenu
      label={`${label} value options`}
      symbol="more_vert"
      title={canUnset ? `${action} / Unset` : action}
      disabled={props.disabled || props.readonly}
    >
      {() => [
        <MenuItem
          key="value"
          onClick={() => set(formData === null ? editValue() : null)}
        >
          {action}
        </MenuItem>,
        canUnset && (
          <MenuItem
            key="unset"
            disabled={formData === undefined}
            onClick={() => set(undefined)}
          >
            Unset
          </MenuItem>
        ),
      ]}
    </FieldMenu>
  );
  return (
    <FieldActions id={fieldPathId.$id} actions={menu}>
      {formData === null ? (
        <ActionTextField
          id={fieldPathId.$id}
          label={label}
          required={props.required}
          disabled={props.disabled}
          value="null"
          InputProps={{ readOnly: true }}
        />
      ) : (
        <Field
          {...props}
          schema={concrete}
          uiSchema={inlineUiSchema(props.uiSchema)}
        />
      )}
    </FieldActions>
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
    acceptsNull(props.schema, props.registry);
  // RJSF's NullField silently sets missing properties to null on mount.
  const enumField =
    ((schema.enum ||
      (schema.oneOf && props.registry.schemaUtils.isSelect(schema))) &&
      props.uiSchema?.['ui:widget'] !== 'hidden') ||
    (schema.type === 'null' &&
      typeof props.fieldPathId.path.at(-1) === 'string');
  const field = schema.anyOf
    ? ParallelAnyOfField
    : nullable
      ? NullableField
      : enumField
        ? EnumField
        : undefined;
  return (
    <nativeFields.SchemaField
      {...props}
      uiSchema={
        field
          ? {
              'ui:field': field,
              'ui:fieldReplacesAnyOrOneOf': true,
              ...props.uiSchema,
            }
          : props.uiSchema
      }
    />
  );
}

function OneOfField(props: FieldProps) {
  // RJSF otherwise reports undefined when the author selects a null branch.
  const options = (props.options as RJSFSchema[]).map((option) => ({
    ...option,
    ...(option.type === 'null' && { default: null }),
    title:
      option.title ??
      (typeof option.type === 'string' ? option.type : undefined),
  }));
  return <nativeFields.OneOfField {...props} options={options} />;
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
  if (!root) return <RawField {...props} />;

  const raw = editJson || props.rawDraft !== undefined;
  const editor = (
    <Form
      tagName="div"
      idPrefix={id}
      schema={root}
      uiSchema={{
        'ui:title': props.label ?? 'Config',
        'ui:submitButtonOptions': { norender: true },
        ...props.uiSchema,
      }}
      fields={{
        SchemaField,
        OneOfField,
        ...(props.nodeDefaults && { NumberField: NodeNumberField }),
      }}
      formData={value}
      disabled={props.disabled}
      validator={validator}
      widgets={{
        SelectWidget: SelectWithActions,
        CheckboxWidget: CheckboxWithActions,
      }}
      templates={{
        BaseInputTemplate: InputWithActions,
        TitleFieldTemplate: TitleWithActions,
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
      {raw ? <RawField {...props} /> : editor}
      <Button
        sx={{ alignSelf: 'flex-end' }}
        size="small"
        disabled={props.rawDraft !== undefined}
        onClick={() => setEditJson(!raw)}
      >
        {raw ? 'Edit form' : 'Edit JSON'}
      </Button>
    </Stack>
  );
}
