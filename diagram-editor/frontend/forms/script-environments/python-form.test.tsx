import { fireEvent, render, screen } from '@testing-library/react';
import { PythonPropertiesForm } from './python-form';

describe('PythonPropertiesForm', () => {
  const mockOnChange = jest.fn();
  const mockOnValidationError = jest.fn();
  const defaultRegistry = {
    scripting: {
      'process-bound-python': {
        config_schema: {
          properties: {
            ownership: {
              enum: ['shared', 'persistent', 'isolated'],
            },
          },
        },
      },
    },
  };

  beforeEach(() => {
    jest.clearAllMocks();
  });

  test('renders select field with current ownership value', () => {
    render(
      <PythonPropertiesForm
        config={JSON.stringify({ ownership: 'persistent', script: 'print(1)' })}
        onChange={mockOnChange}
        onValidationError={mockOnValidationError}
        mode="edit"
        registry={defaultRegistry}
        scriptText="print(1)"
      />,
    );

    const selectElement = screen.getByLabelText('Environment Ownership');
    expect(selectElement).toBeTruthy();
    expect(screen.getByText('Persistent')).toBeTruthy();
  });

  test('invokes onChange when ownership option is changed', async () => {
    render(
      <PythonPropertiesForm
        config={JSON.stringify({ ownership: 'persistent', script: 'print(1)' })}
        onChange={mockOnChange}
        onValidationError={mockOnValidationError}
        mode="edit"
        registry={defaultRegistry}
        scriptText="print(1)"
      />,
    );

    // Click on the select dropdown
    const selectElement = screen.getByRole('combobox');
    fireEvent.mouseDown(selectElement);

    // Select 'Isolated'
    const isolatedOption = screen.getByRole('option', { name: 'Isolated' });
    fireEvent.click(isolatedOption);

    expect(mockOnChange).toHaveBeenCalled();
    const parsedCall = JSON.parse(mockOnChange.mock.calls[0][0]);
    expect(parsedCall.ownership).toBe('isolated');
  });

  test('invokes onValidationError(null) on valid JSON config', () => {
    render(
      <PythonPropertiesForm
        config={JSON.stringify({ ownership: 'persistent', script: 'print(1)' })}
        onChange={mockOnChange}
        onValidationError={mockOnValidationError}
        mode="edit"
        registry={defaultRegistry}
        scriptText="print(1)"
      />,
    );

    expect(mockOnValidationError).toHaveBeenCalledWith(null);
  });

  test('invokes onValidationError with error on invalid JSON config', () => {
    render(
      <PythonPropertiesForm
        config="{ ownership: 'persistent' " // Missing closing brace and quote keys
        onChange={mockOnChange}
        onValidationError={mockOnValidationError}
        mode="edit"
        registry={defaultRegistry}
        scriptText="print(1)"
      />,
    );

    expect(mockOnValidationError).toHaveBeenCalledWith(expect.any(String));
  });

  test('extracts enum values dynamically from ref definitions in schema', () => {
    const complexRegistry = {
      scripting: {
        'process-bound-python': {
          config_schema: {
            properties: {
              ownership: {
                $ref: '#/definitions/PythonEnvironmentOwnership',
              },
            },
            definitions: {
              PythonEnvironmentOwnership: {
                enum: ['shared_custom', 'persistent_custom'],
              },
            },
          },
        },
      },
    };

    render(
      <PythonPropertiesForm
        config={JSON.stringify({ ownership: 'shared_custom', script: '' })}
        onChange={mockOnChange}
        onValidationError={mockOnValidationError}
        mode="edit"
        registry={complexRegistry}
        scriptText=""
      />,
    );

    const selectElement = screen.getByRole('combobox');
    fireEvent.mouseDown(selectElement);

    expect(screen.getByRole('option', { name: 'Shared_custom' })).toBeTruthy();
    expect(
      screen.getByRole('option', { name: 'Persistent_custom' }),
    ).toBeTruthy();
  });
});
