import { fireEvent, screen, waitFor } from '@testing-library/react';
import { createOperationNode } from '../nodes';
import { render } from '../nodes/test-utils';
import type { DiagramElementMetadata, NodeMetadata } from '../types/api';
import NodeForm from './node-form';
import SchemaConfigForm from './schema-config-form';

const supportedSchema = {
  type: 'object',
  properties: {
    title: { type: 'string', description: 'Shown to operators.' },
    threshold: { type: 'number' },
    attempts: { type: 'integer' },
    enabled: { type: 'boolean' },
    strategy: { type: 'string', enum: ['fast', 'safe'] },
  },
  required: ['title', 'threshold', 'attempts', 'enabled', 'strategy'],
};

test('schema config form renders resolved properties', () => {
  render(
    <SchemaConfigForm
      schema={supportedSchema}
      definitions={{}}
      value={{ title: 'Direct' }}
      onChange={jest.fn()}
    />,
  );
  expect(screen.getByRole('textbox', { name: /title/ })).toHaveValue('Direct');
});

function createRegistry(
  configSchema: NodeMetadata['config_schema'],
  schemas: DiagramElementMetadata['schemas'] = {},
): DiagramElementMetadata {
  return {
    messages: [],
    nodes: {
      test_builder: {
        config_examples: [],
        config_schema: configSchema,
        default_display_text: 'Test node',
        request: 0,
        response: 0,
        streams: {},
      },
    },
    reverse_message_lookup: {
      result: [],
      split: [],
      unzip: [],
    },
    schemas,
    scripting: {},
    sections: {},
    trace_supported: false,
  };
}

function createNode(config?: unknown) {
  return createOperationNode(
    'root',
    undefined,
    { x: 0, y: 0 },
    {
      type: 'node',
      builder: 'test_builder',
      config,
      next: { builtin: 'dispose' },
    },
    'test_node',
  );
}

test('renders typed controls for a supported object schema and loads existing config', async () => {
  const registry = createRegistry(supportedSchema);

  render(
    <NodeForm
      node={createNode({
        title: 'Existing title',
        threshold: 1.5,
        attempts: 3,
        enabled: true,
        strategy: 'safe',
      })}
    />,
    registry,
  );

  expect(
    await screen.findByRole('button', { name: 'Edit raw JSON' }),
  ).toBeInTheDocument();
  expect(screen.getByRole('textbox', { name: /title/ })).toHaveValue(
    'Existing title',
  );
  expect(screen.getByText('Shown to operators.')).toBeInTheDocument();
  expect(screen.getByRole('spinbutton', { name: /threshold/ })).toHaveValue(
    1.5,
  );
  expect(screen.getByRole('spinbutton', { name: /attempts/ })).toHaveValue(3);
  expect(screen.getByRole('checkbox', { name: 'enabled' })).toBeChecked();
  expect(screen.getByRole('combobox', { name: /strategy/ })).toHaveTextContent(
    'safe',
  );
  expect(screen.queryByLabelText('Config')).not.toBeInTheDocument();
});

test('does not apply defaults merely by opening an existing config-less node', async () => {
  const onChange = jest.fn();
  const registry = createRegistry(
    { $ref: '#/$defs/TestConfig' },
    {
      TestConfig: {
        type: 'object',
        properties: {
          label: { type: 'string', default: 'New node' },
          retries: { type: 'integer', default: 2 },
          options: { $ref: '#/$defs/TestOptions' },
        },
      },
      TestOptions: {
        type: 'object',
        properties: {
          enabled: { type: 'boolean', default: true },
        },
      },
    },
  );

  render(<NodeForm node={createNode()} onChange={onChange} />, registry);

  await screen.findByRole('textbox', { name: /label/ });
  await waitFor(() => expect(onChange).not.toHaveBeenCalled());
});

test('applies defaults when a builder is selected for a new generic node', async () => {
  const onChange = jest.fn();
  const registry = createRegistry({
    type: 'object',
    properties: {
      label: { type: 'string', default: 'New node' },
    },
  });
  const node = createNode();
  node.data.op.builder = '';

  render(<NodeForm node={node} onChange={onChange} />, registry);
  const builder = await screen.findByRole('combobox', { name: /Builder/ });
  fireEvent.change(builder, { target: { value: 'test_builder' } });
  fireEvent.click(await screen.findByRole('option', { name: 'test_builder' }));

  const change = onChange.mock.calls.at(-1)?.[0];
  expect(change.item.data.op.builder).toBe('test_builder');
  expect(change.item.data.op.config).toEqual({ label: 'New node' });
});

test('groups object fields and writes nested config values', async () => {
  const onChange = jest.fn();
  const registry = createRegistry({
    type: 'object',
    properties: {
      options: {
        type: 'object',
        properties: {
          label: { type: 'string' },
        },
        required: ['label'],
      },
    },
    required: ['options'],
  });

  render(
    <NodeForm
      node={createNode({ options: { label: 'before' } })}
      onChange={onChange}
    />,
    registry,
  );

  expect(await screen.findByText('options')).toBeInTheDocument();
  fireEvent.change(screen.getByRole('textbox', { name: /label/ }), {
    target: { value: 'after' },
  });

  expect(onChange.mock.calls.at(-1)?.[0].item.data.op.config).toEqual({
    options: { label: 'after' },
  });
});

test('optional properties can be added and omitted', async () => {
  const onChange = jest.fn();
  const registry = createRegistry({
    type: 'object',
    properties: {
      required_text: { type: 'string' },
      optional_text: { type: 'string' },
    },
    required: ['required_text'],
  });

  const { rerender } = render(
    <NodeForm
      node={createNode({ required_text: 'required' })}
      onChange={onChange}
    />,
    registry,
  );

  const includeOptional = await screen.findByRole('checkbox', {
    name: 'Set optional_text',
  });
  expect(includeOptional).not.toBeChecked();
  expect(screen.getByRole('textbox', { name: 'optional_text' })).toBeDisabled();
  fireEvent.click(includeOptional);
  expect(onChange.mock.calls.at(-1)?.[0].item.data.op.config).toEqual({
    required_text: 'required',
    optional_text: '',
  });

  rerender(
    <NodeForm
      node={createNode({
        required_text: 'required',
        optional_text: 'present',
      })}
      onChange={onChange}
    />,
  );
  fireEvent.click(
    await screen.findByRole('checkbox', { name: 'Set optional_text' }),
  );
  expect(onChange.mock.calls.at(-1)?.[0].item.data.op.config).toEqual({
    required_text: 'required',
  });
});

test('control edits write the same JSON-compatible config object', async () => {
  const onChange = jest.fn();
  const registry = createRegistry(supportedSchema);
  const config = {
    title: 'Original',
    threshold: 1.5,
    attempts: 3,
    enabled: true,
    strategy: 'safe',
  };

  render(<NodeForm node={createNode(config)} onChange={onChange} />, registry);
  await screen.findByRole('button', { name: 'Edit raw JSON' });

  fireEvent.change(screen.getByRole('textbox', { name: /title/ }), {
    target: { value: 'Updated' },
  });
  expect(onChange.mock.calls.at(-1)?.[0].item.data.op.config).toEqual({
    ...config,
    title: 'Updated',
  });

  fireEvent.change(screen.getByRole('spinbutton', { name: /threshold/ }), {
    target: { value: '2.75' },
  });
  expect(onChange.mock.calls.at(-1)?.[0].item.data.op.config).toEqual({
    ...config,
    threshold: 2.75,
  });

  fireEvent.click(screen.getByRole('checkbox', { name: 'enabled' }));
  expect(onChange.mock.calls.at(-1)?.[0].item.data.op.config).toEqual({
    ...config,
    enabled: false,
  });

  fireEvent.mouseDown(screen.getByRole('combobox', { name: /strategy/ }));
  fireEvent.click(await screen.findByRole('option', { name: 'fast' }));
  expect(onChange.mock.calls.at(-1)?.[0].item.data.op.config).toEqual({
    ...config,
    strategy: 'fast',
  });
});

test('invalid numeric input is not committed to op.config', async () => {
  const onChange = jest.fn();
  const registry = createRegistry({
    type: 'object',
    properties: { count: { type: 'integer' } },
    required: ['count'],
  });

  render(
    <NodeForm node={createNode({ count: 4 })} onChange={onChange} />,
    registry,
  );
  const count = await screen.findByRole('spinbutton', { name: /count/ });
  onChange.mockClear();
  fireEvent.change(count, { target: { value: '4.5' } });
  fireEvent.change(count, { target: { value: 'not-a-number' } });
  expect(onChange).not.toHaveBeenCalled();
});

test.each([
  ['a boolean schema', true, {}],
  ['an unresolved ref', { $ref: '#/$defs/Missing' }, {}],
  [
    'an unsupported construct',
    {
      type: 'object',
      properties: { values: { type: 'array', items: { type: 'string' } } },
    },
    {},
  ],
  [
    'a recursive schema',
    { $ref: '#/$defs/Recursive' },
    {
      Recursive: {
        type: 'object',
        properties: { child: { $ref: '#/$defs/Recursive' } },
      },
    },
  ],
])('uses the raw JSON fallback for %s', async (_name, schema, schemas) => {
  render(
    <NodeForm node={createNode({ preserved: true })} />,
    createRegistry(schema, schemas),
  );
  expect(await screen.findByLabelText('Config')).toHaveValue(
    JSON.stringify({ preserved: true }),
  );
  expect(
    screen.queryByRole('button', { name: 'Edit raw JSON' }),
  ).not.toBeInTheDocument();
});

test('uses the raw JSON fallback when the builder is not registered', async () => {
  const registry = createRegistry(supportedSchema);
  registry.nodes = {};
  render(<NodeForm node={createNode({ preserved: true })} />, registry);
  expect(await screen.findByLabelText('Config')).toHaveValue(
    JSON.stringify({ preserved: true }),
  );
});

test('allows direct raw JSON editing for a supported schema', async () => {
  const onChange = jest.fn();
  render(
    <NodeForm node={createNode({ title: 'Generated' })} onChange={onChange} />,
    createRegistry({
      type: 'object',
      properties: { title: { type: 'string' } },
      required: ['title'],
    }),
  );

  fireEvent.click(await screen.findByRole('button', { name: 'Edit raw JSON' }));
  const rawConfig = screen.getByLabelText('Config');
  expect(rawConfig).toHaveValue(JSON.stringify({ title: 'Generated' }));
  fireEvent.change(rawConfig, {
    target: { value: '{"title":"Raw","extra":5}' },
  });
  expect(onChange.mock.calls.at(-1)?.[0].item.data.op.config).toEqual({
    title: 'Raw',
    extra: 5,
  });
  expect(
    screen.getByRole('button', { name: 'Use generated form' }),
  ).toBeInTheDocument();
});
