import { fireEvent, render, screen } from '@testing-library/react';
import type { ScriptEnvironmentMetadata } from '../types/api';
import { ScriptEnvironmentPanel } from './script-environment-panel';

const metadata: ScriptEnvironmentMetadata = {
  language: 'python',
  interpreter: '3.13.5 (main, Jun 11 2026) [Clang 17.0.0]',
  config_schema: {},
  config_examples: [],
};

describe('ScriptEnvironmentPanel', () => {
  test('shows runtime details, config, and environment script', () => {
    render(
      <ScriptEnvironmentPanel
        environmentName="analysis"
        environment={{
          builder: 'process-bound-python',
          config: {
            ownership: 'persistent',
            script: 'import numpy as np',
          },
        }}
        metadata={metadata}
        onEdit={jest.fn()}
      />,
    );

    expect(screen.getByText('analysis')).toBeInTheDocument();
    expect(screen.getByText('process-bound-python')).toBeInTheDocument();
    expect(screen.getByText('python')).toBeInTheDocument();
    expect(screen.getByText(metadata.interpreter)).toBeInTheDocument();
    expect(screen.getByText(/"ownership": "persistent"/)).toBeInTheDocument();
    expect(screen.getByText('import numpy as np')).toBeInTheDocument();
  });

  test('opens environment editing from the panel', () => {
    const onEdit = jest.fn();
    render(
      <ScriptEnvironmentPanel
        environmentName="analysis"
        environment={{
          builder: 'process-bound-python',
          config: {},
        }}
        metadata={metadata}
        onEdit={onEdit}
      />,
    );

    fireEvent.click(screen.getByRole('button', { name: 'Edit' }));
    expect(onEdit).toHaveBeenCalledTimes(1);
  });

  test('shows an empty state when no environment is selected', () => {
    render(<ScriptEnvironmentPanel environmentName="" onEdit={jest.fn()} />);

    expect(
      screen.getByText(
        'Select an environment to inspect its runtime and configuration.',
      ),
    ).toBeInTheDocument();
  });
});
