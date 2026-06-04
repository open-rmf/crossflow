import { useMemo, useEffect } from 'react';
import { MenuItem, TextField } from '@mui/material';
import { ScriptEnvironmentFormProps } from './registry';

export function PythonPropertiesForm({
  config,
  onChange,
  onValidationError,
  mode,
  registry,
  scriptText,
}: ScriptEnvironmentFormProps) {
  const ownershipOptions = useMemo(() => {
    const schema = registry?.scripting?.['process-bound-python']?.config_schema as any;
    const ownershipProp = schema?.properties?.ownership;
    const ref = ownershipProp?.$ref || ownershipProp?.anyOf?.[0]?.$ref;
    let enumValues: string[] = ['shared', 'persistent', 'isolated'];
    if (ref) {
      const defName = ref.split('/').pop();
      const def = schema?.definitions?.[defName] || registry?.schemas?.[defName];
      if (def?.enum) {
        enumValues = def.enum;
      }
    } else if (ownershipProp?.enum) {
      enumValues = ownershipProp.enum;
    }
    return enumValues;
  }, [registry]);

  const getOwnershipValue = () => {
    try {
      const obj = JSON.parse(config);
      return obj.ownership || 'persistent';
    } catch {
      return 'persistent';
    }
  };

  // Validate config is valid JSON
  useEffect(() => {
    try {
      JSON.parse(config);
      onValidationError(null);
    } catch (err) {
      onValidationError(err instanceof Error ? err.message : 'Invalid JSON');
    }
  }, [config, onValidationError]);

  const handleOwnershipChange = (newVal: string) => {
    try {
      const obj = JSON.parse(config) || {};
      obj.ownership = newVal;
      onChange(JSON.stringify(obj, null, 2));
    } catch {
      const obj = { ownership: newVal, script: scriptText };
      onChange(JSON.stringify(obj, null, 2));
    }
  };

  return (
    <TextField
      select={mode !== 'view'}
      label="Environment Ownership"
      value={getOwnershipValue()}
      onChange={(e) => handleOwnershipChange(e.target.value)}
      fullWidth
      disabled={mode === 'view'}
      sx={{ flex: 1 }}
    >
      {ownershipOptions.map((opt) => (
        <MenuItem key={opt} value={opt}>
          {opt.charAt(0).toUpperCase() + opt.slice(1)}
        </MenuItem>
      ))}
    </TextField>
  );
}
