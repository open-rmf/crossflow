import { execSync } from 'node:child_process';
import fs, { writeFileSync } from 'node:fs';
import { compile } from 'json-schema-to-typescript';

const MERGE_FIELDS = ['$ref', 'oneOf', 'allOf', 'anyOf'];

/** Keywords that describe a schema without constraining what it accepts. */
const ANNOTATION_FIELDS = [
  'description',
  'title',
  'default',
  'deprecated',
  'examples',
  'readOnly',
  'writeOnly',
];

/**
 * A schema carrying nothing but annotations accepts any JSON value, which is
 * how `serde_json::Value` fields are emitted. json-schema-to-typescript renders
 * that as `{ [k: string]: unknown }` unless the schema is completely empty, so
 * a field holding a number or null gets typed as an object. Pin it to `unknown`
 * instead.
 */
function markUnconstrained(schema) {
  if (Object.keys(schema).every((k) => ANNOTATION_FIELDS.includes(k))) {
    schema.tsType = 'unknown';
  }
}

/**
 * json schema draft-07 (used by json-schema-to-typescript) does not merge certain fields like `$ref` and `oneOf`.
 * Workaround by putting them in a `allOf`.
 */
function moveIntoAllOf(source) {
  let needMove = false;
  for (const k of MERGE_FIELDS) {
    if (k in source) {
      needMove = true;
      break;
    }
  }
  if (!needMove) {
    return;
  }

  const allOf = [];

  for (const k of MERGE_FIELDS) {
    if (!(k in source)) {
      continue;
    }
    const obj = {};
    obj[k] = source[k];
    delete source[k];
    allOf.push(obj);
  }

  const remaining = {};
  for (const k of Object.keys(source)) {
    if (k === 'allOf') {
      continue;
    }
    remaining[k] = source[k];
    delete source[k];
  }
  if ('type' in remaining) {
    allOf.push(remaining);
  }

  // don't need "allOf" if there is only one item in it.
  if (allOf.length === 1) {
    for (const k of Object.keys(allOf[0])) {
      source[k] = allOf[0][k];
    }
  } else {
    source.allOf = allOf;
  }
}

async function generate(name, schema, outputPath, preprocessedOutputPath) {
  // preprocess the schema to workaround https://github.com/bcherny/json-schema-to-typescript/issues/637 and https://github.com/bcherny/json-schema-to-typescript/issues/613
  const workingSet = [...Object.values(schema.$defs)];
  while (workingSet.length > 0) {
    const schema = workingSet.pop();
    if (typeof schema !== 'object') {
      continue;
    }

    if ('properties' in schema) {
      for (const property of Object.values(schema.properties)) {
        workingSet.push(property);
      }
    }

    if ('oneOf' in schema) {
      workingSet.push(...Object.values(schema.oneOf));
    }

    if ('anyOf' in schema) {
      workingSet.push(...Object.values(schema.anyOf));
    }

    markUnconstrained(schema);

    moveIntoAllOf(schema);

    // json schema draft-07 (used by json-schema-to-typescript) does not merge $ref, workaround
    // by putting the $ref and other fields in a `allOf`.
    if ('$ref' in schema && Object.keys(schema).length > 1) {
      const $ref = schema.$ref;
      const copy = {
        ...schema,
      };
      delete copy.$ref;

      for (const k of Object.keys(schema)) {
        delete schema[k];
      }

      if (
        Object.keys(copy).some(
          (k) => !['description', 'title', 'default'].includes(k),
        )
      ) {
        // At least one field is not metadata only field.
        const allOf = [copy, { $ref }];
        schema.allOf = allOf;
      } else {
        // All the remaining fields are metadata only fields. We cannot use `allOf` as the
        // metadata only branch will allow any types.
        schema.$ref = $ref;
      }
    }
  }

  const output = await compile(schema, name, { unreachableDefinitions: true });
  const fd = fs.openSync(outputPath, 'w');
  fs.writeSync(fd, output);
  fs.closeSync(fd);

  // `tsType` directs type generation only, and ajv rejects it as an unknown
  // keyword in strict mode, so keep it out of the schema used for validation.
  writeFileSync(
    preprocessedOutputPath,
    JSON.stringify(
      schema,
      (key, value) => (key === 'tsType' ? undefined : value),
      2,
    ),
  );
}

const schemaEnv = { ...process.env };
delete schemaEnv.BUILD_FRONTEND;

const apiSchema = execSync(
  'cargo run -p crossflow_diagram_editor --no-default-features -F json_schema --bin print_schema',
  {
    encoding: 'utf-8',
    stdio: 'pipe',
    env: schemaEnv,
  },
);

await generate(
  'DiagramEditorApi',
  JSON.parse(apiSchema),
  'frontend/types/api.d.ts',
  'frontend/api.preprocessed.schema.json',
);

execSync('biome format --write frontend/types/api.d.ts');
