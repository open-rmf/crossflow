import type { ComponentType } from 'react';
import { PythonPropertiesForm } from './python-form';

export type ScriptEnvironmentRegistry = {
  schemas?: Record<string, unknown>;
  scripting?: Record<string, { config_schema?: unknown }>;
};

export interface ScriptEnvironmentFormProps {
  /** The current raw JSON configuration string */
  config: string;
  /** Callback to update the raw JSON configuration string */
  onChange: (newConfig: string) => void;
  /** Propagates validation errors back to the parent dialog to toggle the Save button */
  onValidationError: (error: string | null) => void;
  /** The current dialog mode */
  mode: 'view' | 'edit' | 'create';
  /** The dynamic backend registry metadata */
  registry: ScriptEnvironmentRegistry;
  /** The current python/script text being edited in the main editor */
  scriptText: string;
}

export interface ScriptEnvironmentPlugin {
  /** Custom form component to render custom fields (e.g. ownership, credentials) */
  PropertiesForm: ComponentType<ScriptEnvironmentFormProps>;
  /** The default property that holds the code (e.g. 'script'). Hides the standard dropdown. */
  defaultCodeField?: string;
  /** Bootstraps a default config object string when a new environment is created */
  bootstrapConfig?: () => string;
}

export const scriptEnvironmentPlugins: Record<string, ScriptEnvironmentPlugin> =
  {
    'process-bound-python': {
      PropertiesForm: PythonPropertiesForm,
      defaultCodeField: 'script',
      bootstrapConfig: () =>
        JSON.stringify({ ownership: 'persistent', script: '' }, null, 2),
    },
  };
