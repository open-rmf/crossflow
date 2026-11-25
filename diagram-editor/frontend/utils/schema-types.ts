import {
  type FieldTemplateProps,
  findSchemaDefinition,
  guessType,
  type RJSFSchema,
} from '@rjsf/utils';

type Schema = RJSFSchema | boolean;

function resolve(schema: Schema, root: RJSFSchema) {
  if (typeof schema === 'boolean') return undefined;
  try {
    return schema.$ref ? findSchemaDefinition(schema.$ref, root) : schema;
  } catch {
    return undefined;
  }
}

function choices(schema: RJSFSchema) {
  return 'const' in schema ? [schema.const] : schema.enum;
}

export function schemaDocument(
  schema: object | boolean | null | undefined,
  definitions?: Record<string, unknown>,
) {
  return {
    schemas: definitions,
    $defs: definitions,
    definitions,
    ...(typeof schema === 'object' && schema),
  } as RJSFSchema;
}

// An unknown branch may accept additional types, so don't infer a mismatch from it.
export function schemaTypes(
  schema: Schema,
  root: RJSFSchema,
  refs: string[] = [],
): string[] {
  if (typeof schema === 'boolean') return [];
  if (schema.$ref) {
    if (refs.includes(schema.$ref)) return [];
    const resolved = resolve(schema, root);
    return resolved ? schemaTypes(resolved, root, [...refs, schema.$ref]) : [];
  }
  const values = choices(schema);
  if (values) {
    return [
      ...new Set(
        values.map((v) => (Number.isInteger(v) ? 'integer' : guessType(v))),
      ),
    ];
  }
  if (schema.type)
    return Array.isArray(schema.type) ? schema.type : [schema.type];
  const branches = (schema.anyOf ?? schema.oneOf)?.map((s) =>
    schemaTypes(s, root, refs),
  );
  return branches?.every((types) => types.length)
    ? [...new Set(branches.flat())]
    : [];
}

function numericWarning(schema: RJSFSchema, dest: RJSFSchema, types: string[]) {
  // Other source constraints may narrow the range beyond its declared bounds.
  if (
    !schema.type ||
    !types.some((t) => t === 'number' || t === 'integer') ||
    (['multipleOf', 'anyOf', 'oneOf', 'allOf', 'not', 'if'] as const).some(
      (key) => schema[key],
    )
  )
    return;
  const integer = !types.includes('number');
  const lower = schema.exclusiveMinimum ?? -Infinity,
    upper = schema.exclusiveMaximum ?? Infinity;
  const min = integer
    ? Math.max(Math.ceil(schema.minimum ?? -Infinity), Math.floor(lower) + 1)
    : (schema.minimum ?? -Infinity);
  const max = integer
    ? Math.min(Math.floor(schema.maximum ?? Infinity), Math.ceil(upper) - 1)
    : (schema.maximum ?? Infinity);
  if (dest.minimum !== undefined && min < dest.minimum && lower < dest.minimum)
    return `Minimum ${dest.minimum} expected`;
  if (dest.maximum !== undefined && max > dest.maximum && upper > dest.maximum)
    return `Maximum ${dest.maximum} expected`;
  if (
    dest.exclusiveMinimum !== undefined &&
    min <= dest.exclusiveMinimum &&
    lower < dest.exclusiveMinimum
  )
    return `Greater than ${dest.exclusiveMinimum} expected`;
  if (
    dest.exclusiveMaximum !== undefined &&
    max >= dest.exclusiveMaximum &&
    upper > dest.exclusiveMaximum
  )
    return `Less than ${dest.exclusiveMaximum} expected`;
}

export function bindingProperties(
  root: RJSFSchema,
  field?: FieldTemplateProps,
) {
  const target = field && resolve(field.schema, field.registry.rootSchema);
  const expected = field
    ? schemaTypes(field.schema, field.registry.rootSchema)
    : [];
  const accepted = new Set(expected);
  if (accepted.has('number')) accepted.add('integer');

  function warningFor(schema: Schema, types: string[]) {
    if (!field) return;
    const source = resolve(schema, root);
    if (!source) return;
    const values = choices(source);
    if (values) {
      const validator = field.registry.schemaUtils.getValidator();
      for (const value of values) {
        if (
          validator.isValid(source, value, root) &&
          !validator.isValid(field.schema, value, field.registry.rootSchema)
        )
          return `${JSON.stringify(value)} is outside the expected values`;
      }
      return;
    }
    if (accepted.size && types.some((type) => !accepted.has(type)))
      return `${expected.join(' | ')} expected`;
    if (target) return numericWarning(source, target, types);
  }

  return Object.entries(root.properties ?? {}).map(([name, schema]) => {
    const types = schemaTypes(schema, root);
    const ref = typeof schema === 'object' ? schema.$ref : undefined;
    const type =
      ref?.split('/').at(-1) ||
      types.join(' | ') ||
      (schema === false
        ? 'never'
        : schema === true || !Object.keys(schema).length
          ? 'any'
          : 'unknown');
    return { name, type, warning: warningFor(schema, types) };
  });
}
