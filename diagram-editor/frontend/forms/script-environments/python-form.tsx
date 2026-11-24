import type { RJSFSchema } from '@rjsf/utils';
import { useEffect, useMemo } from 'react';
import SchemaConfigForm from '../schema-config-form';
import type { ScriptEnvironmentFormProps } from './registry';

const fallback = {
  type: 'object',
  properties: {
    ownership: { type: 'string', enum: ['shared', 'persistent', 'isolated'] },
  },
};

export function PythonPropertiesForm({
  config,
  onChange,
  onValidationError,
  mode,
  registry,
  scriptText,
}: ScriptEnvironmentFormProps) {
  const parsed = useMemo(() => {
    try {
      return { value: JSON.parse(config), error: null };
    } catch (error) {
      return { value: { script: scriptText }, error: String(error) };
    }
  }, [config, scriptText]);
  useEffect(
    () => onValidationError(parsed.error),
    [parsed.error, onValidationError],
  );
  const schema = registry?.scripting?.['process-bound-python']?.config_schema as
    | RJSFSchema
    | undefined;
  return (
    <SchemaConfigForm
      schema={schema ?? fallback}
      definitions={registry?.schemas}
      value={{ ownership: 'persistent', ...parsed.value }}
      onChange={(value) => onChange(JSON.stringify(value, null, 2))}
      disabled={mode === 'view'}
      uiSchema={{
        'ui:label': false,
        ownership: { 'ui:title': 'Environment Ownership' },
        script: { 'ui:widget': 'hidden' },
      }}
    />
  );
}
